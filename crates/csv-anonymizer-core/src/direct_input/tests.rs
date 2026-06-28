use super::*;
use crate::metadata::build_column_metadata;
use crate::smart::{SmartReplacement, SmartReplacementProvider, SmartReplacementRequest};
use crate::types::PiiRisk;

#[test]
fn analyzes_and_transforms_json_array() {
    let input = r#"[
  {"email":"ada@example.com","id":"123456"},
  {"email":"grace@example.com","id":"987654"}
]"#;
    let analysis = analyze_paste_data(PasteAnalyzeParams {
        content: input.to_string(),
        format: PasteDataFormat::Json,
        sample_row_count: 10,
    })
    .unwrap();

    let email = analysis
        .columns
        .iter()
        .find(|column| column.name == "[].email")
        .unwrap();
    assert_eq!(email.detected_type, DataType::Email);

    let result = transform_paste_data(PasteTransformParams {
        content: input.to_string(),
        format: PasteDataFormat::Json,
        columns: vec![email.index],
        controls: Vec::new(),
        deterministic: true,
        seed: "seed".to_string(),
        preview_smart_replacements: Vec::new(),
    })
    .unwrap();

    assert!(result.output.contains("@example.com"));
    assert!(!result.output.contains("ada@example.com"));
}

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
        deterministic: true,
        seed: "seed".to_string(),
        preview_smart_replacements: Vec::new(),
    })
    .unwrap();

    assert!(result.output.starts_with("email,name\n"));
    assert!(!result.output.contains("ada@example.com"));
}

#[test]
fn transforms_quick_values_with_one_state() {
    let result = transform_quick_values(QuickTransformParams {
        input: "ada@example.com\nada@example.com".to_string(),
        data_type: DataType::Email,
        strategy: AnonymizationStrategy::Auto,
        deterministic: true,
        seed: "seed".to_string(),
    })
    .unwrap();
    let lines = result.output.lines().collect::<Vec<_>>();

    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], lines[1]);
    assert_ne!(lines[0], "ada@example.com");
}

#[test]
fn row_helpers_preview_and_anonymize_rows() {
    let rows = vec![vec!["ada@example.com".to_string()]];
    let headers = vec!["email".to_string()];
    let metadata = build_column_metadata(&headers, &rows);

    let preview = preview_rows(&rows, &metadata, &[0], &[], true, "seed", 3).unwrap();
    assert_eq!(preview.previews[0].samples[0].original, "ada@example.com");

    let (output, report) = anonymize_rows(&rows, &metadata, &[0], &[], true, "seed").unwrap();
    assert_eq!(output.len(), 1);
    assert_ne!(output[0][0], "ada@example.com");
    assert_eq!(report.direct_identifiers, 1);
}

#[test]
fn direct_input_preview_includes_selected_column_warnings() {
    let rows = vec![vec!["ada@example.com".to_string(), "US".to_string()]];
    let headers = vec!["email".to_string(), "country".to_string()];
    let metadata = build_column_metadata(&headers, &rows);
    let preview = preview_rows(
        &rows,
        &metadata,
        &[0, 1],
        &[
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
        true,
        "seed",
        3,
    )
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
fn direct_input_rejects_deterministic_blank_seed() {
    let error = transform_quick_values(QuickTransformParams {
        input: "ada@example.com".to_string(),
        data_type: DataType::Email,
        strategy: AnonymizationStrategy::Auto,
        deterministic: true,
        seed: " ".to_string(),
    })
    .unwrap_err();

    assert!(error.to_string().contains("non-empty private seed"));
}

#[test]
fn paste_transform_rejects_deterministic_blank_seed() {
    let error = transform_paste_data(PasteTransformParams {
        content: r#"[{"email":"ada@example.com"}]"#.to_string(),
        format: PasteDataFormat::Json,
        columns: vec![0],
        controls: Vec::new(),
        deterministic: true,
        seed: " ".to_string(),
        preview_smart_replacements: Vec::new(),
    })
    .unwrap_err();

    assert!(error.to_string().contains("non-empty private seed"));
}

#[test]
fn quick_anonymize_values_wrapper_uses_one_state() {
    let values = vec![
        "550e8400-e29b-41d4-a716-446655440000".to_string(),
        "550e8400-e29b-41d4-a716-446655440000".to_string(),
    ];
    let result = quick_anonymize_values(
        &values,
        DataType::Uuid,
        AnonymizationStrategy::Tokenize,
        true,
        "seed",
    )
    .unwrap();
    let lines = result.output.lines().collect::<Vec<_>>();

    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], lines[1]);
    assert_ne!(lines[0], values[0]);
}

#[test]
fn generates_quick_values_without_user_input() {
    let result = generate_quick_values(QuickGenerateParams {
        data_type: DataType::Email,
        strategy: AnonymizationStrategy::Auto,
        count: 2,
        deterministic: true,
        seed: "seed".to_string(),
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
        deterministic: true,
        seed: "seed".to_string(),
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
        deterministic: true,
        seed: "seed".to_string(),
    })
    .unwrap();

    assert!(ip_result.output.starts_with("198.51.100."));

    let phone_result = generate_quick_values(QuickGenerateParams {
        data_type: DataType::Phone,
        strategy: AnonymizationStrategy::Auto,
        count: 1,
        deterministic: true,
        seed: "seed".to_string(),
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
        deterministic: true,
        seed: "seed".to_string(),
    })
    .unwrap();
    let name = name_result.output.as_str();

    assert_ne!(name, "First1 Last1");
    assert_eq!(name.split_whitespace().count(), 2);

    let timestamp_result = generate_quick_values(QuickGenerateParams {
        data_type: DataType::Timestamp,
        strategy: AnonymizationStrategy::Auto,
        count: 1,
        deterministic: true,
        seed: "seed".to_string(),
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
        deterministic: true,
        seed: "seed".to_string(),
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
        deterministic: true,
        seed: "seed".to_string(),
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
        deterministic: true,
        seed: "seed".to_string(),
        preview_smart_replacements: Vec::new(),
    })
    .unwrap();

    assert!(!result.output.contains("ada@example.com"));
    assert!(!result.output.contains("Ada Lovelace"));
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
        deterministic: true,
        seed: "seed".to_string(),
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
        deterministic: true,
        seed: "seed".to_string(),
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

#[test]
fn rejects_excessive_paste_sample_counts() {
    let error = analyze_paste_data(PasteAnalyzeParams {
        content: "email\nada@example.com\n".to_string(),
        format: PasteDataFormat::Csv,
        sample_row_count: super::shared::PASTE_MAX_SAMPLE_ROWS + 1,
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
        deterministic: true,
        seed: "seed".to_string(),
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
        deterministic: true,
        seed: "seed".to_string(),
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
        deterministic: true,
        seed: "seed".to_string(),
        sample_count: 5,
    })
    .unwrap();

    assert_eq!(preview.previews[0].column_name, "[].email");
    assert_eq!(preview.previews[0].samples[0].original, "ada@example.com");
    assert_ne!(preview.previews[0].samples[0].anonymized, "ada@example.com");
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
            deterministic: true,
            seed: "seed".to_string(),
            sample_count: 5,
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
        deterministic: true,
        seed: "seed".to_string(),
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
            deterministic: true,
            seed: "seed".to_string(),
            sample_count: 1,
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
            deterministic: true,
            seed: "seed".to_string(),
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
fn quick_generation_uses_smart_replacements_when_requested() {
    let mut provider = PrefixSmartProvider;
    let result = generate_quick_values_with_smart_provider(
        QuickGenerateParams {
            data_type: DataType::FullName,
            strategy: AnonymizationStrategy::LocalAi,
            count: 2,
            deterministic: true,
            seed: "seed".to_string(),
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
        deterministic: true,
        seed: "seed".to_string(),
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
        deterministic: true,
        seed: "seed".to_string(),
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
        deterministic: true,
        seed: "seed".to_string(),
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
        deterministic: true,
        seed: "seed".to_string(),
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
