//! A held-out corpus for measuring detection quality.
//!
//! `multilingual_matrix.rs` is a regression guard: every fixture in it was
//! chosen because the detector handles it, and it asserts precision and recall
//! of exactly 1.0. That makes it useless as a measuring instrument — a suite
//! with no headroom cannot tell you whether a change helped, and in particular
//! cannot answer whether a Local AI assist would add anything.
//!
//! This corpus is the opposite. The cases were written to look like real files
//! the detector was never tuned against: cryptic and abbreviated headers,
//! languages outside the taxonomy, messy value formatting, plainly-worded name
//! headers the taxonomy does not enumerate, and benign look-alikes — including
//! `<word> name` columns that hold no person at all, which are the direct cost of
//! reading `name` in a header as evidence. The detector is *expected to fail some
//! of them*, and the recorded [`baseline`] documents exactly how many. The tests
//! fail on regression, never on imperfection.
//!
//! Working with this file:
//!
//! - Improving the detector makes a score exceed its baseline. The test says so
//!   and tells you to raise the number. Do that in the same commit.
//! - Never "fix" a failure by editing the expectation to match what the
//!   detector currently returns. That converts the instrument back into a
//!   mirror, which is the problem this file exists to solve.
//! - Print the full report any time with:
//!   `cargo test -p csv-anonymizer-core held_out -- --nocapture`

use super::*;
use crate::types::ColumnMetadata;
use std::collections::BTreeMap;

/// One held-out column: a header, its sampled values, and what a careful human
/// reviewer would call it.
struct Case {
    /// Why this case is hard, and what it is probing.
    challenge: Challenge,
    header: &'static str,
    values: &'static [&'static str],
    /// The classification a careful reviewer would give this column.
    /// `DataType::String` means "no more specific type applies" — the correct
    /// answer for benign look-alikes that must not be flagged.
    expected: DataType,
    /// A detector that must appear in the column's privacy evidence.
    ///
    /// Needed where the type vocabulary has no name for what the column holds, so
    /// `expected` has to be `String` and the type axis cannot tell success from
    /// "nothing matched". Naming the detector makes the point earned instead of
    /// free. `None` wherever `expected` is discriminating on its own.
    expected_detector: Option<&'static str>,
    /// The lowest privacy risk this column may be assigned. Under-classifying
    /// risk is the dangerous direction: it is what keeps a column from being
    /// auto-selected, so it is scored separately from exact-type accuracy.
    expected_risk_floor: PiiRisk,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Challenge {
    /// Header is abbreviated, machine-generated, or absent.
    CrypticHeader,
    /// Header language is outside the eight-language taxonomy.
    UntaxonomizedLanguage,
    /// Values are a real instance of the type in an awkward format.
    MessyFormatting,
    /// A plainly-worded person-name column whose header the name gate does not
    /// open for.
    ///
    /// Distinct from [`Challenge::CrypticHeader`]: nothing about `agent_name` is
    /// abbreviated, machine-generated, or foreign. The header reads exactly like
    /// what it holds, and the column is still missed, because name detection is
    /// header-gated and the taxonomy enumerates `<word> name` compounds one at a
    /// time (`full name`, `contact name`, `customer name`, …) with the bare
    /// `name` term restricted to `matchMode: "exact"`. Anything outside that list
    /// falls through. Kept apart from `cryptic-header` so the report distinguishes
    /// "the header carries no information" from "the header carries the
    /// information and the taxonomy has no entry for it" — the second is a
    /// coverage gap with a known fix, the first is not.
    HeaderGatedName,
    /// An identifier column whose keys are not written in digits.
    ///
    /// Kept apart from [`Challenge::CrypticHeader`] because nothing here is cryptic:
    /// the header says `id` and the values are plainly keys. What made these hard is
    /// that identifier detection used to corroborate the header with an
    /// integer test, so `employee_id` holding `1000…` was `NumericId` and the same
    /// header holding `E1000…` was `String` — the alphabet the source system minted
    /// its keys in decided whether the column was offered for anonymization at all.
    /// Its own category so the report can show which alphabets a change reaches;
    /// the padded and unpadded cases below are on opposite sides of that line.
    NonNumericKey,
    /// Values resemble a PII type but are not one. Must not be flagged.
    BenignLookAlike,
    /// The column is only partly populated with the type.
    MixedColumn,
}

impl Challenge {
    fn label(self) -> &'static str {
        match self {
            Challenge::CrypticHeader => "cryptic-header",
            Challenge::UntaxonomizedLanguage => "untaxonomized-language",
            Challenge::MessyFormatting => "messy-formatting",
            Challenge::HeaderGatedName => "header-gated-name",
            Challenge::NonNumericKey => "non-numeric-key",
            Challenge::BenignLookAlike => "benign-look-alike",
            Challenge::MixedColumn => "mixed-column",
        }
    }
}

/// Detection quality as measured on this corpus, recorded from an actual run.
///
/// These are floors, not targets. A score below its floor is a regression and
/// fails the build; a score above it means the detector improved and the floor
/// should be raised.
///
/// With one exception, and it is why [`check`](super::check) reports a rise as "moved"
/// rather than "improved": [`BENIGN_SURFACED`] is a recorded *cost*, not a floor. A rise
/// there means the review warning fires on more benign columns than it used to, which is
/// the direction that makes it stop being read. It is pinned in both directions all the
/// same, so that the cost cannot drift either way unremarked.
mod baseline {
    /// Cases whose exact `DataType` the detector gets right. Recorded at 33/51.
    ///
    /// This started at 25/44. Two of the cases it gained are `client_name` and
    /// `legal_name`, which held organization names and were typed `FullName`;
    /// `NON_PERSON_NAME_TOKENS` in `detection::header_rules` closed both, and they are
    /// the same two cases that took [`BENIGN_RISK_EXACT`] from 12 to 14.
    ///
    /// The other four, and the seven cases the population grew by, arrived together
    /// with `detection::header_rules::detect_header_opaque_identifier`. The corpus had
    /// no alphanumeric identifier case at all — its only key column was `seq`, holding
    /// `1000001…`, the one shape the integer-corroborated header rule could already
    /// see — so the gap where an `*_id` column's alphabet decided whether it was
    /// offered for anonymization was invisible here. Three `non-numeric-key` cases and
    /// three benign identifier look-alikes now pass; the fourth `non-numeric-key` case
    /// is the recorded miss listed below.
    ///
    /// The eighteen misses, by category — counts given so the breakdown can be checked
    /// against the total:
    ///
    /// - **Person names, eleven cases.** Name detection is header-gated since the
    ///   gazetteer was withdrawn, so with no value-level name evidence to vote
    ///   with, the header is the only way in. Missed behind a cryptic header
    ///   (`c_nm`), an absent header, an untaxonomized-language header
    ///   (`imię_i_nazwisko`), and all eight `header-gated-name` cases, whose
    ///   headers name their contents in plain English (`agent_name`,
    ///   `patient_name`, …) but are not among the `<word> name` compounds the
    ///   taxonomy enumerates. Diacritics, lowercase values, and low cardinality
    ///   are each represented once inside that eight so a future widening of the
    ///   gate can be shown to reach them; today they fail on the header alone.
    /// - **Postal codes, one case.** In an untaxonomized language, read as `NumericId`.
    /// - **An email wrapped in a display name, one case.**
    ///   `Ada Lovelace <ada@example.com>`, read as `String`.
    /// - **European decimal-comma numbers, two cases.** `omzet` holding `1.234,56` and
    ///   `bedrag` holding `€ 1.234,56`, both read as `String`.
    /// - **IPv6, one case.**
    /// - **An unpadded alphanumeric key, one case.** `order_ref` holding `REF-7`
    ///   through `REF-104213`, read as `String`. The identifier rule asks a column's
    ///   values to agree on a length, which is what tells a minted key from a
    ///   hand-written label, and keys counted up without padding do not.
    /// - **One benign look-alike over-flagged**, listed under
    ///   [`BENIGN_RISK_EXACT`].
    ///
    /// Eleven, one, one, two, one, one and one: eighteen, against 51 cases and a score
    /// of 33.
    ///
    /// Not measured here: whether any of these would be caught with a name
    /// gazetteer or a Local AI assist. The corpus records what the shipped
    /// header-and-shape pipeline does, nothing about what an alternative would do.
    pub(super) const EXACT_TYPE_CORRECT: usize = 33;
    /// Cases assigned at least the privacy risk they warrant. This is the
    /// number that matters for anonymization: it bounds how many columns would
    /// be offered for transformation. Recorded at 38/51.
    ///
    /// Eleven of the thirteen shortfalls are unreachable person-name columns — the
    /// two cryptic ones, the untaxonomized-language one, and all eight
    /// `header-gated-name` cases. Each is a column of full names the app would
    /// leave unselected. The twelfth is IPv6 and the thirteenth is `order_ref`, the
    /// unpadded key. Eight of those eleven are nonetheless
    /// surfaced for review, which [`REVIEW_COVERAGE`] counts and this number
    /// deliberately does not: the review tier grants no risk, so under this axis they are
    /// still columns the app would not offer to anonymize. Every other case clears its floor,
    /// including the ones whose exact type is wrong: a mistyped column can still
    /// be escalated correctly, which is why these two axes are scored separately.
    pub(super) const RISK_FLOOR_MET: usize = 38;
    /// Cases the app would put in front of the user by *some* route, whether or not it
    /// typed them correctly. Recorded at 27/32.
    ///
    /// Scored over the 32 cases whose `expected_risk_floor` is above Low — the ones that
    /// ought to reach a reviewer at all — and counted through the production predicates:
    /// either `metadata::should_auto_select_column` on a real `ColumnMetadata` built for
    /// the column, or `service::possible_person_name_warning_for_column`, the function that
    /// raises the preview review warning, asked directly. See
    /// [`super::surfaced_for_review`].
    ///
    /// Why a fourth axis rather than a wider reading of [`RISK_FLOOR_MET`]. The
    /// review tier deliberately grants no risk: a column whose header ends in a name term
    /// and whose values are shaped like names is recorded at Low, left unselected and not
    /// redacted, because the same evidence is true of `city_name` holding `New York`.
    /// [`RISK_FLOOR_MET`] therefore still scores all eight `header-gated-name` cases as
    /// misses, correctly — being warned about is not being protected. Without this axis
    /// the corpus reported the arrival of the review tier as exactly zero improvement:
    /// remove the second route and the score here is 16/28, which is what the shipped
    /// product scored before that tier existed.
    ///
    /// The five misses are the columns a user accepting the defaults would never see
    /// mentioned anywhere:
    ///
    /// - **Three unreachable person-name columns**, `c_nm`, the header-less one, and
    ///   `imię_i_nazwisko`. The review tier is header-gated too — it needs a header
    ///   *ending* in a name term — so an abbreviated, absent, or untaxonomized-language
    ///   header opens neither route. These three are the residue of the person-name gap
    ///   that the review tier does not touch, and the reason it is worth keeping them
    ///   apart from `header-gated-name` in [`super::Challenge`].
    /// - **IPv6**, `server_addr`, typed `String`/Low: below the auto-select bar, and no
    ///   name term in its header to reach the other route.
    /// - **The unpadded key**, `order_ref`. Its header opens the identifier gate and its
    ///   values close it again, so it is typed `String`/Low and neither route reaches
    ///   it. This is the miss to watch if the uniformity half of
    ///   `column_values_look_like_keys` is ever relaxed.
    ///
    /// Not measured here. That surfacing is *adequate*: a warning on an unselected column
    /// is weaker than auto-selection with a redacting default, and this axis counts them
    /// alike. Nor that a user acts on it. And deliberately not scored in the reverse
    /// direction: a benign look-alike that *is* surfaced is not counted against anything.
    /// Surfacing `city_name` for review is the chosen design, not a defect — see
    /// `service::tests::possible_names::a_place_name_column_is_surfaced_because_nothing_here_can_rule_it_out`
    /// and the reasoning in `possible_person_name_warning_for_column` — so requiring
    /// benign columns to stay unsurfaced would encode the opposite of the decision this
    /// codebase made, and the next widening of the review tier would show up here as a
    /// regression while being an improvement. The direction that costs real utility, risk
    /// escalation on a benign column, is still guarded, by [`BENIGN_RISK_EXACT`].
    pub(super) const REVIEW_COVERAGE: usize = 27;
    /// Benign look-alikes whose risk landed exactly where it should — neither
    /// under-flagged (a leak) nor over-flagged (lost utility). Recorded at 17/18.
    ///
    /// The one shortfall is an over-flag, the safe direction for privacy and the
    /// expensive one for utility: `measurement`, holding dotted quads in a column
    /// plainly not about networking, still becomes `IpAddress`/Medium.
    ///
    /// The population grew from 15 to 18 with the identifier rule, and those three are
    /// the price side of it rather than more of the same evidence: `status_code`, a
    /// vocabulary of five codes; `region_ref`, a foreign key into a four-row dimension
    /// table under a header that *is* in the identifier family; and `booking_ref`, free
    /// text under one. Each fails a different one of the rule's gates — the header
    /// family, the distinctness ratio, the per-value shape — so this number is what
    /// notices if any one of the three is widened on its own.
    ///
    /// This started at 12/15. `client_name` and `legal_name`, both holding
    /// organization names, were false positives reachable in the shipped product —
    /// both terms are enumerated as `full_name`, so the header gate was already open
    /// for them, and value confirmation accepted any two or three capitalized
    /// alphabetic tokens. `NON_PERSON_NAME_TOKENS` in `detection::header_rules` now
    /// rejects values carrying an organization word, which closed both.
    ///
    /// Six of the remaining `<word> name` columns — `company_name`, `city_name`,
    /// `team_name`, `department_name`, `project_name`, `event_name` — are exact for
    /// two different reasons that this number cannot distinguish, and the difference
    /// matters. Two of them, `company_name` and `team_name`, carry an organization word in
    /// enough values to fail the three-quarters name-shaped share test — four of four and
    /// three of four respectively — and would now be rejected on their values whatever the
    /// header said. The other four would not: `Legal Affairs`,
    /// `Northern Lights`, `Annual Summit` and `New York` contain no word in that
    /// vocabulary, and place words are deliberately absent from it because English
    /// surnames are largely toponymic. `is_plausible_full_name` accepts all four columns,
    /// so only the shut `full_name` header gate is keeping them correct here. Those are
    /// the four cases to watch when the gate widens — and they are the four this corpus
    /// already observes being surfaced for review, which [`REVIEW_COVERAGE`] records as
    /// the accepted cost of that tier rather than as a failure.
    ///
    /// The split was measured, not read off the vocabulary: each of the four produces
    /// possible-person-name evidence and the other two do not, and the value test behind
    /// that evidence is the same `is_plausible_full_name` the `full_name` path confirms
    /// with. An earlier version of this comment assigned `department_name` and
    /// `project_name` to the rejected group on the strength of their headers reading like
    /// organization units; their values contain no organization word at all.
    pub(super) const BENIGN_RISK_EXACT: usize = 17;
    /// Benign look-alikes that draw the possible-person-name review warning. Recorded at
    /// 5/18 — just over a quarter of the benign corpus.
    ///
    /// This is the price of the review tier, stated as a number instead of a caveat. The
    /// tier fires on a header ending in a name term whose values pass
    /// `is_plausible_full_name`, and `NON_PERSON_NAME_TOKENS` is the only thing that can
    /// call a column off. That vocabulary is applied to *values*, never to the header, and
    /// it deliberately holds no place, product or codename words — English surnames are
    /// largely toponymic and occupational, so `Park`, `Hill`, `Brooks`, `Baker` and
    /// `Turner` all name people, and a vocabulary wide enough to reject `New York` would
    /// reject them too. So the five below are unreachable by it, each for the same reason
    /// from a different direction:
    ///
    /// - `city_name` — `New York`, `Kansas City`. Place words, absent by that choice.
    /// - `department_name` — `Human Resources`, `Legal Affairs`. `department` and
    ///   `division` *are* in the vocabulary, but they are in this column's header, which
    ///   the vocabulary never sees; no value carries an organization word.
    /// - `project_name` — `Northern Lights`, `Silver Falcon`. Codenames are evocative by
    ///   design, which is to say shaped exactly like names.
    /// - `event_name` — `Partner Forum`, `Winter Retreat`. `partners` is enumerated;
    ///   `Partner` is not it, because the comparison is whole-word.
    /// - `product_name` — `Blue Widget`, `Green Sprocket`. `products` is enumerated; the
    ///   words products are actually called are not.
    ///
    /// What this number does *not* assert. Not that the count should be zero, and not that
    /// any named column should stay unsurfaced: surfacing `city_name` is the chosen design
    /// and the reasoning for it is in `possible_person_name_warning_for_column`. It asserts
    /// only that the count is exactly what is recorded, in both directions, so that a
    /// change surfacing more benign columns is a conscious act with a number raised in the
    /// same commit — the discipline every other axis here gets. A rise means the warning is
    /// getting noisier, and noise is how a warning stops being read; at that point the
    /// question to re-open is not whether the tier is correct in principle but whether
    /// users still act on it. Nearly three quarters of the benign corpus staying quiet is
    /// what makes the warning worth showing at all, and that is the ratio this number
    /// protects.
    ///
    /// Two further benign columns *are* surfaced and are deliberately not counted here,
    /// because they arrive by auto-selection rather than by the warning and this number is
    /// documented as the warning's cost: `measurement`, the `IpAddress` over-flag already
    /// recorded under [`BENIGN_RISK_EXACT`], and `seq`, correctly `NumericId`/Medium by
    /// design. Nothing is lost by excluding them — risk escalation on a benign column is
    /// exactly what [`BENIGN_RISK_EXACT`] scores.
    pub(super) const BENIGN_SURFACED: usize = 5;
}

#[test]
fn held_out_corpus_meets_recorded_baseline() {
    let report = score(cases());
    println!("{}", report.render());

    check(
        &report,
        report.exact_type_correct,
        baseline::EXACT_TYPE_CORRECT,
        "exact-type accuracy",
        "",
    );
    check(
        &report,
        report.risk_floor_met,
        baseline::RISK_FLOOR_MET,
        "risk-floor coverage",
        " This is the privacy-relevant number: a column below its risk floor is a \
         column the app would not offer to anonymize.",
    );
    check(
        &report,
        report.review_coverage,
        baseline::REVIEW_COVERAGE,
        "review coverage",
        " A column counted here is one the app would put in front of the user by some \
         route; a column missing from it is one a user accepting the defaults never sees \
         mentioned at all.",
    );
    check(
        &report,
        report.benign_risk_exact,
        baseline::BENIGN_RISK_EXACT,
        "false-positive control",
        "",
    );
    check(
        &report,
        report.benign_surfaced,
        baseline::BENIGN_SURFACED,
        "benign review surfacing",
        " Read the direction carefully: fewer benign columns drawing the review warning is \
         a precision improvement, not a fault. It is pinned exactly, in both directions, so \
         that neither the cost growing nor the cost shrinking happens without a number \
         moving in the same commit.",
    );
}

/// Compares one score against its recorded number, failing in *either* direction.
///
/// Failing when the score drops is the obvious half. Failing when it rises matters
/// just as much: a floor that silently lags the real number stops being a floor, and
/// the next regression then hides inside the slack. This is the mechanism behind the
/// module docs' instruction to raise the number in the same commit.
///
/// The message for a rise says "moved", not "improved", because not every axis here is one
/// where up is better: a rise in [`baseline::BENIGN_SURFACED`] means the review warning has
/// grown noisier. Each axis supplies its own `note`, which is appended in both directions,
/// so the axis rather than this function says what its direction means.
fn check(report: &Report, scored: usize, floor: usize, label: &str, note: &str) {
    if scored < floor {
        panic!(
            "{label} regressed: {scored}, recorded {floor}.{note}\n{}",
            report.render()
        );
    }
    assert_eq!(
        scored,
        floor,
        "{label} moved to {scored} from a recorded {floor}. Update the recorded number in \
         this commit so it keeps measuring — a stale number hides the next regression.\
         {note}\n{}",
        report.render()
    );
}

/// The corpus only works as an instrument while it still has headroom. If the
/// detector ever scores perfectly here, this file has stopped measuring
/// anything and needs harder cases before it is trusted again.
#[test]
fn held_out_corpus_still_has_headroom() {
    let report = score(cases());

    assert!(
        report.exact_type_correct < report.total,
        "the held-out corpus is now fully solved ({}/{}). It can no longer \
         detect an improvement or a regression in either direction. Add harder \
         cases before relying on it again.\n{}",
        report.exact_type_correct,
        report.total,
        report.render()
    );
    // Checked separately because it saturates first: its population is 28 rather than 44
    // and it is scored over a coarser question. `check` would still fail on the way up,
    // demanding a raised floor, but once the floor reaches the population nothing else in
    // this file notices that the axis has stopped being able to move.
    assert!(
        report.review_coverage < report.review_population,
        "every case that should reach a user now does ({}/{}). The review-coverage axis \
         can no longer show a recall improvement or a regression. Add cases the app would \
         miss — a header a name term cannot be read out of is the obvious kind — before \
         relying on this number.\n{}",
        report.review_coverage,
        report.review_population,
        report.render()
    );
}

struct Report {
    total: usize,
    exact_type_correct: usize,
    risk_floor_met: usize,
    /// Cases eligible for the review-coverage axis: those whose `expected_risk_floor`
    /// is above `PiiRisk::Low`, i.e. the ones that ought to reach the user somehow.
    /// Benign columns are deliberately outside it — see [`baseline::REVIEW_COVERAGE`].
    review_population: usize,
    review_coverage: usize,
    benign_total: usize,
    benign_risk_exact: usize,
    /// Benign look-alikes drawing the possible-person-name review warning. A cost, not a
    /// failure: pinned rather than minimised — see [`baseline::BENIGN_SURFACED`].
    benign_surfaced: usize,
    by_challenge: BTreeMap<Challenge, (usize, usize)>,
    failures: Vec<String>,
}

impl Report {
    fn render(&self) -> String {
        let mut out = String::from("\nheld-out detection corpus\n");
        out.push_str(&format!(
            "  exact type      {}/{} ({:.0}%)\n",
            self.exact_type_correct,
            self.total,
            percentage(self.exact_type_correct, self.total)
        ));
        out.push_str(&format!(
            "  risk floor met  {}/{} ({:.0}%)\n",
            self.risk_floor_met,
            self.total,
            percentage(self.risk_floor_met, self.total)
        ));
        out.push_str(&format!(
            "  surfaced        {}/{} ({:.0}%)\n",
            self.review_coverage,
            self.review_population,
            percentage(self.review_coverage, self.review_population)
        ));
        out.push_str(&format!(
            "  benign risk     {}/{} ({:.0}%)\n",
            self.benign_risk_exact,
            self.benign_total,
            percentage(self.benign_risk_exact, self.benign_total)
        ));
        out.push_str(&format!(
            "  benign surfaced {}/{} ({:.0}%)\n",
            self.benign_surfaced,
            self.benign_total,
            percentage(self.benign_surfaced, self.benign_total)
        ));
        out.push_str("  by challenge\n");
        for (challenge, (correct, total)) in &self.by_challenge {
            out.push_str(&format!(
                "    {:<24} {correct}/{total}\n",
                challenge.label()
            ));
        }
        if !self.failures.is_empty() {
            out.push_str("  misses\n");
            for failure in &self.failures {
                out.push_str(&format!("    {failure}\n"));
            }
        }
        out
    }
}

fn percentage(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        return 100.0;
    }
    part as f64 * 100.0 / whole as f64
}

fn score(cases: &[Case]) -> Report {
    let mut report = Report {
        total: cases.len(),
        exact_type_correct: 0,
        risk_floor_met: 0,
        review_population: 0,
        review_coverage: 0,
        benign_total: 0,
        benign_risk_exact: 0,
        benign_surfaced: 0,
        by_challenge: BTreeMap::new(),
        failures: Vec::new(),
    };

    for case in cases {
        let detection = detect_column_type_with_name(case.header, &strings(case.values));
        let analysis = analyze_column_privacy(
            case.header,
            0,
            &strings(case.values),
            detection.data_type,
            detection.confidence,
        );
        let risk = max_pii_risk(classify_pii_risk(detection.data_type), analysis.pii_risk);

        // Where the type vocabulary cannot express what the column holds, the type
        // alone is not evidence of anything: `String` is also what a total miss
        // returns. Such a case names the detector that has to have fired, so its
        // point is earned rather than granted by the shape of the enum.
        let detector_fired = case.expected_detector.is_none_or(|detector| {
            analysis
                .evidence
                .iter()
                .any(|summary| summary.detectors.iter().any(|found| found == detector))
        });
        let exact = detection.data_type == case.expected && detector_fired;
        let entry = report.by_challenge.entry(case.challenge).or_insert((0, 0));
        entry.1 += 1;
        if exact {
            report.exact_type_correct += 1;
            entry.0 += 1;
        }

        if risk_rank(risk) >= risk_rank(case.expected_risk_floor) {
            report.risk_floor_met += 1;
        }

        // Built once and asked twice: the recall axis below wants either route, the
        // precision axis under it wants the review tier alone.
        let column = column_metadata_for(case);

        // Scored only where the column ought to reach the user at all, which is what a
        // risk floor above Low says. A Low-floor case is either benign or genuinely
        // uninteresting, and for those "surfaced" is not a success — see
        // `baseline::REVIEW_COVERAGE` on why the reverse direction is not scored.
        let in_review_population = risk_rank(case.expected_risk_floor) > risk_rank(PiiRisk::Low);
        let surfaced = in_review_population && surfaced_for_review(&column);
        if in_review_population {
            report.review_population += 1;
            if surfaced {
                report.review_coverage += 1;
            }
        }

        // For benign columns the floor is also a ceiling: over-flagging costs
        // real utility, so the risk has to land exactly where it belongs.
        let benign_over_flagged = case.challenge == Challenge::BenignLookAlike
            && risk_rank(risk) > risk_rank(case.expected_risk_floor);
        if case.challenge == Challenge::BenignLookAlike {
            report.benign_total += 1;
            if risk == case.expected_risk_floor {
                report.benign_risk_exact += 1;
            }
            // Counted, not penalised — and counted through the review route alone, because
            // that is what the number is documented to mean. A benign column pulled in by
            // auto-selection instead is a risk escalation, which is `benign_risk_exact`'s
            // business. See `baseline::BENIGN_SURFACED`.
            if raises_review_warning(&column) {
                report.benign_surfaced += 1;
            }
        }

        let unsurfaced = in_review_population && !surfaced;
        if !exact
            || risk_rank(risk) < risk_rank(case.expected_risk_floor)
            || benign_over_flagged
            || unsurfaced
        {
            report.failures.push(format!(
                "{:<24} header {:<18} expected {:?}/{:?}, got {:?}/{:?}{}",
                case.challenge.label(),
                if case.header.is_empty() {
                    "<none>"
                } else {
                    case.header
                },
                case.expected,
                case.expected_risk_floor,
                detection.data_type,
                risk,
                // Called out explicitly because it is the one miss a reader cannot infer
                // from the two types printed: a case can be mistyped and under-risked and
                // still have reached the user through the review tier, and one that never
                // reached them at all is a different, worse failure.
                if unsurfaced { ", not surfaced" } else { "" }
            ));
        }
    }

    report
}

/// Whether the app would put this column in front of the user by either route it has.
///
/// Route one is auto-selection: a column whose risk clears the bar arrives in the column
/// table already ticked, with a redacting strategy defaulted onto it. The rule is taken
/// from production rather than restated here — `should_auto_select_column` also rejects a
/// column with no sample values, which no restatement would have remembered.
///
/// Route two is [`raises_review_warning`], the shipped warning function itself.
fn surfaced_for_review(column: &ColumnMetadata) -> bool {
    crate::metadata::should_auto_select_column(column) || raises_review_warning(column)
}

/// Whether the preview would carry the possible-person-name review warning for this
/// column, asked of the function that produces it.
///
/// `service::possible_person_name_warning_for_column` is reachable crate-wide, so this
/// tracks the warning's real gating rather than a proxy for it: the axis follows any later
/// change to *when* the warning fires, not merely to what the detector is called.
///
/// One clause of that gating cannot bind here and it is worth naming rather than
/// discovering later. The warning returns `None` for a selected column, on the ground that
/// the user has already seen it; [`column_metadata_for`] leaves every column unselected,
/// which is the state a user meets the table in and the state the warning was written for.
/// The clause is also redundant with route one, which counts a selected column anyway —
/// measured: the axis scores the same 24/28 through the real predicate as it did through
/// the detector constant.
fn raises_review_warning(column: &ColumnMetadata) -> bool {
    crate::service::possible_person_name_warning_for_column(column).is_some()
}

/// Builds the one-column `ColumnMetadata` the surfacing predicates are asked about.
///
/// Deliberately the more faithful path: `build_column_metadata` types the column through
/// `detect_column_type_in_context` with an inferred locale, exactly as a one-column file
/// would be typed on import, rather than through the bare detector the other axes score.
fn column_metadata_for(case: &Case) -> ColumnMetadata {
    let rows: Vec<Vec<String>> = case
        .values
        .iter()
        .map(|value| vec![(*value).to_string()])
        .collect();
    let mut metadata = crate::metadata::build_column_metadata(&[case.header.to_string()], &rows);
    metadata.remove(0)
}

fn risk_rank(risk: PiiRisk) -> u8 {
    match risk {
        PiiRisk::High => 3,
        PiiRisk::Medium => 2,
        PiiRisk::Low => 1,
    }
}

fn cases() -> &'static [Case] {
    &[
        // --- Cryptic, abbreviated, or absent headers -------------------------
        Case {
            challenge: Challenge::CrypticHeader,
            header: "c_nm",
            values: &[
                "Ada Lovelace",
                "Grace Hopper",
                "Alan Turing",
                "Edsger Dijkstra",
            ],
            expected: DataType::FullName,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        Case {
            challenge: Challenge::CrypticHeader,
            header: "eml_addr",
            values: &["ada@example.com", "grace@example.org", "alan@example.net"],
            expected: DataType::Email,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        Case {
            challenge: Challenge::CrypticHeader,
            header: "col_7",
            values: &["+1 415 555 0100", "+1 212 555 0101", "+1 312 555 0102"],
            expected: DataType::Phone,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        Case {
            challenge: Challenge::CrypticHeader,
            header: "",
            values: &[
                "Marie Dubois",
                "Luc Martin",
                "Jean Bernard",
                "Sophie Moreau",
            ],
            expected: DataType::FullName,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        Case {
            challenge: Challenge::CrypticHeader,
            header: "f3",
            values: &[
                "Hauptstrasse 12",
                "Marktplatz 5",
                "Bahnhofstrasse 88",
                "Lindenweg 3",
            ],
            expected: DataType::Address,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        Case {
            challenge: Challenge::CrypticHeader,
            header: "dob",
            values: &["1980-01-02", "1991-03-04", "1975-11-30", "1966-06-21"],
            expected: DataType::Timestamp,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Medium,
        },
        // --- Languages outside the eight-language taxonomy ------------------
        Case {
            challenge: Challenge::UntaxonomizedLanguage,
            header: "imię_i_nazwisko",
            values: &["Jan Kowalski", "Anna Nowak", "Piotr Wiśniewski"],
            expected: DataType::FullName,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        Case {
            challenge: Challenge::UntaxonomizedLanguage,
            header: "telefonnummer",
            values: &["+46 70 123 45 67", "+46 73 234 56 78", "+46 76 345 67 89"],
            expected: DataType::Phone,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        Case {
            challenge: Challenge::UntaxonomizedLanguage,
            header: "adres",
            values: &["ul. Marszałkowska 10", "ul. Długa 22", "ul. Krótka 4"],
            expected: DataType::Address,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        Case {
            challenge: Challenge::UntaxonomizedLanguage,
            header: "posta_kodu",
            values: &["34710", "06510", "35220", "01120"],
            expected: DataType::PostalCode,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Medium,
        },
        Case {
            challenge: Challenge::UntaxonomizedLanguage,
            header: "e-posta",
            values: &["ayse@example.com.tr", "mehmet@example.tr"],
            expected: DataType::Email,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        // --- Real values in awkward formats ---------------------------------
        Case {
            challenge: Challenge::MessyFormatting,
            header: "contact",
            values: &[
                "Ada Lovelace <ada@example.com>",
                "Grace Hopper <grace@example.org>",
            ],
            expected: DataType::Email,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        Case {
            challenge: Challenge::MessyFormatting,
            header: "phone",
            values: &[
                "+1 415 555 0100 x22",
                "+1 212 555 0101 ext. 8",
                "+1 312 555 0102",
            ],
            expected: DataType::Phone,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        Case {
            challenge: Challenge::MessyFormatting,
            header: "bedrag",
            values: &["€ 1.234,56", "€ 987,00", "€ 12.000,10"],
            expected: DataType::Currency,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Low,
        },
        Case {
            challenge: Challenge::MessyFormatting,
            header: "omzet",
            values: &["1.234,56", "987,00", "12.000,10", "45,99"],
            expected: DataType::NumericValue,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Low,
        },
        Case {
            challenge: Challenge::MessyFormatting,
            header: "server_addr",
            values: &["2001:db8::1", "2001:db8:85a3::8a2e:370:7334", "fe80::1"],
            expected: DataType::IpAddress,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Medium,
        },
        // There is no `DataType::Iban`, so `String` is the most specific answer
        // available — and `String` is also what a total miss returns, which would
        // score this case correct for recognizing nothing. Hence the named
        // detector: the point is only awarded if the checksum validator actually
        // fired on these space-separated IBANs.
        Case {
            challenge: Challenge::MessyFormatting,
            header: "iban",
            values: &[
                "NL91 ABNA 0417 1643 00",
                "DE89 3704 0044 0532 0130 00",
                "FR14 2004 1010 0505 0001 3M02 606",
            ],
            expected: DataType::String,
            expected_detector: Some("validator:iban"),
            expected_risk_floor: PiiRisk::High,
        },
        Case {
            challenge: Challenge::MessyFormatting,
            header: "email",
            values: &["ADA@EXAMPLE.COM", " grace@example.org ", "Alan@Example.Net"],
            expected: DataType::Email,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        // --- Person names behind an unenumerated `<word> name` header --------
        //
        // Five role-qualified headers of the kind that dominate exported
        // ticketing, HR, and clinical tables. Each is unambiguous to a reader and
        // invisible to the detector. They are listed individually rather than
        // collapsed into one representative case because widening the header gate
        // is a per-term decision, and a single case cannot show which terms a
        // proposed widening actually reaches.
        Case {
            challenge: Challenge::HeaderGatedName,
            header: "agent_name",
            values: &[
                "Ada Lovelace",
                "Grace Hopper",
                "Alan Turing",
                "Edsger Dijkstra",
            ],
            expected: DataType::FullName,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        Case {
            challenge: Challenge::HeaderGatedName,
            header: "employee_name",
            values: &[
                "Marie Dubois",
                "Luc Martin",
                "Sophie Moreau",
                "Jean Bernard",
            ],
            expected: DataType::FullName,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        Case {
            challenge: Challenge::HeaderGatedName,
            header: "reviewer_name",
            values: &[
                "Nina Petrov",
                "Omar Haddad",
                "Ruth Kelly",
                "Peter Lindqvist",
            ],
            expected: DataType::FullName,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        // A clinical export. The risk floor is High for the same reason as the
        // others — it is the name that identifies, not the health context — but
        // this is the case where a miss costs the most.
        Case {
            challenge: Challenge::HeaderGatedName,
            header: "patient_name",
            values: &[
                "Hilda Berg",
                "Tomas Novak",
                "Aisha Rahman",
                "Victor Oyelaran",
            ],
            expected: DataType::FullName,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        Case {
            challenge: Challenge::HeaderGatedName,
            header: "assignee_name",
            values: &["Karel Janssen", "Ineke de Boer", "Rob Visser", "Lotte Smit"],
            expected: DataType::FullName,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        // Diacritics, on a header the gate does not open for. Two things are
        // being separated here: the value confirmation `is_plausible_full_name`
        // accepts these (its token test is `char::is_alphabetic`, not ASCII), so a
        // miss recorded on this case is attributable to the header alone. If a
        // widening of the gate does not turn this case green, the confirmation
        // regressed on non-ASCII letters and this is the case that says so.
        Case {
            challenge: Challenge::HeaderGatedName,
            header: "staff_name",
            values: &[
                "José Fernández",
                "Ana Sofía Ruiz",
                "Renée Dubois",
                "Björn Nyström",
            ],
            expected: DataType::FullName,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        // Lowercase values, as produced by systems that normalize case on import.
        // Same separation as the diacritics case: the confirmation does not test
        // capitalization for full names, so the header is the only thing failing.
        Case {
            challenge: Challenge::HeaderGatedName,
            header: "operator_name",
            values: &[
                "ada lovelace",
                "grace hopper",
                "alan turing",
                "edsger dijkstra",
            ],
            expected: DataType::FullName,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        // Five distinct names over twenty-five rows: a small team appearing
        // repeatedly, which is the ordinary shape of an owner column. This case
        // carries far more values than its neighbours on purpose — low
        // cardinality is the whole point, and it cannot be expressed in a
        // four-value sample. It is the only case in the corpus that reaches the
        // categorical stage, and so the only one that can catch a widened name
        // rule being placed after the enum check instead of before it.
        Case {
            challenge: Challenge::HeaderGatedName,
            header: "case_owner_name",
            values: &[
                "Ada Lovelace",
                "Grace Hopper",
                "Alan Turing",
                "Edsger Dijkstra",
                "Marie Curie",
                "Ada Lovelace",
                "Grace Hopper",
                "Alan Turing",
                "Edsger Dijkstra",
                "Marie Curie",
                "Ada Lovelace",
                "Grace Hopper",
                "Alan Turing",
                "Edsger Dijkstra",
                "Marie Curie",
                "Ada Lovelace",
                "Grace Hopper",
                "Alan Turing",
                "Edsger Dijkstra",
                "Marie Curie",
                "Ada Lovelace",
                "Grace Hopper",
                "Alan Turing",
                "Edsger Dijkstra",
                "Marie Curie",
            ],
            expected: DataType::FullName,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        // --- Identifier columns whose keys are not digits --------------------
        //
        // Four alphabets under four ordinary `*_id` / `*_ref` headers. The corpus
        // held none of these before: its only identifier case was `seq`, holding
        // `1000001…`, which is the one shape the integer-corroborated header rule
        // could already see. That is why the gap these probe stayed green for as long
        // as it did, and why they are listed one alphabet at a time — recognising a
        // letter prefix says nothing about recognising an opaque hex key.
        Case {
            challenge: Challenge::NonNumericKey,
            header: "employee_id",
            values: &[
                "E1000", "E1001", "E1002", "E1003", "E1004", "E1005", "E1006", "E1007", "E1008",
                "E1009", "E1010", "E1011",
            ],
            expected: DataType::NumericId,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Medium,
        },
        Case {
            challenge: Challenge::NonNumericKey,
            header: "customer_id",
            values: &[
                "CUST-0042",
                "CUST-0043",
                "CUST-0044",
                "CUST-0045",
                "CUST-0046",
                "CUST-0047",
                "CUST-0048",
                "CUST-0049",
                "CUST-0050",
                "CUST-0051",
                "CUST-0052",
                "CUST-0053",
            ],
            expected: DataType::NumericId,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Medium,
        },
        // An opaque key with no prefix to recognise and no separator to lean on: the
        // only thing saying it is a key is that twelve eight-character mixed
        // letter-and-digit tokens are all different. If a future tightening of the
        // shape test starts requiring a recognisable prefix, this is the case that
        // says so.
        Case {
            challenge: Challenge::NonNumericKey,
            header: "case_id",
            values: &[
                "a1b2c0d4", "a1b2c1d4", "9f3e2d1c", "7b6a5949", "3c2b1a09", "e5d4c3b2", "0a1b2c3d",
                "4e5f6a7b", "8c9d0e1f", "2f3a4b5c", "6d7e8f90", "1a2b3c4d",
            ],
            expected: DataType::NumericId,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Medium,
        },
        // Unpadded keys, and a recorded miss. The shape test asks that a column's
        // values agree on a length, which is what separates a minted key from a
        // hand-written label; keys counted up from one without padding do not, so
        // this column is left unselected. Systems that export keys overwhelmingly
        // zero-pad them — the three cases above are the common shape and this is the
        // uncommon one — but the cost of that choice belongs in the instrument rather
        // than in a caveat, and this is where a change that removes it will show up.
        Case {
            challenge: Challenge::NonNumericKey,
            header: "order_ref",
            values: &[
                "REF-7",
                "REF-83",
                "REF-104",
                "REF-1042",
                "REF-9",
                "REF-51",
                "REF-6613",
                "REF-70281",
                "REF-312",
                "REF-104213",
            ],
            expected: DataType::NumericId,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Medium,
        },
        // --- Benign look-alikes that must not be flagged --------------------
        Case {
            challenge: Challenge::BenignLookAlike,
            header: "sku",
            values: &["SKU-00012", "SKU-00013", "SKU-00014", "SKU-00015"],
            expected: DataType::String,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Low,
        },
        Case {
            challenge: Challenge::BenignLookAlike,
            header: "app_version",
            values: &["1.2.3", "1.2.4", "10.0.1", "2.11.0"],
            expected: DataType::String,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Low,
        },
        Case {
            challenge: Challenge::BenignLookAlike,
            header: "checksum",
            values: &[
                "d41d8cd98f00b204e9800998ecf8427e",
                "5d41402abc4b2a76b9719d911017c592",
            ],
            expected: DataType::String,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Low,
        },
        Case {
            challenge: Challenge::BenignLookAlike,
            header: "product_name",
            values: &["Blue Widget", "Red Gadget", "Green Sprocket", "Grey Flange"],
            expected: DataType::String,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Low,
        },
        // Dotted quads in a column that is plainly not about networking. Every
        // value is valid IPv4 syntax, so this probes whether a non-network
        // header keeps a shape match from escalating the column's risk.
        Case {
            challenge: Challenge::BenignLookAlike,
            header: "measurement",
            values: &["12.34.56.78", "98.76.54.32", "11.22.33.44"],
            expected: DataType::String,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Low,
        },
        // --- `<word> name` columns that hold no person -----------------------
        //
        // Every value below is two or three capitalized alphabetic tokens, which
        // is precisely what `is_plausible_full_name` accepts: structurally these
        // are indistinguishable from person names, and no value-level evidence
        // separates them, because the name gazetteer was withdrawn. The only thing
        // keeping them Low today is that the header gate is shut for these terms
        // too. That makes them the cost side of widening it — each one is a column
        // whose utility is destroyed if a widened gate treats `<word> name` as a
        // person-name signal without asking what the word is.
        //
        // They are `BenignLookAlike` rather than a category of their own so they
        // are counted by `benign_risk_exact`, which is scored exactly rather than
        // as a floor. A category of their own would read tidily in the report and
        // would silently drop them out of the only false-positive metric here.
        Case {
            challenge: Challenge::BenignLookAlike,
            header: "company_name",
            values: &[
                "Acme Corporation",
                "Globex Industries",
                "Initech Systems",
                "Umbrella Holdings",
            ],
            expected: DataType::String,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Low,
        },
        // A city is not personal data on its own, and the taxonomy has no
        // locality term, so nothing should escalate this column. Note the values
        // carry no digits, which is what keeps them clear of the value-level
        // address rule.
        Case {
            challenge: Challenge::BenignLookAlike,
            header: "city_name",
            values: &["New York", "San Francisco", "Kansas City", "Salt Lake City"],
            expected: DataType::String,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Low,
        },
        Case {
            challenge: Challenge::BenignLookAlike,
            header: "team_name",
            values: &[
                "Platform Engineering",
                "Customer Success",
                "Data Infrastructure",
                "Quality Assurance",
            ],
            expected: DataType::String,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Low,
        },
        Case {
            challenge: Challenge::BenignLookAlike,
            header: "department_name",
            values: &[
                "Human Resources",
                "Legal Affairs",
                "Internal Audit",
                "Facilities Management",
            ],
            expected: DataType::String,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Low,
        },
        Case {
            challenge: Challenge::BenignLookAlike,
            header: "project_name",
            values: &[
                "Northern Lights",
                "Blue Harbor",
                "Silver Falcon",
                "Open Meadow",
            ],
            expected: DataType::String,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Low,
        },
        Case {
            challenge: Challenge::BenignLookAlike,
            header: "event_name",
            values: &[
                "Annual Summit",
                "Spring Kickoff",
                "Partner Forum",
                "Winter Retreat",
            ],
            expected: DataType::String,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Low,
        },
        // Filenames are the sharpest of these traps. `is_plausible_name_token`
        // permits `.` inside a token so that `Jr.` and initials survive, which
        // means `final.pdf` reads as a name token and `report final.pdf` reads as
        // a two-part person name. The extension is not the safeguard it looks
        // like; only a digit in the value is (see the `product_name` case, where
        // `Widget Pro 3000` is rejected on that ground alone).
        Case {
            challenge: Challenge::BenignLookAlike,
            header: "file_name",
            values: &[
                "report final.pdf",
                "invoice draft.xlsx",
                "notes summary.docx",
                "budget revised.pptx",
            ],
            expected: DataType::String,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Low,
        },
        // The two cases below are not hypothetical costs of a future widening —
        // they are false positives reachable in the shipped product today.
        // `client name` and `legal name` are already enumerated as `full_name`
        // terms, so the gate is open, and the value confirmation is too weak to
        // close it: an organization name satisfies it exactly as a person's name
        // does. In B2B data both headers far more often hold the counterparty
        // entity than a natural person, so these are the realistic reading, not
        // the adversarial one. They are recorded as over-flags, and if a later
        // change makes them exact it has strengthened value confirmation, which is
        // the improvement that would make widening the gate safe at all.
        Case {
            challenge: Challenge::BenignLookAlike,
            header: "client_name",
            values: &[
                "Acme Corporation",
                "Globex Industries",
                "Initech Systems",
                "Umbrella Holdings",
            ],
            expected: DataType::String,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Low,
        },
        Case {
            challenge: Challenge::BenignLookAlike,
            header: "legal_name",
            values: &[
                "Northwind Traders",
                "Contoso Manufacturing",
                "Fabrikam Logistics",
                "Tailspin Toys",
            ],
            expected: DataType::String,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Low,
        },
        // Sequence numbers are genuinely `NumericId`, and the app treats that as
        // Medium risk by design — a surrogate key can still be a re-identifier.
        // Scored as an exact-risk case, so escalating it further would fail.
        Case {
            challenge: Challenge::BenignLookAlike,
            header: "seq",
            values: &["1000001", "1000002", "1000003", "1000004"],
            expected: DataType::NumericId,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Medium,
        },
        // The cost side of reading a key shape as an identifier, one gate at a time.
        // Every value below would pass the per-value shape test on its own, so each
        // of these columns is what the column-level gates exist to refuse.
        //
        // A status vocabulary: five codes over twenty rows. `*_code` is deliberately
        // outside the identifier header family — `status_code`, `reason_code`,
        // `currency_code` and `product_code` dominate that suffix, and a rule that
        // destroys a product key buys no privacy — so this column is not even
        // offered to the rule. `Enum` is the reviewer's answer as well as the
        // detector's: a repeated finite set is what it is.
        Case {
            challenge: Challenge::BenignLookAlike,
            header: "status_code",
            values: &[
                "ACT1", "PND2", "CLS3", "HLD4", "CAN5", "ACT1", "PND2", "CLS3", "HLD4", "CAN5",
                "ACT1", "PND2", "CLS3", "HLD4", "CAN5", "ACT1", "PND2", "CLS3", "HLD4", "CAN5",
            ],
            expected: DataType::Enum,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Low,
        },
        // A foreign key into a four-row dimension table, under a header that *is* in
        // the identifier family. Nothing about the values distinguishes it from
        // `employee_id` above; what does is that there are four of them across twenty
        // rows, which is the distinctness gate's entire job. Read the direction
        // carefully: the same gate rejects a genuine low-cardinality foreign key, so
        // this case pins a decision that costs recall, not a free win.
        Case {
            challenge: Challenge::BenignLookAlike,
            header: "region_ref",
            values: &[
                "RGN-01", "RGN-02", "RGN-03", "RGN-04", "RGN-01", "RGN-02", "RGN-03", "RGN-04",
                "RGN-01", "RGN-02", "RGN-03", "RGN-04", "RGN-01", "RGN-02", "RGN-03", "RGN-04",
                "RGN-01", "RGN-02", "RGN-03", "RGN-04",
            ],
            expected: DataType::Enum,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Low,
        },
        // Free text under an identifier header, which is the ordinary fate of a
        // nullable reference column in a system people type into. The header opens
        // the gate; the values have to close it.
        Case {
            challenge: Challenge::BenignLookAlike,
            header: "booking_ref",
            values: &[
                "confirmed by phone",
                "rebooked for tuesday",
                "no answer, left message",
                "cancelled at the desk",
                "waiting on the deposit",
                "moved to the later slot",
                "guest asked for a quiet room",
                "arriving after midnight",
                "paid the balance in cash",
                "needs an accessible room",
            ],
            expected: DataType::String,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Low,
        },
        // --- Partly-populated columns ---------------------------------------
        Case {
            challenge: Challenge::MixedColumn,
            header: "notes",
            values: &[
                "call back",
                "ada@example.com",
                "no answer",
                "grace@example.org",
                "left voicemail",
            ],
            expected: DataType::String,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        Case {
            challenge: Challenge::MixedColumn,
            header: "identifier",
            values: &[
                "ada@example.com",
                "grace@example.org",
                "alan@example.net",
                "unknown",
                "n/a",
            ],
            expected: DataType::Email,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        Case {
            challenge: Challenge::MixedColumn,
            header: "reference",
            values: &[
                "550e8400-e29b-41d4-a716-446655440000",
                "6ba7b810-9dad-11d1-80b4-00c04fd430c8",
                "pending",
                "7c9e6679-7425-40de-944b-e07fc1f90ae7",
            ],
            expected: DataType::Uuid,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Medium,
        },
    ]
}
