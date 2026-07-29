use super::*;
use crate::smart::{SmartReplacement, SmartReplacementProvider, SmartReplacementRequest};
use crate::types::{
    AnonymizationStrategy, ColumnControl, DataType, MAX_SAMPLE_ROW_COUNT, PiiRisk,
    SmartReplacementEntry, SmartReplacementRejectionCount, SmartReplacementRejectionReason,
};

mod redaction;

#[test]
fn transforms_csv_text_with_existing_csv_rules() {
    let input = "email,name\nada@example.com,Ada Lovelace\n";
    let analysis = analyze_paste_data(PasteAnalyzeParams {
        content: input.to_string(),
        format: PasteDataFormat::Csv,
        sample_row_count: 10,
    })
    .unwrap();
    let selected = analysis
        .columns
        .iter()
        .filter(|column| matches!(column.pii_risk, PiiRisk::High | PiiRisk::Medium))
        .map(|column| column.index)
        .collect::<Vec<_>>();

    let result = transform_paste_data(PasteTransformParams {
        content: input.to_string(),
        format: PasteDataFormat::Csv,
        columns: selected,
        controls: Vec::new(),
        sample_row_count: 100,
        preview_smart_replacements: Vec::new(),
    })
    .unwrap();

    assert!(result.output.starts_with("email,name\n"));
    assert!(!result.output.contains("ada@example.com"));
}

#[test]
fn csv_text_type_override_preserves_direct_identifier_reporting() {
    let input = "value\nalpha\nbravo\ncharlie\ndelta\necho\nlate@example.com\n";

    let result = transform_paste_data(PasteTransformParams {
        content: input.to_string(),
        format: PasteDataFormat::Csv,
        columns: vec![0],
        controls: vec![ColumnControl {
            column_index: 0,
            type_override: Some(DataType::String),
            strategy: AnonymizationStrategy::Redact,
        }],
        sample_row_count: 100,
        preview_smart_replacements: Vec::new(),
    })
    .unwrap();

    assert_eq!(result.privacy_report.direct_identifiers, 1);
    assert_eq!(result.privacy_report.quasi_identifiers, 0);
    assert!(!result.output.contains("late@example.com"));
}

#[test]
fn analyze_paste_data_auto_selects_columns_with_core_policy() {
    let input = "email,notes\nada@example.com,\n";
    let analysis = analyze_paste_data(PasteAnalyzeParams {
        content: input.to_string(),
        format: PasteDataFormat::Csv,
        sample_row_count: 10,
    })
    .unwrap();

    let email = analysis
        .columns
        .iter()
        .find(|column| column.name == "email")
        .unwrap();
    assert!(matches!(email.pii_risk, PiiRisk::High | PiiRisk::Medium));
    assert!(!email.sample_values.is_empty());
    assert!(email.is_selected);

    let notes = analysis
        .columns
        .iter()
        .find(|column| column.name == "notes")
        .unwrap();
    assert!(notes.sample_values.is_empty());
    assert!(!notes.is_selected);

    // The shared policy also rejects high-risk columns without sample values:
    // analyze_paste_data applies should_auto_select_column verbatim.
    let mut empty_high_risk = email.clone();
    empty_high_risk.sample_values.clear();
    assert!(!crate::metadata::should_auto_select_column(
        &empty_high_risk
    ));
}

/// The paste preview must classify on the same spread sample as the transform.
///
/// It used to detect on the display window — `sample_count * 2` head rows — so a
/// long paste whose PII starts past that window previewed as one type and
/// transformed as another. The `@localhost` values below are the probe: they hold
/// an `@` but fail the email validator, so a head-only window sees no email
/// column at all, while a window that reaches the real addresses further down
/// classifies the column as Email — and Email redacts to a typed marker, which is
/// the difference this asserts.
#[test]
fn direct_input_preview_classifies_on_the_whole_paste_not_the_display_window() {
    let mut content = String::from("contact\n");
    for row in 0..6 {
        content.push_str(&format!("user{row}@localhost\n"));
    }
    for row in 6..400 {
        content.push_str(&format!("user{row}@example.com\n"));
    }

    let preview = preview_paste_data(PastePreviewParams {
        content,
        format: PasteDataFormat::Csv,
        columns: vec![0],
        controls: vec![],
        sample_count: 3,
        sample_row_count: 100,
    })
    .unwrap();

    let samples = &preview.previews[0].samples;
    assert!(!samples.is_empty(), "the preview should show display rows");
    for sample in samples {
        assert!(
            sample.original.ends_with("@localhost"),
            "the display window should still be the paste's opening rows, got {:?}",
            sample.original
        );
        assert_eq!(
            sample.anonymized, "[EMAIL]",
            "the column is 98% email addresses, so the preview must classify it as \
             Email; {:?} means it classified on the display window instead",
            sample.anonymized
        );
    }
}

/// Paste analyze reaches the same detection floor as paste preview and transform.
///
/// The floor lives in `paste_detection_sample_rows`, which every format's analyze,
/// preview and transform entry point routes through, so a small "Sample rows"
/// setting can no longer give the column table a narrower basis than the run that
/// follows it. The probe is the same one the CSV preview test uses: `@localhost`
/// values hold an `@` but fail the email validator, so a basis that stops short of
/// the real addresses sees no email column at all.
#[test]
fn paste_analysis_classifies_on_the_detection_floor_not_the_requested_sample() {
    let mut content = String::from("contact\n");
    for row in 0..6 {
        content.push_str(&format!("user{row}@localhost\n"));
    }
    for row in 6..400 {
        content.push_str(&format!("user{row}@example.com\n"));
    }

    for sample_row_count in [1, 3, 6, 100] {
        let analysis = analyze_paste_data(PasteAnalyzeParams {
            content: content.clone(),
            format: PasteDataFormat::Csv,
            sample_row_count,
        })
        .unwrap();

        assert_eq!(
            analysis.columns[0].detected_type,
            DataType::Email,
            "a request of {sample_row_count} rows classified the column as {:?}",
            analysis.columns[0].detected_type
        );
    }
}

/// And a large "Sample rows" setting raises all three paste entry points, not just
/// analyze.
///
/// A larger basis is worth asking for because it finds rare values — the field below
/// is a thousand notes with six email addresses buried in them, which a hundred-row
/// basis is too small to be likely to see. Preview and transform used to read the
/// floor as a constant while analyze rose above it, so the column table promised a
/// redaction that neither the preview nor the run applied.
#[test]
fn paste_preview_and_transform_follow_a_raised_sample_row_count() {
    const SAMPLE_ROWS: usize = 1_000;

    let mut content = String::from("note\n");
    for row in 0..SAMPLE_ROWS {
        if row % 167 == 3 {
            content.push_str(&format!("reached them at user{row}@example.com\n"));
        } else {
            content.push_str(&format!("internal note {row}\n"));
        }
    }

    let analysis = analyze_paste_data(PasteAnalyzeParams {
        content: content.clone(),
        format: PasteDataFormat::Csv,
        sample_row_count: SAMPLE_ROWS,
    })
    .unwrap();
    assert_eq!(
        analysis.columns[0].pii_risk,
        PiiRisk::High,
        "the fixture is meant to read as PII only on the larger basis"
    );

    let preview = preview_paste_data(PastePreviewParams {
        content: content.clone(),
        format: PasteDataFormat::Csv,
        columns: vec![0],
        controls: vec![],
        sample_count: 3,
        sample_row_count: SAMPLE_ROWS,
    })
    .unwrap();
    for sample in &preview.previews[0].samples {
        assert_eq!(
            sample.anonymized, "[EMAIL]",
            "the preview classified on the floor rather than the requested basis"
        );
    }

    let result = transform_paste_data(PasteTransformParams {
        content,
        format: PasteDataFormat::Csv,
        columns: vec![0],
        controls: Vec::new(),
        sample_row_count: SAMPLE_ROWS,
        preview_smart_replacements: Vec::new(),
    })
    .unwrap();
    assert!(
        !result.output.contains("@example.com"),
        "the transform classified on the floor rather than the requested basis, \
         leaving email addresses in the output"
    );
}

/// A field of a field-shaped paste is classified on a sample of *all* its values,
/// not on its opening ones.
///
/// Collection used to keep the first N values per field, so a field whose PII started
/// past the detection basis was classified off whatever came before it: a log or
/// export where a field is a placeholder for its first few hundred records came back
/// `String` at Low risk and was not offered for anonymization, while the transform
/// walked every record and copied the real values out. The display window is a
/// separate window and stays head-anchored, which is what the second half asserts —
/// the two must not be collapsed back into one.
#[test]
fn field_shaped_paste_detects_pii_that_starts_past_the_detection_basis() {
    for (format, build) in [
        (
            PasteDataFormat::Xml,
            xml_records as fn(usize, usize) -> String,
        ),
        (PasteDataFormat::Json, json_records),
    ] {
        let content = build(2_000, 500);

        let analysis = analyze_paste_data(PasteAnalyzeParams {
            content: content.clone(),
            format,
            sample_row_count: 100,
        })
        .unwrap();
        let column = &analysis.columns[0];

        assert_eq!(
            column.detected_type,
            DataType::Email,
            "{format:?}: a field of 1,500 email addresses read as {:?}",
            column.detected_type
        );
        assert_eq!(column.pii_risk, PiiRisk::High, "{format:?}");
        assert!(
            column.is_selected,
            "{format:?}: the field was not offered for anonymization"
        );

        let preview = preview_paste_data(PastePreviewParams {
            content,
            format,
            columns: vec![0],
            controls: vec![],
            sample_count: 3,
            sample_row_count: 100,
        })
        .unwrap();
        for sample in &preview.previews[0].samples {
            assert!(
                sample.original.starts_with("ref-"),
                "{format:?}: the display window must stay the paste's opening values, \
                 got {:?}",
                sample.original
            );
            assert_eq!(
                sample.anonymized, "[EMAIL]",
                "{format:?}: the preview classified on the display window"
            );
        }
    }
}

/// The record count reported for an XML paste is the document's, not the sample's.
///
/// It was read off the retained value count, which the detection basis caps, so a
/// 2,000-record paste reported 100 records — and `rowCountIsComplete` said that was
/// exact.
#[test]
fn xml_paste_reports_the_documents_record_count_not_the_sample_size() {
    let analysis = analyze_paste_data(PasteAnalyzeParams {
        content: xml_records(2_000, 2_000),
        format: PasteDataFormat::Xml,
        sample_row_count: 100,
    })
    .unwrap();

    assert_eq!(analysis.row_count, 2_000);
    assert!(analysis.row_count_is_complete);
}

fn xml_records(total: usize, pii_starts_at: usize) -> String {
    let mut content = String::from("<rows>\n");
    for row in 0..total {
        content.push_str(&format!(
            "  <row><contact>{}</contact></row>\n",
            record_value(row, pii_starts_at)
        ));
    }
    content.push_str("</rows>\n");
    content
}

fn json_records(total: usize, pii_starts_at: usize) -> String {
    let records = (0..total)
        .map(|row| format!(r#"{{"contact":"{}"}}"#, record_value(row, pii_starts_at)))
        .collect::<Vec<_>>()
        .join(",");
    format!("[{records}]")
}

fn record_value(row: usize, pii_starts_at: usize) -> String {
    if row < pii_starts_at {
        format!("ref-{row}")
    } else {
        format!("user{row}@example.com")
    }
}

/// The field-shaped formats split detection from display exactly as the CSV paste
/// path does.
///
/// XML, JSON/YAML and free text all preview through
/// `preview_from_fields_with_smart_provider`, which used to classify the display
/// window — `sample_count * 2` values — so a preview of three rows decided a
/// column's type from six values while the transform decided it from a hundred. The
/// `@localhost` prefix below is the discriminator: classify on the display window
/// and the column is a plain string, classify on the detection floor and it is an
/// Email that redacts to a typed marker.
#[test]
fn field_shaped_preview_classifies_on_the_detection_floor_not_the_display_window() {
    let mut content = String::from("<rows>\n");
    for row in 0..6 {
        content.push_str(&format!(
            "  <row><contact>user{row}@localhost</contact></row>\n"
        ));
    }
    for row in 6..400 {
        content.push_str(&format!(
            "  <row><contact>user{row}@example.com</contact></row>\n"
        ));
    }
    content.push_str("</rows>\n");

    let preview = preview_paste_data(PastePreviewParams {
        content,
        format: PasteDataFormat::Xml,
        columns: vec![0],
        controls: vec![],
        sample_count: 3,
        sample_row_count: 100,
    })
    .unwrap();

    let samples = &preview.previews[0].samples;
    assert!(!samples.is_empty(), "the preview should show display rows");
    for sample in samples {
        assert!(
            sample.original.ends_with("@localhost"),
            "the display window should still be the paste's opening values, got {:?}",
            sample.original
        );
        assert_eq!(
            sample.anonymized, "[EMAIL]",
            "the field is 98% email addresses, so the preview must classify it as \
             Email; {:?} means it classified on the display window instead",
            sample.anonymized
        );
    }
}

#[test]
fn direct_input_preview_includes_selected_column_warnings() {
    let preview = preview_paste_data(PastePreviewParams {
        content: "email,country\nada@example.com,US\n".to_string(),
        format: PasteDataFormat::Csv,
        columns: vec![0, 1],
        controls: vec![
            ColumnControl {
                column_index: 0,
                type_override: None,
                strategy: AnonymizationStrategy::PassThrough,
            },
            ColumnControl {
                column_index: 1,
                type_override: Some(DataType::CountryCode),
                strategy: AnonymizationStrategy::Auto,
            },
        ],
        sample_count: 3,
        sample_row_count: 100,
    })
    .unwrap();

    assert_eq!(preview.warnings.len(), 2);
    assert!(
        preview
            .warnings
            .iter()
            .any(|warning| warning.column_name == "email" && warning.message.contains("unchanged"))
    );
    assert!(
        preview
            .warnings
            .iter()
            .any(|warning| warning.column_name == "country"
                && warning.message.contains("pass-through behavior"))
    );
}

#[test]
fn generates_quick_values_without_user_input() {
    let result = generate_quick_values(QuickGenerateParams {
        data_type: DataType::Email,
        strategy: AnonymizationStrategy::Auto,
        count: 2,
    })
    .unwrap();
    let lines = result.output.lines().collect::<Vec<_>>();

    assert_eq!(result.row_count, 2);
    assert_eq!(lines.len(), 2);
    assert!(lines.iter().all(|line| line.ends_with("@example.invalid")));
    assert!(!result.output.contains("person1@example.invalid"));
}

#[test]
fn generates_type_shaped_quick_values() {
    let uuid_result = generate_quick_values(QuickGenerateParams {
        data_type: DataType::Uuid,
        strategy: AnonymizationStrategy::Auto,
        count: 1,
    })
    .unwrap();
    let uuid = uuid_result.output.as_str();

    assert_eq!(uuid.len(), 36);
    assert_eq!(uuid.chars().nth(14), Some('4'));
    assert!(matches!(uuid.chars().nth(19), Some('8' | '9' | 'a' | 'b')));

    let ip_result = generate_quick_values(QuickGenerateParams {
        data_type: DataType::IpAddress,
        strategy: AnonymizationStrategy::Auto,
        count: 1,
    })
    .unwrap();

    assert!(ip_result.output.starts_with("198.51.100."));

    let phone_result = generate_quick_values(QuickGenerateParams {
        data_type: DataType::Phone,
        strategy: AnonymizationStrategy::Auto,
        count: 1,
    })
    .unwrap();
    let phone = phone_result.output.as_str();

    assert_eq!(phone.len(), "555-020-0000".len());
    assert_eq!(phone.chars().nth(3), Some('-'));
    assert_eq!(phone.chars().nth(7), Some('-'));
    assert!(
        phone
            .chars()
            .enumerate()
            .all(|(index, character)| character.is_ascii_digit() || matches!(index, 3 | 7))
    );

    let name_result = generate_quick_values(QuickGenerateParams {
        data_type: DataType::FullName,
        strategy: AnonymizationStrategy::Auto,
        count: 1,
    })
    .unwrap();
    let name = name_result.output.as_str();

    assert_ne!(name, "First1 Last1");
    assert_eq!(name.split_whitespace().count(), 2);

    let timestamp_result = generate_quick_values(QuickGenerateParams {
        data_type: DataType::Timestamp,
        strategy: AnonymizationStrategy::Auto,
        count: 1,
    })
    .unwrap();
    let timestamp = timestamp_result.output.as_str();

    assert_eq!(timestamp.len(), "2024-01-01T00:00:00Z".len());
    assert_eq!(timestamp.chars().nth(4), Some('-'));
    assert_eq!(timestamp.chars().nth(7), Some('-'));
    assert!(timestamp.contains('T'));
    assert!(timestamp.ends_with('Z'));
}

#[test]
fn generates_tokenized_quick_values() {
    let result = generate_quick_values(QuickGenerateParams {
        data_type: DataType::Email,
        strategy: AnonymizationStrategy::Tokenize,
        count: 2,
    })
    .unwrap();
    let lines = result.output.lines().collect::<Vec<_>>();

    assert_eq!(result.row_count, 2);
    assert_eq!(lines.len(), 2);
    assert!(lines.iter().all(|line| line.starts_with("tok_")));
    assert!(lines.iter().all(|line| !line.contains('@')));
    assert_ne!(lines[0], lines[1]);
}

#[test]
fn quick_generation_rejects_input_only_strategies() {
    let error = generate_quick_values(QuickGenerateParams {
        data_type: DataType::Email,
        strategy: AnonymizationStrategy::Mask,
        count: 1,
    })
    .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("auto, pseudonymize, tokenize, or smart replacement")
    );
}

#[test]
fn transforms_xml_attributes_and_text() {
    let input = r#"<users><user email="ada@example.com"><name>Ada Lovelace</name></user></users>"#;
    let analysis = analyze_paste_data(PasteAnalyzeParams {
        content: input.to_string(),
        format: PasteDataFormat::Xml,
        sample_row_count: 10,
    })
    .unwrap();
    let selected = analysis
        .columns
        .iter()
        .filter(|column| column.name == "users.user.@email" || column.name == "users.user.name")
        .map(|column| column.index)
        .collect::<Vec<_>>();

    let result = transform_paste_data(PasteTransformParams {
        content: input.to_string(),
        format: PasteDataFormat::Xml,
        columns: selected,
        controls: Vec::new(),
        sample_row_count: 100,
        preview_smart_replacements: Vec::new(),
    })
    .unwrap();

    assert!(!result.output.contains("ada@example.com"));
    assert!(!result.output.contains("Ada Lovelace"));
}

#[test]
fn transforms_selected_xml_cdata() {
    let input = r#"<users><user><name><![CDATA[Ada Lovelace]]></name></user></users>"#;
    let analysis = analyze_paste_data(PasteAnalyzeParams {
        content: input.to_string(),
        format: PasteDataFormat::Xml,
        sample_row_count: 10,
    })
    .unwrap();
    let name = analysis
        .columns
        .iter()
        .find(|column| column.name == "users.user.name")
        .unwrap();

    let result = transform_paste_data(PasteTransformParams {
        content: input.to_string(),
        format: PasteDataFormat::Xml,
        columns: vec![name.index],
        controls: vec![ColumnControl {
            column_index: name.index,
            type_override: Some(DataType::FullName),
            strategy: AnonymizationStrategy::Redact,
        }],
        sample_row_count: 100,
        preview_smart_replacements: Vec::new(),
    })
    .unwrap();

    assert!(!result.output.contains("Ada Lovelace"));
    assert!(result.output.contains("<![CDATA["));
}

#[test]
fn json_paths_distinguish_literal_dotted_keys_from_nested_keys() {
    let input = r#"{
  "a.b": "literal@example.com",
  "a": { "b": "nested@example.com" },
  "items[]": "literal-brackets@example.com",
  "items": ["array@example.com"]
}"#;
    let analysis = analyze_paste_data(PasteAnalyzeParams {
        content: input.to_string(),
        format: PasteDataFormat::Json,
        sample_row_count: 10,
    })
    .unwrap();

    let literal = analysis
        .columns
        .iter()
        .find(|column| column.name == r#"["a.b"]"#)
        .unwrap();
    let nested = analysis
        .columns
        .iter()
        .find(|column| column.name == "a.b")
        .unwrap();
    let literal_brackets = analysis
        .columns
        .iter()
        .find(|column| column.name == r#"["items[]"]"#)
        .unwrap();
    let array_value = analysis
        .columns
        .iter()
        .find(|column| column.name == "items[]")
        .unwrap();

    assert_ne!(literal.source_path, nested.source_path);
    assert_ne!(literal_brackets.source_path, array_value.source_path);

    let result = transform_paste_data(PasteTransformParams {
        content: input.to_string(),
        format: PasteDataFormat::Json,
        columns: vec![nested.index, array_value.index],
        controls: Vec::new(),
        sample_row_count: 100,
        preview_smart_replacements: Vec::new(),
    })
    .unwrap();

    assert!(result.output.contains("literal@example.com"));
    assert!(result.output.contains("literal-brackets@example.com"));
    assert!(!result.output.contains("nested@example.com"));
    assert!(!result.output.contains("array@example.com"));
}

#[test]
fn json_transform_preserves_scalar_value_types() {
    let input = r#"{"age":42,"ratio":12.5,"flag":true}"#;
    let analysis = analyze_paste_data(PasteAnalyzeParams {
        content: input.to_string(),
        format: PasteDataFormat::Json,
        sample_row_count: 10,
    })
    .unwrap();

    let age = analysis
        .columns
        .iter()
        .find(|column| column.name == "age")
        .unwrap();
    let ratio = analysis
        .columns
        .iter()
        .find(|column| column.name == "ratio")
        .unwrap();
    let flag = analysis
        .columns
        .iter()
        .find(|column| column.name == "flag")
        .unwrap();

    let result = transform_paste_data(PasteTransformParams {
        content: input.to_string(),
        format: PasteDataFormat::Json,
        columns: vec![age.index, ratio.index, flag.index],
        controls: vec![
            ColumnControl {
                column_index: age.index,
                type_override: Some(DataType::NumericId),
                strategy: AnonymizationStrategy::Auto,
            },
            ColumnControl {
                column_index: ratio.index,
                type_override: Some(DataType::NumericValue),
                strategy: AnonymizationStrategy::Auto,
            },
            ColumnControl {
                column_index: flag.index,
                type_override: Some(DataType::Boolean),
                strategy: AnonymizationStrategy::Auto,
            },
        ],
        sample_row_count: 100,
        preview_smart_replacements: Vec::new(),
    })
    .unwrap();

    let output: serde_json::Value = serde_json::from_str(&result.output).unwrap();
    assert!(output["age"].is_number());
    assert!(output["ratio"].is_number());
    assert!(output["flag"].is_boolean());
}

#[test]
fn rejects_oversized_pasted_payloads() {
    let input = "x".repeat(super::shared::PASTE_MAX_CONTENT_BYTES + 1);
    let error = analyze_paste_data(PasteAnalyzeParams {
        content: input,
        format: PasteDataFormat::PlainText,
        sample_row_count: 10,
    })
    .unwrap_err();

    assert!(error.to_string().contains("at most 5 MiB"));
}

/// The paste ceiling is the same one the "Sample rows" setting is clamped to, so
/// every value the setting can hold is a value paste analysis accepts. A lower
/// paste-only ceiling made a valid setting fail on pasted input while files kept
/// working, which is why the accepting half is asserted here too.
#[test]
fn paste_sample_counts_accept_the_settings_ceiling_and_reject_more() {
    let content = "email\nada@example.com\n".to_string();

    assert!(
        analyze_paste_data(PasteAnalyzeParams {
            content: content.clone(),
            format: PasteDataFormat::Csv,
            sample_row_count: MAX_SAMPLE_ROW_COUNT,
        })
        .is_ok()
    );

    let error = analyze_paste_data(PasteAnalyzeParams {
        content,
        format: PasteDataFormat::Csv,
        sample_row_count: MAX_SAMPLE_ROW_COUNT + 1,
    })
    .unwrap_err();

    assert!(error.to_string().contains("sample row count"));
}

#[test]
fn rejects_too_many_structured_fields() {
    let fields = (0..=super::shared::PASTE_MAX_FIELDS)
        .map(|index| format!(r#""field{index}":"value{index}@example.com""#))
        .collect::<Vec<_>>()
        .join(",");
    let error = analyze_paste_data(PasteAnalyzeParams {
        content: format!("{{{fields}}}"),
        format: PasteDataFormat::Json,
        sample_row_count: 10,
    })
    .unwrap_err();

    assert!(error.to_string().contains("Detected more than"));
}

#[test]
fn xml_paths_distinguish_dotted_element_names_from_nested_elements() {
    let input = r#"<root><a.b email="literal@example.com">Literal</a.b><a><b email="nested@example.com">Nested</b></a></root>"#;
    let analysis = analyze_paste_data(PasteAnalyzeParams {
        content: input.to_string(),
        format: PasteDataFormat::Xml,
        sample_row_count: 10,
    })
    .unwrap();

    let literal = analysis
        .columns
        .iter()
        .find(|column| column.name == r#"root.["a.b"].@email"#)
        .unwrap();
    let nested = analysis
        .columns
        .iter()
        .find(|column| column.name == "root.a.b.@email")
        .unwrap();

    assert_ne!(literal.source_path, nested.source_path);

    let result = transform_paste_data(PasteTransformParams {
        content: input.to_string(),
        format: PasteDataFormat::Xml,
        columns: vec![nested.index],
        controls: Vec::new(),
        sample_row_count: 100,
        preview_smart_replacements: Vec::new(),
    })
    .unwrap();

    assert!(result.output.contains("literal@example.com"));
    assert!(!result.output.contains("nested@example.com"));
}

#[test]
fn xml_paths_distinguish_dotted_attribute_names_from_nested_paths() {
    let input =
        r#"<root><item a.b="literal@example.com"><a b="nested@example.com"/></item></root>"#;
    let analysis = analyze_paste_data(PasteAnalyzeParams {
        content: input.to_string(),
        format: PasteDataFormat::Xml,
        sample_row_count: 10,
    })
    .unwrap();

    let literal_attribute = analysis
        .columns
        .iter()
        .find(|column| column.name == r#"root.item.@["a.b"]"#)
        .unwrap();
    let nested_attribute = analysis
        .columns
        .iter()
        .find(|column| column.name == "root.item.a.@b")
        .unwrap();

    assert_ne!(literal_attribute.source_path, nested_attribute.source_path);

    let result = transform_paste_data(PasteTransformParams {
        content: input.to_string(),
        format: PasteDataFormat::Xml,
        columns: vec![nested_attribute.index],
        controls: Vec::new(),
        sample_row_count: 100,
        preview_smart_replacements: Vec::new(),
    })
    .unwrap();

    assert!(result.output.contains("literal@example.com"));
    assert!(!result.output.contains("nested@example.com"));
}

#[test]
fn previews_pasted_json_fields() {
    let input = r#"[{"email":"ada@example.com"}]"#;
    let analysis = analyze_paste_data(PasteAnalyzeParams {
        content: input.to_string(),
        format: PasteDataFormat::Auto,
        sample_row_count: 10,
    })
    .unwrap();
    assert_eq!(analysis.format, PasteDataFormat::Json);

    let email = analysis
        .columns
        .iter()
        .find(|column| column.name == "[].email")
        .unwrap();
    let preview = preview_paste_data(PastePreviewParams {
        content: input.to_string(),
        format: PasteDataFormat::Json,
        columns: vec![email.index],
        controls: Vec::new(),
        sample_count: 5,
        sample_row_count: 100,
    })
    .unwrap();

    assert_eq!(preview.previews[0].column_name, "[].email");
    assert_eq!(preview.previews[0].samples[0].original, "ada@example.com");
    assert_ne!(preview.previews[0].samples[0].anonymized, "ada@example.com");
}

#[test]
fn previews_xml_fields_through_shared_orchestration() {
    let input = "<root><name>Ada Lovelace</name></root>";
    assert_shared_smart_preview(input, PasteDataFormat::Xml);
}

#[test]
fn previews_plain_text_and_logs_through_shared_orchestration() {
    let input = "contact=ada@example.com";
    assert_shared_smart_preview(input, PasteDataFormat::PlainText);
    assert_shared_smart_preview(input, PasteDataFormat::Logs);
}

fn assert_shared_smart_preview(input: &str, format: PasteDataFormat) {
    let analysis = analyze_paste_data(PasteAnalyzeParams {
        content: input.to_string(),
        format,
        sample_row_count: 10,
    })
    .unwrap();
    let column = analysis.columns.first().expect("detected preview column");
    let original = column
        .sample_values
        .first()
        .expect("detected preview sample")
        .clone();
    let mut provider = PrefixSmartProvider;

    let preview = preview_paste_data_with_smart_provider(
        PastePreviewParams {
            content: input.to_string(),
            format,
            columns: vec![column.index],
            controls: vec![ColumnControl {
                column_index: column.index,
                type_override: Some(DataType::FullName),
                strategy: AnonymizationStrategy::LocalAi,
            }],
            sample_count: 5,
            sample_row_count: 100,
        },
        Some(&mut provider),
    )
    .unwrap();

    assert_eq!(preview.previews[0].samples[0].original, original);
    assert_eq!(preview.previews[0].samples[0].anonymized, "Smart Person 1");
    assert_eq!(preview.smart_replacements.len(), 1);
    assert!(preview.warnings.iter().any(
        |warning| warning.column_index == column.index && warning.message.contains("Local AI")
    ));
}

#[test]
fn previews_and_transforms_paste_data_with_smart_replacements() {
    let input = r#"[{"name":"Ada Lovelace"},{"name":"Grace Hopper"}]"#;
    let analysis = analyze_paste_data(PasteAnalyzeParams {
        content: input.to_string(),
        format: PasteDataFormat::Json,
        sample_row_count: 10,
    })
    .unwrap();
    let name = analysis
        .columns
        .iter()
        .find(|column| column.name == "[].name")
        .unwrap();
    let controls = vec![ColumnControl {
        column_index: name.index,
        type_override: Some(DataType::FullName),
        strategy: AnonymizationStrategy::LocalAi,
    }];
    let mut preview_provider = PrefixSmartProvider;

    let preview = preview_paste_data_with_smart_provider(
        PastePreviewParams {
            content: input.to_string(),
            format: PasteDataFormat::Json,
            columns: vec![name.index],
            controls: controls.clone(),
            sample_count: 5,
            sample_row_count: 100,
        },
        Some(&mut preview_provider),
    )
    .unwrap();

    assert_eq!(preview.smart_replacements.len(), 2);
    assert_eq!(preview.previews[0].samples[0].anonymized, "Smart Person 1");
    assert!(
        preview
            .warnings
            .iter()
            .any(|warning| warning.column_name == "[].name"
                && warning.message.contains("Local AI"))
    );

    let result = transform_paste_data(PasteTransformParams {
        content: input.to_string(),
        format: PasteDataFormat::Json,
        columns: vec![name.index],
        controls,
        sample_row_count: 100,
        preview_smart_replacements: preview.smart_replacements,
    })
    .unwrap();

    assert!(result.output.contains("Smart Person 1"));
    assert!(result.output.contains("Smart Person 2"));
    assert_eq!(result.privacy_report.smart_replacement_columns, 1);
    assert_eq!(result.privacy_report.smart_replacement_values, 2);
    assert_eq!(result.privacy_report.smart_replacement_fallbacks, 0);
}

#[test]
fn paste_transform_reuses_preview_smart_replacements_and_generates_missing_values() {
    let input = r#"[
  {"name":"Ada Lovelace"},
  {"name":"Grace Hopper"},
  {"name":"Katherine Johnson"}
]"#;
    let analysis = analyze_paste_data(PasteAnalyzeParams {
        content: input.to_string(),
        format: PasteDataFormat::Json,
        sample_row_count: 10,
    })
    .unwrap();
    let name = analysis
        .columns
        .iter()
        .find(|column| column.name == "[].name")
        .unwrap();
    let controls = vec![ColumnControl {
        column_index: name.index,
        type_override: Some(DataType::FullName),
        strategy: AnonymizationStrategy::LocalAi,
    }];
    let mut preview_provider = PrefixSmartProvider;
    let preview = preview_paste_data_with_smart_provider(
        PastePreviewParams {
            content: input.to_string(),
            format: PasteDataFormat::Json,
            columns: vec![name.index],
            controls: controls.clone(),
            sample_count: 1,
            sample_row_count: 100,
        },
        Some(&mut preview_provider),
    )
    .unwrap();
    let mut transform_provider = RecordingSmartProvider::default();

    let result = transform_paste_data_with_smart_provider(
        PasteTransformParams {
            content: input.to_string(),
            format: PasteDataFormat::Json,
            columns: vec![name.index],
            controls,
            sample_row_count: 100,
            preview_smart_replacements: preview.smart_replacements,
        },
        Some(&mut transform_provider),
    )
    .unwrap();

    assert_eq!(
        transform_provider.requests,
        vec![vec!["Katherine Johnson".to_string()]]
    );
    assert!(result.output.contains("Smart Person 1"));
    assert!(result.output.contains("Smart Person 2"));
    assert!(result.output.contains("Generated Person 1"));
    assert!(!result.output.contains("Ada Lovelace"));
    assert!(!result.output.contains("Grace Hopper"));
    assert!(!result.output.contains("Katherine Johnson"));
    assert_eq!(result.privacy_report.smart_replacement_values, 3);
}

#[test]
fn paste_transform_rejects_invalid_preview_smart_replacements() {
    let input = r#"[{"name":"Ada Lovelace"}]"#;
    let analysis = analyze_paste_data(PasteAnalyzeParams {
        content: input.to_string(),
        format: PasteDataFormat::Json,
        sample_row_count: 10,
    })
    .unwrap();
    let name = analysis
        .columns
        .iter()
        .find(|column| column.name == "[].name")
        .unwrap();
    let controls = vec![ColumnControl {
        column_index: name.index,
        type_override: Some(DataType::FullName),
        strategy: AnonymizationStrategy::LocalAi,
    }];
    let mut provider = RecordingSmartProvider::default();

    let result = transform_paste_data_with_smart_provider(
        PasteTransformParams {
            content: input.to_string(),
            format: PasteDataFormat::Json,
            columns: vec![name.index],
            controls,
            sample_row_count: 100,
            preview_smart_replacements: vec![SmartReplacementEntry {
                column_index: name.index,
                original: "Ada Lovelace".to_string(),
                replacement: "Ada Lovelace".to_string(),
            }],
        },
        Some(&mut provider),
    )
    .unwrap();

    assert_eq!(provider.requests, vec![vec!["Ada Lovelace".to_string()]]);
    assert!(result.output.contains("Generated Person 1"));
    assert_eq!(result.privacy_report.smart_replacement_values, 1);
    assert_eq!(result.privacy_report.smart_replacement_rejections, 1);
    assert_eq!(
        result.privacy_report.smart_replacement_rejection_reasons,
        vec![SmartReplacementRejectionCount {
            reason: SmartReplacementRejectionReason::SameAsOriginal,
            count: 1,
        }]
    );
}

#[test]
fn quick_generation_uses_smart_replacements_when_requested() {
    let mut provider = PrefixSmartProvider;
    let result = generate_quick_values_with_smart_provider(
        QuickGenerateParams {
            data_type: DataType::FullName,
            strategy: AnonymizationStrategy::LocalAi,
            count: 2,
        },
        Some(&mut provider),
    )
    .unwrap();
    let lines = result.output.lines().collect::<Vec<_>>();

    assert_eq!(lines.len(), 2);
    assert!(lines[0].starts_with("Smart Person "));
    assert!(lines[1].starts_with("Smart Person "));
    assert_eq!(result.privacy_report.smart_replacement_columns, 1);
    assert_eq!(result.privacy_report.smart_replacement_values, 2);
    assert_eq!(result.privacy_report.smart_replacement_fallbacks, 0);
}

#[test]
fn quick_generation_requires_provider_for_smart_replacement() {
    let error = generate_quick_values(QuickGenerateParams {
        data_type: DataType::FullName,
        strategy: AnonymizationStrategy::LocalAi,
        count: 1,
    })
    .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("Smart replacement needs Local AI")
    );
}

#[test]
fn transforms_plain_text_and_preserves_surrounding_text() {
    let input =
        "contact ada@example.com from 192.168.0.10 request 550e8400-e29b-41d4-a716-446655440000";
    let analysis = analyze_paste_data(PasteAnalyzeParams {
        content: input.to_string(),
        format: PasteDataFormat::PlainText,
        sample_row_count: 10,
    })
    .unwrap();
    let selected = analysis
        .columns
        .iter()
        .filter(|column| column.name == "email" || column.name == "uuid")
        .map(|column| column.index)
        .collect::<Vec<_>>();

    let result = transform_paste_data(PasteTransformParams {
        content: input.to_string(),
        format: PasteDataFormat::PlainText,
        columns: selected,
        controls: Vec::new(),
        sample_row_count: 100,
        preview_smart_replacements: Vec::new(),
    })
    .unwrap();

    assert!(result.output.starts_with("contact "));
    assert!(result.output.contains(" from 192.168.0.10 request "));
    assert!(!result.output.contains("ada@example.com"));
    assert!(
        !result
            .output
            .contains("550e8400-e29b-41d4-a716-446655440000")
    );
}

#[test]
fn plain_text_detection_keeps_overlapping_tokens_single_pass() {
    let input = "profile=https://ada@example.com/users/42";
    let analysis = analyze_paste_data(PasteAnalyzeParams {
        content: input.to_string(),
        format: PasteDataFormat::PlainText,
        sample_row_count: 10,
    })
    .unwrap();

    let url = analysis
        .columns
        .iter()
        .find(|column| column.name == "url")
        .unwrap();
    assert!(!analysis.columns.iter().any(|column| column.name == "email"));

    let result = transform_paste_data(PasteTransformParams {
        content: input.to_string(),
        format: PasteDataFormat::PlainText,
        columns: vec![url.index],
        controls: Vec::new(),
        sample_row_count: 100,
        preview_smart_replacements: Vec::new(),
    })
    .unwrap();

    assert!(result.output.starts_with("profile="));
    assert!(!result.output.contains("https://ada@example.com/users/42"));
}

#[test]
fn auto_detects_logs_and_replaces_inline_values() {
    let input = "2026-06-25T12:00:00 ERROR user=jane@example.com ip=10.1.2.3";
    let analysis = analyze_paste_data(PasteAnalyzeParams {
        content: input.to_string(),
        format: PasteDataFormat::Auto,
        sample_row_count: 10,
    })
    .unwrap();
    assert_eq!(analysis.format, PasteDataFormat::Logs);

    let selected = analysis
        .columns
        .iter()
        .filter(|column| column.name == "email" || column.name == "ipAddress")
        .map(|column| column.index)
        .collect::<Vec<_>>();
    let result = transform_paste_data(PasteTransformParams {
        content: input.to_string(),
        format: analysis.format,
        columns: selected,
        controls: Vec::new(),
        sample_row_count: 100,
        preview_smart_replacements: Vec::new(),
    })
    .unwrap();

    assert!(result.output.contains("ERROR user="));
    assert!(!result.output.contains("jane@example.com"));
    assert!(!result.output.contains("10.1.2.3"));
}

struct PrefixSmartProvider;

impl SmartReplacementProvider for PrefixSmartProvider {
    fn generate_replacements(
        &mut self,
        request: SmartReplacementRequest<'_>,
    ) -> Result<Vec<SmartReplacement>> {
        Ok(request
            .values
            .iter()
            .enumerate()
            .map(|(index, value)| SmartReplacement {
                original: value.clone(),
                replacement: format!("Smart Person {}", index + 1),
            })
            .collect())
    }
}

#[derive(Default)]
struct RecordingSmartProvider {
    requests: Vec<Vec<String>>,
}

impl SmartReplacementProvider for RecordingSmartProvider {
    fn generate_replacements(
        &mut self,
        request: SmartReplacementRequest<'_>,
    ) -> Result<Vec<SmartReplacement>> {
        let values = request.values.to_vec();
        self.requests.push(values.clone());
        Ok(values
            .into_iter()
            .enumerate()
            .map(|(index, value)| SmartReplacement {
                original: value,
                replacement: format!("Generated Person {}", index + 1),
            })
            .collect())
    }
}
