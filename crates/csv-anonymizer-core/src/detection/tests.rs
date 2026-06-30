use super::*;
use serde::Deserialize;

mod column_type;
mod privacy;
mod taxonomy;
mod validators;

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn analyze(header: &str, values: &[String]) -> ColumnPrivacyAnalysis {
    let detection = detect_column_type_with_name(header, values);
    analyze_column_privacy(header, 0, values, detection.data_type, detection.confidence)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StdnumVatFixtures {
    valid_vat_ids: Vec<StdnumVatCase>,
    invalid_vat_ids: Vec<StdnumVatCase>,
    valid_dutch_btw_tax_numbers: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct StdnumVatCase {
    country: String,
    value: String,
}

fn stdnum_vat_fixtures() -> StdnumVatFixtures {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/stdnum-vat-cases.json"
    )))
    .expect("stdnum VAT fixture JSON should be valid")
}
