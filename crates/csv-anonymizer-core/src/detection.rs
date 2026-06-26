use crate::types::{Confidence, DataType, DetectionResult, EmptyFormat, PiiRisk};
use regex::Regex;
use std::collections::HashSet;
use std::sync::OnceLock;

struct HeaderTerms {
    compact: String,
    tokens: HashSet<String>,
}

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

    if let Some(result) = detect_header_postal_code(column_name, &non_empty_values, values.len()) {
        return result;
    }

    if let Some(result) = detect_header_address(column_name, &non_empty_values, values.len()) {
        return result;
    }

    if let Some(result) = detect_header_tax_id(column_name, &non_empty_values, values.len()) {
        return result;
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

    if let Some(result) = detect_header_numeric_id(column_name, &non_empty_values, values.len()) {
        return result;
    }

    if let Some(result) = detect_numeric_value_type(values, total_non_empty) {
        return result;
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
        DataType::Email
        | DataType::Phone
        | DataType::FullName
        | DataType::Address
        | DataType::TaxId => PiiRisk::High,
        DataType::FirstName
        | DataType::LastName
        | DataType::Uuid
        | DataType::NumericId
        | DataType::PostalCode
        | DataType::IpAddress
        | DataType::Url
        | DataType::MacAddress => PiiRisk::Medium,
        DataType::Timestamp
        | DataType::NumericValue
        | DataType::Boolean
        | DataType::Currency
        | DataType::Percentage
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

fn detection_priority() -> [(DataType, DetectionPredicate); 13] {
    [
        (DataType::Email, is_email),
        (DataType::Uuid, is_uuid),
        (DataType::Timestamp, is_timestamp),
        (DataType::Phone, is_phone),
        (DataType::IpAddress, is_ip_address),
        (DataType::MacAddress, is_mac_address),
        (DataType::Url, is_url),
        (DataType::TaxId, is_tax_id),
        (DataType::Boolean, is_boolean),
        (DataType::Currency, is_currency),
        (DataType::Percentage, is_percentage),
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

fn is_unsigned_integer(value: &str) -> bool {
    unsigned_integer_pattern().is_match(value)
}

fn is_numeric_value(value: &str) -> bool {
    numeric_value_pattern().is_match(value)
}

fn is_ip_address(value: &str) -> bool {
    let parts: Vec<&str> = value.split('.').collect();
    parts.len() == 4
        && parts.iter().all(|part| {
            !part.is_empty()
                && part.len() <= 3
                && part.chars().all(|character| character.is_ascii_digit())
                && part.parse::<u8>().is_ok()
        })
}

fn is_mac_address(value: &str) -> bool {
    mac_address_pattern().is_match(value)
}

fn is_url(value: &str) -> bool {
    url_pattern().is_match(value)
}

fn is_tax_id(value: &str) -> bool {
    tax_id_pattern().is_match(value)
}

fn is_boolean(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "true" | "false" | "yes" | "no"
    )
}

fn is_currency(value: &str) -> bool {
    currency_pattern().is_match(value)
}

fn is_percentage(value: &str) -> bool {
    percentage_pattern().is_match(value)
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

fn unsigned_integer_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^\d+$").unwrap())
}

fn numeric_value_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^[+-]?(?:\d+|\d+\.\d+|\.\d+)$").unwrap())
}

fn mac_address_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^(?:[0-9A-Fa-f]{2}[:-]){5}[0-9A-Fa-f]{2}$").unwrap())
}

fn url_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^(https?://|www\.)[^\s/$.?#].[^\s]*$").unwrap())
}

fn tax_id_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^(\d{3}-\d{2}-\d{4}|\d{2}-\d{7})$").unwrap())
}

fn currency_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^\$\s?(?:\d+|\d{1,3}(?:,\d{3})+)(?:\.\d{2})?$").unwrap())
}

fn percentage_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^[+-]?\d+(?:\.\d+)?%$").unwrap())
}

fn country_code_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^[A-Z]{2}$").unwrap())
}

fn detect_header_numeric_id(
    column_name: &str,
    non_empty_values: &[&String],
    total_samples: usize,
) -> Option<DetectionResult> {
    if !infer_numeric_id_from_header(column_name) {
        return None;
    }

    let match_count = non_empty_values
        .iter()
        .filter(|value| is_unsigned_integer(value))
        .count();
    let confidence = calculate_confidence(match_count, non_empty_values.len());

    if confidence == Confidence::Low {
        return None;
    }

    Some(DetectionResult {
        data_type: DataType::NumericId,
        confidence,
        sample_matches: match_count,
        total_samples,
    })
}

fn detect_numeric_value_type(values: &[String], total_non_empty: usize) -> Option<DetectionResult> {
    let match_count = values
        .iter()
        .filter(|value| !is_empty_value(value) && is_numeric_value(value))
        .count();
    let confidence = calculate_confidence(match_count, total_non_empty);

    if confidence == Confidence::Low {
        return None;
    }

    Some(DetectionResult {
        data_type: DataType::NumericValue,
        confidence,
        sample_matches: match_count,
        total_samples: values.len(),
    })
}

fn infer_numeric_id_from_header(column_name: &str) -> bool {
    let terms = header_terms(column_name);

    matches!(
        terms.compact.as_str(),
        "id" | "userid"
            | "usernumber"
            | "customerid"
            | "customernumber"
            | "clientid"
            | "clientnumber"
            | "accountid"
            | "accountnumber"
            | "orderid"
            | "ordernumber"
            | "code"
    ) || terms.tokens.contains("id")
        || terms.tokens.contains("identifier")
        || terms.tokens.contains("code")
        || terms.tokens.contains("account") && terms.tokens.contains("number")
        || terms.tokens.contains("customer") && terms.tokens.contains("number")
        || terms.tokens.contains("client") && terms.tokens.contains("number")
        || terms.tokens.contains("order") && terms.tokens.contains("number")
}

fn detect_header_postal_code(
    column_name: &str,
    non_empty_values: &[&String],
    total_samples: usize,
) -> Option<DetectionResult> {
    if !infer_postal_code_from_header(column_name) {
        return None;
    }

    let match_count = non_empty_values
        .iter()
        .filter(|value| is_postal_code(value))
        .count();
    let confidence = calculate_confidence(match_count, non_empty_values.len());
    if confidence == Confidence::Low {
        return None;
    }

    Some(DetectionResult {
        data_type: DataType::PostalCode,
        confidence,
        sample_matches: match_count,
        total_samples,
    })
}

fn detect_header_address(
    column_name: &str,
    non_empty_values: &[&String],
    total_samples: usize,
) -> Option<DetectionResult> {
    if !infer_address_from_header(column_name) {
        return None;
    }

    let match_count = non_empty_values
        .iter()
        .filter(|value| is_plausible_address(value))
        .count();
    let confidence = calculate_confidence(match_count, non_empty_values.len());
    if confidence == Confidence::Low {
        return None;
    }

    Some(DetectionResult {
        data_type: DataType::Address,
        confidence,
        sample_matches: match_count,
        total_samples,
    })
}

fn detect_header_tax_id(
    column_name: &str,
    non_empty_values: &[&String],
    total_samples: usize,
) -> Option<DetectionResult> {
    if !infer_tax_id_from_header(column_name) {
        return None;
    }

    let match_count = non_empty_values
        .iter()
        .filter(|value| is_tax_id(value) || is_unformatted_tax_id(value))
        .count();
    let confidence = calculate_confidence(match_count, non_empty_values.len());
    if confidence == Confidence::Low {
        return None;
    }

    Some(DetectionResult {
        data_type: DataType::TaxId,
        confidence,
        sample_matches: match_count,
        total_samples,
    })
}

fn infer_postal_code_from_header(column_name: &str) -> bool {
    let compact = compact_header(column_name);
    let tokens = header_tokens(column_name);

    matches!(
        compact.as_str(),
        "zip" | "zipcode" | "postalcode" | "postcode"
    ) || tokens.contains("zip")
        || tokens.contains("postal") && tokens.contains("code")
        || tokens.contains("post") && tokens.contains("code")
}

fn infer_address_from_header(column_name: &str) -> bool {
    let compact = compact_header(column_name);
    let tokens = header_tokens(column_name);

    matches!(
        compact.as_str(),
        "address" | "streetaddress" | "mailingaddress"
    ) || tokens.contains("address")
        || tokens.contains("street")
}

fn infer_tax_id_from_header(column_name: &str) -> bool {
    let compact = compact_header(column_name);
    let tokens = header_tokens(column_name);

    matches!(compact.as_str(), "ssn" | "taxid" | "taxnumber" | "ein")
        || tokens.contains("ssn")
        || tokens.contains("ein")
        || tokens.contains("tax") && (tokens.contains("id") || tokens.contains("number"))
}

fn compact_header(column_name: &str) -> String {
    column_name
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn header_tokens(column_name: &str) -> HashSet<String> {
    column_name
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

fn is_postal_code(value: &str) -> bool {
    let trimmed = value.trim();
    (3..=12).contains(&trimmed.len())
        && trimmed.chars().any(|character| character.is_ascii_digit())
        && trimmed
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, ' ' | '-'))
}

fn is_plausible_address(value: &str) -> bool {
    let trimmed = value.trim().to_ascii_lowercase();
    trimmed.chars().any(|character| character.is_ascii_digit())
        && [
            " st",
            " street",
            " ave",
            " avenue",
            " rd",
            " road",
            " blvd",
            " boulevard",
            " dr",
            " drive",
            " ln",
            " lane",
            " way",
            " court",
            " ct",
        ]
        .iter()
        .any(|suffix| trimmed.contains(suffix))
}

fn is_unformatted_tax_id(value: &str) -> bool {
    value.len() == 9 && value.chars().all(|character| character.is_ascii_digit())
}

fn detect_name_type(
    column_name: &str,
    non_empty_values: &[&String],
    total_samples: usize,
) -> Option<DetectionResult> {
    let data_type = infer_name_type_from_header(column_name)?;
    let mut match_count = non_empty_values
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
        if data_type == DataType::FullName && infer_generic_name_header(column_name) {
            match_count = non_empty_values
                .iter()
                .filter(|value| is_plausible_name_part(value, 1))
                .count();
            let confidence = calculate_confidence(match_count, non_empty_values.len());

            if confidence != Confidence::Low {
                return Some(DetectionResult {
                    data_type: DataType::FirstName,
                    confidence,
                    sample_matches: match_count,
                    total_samples,
                });
            }
        }

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
    let terms = header_terms(column_name);

    if matches!(
        terms.compact.as_str(),
        "firstname" | "givenname" | "forename"
    ) || terms.tokens.contains("firstname")
        || terms.tokens.contains("forename")
        || terms.tokens.contains("given")
        || terms.tokens.contains("first") && terms.tokens.contains("name")
    {
        return Some(DataType::FirstName);
    }

    if matches!(
        terms.compact.as_str(),
        "lastname" | "surname" | "familyname"
    ) || terms.tokens.contains("lastname")
        || terms.tokens.contains("surname")
        || terms.tokens.contains("family") && terms.tokens.contains("name")
        || terms.tokens.contains("last") && terms.tokens.contains("name")
    {
        return Some(DataType::LastName);
    }

    if matches!(
        terms.compact.as_str(),
        "name"
            | "fullname"
            | "displayname"
            | "legalname"
            | "personname"
            | "contactname"
            | "customername"
            | "clientname"
    ) || terms.tokens.contains("fullname")
        || terms.tokens.contains("display") && terms.tokens.contains("name")
        || terms.tokens.contains("legal") && terms.tokens.contains("name")
        || terms.tokens.contains("person") && terms.tokens.contains("name")
        || terms.tokens.contains("contact") && terms.tokens.contains("name")
        || terms.tokens.contains("customer") && terms.tokens.contains("name")
        || terms.tokens.contains("client") && terms.tokens.contains("name")
        || terms.tokens.contains("full") && terms.tokens.contains("name")
    {
        return Some(DataType::FullName);
    }

    None
}

fn header_terms(column_name: &str) -> HeaderTerms {
    HeaderTerms {
        compact: column_name
            .chars()
            .filter(|character| character.is_ascii_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect(),
        tokens: column_name
            .split(|character: char| !character.is_ascii_alphanumeric())
            .filter(|token| !token.is_empty())
            .map(|token| token.to_ascii_lowercase())
            .collect(),
    }
}

fn infer_generic_name_header(column_name: &str) -> bool {
    compact_header(column_name) == "name"
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
