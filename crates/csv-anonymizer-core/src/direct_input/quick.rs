use crate::detection::{classify_pii_risk, detect_column_type_with_name, detect_empty_format};
use crate::error::{AnonymizerError, Result};
use crate::hash::{deterministic_number, deterministic_string, deterministic_uuid, random_uuid_v4};
use crate::service::build_privacy_report;
use crate::smart::{SmartReplacementProvider, prepare_smart_replacements_from_rows};
use crate::strategies::{TransformState, transform_value_with_state};
use crate::types::{
    AnonymizationStrategy, ColumnMetadata, DataType, QuickGenerateParams, QuickTransformData,
    QuickTransformParams, SampleTransform, TransformContext,
};
use rand::Rng;

use super::shared::transform_state_for_smart_replacements;

const QUICK_GENERATE_MAX_COUNT: usize = 1_000;
const HEX_CHARSET: &str = "0123456789abcdef";

pub fn transform_quick_values(input: QuickTransformParams) -> Result<QuickTransformData> {
    let values = parse_quick_lines(&input.input);
    if values.is_empty() {
        return Err(AnonymizerError::input_parse(
            "quick values",
            "Paste at least one value to anonymize.",
        ));
    }

    let column = quick_column(input.data_type, input.strategy, &values);
    let selected_columns = vec![column.clone()];
    let mut state = TransformState::new(input.deterministic, &input.seed);
    let mut transformed = Vec::with_capacity(values.len());
    let mut samples = Vec::with_capacity(values.len());

    for (row_index, value) in values.iter().enumerate() {
        let context = TransformContext {
            column_name: &column.name,
            column_index: column.index,
            row_index,
            seed: &input.seed,
            deterministic: input.deterministic,
            empty_format: column.empty_format,
        };
        let anonymized = transform_value_with_state(value, &column, &context, &mut state);
        transformed.push(anonymized.clone());
        samples.push(SampleTransform {
            original: value.clone(),
            anonymized,
        });
    }

    Ok(QuickTransformData {
        output: transformed.join("\n"),
        row_count: transformed.len(),
        values: samples,
        privacy_report: build_privacy_report(
            &selected_columns,
            state.report(),
            input.deterministic,
        ),
    })
}

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
        .map(|row_index| {
            generated_quick_value(input.data_type, row_index, input.deterministic, &input.seed)
        })
        .collect::<Vec<_>>();
    let column = quick_column(input.data_type, input.strategy, &source_values);
    let selected_columns = vec![column.clone()];
    let source_rows = source_values
        .iter()
        .map(|value| vec![value.clone()])
        .collect::<Vec<_>>();
    let smart_replacements = prepare_smart_replacements_from_rows(
        &source_rows,
        &selected_columns,
        input.deterministic,
        &input.seed,
        None,
        provider,
    )?;
    let mut state = transform_state_for_smart_replacements(
        input.deterministic,
        &input.seed,
        smart_replacements,
    );
    let mut output_values = Vec::with_capacity(input.count);
    let mut samples = Vec::with_capacity(input.count);

    for (row_index, source_value) in source_values.iter().enumerate() {
        let anonymized = if should_transform_generated_value(input.data_type, input.strategy) {
            let context = TransformContext {
                column_name: &column.name,
                column_index: column.index,
                row_index,
                seed: &input.seed,
                deterministic: input.deterministic,
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
        privacy_report: build_privacy_report(
            &selected_columns,
            state.report(),
            input.deterministic,
        ),
    })
}
pub fn quick_anonymize_values(
    values: &[String],
    data_type: DataType,
    strategy: AnonymizationStrategy,
    deterministic: bool,
    seed: &str,
) -> Result<QuickTransformData> {
    transform_quick_values(QuickTransformParams {
        input: values.join("\n"),
        data_type,
        strategy,
        deterministic,
        seed: seed.to_string(),
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

fn parse_quick_lines(input: &str) -> Vec<String> {
    input
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect()
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
        || matches!(
            data_type,
            DataType::Email
                | DataType::Uuid
                | DataType::Timestamp
                | DataType::NumericId
                | DataType::NumericValue
                | DataType::Phone
                | DataType::FirstName
                | DataType::LastName
                | DataType::FullName
                | DataType::String
                | DataType::Unknown
        )
}

fn generated_quick_value(
    data_type: DataType,
    row_index: usize,
    deterministic: bool,
    seed: &str,
) -> String {
    let ordinal = row_index + 1;
    match data_type {
        DataType::Email => format!("person{ordinal}@example.invalid"),
        DataType::Uuid => generated_uuid(data_type, row_index, deterministic, seed),
        DataType::Timestamp => format!(
            "2024-{month:02}-{day:02}T{hour:02}:{minute:02}:00Z",
            month =
                generated_quick_number(data_type, row_index, deterministic, seed, "month", 1, 12),
            day = generated_quick_number(data_type, row_index, deterministic, seed, "day", 1, 28),
            hour = generated_quick_number(data_type, row_index, deterministic, seed, "hour", 0, 23),
            minute =
                generated_quick_number(data_type, row_index, deterministic, seed, "minute", 0, 59),
        ),
        DataType::NumericId => generated_quick_number(
            data_type,
            row_index,
            deterministic,
            seed,
            "numeric-id",
            100_000,
            999_999,
        )
        .to_string(),
        DataType::NumericValue => format!(
            "{}.{:02}",
            generated_quick_number(
                data_type,
                row_index,
                deterministic,
                seed,
                "numeric-value",
                100,
                9_999
            ),
            generated_quick_number(data_type, row_index, deterministic, seed, "fraction", 0, 99),
        ),
        DataType::PostalCode => format!(
            "{:05}",
            generated_quick_number(
                data_type,
                row_index,
                deterministic,
                seed,
                "postal-code",
                1_000,
                99_950
            )
        ),
        DataType::Address => format!(
            "{} {} {}, {}",
            generated_quick_number(
                data_type,
                row_index,
                deterministic,
                seed,
                "house",
                10,
                9_999
            ),
            generated_quick_choice(
                &["Cedar", "Maple", "Oak", "Pine", "River", "Summit"],
                data_type,
                row_index,
                deterministic,
                seed,
                "street",
            ),
            generated_quick_choice(
                &["Street", "Avenue", "Lane", "Road", "Way"],
                data_type,
                row_index,
                deterministic,
                seed,
                "suffix",
            ),
            generated_quick_choice(
                &[
                    "Arborfield",
                    "Brookhaven",
                    "Fairview",
                    "Lakeside",
                    "Riverton"
                ],
                data_type,
                row_index,
                deterministic,
                seed,
                "city",
            ),
        ),
        DataType::IpAddress => format!(
            "198.51.100.{}",
            generated_quick_number(data_type, row_index, deterministic, seed, "host", 1, 254)
        ),
        DataType::Url => format!(
            "https://example.invalid/{}/{}",
            generated_quick_choice(
                &["accounts", "orders", "profiles", "reports", "sessions"],
                data_type,
                row_index,
                deterministic,
                seed,
                "path",
            ),
            generated_quick_number(
                data_type,
                row_index,
                deterministic,
                seed,
                "id",
                1_000,
                99_999
            ),
        ),
        DataType::MacAddress => format!(
            "02:00:{}:{}:{}:{}",
            generated_quick_hex_pair(data_type, row_index, deterministic, seed, "mac-1"),
            generated_quick_hex_pair(data_type, row_index, deterministic, seed, "mac-2"),
            generated_quick_hex_pair(data_type, row_index, deterministic, seed, "mac-3"),
            generated_quick_hex_pair(data_type, row_index, deterministic, seed, "mac-4"),
        ),
        DataType::TaxId => format!(
            "900-{:02}-{:04}",
            generated_quick_number(data_type, row_index, deterministic, seed, "group", 1, 99),
            generated_quick_number(
                data_type,
                row_index,
                deterministic,
                seed,
                "serial",
                1,
                9_999
            ),
        ),
        DataType::Boolean => {
            generated_quick_bool(data_type, row_index, deterministic, seed).to_string()
        }
        DataType::Currency => format!(
            "${}.{:02}",
            generated_quick_number(
                data_type,
                row_index,
                deterministic,
                seed,
                "dollars",
                10,
                50_000
            ),
            generated_quick_number(data_type, row_index, deterministic, seed, "cents", 0, 99),
        ),
        DataType::Percentage => format!(
            "{}.{}%",
            generated_quick_number(data_type, row_index, deterministic, seed, "whole", 0, 100),
            generated_quick_number(data_type, row_index, deterministic, seed, "decimal", 0, 9),
        ),
        DataType::CountryCode => generated_quick_choice(
            &["US", "NL", "DE", "FR", "GB", "CA", "AU", "JP"],
            data_type,
            row_index,
            deterministic,
            seed,
            "country",
        )
        .to_string(),
        DataType::Phone => format!(
            "555-020-{:04}",
            generated_quick_number(data_type, row_index, deterministic, seed, "phone", 0, 9_999)
        ),
        DataType::FirstName => format!("First{ordinal}"),
        DataType::LastName => format!("Last{ordinal}"),
        DataType::FullName => format!("First{ordinal} Last{ordinal}"),
        DataType::Enum => generated_quick_choice(
            &["active", "pending", "review", "archived", "closed"],
            data_type,
            row_index,
            deterministic,
            seed,
            "enum",
        )
        .to_string(),
        DataType::String => format!(
            "sample-{}-{}",
            ordinal,
            generated_quick_string(data_type, row_index, deterministic, seed, "string", 8)
        ),
        DataType::Unknown => format!(
            "value-{}-{}",
            ordinal,
            generated_quick_string(data_type, row_index, deterministic, seed, "unknown", 8)
        ),
    }
}

fn generated_quick_number(
    data_type: DataType,
    row_index: usize,
    deterministic: bool,
    seed: &str,
    label: &str,
    min: i64,
    max: i64,
) -> i64 {
    if deterministic {
        let key = quick_generation_key(data_type, row_index);
        deterministic_number(&key, &format!("{seed}:quick-generate:{label}"), min, max)
    } else {
        rand::thread_rng().gen_range(min..=max)
    }
}

fn generated_quick_bool(
    data_type: DataType,
    row_index: usize,
    deterministic: bool,
    seed: &str,
) -> bool {
    generated_quick_number(data_type, row_index, deterministic, seed, "bool", 0, 1) == 1
}

fn generated_quick_choice<'a>(
    choices: &'a [&'a str],
    data_type: DataType,
    row_index: usize,
    deterministic: bool,
    seed: &str,
    label: &str,
) -> &'a str {
    let index = generated_quick_number(
        data_type,
        row_index,
        deterministic,
        seed,
        label,
        0,
        choices.len().saturating_sub(1) as i64,
    ) as usize;
    choices[index]
}

fn generated_quick_hex_pair(
    data_type: DataType,
    row_index: usize,
    deterministic: bool,
    seed: &str,
    label: &str,
) -> String {
    generated_quick_string(data_type, row_index, deterministic, seed, label, 2)
}

fn generated_quick_string(
    data_type: DataType,
    row_index: usize,
    deterministic: bool,
    seed: &str,
    label: &str,
    length: usize,
) -> String {
    if deterministic {
        let key = quick_generation_key(data_type, row_index);
        deterministic_string(
            &key,
            &format!("{seed}:quick-generate:{label}"),
            length,
            HEX_CHARSET,
        )
    } else {
        let chars = HEX_CHARSET.as_bytes();
        let mut rng = rand::thread_rng();
        (0..length)
            .map(|_| chars[rng.gen_range(0..chars.len())] as char)
            .collect()
    }
}

fn generated_uuid(
    data_type: DataType,
    row_index: usize,
    deterministic: bool,
    seed: &str,
) -> String {
    if deterministic {
        deterministic_uuid(
            &quick_generation_key(data_type, row_index),
            &format!("{seed}:quick-generate:uuid"),
        )
    } else {
        random_uuid_v4()
    }
}

fn quick_generation_key(data_type: DataType, row_index: usize) -> String {
    format!("quick-generate:{data_type:?}:{row_index}")
}
