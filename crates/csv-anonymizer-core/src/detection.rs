use crate::types::{Confidence, DataType, EmptyFormat, PiiRisk};

mod candidate;
mod header;
mod header_rules;
mod locale;
mod national_id;
mod privacy;
mod scoring;
mod spans;
mod validators;
mod value;

use header_rules::{HeaderDetectionRule, first_header_detection};
use scoring::{attach_single_trace, detection_result, trace_item};
use value::{
    detect_enum_type, detect_iban_value_type, detect_numeric_value_type, detect_priority_pattern,
    detect_vat_value_type,
};

pub use locale::{LocaleContext, infer_locale_context};
pub use privacy::{ColumnPrivacyAnalysis, analyze_column_privacy, max_pii_risk};
pub use spans::{PrivacySpan, collect_privacy_spans};

pub(in crate::detection) use header_rules::{
    TaxIdHeaderContext, has_dutch_btw_context, is_contextual_unformatted_us_tax_id,
    tax_id_header_context,
};
pub(in crate::detection) use value::is_timestamp;

#[cfg(test)]
use validators::{
    is_dutch_btw_tax_number, is_email, is_payment_card_number, is_tax_id, is_url, is_vat_id,
};

#[cfg(test)]
pub(crate) fn validators_test_hook_is_valid_phone_in_context(
    value: &str,
    locale: &LocaleContext,
) -> bool {
    validators::is_valid_phone_number_in_context(value, locale)
}

pub fn is_empty_value(value: &str) -> bool {
    value.is_empty() || value.eq_ignore_ascii_case("null")
}

pub(super) fn utf16_index_for_byte(value: &str, byte_index: usize) -> usize {
    match value.get(..byte_index) {
        Some(prefix) => utf16_len(prefix),
        None => value
            .char_indices()
            .take_while(|(index, _)| *index < byte_index)
            .map(|(_, character)| character.len_utf16())
            .sum(),
    }
}

pub(super) fn utf16_len(value: &str) -> usize {
    value.encode_utf16().count()
}

pub fn detect_column_type(values: &[String]) -> crate::types::DetectionResult {
    detect_column_type_in_context("", values, &LocaleContext::default())
}

pub fn detect_column_type_with_name(
    column_name: &str,
    values: &[String],
) -> crate::types::DetectionResult {
    detect_column_type_in_context(column_name, values, &LocaleContext::default())
}

pub fn detect_column_type_in_context(
    column_name: &str,
    values: &[String],
    locale: &LocaleContext,
) -> crate::types::DetectionResult {
    let non_empty_values: Vec<&String> = values
        .iter()
        .filter(|value| !is_empty_value(value))
        .collect();
    let total_non_empty = non_empty_values.len();

    if total_non_empty == 0 {
        return detection_result(
            DataType::Unknown,
            Confidence::Low,
            0,
            values.len(),
            total_non_empty,
            "No non-empty sample values were available for detection.",
            Vec::new(),
        );
    }

    if let Some(result) = first_header_detection(
        column_name,
        &non_empty_values,
        values.len(),
        total_non_empty,
        &header_rules::early_header_detection_rules(),
    ) {
        return result;
    }

    if let Some(result) = detect_vat_value_type(values, total_non_empty) {
        return result;
    }

    if let Some(result) = detect_iban_value_type(values, total_non_empty) {
        return result;
    }

    let candidates = match detect_priority_pattern(values, total_non_empty, locale) {
        Ok(result) => return result,
        Err(candidates) => candidates,
    };

    if let Some(result) = first_header_detection(
        column_name,
        &non_empty_values,
        values.len(),
        total_non_empty,
        &[HeaderDetectionRule {
            detect: header_rules::detect_header_numeric_id,
            selected_reason: "Header terms and integer sample shape matched numeric ID detection.",
            trace_reason: "header numeric ID rule",
        }],
    ) {
        return result;
    }

    if let Some(result) = detect_numeric_value_type(values, total_non_empty) {
        return attach_single_trace(
            result,
            total_non_empty,
            "Sample values matched numeric value detection after identifier rules were rejected.",
            "numeric value rule",
        );
    }

    if let Some(result) = first_header_detection(
        column_name,
        &non_empty_values,
        values.len(),
        total_non_empty,
        &[HeaderDetectionRule {
            detect: header_rules::detect_name_type,
            selected_reason: "Header terms and sample shape matched name detection.",
            trace_reason: "header name rule",
        }],
    ) {
        return result;
    }

    if detect_enum_type(&non_empty_values) {
        return detection_result(
            DataType::Enum,
            Confidence::High,
            non_empty_values.len(),
            values.len(),
            total_non_empty,
            "Sample values formed a repeated finite set.",
            vec![trace_item(
                DataType::Enum,
                "finite repeated values",
                non_empty_values.len(),
                total_non_empty,
                Confidence::High,
                true,
            )],
        );
    }

    detection_result(
        DataType::String,
        Confidence::Low,
        non_empty_values.len(),
        values.len(),
        total_non_empty,
        "No sensitive pattern, header, numeric, name, or enum rule passed the threshold.",
        candidates,
    )
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

pub fn detect_empty_format(values: &[String]) -> crate::types::EmptyFormat {
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

#[cfg(test)]
mod tests;
