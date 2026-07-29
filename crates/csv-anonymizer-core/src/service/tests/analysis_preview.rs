use super::*;

#[test]
fn analyzes_csv_headers_and_default_output_path() {
    let service = AnonymizerService::new("test-version");
    let result = service.analyze_csv(fixture("sample.csv")).unwrap();

    assert_eq!(result.row_count, 5);
    assert!(result.row_count_is_complete);
    assert!(
        result
            .default_output_path
            .ends_with("sample_private_output.csv")
    );
    assert_eq!(result.columns[1].name, "email");
}

#[test]
fn sampled_analysis_still_reports_the_exact_row_count() {
    let service = AnonymizerService::new("test-version");
    let result = service
        .analyze_csv_with_sample_rows(fixture("large.csv"), 25)
        .unwrap();

    // The sample is capped at 25 rows, but detection streams every row, so the
    // count is exact rather than deferred to a second pass.
    assert_eq!(result.row_count, 10_500);
    assert!(result.row_count_is_complete);
    assert_eq!(
        service.count_csv_rows(fixture("large.csv")).unwrap(),
        result.row_count
    );
}

/// A head-anchored detection window makes the analyzer blind to PII that only
/// starts partway down the file, while the transform still streams every row.
/// The column then looks benign, is never auto-selected, and its real values are
/// copied verbatim into the "anonymized" output. Detection must sample across
/// the whole file, not just its opening rows.
#[test]
fn detects_pii_that_only_starts_after_the_sample_window() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("late-pii.csv");

    let mut content = String::from("flag\n");
    for row in 0..1_000 {
        if row < 100 {
            content.push_str(if row % 2 == 0 { "true\n" } else { "false\n" });
        } else {
            content.push_str(&format!("user{row}@example.com\n"));
        }
    }
    fs::write(&input_path, &content).unwrap();

    let result = service
        .analyze_csv_with_sample_rows(&input_path, 100)
        .unwrap();
    let column = &result.columns[0];

    assert_eq!(result.row_count, 1_000);
    assert_eq!(column.detected_type, DataType::Email);
    assert_eq!(column.pii_risk, PiiRisk::High);
    assert!(
        crate::should_auto_select_column(column),
        "a column that is 90% email addresses must be offered for anonymization"
    );
}

/// Flattened exports are periodic: one logical record per k rows, each field on a
/// fixed row of the block. Detection samples a bounded number of rows out of the
/// whole file, so if the choice of rows is a fixed period of its own — every nth row
/// — the two periods align and the sample lands on one phase of the record block.
/// A column holding one real email address per block is then classified entirely off
/// the filler rows: benign type, Low risk, not auto-selected, and the addresses are
/// copied verbatim into the "anonymized" output.
///
/// Four rows per record and a stride that is always a power of two is the case that
/// bit: 100 email addresses, none of them sampled. See `csv_io::spread_priority`.
#[test]
fn detects_pii_that_repeats_on_a_power_of_two_period() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();

    for period in [2, 3, 4, 5, 8, 16] {
        let input_path = temp_dir.path().join(format!("record-blocks-{period}.csv"));
        let mut content = String::from("value\n");
        for row in 0..period * 100 {
            if row % period == period - 1 {
                content.push_str(&format!("user{row}@example.com\n"));
            } else {
                content.push_str(&format!("field {row}\n"));
            }
        }
        fs::write(&input_path, &content).unwrap();

        let result = service.analyze_csv(&input_path).unwrap();
        let column = &result.columns[0];

        assert_eq!(
            column.pii_risk,
            PiiRisk::High,
            "a {period}-row record block hid 100 email addresses: {:?} at {:?} risk",
            column.detected_type,
            column.pii_risk
        );
        assert!(
            crate::should_auto_select_column(column),
            "a {period}-row record block left its email column unselected"
        );
    }
}

/// The "Sample rows" setting may ask detection to look at more values. It may not
/// ask it to look at fewer.
///
/// Analyze fills the column table the user selects from; preflight, preview and the
/// run classify the same file through their own entry points. Detection votes on the
/// ratio of matching values in its sample, so a column that is part one type and
/// part another genuinely lands on different answers at different sample sizes —
/// the fixture below reads as Email at two rows and as String at a hundred. Whichever
/// is right, a setting that lowers only *some* of those entry points makes the table
/// promise a classification the run does not apply.
///
/// `detection_sample_rows` is the floor that prevents it, and this is the test that
/// notices if it is removed: without it the two assertions below disagree.
#[test]
fn a_small_sample_row_request_cannot_lower_the_detection_basis() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("partly-email.csv");

    let mut content = String::from("note\n");
    for row in 0..400 {
        if row % 3 == 0 {
            content.push_str(&format!("user{row}@example.com\n"));
        } else {
            content.push_str(&format!("plain note {row}\n"));
        }
    }
    fs::write(&input_path, &content).unwrap();

    let floored = service
        .analyze_csv_with_sample_rows(&input_path, 100)
        .unwrap();

    for requested in [1, 2, 3, 5, 10, 50] {
        let result = service
            .analyze_csv_with_sample_rows(&input_path, requested)
            .unwrap();

        assert_eq!(
            result.columns[0].detected_type, floored.columns[0].detected_type,
            "a request of {requested} rows classified the column as {:?} where the \
             default basis says {:?}; the run would apply the second one",
            result.columns[0].detected_type, floored.columns[0].detected_type,
        );
        assert_eq!(
            result.columns[0].confidence, floored.columns[0].confidence,
            "a request of {requested} rows changed the reported confidence"
        );
    }
}

/// Raising "Sample rows" has to raise it for the preview too.
///
/// A larger basis is worth asking for because it finds PII the default basis is too
/// small to see: rare values. The fixture below is a free-text column with a dozen
/// email addresses buried in two thousand notes, which the default hundred-row
/// sample almost certainly misses and a whole-file sample cannot. Analyze took the
/// setting; the preview derived its basis from the *display* count, which is capped
/// at a hundred, so it classified on the smaller sample no matter what the setting
/// said — the column table offered a redaction the preview then contradicted.
#[test]
fn preview_classifies_on_the_same_basis_the_setting_gave_analyze() {
    const SAMPLE_ROWS: usize = 2_000;

    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("rare-pii.csv");

    let mut content = String::from("note\n");
    for row in 0..SAMPLE_ROWS {
        if row % 167 == 3 {
            content.push_str(&format!("reached them at user{row}@example.com\n"));
        } else {
            content.push_str(&format!("internal note {row}\n"));
        }
    }
    fs::write(&input_path, &content).unwrap();

    let analyzed = service
        .analyze_csv_with_sample_rows(&input_path, SAMPLE_ROWS)
        .unwrap();
    assert_eq!(
        analyzed.columns[0].pii_risk,
        PiiRisk::High,
        "the fixture is meant to read as PII only on the larger basis"
    );

    let preview = service
        .preview_anonymization(PreviewParams {
            file_path: input_path,
            columns: vec![0],
            controls: Vec::new(),
            sample_count: 3,
            sample_row_count: SAMPLE_ROWS,
        })
        .unwrap();

    for sample in &preview.previews[0].samples {
        assert_eq!(
            sample.anonymized, "[EMAIL]",
            "analyze offered this column as High-risk contact data, so the preview \
             must show the redaction the run will apply, not {:?}",
            sample.anonymized
        );
    }
}

#[test]
fn preview_reuses_repeated_values_within_one_run() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("repeated-preview-values.csv");
    fs::write(
        &input_path,
        "email\nada@example.com\nada@example.com\ngrace@example.com\n",
    )
    .unwrap();

    let preview = service
        .preview_anonymization(PreviewParams {
            file_path: input_path,
            columns: vec![0],
            controls: vec![ColumnControl {
                column_index: 0,
                type_override: Some(DataType::Email),
                strategy: AnonymizationStrategy::Auto,
            }],
            sample_count: 3,
            sample_row_count: 100,
        })
        .unwrap();

    assert_eq!(preview.previews[0].samples.len(), 3);
    assert_eq!(
        preview.previews[0].samples[0].anonymized,
        preview.previews[0].samples[1].anonymized
    );
    assert_ne!(
        preview.previews[0].samples[0].anonymized,
        preview.previews[0].samples[2].anonymized
    );
}

#[test]
fn preview_preserves_short_numeric_code_shape() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("numeric-looking.csv");
    fs::write(&input_path, "code\n1\n2\n3\n").unwrap();

    let preview = service
        .preview_anonymization(PreviewParams {
            file_path: input_path,
            columns: vec![0],
            controls: vec![ColumnControl {
                column_index: 0,
                type_override: None,
                strategy: AnonymizationStrategy::Auto,
            }],
            sample_count: 3,
            sample_row_count: 100,
        })
        .unwrap();

    assert_eq!(preview.previews[0].samples.len(), 3);
    assert!(preview.previews[0].samples.iter().all(|sample| {
        sample
            .anonymized
            .chars()
            .all(|character| character.is_ascii_digit())
    }));
}

#[test]
fn preview_preserves_decimal_numeric_shape() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("decimal-values.csv");
    fs::write(&input_path, "amount\n-12.50\n0.00\n42.75\n").unwrap();

    let preview = service
        .preview_anonymization(PreviewParams {
            file_path: input_path,
            columns: vec![0],
            controls: vec![],
            sample_count: 3,
            sample_row_count: 100,
        })
        .unwrap();

    assert_eq!(preview.previews[0].samples.len(), 3);
    assert!(
        preview.previews[0]
            .samples
            .iter()
            .all(|sample| sample.anonymized.parse::<f64>().is_ok())
    );
    assert_eq!(
        preview.previews[0].samples[0].anonymized.len(),
        "-12.50".len()
    );
    assert!(preview.previews[0].samples[0].anonymized.starts_with('-'));
}

#[test]
fn preview_skips_empty_and_null_samples() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("empty-values.csv");
    fs::write(&input_path, "email\n\nnull\nuser@example.com\n").unwrap();

    let preview = service
        .preview_anonymization(PreviewParams {
            file_path: input_path,
            columns: vec![0],
            controls: vec![],
            sample_count: 3,
            sample_row_count: 100,
        })
        .unwrap();

    assert_eq!(preview.previews[0].samples.len(), 1);
    assert_eq!(preview.previews[0].samples[0].original, "user@example.com");
}

#[test]
fn preview_uses_type_specific_phone_and_name_strategies() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("people.csv");
    fs::write(
        &input_path,
        "phone,first_name,last_name,full_name\n555-867-5309,Alice,Smith,Alice Smith\n",
    )
    .unwrap();

    let preview = service
        .preview_anonymization(PreviewParams {
            file_path: input_path,
            columns: vec![0, 1, 2, 3],
            controls: vec![
                ColumnControl {
                    column_index: 0,
                    type_override: None,
                    strategy: AnonymizationStrategy::Auto,
                },
                ColumnControl {
                    column_index: 1,
                    type_override: None,
                    strategy: AnonymizationStrategy::Auto,
                },
                ColumnControl {
                    column_index: 2,
                    type_override: None,
                    strategy: AnonymizationStrategy::Auto,
                },
                ColumnControl {
                    column_index: 3,
                    type_override: None,
                    strategy: AnonymizationStrategy::Auto,
                },
            ],
            sample_count: 1,
            sample_row_count: 100,
        })
        .unwrap();

    let phone = &preview.previews[0].samples[0].anonymized;
    let first = &preview.previews[1].samples[0].anonymized;
    let last = &preview.previews[2].samples[0].anonymized;
    let full = &preview.previews[3].samples[0].anonymized;

    assert_eq!(phone.len(), "555-867-5309".len());
    assert_eq!(
        phone.chars().filter(|character| *character == '-').count(),
        2
    );
    assert!(first.chars().all(|character| character.is_alphabetic()));
    assert!(last.chars().all(|character| character.is_alphabetic()));
    assert_eq!(full.split_whitespace().count(), 2);
}

#[test]
fn people_names_fixture_previews_name_like_full_names() {
    let service = AnonymizerService::new("test-version");

    let preview = service
        .preview_anonymization(PreviewParams {
            file_path: fixture("people-names.csv"),
            columns: vec![2],
            controls: vec![ColumnControl {
                column_index: 2,
                type_override: None,
                strategy: AnonymizationStrategy::Auto,
            }],
            sample_count: 5,
            sample_row_count: 100,
        })
        .unwrap();

    assert_eq!(preview.previews[0].column_name, "full_name");
    assert_eq!(preview.previews[0].samples.len(), 5);
    for sample in &preview.previews[0].samples {
        assert_ne!(sample.anonymized, sample.original);
        assert_eq!(
            sample.anonymized.split_whitespace().count(),
            sample.original.split_whitespace().count()
        );
        assert!(
            sample
                .anonymized
                .chars()
                .all(|character| character.is_alphabetic() || character.is_whitespace())
        );
        assert!(
            !sample
                .anonymized
                .chars()
                .any(|character| character.is_ascii_digit() || matches!(character, '_' | '-'))
        );
    }
}

#[test]
fn people_names_fixture_treats_single_token_name_column_as_name() {
    let service = AnonymizerService::new("test-version");

    let preview = service
        .preview_anonymization(PreviewParams {
            file_path: fixture("people-names.csv"),
            columns: vec![0, 1, 2, 3],
            controls: vec![
                ColumnControl {
                    column_index: 0,
                    type_override: None,
                    strategy: AnonymizationStrategy::Auto,
                },
                ColumnControl {
                    column_index: 1,
                    type_override: None,
                    strategy: AnonymizationStrategy::Auto,
                },
                ColumnControl {
                    column_index: 2,
                    type_override: None,
                    strategy: AnonymizationStrategy::Auto,
                },
                ColumnControl {
                    column_index: 3,
                    type_override: None,
                    strategy: AnonymizationStrategy::Auto,
                },
            ],
            sample_count: 5,
            sample_row_count: 100,
        })
        .unwrap();

    assert_eq!(preview.previews[3].column_name, "name");

    for row_index in 0..preview.previews[3].samples.len() {
        let first = &preview.previews[0].samples[row_index].anonymized;
        let last = &preview.previews[1].samples[row_index].anonymized;
        let full = &preview.previews[2].samples[row_index].anonymized;
        let name = &preview.previews[3].samples[row_index].anonymized;
        let original_first = &preview.previews[0].samples[row_index].original;
        let original_last = &preview.previews[1].samples[row_index].original;
        let original_name = &preview.previews[3].samples[row_index].original;
        let original_tokens: Vec<&str> = original_first
            .split_whitespace()
            .chain(original_last.split_whitespace())
            .collect();

        assert_eq!(name, first);
        assert_eq!(full, &format!("{first} {last}"));
        assert_ne!(name, original_name);
        assert!(!full.split_whitespace().any(|token| {
            original_tokens
                .iter()
                .any(|original| token.eq_ignore_ascii_case(original))
        }));
        assert!(name.chars().all(|character| character.is_alphabetic()));
        assert!(
            !name
                .chars()
                .any(|character| character.is_ascii_digit() || matches!(character, '_' | '-'))
        );
    }
}

#[test]
fn preview_name_mappings_are_consistent_within_previewed_rows() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("preview-full-names.csv");
    fs::write(
        &input_path,
        "first_name,last_name,full_name\nAlice,Smith,Alice Smith\nBianca,Jones,Bianca Jones\n",
    )
    .unwrap();
    let controls = vec![
        ColumnControl {
            column_index: 0,
            type_override: Some(DataType::FirstName),
            strategy: AnonymizationStrategy::Auto,
        },
        ColumnControl {
            column_index: 1,
            type_override: Some(DataType::LastName),
            strategy: AnonymizationStrategy::Auto,
        },
        ColumnControl {
            column_index: 2,
            type_override: Some(DataType::FullName),
            strategy: AnonymizationStrategy::Auto,
        },
    ];

    let preview = service
        .preview_anonymization(PreviewParams {
            file_path: input_path.clone(),
            columns: vec![0, 1, 2],
            controls: controls.clone(),
            sample_count: 2,
            sample_row_count: 100,
        })
        .unwrap();

    for row_index in 0..2 {
        assert_eq!(
            preview.previews[2].samples[row_index].anonymized,
            format!(
                "{} {}",
                preview.previews[0].samples[row_index].anonymized,
                preview.previews[1].samples[row_index].anonymized
            )
        );
    }
}

#[test]
fn preview_applies_per_column_type_and_strategy_controls() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("controls.csv");
    fs::write(&input_path, "value\n123\n").unwrap();

    let preview = service
        .preview_anonymization(PreviewParams {
            file_path: input_path,
            columns: vec![0],
            controls: vec![ColumnControl {
                column_index: 0,
                type_override: Some(DataType::Email),
                strategy: AnonymizationStrategy::Mask,
            }],
            sample_count: 1,
            sample_row_count: 100,
        })
        .unwrap();

    assert_eq!(preview.previews[0].samples[0].anonymized, "***");
    assert!(preview.warnings.is_empty());
}

#[test]
fn type_override_updates_report_risk_for_effective_type() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("type-override-risk.csv");
    let output_path = temp_dir.path().join("type-override-risk-output.csv");
    fs::write(&input_path, "value\nnot-an-email\n").unwrap();

    let result = service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path,
            columns: vec![0],
            controls: vec![ColumnControl {
                column_index: 0,
                type_override: Some(DataType::Email),
                strategy: AnonymizationStrategy::Redact,
            }],
            force: false,
            preview_smart_replacements: vec![],
        })
        .unwrap();

    let report = &result.privacy_report.column_reports[0];
    assert_eq!(report.detected_type, DataType::Email);
    assert_eq!(report.pii_risk, crate::types::PiiRisk::High);
}

#[test]
fn type_override_preserves_privacy_evidence_beyond_retained_samples() {
    let headers = vec!["value".to_string()];
    let rows = [
        "alpha",
        "bravo",
        "charlie",
        "delta",
        "echo",
        "late@example.com",
    ]
    .into_iter()
    .map(|value| vec![value.to_string()])
    .collect::<Vec<_>>();
    let metadata = build_column_metadata(&headers, &rows);

    assert_eq!(metadata[0].sample_values.len(), 5);
    assert_eq!(metadata[0].pii_risk, crate::types::PiiRisk::High);
    assert!(
        metadata[0]
            .privacy_findings
            .iter()
            .any(|finding| finding.sample_value == "late@example.com")
    );

    let controlled = apply_column_controls(
        &metadata,
        &[ColumnControl {
            column_index: 0,
            type_override: Some(DataType::String),
            strategy: AnonymizationStrategy::Redact,
        }],
    )
    .unwrap();

    assert_eq!(controlled[0].detected_type, DataType::String);
    assert_eq!(controlled[0].pii_risk, crate::types::PiiRisk::High);
    assert!(
        controlled[0]
            .privacy_findings
            .iter()
            .any(|finding| finding.sample_value == "late@example.com")
    );
    assert!(
        controlled[0]
            .privacy_evidence
            .iter()
            .any(|evidence| evidence.kind == crate::types::PrivacyFindingKind::Contact)
    );
}

#[test]
fn preview_warns_for_pass_through_and_no_op_columns() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("warnings.csv");
    fs::write(&input_path, "country,email\nUS,user@example.com\n").unwrap();

    let preview = service
        .preview_anonymization(PreviewParams {
            file_path: input_path,
            columns: vec![0, 1],
            controls: vec![ColumnControl {
                column_index: 1,
                type_override: None,
                strategy: AnonymizationStrategy::PassThrough,
            }],
            sample_count: 1,
            sample_row_count: 100,
        })
        .unwrap();

    assert_eq!(preview.warnings.len(), 2);
    assert!(
        preview
            .warnings
            .iter()
            .any(|warning| warning.column_index == 0 && warning.message.contains("pass-through"))
    );
    assert!(
        preview
            .warnings
            .iter()
            .any(|warning| warning.column_index == 1 && warning.message.contains("unchanged"))
    );
}
