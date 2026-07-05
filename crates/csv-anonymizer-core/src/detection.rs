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
use national_id::national_id_countries;
use scoring::{attach_single_trace, detection_result, raise_one_tier, trace_item};
use value::{
    PatternOutcome, detect_enum_type, detect_iban_value_type, detect_numeric_value_type,
    detect_priority_pattern, detect_vat_value_type,
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

const DETECTION_SAMPLE_CAP: usize = 200;

fn sample_evenly(values: &[String], cap: usize) -> Vec<&String> {
    let non_empty: Vec<&String> = values
        .iter()
        .filter(|value| !is_empty_value(value))
        .collect();
    if non_empty.len() <= cap {
        return non_empty;
    }
    (0..cap)
        .map(|slot| non_empty[slot * non_empty.len() / cap])
        .collect()
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
    let sampled: Vec<&String> = sample_evenly(values, DETECTION_SAMPLE_CAP);
    let total_non_empty = sampled.len();

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

    // 1. Value battery first. Checksum/validator-backed selections (VAT, IBAN,
    //    and validator-evidence priority patterns such as national IDs) are
    //    final: the column *is* that sensitive type. The header may only
    //    agree-and-boost (contributing its richer taxonomy trace and raising
    //    confidence one tier); it can never suppress or replace the selection.
    let early_header_rules = header_rules::early_header_detection_rules();

    if let Some(result) = detect_vat_value_type(&sampled, values.len(), total_non_empty) {
        return finalize_validator(
            column_name,
            result,
            &sampled,
            values.len(),
            total_non_empty,
            &early_header_rules,
        );
    }

    if let Some(result) = detect_iban_value_type(&sampled, values.len(), total_non_empty) {
        return finalize_validator(
            column_name,
            result,
            &sampled,
            values.len(),
            total_non_empty,
            &early_header_rules,
        );
    }

    let pattern = detect_priority_pattern(&sampled, values.len(), total_non_empty, locale);
    if pattern.selected_is_validator() {
        let mut result = pattern
            .result()
            .expect("validator selection yields a result");
        label_national_id_country(&mut result, &pattern, &sampled);
        return finalize_validator(
            column_name,
            result,
            &sampled,
            values.len(),
            total_non_empty,
            &early_header_rules,
        );
    }

    // 2. No validator claimed the column: the header rules run exactly as
    //    before (early rules, numeric-id, name).
    if let Some(result) = first_header_detection(
        column_name,
        &sampled,
        values.len(),
        total_non_empty,
        &early_header_rules,
    ) {
        return result;
    }

    // 3. The deferred non-validator pattern selection keeps its original slot,
    //    after the early header rules and before the numeric-id / numeric /
    //    name rules.
    if let Some(result) = pattern.result() {
        return result;
    }

    if let Some(result) = first_header_detection(
        column_name,
        &sampled,
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

    if let Some(result) = detect_numeric_value_type(&sampled, values.len(), total_non_empty) {
        return attach_single_trace(
            result,
            total_non_empty,
            "Sample values matched numeric value detection after identifier rules were rejected.",
            "numeric value rule",
        );
    }

    if let Some(result) = first_header_detection(
        column_name,
        &sampled,
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

    if detect_enum_type(&sampled) {
        return detection_result(
            DataType::Enum,
            Confidence::High,
            sampled.len(),
            values.len(),
            total_non_empty,
            "Sample values formed a repeated finite set.",
            vec![trace_item(
                DataType::Enum,
                "finite repeated values",
                sampled.len(),
                total_non_empty,
                Confidence::High,
                true,
            )],
        );
    }

    detection_result(
        DataType::String,
        Confidence::Low,
        sampled.len(),
        values.len(),
        total_non_empty,
        "No sensitive pattern, header, numeric, name, or enum rule passed the threshold.",
        pattern.trace_items,
    )
}

/// Commit a validator-backed selection. The value evidence is final: the
/// column *is* `result.data_type`. If the column header independently agrees
/// on that same type (its matching early-header rule fires), we prefer that
/// rule's result — it carries the richer header-taxonomy trace — and raise its
/// confidence one tier (capped at High), appending a `"header agreement boost"`
/// trace item. The header rule can only fire for the validator's own type, so
/// it can never suppress or replace the selection; absent header agreement, the
/// validator result stands unchanged.
fn finalize_validator(
    column_name: &str,
    validator_result: crate::types::DetectionResult,
    sampled: &[&String],
    total_samples: usize,
    total_non_empty: usize,
    early_header_rules: &[HeaderDetectionRule],
) -> crate::types::DetectionResult {
    let Some(mut agreeing) = first_header_detection(
        column_name,
        sampled,
        total_samples,
        total_non_empty,
        early_header_rules,
    )
    .filter(|header_result| header_result.data_type == validator_result.data_type) else {
        return validator_result;
    };

    let boosted = raise_one_tier(agreeing.confidence);
    agreeing.confidence = boosted;
    if let Some(trace) = agreeing.trace.as_mut() {
        trace.candidates.push(trace_item(
            agreeing.data_type,
            "header agreement boost",
            agreeing.sample_matches,
            trace.total_non_empty,
            boosted,
            true,
        ));
    }

    agreeing
}

/// For a national-ID (idsmith) validator selection, append `":{country}"` to
/// the selected trace item's reason using the first matching sample. This is
/// the deferred trace-label step from Task 5.
fn label_national_id_country(
    result: &mut crate::types::DetectionResult,
    pattern: &PatternOutcome,
    sampled: &[&String],
) {
    let Some(selected) = pattern.selected.as_ref() else {
        return;
    };
    if selected.reason != "validator:idsmith" {
        return;
    }
    let Some(country) = sampled
        .iter()
        .find_map(|value| national_id_countries(value).into_iter().next())
    else {
        return;
    };

    if let Some(trace) = result.trace.as_mut() {
        for item in trace.candidates.iter_mut() {
            if item.reason == "validator:idsmith" {
                item.reason = format!("validator:idsmith:{country}");
            }
        }
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
