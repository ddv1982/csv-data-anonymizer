use crate::types::{Confidence, DataType, DetectionResult, EmptyFormat, PiiRisk};
use regex::Regex;
use std::collections::HashSet;
use std::sync::OnceLock;

pub fn is_empty_value(value: &str) -> bool {
    value.is_empty() || value.eq_ignore_ascii_case("null")
}

pub fn detect_column_type(values: &[String]) -> DetectionResult {
    detect_column_type_with_name("", values)
}

pub fn detect_column_type_with_name(column_name: &str, values: &[String]) -> DetectionResult {
    let non_empty_values: Vec<&String> = values
        .iter()
        .filter(|value| !is_empty_value(value))
        .collect();
    let total_non_empty = non_empty_values.len();

    if total_non_empty == 0 {
        return DetectionResult {
            data_type: DataType::Unknown,
            confidence: Confidence::Low,
            sample_matches: 0,
            total_samples: values.len(),
        };
    }

    for (data_type, matches) in detection_priority() {
        let match_count = values
            .iter()
            .filter(|value| !is_empty_value(value) && matches(value))
            .count();
        let confidence = calculate_confidence(match_count, total_non_empty);

        if confidence != Confidence::Low {
            return DetectionResult {
                data_type,
                confidence,
                sample_matches: match_count,
                total_samples: values.len(),
            };
        }
    }

    if let Some(result) = detect_name_type(column_name, &non_empty_values, values.len()) {
        return result;
    }

    if detect_enum_type(&non_empty_values) {
        return DetectionResult {
            data_type: DataType::Enum,
            confidence: Confidence::High,
            sample_matches: non_empty_values.len(),
            total_samples: values.len(),
        };
    }

    DetectionResult {
        data_type: DataType::String,
        confidence: Confidence::Low,
        sample_matches: non_empty_values.len(),
        total_samples: values.len(),
    }
}

pub fn classify_pii_risk(data_type: DataType) -> PiiRisk {
    match data_type {
        DataType::Email | DataType::Phone | DataType::FullName => PiiRisk::High,
        DataType::FirstName | DataType::LastName | DataType::Uuid | DataType::NumericId => {
            PiiRisk::Medium
        }
        DataType::Timestamp
        | DataType::CountryCode
        | DataType::Enum
        | DataType::String
        | DataType::Unknown => PiiRisk::Low,
    }
}

pub fn detect_empty_format(values: &[String]) -> EmptyFormat {
    let mut has_empty_string = false;
    let mut has_null_string = false;

    for value in values {
        if value.is_empty() {
            has_empty_string = true;
        } else if value.eq_ignore_ascii_case("null") {
            has_null_string = true;
        }

        if has_empty_string && has_null_string {
            return EmptyFormat::Mixed;
        }
    }

    if has_null_string {
        EmptyFormat::Null
    } else {
        EmptyFormat::EmptyString
    }
}

fn calculate_confidence(match_count: usize, total_non_empty: usize) -> Confidence {
    if total_non_empty == 0 {
        return Confidence::Low;
    }

    let percentage = match_count as f64 / total_non_empty as f64;
    if percentage >= 0.8 {
        Confidence::High
    } else if percentage >= 0.5 {
        Confidence::Medium
    } else {
        Confidence::Low
    }
}

fn detect_enum_type(non_empty_values: &[&String]) -> bool {
    if non_empty_values.len() <= 10 {
        return false;
    }
    let unique_values: HashSet<&str> = non_empty_values
        .iter()
        .map(|value| value.as_str())
        .collect();
    unique_values.len() <= 20
}

type DetectionPredicate = fn(&str) -> bool;

fn detection_priority() -> [(DataType, DetectionPredicate); 6] {
    [
        (DataType::Email, is_email),
        (DataType::Uuid, is_uuid),
        (DataType::Timestamp, is_timestamp),
        (DataType::Phone, is_phone),
        (DataType::NumericId, is_numeric_id),
        (DataType::CountryCode, is_country_code),
    ]
}

fn is_email(value: &str) -> bool {
    email_pattern().is_match(value)
}

fn is_uuid(value: &str) -> bool {
    uuid_pattern().is_match(value)
}

fn is_timestamp(value: &str) -> bool {
    timestamp_pattern().is_match(value)
}

fn is_phone(value: &str) -> bool {
    let trimmed = value.trim();
    if !phone_pattern().is_match(trimmed) {
        return false;
    }

    let digit_count = trimmed
        .chars()
        .filter(|character| character.is_ascii_digit())
        .count();
    if !(10..=15).contains(&digit_count) {
        return false;
    }

    trimmed.starts_with('+') || trimmed.chars().any(is_phone_separator)
}

fn is_phone_separator(character: char) -> bool {
    matches!(character, ' ' | '-' | '(' | ')' | '.')
}

fn is_numeric_id(value: &str) -> bool {
    numeric_id_pattern().is_match(value)
}

fn is_country_code(value: &str) -> bool {
    country_code_pattern().is_match(value)
}

fn email_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap())
}

fn uuid_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$")
            .unwrap()
    })
}

fn timestamp_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN
        .get_or_init(|| Regex::new(r"^\d{4}-\d{2}-\d{2}([T ]\d{2}:\d{2}:\d{2}(\.\d+)?)?$").unwrap())
}

fn phone_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^\+?[\d\s\-().]{10,}$").unwrap())
}

fn numeric_id_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^\d{4,}$").unwrap())
}

fn country_code_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^[A-Z]{2}$").unwrap())
}

fn detect_name_type(
    column_name: &str,
    non_empty_values: &[&String],
    total_samples: usize,
) -> Option<DetectionResult> {
    let data_type = infer_name_type_from_header(column_name)?;
    let match_count = non_empty_values
        .iter()
        .filter(|value| match data_type {
            DataType::FirstName => is_plausible_name_part(value, 2),
            DataType::LastName => is_plausible_name_part(value, 4),
            DataType::FullName => is_plausible_full_name(value),
            _ => false,
        })
        .count();
    let confidence = calculate_confidence(match_count, non_empty_values.len());

    if confidence == Confidence::Low {
        return None;
    }

    Some(DetectionResult {
        data_type,
        confidence,
        sample_matches: match_count,
        total_samples,
    })
}

fn infer_name_type_from_header(column_name: &str) -> Option<DataType> {
    let compact: String = column_name
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    let tokens: HashSet<String> = column_name
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect();

    if matches!(compact.as_str(), "firstname" | "givenname" | "forename")
        || tokens.contains("firstname")
        || tokens.contains("forename")
        || tokens.contains("given")
        || tokens.contains("first") && tokens.contains("name")
    {
        return Some(DataType::FirstName);
    }

    if matches!(compact.as_str(), "lastname" | "surname" | "familyname")
        || tokens.contains("lastname")
        || tokens.contains("surname")
        || tokens.contains("family") && tokens.contains("name")
        || tokens.contains("last") && tokens.contains("name")
    {
        return Some(DataType::LastName);
    }

    if matches!(
        compact.as_str(),
        "name"
            | "fullname"
            | "displayname"
            | "legalname"
            | "personname"
            | "contactname"
            | "customername"
            | "clientname"
    ) || tokens.contains("fullname")
        || tokens.contains("display") && tokens.contains("name")
        || tokens.contains("legal") && tokens.contains("name")
        || tokens.contains("person") && tokens.contains("name")
        || tokens.contains("contact") && tokens.contains("name")
        || tokens.contains("customer") && tokens.contains("name")
        || tokens.contains("client") && tokens.contains("name")
        || tokens.contains("full") && tokens.contains("name")
    {
        return Some(DataType::FullName);
    }

    None
}

fn is_plausible_name_part(value: &str, max_tokens: usize) -> bool {
    let trimmed = value.trim();
    if !(2..=80).contains(&trimmed.len()) {
        return false;
    }

    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    !tokens.is_empty()
        && tokens.len() <= max_tokens
        && tokens.iter().all(|token| is_plausible_name_token(token))
}

fn is_plausible_full_name(value: &str) -> bool {
    let trimmed = value.trim();
    if !(5..=120).contains(&trimmed.len()) {
        return false;
    }

    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    (2..=6).contains(&tokens.len()) && tokens.iter().all(|token| is_plausible_name_token(token))
}

fn is_plausible_name_token(token: &str) -> bool {
    let mut chars = token.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_alphabetic()
        && chars.all(|character| character.is_alphabetic() || matches!(character, '\'' | '-' | '.'))
}

#[cfg(test)]
mod tests;
