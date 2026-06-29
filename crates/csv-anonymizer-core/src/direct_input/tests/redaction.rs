use crate::direct_input::{
    analyze_paste_data, preview_paste_data,
    shared::{FieldSamples, fields_to_rows, metadata_from_fields},
    transform_paste_data,
};
use crate::types::{
    AnonymizationStrategy, ColumnControl, ColumnMetadata, DataType, PasteAnalyzeData,
    PasteAnalyzeParams, PasteDataFormat, PastePreviewParams, PasteTransformData,
    PasteTransformParams, PiiRisk, PreviewData, PrivacyFindingKind,
};

const SEED: &str = "seed";
const SCALAR_WARNING: &str = "may change scalar value types";

fn analyze(content: &str, format: PasteDataFormat) -> PasteAnalyzeData {
    analyze_paste_data(PasteAnalyzeParams {
        content: content.to_string(),
        format,
        sample_row_count: 10,
    })
    .unwrap()
}

fn column_named<'a>(analysis: &'a PasteAnalyzeData, name: &str) -> &'a ColumnMetadata {
    analysis
        .columns
        .iter()
        .find(|column| column.name == name)
        .unwrap_or_else(|| panic!("missing column {name}"))
}

fn preview(
    content: &str,
    format: PasteDataFormat,
    columns: Vec<usize>,
    controls: Vec<ColumnControl>,
) -> PreviewData {
    preview_paste_data(PastePreviewParams {
        content: content.to_string(),
        format,
        columns,
        controls,
        deterministic: true,
        seed: SEED.to_string(),
        sample_count: 3,
    })
    .unwrap()
}

fn transform(
    content: &str,
    format: PasteDataFormat,
    columns: Vec<usize>,
    controls: Vec<ColumnControl>,
) -> PasteTransformData {
    transform_paste_data(PasteTransformParams {
        content: content.to_string(),
        format,
        columns,
        controls,
        deterministic: true,
        seed: SEED.to_string(),
        preview_smart_replacements: Vec::new(),
    })
    .unwrap()
}

fn redact_control(column: &ColumnMetadata) -> ColumnControl {
    ColumnControl {
        column_index: column.index,
        type_override: None,
        strategy: AnonymizationStrategy::Redact,
    }
}

fn assert_scalar_warning(preview: &PreviewData) {
    assert!(
        preview
            .warnings
            .iter()
            .any(|warning| warning.message.contains(SCALAR_WARNING))
    );
}

fn assert_scalar_note(result: &PasteTransformData) {
    assert!(
        result
            .privacy_report
            .notes
            .iter()
            .any(|note| note.contains(SCALAR_WARNING))
    );
}

#[test]
fn analyzes_and_transforms_json_array() {
    let input = r#"[
  {"email":"ada@example.com","id":"123456"},
  {"email":"grace@example.com","id":"987654"}
]"#;
    let analysis = analyze(input, PasteDataFormat::Json);
    let email = column_named(&analysis, "[].email");

    let result = transform(input, PasteDataFormat::Json, vec![email.index], Vec::new());

    assert_eq!(email.detected_type, DataType::Email);
    assert!(result.output.contains("[EMAIL]"));
    assert!(!result.output.contains("ada@example.com"));
    assert_eq!(result.privacy_report.redacted_columns, 1);
    assert_eq!(result.columns_anonymized, 1);
}

#[test]
fn json_direct_input_defaults_sensitive_fields_to_redact() {
    let input = r#"{
  "id": 93019,
  "username": "johndoe",
  "email": "rpawyc801@example.com",
  "dateOfBirth": "1989-07-01",
  "phoneNumber": "+1-555-0123",
  "address": {
    "street": "19nilkXSvWfHvlYr",
    "zipCode": "81711"
  },
  "lastLoginAt": "2024-12-15T14:22:00"
}"#;
    let analysis = analyze(input, PasteDataFormat::Json);

    assert_eq!(
        column_named(&analysis, "email").strategy,
        AnonymizationStrategy::Redact
    );
    let username = column_named(&analysis, "username");
    assert_eq!(username.strategy, AnonymizationStrategy::Redact);
    assert_eq!(username.detected_type, DataType::String);
    assert!(username.privacy_evidence.iter().any(|summary| {
        summary.kind == PrivacyFindingKind::AccountOrFinancialId
            && summary.data_type == DataType::String
    }));
    assert_eq!(
        column_named(&analysis, "phoneNumber").detected_type,
        DataType::Phone
    );
    assert_eq!(
        column_named(&analysis, "phoneNumber").strategy,
        AnonymizationStrategy::Redact
    );
    assert_eq!(
        column_named(&analysis, "address.zipCode").detected_type,
        DataType::PostalCode
    );
    assert_eq!(
        column_named(&analysis, "address.zipCode").strategy,
        AnonymizationStrategy::Redact
    );
    assert_eq!(
        column_named(&analysis, "dateOfBirth").strategy,
        AnonymizationStrategy::Redact
    );
    assert_eq!(
        column_named(&analysis, "lastLoginAt").pii_risk,
        PiiRisk::Medium
    );
    assert_eq!(
        column_named(&analysis, "lastLoginAt").strategy,
        AnonymizationStrategy::Redact
    );

    let selected = analysis
        .columns
        .iter()
        .filter(|column| matches!(column.pii_risk, PiiRisk::High | PiiRisk::Medium))
        .map(|column| column.index)
        .collect::<Vec<_>>();
    let result = transform(input, PasteDataFormat::Json, selected, Vec::new());

    assert!(result.output.contains("\"email\": \"[EMAIL]\""));
    assert!(result.output.contains("\"username\": \"[ACCOUNT_ID]\""));
    assert!(result.output.contains("\"phoneNumber\": \"[PHONE]\""));
    assert!(result.output.contains("\"zipCode\": \"[ADDRESS]\""));
    assert!(result.output.contains("\"dateOfBirth\": \"[DATE]\""));
    assert!(result.output.contains("\"lastLoginAt\": \"[DATE]\""));
    assert!(!result.output.contains("rpawyc801@example.com"));
    assert!(!result.output.contains("johndoe"));
}

#[test]
fn json_direct_input_can_override_redact_default() {
    let input = r#"[{"email":"ada@example.com"}]"#;
    let analysis = analyze(input, PasteDataFormat::Json);
    let email = column_named(&analysis, "[].email");

    let result = transform(
        input,
        PasteDataFormat::Json,
        vec![email.index],
        vec![ColumnControl {
            column_index: email.index,
            type_override: None,
            strategy: AnonymizationStrategy::Pseudonymize,
        }],
    );

    assert!(result.output.contains("@example.com"));
    assert!(!result.output.contains("[EMAIL]"));
    assert!(!result.output.contains("ada@example.com"));
    assert_eq!(result.privacy_report.pseudonymized_columns, 1);
    assert_eq!(result.privacy_report.redacted_columns, 0);
}

#[test]
fn redacting_json_numeric_scalars_warns_for_default_and_manual_strategy() {
    let default_input = r#"{"id":93019}"#;
    let default_analysis = analyze(default_input, PasteDataFormat::Json);
    let id = column_named(&default_analysis, "id");
    let default_preview = preview(
        default_input,
        PasteDataFormat::Json,
        vec![id.index],
        Vec::new(),
    );
    let default_result = transform(
        default_input,
        PasteDataFormat::Json,
        vec![id.index],
        Vec::new(),
    );

    assert_scalar_warning(&default_preview);
    assert!(default_result.output.contains("\"id\": \"[ACCOUNT_ID]\""));
    assert_scalar_note(&default_result);

    let manual_input = r#"{"age":42}"#;
    let manual_analysis = analyze(manual_input, PasteDataFormat::Json);
    let age = column_named(&manual_analysis, "age");
    let controls = vec![redact_control(age)];
    let manual_preview = preview(
        manual_input,
        PasteDataFormat::Json,
        vec![age.index],
        controls.clone(),
    );
    let manual_result = transform(
        manual_input,
        PasteDataFormat::Json,
        vec![age.index],
        controls,
    );

    assert_scalar_warning(&manual_preview);
    assert!(manual_result.output.contains("\"age\": \"[REDACTED]\""));
    assert_scalar_note(&manual_result);
}

#[test]
fn redacting_yaml_scalar_values_warns_about_type_changes() {
    let default_input = "id: 93019\n";
    let default_analysis = analyze(default_input, PasteDataFormat::Yaml);
    let id = column_named(&default_analysis, "id");
    let default_preview = preview(
        default_input,
        PasteDataFormat::Yaml,
        vec![id.index],
        Vec::new(),
    );
    let default_result = transform(
        default_input,
        PasteDataFormat::Yaml,
        vec![id.index],
        Vec::new(),
    );

    assert_scalar_warning(&default_preview);
    assert!(default_result.output.contains("[ACCOUNT_ID]"));
    assert_scalar_note(&default_result);

    let manual_input = "enabled: true\n";
    let manual_analysis = analyze(manual_input, PasteDataFormat::Yaml);
    let enabled = column_named(&manual_analysis, "enabled");
    let controls = vec![redact_control(enabled)];
    let manual_preview = preview(
        manual_input,
        PasteDataFormat::Yaml,
        vec![enabled.index],
        controls.clone(),
    );
    let manual_result = transform(
        manual_input,
        PasteDataFormat::Yaml,
        vec![enabled.index],
        controls,
    );

    assert_scalar_warning(&manual_preview);
    assert!(manual_result.output.contains("[REDACTED]"));
    assert_scalar_note(&manual_result);
}

#[test]
fn typed_field_override_preserves_existing_privacy_risk_before_redact_default() {
    let fields = vec![FieldSamples {
        source_path: Some("text/username".to_string()),
        name: "username".to_string(),
        values: vec!["johndoe".to_string()],
        data_type: Some(DataType::String),
    }];
    let (headers, rows) = fields_to_rows(&fields, 10);
    let metadata = metadata_from_fields(&fields, &headers, &rows);
    let username = &metadata[0];

    assert_eq!(username.detected_type, DataType::String);
    assert_eq!(username.pii_risk, PiiRisk::High);
    assert_eq!(username.strategy, AnonymizationStrategy::Redact);
}
