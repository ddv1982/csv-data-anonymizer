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

pub fn validate_file(file_path: &Path) -> Result<()> {
    let metadata = fs::metadata(file_path)
        .map_err(|_| AnonymizerError::FileNotFound(file_path.to_path_buf()))?;
    if !metadata.is_file() {
        return Err(AnonymizerError::FileNotFound(file_path.to_path_buf()));
    }
    Ok(())
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
/// it — see [`RowSampler`] for how, and for why the choice of rows has to be
/// pseudorandom. Costs one streaming pass; memory stays bounded by `row_count`
/// rows.
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
}

impl RowSampler {
    fn new(window: SampleWindow, wanted: usize) -> Self {
        // A zero-row sample carries no information; one row is the floor.
        let wanted = wanted.max(1);
        Self {
            window,
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
        self.rows.push(row);
        true
    }

    fn scanned(&self) -> usize {
        self.rows.offered()
    }

    fn into_rows(self) -> Vec<Vec<String>> {
        self.rows.into_items()
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

        if row.iter().all(|value| value.is_empty()) {
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
        if row.iter().all(|value| value.is_empty()) {
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

pub fn process_csv_text(
    input: &str,
    columns: &[ColumnMetadata],
    options: ProcessOptions<'_>,
) -> Result<(String, ProcessResult)> {
    process_csv_data(input, columns, options)
}

pub fn process_file(
    input_path: &Path,
    output_path: &Path,
    columns: &[ColumnMetadata],
    options: ProcessOptions<'_>,
) -> Result<ProcessResult> {
    process_file_with_control(input_path, output_path, columns, options, None)
}

pub fn process_file_with_control(
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
    let mut transform_state = match options.smart_replacements {
        Some(smart_replacements) => {
            TransformState::with_smart_replacements(smart_replacements.clone())
        }
        None => TransformState::new(),
    };

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
        write_csv_output_record(writer, transformed_row.iter().map(String::as_str))?;
        row_count += 1;
        report_progress(&mut control, row_count);
    }

    check_canceled(&mut control)?;
    writer.flush()?;

    Ok(ProcessResult {
        row_count,
        success: true,
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

fn is_blank_data_row(row: &[String]) -> bool {
    row.iter().all(|value| value.trim().is_empty())
}

pub(crate) fn strip_bom(value: &str) -> &str {
    value.strip_prefix('\u{feff}').unwrap_or(value)
}

#[cfg(test)]
mod tests;
