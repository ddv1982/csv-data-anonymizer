use crate::types::{Confidence, DataType};
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::OnceLock;
use strsim::jaro_winkler;
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

const HEADER_TAXONOMY_JSON: &str = include_str!("header_taxonomy.json");

#[derive(Debug, Clone)]
pub(super) struct HeaderTerms {
    pub(super) compact: String,
    tokens: HashSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct HeaderSignal {
    pub(super) kind: String,
    pub(super) concept: String,
    pub(super) data_type: DataType,
    pub(super) confidence: Confidence,
    pub(super) score: u8,
    pub(super) detector: String,
    pub(super) reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HeaderTaxonomy {
    terms: Vec<TaxonomyTerm>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaxonomyTerm {
    kind: String,
    concept: String,
    data_type: DataType,
    lang: String,
    text: String,
    weight: u8,
    #[serde(default)]
    match_mode: MatchMode,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum MatchMode {
    Exact,
    #[default]
    Token,
    AllTokens,
    Suffix,
    Contains,
}

impl HeaderTerms {
    #[allow(dead_code)]
    fn has(&self, token: &str) -> bool {
        self.tokens.contains(&fold_key(token))
    }

    #[allow(dead_code)]
    fn has_all(&self, tokens: &[&str]) -> bool {
        tokens.iter().all(|token| self.has(token))
    }
}

pub(super) fn terms(column_name: &str) -> HeaderTerms {
    HeaderTerms {
        compact: compact(column_name),
        tokens: tokens(column_name),
    }
}

pub(super) fn best_signal_for_kinds(terms: &HeaderTerms, kinds: &[&str]) -> Option<HeaderSignal> {
    taxonomy_terms()
        .iter()
        .filter(|term| kinds.contains(&term.kind.as_str()))
        .filter_map(|term| taxonomy_term_match(terms, term))
        .max_by(|left, right| {
            left.score
                .cmp(&right.score)
                .then(left.term.text.len().cmp(&right.term.text.len()))
        })
        .map(signal_from_match)
}

#[allow(dead_code)]
pub(super) fn infer_secret(terms: &HeaderTerms) -> bool {
    best_signal_for_kinds(terms, &["secret"]).is_some()
}

#[allow(dead_code)]
pub(super) fn infer_account_number(terms: &HeaderTerms) -> bool {
    best_signal_for_kinds(terms, &["account_number"]).is_some()
}

#[allow(dead_code)]
pub(super) fn infer_account_identifier(terms: &HeaderTerms) -> bool {
    best_signal_for_kinds(terms, &["account_identifier"]).is_some()
}

#[allow(dead_code)]
pub(super) fn infer_numeric_id(column_name: &str) -> bool {
    let terms = terms(column_name);
    best_signal_for_kinds(&terms, &["numeric_id"]).is_some()
}

pub(super) fn infer_private_date(column_name: &str) -> bool {
    let terms = terms(column_name);
    best_signal_for_kinds(&terms, &["private_date"]).is_some()
}

pub(super) fn infer_user_event_date(column_name: &str) -> bool {
    let terms = terms(column_name);
    best_signal_for_kinds(&terms, &["user_event_date"]).is_some()
}

#[allow(dead_code)]
pub(super) fn infer_postal_code(column_name: &str) -> bool {
    let terms = terms(column_name);
    best_signal_for_kinds(&terms, &["postal_code"]).is_some()
}

#[allow(dead_code)]
pub(super) fn infer_phone(column_name: &str) -> bool {
    let terms = terms(column_name);
    best_signal_for_kinds(&terms, &["phone"]).is_some()
}

#[allow(dead_code)]
pub(super) fn infer_address(column_name: &str) -> bool {
    let terms = terms(column_name);
    best_signal_for_kinds(&terms, &["address"]).is_some()
}

#[allow(dead_code)]
pub(super) fn infer_tax_id(column_name: &str) -> bool {
    let terms = terms(column_name);
    best_signal_for_kinds(&terms, &["tax_id"]).is_some()
}

#[allow(dead_code)]
pub(super) fn infer_name_type(column_name: &str) -> Option<DataType> {
    let terms = terms(column_name);

    for kind in ["first_name", "last_name", "full_name", "generic_name"] {
        if let Some(signal) = best_signal_for_kinds(&terms, &[kind]) {
            return Some(signal.data_type);
        }
    }

    None
}

pub(super) fn infer_generic_name(column_name: &str) -> bool {
    let terms = terms(column_name);
    best_signal_for_kinds(&terms, &["generic_name"]).is_some()
}

fn taxonomy_terms() -> &'static [TaxonomyTerm] {
    static TERMS: OnceLock<Vec<TaxonomyTerm>> = OnceLock::new();
    TERMS
        .get_or_init(|| {
            serde_json::from_str::<HeaderTaxonomy>(HEADER_TAXONOMY_JSON)
                .expect("header taxonomy JSON should be valid")
                .terms
        })
        .as_slice()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HeaderMatchKind {
    Exact,
    Fuzzy,
}

#[derive(Debug, Clone, Copy)]
struct HeaderTermMatch<'a> {
    term: &'a TaxonomyTerm,
    score: u8,
    kind: HeaderMatchKind,
}

fn taxonomy_term_match<'a>(
    terms: &HeaderTerms,
    term: &'a TaxonomyTerm,
) -> Option<HeaderTermMatch<'a>> {
    if taxonomy_term_matches(terms, term) {
        return Some(HeaderTermMatch {
            term,
            score: term.weight,
            kind: HeaderMatchKind::Exact,
        });
    }

    taxonomy_term_fuzzy_matches(terms, term).then_some(HeaderTermMatch {
        term,
        score: fuzzy_weight(term.weight),
        kind: HeaderMatchKind::Fuzzy,
    })
}

fn taxonomy_term_matches(terms: &HeaderTerms, term: &TaxonomyTerm) -> bool {
    let term_terms = self::terms(&term.text);
    if term_terms.compact.is_empty() {
        return false;
    }

    match term.match_mode {
        MatchMode::Exact => terms.compact == term_terms.compact,
        MatchMode::Token | MatchMode::AllTokens => {
            terms.compact == term_terms.compact
                || term_terms
                    .tokens
                    .iter()
                    .all(|token| terms.tokens.contains(token))
        }
        MatchMode::Suffix => terms.compact.ends_with(&term_terms.compact),
        MatchMode::Contains => terms.compact.contains(&term_terms.compact),
    }
}

fn taxonomy_term_fuzzy_matches(terms: &HeaderTerms, term: &TaxonomyTerm) -> bool {
    if term.weight < 88 {
        return false;
    }
    if matches!(term.kind.as_str(), "private_date" | "user_event_date") {
        return false;
    }

    let term_terms = self::terms(&term.text);
    if !can_fuzzy_match_term(&term_terms) {
        return false;
    }

    fuzzy_all_tokens_match(terms, &term_terms)
        || fuzzy_compact_match(terms, &term_terms, term.match_mode)
}

fn can_fuzzy_match_term(term_terms: &HeaderTerms) -> bool {
    term_terms.compact.len() >= 5
        && term_terms.compact.is_ascii()
        && term_terms.tokens.iter().all(|token| {
            token.len() >= 5
                || token
                    .chars()
                    .all(|character| character.is_ascii_alphabetic())
        })
}

fn fuzzy_all_tokens_match(terms: &HeaderTerms, term_terms: &HeaderTerms) -> bool {
    !term_terms.tokens.is_empty()
        && term_terms
            .tokens
            .iter()
            .all(|term_token| fuzzy_token_present(terms, term_token))
}

fn fuzzy_token_present(terms: &HeaderTerms, term_token: &str) -> bool {
    if term_token.len() < 5 {
        return terms.tokens.contains(term_token);
    }

    terms.tokens.iter().any(|header_token| {
        header_token.len() >= 5 && jaro_winkler(header_token, term_token) >= 0.92
    })
}

fn fuzzy_compact_match(
    terms: &HeaderTerms,
    term_terms: &HeaderTerms,
    match_mode: MatchMode,
) -> bool {
    let term_len = term_terms.compact.len();
    if !terms.compact.is_ascii()
        || term_len < 8
        || (terms.compact.len().abs_diff(term_len) > 3 && match_mode != MatchMode::Suffix)
    {
        return false;
    }

    let candidate = if match_mode == MatchMode::Suffix && terms.compact.len() > term_len {
        &terms.compact[terms.compact.len() - term_len..]
    } else {
        terms.compact.as_str()
    };

    candidate.len().abs_diff(term_len) <= 3
        && has_matching_edge_chars(candidate, &term_terms.compact)
        && jaro_winkler(candidate, &term_terms.compact) >= 0.93
}

fn has_matching_edge_chars(left: &str, right: &str) -> bool {
    left.chars().next() == right.chars().next() && left.chars().last() == right.chars().last()
}

fn fuzzy_weight(weight: u8) -> u8 {
    weight.saturating_sub(12).max(70)
}

fn signal_from_match(term_match: HeaderTermMatch<'_>) -> HeaderSignal {
    let term = term_match.term;
    let kind_label = term.kind.replace('_', "-");
    let detector = match term_match.kind {
        HeaderMatchKind::Exact => format!("header:taxonomy:{kind_label}"),
        HeaderMatchKind::Fuzzy => format!("header:taxonomy-fuzzy:{kind_label}"),
    };
    let reason = match term_match.kind {
        HeaderMatchKind::Exact => format!(
            "Header taxonomy term '{}' ({}) matched {}.",
            term.text, term.lang, term.concept
        ),
        HeaderMatchKind::Fuzzy => format!(
            "Header approximately matched taxonomy term '{}' ({}) for {}.",
            term.text, term.lang, term.concept
        ),
    };

    HeaderSignal {
        kind: term.kind.clone(),
        concept: term.concept.clone(),
        data_type: term.data_type,
        confidence: confidence_for_weight(term_match.score),
        score: term_match.score,
        detector,
        reason,
    }
}

fn confidence_for_weight(weight: u8) -> Confidence {
    if weight >= 90 {
        Confidence::High
    } else if weight >= 70 {
        Confidence::Medium
    } else {
        Confidence::Low
    }
}

fn compact(column_name: &str) -> String {
    fold_key(column_name)
        .chars()
        .filter(|character| character.is_alphanumeric())
        .collect()
}

fn tokens(column_name: &str) -> HashSet<String> {
    let mut tokens = HashSet::new();
    let normalized = column_name.nfkc().collect::<String>();

    for token in normalized.unicode_words() {
        insert_token_and_camel_case_subtokens(&mut tokens, token);
    }

    for token in normalized.split(|character: char| !character.is_alphanumeric()) {
        insert_token_and_camel_case_subtokens(&mut tokens, token);
    }

    tokens
}

fn insert_token_and_camel_case_subtokens(tokens: &mut HashSet<String>, token: &str) {
    if token.is_empty() {
        return;
    }

    insert_token(tokens, token);
    for subtoken in camel_case_tokens(token) {
        insert_token(tokens, &subtoken);
    }
}

fn insert_token(tokens: &mut HashSet<String>, token: &str) {
    let folded = fold_key(token);
    if !folded.is_empty() {
        tokens.insert(folded);
    }
}

fn fold_key(value: &str) -> String {
    let normalized = value.nfkc().collect::<String>();
    normalized
        .nfd()
        .filter(|character| !is_combining_mark(*character))
        .flat_map(char::to_lowercase)
        .collect()
}

fn is_combining_mark(character: char) -> bool {
    matches!(
        character as u32,
        0x0300..=0x036F
            | 0x1AB0..=0x1AFF
            | 0x1DC0..=0x1DFF
            | 0x20D0..=0x20FF
            | 0xFE20..=0xFE2F
    )
}

fn camel_case_tokens(token: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut previous: Option<char> = None;

    for character in token.chars() {
        if should_split_camel_case(previous, character, &current) {
            tokens.push(current.clone());
            current.clear();
        }
        current.push(character);
        previous = Some(character);
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

fn should_split_camel_case(previous: Option<char>, current: char, token: &str) -> bool {
    !token.is_empty()
        && current.is_uppercase()
        && previous.is_some_and(|previous| previous.is_lowercase() || previous.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_preserves_non_ascii_headers() {
        let terms = terms("teléfono_電話番号");

        assert_eq!(terms.compact, "telefono電話番号");
        assert!(terms.has("telefono"));
        assert!(terms.has("電話番号"));
    }

    #[test]
    fn taxonomy_matches_accent_folded_and_non_latin_terms() {
        assert!(infer_phone("teléfono"));
        assert!(infer_phone("電話番号"));
        assert!(infer_address("dirección"));
        assert_eq!(infer_name_type("prénom"), Some(DataType::FirstName));
    }

    #[test]
    fn taxonomy_keeps_existing_camel_case_behavior() {
        let header_terms = terms("dateOfBirth");

        assert!(header_terms.has_all(&["date", "birth"]));
        assert!(infer_private_date("dateOfBirth"));
        assert!(infer_phone("phoneNumber"));
        assert!(infer_phone("homephone"));
        assert!(infer_phone("workphone"));
        assert!(infer_secret(&terms("apikey")));
        assert!(!infer_phone("headphone"));
    }

    #[test]
    fn taxonomy_fuzzy_matches_long_typos_only() {
        let phone_signal =
            best_signal_for_kinds(&terms("telefoonnumer"), &["phone"]).expect("fuzzy phone signal");
        assert_eq!(phone_signal.data_type, DataType::Phone);
        assert!(phone_signal.detector.starts_with("header:taxonomy-fuzzy"));

        let tax_signal =
            best_signal_for_kinds(&terms("btw_numner"), &["tax_id"]).expect("fuzzy tax signal");
        assert_eq!(tax_signal.data_type, DataType::TaxId);
        assert!(tax_signal.detector.starts_with("header:taxonomy-fuzzy"));

        assert!(best_signal_for_kinds(&terms("idx"), &["numeric_id"]).is_none());
        assert!(best_signal_for_kinds(&terms("nam"), &["generic_name"]).is_none());
        assert!(best_signal_for_kinds(&terms("niff"), &["tax_id"]).is_none());
        assert!(!infer_phone("headphone"));
    }

    #[test]
    fn taxonomy_terms_are_well_formed_and_unique() {
        let mut seen = HashSet::new();

        for term in taxonomy_terms() {
            assert!(!term.kind.trim().is_empty(), "{term:?}");
            assert!(!term.concept.trim().is_empty(), "{term:?}");
            assert!(!term.lang.trim().is_empty(), "{term:?}");
            assert!(!term.text.trim().is_empty(), "{term:?}");
            assert!(term.weight > 0, "{term:?}");
            assert!(
                matches!(
                    term.data_type,
                    DataType::String
                        | DataType::NumericId
                        | DataType::Timestamp
                        | DataType::PostalCode
                        | DataType::Phone
                        | DataType::Address
                        | DataType::TaxId
                        | DataType::FirstName
                        | DataType::LastName
                        | DataType::FullName
                ),
                "{term:?}"
            );

            let key = format!(
                "{}:{}:{}:{:?}",
                term.kind,
                term.lang,
                term.text.nfkc().collect::<String>().to_lowercase(),
                term.match_mode
            );
            assert!(seen.insert(key), "duplicate taxonomy term {term:?}");
        }
    }
}
