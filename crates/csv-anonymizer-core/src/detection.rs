use crate::types::{Confidence, DataType, DetectionResult, EmptyFormat, PiiRisk};
use regex::Regex;
use std::collections::HashSet;
use std::sync::OnceLock;

pub fn is_empty_value(value: &str) -> bool {
    value.is_empty() || value.eq_ignore_ascii_case("null")
}

pub fn detect_column_type(values: &[String]) -> DetectionResult {
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

    for (data_type, pattern) in detection_priority() {
        let match_count = values
            .iter()
            .filter(|value| !is_empty_value(value) && pattern.is_match(value))
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

fn detection_priority() -> [(DataType, &'static Regex); 6] {
    [
        (DataType::Email, email_pattern()),
        (DataType::Uuid, uuid_pattern()),
        (DataType::Timestamp, timestamp_pattern()),
        (DataType::Phone, phone_pattern()),
        (DataType::NumericId, numeric_id_pattern()),
        (DataType::CountryCode, country_code_pattern()),
    ]
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

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn detects_email_with_high_confidence() {
        let result = detect_column_type(&strings(&[
            "user1@example.com",
            "user2@example.com",
            "user3@example.com",
        ]));
        assert_eq!(result.data_type, DataType::Email);
        assert_eq!(result.confidence, Confidence::High);
    }

    #[test]
    fn long_numeric_strings_follow_phone_priority() {
        let result = detect_column_type(&strings(&[
            "1234567890123",
            "9876543210987",
            "1111222233334",
        ]));
        assert_eq!(result.data_type, DataType::Phone);
    }

    #[test]
    fn detects_enum_after_patterns() {
        let result = detect_column_type(&strings(&[
            "active", "inactive", "pending", "active", "inactive", "pending", "active", "inactive",
            "pending", "active", "inactive",
        ]));
        assert_eq!(result.data_type, DataType::Enum);
    }

    #[test]
    fn detects_mixed_empty_format() {
        let result = detect_empty_format(&strings(&["", "null", "value"]));
        assert_eq!(result, EmptyFormat::Mixed);
    }
}
