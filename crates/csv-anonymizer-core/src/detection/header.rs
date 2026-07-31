use crate::types::{Confidence, DataType};
use serde::Deserialize;
use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
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
    pub(super) concept: String,
    pub(super) data_type: DataType,
    pub(super) confidence: Confidence,
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

/// How a taxonomy term is compared against a normalized header.
///
/// `Token` and `AllTokens` deliberately share one rule: every token of the term
/// must appear in the header. They are not two behaviors but two names for the
/// same one, distinguishing arity — `Token` is used for single-word terms, where
/// "all of its tokens" is just "its one token", and `AllTokens` for multi-word
/// terms, where the phrase's words may appear in any order but must all be
/// present. `taxonomy_match_modes_match_term_arity` pins that convention, so if
/// a multi-word term ever arrives as `Token` the suite says so rather than
/// letting the two names quietly mean different things.
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum MatchMode {
    Exact,
    #[default]
    Token,
    AllTokens,
    Suffix,
}

fn terms(column_name: &str) -> HeaderTerms {
    HeaderTerms {
        compact: compact(column_name),
        tokens: tokens(column_name),
    }
}

/// Everything the taxonomy has to say about one column header, from one scan.
///
/// A single column consults the taxonomy from many places: each header rule, the
/// name rule, and several branches of the privacy analysis. Asking per
/// consultation meant rescanning all taxonomy terms — including the fuzzy
/// Jaro-Winkler comparisons — five to ten times per column, which dominated
/// analysis time on wide files. Scanning once and then answering by kind makes
/// the cost proportional to the number of headers instead of to the number of
/// rules that ask about them.
pub(super) struct HeaderAnalysis {
    terms: HeaderTerms,
    /// Best-scoring match per taxonomy kind; see [`HeaderAnalysis::best_for_kinds`]
    /// for how a set of kinds is then ranked against each other.
    best_by_kind: HashMap<&'static str, HeaderTermMatch<'static>>,
}

pub(super) fn analyze(column_name: &str) -> HeaderAnalysis {
    let terms = terms(column_name);
    let mut best_by_kind: HashMap<&'static str, HeaderTermMatch<'static>> = HashMap::new();

    for prepared in taxonomy_terms() {
        let Some(term_match) = taxonomy_term_match(&terms, prepared) else {
            continue;
        };
        let kind = prepared.term.kind.as_str();
        let replaces = best_by_kind
            .get(kind)
            .is_none_or(|existing| match_rank(existing) < match_rank(&term_match));
        if replaces {
            best_by_kind.insert(kind, term_match);
        }
    }

    HeaderAnalysis {
        terms,
        best_by_kind,
    }
}

impl HeaderAnalysis {
    pub(super) fn terms(&self) -> &HeaderTerms {
        &self.terms
    }

    /// The strongest taxonomy match among `kinds`.
    ///
    /// `kinds` is in caller priority order and equal ranks resolve to the earlier
    /// one, which matters because for `detect_name_type` the winning kind *is* the
    /// classification: its `["first_name", "last_name", "full_name",
    /// "generic_name"]` runs specific to generic, so a tie has to fall to the
    /// specific end rather than to wherever iteration happened to finish. Ranking
    /// the kinds' best matches is equivalent to ranking every match at once
    /// because the comparator is the same, but only for picking the maximum —
    /// which of two equally ranked matches wins is this tie rule's business, so it
    /// is stated rather than inherited.
    pub(super) fn best_for_kinds(&self, kinds: &[&str]) -> Option<HeaderSignal> {
        kinds
            .iter()
            .enumerate()
            .filter_map(|(position, kind)| {
                self.best_by_kind
                    .get(*kind)
                    .map(|term_match| (position, term_match))
            })
            .max_by_key(|(position, term_match)| (match_rank(term_match), Reverse(*position)))
            .map(|(_, term_match)| *term_match)
            .map(signal_from_match)
    }

    pub(super) fn matches_kind(&self, kind: &str) -> bool {
        self.best_by_kind.contains_key(kind)
    }
}

/// Higher wins: stronger score first, then the longer taxonomy term, which is
/// the more specific phrase.
fn match_rank(term_match: &HeaderTermMatch<'_>) -> (u8, usize) {
    (term_match.score, term_match.term.text.len())
}

/// A taxonomy term with its normalized form precomputed.
///
/// Normalizing a term does not depend on the header being matched, so doing it at
/// load time keeps [`analyze`] from re-normalizing every candidate term on every
/// column — which it otherwise does once per taxonomy term per column.
struct PreparedTerm {
    term: TaxonomyTerm,
    normalized: HeaderTerms,
}

fn taxonomy_terms() -> &'static [PreparedTerm] {
    static TERMS: OnceLock<Vec<PreparedTerm>> = OnceLock::new();
    TERMS
        .get_or_init(|| {
            serde_json::from_str::<HeaderTaxonomy>(HEADER_TAXONOMY_JSON)
                .expect("header taxonomy JSON should be valid")
                .terms
                .into_iter()
                .map(|term| PreparedTerm {
                    normalized: terms(&term.text),
                    term,
                })
                .collect()
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
    prepared: &'a PreparedTerm,
) -> Option<HeaderTermMatch<'a>> {
    let term = &prepared.term;
    if taxonomy_term_matches(terms, prepared) {
        return Some(HeaderTermMatch {
            term,
            score: term.weight,
            kind: HeaderMatchKind::Exact,
        });
    }

    taxonomy_term_fuzzy_matches(terms, prepared).then_some(HeaderTermMatch {
        term,
        score: fuzzy_weight(term.weight),
        kind: HeaderMatchKind::Fuzzy,
    })
}

fn taxonomy_term_matches(terms: &HeaderTerms, prepared: &PreparedTerm) -> bool {
    let term = &prepared.term;
    let term_terms = &prepared.normalized;
    if term_terms.compact.is_empty() {
        return false;
    }

    match term.match_mode {
        MatchMode::Exact => terms.compact == term_terms.compact,
        // See `MatchMode`: one rule, two names for it.
        MatchMode::Token | MatchMode::AllTokens => {
            terms.compact == term_terms.compact
                || term_terms
                    .tokens
                    .iter()
                    .all(|token| terms.tokens.contains(token))
        }
        MatchMode::Suffix => terms.compact.ends_with(&term_terms.compact),
    }
}

fn taxonomy_term_fuzzy_matches(terms: &HeaderTerms, prepared: &PreparedTerm) -> bool {
    let term = &prepared.term;
    if term.weight < 88 {
        return false;
    }
    if matches!(term.kind.as_str(), "private_date" | "user_event_date") {
        return false;
    }

    let term_terms = &prepared.normalized;
    if !can_fuzzy_match_term(term_terms) {
        return false;
    }

    fuzzy_all_tokens_match(terms, term_terms)
        || fuzzy_compact_match(terms, term_terms, term.match_mode)
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
        concept: term.concept.clone(),
        data_type: term.data_type,
        confidence: confidence_for_weight(term_match.score),
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

    fn has_token(column_name: &str, token: &str) -> bool {
        terms(column_name).tokens.contains(&fold_key(token))
    }

    fn matches_kind(column_name: &str, kind: &str) -> bool {
        analyze(column_name).matches_kind(kind)
    }

    fn signal_for(column_name: &str, kind: &str) -> Option<HeaderSignal> {
        analyze(column_name).best_for_kinds(&[kind])
    }

    #[test]
    fn normalization_preserves_non_ascii_headers() {
        assert_eq!(terms("teléfono_電話番号").compact, "telefono電話番号");
        assert!(has_token("teléfono_電話番号", "telefono"));
        assert!(has_token("teléfono_電話番号", "電話番号"));
    }

    #[test]
    fn taxonomy_matches_accent_folded_and_non_latin_terms() {
        assert!(matches_kind("teléfono", "phone"));
        assert!(matches_kind("電話番号", "phone"));
        assert!(matches_kind("dirección", "address"));
        assert_eq!(
            signal_for("prénom", "first_name").map(|signal| signal.data_type),
            Some(DataType::FirstName)
        );
    }

    #[test]
    fn taxonomy_keeps_existing_camel_case_behavior() {
        assert!(has_token("dateOfBirth", "date"));
        assert!(has_token("dateOfBirth", "birth"));
        assert!(matches_kind("dateOfBirth", "private_date"));
        assert!(matches_kind("phoneNumber", "phone"));
        assert!(matches_kind("homephone", "phone"));
        assert!(matches_kind("workphone", "phone"));
        assert!(matches_kind("apikey", "secret"));
        assert!(!matches_kind("headphone", "phone"));
    }

    #[test]
    fn taxonomy_fuzzy_matches_long_typos_only() {
        let phone_signal = signal_for("telefoonnumer", "phone").expect("fuzzy phone signal");
        assert_eq!(phone_signal.data_type, DataType::Phone);
        assert!(phone_signal.detector.starts_with("header:taxonomy-fuzzy"));

        let tax_signal = signal_for("btw_numner", "tax_id").expect("fuzzy tax signal");
        assert_eq!(tax_signal.data_type, DataType::TaxId);
        assert!(tax_signal.detector.starts_with("header:taxonomy-fuzzy"));

        assert!(!matches_kind("idx", "numeric_id"));
        assert!(!matches_kind("nam", "generic_name"));
        assert!(!matches_kind("niff", "tax_id"));
        assert!(!matches_kind("headphone", "phone"));
    }

    /// `Token` and `AllTokens` are one rule under two names, and that only holds
    /// because the names track arity: single-word terms use `Token`, multi-word
    /// terms use `AllTokens`. A multi-word `Token` entry would be the first case
    /// where the two names could plausibly mean different things — any-token
    /// versus all-tokens — so it has to be a deliberate decision, not a silent
    /// taxonomy edit.
    #[test]
    fn taxonomy_match_modes_match_term_arity() {
        for prepared in taxonomy_terms() {
            let term = &prepared.term;
            let token_count = prepared.normalized.tokens.len();
            match term.match_mode {
                MatchMode::Token => assert_eq!(
                    token_count, 1,
                    "multi-word term {:?} uses matchMode 'token'; decide whether it means \
                     any-token or all-tokens before adding it",
                    term.text
                ),
                MatchMode::AllTokens => assert!(
                    token_count > 1,
                    "single-word term {:?} uses matchMode 'allTokens', which is just 'token' \
                     for one word",
                    term.text
                ),
                MatchMode::Exact | MatchMode::Suffix => {}
            }
        }
    }

    /// Ranking a set of kinds must not depend on the order the kinds happen to be
    /// stored or iterated in. `best_for_kinds` resolves equal ranks to the earlier
    /// kind in the caller's list, so reversing the list can only change the answer
    /// when there is a genuine tie — and for `detect_name_type` the winning kind is
    /// the classification, so a tie silently resolving the other way would change a
    /// detected type.
    #[test]
    fn best_for_kinds_prefers_the_earlier_kind_on_equal_rank() {
        let name_kinds = ["first_name", "last_name", "full_name", "generic_name"];
        let reversed = ["generic_name", "full_name", "last_name", "first_name"];

        for prepared in taxonomy_terms() {
            let analysis = analyze(&prepared.term.text);
            let Some(forward) = analysis.best_for_kinds(&name_kinds) else {
                continue;
            };
            let backward = analysis
                .best_for_kinds(&reversed)
                .expect("the same kinds match either way round");

            // A disagreement here is a tie, and a tie must resolve to the kind the
            // caller listed first — so the forward answer has to be the one whose
            // kind appears earliest in `name_kinds`.
            if forward != backward {
                let position = |signal: &HeaderSignal| {
                    name_kinds
                        .iter()
                        .position(|kind| analysis.best_for_kinds(&[kind]).as_ref() == Some(signal))
                };
                assert!(
                    position(&forward) < position(&backward),
                    "header {:?} ties across name kinds and resolved to the later kind",
                    prepared.term.text
                );
            }
        }
    }

    /// Terms that can tie *within* one kind have to mean the same thing.
    ///
    /// `analyze` keeps one match per kind and `match_rank` is `(score, text.len())`, so
    /// same-kind terms of equal score and equal length are separated by nothing but
    /// taxonomy order. 41 such groups exist today and 15 of them fire on a real header —
    /// `postal code` against `code postal`, `adresse` in fr against de, `nome` in pt
    /// against it — because a term's translations are naturally the same length as often
    /// as not.
    ///
    /// Every one of those groups agrees on `concept` and `data_type`, which is the whole
    /// reason the tie is cosmetic: the winner decides only which term text and language
    /// the evidence `reason` quotes, never what the column is classified as. That is a
    /// property of the taxonomy's contents, not of the code, so a term added tomorrow can
    /// take it away — a same-kind synonym pointing at a different `dataType` would turn
    /// taxonomy line order into a silent classification decision. This test is the guard
    /// that fires on that edit rather than after it ships.
    #[test]
    fn equal_ranked_terms_of_one_kind_agree_on_concept_and_data_type() {
        // A term scores its weight on an exact match and `fuzzy_weight` on a fuzzy one,
        // so both are reachable and either can be the score that ties.
        let mut by_rank: HashMap<(&str, u8, usize), Vec<&TaxonomyTerm>> = HashMap::new();
        for prepared in taxonomy_terms() {
            let term = &prepared.term;
            for score in [term.weight, fuzzy_weight(term.weight)] {
                by_rank
                    .entry((term.kind.as_str(), score, term.text.len()))
                    .or_default()
                    .push(term);
            }
        }

        let mut conflicts = Vec::new();
        for ((kind, score, length), terms) in &by_rank {
            for term in terms {
                let first = terms[0];
                if term.concept != first.concept || term.data_type != first.data_type {
                    conflicts.push(format!(
                        "{kind} at score {score} length {length}: {:?} ({}) is {} / {:?} but \
                         {:?} ({}) is {} / {:?}",
                        first.text,
                        first.lang,
                        first.concept,
                        first.data_type,
                        term.text,
                        term.lang,
                        term.concept,
                        term.data_type
                    ));
                }
            }
        }

        assert!(
            conflicts.is_empty(),
            "equally ranked terms of one kind disagree on what they mean, so taxonomy line \
             order now decides a classification:\n  {}",
            conflicts.join("\n  ")
        );
    }

    /// The intra-kind tie resolves to the earlier taxonomy term.
    ///
    /// This changeset inverted that. The removed `best_signal_for_kinds` ranked with
    /// `max_by`, which returns the *last* equal maximum; `analyze` replaces its stored
    /// match only on a strictly higher rank, so the *first* term now wins. Both orders are
    /// defensible and nothing pinned either, which is the problem: the rule was a side
    /// effect of which combinator someone reached for, so it changed without anyone
    /// choosing to change it.
    ///
    /// `equal_ranked_terms_of_one_kind_agree_on_concept_and_data_type` is what keeps the
    /// rule cheap to hold — while it passes, the only thing riding on this is the term
    /// quoted in the evidence `reason`. Pinning it makes that a decision instead.
    #[test]
    fn equal_ranked_terms_of_one_kind_resolve_to_the_earlier_term() {
        let header = "postal code";
        let kind = "postal_code";

        let normalized = terms(header);
        let matches: Vec<(&TaxonomyTerm, (u8, usize))> = taxonomy_terms()
            .iter()
            .filter(|prepared| prepared.term.kind == kind)
            .filter_map(|prepared| {
                taxonomy_term_match(&normalized, prepared)
                    .map(|term_match| (&prepared.term, match_rank(&term_match)))
            })
            .collect();

        let best_rank = matches
            .iter()
            .map(|(_, rank)| *rank)
            .max()
            .expect("the header matches at least one term of its own kind");
        let tied: Vec<&TaxonomyTerm> = matches
            .iter()
            .filter(|(_, rank)| *rank == best_rank)
            .map(|(term, _)| *term)
            .collect();

        assert!(
            tied.len() > 1,
            "{header:?} no longer produces an intra-kind tie, so this test is not exercising \
             the tie-break; pick a header that still ties"
        );

        let signal = analyze(header)
            .best_for_kinds(&[kind])
            .expect("the header matches its kind");
        assert!(
            signal.reason.contains(&tied[0].text),
            "the tie resolved to {:?} instead of the earliest tied term {:?}",
            signal.reason,
            tied[0].text
        );
    }

    /// A term that means "plain identifier" must not also claim to mean "bank
    /// account".
    ///
    /// The two kinds look interchangeable — both carry `dataType: numericId`, and
    /// `detect_header_numeric_id` consults them together — but they part company in
    /// the privacy analysis: `account_number` produces an `AccountOrFinancialId`
    /// finding, which is High risk, while `numeric_id` leaves the column at the
    /// `RecordIdentifier` Medium. `user id` and `customer id` were listed under both,
    /// so a column of customer keys reported as financial data. Keeping the texts
    /// disjoint is what stops that recurring.
    #[test]
    fn financial_and_plain_identifier_terms_do_not_overlap() {
        let texts_for = |kind: &str| {
            taxonomy_terms()
                .iter()
                .filter(|prepared| prepared.term.kind == kind)
                .map(|prepared| prepared.term.text.as_str())
                .collect::<HashSet<_>>()
        };

        let overlap: Vec<&str> = texts_for("account_number")
            .intersection(&texts_for("numeric_id"))
            .copied()
            .collect();

        assert!(
            overlap.is_empty(),
            "these terms claim to be both a financial account and a plain record \
             identifier, which are different privacy risks: {overlap:?}"
        );
    }

    /// A typo is never strong evidence.
    ///
    /// `confidence_for_weight` calls anything from 90 up High, and the taxonomy has
    /// plenty of terms above that — but a Jaro-Winkler near-match is a guess about
    /// what the author meant, so it must not reach the tier that says "certain".
    /// `fuzzy_weight` enforces that by construction; this checks the arithmetic
    /// actually lands below the threshold for every term that can fuzzy-match at all.
    #[test]
    fn fuzzy_matches_never_reach_high_confidence() {
        let fuzzy_eligible: Vec<u8> = taxonomy_terms()
            .iter()
            .map(|prepared| prepared.term.weight)
            .filter(|weight| *weight >= 88)
            .collect();

        assert!(
            !fuzzy_eligible.is_empty(),
            "no term can fuzzy-match, so this test is checking nothing"
        );

        for weight in fuzzy_eligible {
            assert_ne!(
                confidence_for_weight(fuzzy_weight(weight)),
                Confidence::High,
                "a fuzzy match on a weight-{weight} term would report High confidence"
            );
        }
    }

    #[test]
    fn taxonomy_terms_are_well_formed_and_unique() {
        let mut seen = HashSet::new();

        for term in taxonomy_terms().iter().map(|prepared| &prepared.term) {
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
                        | DataType::Uuid
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
