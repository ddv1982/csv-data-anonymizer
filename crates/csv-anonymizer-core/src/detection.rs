use crate::types::{Confidence, DataType, EmptyFormat, PiiRisk};

mod candidate;
mod header;
mod header_rules;
mod locale;
mod national_id;
mod patterns;
mod postal;
mod privacy;
mod scoring;
mod spans;
mod validators;
mod value;

use header_rules::{
    HeaderDetectionRule, contains_address_keyword, first_header_detection, is_plausible_address,
};
use national_id::national_id_countries;
use postal::postal_match_country;
use scoring::{
    attach_single_trace, calculate_confidence, detection_result, raise_one_tier, trace_item,
};
use value::{
    PatternOutcome, detect_enum_type, detect_iban_value_type, detect_numeric_value_type,
    detect_priority_pattern, detect_vat_value_type,
};

pub use locale::{LocaleContext, infer_locale_context};
pub(crate) use privacy::POSSIBLE_PERSON_NAME_DETECTOR;
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

    let early_header_rules = header_rules::early_header_detection_rules();
    // One taxonomy scan for this column, shared by every header rule below and
    // by `finalize_validator`.
    let header = header::analyze(column_name);
    let finalize_validator_selection = |result| {
        finalize_validator(
            &header,
            result,
            &sampled,
            values.len(),
            total_non_empty,
            &early_header_rules,
            locale,
        )
    };

    // STAGE 1 — Validator-backed value evidence, and it is final: the column
    // *is* that sensitive type. The header may only agree-and-boost (adding its
    // richer taxonomy trace and raising confidence one tier); it can never
    // suppress or replace the selection. VAT and IBAN run before the pattern
    // battery so that the battery's cost is skipped when they already claim the
    // column.
    if let Some(result) = detect_vat_value_type(&sampled, values.len(), total_non_empty) {
        return finalize_validator_selection(result);
    }

    if let Some(result) = detect_iban_value_type(&sampled, values.len(), total_non_empty) {
        return finalize_validator_selection(result);
    }

    let pattern = detect_priority_pattern(&sampled, values.len(), total_non_empty, locale);
    if pattern.selected_is_validator() {
        let mut result = pattern
            .result()
            .expect("validator selection yields a result");
        label_national_id_country(&mut result, &pattern, &sampled);
        return finalize_validator_selection(result);
    }

    // STAGE 2 — Header rules whose value evidence is a shape rather than a
    // validator: phone, postal code, address, tax ID.
    if let Some(result) = first_header_detection(
        &header,
        &sampled,
        values.len(),
        total_non_empty,
        &early_header_rules,
        locale,
    ) {
        return result;
    }

    // STAGE 3 — Postal codes that the file's locale context vouches for. Where
    // a country's postal format is bare digits (DE/FR/US/IT/ES/SE/JP...), those
    // values also match the numeric-id shape (`^\d{4,}$`), so this has to come
    // before the pattern battery's own selection in stage 4 or a genuine postal
    // column in a locale-tagged file would be classified as NumericId. It is
    // safe to put it first: the voter only counts values matching a
    // *context-present* country format (see `postal_match_country`'s
    // `requires_context` gate), so a file without matching context — or a
    // bare-digit column whose context format it does not match — falls straight
    // through. Gated on non-empty context so context-free files are unaffected;
    // for them the voter runs at its lower precedence in stage 8 instead.
    let locale_vouches_for_postal = !locale.countries().is_empty();
    if locale_vouches_for_postal
        && let Some(result) =
            detect_postal_value_type(&sampled, values.len(), total_non_empty, locale)
    {
        return result;
    }

    // STAGE 4 — The pattern battery's non-validator selection.
    if let Some(result) = pattern.result() {
        return result;
    }

    // STAGE 5 — Identifiers, header-gated so that a bare integer column is only
    // called an identifier when its header says so.
    //
    // The opaque rule behind the integer rule covers the same headers for keys
    // written in an alphabet the integer rule cannot read (`E1000`, `CUST-0042`,
    // `a1b2c3d4`) — see `header_rules::detect_header_opaque_identifier`. It
    // requires a letter in the value, so the two rules cannot both fire on one
    // column and the order between them is documentation rather than precedence.
    if let Some(result) = first_header_detection(
        &header,
        &sampled,
        values.len(),
        total_non_empty,
        &[
            HeaderDetectionRule {
                detect: header_rules::detect_header_numeric_id,
                selected_reason: "Header terms and integer sample shape matched numeric ID detection.",
                trace_reason: "header numeric ID rule",
            },
            HeaderDetectionRule {
                detect: header_rules::detect_header_opaque_identifier,
                selected_reason: "Header terms and uniform high-cardinality key shape matched identifier \
                     detection.",
                trace_reason: "header opaque identifier rule",
            },
        ],
        locale,
    ) {
        return result;
    }

    // STAGE 6 — Plain measurements, once every identifier rule has declined.
    if let Some(result) = detect_numeric_value_type(&sampled, values.len(), total_non_empty) {
        return attach_single_trace(
            result,
            total_non_empty,
            "Sample values matched numeric value detection after identifier rules were rejected.",
            "numeric value rule",
        );
    }

    // STAGE 7 — Person names. Header-gated: the name gazetteer was withdrawn on
    // data-minimization grounds, so there is no value-level name evidence to
    // vote with (see docs/value-first-detection-design.md).
    if let Some(result) = first_header_detection(
        &header,
        &sampled,
        values.len(),
        total_non_empty,
        &[HeaderDetectionRule {
            detect: header_rules::detect_name_type,
            selected_reason: "Header terms and sample shape matched name detection.",
            trace_reason: "header name rule",
        }],
        locale,
    ) {
        return result;
    }

    // STAGE 8 — Postal and address on value evidence alone. After the name rule
    // so shape-based winners keep priority, but before the enum check so a real
    // postal-code or street-address column is not swallowed by the
    // finite-repeated-values heuristic.
    //
    // Skipped when the locale already vouched for postal codes: stage 3 asked
    // this exact question with these exact arguments and got no for an answer,
    // so asking again can only produce the same no.
    if !locale_vouches_for_postal
        && let Some(result) =
            detect_postal_value_type(&sampled, values.len(), total_non_empty, locale)
    {
        return result;
    }

    if let Some(result) = detect_address_value_type(&sampled, values.len(), total_non_empty) {
        return result;
    }

    // STAGE 9 — Categorical, then the unclassified fallback.

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
    header: &header::HeaderAnalysis,
    validator_result: crate::types::DetectionResult,
    sampled: &[&String],
    total_samples: usize,
    total_non_empty: usize,
    early_header_rules: &[HeaderDetectionRule],
    locale: &LocaleContext,
) -> crate::types::DetectionResult {
    let Some(mut agreeing) = first_header_detection(
        header,
        sampled,
        total_samples,
        total_non_empty,
        early_header_rules,
        locale,
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

/// Postal-code value voter: counts samples whose shape matches a known
/// per-country postal format (bare-digit formats require the country to be
/// present in `locale`; unambiguous formats like NL do not). On a Medium+
/// match ratio, selects `DataType::PostalCode` with a trace reason naming the
/// most frequently matching country.
fn detect_postal_value_type(
    sampled: &[&String],
    total_samples: usize,
    total_non_empty: usize,
    locale: &LocaleContext,
) -> Option<crate::types::DetectionResult> {
    let mut country_counts: std::collections::HashMap<&'static str, usize> =
        std::collections::HashMap::new();
    for value in sampled {
        if let Some(country) = postal_match_country(value, locale) {
            *country_counts.entry(country).or_default() += 1;
        }
    }
    let match_count: usize = country_counts.values().sum();
    let confidence = calculate_confidence(match_count, total_non_empty);
    if confidence == Confidence::Low {
        return None;
    }

    let top_country = country_counts
        .into_iter()
        .max_by_key(|(country, count)| (*count, std::cmp::Reverse(*country)))
        .map(|(country, _)| country)
        .unwrap_or("");

    Some(detection_result(
        DataType::PostalCode,
        confidence,
        match_count,
        total_samples,
        total_non_empty,
        "Sample values matched a per-country postal code format.",
        vec![trace_item(
            DataType::PostalCode,
            format!("postal:{top_country}"),
            match_count,
            total_non_empty,
            confidence,
            true,
        )],
    ))
}

/// Address value voter: counts samples with a plausible street-address shape
/// (digits + letters, comma/whitespace structure, or a known street keyword).
/// Requires that at least 30% of the *matching* values contain a street
/// keyword, guarding against generic digit+letter strings being misread as
/// addresses. On a Medium+ match ratio, selects `DataType::Address`.
fn detect_address_value_type(
    sampled: &[&String],
    total_samples: usize,
    total_non_empty: usize,
) -> Option<crate::types::DetectionResult> {
    let matches: Vec<&&String> = sampled
        .iter()
        .filter(|value| is_plausible_address(value))
        .collect();
    let match_count = matches.len();
    let confidence = calculate_confidence(match_count, total_non_empty);
    if confidence == Confidence::Low {
        return None;
    }

    let keyword_count = matches
        .iter()
        .filter(|value| {
            let normalized = value.to_lowercase();
            contains_address_keyword(&normalized)
        })
        .count();
    if keyword_count * 10 < match_count * 3 {
        return None;
    }

    Some(detection_result(
        DataType::Address,
        confidence,
        match_count,
        total_samples,
        total_non_empty,
        "Sample values matched address shape and street keywords.",
        vec![trace_item(
            DataType::Address,
            "address shape + street keywords",
            match_count,
            total_non_empty,
            confidence,
            true,
        )],
    ))
}

/// The risk a column carries on the strength of its type alone.
///
/// This is a floor. Callers combine it with `analyze_column_privacy`'s findings
/// through [`max_pii_risk`], and a finding can only raise the result — so the
/// answer here is a lower bound on what the app acts on, never an upper one.
///
/// It must nonetheless agree with the risk of the type's own
/// `privacy_finding_kind_and_reason`. Where the two disagree the lower one is
/// unreachable, which makes it dead code that still reads as the rule — and the
/// dead branch is the one a reader trusts. `every_data_type_gets_one_consistent_risk_from_both_sources`
/// fails when that happens.
pub fn classify_pii_risk(data_type: DataType) -> PiiRisk {
    match data_type {
        // A column of given names or of surnames is personal data about identifiable
        // people, which is why each one already carries a `Person` finding at High.
        DataType::Email
        | DataType::Phone
        | DataType::FirstName
        | DataType::LastName
        | DataType::FullName
        | DataType::Address
        | DataType::TaxId => PiiRisk::High,
        DataType::Uuid
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
