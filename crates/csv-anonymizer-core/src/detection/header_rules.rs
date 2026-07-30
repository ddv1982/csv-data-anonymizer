use std::collections::{HashMap, HashSet};

use crate::types::{Confidence, DataType, DetectionResult};

use super::header;
use super::locale::LocaleContext;
use super::postal::postal_match_country;
use super::scoring::{calculate_confidence, detection_result, trace_item};
use super::validators::{
    is_dutch_btw_tax_number, is_phone_value, is_tax_id, is_unformatted_tax_id, is_us_ein,
    is_us_ssn, is_vat_id,
};
use super::value::is_unsigned_integer;

pub(in crate::detection) type HeaderDetector =
    fn(&header::HeaderAnalysis, &[&String], usize, &LocaleContext) -> Option<HeaderDetection>;

/// A header rule's verdict on a column: the type it claims, the evidence behind the
/// claim, and the taxonomy signal that let the rule run at all.
///
/// Carries a [`CorroboratedConfidence`] rather than a [`Confidence`] because every rule
/// in this module declines instead of returning a Low-confidence detection, and that is
/// what [`first_header_detection`] reports as `accepted`. Spelling the filter into the
/// type keeps the two from drifting apart.
pub(in crate::detection) struct HeaderDetection {
    data_type: DataType,
    confidence: CorroboratedConfidence,
    sample_matches: usize,
    total_samples: usize,
    signal: header::HeaderSignal,
}

/// A confidence that has already cleared the Low filter every header rule applies.
///
/// [`Confidence::Low`] is not representable here, and the type is what enforces that: a
/// header rule fires only when enough sampled values corroborate the header, so a Low
/// ratio means "no detection" rather than "a weak detection". The filter lives in
/// [`Self::from_match_ratio`] and nowhere else, which is why the trace item's `accepted`
/// flag is the constant [`Self::ACCEPTED`]. A rule that wants to report a *rejected*
/// header detection cannot express it in this type and has to change both, together.
#[derive(Clone, Copy)]
enum CorroboratedConfidence {
    Medium,
    High,
}

impl CorroboratedConfidence {
    /// What a header-rule trace item's `accepted` flag is.
    ///
    /// Constant, because a [`HeaderDetection`] exists only where
    /// [`Self::from_match_ratio`] returned `Some`. Named rather than inlined as `true` so
    /// the trace item points a reader at the type that guarantees it.
    const ACCEPTED: bool = true;

    /// The single place the Low filter lives: `None` when the match ratio does not
    /// clear it, so the caller's `?` turns a weak column into no detection.
    ///
    /// The `match` is wildcard-free on purpose — the enumeration idiom used elsewhere in
    /// this crate — so adding a `Confidence` tier is a compile error here rather than a
    /// silent decision about which side of the filter the new tier falls on.
    fn from_match_ratio(match_count: usize, total_non_empty: usize) -> Option<Self> {
        match calculate_confidence(match_count, total_non_empty) {
            Confidence::Low => None,
            Confidence::Medium => Some(Self::Medium),
            Confidence::High => Some(Self::High),
        }
    }

    fn confidence(self) -> Confidence {
        match self {
            Self::Medium => Confidence::Medium,
            Self::High => Confidence::High,
        }
    }
}

#[derive(Clone, Copy)]
pub(in crate::detection) struct HeaderDetectionRule {
    pub detect: HeaderDetector,
    pub selected_reason: &'static str,
    pub trace_reason: &'static str,
}

pub(in crate::detection) fn early_header_detection_rules() -> [HeaderDetectionRule; 4] {
    [
        HeaderDetectionRule {
            detect: detect_header_phone,
            selected_reason: "Header terms and sample shape matched phone detection.",
            trace_reason: "header phone rule",
        },
        HeaderDetectionRule {
            detect: detect_header_postal_code,
            selected_reason: "Header terms and sample shape matched postal code detection.",
            trace_reason: "header postal code rule",
        },
        HeaderDetectionRule {
            detect: detect_header_address,
            selected_reason: "Header terms and sample shape matched address detection.",
            trace_reason: "header address rule",
        },
        HeaderDetectionRule {
            detect: detect_header_tax_id,
            selected_reason: "Header terms and sample shape matched tax ID detection.",
            trace_reason: "header tax ID rule",
        },
    ]
}

pub(in crate::detection) fn first_header_detection(
    header: &header::HeaderAnalysis,
    non_empty_values: &[&String],
    total_samples: usize,
    total_non_empty: usize,
    rules: &[HeaderDetectionRule],
    locale: &LocaleContext,
) -> Option<DetectionResult> {
    // The first matching rule wins outright: header rules are not scored against
    // each other, so this reports the match directly as a trace item rather than
    // building a candidate for a selection that never happens.
    //
    // The trace item is always accepted, and [`CorroboratedConfidence`] is why: a rule
    // that does not clear Low returns `None` and never reaches here, so there is no
    // rejected header-rule trace item to report. Unlike `DetectorCandidate`, whose
    // rejected entries are the point of its trace, this trace has one entry — the winner.
    rules.iter().find_map(|rule| {
        (rule.detect)(header, non_empty_values, total_samples, locale).map(|detection| {
            let confidence = detection.confidence.confidence();
            detection_result(
                detection.data_type,
                confidence,
                detection.sample_matches,
                detection.total_samples,
                total_non_empty,
                format!("{} {}", detection.signal.reason, rule.selected_reason),
                vec![trace_item(
                    detection.data_type,
                    format!(
                        "{}: {} ({:?}, {:?} confidence)",
                        rule.trace_reason,
                        detection.signal.concept,
                        detection.signal.data_type,
                        detection.signal.confidence
                    ),
                    detection.sample_matches,
                    total_non_empty,
                    confidence,
                    CorroboratedConfidence::ACCEPTED,
                )],
            )
        })
    })
}

/// The shape every fixed-type header rule shares.
///
/// A taxonomy signal for `kinds` gates the rule — no signal, no detection. A
/// value predicate then decides how many sampled values corroborate the header,
/// and the column is classified as `data_type` only if that ratio clears Low.
/// Header evidence boosts and disambiguates; it never classifies on its own.
///
/// `count_matches` receives the parsed header terms because some rules widen
/// their predicate based on further header context (Dutch BTW, US tax IDs).
fn header_signal_detection(
    header: &header::HeaderAnalysis,
    non_empty_values: &[&String],
    total_samples: usize,
    kinds: &[&str],
    data_type: DataType,
    count_matches: impl Fn(&header::HeaderTerms, &[&String]) -> usize,
) -> Option<HeaderDetection> {
    let signal = header.best_for_kinds(kinds)?;

    let match_count = count_matches(header.terms(), non_empty_values);
    let confidence = CorroboratedConfidence::from_match_ratio(match_count, non_empty_values.len())?;

    Some(HeaderDetection {
        data_type,
        confidence,
        sample_matches: match_count,
        total_samples,
        signal,
    })
}

fn count_matching<'a>(values: &[&'a String], predicate: impl Fn(&'a str) -> bool) -> usize {
    values.iter().filter(|value| predicate(value)).count()
}

pub(in crate::detection) fn detect_header_numeric_id(
    header: &header::HeaderAnalysis,
    non_empty_values: &[&String],
    total_samples: usize,
    _locale: &LocaleContext,
) -> Option<HeaderDetection> {
    header_signal_detection(
        header,
        non_empty_values,
        total_samples,
        &["numeric_id", "account_number"],
        DataType::NumericId,
        |_, values| count_matching(values, is_unsigned_integer),
    )
}

/// The same question [`detect_header_numeric_id`] asks, for a column whose keys are
/// not written in digits.
///
/// That rule corroborates an identifier header with [`is_unsigned_integer`], so on its
/// own it would leave whether an `*_id` column is called an identifier turning on the
/// alphabet its keys happen to use: `employee_id` holding `1000…` classified and
/// auto-selected, the same header holding `E1000…` left as `String`/Low. An unselected
/// primary key such as `PT-4471` joins a released file straight back to the source
/// system, so the alphabet must not decide it.
///
/// Deliberately *not* a widening of [`detect_header_numeric_id`]: this rule requires a
/// letter in the value, so the two are disjoint and no column that rule classifies can
/// be re-classified here. The only difference between them is the alphabet.
///
/// Why the `opaque_identifier` kind is scoped the way it is. Its terms are the
/// identifier words themselves — `id`, `identifier`, `uid`, `ref`, `reference` and
/// their translations — matched as *tokens*, so `patient_id`, `caseRef` and
/// `identificador` open the gate and `valid`, `humid` and `grid` do not: a token match
/// needs a separator or a camel-case boundary, which is exactly what distinguishes a
/// compound header from a word that happens to end in those letters. Three families
/// that read as identifiers were left out on purpose, each because its false positives
/// are common and expensive:
///
/// - `*_code`, because `status_code`, `reason_code`, `currency_code` and
///   `product_code` dominate the population. The cardinality gate below rejects the
///   first three, but nothing rejects a SKU column, and destroying a product key buys
///   no privacy. A bare `code` header is still reachable through the `numeric_id` kind.
/// - `*_key`, because `api_key` and `session_key` are secrets, already classified High
///   by the `secret` branch of the privacy analysis, and `sort_key`/`partition_key` are
///   structural rather than referential.
/// - `*_no` and `*_number`, because `no` is an ordinary English word, `phone_number`
///   and `house_number` belong to rules that run earlier, and the entity-qualified
///   forms that matter (`customer number`, `order number`, …) are already enumerated
///   under `numeric_id`.
///
/// `numeric_id` is consulted alongside the new kind so that the entity-qualified and
/// non-English terms already enumerated there — `klantnummer`, `kundennummer`,
/// `numero client` — reach the alphanumeric case too; they are single compact words
/// that no token rule could match.
pub(in crate::detection) fn detect_header_opaque_identifier(
    header: &header::HeaderAnalysis,
    non_empty_values: &[&String],
    total_samples: usize,
    _locale: &LocaleContext,
) -> Option<HeaderDetection> {
    // Asked before the header, because it is the expensive half of the judgement and
    // the half that can decline for a reason the ratio below cannot express: a column
    // can be 100% key-shaped values and still be a category.
    if !column_values_look_like_keys(non_empty_values) {
        return None;
    }

    header_signal_detection(
        header,
        non_empty_values,
        total_samples,
        &["opaque_identifier", "numeric_id"],
        DataType::NumericId,
        |_, values| count_matching(values, is_alphanumeric_key_token),
    )
}

/// Punctuation an exported key may carry. Deliberately short: `.` is excluded because
/// it makes `1.2.3` and `report.pdf` keys, and everything outside this set implies a
/// value with internal structure that some other rule owns — `@` an address, `:` a
/// URL, whitespace a phrase.
const KEY_TOKEN_SEPARATORS: [char; 3] = ['-', '_', '/'];

/// Whether one value is shaped like a key an integer rule cannot read.
///
/// Both the letter and the digit are required, and they do different jobs. The digit is
/// what separates a key from a word, so `ACTIVE`, `PENDING` and `EUR` are rejected on
/// their own shape rather than left to the cardinality gate. The letter is what keeps
/// this rule disjoint from [`detect_header_numeric_id`]: a column of bare integers
/// cannot match here, so nothing that rule already classifies can be re-classified by
/// this one.
///
/// The length floor of three is what keeps `house_number` values (`12`, `14b`) out.
fn is_alphanumeric_key_token(value: &str) -> bool {
    let trimmed = value.trim();
    (3..=64).contains(&trimmed.len())
        && trimmed.chars().all(|character| {
            character.is_ascii_alphanumeric() || KEY_TOKEN_SEPARATORS.contains(&character)
        })
        && trimmed.chars().any(|character| character.is_ascii_digit())
        && trimmed
            .chars()
            .any(|character| character.is_ascii_alphabetic())
}

/// The part of a key's shape that a column is expected to hold constant.
///
/// Length and separator presence only, because [`is_alphanumeric_key_token`] has
/// already pinned the rest of the character-class set — every counted value has at
/// least one letter and at least one digit. So `CUST-0042` and `CUST-0043` share a
/// profile, `E1000` and `a1b2c3d4` each share one with their own column, and a column
/// mixing `AB-12`, `X9`, and `ORDER-2024-000117` shares none.
type KeyTokenProfile = (usize, bool);

fn key_token_profile(value: &str) -> Option<KeyTokenProfile> {
    if !is_alphanumeric_key_token(value) {
        return None;
    }
    let trimmed = value.trim();
    Some((
        trimmed.len(),
        trimmed
            .chars()
            .any(|character| KEY_TOKEN_SEPARATORS.contains(&character)),
    ))
}

/// Share of a column's values that must agree, both on shape and on being distinct.
///
/// Three quarters, the same share `privacy`'s name gate requires, and for the same
/// reason: a real key column carries the odd `UNKNOWN`, `N/A` or legacy-format row, and
/// one such value must not disqualify it, while a column that is mostly something else
/// cannot reach it.
const MIN_KEY_AGREEMENT_NUMERATOR: usize = 3;
const MIN_KEY_AGREEMENT_DENOMINATOR: usize = 4;

/// Minimum non-empty values before the distinctness test is allowed to decide anything.
///
/// Below this, "mostly distinct" is not evidence — four different values out of four is
/// equally true of a key column and of a five-value status vocabulary seen once each.
/// Production analyses at least 100 rows (`types::DETECTION_SAMPLE_ROW_FLOOR`), so this only
/// binds on hand-written fixtures and on files that really are that short.
const MIN_KEY_VALUES: usize = 8;

/// Whether the column as a whole behaves like a key column, as opposed to a category
/// vocabulary that happens to be spelled in an identifier-ish alphabet.
///
/// Two aggregates, because neither alone is enough. Uniformity says the values were
/// minted by one generator rather than written by hand. Distinctness is what separates
/// a key from a label: `status_code` holding five values across five thousand rows
/// passes every per-value shape test this module has and is not an identifier, and the
/// column-level ratio is the only place that fact is visible.
///
/// Two known misses, both in the safe direction for utility and the unsafe one for
/// privacy, and both stated rather than hidden:
///
/// - A low-cardinality *foreign* key — `customer_id` in an orders table with fifty
///   customers — is indistinguishable from a category label by shape alone, and this
///   rejects it. Sampling helps more than it looks: `DETECTION_SAMPLE_CAP` reads at
///   most 200 values, so a key column with hundreds of distinct values still reads as
///   nearly all-distinct.
/// - Unpadded variable-width keys (`CUST-1` through `CUST-1000`) fail the uniformity
///   half. Exported systems zero-pad precisely to avoid this, which is why the trade is
///   worth making, but it is a trade.
fn column_values_look_like_keys(non_empty_values: &[&String]) -> bool {
    if non_empty_values.len() < MIN_KEY_VALUES {
        return false;
    }

    let mut profiles: HashMap<KeyTokenProfile, usize> = HashMap::new();
    for value in non_empty_values {
        if let Some(profile) = key_token_profile(value) {
            *profiles.entry(profile).or_default() += 1;
        }
    }
    let modal_profile_count = profiles.into_values().max().unwrap_or(0);
    if !clears_key_agreement_share(modal_profile_count, non_empty_values.len()) {
        return false;
    }

    // Case-folded, because a key column that spells the same key `AB-1` and `ab-1` is
    // one key written twice, and counting it as two would let a two-value vocabulary
    // masquerade as distinct.
    let distinct: HashSet<String> = non_empty_values
        .iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .collect();
    clears_key_agreement_share(distinct.len(), non_empty_values.len())
}

fn clears_key_agreement_share(count: usize, total: usize) -> bool {
    count * MIN_KEY_AGREEMENT_DENOMINATOR >= total * MIN_KEY_AGREEMENT_NUMERATOR
}

fn detect_header_phone(
    header: &header::HeaderAnalysis,
    non_empty_values: &[&String],
    total_samples: usize,
    _locale: &LocaleContext,
) -> Option<HeaderDetection> {
    header_signal_detection(
        header,
        non_empty_values,
        total_samples,
        &["phone"],
        DataType::Phone,
        |_, values| count_matching(values, is_phone_value),
    )
}

fn detect_header_postal_code(
    header: &header::HeaderAnalysis,
    non_empty_values: &[&String],
    total_samples: usize,
    locale: &LocaleContext,
) -> Option<HeaderDetection> {
    header_signal_detection(
        header,
        non_empty_values,
        total_samples,
        &["postal_code"],
        DataType::PostalCode,
        |_, values| {
            // With a known file locale, header-labeled postal columns are held to
            // the per-country formats: when any sample matches a country format,
            // only those matches count. Without locale context (the default), the
            // loose shape check stands unchanged, preserving columns that mix
            // unambiguous formats with unlabeled bare-digit zips (e.g. GB + US).
            let context_match_count = if locale.countries().is_empty() {
                0
            } else {
                count_matching(values, |value| {
                    postal_match_country(value, locale).is_some()
                })
            };
            if context_match_count > 0 {
                context_match_count
            } else {
                count_matching(values, is_postal_code)
            }
        },
    )
}

fn detect_header_address(
    header: &header::HeaderAnalysis,
    non_empty_values: &[&String],
    total_samples: usize,
    _locale: &LocaleContext,
) -> Option<HeaderDetection> {
    header_signal_detection(
        header,
        non_empty_values,
        total_samples,
        &["address"],
        DataType::Address,
        |_, values| count_matching(values, is_plausible_address),
    )
}

fn detect_header_tax_id(
    header: &header::HeaderAnalysis,
    non_empty_values: &[&String],
    total_samples: usize,
    _locale: &LocaleContext,
) -> Option<HeaderDetection> {
    header_signal_detection(
        header,
        non_empty_values,
        total_samples,
        &["tax_id"],
        DataType::TaxId,
        |header_terms, values| {
            let allow_dutch_btw_number = has_dutch_btw_context(header_terms);
            let tax_id_context = tax_id_header_context(header_terms);
            count_matching(values, |value| {
                is_tax_id(value)
                    || is_contextual_unformatted_us_tax_id(value, tax_id_context)
                    || is_vat_id(value)
                    || (allow_dutch_btw_number && is_dutch_btw_tax_number(value))
            })
        },
    )
}

pub(in crate::detection) fn detect_name_type(
    header: &header::HeaderAnalysis,
    non_empty_values: &[&String],
    total_samples: usize,
    _locale: &LocaleContext,
) -> Option<HeaderDetection> {
    let signal =
        header.best_for_kinds(&["first_name", "last_name", "full_name", "generic_name"])?;
    let data_type = signal.data_type;
    let match_count = non_empty_values
        .iter()
        .filter(|value| match data_type {
            DataType::FirstName => is_plausible_name_part(value, 2),
            DataType::LastName => is_plausible_name_part(value, 4),
            DataType::FullName => is_plausible_full_name(value),
            _ => false,
        })
        .count();

    if let Some(confidence) =
        CorroboratedConfidence::from_match_ratio(match_count, non_empty_values.len())
    {
        return Some(HeaderDetection {
            data_type,
            confidence,
            sample_matches: match_count,
            total_samples,
            signal,
        });
    }

    // A generic `name` header whose values are single tokens: `name` asks for a full
    // name, so a column of bare given names fails the two-token predicate outright. The
    // retry reads them as first names, and applies the same Low filter to its own ratio,
    // so a column that corroborates neither predicate still yields no detection.
    if data_type != DataType::FullName || !header.matches_kind("generic_name") {
        return None;
    }

    let match_count = non_empty_values
        .iter()
        .filter(|value| is_plausible_generic_single_name(value))
        .count();
    let confidence = CorroboratedConfidence::from_match_ratio(match_count, non_empty_values.len())?;

    Some(HeaderDetection {
        data_type: DataType::FirstName,
        confidence,
        sample_matches: match_count,
        total_samples,
        signal,
    })
}

fn is_postal_code(value: &str) -> bool {
    let trimmed = value.trim();
    (3..=12).contains(&trimmed.len())
        && trimmed.chars().any(|character| character.is_ascii_digit())
        && trimmed
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, ' ' | '-'))
}

pub(in crate::detection) fn is_plausible_address(value: &str) -> bool {
    let trimmed = value.trim();
    if !(5..=200).contains(&trimmed.len())
        || !trimmed.chars().any(|character| character.is_ascii_digit())
        || !trimmed.chars().any(|character| character.is_alphabetic())
    {
        return false;
    }

    let normalized = trimmed.to_lowercase();
    if contains_address_keyword(&normalized) {
        return true;
    }

    if trimmed
        .chars()
        .any(|character| character.is_alphabetic() && !character.is_ascii())
        && trimmed.contains('-')
    {
        return true;
    }

    trimmed.contains(',') || trimmed.matches(char::is_whitespace).count() >= 2
}

pub(in crate::detection) fn has_dutch_btw_context(terms: &header::HeaderTerms) -> bool {
    matches!(
        terms.compact.as_str(),
        "btw" | "btwnr" | "btwnummer" | "btwid" | "btwidentificatienummer" | "omzetbelastingnummer"
    ) || terms.compact.ends_with("btwnummer")
        || terms.compact.ends_with("omzetbelastingnummer")
}

#[derive(Clone, Copy)]
pub(in crate::detection) enum TaxIdHeaderContext {
    Generic,
    Ssn,
    Ein,
}

pub(in crate::detection) fn tax_id_header_context(
    terms: &header::HeaderTerms,
) -> TaxIdHeaderContext {
    match terms.compact.as_str() {
        "ssn" | "socialsecuritynumber" => TaxIdHeaderContext::Ssn,
        "ein" | "employeridentificationnumber" => TaxIdHeaderContext::Ein,
        _ => TaxIdHeaderContext::Generic,
    }
}

pub(in crate::detection) fn is_contextual_unformatted_us_tax_id(
    value: &str,
    context: TaxIdHeaderContext,
) -> bool {
    match context {
        TaxIdHeaderContext::Ssn => is_us_ssn(value),
        TaxIdHeaderContext::Ein => is_us_ein(value),
        TaxIdHeaderContext::Generic => is_unformatted_tax_id(value),
    }
}

pub(in crate::detection) fn address_keywords() -> &'static [&'static str] {
    &[
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
        "straat",
        "weg",
        "laan",
        "plein",
        "strasse",
        "straße",
        "platz",
        "allee",
        "rue",
        "avenue",
        "boulevard",
        "calle",
        "avenida",
        "carrera",
        "rua",
        "travessa",
        "via",
        "viale",
        "piazza",
    ]
}

pub(in crate::detection) fn contains_address_keyword(normalized: &str) -> bool {
    normalized
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .any(|token| {
            address_keywords().iter().any(|keyword| {
                let keyword = keyword.trim();
                token == keyword
                    || compound_address_suffixes().contains(&keyword)
                        && token.len() > keyword.len()
                        && token.ends_with(keyword)
            })
        })
}

fn compound_address_suffixes() -> &'static [&'static str] {
    &[
        "straat", "weg", "laan", "plein", "strasse", "straße", "platz", "allee",
    ]
}

/// Words that mark a value as the name of an organisation rather than a person.
///
/// Negative evidence, because it is the only kind available. The name gazetteer was
/// withdrawn on data-minimization grounds, so there is no list of real names to match
/// against, and `Acme Corporation` is structurally identical to `Grace Hopper` — two
/// capitalised alphabetic tokens. What separates them is not the shape but a small
/// closed vocabulary of words people are not called. That list carries no personal
/// data, which is exactly why it can exist where a gazetteer of names could not.
///
/// Every entry is a word that is common in organisation, team and department names and
/// vanishingly rare as a person's name. **Place words are deliberately absent**, and
/// that omission is the important part: English surnames are overwhelmingly toponymic,
/// so `Park`, `Hill`, `Ford`, `Brooks`, `Stone`, `Wood`, `Banks`, `West` and `North`
/// are all real surnames. A place vocabulary would reject people, which is the
/// dangerous direction. `New York` and `San Francisco` therefore still read as
/// plausible names here, and separating them from `Grace Hopper` needs evidence this
/// module does not have.
///
/// Applied per value and aggregated by ratio through `calculate_confidence`, which is
/// what makes rejecting on a single token safe: a genuine person surnamed `Church` in a
/// column of ordinary names is one rejected value among many and cannot pull the
/// column's confidence down, while a column of company names has nearly every value
/// rejected and declines as it should.
const NON_PERSON_NAME_TOKENS: &[&str] = &[
    // Legal forms. Two-letter abbreviations (bv, nv, ag, sa, ab, as) are omitted
    // because they collide with initials and particles.
    "ltd",
    "limited",
    "inc",
    "incorporated",
    "llc",
    "llp",
    "plc",
    "corp",
    "corporation",
    "company",
    "gmbh",
    "sarl",
    "srl",
    "holding",
    "holdings",
    // Organisation words.
    "group",
    "industries",
    "enterprises",
    "ventures",
    "partners",
    "associates",
    "solutions",
    "systems",
    "technologies",
    "laboratories",
    "foundation",
    "institute",
    "university",
    "hospital",
    "clinic",
    "agency",
    "bureau",
    "council",
    "committee",
    "association",
    "society",
    "federation",
    // Team, department and project words.
    "department",
    "division",
    "team",
    "office",
    "engineering",
    "operations",
    "marketing",
    "sales",
    "finance",
    "procurement",
    "logistics",
    "support",
    "success",
    "reliability",
    "security",
    "infrastructure",
    "platform",
    "analytics",
    "science",
    "research",
    "development",
    // Commercial-entity words. Occupational and toponymic surnames are excluded for
    // the same reason place words are: `Steel`, `Mills`, `Banks`, `Fields`, `Brooks`,
    // `Baker`, `Miller`, `Turner` and `Carter` all name people.
    "manufacturing",
    "traders",
    "trading",
    "retail",
    "wholesale",
    "distribution",
    "media",
    "communications",
    "consulting",
    "consultancy",
    "capital",
    "insurance",
    "pharmaceuticals",
    "energy",
    "utilities",
    "telecom",
    "networks",
    "studios",
    "supply",
    "supplies",
    "equipment",
    "materials",
    "products",
    "brands",
    "textiles",
    "chemicals",
    "mining",
    "shipping",
    "freight",
    "rentals",
    "leasing",
    "properties",
    "realty",
    "estates",
    "contractors",
    "construction",
    "airlines",
    "motors",
    "foods",
    "toys",
];

/// Whether any of `value`'s tokens is in [`NON_PERSON_NAME_TOKENS`].
///
/// Compared without case, because the vocabulary describes the word rather than its
/// presentation: `LIMITED`, `Limited` and `limited` are the same evidence.
fn has_non_person_name_token(value: &str) -> bool {
    value.split_whitespace().any(|token| {
        let word = token.trim_matches(|character: char| !character.is_alphanumeric());
        NON_PERSON_NAME_TOKENS
            .iter()
            .any(|entry| word.eq_ignore_ascii_case(entry))
    })
}

/// Whether a token looks like a filename rather than a name part.
///
/// [`is_plausible_name_token`] allows `.` so that initials (`J.`) and abbreviations
/// (`Jr.`) survive, and that allowance is what let `report final.pdf` read as a
/// two-token person name. An initial is a single letter before the dot; anything with
/// several letters on both sides of it is a file, a domain, or a version.
fn looks_like_filename_token(token: &str) -> bool {
    let Some((stem, extension)) = token.rsplit_once('.') else {
        return false;
    };
    stem.chars()
        .filter(|character| character.is_alphanumeric())
        .count()
        > 1
        && (2..=5).contains(&extension.len())
        && extension.chars().all(|character| character.is_alphabetic())
}

fn is_plausible_name_part(value: &str, max_tokens: usize) -> bool {
    let trimmed = value.trim();
    if !(2..=80).contains(&trimmed.len()) {
        return false;
    }
    if has_non_person_name_token(trimmed) {
        return false;
    }

    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    !tokens.is_empty()
        && tokens.len() <= max_tokens
        && tokens.iter().all(|token| is_plausible_name_token(token))
        && !tokens.iter().any(|token| looks_like_filename_token(token))
}

pub(in crate::detection) fn is_plausible_full_name(value: &str) -> bool {
    let trimmed = value.trim();
    if !(5..=120).contains(&trimmed.len()) {
        return false;
    }
    if has_non_person_name_token(trimmed) {
        return false;
    }

    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    (2..=6).contains(&tokens.len())
        && tokens.iter().all(|token| is_plausible_name_token(token))
        && !tokens.iter().any(|token| looks_like_filename_token(token))
}

fn is_plausible_generic_single_name(value: &str) -> bool {
    let trimmed = value.trim();
    is_plausible_name_part(trimmed, 1)
        && trimmed
            .chars()
            .next()
            .is_some_and(|character| character.is_uppercase())
}

fn is_plausible_name_token(token: &str) -> bool {
    let mut chars = token.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_alphabetic()
        && chars.all(|character| character.is_alphabetic() || matches!(character, '\'' | '-' | '.'))
}
