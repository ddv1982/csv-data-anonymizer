use crate::error::{AnonymizerError, Result, csv_error};
use crate::file_ops::replace_file_atomically;
use crate::process_control::{check_canceled, report_progress};
use crate::sampling::SpreadSampler;
use crate::strategies::{TransformState, transform_row_with_state};
use crate::types::{ColumnMetadata, ParsedSample, ProcessControl, ProcessOptions, ProcessResult};
use csv::{ReaderBuilder, StringRecord, Trim, WriterBuilder};
use std::borrow::Cow;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Gate every input file passes before any reader opens it: it exists, it is a
/// file, and it is text this crate can actually parse.
///
/// The encoding check lives here rather than at each `from_path` call because
/// this is the one place all input paths already share — the detection sample,
/// the row count, the transform and the smart-replacement value scan all call it
/// first. A per-reader check would have to be repeated four times and would be
/// silently skipped the next time a reader is added.
pub fn validate_file(file_path: &Path) -> Result<()> {
    let metadata = fs::metadata(file_path)
        .map_err(|_| AnonymizerError::FileNotFound(file_path.to_path_buf()))?;
    if !metadata.is_file() {
        return Err(AnonymizerError::FileNotFound(file_path.to_path_buf()));
    }
    reject_unsupported_encoding(file_path)
}

/// How much of the head of a file the encoding sniffer looks at.
///
/// Bounded so opening a multi-gigabyte export costs one small read, and large
/// enough that the byte-density rule below has a meaningful sample even when the
/// header row alone is short.
const ENCODING_SNIFF_BYTES: usize = 8 * 1024;

/// A prefix has to be at least this long before the density rule is trusted.
///
/// Two bytes of "NULs are 50% of the file" is noise; a short header row is not.
const MIN_DENSITY_SAMPLE_BYTES: usize = 16;

/// Fraction of sniffed bytes that must be NUL before the input is called UTF-16.
///
/// UTF-16-encoded ASCII is very close to 50% NUL. The threshold sits well under
/// that so a file with some non-Latin text still trips it, and well over what any
/// real UTF-8 CSV can reach — a UTF-8 CSV's NUL rate is zero, not merely low.
const UTF16_NUL_RATIO_PERCENT: usize = 20;

/// Fraction of a prefix's NUL bytes that must share one offset parity for the
/// input to be called UTF-16 rather than merely binary.
///
/// UTF-16LE ASCII puts every NUL at an odd offset and UTF-16BE at an even one, so
/// a real UTF-16 text file scores 100 here. A binary blob's NULs land wherever
/// the format put them and score near 50, which is what keeps the two verdicts —
/// and their two different remedies — from being confused for each other.
const UTF16_PARITY_CONCENTRATION_PERCENT: usize = 90;

/// An input the CSV parser must not be handed, and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnsupportedEncoding {
    /// UTF-16, byte order named so the message can tell the user what they have.
    Utf16 { little_endian: bool },
    /// NUL-dense but not shaped like UTF-16: not text at all.
    Binary,
}

impl UnsupportedEncoding {
    /// The single wording both the analyze path and the transform path show,
    /// because they both reach it through [`validate_file`].
    fn message(self) -> String {
        match self {
            Self::Utf16 { little_endian } => {
                let byte_order = if little_endian { "LE" } else { "BE" };
                format!(
                    "this file is UTF-16{byte_order} text, not UTF-8, so it cannot be read as \
                     CSV. Re-save it as UTF-8 and run it again — in Excel choose \"CSV UTF-8\", in \
                     PowerShell add `-Encoding utf8` to Out-File or Set-Content, and with bcp use \
                     -c rather than -w. Converting it here is deliberately refused: a wrongly \
                     guessed encoding produces values that look plausible but are wrong, and this \
                     tool will not publish an output it cannot vouch for."
                )
            }
            Self::Binary => "this file contains NUL bytes, so it is a binary file rather than \
                             text, and cannot be read as CSV. Export the data to a UTF-8 CSV file \
                             and run that instead."
                .to_string(),
        }
    }
}

/// Refuses inputs whose bytes are not UTF-8 CSV text.
///
/// Prevents the worst failure this tool has: a BOM-less UTF-16 export — what
/// PowerShell's `Out-File`/`Set-Content` and SQL Server's `bcp -w` produce by
/// default — is *valid UTF-8* once its NULs are read as ordinary characters, so
/// nothing errors. The headers parse as `n\0a\0m\0e\0`, no detector matches them,
/// and a file full of names and email addresses is reported as holding no
/// sensitive data. Refusing loudly is the only safe answer; UTF-16 *with* a BOM
/// already failed, but with an unactionable "invalid utf-8" and it is routed here
/// too so both spellings say the same thing.
fn reject_unsupported_encoding(file_path: &Path) -> Result<()> {
    let prefix = read_file_prefix(file_path, ENCODING_SNIFF_BYTES)?;
    match sniff_unsupported_encoding(&prefix) {
        Some(encoding) => Err(AnonymizerError::csv_parse(encoding.message(), None)),
        None => Ok(()),
    }
}

/// Reads at most `limit` bytes from the head of `file_path`.
///
/// Only `NotFound` becomes [`AnonymizerError::FileNotFound`]. Every other open error keeps
/// its own kind, because the one that actually reaches a user here is `PermissionDenied` —
/// a file they can see and the app cannot read — and telling them it does not exist sends
/// them looking for the wrong problem. The caller checked existence a moment ago, so a
/// `NotFound` at this point means the file went away mid-run, which is what that variant says.
fn read_file_prefix(file_path: &Path, limit: usize) -> Result<Vec<u8>> {
    let file = fs::File::open(file_path).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => AnonymizerError::FileNotFound(file_path.to_path_buf()),
        _ => AnonymizerError::from(error),
    })?;
    let mut prefix = Vec::new();
    file.take(limit as u64).read_to_end(&mut prefix)?;
    Ok(prefix)
}

/// Classifies a sniffed prefix, or returns `None` for anything that may be CSV.
///
/// `None` is the answer that must never be wrong in the refusing direction, so
/// every rule keys on NUL bytes: a UTF-8 CSV cannot contain one — U+0000 is not
/// produced by any exporter and is not a legal character in a CSV field anyone
/// means to write — while UTF-16 text is roughly half NULs by construction. The
/// small allowance below the binary threshold exists so a file that somehow
/// carries a stray control byte still gets parsed and reported on rather than
/// rejected out of hand.
fn sniff_unsupported_encoding(prefix: &[u8]) -> Option<UnsupportedEncoding> {
    // A BOM is a declaration, not a guess, so it outranks the density rules and
    // catches even a UTF-16 file whose sniffed prefix happens to be NUL-poor.
    match prefix {
        [0xff, 0xfe, ..] => {
            return Some(UnsupportedEncoding::Utf16 {
                little_endian: true,
            });
        }
        [0xfe, 0xff, ..] => {
            return Some(UnsupportedEncoding::Utf16 {
                little_endian: false,
            });
        }
        _ => {}
    }

    let nul_count = prefix.iter().filter(|byte| **byte == 0).count();
    if nul_count == 0 {
        return None;
    }

    let at_odd_offsets = prefix
        .iter()
        .enumerate()
        .filter(|(offset, byte)| **byte == 0 && offset % 2 == 1)
        .count();
    let dominant_parity = at_odd_offsets.max(nul_count - at_odd_offsets);

    let dense_enough = prefix.len() >= MIN_DENSITY_SAMPLE_BYTES
        && nul_count * 100 >= prefix.len() * UTF16_NUL_RATIO_PERCENT;
    let aligned_enough = dominant_parity * 100 >= nul_count * UTF16_PARITY_CONCENTRATION_PERCENT;

    if dense_enough && aligned_enough {
        // UTF-16LE holds ASCII as `text, NUL`, so its NULs sit at odd offsets;
        // UTF-16BE is the mirror image.
        return Some(UnsupportedEncoding::Utf16 {
            little_endian: at_odd_offsets >= nul_count - at_odd_offsets,
        });
    }

    // Not UTF-16-shaped, but NUL-heavy enough that no CSV export explains it:
    // more than one percent of the prefix, and never on the strength of a single
    // byte, so one stray control character in an otherwise readable file is
    // parsed and reported on instead of refused.
    if nul_count >= 2 && nul_count * 100 > prefix.len() {
        return Some(UnsupportedEncoding::Binary);
    }

    None
}

/// Which data rows survive when the input holds more rows than the caller wants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SampleWindow {
    /// Keep the first `row_count` data rows and stop reading. Display windows
    /// want the input's opening rows, not a representative spread.
    Head,
    /// Read every data row and keep `row_count` of them drawn from across the
    /// whole input. Detection must use this: the transform streams every row, so
    /// a head window would leave detection blind to PII that only starts partway
    /// down the input, and unseen PII is never auto-selected.
    Spread,
}

/// Reads the header row plus the input's first `row_count` data rows.
///
/// Only for display and for reading back small outputs. Anything that feeds
/// detection must use [`read_detection_sample`] instead.
pub fn read_sample(file_path: &Path, row_count: usize) -> Result<ParsedSample> {
    read_file_sample(file_path, row_count, SampleWindow::Head)
}

/// Reads the header row plus a `row_count`-row sample of the whole file, and
/// reports the file's exact data-row count.
///
/// The sample is drawn from every part of the file rather than from a window of
/// it — see `RowSampler` in this module for how, and for why the choice of rows
/// has to be pseudorandom. Costs one streaming pass; memory stays bounded by
/// `row_count` rows.
pub fn read_detection_sample(file_path: &Path, row_count: usize) -> Result<ParsedSample> {
    read_file_sample(file_path, row_count, SampleWindow::Spread)
}

fn read_file_sample(
    file_path: &Path,
    row_count: usize,
    window: SampleWindow,
) -> Result<ParsedSample> {
    validate_file(file_path)?;

    let mut reader = ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .trim(Trim::All)
        .from_path(file_path)
        .map_err(csv_error)?;

    read_sample_from_csv_reader(&mut reader, row_count, window)
}

/// CSV-text counterpart of [`read_sample`]: the first `row_count` data rows.
pub fn read_csv_sample_from_str(input: &str, row_count: usize) -> Result<ParsedSample> {
    read_str_sample(input, row_count, SampleWindow::Head)
}

/// CSV-text counterpart of [`read_detection_sample`]: a `row_count`-row sample of
/// the whole input, plus the exact data-row count.
pub fn read_csv_detection_sample_from_str(input: &str, row_count: usize) -> Result<ParsedSample> {
    read_str_sample(input, row_count, SampleWindow::Spread)
}

fn read_str_sample(input: &str, row_count: usize, window: SampleWindow) -> Result<ParsedSample> {
    let mut reader = ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .trim(Trim::All)
        .from_reader(input.as_bytes());

    read_sample_from_csv_reader(&mut reader, row_count, window)
}

/// Collects `wanted` data rows under a [`SampleWindow`], over a [`SpreadSampler`].
///
/// The sampler is the whole rule; this adds only the early stop, which is specific
/// to reading a file: once a head window is full, nothing later can enter it, so
/// there is no reason to keep parsing rows.
struct RowSampler {
    window: SampleWindow,
    rows: SpreadSampler<Vec<String>>,
    strict_anchor_rows: Vec<Vec<String>>,
    anchored_columns: Vec<bool>,
    capacity: usize,
}

impl RowSampler {
    fn new(window: SampleWindow, wanted: usize) -> Self {
        // A zero-row sample carries no information; one row is the floor.
        let wanted = wanted.max(1);
        Self {
            window,
            strict_anchor_rows: Vec::new(),
            anchored_columns: Vec::new(),
            capacity: wanted,
            rows: match window {
                SampleWindow::Head => SpreadSampler::head(wanted),
                SampleWindow::Spread => SpreadSampler::spread(wanted),
            },
        }
    }

    /// Offers one data row. Returns `false` once the caller can stop reading,
    /// which only happens for a head window that has filled its buffer.
    fn push(&mut self, row: Vec<String>) -> bool {
        if self.window == SampleWindow::Head && self.rows.is_full() {
            return false;
        }
        if self.window == SampleWindow::Spread {
            self.record_strict_anchor(&row);
        }
        self.rows.push(row);
        true
    }

    /// Keeps at most one row per column containing validator-backed High-confidence
    /// privacy evidence. The ordinary spread sample remains the statistical basis;
    /// these anchors only ensure a rare strict identifier encountered during the
    /// same complete streaming pass cannot disappear because its row lost the
    /// sampling lottery.
    fn record_strict_anchor(&mut self, row: &[String]) {
        if self.strict_anchor_rows.len() >= self.capacity {
            return;
        }
        if self.anchored_columns.len() < row.len() {
            self.anchored_columns.resize(row.len(), false);
        }
        let mut matched_new_column = false;
        for (column, value) in row.iter().enumerate() {
            if self.anchored_columns[column] || crate::detection::is_empty_value(value) {
                continue;
            }
            if crate::detection::collect_privacy_spans(value)
                .iter()
                .any(|span| span.confidence == crate::types::Confidence::High)
            {
                self.anchored_columns[column] = true;
                matched_new_column = true;
            }
        }
        if matched_new_column {
            self.strict_anchor_rows.push(row.to_vec());
        }
    }

    fn scanned(&self) -> usize {
        self.rows.offered()
    }

    fn into_rows(self) -> Vec<Vec<String>> {
        let mut rows = self.rows.into_items();
        for anchor in self.strict_anchor_rows {
            if rows.iter().any(|row| row == &anchor) {
                continue;
            }
            // Anchors are supplemental evidence. Evicting spread rows here lets a
            // wide file fill the entire statistical sample with exceptional rows,
            // distorting classification of every unrelated column.
            rows.push(anchor);
        }
        rows
    }
}

fn read_sample_from_csv_reader<R: Read>(
    reader: &mut csv::Reader<R>,
    row_count: usize,
    window: SampleWindow,
) -> Result<ParsedSample> {
    let mut headers: Vec<String> = Vec::new();
    let mut sampler = RowSampler::new(window, row_count);
    let mut stopped_early = false;

    for result in reader.records() {
        let record = result.map_err(csv_error)?;
        let mut row = record_to_vec(&record);

        if headers.is_empty() {
            if let Some(first) = row.first_mut() {
                *first = strip_bom(first).to_string();
            }
            if row.is_empty() {
                return Err(AnonymizerError::csv_parse(
                    "CSV file is empty or has no valid headers",
                    None,
                ));
            }
            headers = row;
            continue;
        }

        if is_blank_data_row(&row) {
            continue;
        }

        row = normalize_data_row(row, headers.len(), record.position().map(|pos| pos.line()))?;

        if !sampler.push(row) {
            stopped_early = true;
            break;
        }
    }

    if headers.is_empty() {
        return Err(AnonymizerError::csv_parse(
            "CSV file is empty or has no valid headers",
            None,
        ));
    }

    Ok(ParsedSample {
        headers,
        data_rows_scanned: sampler.scanned(),
        scanned_entire_input: !stopped_early,
        rows: sampler.into_rows(),
    })
}

/// Counts data rows without keeping any of them.
///
/// Only for callers that want the count alone; anything that also needs values
/// gets the count for free from [`read_detection_sample`].
pub fn count_csv_data_rows(file_path: &Path) -> Result<usize> {
    validate_file(file_path)?;
    let mut reader = ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .trim(Trim::All)
        .from_path(file_path)
        .map_err(csv_error)?;

    let mut header_processed = false;
    let mut row_count = 0;

    for result in reader.records() {
        let record = result.map_err(csv_error)?;
        let row = record_to_vec(&record);
        if !header_processed {
            header_processed = true;
            continue;
        }
        if is_blank_data_row(&row) {
            continue;
        }
        row_count += 1;
    }

    Ok(row_count)
}

pub fn process_csv_data(
    input: &str,
    columns: &[ColumnMetadata],
    options: ProcessOptions<'_>,
) -> Result<(String, ProcessResult)> {
    let start_time = Instant::now();
    let mut reader = ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(input.as_bytes());
    let mut writer = WriterBuilder::new()
        .has_headers(false)
        .from_writer(Vec::new());
    let result = process_csv_reader_to_writer(
        &mut reader,
        &mut writer,
        columns,
        options,
        None,
        PathBuf::new(),
        start_time,
    )?;
    let bytes = writer
        .into_inner()
        .map_err(|error| AnonymizerError::csv_parse(error.to_string(), None))?;
    let output = String::from_utf8(bytes)
        .map_err(|error| AnonymizerError::csv_parse(error.to_string(), None))?;

    Ok((output, result))
}

/// [`process_file_with_control`] with no cancellation handle, for tests that never cancel.
///
/// Test-only: the service always has a control to pass, because a desktop run has to stay
/// cancellable.
#[cfg(test)]
pub(crate) fn process_file(
    input_path: &Path,
    output_path: &Path,
    columns: &[ColumnMetadata],
    options: ProcessOptions<'_>,
) -> Result<ProcessResult> {
    process_file_with_control(input_path, output_path, columns, options, None)
}

/// [`process_file_with_control_and_overwrite`] that refuses to overwrite.
///
/// Test-only. Production reaches the overwrite-aware form directly, because whether an
/// existing output may be replaced is the user's answer to a dialog, never a default.
#[cfg(test)]
pub(crate) fn process_file_with_control(
    input_path: &Path,
    output_path: &Path,
    columns: &[ColumnMetadata],
    options: ProcessOptions<'_>,
    control: Option<&mut ProcessControl<'_>>,
) -> Result<ProcessResult> {
    process_file_with_control_and_overwrite(
        input_path,
        output_path,
        columns,
        options,
        control,
        true,
    )
}

pub(crate) fn process_file_with_control_and_overwrite(
    input_path: &Path,
    output_path: &Path,
    columns: &[ColumnMetadata],
    options: ProcessOptions<'_>,
    control: Option<&mut ProcessControl<'_>>,
    overwrite: bool,
) -> Result<ProcessResult> {
    validate_file(input_path)?;
    let start_time = Instant::now();
    let mut result = replace_file_atomically(output_path, overwrite, |temporary_output_path| {
        process_file_to_temporary_output(
            input_path,
            temporary_output_path,
            columns,
            options,
            control,
            start_time,
        )
    })?;
    result.output_path = output_path.to_path_buf();
    Ok(result)
}

fn process_file_to_temporary_output(
    input_path: &Path,
    temporary_output_path: &Path,
    columns: &[ColumnMetadata],
    options: ProcessOptions<'_>,
    control: Option<&mut ProcessControl<'_>>,
    start_time: Instant,
) -> Result<ProcessResult> {
    let mut reader = ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_path(input_path)
        .map_err(csv_error)?;
    let mut writer = WriterBuilder::new()
        .has_headers(false)
        .from_path(temporary_output_path)
        .map_err(csv_error)?;

    process_csv_reader_to_writer(
        &mut reader,
        &mut writer,
        columns,
        options,
        control,
        temporary_output_path.to_path_buf(),
        start_time,
    )
}

fn process_csv_reader_to_writer<R: Read, W: Write>(
    reader: &mut csv::Reader<R>,
    writer: &mut csv::Writer<W>,
    columns: &[ColumnMetadata],
    options: ProcessOptions<'_>,
    mut control: Option<&mut ProcessControl<'_>>,
    output_path: PathBuf,
    start_time: Instant,
) -> Result<ProcessResult> {
    let mut header_processed = false;
    let mut header_len = 0;
    let mut row_count = 0;
    let mut transform_state = options.smart_replacements.cloned().map_or_else(
        TransformState::new,
        TransformState::with_smart_replacements_if_active,
    );
    if options.tokenization_key.is_some() {
        transform_state = transform_state.with_tokenization_key(options.tokenization_key.cloned());
    }
    // Resolving installed memory refreshes operating-system state. Do it once per
    // transform, never once per row.
    let mapping_entry_ceiling = options
        .mapping_entry_ceiling
        .unwrap_or_else(TransformState::runtime_mapping_entry_ceiling);

    check_canceled(&mut control)?;

    for result in reader.records() {
        let record = result.map_err(csv_error)?;
        let mut row = record_to_vec(&record);
        check_canceled(&mut control)?;

        if !header_processed {
            if let Some(first) = row.first_mut() {
                *first = strip_bom(first).to_string();
            }
            header_len = row.len();
            write_csv_output_record(writer, row.iter().map(String::as_str))?;
            header_processed = true;
            continue;
        }

        row = normalize_data_row(row, header_len, record.position().map(|pos| pos.line()))?;

        if is_blank_data_row(&row) {
            write_csv_output_record(writer, row.iter().map(String::as_str))?;
            continue;
        }

        let transformed_row =
            transform_row_with_state(&row, columns, row_count, &mut transform_state);
        // After the transform, because that is when the mapping grows, and before the
        // write, so a refused run leaves nothing half-written: every file run lands
        // through `replace_file_atomically`, which discards the temporary file and
        // leaves the destination untouched on any `Err`. Without this call the ceiling
        // in `TransformState` would be unreachable code and the process would still be
        // OOM-killed, so this line is the whole guard.
        transform_state.check_mapping_budget_against(mapping_entry_ceiling)?;
        write_csv_output_record(writer, transformed_row.iter().map(String::as_str))?;
        row_count += 1;
        report_progress(&mut control, row_count);
    }

    check_canceled(&mut control)?;
    writer.flush()?;

    Ok(ProcessResult {
        row_count,
        output_path,
        duration_ms: start_time.elapsed().as_millis(),
        transform_report: transform_state.report(),
    })
}

pub(crate) fn record_to_vec(record: &StringRecord) -> Vec<String> {
    record.iter().map(ToString::to_string).collect()
}

pub(crate) fn normalize_data_row(
    mut row: Vec<String>,
    header_len: usize,
    row_number: Option<u64>,
) -> Result<Vec<String>> {
    if row.len() > header_len {
        let extra_count = row.len() - header_len;
        if row[header_len..]
            .iter()
            .any(|value| !value.trim().is_empty())
        {
            return Err(AnonymizerError::csv_parse(
                format!(
                    "CSV privacy error: row contains {extra_count} non-header field(s); non-empty data beyond the header cannot be safely modeled or written"
                ),
                row_number,
            ));
        }
        row.truncate(header_len);
    }

    if row.len() < header_len {
        row.resize(header_len, String::new());
    }

    Ok(row)
}

pub(crate) fn write_csv_output_record<'a, W: Write>(
    writer: &mut csv::Writer<W>,
    record: impl IntoIterator<Item = &'a str>,
) -> Result<()> {
    let neutralized = record
        .into_iter()
        .map(neutralize_spreadsheet_formula)
        .collect::<Vec<_>>();
    writer
        .write_record(neutralized.iter().map(|value| value.as_ref()))
        .map_err(csv_error)
}

pub(crate) fn neutralize_spreadsheet_formula(value: &str) -> Cow<'_, str> {
    if could_be_spreadsheet_formula(value) {
        Cow::Owned(format!("'{value}"))
    } else {
        Cow::Borrowed(value)
    }
}

fn could_be_spreadsheet_formula(value: &str) -> bool {
    let Some(first) = value.chars().next() else {
        return false;
    };

    // Plain signed numbers ("-42.50") are parsed by spreadsheets as numbers,
    // never as formulas; neutralizing them would corrupt untouched numeric data.
    if is_strict_signed_number(value) {
        return false;
    }

    if is_spreadsheet_formula_prefix(first) || matches!(first, '\t' | '\r' | '\n') {
        return true;
    }

    if first.is_whitespace() {
        return value
            .trim_start_matches(char::is_whitespace)
            .chars()
            .next()
            .is_some_and(is_spreadsheet_formula_prefix);
    }

    false
}

fn is_strict_signed_number(value: &str) -> bool {
    let trimmed = value.trim();
    let unsigned = trimmed.strip_prefix(['-', '+']).unwrap_or(trimmed);
    if unsigned.is_empty() || unsigned == trimmed {
        // Only sign-prefixed values need the exemption; everything else keeps
        // the existing prefix-based neutralization decision.
        return false;
    }
    let mut decimal_point_seen = false;
    unsigned.chars().all(|character| {
        if character == '.' {
            if decimal_point_seen {
                return false;
            }
            decimal_point_seen = true;
            return true;
        }
        character.is_ascii_digit()
    })
}

fn is_spreadsheet_formula_prefix(character: char) -> bool {
    matches!(
        character,
        '=' | '+' | '-' | '@' | '\u{ff1d}' | '\u{ff0b}' | '\u{ff0d}' | '\u{ff20}'
    )
}

/// The one definition of "this row carries no data", used by every reader here.
///
/// It trims rather than testing `is_empty`, so it holds whether or not the reader was built
/// with [`Trim::All`] — the sampling readers are, the processing ones are not. The two must
/// agree: [`count_csv_data_rows`] produces the denominator of the detection-coverage figure
/// and [`read_detection_sample`] the numerator, so a reader-dependent predicate could report
/// coverage of more rows than the file has, which reads as more scrutiny than the data got.
fn is_blank_data_row(row: &[String]) -> bool {
    row.iter().all(|value| value.trim().is_empty())
}

pub(crate) fn strip_bom(value: &str) -> &str {
    value.strip_prefix('\u{feff}').unwrap_or(value)
}

#[cfg(test)]
mod tests;
