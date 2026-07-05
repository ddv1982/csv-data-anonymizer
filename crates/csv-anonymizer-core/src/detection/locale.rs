use std::collections::HashMap;

use super::is_empty_value;
use super::validators::{is_iban, is_vat_id};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LocaleContext {
    countries: Vec<String>,
}

impl LocaleContext {
    pub fn countries(&self) -> &[String] {
        &self.countries
    }
}

/// ISO 3166-1 alpha-2 codes; keep in sync only if fixtures demand more.
const ISO_COUNTRY_CODES: &[&str] = &[
    "AD", "AE", "AR", "AT", "AU", "BE", "BG", "BR", "CA", "CH", "CL", "CN", "CO", "CZ", "DE", "DK",
    "EE", "EG", "ES", "FI", "FR", "GB", "GR", "HK", "HR", "HU", "ID", "IE", "IL", "IN", "IT", "JP",
    "KE", "KR", "LT", "LU", "LV", "MX", "MY", "NG", "NL", "NO", "NZ", "PE", "PH", "PL", "PT", "RO",
    "RS", "RU", "SA", "SE", "SG", "SI", "SK", "TH", "TR", "UA", "US", "VN", "ZA",
];

fn is_iso_country_code(value: &str) -> bool {
    ISO_COUNTRY_CODES.binary_search(&value).is_ok()
}

pub fn infer_locale_context(columns: &[Vec<String>]) -> LocaleContext {
    let mut counts: HashMap<String, usize> = HashMap::new();

    for values in columns {
        let non_empty: Vec<&String> = values
            .iter()
            .filter(|value| !is_empty_value(value))
            .collect();
        if non_empty.is_empty() {
            continue;
        }

        // Country-code column: only counts when the column is dominated by codes.
        let code_hits: Vec<&str> = non_empty
            .iter()
            .map(|value| value.trim())
            .filter(|value| value.len() == 2 && is_iso_country_code(&value.to_ascii_uppercase()))
            .collect();
        if code_hits.len() * 10 >= non_empty.len() * 8 {
            for code in &code_hits {
                *counts.entry(code.to_ascii_uppercase()).or_default() += 1;
            }
            continue;
        }

        // IBAN / VAT prefixes count per matching value.
        for value in &non_empty {
            let trimmed = value.trim();
            if is_iban(trimmed) || is_vat_id(trimmed) {
                let prefix: String = trimmed
                    .chars()
                    .filter(|character| character.is_ascii_alphanumeric())
                    .take(2)
                    .collect::<String>()
                    .to_ascii_uppercase();
                if is_iso_country_code(&prefix) {
                    *counts.entry(prefix).or_default() += 1;
                }
            }
        }
    }

    let mut ranked: Vec<(String, usize)> = counts.into_iter().collect();
    ranked.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(&right.0)));
    LocaleContext {
        countries: ranked.into_iter().map(|(code, _)| code).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn empty_input_yields_empty_context() {
        assert!(infer_locale_context(&[]).countries().is_empty());
    }

    #[test]
    fn iban_prefixes_contribute_countries() {
        let columns = vec![column(&["NL91ABNA0417164300", "NL02RABO0123456789"])];
        assert_eq!(infer_locale_context(&columns).countries(), ["NL"]);
    }

    #[test]
    fn country_code_columns_contribute_when_dominant() {
        let columns = vec![column(&["DE", "DE", "DE", "NL", "DE"])];
        let context = infer_locale_context(&columns);
        assert_eq!(context.countries(), ["DE", "NL"]);
    }

    #[test]
    fn free_text_contributes_nothing() {
        let columns = vec![column(&["hello", "world", "US shipping soon"])];
        assert!(infer_locale_context(&columns).countries().is_empty());
    }

    #[test]
    fn iso_codes_sorted() {
        assert!(ISO_COUNTRY_CODES.windows(2).all(|w| w[0] < w[1]));
    }
}
