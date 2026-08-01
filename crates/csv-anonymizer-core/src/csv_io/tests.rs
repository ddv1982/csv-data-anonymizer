use super::*;
use crate::error::AnonymizerError;
use crate::metadata::{apply_column_selection, build_column_metadata};
use crate::types::ProcessProgress;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

#[test]
fn reads_sample_and_strips_bom() {
    let sample = read_sample(&fixture("bom-file.csv"), 10).unwrap();
    assert_eq!(sample.headers[0], "id");
}

#[test]
fn head_sample_keeps_the_opening_rows_and_reports_it_stopped_early() {
    let content = build_numbered_csv(50);
    let sample = read_csv_sample_from_str(&content, 10).unwrap();

    assert_eq!(sample.rows.len(), 10);
    assert_eq!(sample.rows[0][0], "0");
    assert_eq!(sample.rows[9][0], "9");
    assert!(!sample.scanned_entire_input);
}

/// What this layer owes on top of the sampler: the row bookkeeping, and input order.
///
/// The sampler's own statistical properties — that it draws from every part of the
/// stream, that it does not align with a periodic one, that it is deterministic — are
/// pinned once in [`crate::sampling`] against `SpreadSampler` directly, which is where
/// they live now that `read_csv_detection_sample_from_str` delegates to it. Repeating
/// them here would restate the same arithmetic through a CSV parser and would fail for
/// the same reason, one layer further from the cause.
///
/// `data_rows_scanned` and `scanned_entire_input` are not the sampler's, though: they
/// are what tells detection whether it read the file or a sample of it, and nothing in
/// `sampling` can check them. Input order is asserted here as well, because it is the
/// property this function's callers index against — the preview reads row `n` of the
/// sample and calls it row `n` of the file.
#[test]
fn detection_sample_spans_the_whole_input_in_input_order() {
    let content = build_numbered_csv(1_000);
    let sample = read_csv_detection_sample_from_str(&content, 100).unwrap();

    assert_eq!(sample.rows.len(), 100);
    assert_eq!(sample.data_rows_scanned, 1_000);
    assert!(sample.scanned_entire_input);

    let kept = kept_row_numbers(&sample);
    assert!(
        kept.windows(2).all(|pair| pair[0] < pair[1]),
        "the sample must come back in input order, got {kept:?}"
    );
}

fn kept_row_numbers(sample: &ParsedSample) -> Vec<usize> {
    sample
        .rows
        .iter()
        .map(|row| row[0].parse().unwrap())
        .collect()
}

#[test]
fn detection_sample_keeps_every_row_of_a_short_input() {
    let content = build_numbered_csv(20);
    let sample = read_csv_detection_sample_from_str(&content, 100).unwrap();

    assert_eq!(sample.rows.len(), 20);
    assert_eq!(sample.data_rows_scanned, 20);
    assert!(sample.scanned_entire_input);
}

/// The kept count must be exactly what the caller asked for, at any input length.
/// Detection votes on match ratios, so a sample that quietly varies in size with
/// where the input's length happens to fall is a vote of varying and unstated
/// confidence. An earlier thinning sampler returned between half the request and
/// all of it for that reason — 200 rows yielded 50, a million yielded 62.
#[test]
fn detection_sample_keeps_exactly_the_requested_row_count() {
    const WANTED: usize = 100;

    for row_count in [150, 200, 300, 512, 1_000, 4_097, 10_500, 65_536] {
        let sample = read_csv_detection_sample_from_str(&build_numbered_csv(row_count), WANTED)
            .expect("sample reads");

        assert_eq!(
            sample.rows.len(),
            WANTED,
            "a {row_count}-row input yielded {} of {WANTED} requested rows",
            sample.rows.len()
        );
        assert_eq!(sample.data_rows_scanned, row_count);
    }
}

#[test]
fn strict_anchors_are_supplemental_to_the_statistical_sample() {
    let mut sampler = RowSampler::new(SampleWindow::Spread, 2);
    sampler.push(vec!["ordinary-a".to_string()]);
    sampler.push(vec!["ordinary-b".to_string()]);

    // Model a strict identifier discovered after the spread sample is full. The
    // anchor must add evidence without evicting either representative row.
    sampler
        .strict_anchor_rows
        .push(vec!["rare.person@example.com".to_string()]);

    let rows = sampler.into_rows();
    assert_eq!(rows.len(), 3);
    assert!(rows.contains(&vec!["ordinary-a".to_string()]));
    assert!(rows.contains(&vec!["ordinary-b".to_string()]));
    assert!(rows.contains(&vec!["rare.person@example.com".to_string()]));
}

fn build_numbered_csv(row_count: usize) -> String {
    let mut content = String::from("n\n");
    for row in 0..row_count {
        content.push_str(&format!("{row}\n"));
    }
    content
}

#[test]
fn reads_csv_sample_from_str() {
    let sample = read_csv_sample_from_str("email\nada@example.com\n", 10).unwrap();

    assert_eq!(sample.headers, vec!["email"]);
    assert_eq!(sample.rows, vec![vec!["ada@example.com"]]);
}

#[test]
fn processes_csv_text() {
    let input = "email\nada@example.com\n";
    let sample = read_csv_sample_from_str(input, 10).unwrap();
    let columns =
        apply_column_selection(&build_column_metadata(&sample.headers, &sample.rows), &[0]);
    let (output, result) = process_csv_data(
        input,
        &columns,
        ProcessOptions {
            smart_replacements: None,
            tokenization_key: None,
            mapping_entry_ceiling: None,
        },
    )
    .unwrap();

    assert_eq!(result.row_count, 1);
    assert!(output.starts_with("email\n"));
    assert!(!output.contains("ada@example.com"));
}

#[test]
fn processes_selected_columns() {
    let input_path = fixture("sample.csv");
    let temp_dir = tempfile::tempdir().unwrap();
    let output_path = temp_dir.path().join("sample-output.csv");
    let sample = read_sample(&input_path, 100).unwrap();
    let columns =
        apply_column_selection(&build_column_metadata(&sample.headers, &sample.rows), &[1]);

    let result = process_file(
        &input_path,
        &output_path,
        &columns,
        ProcessOptions {
            smart_replacements: None,
            tokenization_key: None,
            mapping_entry_ceiling: None,
        },
    )
    .unwrap();

    assert_eq!(result.row_count, 5);
    let output = read_sample(&output_path, 100).unwrap();
    assert_eq!(output.headers, sample.headers);
    assert_eq!(output.rows[0][1], "[EMAIL]");
    assert_eq!(output.rows[0][0], sample.rows[0][0]);
}

#[test]
fn rejects_non_empty_fields_beyond_headers_without_committing_output() {
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("ragged.csv");
    let output_path = temp_dir.path().join("ragged-output.csv");
    fs::write(
        &input_path,
        "id,email\n1,a@example.com,unmodeled-secret\n2,b@example.com\n",
    )
    .unwrap();
    let sample = read_sample(&input_path, 100).unwrap_err();

    assert!(sample.to_string().contains("CSV privacy error"));

    // Only the second column is selected: the run has to reject the ragged row while
    // transforming something, or the rejection could be an artefact of there being no work.
    let columns = vec![
        crate::test_support::column(
            0,
            "id",
            crate::types::DataType::NumericId,
            crate::types::AnonymizationStrategy::Auto,
        ),
        crate::test_support::selected_column(
            1,
            "email",
            crate::types::DataType::Email,
            crate::types::AnonymizationStrategy::Auto,
        ),
    ];

    let error = process_file(
        &input_path,
        &output_path,
        &columns,
        ProcessOptions {
            smart_replacements: None,
            tokenization_key: None,
            mapping_entry_ceiling: None,
        },
    )
    .unwrap_err();

    assert!(error.to_string().contains("non-header field"));
    assert!(!output_path.exists());
}

#[test]
fn pads_short_rows_and_truncates_empty_extra_cells() {
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("short-rows.csv");
    let output_path = temp_dir.path().join("short-rows-output.csv");
    fs::write(
        &input_path,
        "id,email,city\n1,a@example.com\n2,b@example.com,NL,,\n",
    )
    .unwrap();
    let sample = read_sample(&input_path, 100).unwrap();
    let columns =
        apply_column_selection(&build_column_metadata(&sample.headers, &sample.rows), &[1]);

    process_file(
        &input_path,
        &output_path,
        &columns,
        ProcessOptions {
            smart_replacements: None,
            tokenization_key: None,
            mapping_entry_ceiling: None,
        },
    )
    .unwrap();

    let output = read_sample(&output_path, 100).unwrap();
    assert_eq!(output.rows[0].len(), 3);
    assert_eq!(output.rows[0][2], "");
    assert_eq!(output.rows[1].len(), 3);
    assert_eq!(output.rows[1][2], "NL");
}

#[test]
fn neutralizes_formula_like_headers_and_cells_in_standard_output() {
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("formula.csv");
    let output_path = temp_dir.path().join("formula-output.csv");
    fs::write(
        &input_path,
        "=name,email\n=cmd,a@example.com\n  +SUM(1 1),b@example.com\n\tTabbed,c@example.com\n",
    )
    .unwrap();
    let sample = read_sample(&input_path, 100).unwrap();
    let columns =
        apply_column_selection(&build_column_metadata(&sample.headers, &sample.rows), &[1]);

    process_file(
        &input_path,
        &output_path,
        &columns,
        ProcessOptions {
            smart_replacements: None,
            tokenization_key: None,
            mapping_entry_ceiling: None,
        },
    )
    .unwrap();

    let output = read_sample(&output_path, 100).unwrap();
    assert_eq!(output.headers[0], "'=name");
    assert_eq!(output.rows[0][0], "'=cmd");
    assert_eq!(output.rows[1][0], "'  +SUM(1 1)");
    assert_eq!(output.rows[2][0], "'\tTabbed");
}

#[test]
fn neutralizes_full_width_formula_prefixes() {
    assert_eq!(neutralize_spreadsheet_formula("＝cmd").as_ref(), "'＝cmd");
    assert_eq!(
        neutralize_spreadsheet_formula("＋SUM(1 1)").as_ref(),
        "'＋SUM(1 1)"
    );
    assert_eq!(neutralize_spreadsheet_formula("－10").as_ref(), "'－10");
    assert_eq!(neutralize_spreadsheet_formula("＠cmd").as_ref(), "'＠cmd");
    assert_eq!(
        neutralize_spreadsheet_formula("\u{3000}＋SUM(1 1)").as_ref(),
        "'\u{3000}＋SUM(1 1)"
    );
    assert_eq!(
        neutralize_spreadsheet_formula("ordinary text").as_ref(),
        "ordinary text"
    );
}

#[test]
fn process_row_count_skips_blank_data_rows_but_preserves_them() {
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("blank-rows.csv");
    let output_path = temp_dir.path().join("blank-rows-output.csv");
    fs::write(
        &input_path,
        "id,email\n1,a@example.com\n,\n2,b@example.com\n   ,   \n3,c@example.com\n",
    )
    .unwrap();
    let sample = read_sample(&input_path, 100).unwrap();
    let columns =
        apply_column_selection(&build_column_metadata(&sample.headers, &sample.rows), &[1]);

    let result = process_file(
        &input_path,
        &output_path,
        &columns,
        ProcessOptions {
            smart_replacements: None,
            tokenization_key: None,
            mapping_entry_ceiling: None,
        },
    )
    .unwrap();

    // All three counts come from separate readers, only some of which trim. They have to
    // land on the same number: `data_rows_scanned` over `count_csv_data_rows` is the
    // coverage fraction the user is shown, and a numerator counting the whitespace-only
    // rows the denominator skipped would claim detection saw more of the file than it did.
    assert_eq!(count_csv_data_rows(&input_path).unwrap(), 3);
    assert_eq!(sample.data_rows_scanned, 3);
    assert_eq!(result.row_count, 3);
    let output = fs::read_to_string(&output_path).unwrap();
    assert!(output.contains("\n,\n"));
    assert!(output.contains("\n   ,   \n"));
}

#[test]
fn process_control_reports_progress_and_cancels_before_next_row() {
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("cancel.csv");
    let output_path = temp_dir.path().join("cancel-output.csv");
    fs::write(
        &input_path,
        "id,email\n1,a@example.com\n,\n2,b@example.com\n3,c@example.com\n",
    )
    .unwrap();
    let sample = read_sample(&input_path, 100).unwrap();
    let columns =
        apply_column_selection(&build_column_metadata(&sample.headers, &sample.rows), &[1]);
    let last_progress = std::cell::Cell::new(0);
    let mut progress_events = Vec::new();
    let error = {
        let mut on_progress = |progress: ProcessProgress| {
            last_progress.set(progress.rows_processed);
            progress_events.push(progress.rows_processed);
        };
        let should_cancel = || last_progress.get() >= 2;
        let mut control = ProcessControl {
            on_progress: Some(&mut on_progress),
            should_cancel: Some(&should_cancel),
        };

        process_file_with_control(
            &input_path,
            &output_path,
            &columns,
            ProcessOptions {
                smart_replacements: None,
                tokenization_key: None,
                mapping_entry_ceiling: None,
            },
            Some(&mut control),
        )
        .unwrap_err()
    };

    assert!(matches!(error, AnonymizerError::Canceled));
    assert_eq!(progress_events, vec![1, 2]);
    assert!(!output_path.exists());
}

#[test]
fn process_control_cancels_after_final_progress_before_output_commit() {
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("cancel-final.csv");
    let output_path = temp_dir.path().join("cancel-final-output.csv");
    fs::write(&input_path, "id,email\n1,a@example.com\n,\n").unwrap();
    let sample = read_sample(&input_path, 100).unwrap();
    let columns =
        apply_column_selection(&build_column_metadata(&sample.headers, &sample.rows), &[1]);
    let last_progress = std::cell::Cell::new(0);
    let error = {
        let mut on_progress =
            |progress: ProcessProgress| last_progress.set(progress.rows_processed);
        let should_cancel = || last_progress.get() >= 1;
        let mut control = ProcessControl {
            on_progress: Some(&mut on_progress),
            should_cancel: Some(&should_cancel),
        };

        process_file_with_control(
            &input_path,
            &output_path,
            &columns,
            ProcessOptions {
                smart_replacements: None,
                tokenization_key: None,
                mapping_entry_ceiling: None,
            },
            Some(&mut control),
        )
        .unwrap_err()
    };

    assert!(matches!(error, AnonymizerError::Canceled));
    assert!(!output_path.exists());
}

#[test]
fn plain_signed_numbers_are_not_neutralized() {
    assert_eq!(neutralize_spreadsheet_formula("-42.50").as_ref(), "-42.50");
    assert_eq!(neutralize_spreadsheet_formula("+31").as_ref(), "+31");
    assert_eq!(neutralize_spreadsheet_formula(" -7 ").as_ref(), " -7 ");
}

#[test]
fn signed_non_numeric_values_are_still_neutralized() {
    assert_eq!(neutralize_spreadsheet_formula("-2+3").as_ref(), "'-2+3");
    assert_eq!(neutralize_spreadsheet_formula("-1.2.3").as_ref(), "'-1.2.3");
    assert_eq!(
        neutralize_spreadsheet_formula("+cmd|calc").as_ref(),
        "'+cmd|calc"
    );
    assert_eq!(neutralize_spreadsheet_formula("－10").as_ref(), "'－10");
}

/// The mapping ceiling has to be *consulted by the run loop*, not merely defined.
///
/// `TransformState::check_mapping_budget_against` can be perfectly correct in
/// isolation while nothing ever calls it, and that failure mode is invisible in the
/// worst way: every unit test passes, the error message is well worded, the README
/// describes a guard — and a large run is still killed by the operating system with no
/// explanation, because the check sits in unreachable code. This test fails if the
/// call is removed from the loop, which no test of the method itself can do.
///
/// The real ceiling stands for roughly 5 GB of mapping, far past what a test can
/// build, which is the whole reason `ProcessOptions::mapping_entry_ceiling` exists.
///
/// It also asserts the destination is untouched, because refusing part-way through is
/// only an improvement on running out of memory if it does not leave a half-written
/// file behind for someone to mistake for a finished one.
#[test]
fn refuses_a_run_that_outgrows_its_mapping_ceiling_without_committing_output() {
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("wide-cardinality.csv");
    let output_path = temp_dir.path().join("wide-cardinality-output.csv");
    let mut text = String::from("id,email\n");
    for row in 0..40 {
        text.push_str(&format!("{row},person{row}@example.com\n"));
    }
    fs::write(&input_path, &text).unwrap();

    let sample = read_sample(&input_path, 100).unwrap();
    let mut columns =
        apply_column_selection(&build_column_metadata(&sample.headers, &sample.rows), &[1]);
    // Pseudonymize explicitly. Left on `Auto` this column is High risk and so defaults
    // to Redact, which holds no mapping at all — the run then finishes cleanly at any
    // ceiling, and the test would pass while proving nothing about the guard.
    columns[1].strategy = crate::types::AnonymizationStrategy::Pseudonymize;

    let error = process_file(
        &input_path,
        &output_path,
        &columns,
        ProcessOptions {
            smart_replacements: None,
            tokenization_key: None,
            // Every row here introduces a distinct value, so a ceiling this small is
            // passed within the first few rows: the refusal lands mid-file, which is
            // the case that matters for the output-commit assertion below.
            mapping_entry_ceiling: Some(4),
        },
    )
    .unwrap_err();

    assert!(
        matches!(error, AnonymizerError::MappingBudgetExceeded { .. }),
        "expected the mapping ceiling to refuse the run, got {error:?}"
    );
    assert!(error.to_string().contains("No output was written"));
    assert!(!output_path.exists());
}

/// A UTF-16LE export of `name,email` with two data rows, byte for byte.
///
/// Written as bytes rather than loaded from a fixture file so CI covers the case
/// even if a checkout, an editor or a `.gitattributes` rule would have "fixed"
/// the encoding of a real file on the way in — which is exactly the kind of
/// silent repair that would leave this regression untested while looking green.
fn utf16_bytes(text: &str, little_endian: bool) -> Vec<u8> {
    text.encode_utf16()
        .flat_map(|unit| {
            let bytes = if little_endian {
                unit.to_le_bytes()
            } else {
                unit.to_be_bytes()
            };
            bytes.into_iter()
        })
        .collect()
}

const ENCODING_SAMPLE_CSV: &str =
    "name,email\nAlice Smith,alice@example.com\nBob Jones,bob@example.com\n";

fn encoding_refusal(bytes: &[u8]) -> AnonymizerError {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("input.csv");
    fs::write(&path, bytes).unwrap();
    validate_file(&path).unwrap_err()
}

/// An unreadable file is reported as unreadable, not as missing.
///
/// The encoding sniff opens the file a second time, after the checks that established it
/// exists. Mapping every failure of that open to `FileNotFound` told a user whose file was
/// plainly there that it was not, which sends them looking for a path problem they do not
/// have. Only a genuine `NotFound` — the file removed between the two opens — keeps that
/// variant.
#[cfg(unix)]
#[test]
fn an_unreadable_input_is_not_reported_as_missing() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("unreadable.csv");
    fs::write(&path, ENCODING_SAMPLE_CSV).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::File::open(&path).is_ok() {
        // Running as root, where the mode does not deny anything.
        return;
    }

    let error = validate_file(&path).unwrap_err();

    assert!(
        !matches!(error, AnonymizerError::FileNotFound(_)),
        "a file that exists but cannot be read is not missing: {error:?}"
    );
}

/// BOM-less UTF-16 is *valid UTF-8* — the text bytes simply interleave with NULs
/// — so nothing in the CSV parser errors on it. Left unchecked, a file of names
/// and email addresses parsed as headers like `n\0a\0m\0e\0`, matched no
/// detector, and was reported as containing no sensitive data at all. For a
/// privacy tool that is the worst possible direction to be wrong in, so the read
/// has to be refused before it starts.
#[test]
fn refuses_bom_less_utf16_input_in_both_byte_orders() {
    for (little_endian, expected) in [(true, "UTF-16LE"), (false, "UTF-16BE")] {
        let error = encoding_refusal(&utf16_bytes(ENCODING_SAMPLE_CSV, little_endian));
        let text = error.to_string();
        assert!(
            text.contains(expected) && text.contains("Re-save it as UTF-8"),
            "expected an actionable {expected} refusal, got {text}"
        );
    }
}

/// UTF-16 *with* a BOM already failed, but with an "invalid utf-8" message that
/// names no remedy. It routes through the same check so a user who re-saves from
/// one Windows tool and hits the other spelling reads the same instruction.
#[test]
fn refuses_utf16_input_carrying_a_byte_order_mark() {
    for (bom, little_endian, expected) in [
        ([0xff, 0xfe], true, "UTF-16LE"),
        ([0xfe, 0xff], false, "UTF-16BE"),
    ] {
        let mut bytes = bom.to_vec();
        bytes.extend(utf16_bytes(ENCODING_SAMPLE_CSV, little_endian));
        let error = encoding_refusal(&bytes);
        assert!(
            error.to_string().contains(expected),
            "expected a {expected} refusal, got {error}"
        );
    }
}

/// The refusal must reach the transform, not just the analysis. A refusal that
/// only fires on the analyze path still lets a user who picks columns by hand
/// write a corrupt mixed-encoding output.
#[test]
fn refuses_utf16_input_on_the_transform_path_with_the_same_message() {
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("utf16.csv");
    let output_path = temp_dir.path().join("out.csv");
    fs::write(&input_path, utf16_bytes(ENCODING_SAMPLE_CSV, true)).unwrap();

    let analyze_error = read_detection_sample(&input_path, 100)
        .unwrap_err()
        .to_string();
    let count_error = count_csv_data_rows(&input_path).unwrap_err().to_string();
    let transform_error = process_file(
        &input_path,
        &output_path,
        &[],
        ProcessOptions {
            smart_replacements: None,
            tokenization_key: None,
            mapping_entry_ceiling: None,
        },
    )
    .unwrap_err()
    .to_string();

    assert!(analyze_error.contains("UTF-16LE"), "{analyze_error}");
    assert_eq!(analyze_error, count_error);
    assert_eq!(analyze_error, transform_error);
    assert!(!output_path.exists(), "a refused run must write no output");
}

/// A NUL-dense input that is not laid out like UTF-16 is not text at all, and
/// telling the user to change encoding would send them chasing the wrong fix.
#[test]
fn refuses_binary_input_with_a_message_about_binary_rather_than_encoding() {
    let mut bytes = b"name,email\n".to_vec();
    bytes.extend([0u8; 64]);
    bytes.extend(b"\x01\x02\x03payload".repeat(8));

    let error = encoding_refusal(&bytes).to_string();
    assert!(error.contains("binary file"), "{error}");
    assert!(!error.contains("UTF-16"), "{error}");
}

/// The one verdict that must never be wrong: no ordinary UTF-8 CSV may be
/// refused, with or without a BOM, and a lone stray control byte is not enough
/// evidence to reject a file the parser could otherwise read and report on.
#[test]
fn accepts_utf8_input_including_a_bom_and_an_isolated_stray_nul() {
    let mut with_bom = vec![0xef, 0xbb, 0xbf];
    with_bom.extend(ENCODING_SAMPLE_CSV.as_bytes());
    let mut stray_nul = ENCODING_SAMPLE_CSV.as_bytes().to_vec();
    stray_nul.insert(20, 0);

    for bytes in [
        ENCODING_SAMPLE_CSV.as_bytes().to_vec(),
        with_bom,
        stray_nul,
        // Non-Latin text is multi-byte in UTF-8 and holds no NULs either.
        "naam,stad\nJos\u{e9},\u{4e2d}\u{56fd}\n"
            .as_bytes()
            .to_vec(),
    ] {
        assert_eq!(
            sniff_unsupported_encoding(&bytes),
            None,
            "a readable UTF-8 CSV must never be refused"
        );
    }
}

/// A short input must not be classified on the strength of a couple of bytes:
/// the density rule needs a sample before its percentages mean anything.
#[test]
fn does_not_classify_a_tiny_prefix_as_utf16() {
    assert_eq!(sniff_unsupported_encoding(b"a\0"), None);
    assert_eq!(sniff_unsupported_encoding(b""), None);
}

/// The finding this whole measure exists for, run end to end through the real transform.
///
/// A file whose name column is redacted and whose postcode, birth date and job title are
/// released untouched. Every per-column check is satisfied — the one identifying column is
/// dealt with — and the three released columns still single two people out between them.
/// Nothing in this crate could say so before, because nothing looked at more than one
/// column at a time.
///
/// The class structure is written into the fixture rather than discovered: three groups of
/// six sharing a triple, then two rows holding a triple of their own.
#[test]
fn released_quasi_identifiers_are_measured_together() {
    let groups = [
        ("1011AB", "1984-02-11", "nurse"),
        ("2033CD", "1979-07-30", "driver"),
        ("3055EF", "1991-12-02", "teacher"),
    ];
    let mut content = String::from("full_name,postal_code,birth_date,job_title\n");
    for (row, (postcode, birth_date, job)) in groups.iter().enumerate() {
        for repeat in 0..6 {
            content.push_str(&format!(
                "Person {row}{repeat},{postcode},{birth_date},{job}\n"
            ));
        }
    }
    content.push_str("Alone One,9099ZZ,1962-01-05,archivist\n");
    content.push_str("Alone Two,9088YY,1955-06-19,harbourmaster\n");

    let sample = read_csv_detection_sample_from_str(&content, 100).unwrap();
    let mut columns = build_column_metadata(&sample.headers, &sample.rows);
    for column in &mut columns {
        // The name is handled; the other three are released as they stand, which is the
        // shape of the file this measure was written about.
        let handled = column.name == "full_name";
        column.is_selected = handled;
        column.strategy = crate::types::AnonymizationStrategy::Redact;
    }

    let (_, result) = process_csv_data(
        &content,
        &columns,
        ProcessOptions {
            smart_replacements: None,
            tokenization_key: None,
            mapping_entry_ceiling: None,
        },
    )
    .unwrap();

    let uniqueness = result
        .transform_report
        .row_uniqueness
        .expect("a CSV run has rows, so it must report a measurement");

    assert_eq!(uniqueness.rows_measured, 20);
    // The redacted name is not linkable; the other three are.
    assert_eq!(
        uniqueness
            .matched_columns
            .iter()
            .map(|matched| matched.column_index)
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert_eq!(uniqueness.distinct_classes, 5);
    assert_eq!(uniqueness.unique_rows, 2);
    assert_eq!(uniqueness.smallest_class, 1);
    assert!(!uniqueness.measurement_incomplete);
    // The redacted column collapses to one token, so it cannot be what separates the
    // rows: the all-column figure has to agree with the linkable one here.
    assert_eq!(uniqueness.distinct_rows_all_columns, Some(5));
}
