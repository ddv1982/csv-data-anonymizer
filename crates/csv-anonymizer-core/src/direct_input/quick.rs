use crate::detection::{classify_pii_risk, detect_column_type_with_name, detect_empty_format};
use crate::error::{AnonymizerError, Result};
use crate::hash::random_uuid_v4;
use crate::service::build_privacy_report;
use crate::smart::{SmartReplacementProvider, prepare_smart_replacements_from_rows};
use crate::strategies::transform_value_with_state;
use crate::types::{
    AnonymizationStrategy, ColumnMetadata, DataType, QuickGenerateParams, QuickTransformData,
    SampleTransform, TransformContext,
};
use rand::Rng;

use super::shared::transform_state_for_smart_replacements;

const QUICK_GENERATE_MAX_COUNT: usize = 1_000;
const HEX_CHARSET: &str = "0123456789abcdef";

pub fn generate_quick_values(input: QuickGenerateParams) -> Result<QuickTransformData> {
    generate_quick_values_with_smart_provider(input, None)
}

pub fn generate_quick_values_with_smart_provider(
    input: QuickGenerateParams,
    provider: Option<&mut dyn SmartReplacementProvider>,
) -> Result<QuickTransformData> {
    if input.count == 0 {
        return Err(AnonymizerError::input_parse(
            "quick generation",
            "Generate at least one value.",
        ));
    }
    if input.count > QUICK_GENERATE_MAX_COUNT {
        return Err(AnonymizerError::input_parse(
            "quick generation",
            format!("Generate no more than {QUICK_GENERATE_MAX_COUNT} values at a time."),
        ));
    }
    if !supports_quick_generate_strategy(input.strategy) {
        return Err(AnonymizerError::input_parse(
            "quick generation",
            "Quick generation supports auto, pseudonymize, tokenize, or smart replacement.",
        ));
    }

    let source_values = (0..input.count)
        .map(|row_index| generated_quick_value(input.data_type, row_index))
        .collect::<Vec<_>>();
    let column = quick_column(input.data_type, input.strategy, &source_values);
    let selected_columns = vec![column.clone()];
    let source_rows = source_values
        .iter()
        .map(|value| vec![value.clone()])
        .collect::<Vec<_>>();
    let smart_replacements =
        prepare_smart_replacements_from_rows(&source_rows, &selected_columns, None, provider)?;
    let mut state = transform_state_for_smart_replacements(smart_replacements);
    let mut output_values = Vec::with_capacity(input.count);
    let mut samples = Vec::with_capacity(input.count);

    for (row_index, source_value) in source_values.iter().enumerate() {
        let anonymized = if should_transform_generated_value(input.data_type, input.strategy) {
            let context = TransformContext {
                column_name: &column.name,
                column_index: column.index,
                row_index,
                empty_format: column.empty_format,
            };
            transform_value_with_state(source_value, &column, &context, &mut state)
        } else {
            source_value.clone()
        };

        output_values.push(anonymized.clone());
        samples.push(SampleTransform {
            original: source_value.clone(),
            anonymized,
        });
    }

    Ok(QuickTransformData {
        output: output_values.join("\n"),
        row_count: output_values.len(),
        values: samples,
        privacy_report: build_privacy_report(&selected_columns, state.report()),
    })
}

fn quick_column(
    data_type: DataType,
    strategy: AnonymizationStrategy,
    values: &[String],
) -> ColumnMetadata {
    let detection = detect_column_type_with_name("value", values);
    ColumnMetadata {
        name: "value".to_string(),
        source_path: None,
        index: 0,
        detected_type: data_type,
        confidence: detection.confidence,
        detection_trace: detection.trace,
        privacy_findings: Vec::new(),
        privacy_evidence: Vec::new(),
        pii_risk: classify_pii_risk(data_type),
        sample_values: values
            .iter()
            .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("null"))
            .take(5)
            .cloned()
            .collect(),
        empty_format: detect_empty_format(values),
        is_selected: true,
        strategy,
    }
}

fn supports_quick_generate_strategy(strategy: AnonymizationStrategy) -> bool {
    matches!(
        strategy,
        AnonymizationStrategy::Auto
            | AnonymizationStrategy::Pseudonymize
            | AnonymizationStrategy::Tokenize
            | AnonymizationStrategy::LocalAi
    )
}

fn should_transform_generated_value(data_type: DataType, strategy: AnonymizationStrategy) -> bool {
    strategy == AnonymizationStrategy::Tokenize
        || strategy == AnonymizationStrategy::LocalAi
        || data_type.transforms_generated_quick_value()
}

fn generated_quick_value(data_type: DataType, row_index: usize) -> String {
    let ordinal = row_index + 1;
    match data_type {
        DataType::Email
        | DataType::Phone
        | DataType::FirstName
        | DataType::LastName
        | DataType::FullName => generated_person_quick_value(data_type, ordinal),
        DataType::Uuid | DataType::IpAddress | DataType::Url | DataType::MacAddress => {
            generated_network_quick_value(data_type)
        }
        DataType::Timestamp
        | DataType::NumericId
        | DataType::NumericValue
        | DataType::PostalCode
        | DataType::TaxId
        | DataType::Boolean
        | DataType::Currency
        | DataType::Percentage => generated_scalar_quick_value(data_type),
        DataType::Address | DataType::CountryCode | DataType::Enum => {
            generated_choice_quick_value(data_type)
        }
        DataType::String => generated_text_quick_value("sample", ordinal),
        DataType::Unknown => generated_text_quick_value("value", ordinal),
    }
}

fn generated_person_quick_value(data_type: DataType, ordinal: usize) -> String {
    match data_type {
        DataType::Email => format!("person{ordinal}@example.invalid"),
        DataType::Phone => format!("555-020-{:04}", generated_quick_number(0, 9_999)),
        DataType::FirstName => format!("First{ordinal}"),
        DataType::LastName => format!("Last{ordinal}"),
        DataType::FullName => format!("First{ordinal} Last{ordinal}"),
        _ => unreachable!("person quick value helper called with non-person type"),
    }
}

fn generated_network_quick_value(data_type: DataType) -> String {
    match data_type {
        DataType::Uuid => generated_uuid(),
        DataType::IpAddress => format!("198.51.100.{}", generated_quick_number(1, 254)),
        DataType::Url => format!(
            "https://example.invalid/{}/{}",
            generated_quick_choice(&["accounts", "orders", "profiles", "reports", "sessions"]),
            generated_quick_number(1_000, 99_999),
        ),
        DataType::MacAddress => format!(
            "02:00:{}:{}:{}:{}",
            generated_quick_hex_pair(),
            generated_quick_hex_pair(),
            generated_quick_hex_pair(),
            generated_quick_hex_pair(),
        ),
        _ => unreachable!("network quick value helper called with non-network type"),
    }
}

fn generated_scalar_quick_value(data_type: DataType) -> String {
    match data_type {
        DataType::Timestamp => format!(
            "2024-{month:02}-{day:02}T{hour:02}:{minute:02}:00Z",
            month = generated_quick_number(1, 12),
            day = generated_quick_number(1, 28),
            hour = generated_quick_number(0, 23),
            minute = generated_quick_number(0, 59),
        ),
        DataType::NumericId => generated_quick_number(100_000, 999_999).to_string(),
        DataType::NumericValue => format!(
            "{}.{:02}",
            generated_quick_number(100, 9_999),
            generated_quick_number(0, 99),
        ),
        DataType::PostalCode => format!("{:05}", generated_quick_number(1_000, 99_950)),
        DataType::TaxId => format!(
            "900-{:02}-{:04}",
            generated_quick_number(1, 99),
            generated_quick_number(1, 9_999),
        ),
        DataType::Boolean => generated_quick_bool().to_string(),
        DataType::Currency => format!(
            "${}.{:02}",
            generated_quick_number(10, 50_000),
            generated_quick_number(0, 99),
        ),
        DataType::Percentage => format!(
            "{}.{}%",
            generated_quick_number(0, 100),
            generated_quick_number(0, 9),
        ),
        _ => unreachable!("scalar quick value helper called with non-scalar type"),
    }
}

fn generated_choice_quick_value(data_type: DataType) -> String {
    match data_type {
        DataType::Address => format!(
            "{} {} {}, {}",
            generated_quick_number(10, 9_999),
            generated_quick_choice(&["Cedar", "Maple", "Oak", "Pine", "River", "Summit"]),
            generated_quick_choice(&["Street", "Avenue", "Lane", "Road", "Way"]),
            generated_quick_choice(&[
                "Arborfield",
                "Brookhaven",
                "Fairview",
                "Lakeside",
                "Riverton",
            ]),
        ),
        DataType::CountryCode => {
            generated_quick_choice(&["US", "NL", "DE", "FR", "GB", "CA", "AU", "JP"]).to_string()
        }
        DataType::Enum => {
            generated_quick_choice(&["active", "pending", "review", "archived", "closed"])
                .to_string()
        }
        _ => unreachable!("choice quick value helper called with non-choice type"),
    }
}

fn generated_text_quick_value(prefix: &str, ordinal: usize) -> String {
    format!("{prefix}-{ordinal}-{}", generated_quick_string(8))
}

fn generated_quick_number(min: i64, max: i64) -> i64 {
    rand::thread_rng().gen_range(min..=max)
}

fn generated_quick_bool() -> bool {
    generated_quick_number(0, 1) == 1
}

fn generated_quick_choice<'a>(choices: &'a [&'a str]) -> &'a str {
    let index = generated_quick_number(0, choices.len().saturating_sub(1) as i64) as usize;
    choices[index]
}

fn generated_quick_hex_pair() -> String {
    generated_quick_string(2)
}

fn generated_quick_string(length: usize) -> String {
    let chars = HEX_CHARSET.as_bytes();
    let mut rng = rand::thread_rng();
    (0..length)
        .map(|_| chars[rng.gen_range(0..chars.len())] as char)
        .collect()
}

fn generated_uuid() -> String {
    random_uuid_v4()
}
