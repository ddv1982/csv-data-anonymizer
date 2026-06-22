use crate::detection::is_empty_value;
use crate::types::DataType;

pub(super) const MAX_GENERALIZATION_LEVEL: u8 = 4;

pub(super) fn generalize_value(value: &str, data_type: DataType, level: u8) -> String {
    if level == 0 || is_empty_value(value) {
        return value.to_string();
    }
    if level >= MAX_GENERALIZATION_LEVEL {
        return "*".to_string();
    }

    match data_type {
        DataType::NumericId
        | DataType::NumericValue
        | DataType::Currency
        | DataType::Percentage => generalize_number(value, level),
        DataType::Timestamp => generalize_timestamp(value, level),
        DataType::PostalCode => generalize_prefix(value, level, 2),
        DataType::IpAddress => generalize_ip(value, level),
        DataType::CountryCode | DataType::Boolean | DataType::Enum => {
            if level >= 2 {
                "*".to_string()
            } else {
                generalize_prefix(value, level, 1)
            }
        }
        _ => generalize_prefix(value, level, 2),
    }
}

fn generalize_number(value: &str, level: u8) -> String {
    let Ok(number) = value.parse::<f64>() else {
        return generalize_prefix(value, level, 2);
    };
    if level >= 3 {
        return "*".to_string();
    }
    let bucket_size = 10_f64.powi(level as i32);
    let lower = (number / bucket_size).floor() * bucket_size;
    let upper = lower + bucket_size - 1.0;
    format!("[{}-{}]", lower.trunc(), upper.trunc())
}

fn generalize_timestamp(value: &str, level: u8) -> String {
    let date = value.get(0..10).unwrap_or(value);
    if level == 1 && date.len() >= 7 {
        return date[..7].to_string();
    }
    if level == 2 && date.len() >= 4 {
        return date[..4].to_string();
    }
    "*".to_string()
}

fn generalize_ip(value: &str, level: u8) -> String {
    let parts = value.split('.').collect::<Vec<_>>();
    if parts.len() != 4 {
        return generalize_prefix(value, level, 2);
    }
    match level {
        1 => format!("{}.{}.{}.*", parts[0], parts[1], parts[2]),
        2 => format!("{}.{}.*.*", parts[0], parts[1]),
        3 => format!("{}.*.*.*", parts[0]),
        _ => "*".to_string(),
    }
}

fn generalize_prefix(value: &str, level: u8, chars_per_level: usize) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    let keep = chars.len().saturating_sub(level as usize * chars_per_level);
    if keep == 0 {
        return "*".to_string();
    }
    let prefix = chars.iter().take(keep).collect::<String>();
    format!("{prefix}*")
}
