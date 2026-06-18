use crate::detection::is_empty_value;
use crate::hash::{deterministic_number, deterministic_string, deterministic_uuid};
use crate::types::{AnonymizationStrategy, ColumnMetadata, DataType, TransformContext};
use chrono::{Duration, NaiveDate};
use rand::Rng;

const FIRST_NAMES: &[&str] = &[
    "Alex", "Bailey", "Casey", "Dana", "Elliot", "Finley", "Jordan", "Morgan", "Quinn", "Riley",
    "Taylor", "Avery",
];
const LAST_NAMES: &[&str] = &[
    "Bennett", "Carter", "Hayes", "Morgan", "Parker", "Reed", "Sullivan", "Turner", "Walker",
    "Young", "Brooks", "Coleman",
];

pub fn transform_value(
    value: &str,
    column: &ColumnMetadata,
    context: &TransformContext<'_>,
) -> String {
    if is_empty_value(value) {
        return value.to_string();
    }

    match column.strategy {
        AnonymizationStrategy::PassThrough => return value.to_string(),
        AnonymizationStrategy::Mask => return mask_value(value),
        AnonymizationStrategy::Auto | AnonymizationStrategy::Pseudonymize => {}
    }

    match column.detected_type {
        DataType::Email => transform_email(value, context),
        DataType::Uuid => transform_uuid(value, context),
        DataType::Timestamp => transform_timestamp(value, context),
        DataType::NumericId => transform_numeric_id(value, context),
        DataType::NumericValue => transform_numeric_value(value, context),
        DataType::Phone => transform_phone(value, context),
        DataType::FirstName => transform_first_name(value, context),
        DataType::LastName => transform_last_name(value, context),
        DataType::FullName => transform_full_name(value, context),
        DataType::PostalCode
        | DataType::Address
        | DataType::IpAddress
        | DataType::Url
        | DataType::MacAddress
        | DataType::TaxId
        | DataType::String
        | DataType::Unknown => transform_generic_string(value, context),
        DataType::Boolean | DataType::Currency | DataType::Percentage => value.to_string(),
        DataType::CountryCode | DataType::Enum => value.to_string(),
    }
}

fn mask_value(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_whitespace() {
                character
            } else {
                '*'
            }
        })
        .collect()
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
    let uuid = if context.deterministic {
        deterministic_uuid(value, context.seed)
    } else {
        random_uuid_v4()
    };
    if value == value.to_uppercase() {
        uuid.to_uppercase()
    } else {
        uuid
    }
}

fn random_uuid_v4() -> String {
    let mut bytes = [0_u8; 16];
    rand::thread_rng().fill(&mut bytes);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
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
        return generate_zero_width_numeric_id(digit_count, value, context);
    }

    generate_numeric_id(digit_count, value, context)
}

fn generate_zero_width_numeric_id(
    length: usize,
    value: &str,
    context: &TransformContext<'_>,
) -> String {
    if context.deterministic {
        deterministic_string(
            value,
            &format!("{}:zero", context.seed),
            length,
            "0123456789",
        )
    } else {
        let mut rng = rand::thread_rng();
        (0..length)
            .map(|_| rng.gen_range(0..=9).to_string())
            .collect()
    }
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

fn transform_numeric_value(value: &str, context: &TransformContext<'_>) -> String {
    let (sign, unsigned) = match value.as_bytes().first() {
        Some(b'+') | Some(b'-') => (&value[..1], &value[1..]),
        _ => ("", value),
    };

    let Some((integer_part, fractional_part)) = unsigned.split_once('.') else {
        return format!(
            "{sign}{}",
            generate_numeric_component(unsigned, value, context)
        );
    };

    let integer = generate_numeric_component(integer_part, value, context);
    let fraction = generate_fractional_component(fractional_part, value, context);

    format!("{sign}{integer}.{fraction}")
}

fn generate_numeric_component(
    component: &str,
    value: &str,
    context: &TransformContext<'_>,
) -> String {
    if component.is_empty() {
        return String::new();
    }

    let leading_zero_count = component
        .chars()
        .take_while(|character| *character == '0')
        .count();
    if leading_zero_count == component.len() {
        return component.to_string();
    }

    let generated = generate_numeric_id(component.len() - leading_zero_count, value, context);
    format!("{}{}", "0".repeat(leading_zero_count), generated)
}

fn generate_fractional_component(
    component: &str,
    value: &str,
    context: &TransformContext<'_>,
) -> String {
    if component.is_empty() {
        return String::new();
    }

    if context.deterministic {
        deterministic_string(
            value,
            &format!("{}:fraction", context.seed),
            component.len(),
            "0123456789",
        )
    } else {
        let mut rng = rand::thread_rng();
        (0..component.len())
            .map(|_| rng.gen_range(0..=9).to_string())
            .collect()
    }
}

fn transform_phone(value: &str, context: &TransformContext<'_>) -> String {
    let mut digit_index = 0;
    value
        .chars()
        .map(|character| {
            if !character.is_ascii_digit() {
                return character.to_string();
            }

            let seed = format!("{}:phone:{digit_index}", context.seed);
            digit_index += 1;
            if context.deterministic {
                deterministic_string(value, &seed, 1, "0123456789")
            } else {
                rand::thread_rng().gen_range(0..=9).to_string()
            }
        })
        .collect()
}

fn transform_first_name(value: &str, context: &TransformContext<'_>) -> String {
    choose_name(value, context, FIRST_NAMES).to_string()
}

fn transform_last_name(value: &str, context: &TransformContext<'_>) -> String {
    choose_name(value, context, LAST_NAMES).to_string()
}

fn transform_full_name(value: &str, context: &TransformContext<'_>) -> String {
    let token_count = value.split_whitespace().count();
    if token_count <= 1 {
        return transform_first_name(value, context);
    }

    let first = transform_first_name(value, context);
    let last = transform_last_name(value, context);
    if token_count == 2 {
        return format!("{first} {last}");
    }

    let middle_count = token_count.saturating_sub(2);
    let middle = (0..middle_count)
        .map(|index| {
            let seed = format!("{}:middle:{index}", context.seed);
            choose_name_with_seed(value, &seed, context.deterministic, FIRST_NAMES).to_string()
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!("{first} {middle} {last}")
}

fn choose_name<'a>(value: &str, context: &TransformContext<'_>, names: &'a [&str]) -> &'a str {
    choose_name_with_seed(value, context.seed, context.deterministic, names)
}

fn choose_name_with_seed<'a>(
    value: &str,
    seed: &str,
    deterministic: bool,
    names: &'a [&str],
) -> &'a str {
    let index = if deterministic {
        deterministic_number(value, seed, 0, names.len() as i64 - 1) as usize
    } else {
        rand::thread_rng().gen_range(0..names.len())
    };
    names[index]
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
mod tests;
