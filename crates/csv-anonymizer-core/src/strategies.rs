use crate::detection::is_empty_value;
use crate::hash::{deterministic_number, deterministic_string, deterministic_uuid};
use crate::types::{ColumnMetadata, DataType, TransformContext};
use chrono::{Duration, NaiveDate};
use rand::Rng;

pub fn transform_value(
    value: &str,
    column: &ColumnMetadata,
    context: &TransformContext<'_>,
) -> String {
    if is_empty_value(value) {
        return value.to_string();
    }

    match column.detected_type {
        DataType::Email => transform_email(value, context),
        DataType::Uuid => transform_uuid(value, context),
        DataType::Timestamp => transform_timestamp(value, context),
        DataType::NumericId => transform_numeric_id(value, context),
        DataType::Phone
        | DataType::FirstName
        | DataType::LastName
        | DataType::FullName
        | DataType::String => transform_generic_string(value, context),
        DataType::CountryCode | DataType::Enum | DataType::Unknown => value.to_string(),
    }
}

pub fn transform_row(
    row: &[String],
    columns: &[ColumnMetadata],
    row_index: usize,
    seed: &str,
    deterministic: bool,
) -> Vec<String> {
    row.iter()
        .enumerate()
        .map(|(column_index, value)| {
            let Some(column) = columns.get(column_index) else {
                return value.clone();
            };

            if !column.is_selected {
                return value.clone();
            }

            let context = TransformContext {
                column_name: &column.name,
                column_index: column.index,
                row_index,
                seed,
                deterministic,
                empty_format: column.empty_format,
            };
            transform_value(value, column, &context)
        })
        .collect()
}

fn transform_email(value: &str, context: &TransformContext<'_>) -> String {
    let Some(at_index) = value.rfind('@') else {
        return value.to_string();
    };
    let domain = &value[at_index..];
    let local_part = if context.deterministic {
        let prefix = deterministic_string(
            value,
            &format!("{}:prefix", context.seed),
            6,
            "abcdefghijklmnopqrstuvwxyz",
        );
        let suffix =
            deterministic_string(value, &format!("{}:suffix", context.seed), 3, "0123456789");
        format!("{prefix}{suffix}")
    } else {
        let mut rng = rand::thread_rng();
        format!("user{}", rng.gen_range(1..=999_999))
    };
    format!("{local_part}{domain}")
}

fn transform_uuid(value: &str, context: &TransformContext<'_>) -> String {
    let uuid = deterministic_uuid(value, context.seed);
    if value == value.to_uppercase() {
        uuid.to_uppercase()
    } else {
        uuid
    }
}

fn transform_timestamp(value: &str, context: &TransformContext<'_>) -> String {
    if value.len() < 10 {
        return value.to_string();
    }

    let Ok(date) = NaiveDate::parse_from_str(&value[..10], "%Y-%m-%d") else {
        return value.to_string();
    };

    let offset_days = if context.deterministic {
        deterministic_number(value, context.seed, -365, 365)
    } else {
        rand::thread_rng().gen_range(-365..=365)
    };

    let Some(offset_date) = date.checked_add_signed(Duration::days(offset_days)) else {
        return value.to_string();
    };

    format!("{}{}", offset_date.format("%Y-%m-%d"), &value[10..])
}

fn transform_numeric_id(value: &str, context: &TransformContext<'_>) -> String {
    let digit_count = value.len();
    if digit_count == 0 {
        return value.to_string();
    }

    let leading_zero_count = value
        .chars()
        .take_while(|character| *character == '0')
        .count();
    if leading_zero_count > 0 && leading_zero_count < digit_count {
        let generated = generate_numeric_id(digit_count - leading_zero_count, value, context);
        return format!("{}{}", "0".repeat(leading_zero_count), generated);
    }

    if leading_zero_count == digit_count {
        return value.to_string();
    }

    generate_numeric_id(digit_count, value, context)
}

fn generate_numeric_id(length: usize, value: &str, context: &TransformContext<'_>) -> String {
    if context.deterministic {
        let first_digit =
            deterministic_string(value, &format!("{}:first", context.seed), 1, "123456789");
        if length == 1 {
            return first_digit;
        }
        let rest_digits = deterministic_string(
            value,
            &format!("{}:rest", context.seed),
            length - 1,
            "0123456789",
        );
        format!("{first_digit}{rest_digits}")
    } else {
        let mut rng = rand::thread_rng();
        let first_digit = rng.gen_range(1..=9).to_string();
        let rest: String = (1..length)
            .map(|_| rng.gen_range(0..=9).to_string())
            .collect();
        format!("{first_digit}{rest}")
    }
}

fn transform_generic_string(value: &str, context: &TransformContext<'_>) -> String {
    let target_length = value.len();
    if target_length == 0 {
        return value.to_string();
    }

    let min_length = (target_length as f64 * 0.8).floor().max(1.0) as usize;
    let max_length = (target_length as f64 * 1.2).ceil() as usize;
    let charset = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-";

    let output_length = if context.deterministic {
        deterministic_number(
            value,
            &format!("{}:length", context.seed),
            min_length as i64,
            max_length as i64,
        ) as usize
    } else {
        rand::thread_rng().gen_range(min_length..=max_length)
    };

    if context.deterministic {
        deterministic_string(
            value,
            &format!("{}:content", context.seed),
            output_length,
            charset,
        )
    } else {
        let chars: Vec<char> = charset.chars().collect();
        let mut rng = rand::thread_rng();
        (0..output_length)
            .map(|_| chars[rng.gen_range(0..chars.len())])
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ColumnMetadata, Confidence, EmptyFormat, PiiRisk};

    fn column(detected_type: DataType) -> ColumnMetadata {
        ColumnMetadata {
            name: "value".to_string(),
            index: 0,
            detected_type,
            confidence: Confidence::High,
            pii_risk: PiiRisk::Medium,
            sample_values: vec![],
            empty_format: EmptyFormat::EmptyString,
            is_selected: true,
        }
    }

    fn context<'a>(seed: &'a str) -> TransformContext<'a> {
        TransformContext {
            column_name: "value",
            column_index: 0,
            row_index: 0,
            seed,
            deterministic: true,
            empty_format: EmptyFormat::EmptyString,
        }
    }

    #[test]
    fn email_preserves_domain() {
        let result = transform_value(
            "john.doe@example.com",
            &column(DataType::Email),
            &context("seed"),
        );
        assert!(result.ends_with("@example.com"));
        assert_ne!(result, "john.doe@example.com");
    }

    #[test]
    fn uuid_preserves_uppercase() {
        let result = transform_value(
            "550E8400-E29B-41D4-A716-446655440000",
            &column(DataType::Uuid),
            &context("seed"),
        );
        assert_eq!(result, result.to_uppercase());
    }

    #[test]
    fn timestamp_preserves_time() {
        let result = transform_value(
            "2024-06-15 10:30:45.123456",
            &column(DataType::Timestamp),
            &context("seed"),
        );
        assert!(result.ends_with(" 10:30:45.123456"));
        assert_ne!(result, "2024-06-15 10:30:45.123456");
    }

    #[test]
    fn numeric_id_preserves_leading_zeros() {
        let result = transform_value("001234", &column(DataType::NumericId), &context("seed"));
        assert!(result.starts_with("00"));
        assert_eq!(result.len(), 6);
    }
}
