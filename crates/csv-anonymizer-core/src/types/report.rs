use super::*;
use serde::{Deserialize, Serialize};
/// What one column's consistent pseudonyms reveal about the values behind them.
///
/// A consistent pseudonym preserves equality, which is why the strategies that
/// produce one keep a dataset joinable. The cost is that equality also preserves
/// the *shape* of the value distribution, and the shape is enough to work against
/// the mapping:
///
/// - Few `distinct_values` over many `total_values` means the mapping can be
///   relabelled by frequency, by anyone who knows how the real field is
///   distributed. The tokens stay opaque; the histogram does not.
/// - A `singleton_values` entry is a pseudonym covering exactly one row, which
///   singles that record out however unguessable the token looks.
/// - `max_value_occurrences` is the most common value's row count, and it is the
///   sharpest of the four for frequency inversion — the dominant value is the
///   attacker's easiest anchor, and inverting that one pseudonym recovers
///   `max_value_occurrences / total_values` of the column in a single step. Acted on
///   as a *share* rather than a count, because a count means something different in a
///   200-row file than in a 5-million-row one. The threshold it is compared against is
///   `MIN_INVERTIBLE_DOMINANT_SHARE`, which is private, so this names it rather than
///   linking to it.
///
/// Reported per column because the risk is per column: one low-cardinality column
/// is not made safer by a high-cardinality neighbour.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnValueDistribution {
    pub column_index: usize,
    pub distinct_values: usize,
    pub total_values: usize,
    pub singleton_values: usize,
    /// Values seen exactly twice. Carried for the same reason as the singleton
    /// count: the two together are what let a *sampled* distribution say anything
    /// about the input it was drawn from — see `estimated_distinct_values`, which is
    /// private, so this names it rather than linking to it.
    #[serde(default)]
    pub doubleton_values: usize,
    pub max_value_occurrences: usize,
}

/// Minimum values before the cardinality test means anything.
///
/// A floor is not optional. `distinct_values <= total_values`, so any
/// `distinct_values < K` test is *vacuously true* whenever `total_values < K` — on a
/// 5-row fixture every column looks low-cardinality, including the unique ID
/// columns. Measured on `tests/fixtures/large.csv` truncated to varying lengths: at
/// 5 rows all 7 columns trip the test, including `id`, `email` and `user_uuid`; the
/// mis-flags disappear at 10 rows, i.e. at exactly K.
///
/// 50 rather than the minimum 10 because the margin matters: at 20 rows the
/// highest-cardinality realistic column (`name`) shows 19 distinct values, one step
/// from the boundary, while at 50 rows it shows 42 — a margin of 32. It also
/// silences every short test fixture, which is where all the noise was.
const CARDINALITY_FLOOR: usize = 50;

/// Distinct values below which a mapping is treated as frequency-invertible.
///
/// The two genuinely low-cardinality columns in the corpus saturate at 8 (`country`)
/// and 4 (`status`); the next column up saturates at 100 (`name`). The data pins this
/// constant only to the interval (8, 100] — the decade between is unpopulated — so 10
/// is the conservative end of the measured interval, and it agrees with the common
/// convention that fewer than about ten buckets makes a frequency table trivially
/// labellable.
const MAX_INVERTIBLE_DISTINCT_VALUES: usize = 10;

/// Distinct-to-total ratio below which a mapping is treated as frequency-invertible,
/// for columns too large for the absolute test to catch.
///
/// Reads as "each pseudonym covers more than twenty rows on average". It exists for
/// the case the absolute test misses: `large.csv name` has 100 distinct values over
/// 10500 rows, so `distinct < 10` is false while the ratio is 0.0095.
///
/// 0.05 rather than a looser 0.20 by a subsumption argument rather than measurement.
/// `distinct/total < 0.05` is already implied by `distinct < 10` for any total below
/// 200, so the ratio term is inert on small and medium inputs and the verdict there
/// rests entirely on the stable statistic — the absolute count, which saturates. The
/// ratio only takes over once there are enough rows for it to have converged. A 0.20
/// threshold would instead activate immediately above the floor and flag, say, 2000
/// distinct names in a 10500-row file, which is a five-row group.
const MAX_INVERTIBLE_DISTINCT_RATIO: f64 = 0.05;

/// Sample coverage below which a *sampled* distribution is treated as saying nothing
/// about the input's distinct count, so the ratio test is skipped. Good–Turing coverage,
/// `1 - singletons/values`; the measurements pin it only to the interval (0.40, 0.87] and
/// 0.75 sits inside with margin at both ends. Not measured on real production data,
/// non-Zipf skew, or columns sitting near the gate itself.
/// See docs/calibration.md#min_sample_coverage for the measurements behind this.
const MIN_SAMPLE_COVERAGE: f64 = 0.75;

/// Share of a column's values carried by its single most common value, at or above which
/// the mapping is treated as frequency-invertible. Catches the shape neither other term
/// sees: thousands of distinct values, one of them covering most of the rows. A share and
/// not a count, so it means the same in a 200-row file and a 5-million-row one. The
/// measurements pin it to [1/3, 0.35]; not measured on real production columns.
/// See docs/calibration.md#min_invertible_dominant_share for the measurements behind this.
const MIN_INVERTIBLE_DOMINANT_SHARE: f64 = 1.0 / 3.0;

/// Which of the three tests judged a distribution frequency-invertible.
///
/// Exists so a warning can name the evidence it actually has. The three terms catch
/// genuinely different shapes — a handful of values, one value dominating a diverse
/// column, and many small groups across a large one — and a single wording cannot
/// describe all three without describing at least two of them wrongly. Reporting a
/// column of 101 values where one covers half the rows as holding "only 101 distinct
/// value(s)" would be true, would read as reassuring, and would name a risk the column
/// does not have while staying silent about the one it does.
///
/// Each variant carries the figure its wording needs, computed at the point the test
/// fires, because [`ColumnValueDistribution::estimated_distinct_values`] and
/// [`ColumnValueDistribution::dominant_value_share`] are private and a message
/// builder outside this module cannot recover them from the public fields.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum FrequencyInversionRisk {
    /// Few enough distinct values that the whole frequency table is labellable.
    FewDistinctValues,
    /// One value carries [`MIN_INVERTIBLE_DOMINANT_SHARE`] or more of the column, so
    /// inverting that single pseudonym recovers `share` of it.
    DominantValue { share: f64 },
    /// Enough rows per distinct value that the groups can be matched by size, even
    /// though no single value dominates and there are too many to enumerate.
    LargeGroups { estimated_distinct_values: usize },
}

impl ColumnValueDistribution {
    /// Builds the distribution of `values`, skipping the ones a transform would skip.
    ///
    /// Uses [`crate::detection::is_empty_value`] and [`crate::smart::value_identity_key`]
    /// so that a distribution measured over a detection sample is comparable with one
    /// accumulated during a run: the transform returns early on empty values and folds
    /// case and padding, so counting either differently here would make the pre-run
    /// warning disagree with the post-run report on the same data.
    pub(crate) fn from_values(column_index: usize, values: &[String]) -> Self {
        let mut occurrences: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut total_values = 0usize;
        for value in values {
            let trimmed = value.trim();
            if crate::detection::is_empty_value(trimmed) {
                continue;
            }
            total_values += 1;
            *occurrences
                .entry(crate::smart::value_identity_key(trimmed))
                .or_insert(0) += 1;
        }

        Self {
            column_index,
            distinct_values: occurrences.len(),
            total_values,
            singleton_values: occurrences.values().filter(|count| **count == 1).count(),
            doubleton_values: occurrences.values().filter(|count| **count == 2).count(),
            max_value_occurrences: occurrences.values().copied().max().unwrap_or(0),
        }
    }

    /// Whether consistent pseudonyms over this distribution could be relabelled by
    /// frequency analysis.
    ///
    /// Singleton counts deliberately play no part. A singleton rule reads as the
    /// record-isolation risk and sounds right, but measured against the corpus it
    /// flags every unique-key column — `id`, `email`, `user_uuid`, `created_at` —
    /// which are exactly the columns users pseudonymize as a matter of course. That
    /// isolation is a property of the input being unique already, not something
    /// consistent pseudonymization introduces, and `distinct == total` says it more
    /// directly. A warning that fires on every email column is noise.
    ///
    /// See [`CARDINALITY_FLOOR`], [`MAX_INVERTIBLE_DISTINCT_VALUES`],
    /// [`MIN_INVERTIBLE_DOMINANT_SHARE`], [`MAX_INVERTIBLE_DISTINCT_RATIO`] and
    /// [`MIN_SAMPLE_COVERAGE`] for what each constant is measured against.
    ///
    /// For a distribution measured over the whole column. A sampled one has to say
    /// how large the column really is — see [`Self::frequency_inversion_risk_in`].
    pub(crate) fn risks_frequency_inversion(&self) -> bool {
        self.frequency_inversion_risk().is_some()
    }

    /// [`Self::risks_frequency_inversion`], with the reason it answered yes.
    pub(crate) fn frequency_inversion_risk(&self) -> Option<FrequencyInversionRisk> {
        self.frequency_inversion_risk_in(self.total_values)
    }

    /// As [`Self::risks_frequency_inversion`], for a distribution measured over a
    /// sample of a column that holds `population_values` values in total.
    ///
    /// The distinction is not pedantic — getting it wrong is what made the pre-run
    /// warning miss the case it exists for. The ratio test divides distinct values by
    /// the column's size, and a sample's size is the *sample's*, capped at a hundred
    /// or so. Measured against that cap, `distinct / total < 0.05` needs fewer than
    /// five distinct values in a hundred, which the absolute test has already caught:
    /// the ratio term was inert on every sampled distribution while being the only
    /// term that catches a column of thirty values in five thousand rows. Such a
    /// column drew no warning until after its output had been written.
    ///
    /// Dividing by the true row count instead is wrong in the other direction, and
    /// worse: a hundred sampled values cannot look like a million distinct ones, so a
    /// fully unique column would score 0.005 and be flagged as trivially invertible.
    /// [`MIN_SAMPLE_COVERAGE`] is the gate that separates the two, and
    /// [`Self::estimated_distinct_values`] is what gets compared once it opens.
    ///
    /// `population_values` is the column's row count, an upper bound on how many
    /// values it holds — empty cells are not values. Erring high makes the ratio
    /// smaller and the warning likelier, which is the safe direction for a warning
    /// the user can dismiss by choosing another strategy.
    ///
    /// The dominant-value term needs no equivalent adjustment, and that is the reason it
    /// sits where it does. It compares two figures the distribution measured *itself* —
    /// a share of its own values — so `population_values` never enters. A share is also
    /// the one statistic a small sample can estimate here, unlike its distinct count,
    /// which is why only the distinct-count term needs a population figure and a gate.
    /// See docs/calibration.md#sample-share-vs-distinct-count for the measurements.
    ///
    /// The order of the three tests is the order of decreasing certainty, and it is
    /// also what each variant means: a later variant implies the earlier tests
    /// declined. So `LargeGroups` is only ever reported for a column with at least
    /// [`MAX_INVERTIBLE_DISTINCT_VALUES`] distinct values and no dominant one, which is
    /// what lets its wording talk about average group size without qualification.
    pub(crate) fn frequency_inversion_risk_in(
        &self,
        population_values: usize,
    ) -> Option<FrequencyInversionRisk> {
        if self.total_values < CARDINALITY_FLOOR {
            return None;
        }
        if self.distinct_values < MAX_INVERTIBLE_DISTINCT_VALUES {
            return Some(FrequencyInversionRisk::FewDistinctValues);
        }
        // Deliberately ahead of the coverage gate. The shape this catches — one dominant
        // value over a long tail of near-unique ones — is singleton-heavy and therefore
        // *low*-coverage, so behind the gate this term would be silent on exactly the
        // See docs/calibration.md#min_sample_coverage for the figures.
        let share = self.dominant_value_share();
        if share >= MIN_INVERTIBLE_DOMINANT_SHARE {
            return Some(FrequencyInversionRisk::DominantValue { share });
        }
        if self.sample_coverage() < MIN_SAMPLE_COVERAGE {
            return None;
        }
        let estimated_distinct_values = self.estimated_distinct_values();
        if (estimated_distinct_values as f64) / (population_values.max(1) as f64)
            < MAX_INVERTIBLE_DISTINCT_RATIO
        {
            return Some(FrequencyInversionRisk::LargeGroups {
                estimated_distinct_values,
            });
        }
        None
    }

    /// Share of this distribution's values carried by its single most common value.
    ///
    /// The fraction of the column that one inverted pseudonym recovers, so it is also
    /// the size of the leak the warning is reporting. `0.0` for a distribution that
    /// measured nothing, and `1 / total_values` for a fully unique column — which is
    /// below [`MIN_INVERTIBLE_DOMINANT_SHARE`] for any column past
    /// [`CARDINALITY_FLOOR`], so a unique column cannot trip the term that reads this.
    fn dominant_value_share(&self) -> f64 {
        if self.total_values == 0 {
            return 0.0;
        }
        (self.max_value_occurrences as f64) / (self.total_values as f64)
    }

    /// Good–Turing sample coverage: the estimated share of this column's values that
    /// belong to a group the distribution has already seen.
    ///
    /// Exactly `1.0` for a distribution measured over a whole column with no
    /// singletons, and `0.0` for one where every value was seen once — which is both
    /// a unique column measured exactly and a sample that has learned nothing.
    /// [`Self::frequency_inversion_risk_in`] is what distinguishes those, by only
    /// consulting coverage once the absolute test has declined.
    fn sample_coverage(&self) -> f64 {
        if self.total_values == 0 {
            return 0.0;
        }
        1.0 - (self.singleton_values as f64) / (self.total_values as f64)
    }

    /// Chao1: the distinct count this distribution implies for the whole column,
    /// including groups it did not see.
    ///
    /// `distinct + f1² / 2·f2` over singletons and doubletons — the standard
    /// lower-bound estimator, on the reasoning that how many groups you saw *once*
    /// tells you how many you missed entirely. Returns the observed count unchanged
    /// for a distribution with no singletons, which is what a fully measured column
    /// with every value repeated looks like.
    ///
    /// A lower bound, so it under-estimates rather than over-estimates a diverse
    /// column, which is the wrong direction for a warning — hence
    /// [`MIN_SAMPLE_COVERAGE`] refusing the comparison before this is consulted.
    /// `pub(crate)` because the preflight memory projection needs the same estimate
    /// and a second copy of a statistical estimator is a copy that drifts.
    pub(crate) fn estimated_distinct_values(&self) -> usize {
        if self.doubleton_values > 0 {
            return self.distinct_values
                + self.singleton_values.saturating_pow(2) / (2 * self.doubleton_values);
        }
        // The bias-corrected form, for when nothing was seen exactly twice and the
        // ratio above would divide by zero.
        self.distinct_values
            + self
                .singleton_values
                .saturating_mul(self.singleton_values.saturating_sub(1))
                / 2
    }
}
/// How exposed the released rows are once every column is read together.
///
/// The rest of this crate's privacy figures are per column, so a file can be reported as
/// having no unselected high or medium risk column while postcode, birth date and job
/// title jointly single out a third of its rows. This is the figure that says so.
///
/// Measured over the columns an outsider could match against data they already hold —
/// see `crate::uniqueness::LinkableProjection` for how that subset is decided and what it
/// deliberately leaves out. Every count here is over that subset except
/// `distinct_rows_all_columns`.
///
/// Absent rather than zeroed where there are no rows: unstructured text and single pasted
/// values never populate it, and a summary claiming zero unique rows would read as a clean
/// result from a check that never ran.
///
/// A DTO, like [`DetectionCoverageSummary`] and unlike the private `DetectionCoverage`: a flat
/// snapshot with no invariant of its own to protect, so `Deserialize` costs nothing here.
/// The relationships between these figures — that the counts are over the columns named,
/// that a stopped measurement zeroes the rest — are established in
/// `crate::uniqueness::RowUniquenessTracker::summary`, which is the only thing that builds
/// one outside tests.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RowUniquenessSummary {
    /// Data rows hashed. Blank rows are written through untransformed and are not counted.
    pub rows_measured: usize,
    /// The columns the measure read, each with what an outsider could match it on, in
    /// column order. Empty means nothing released is matchable — a statement about how the
    /// columns were transformed, not a finding that the data is anonymous.
    ///
    /// Only columns that actually yielded something are listed. A column whose projection
    /// came back empty on every row contributed nothing to any class and is named nowhere,
    /// so no claim here can rest on a column that in fact carried no signal.
    pub matched_columns: Vec<MatchedColumn>,
    /// Distinct equivalence classes over the subset.
    pub distinct_classes: usize,
    /// Rows alone in their class: someone holding those columns for a person finds
    /// exactly their row.
    pub unique_rows: usize,
    /// The k-anonymity floor — the smallest class present. One freak record sets it, which
    /// is why `fifth_percentile_class_size` is reported beside it.
    pub smallest_class: usize,
    /// The class size at or below which the most exposed 5% of rows sit.
    pub fifth_percentile_class_size: usize,
    /// Distinct rows over *every* released column, subset rule not applied. Answers the
    /// separate and simpler question of whether the file could be shuffled or aggregated,
    /// and acts as a check on the subset rule: a wide gap between this and
    /// `distinct_classes` means the rule is doing a lot of work and deserves a look.
    ///
    /// `None` when this histogram alone outgrew what the check keeps. It fills faster than
    /// the other one by construction — whole rows against projections of a subset of them —
    /// so it is suppressed on its own rather than taking the joint measure down with it.
    pub distinct_rows_all_columns: Option<usize>,
    /// The *joint* measurement stopped early because the file held more classes than the
    /// check keeps. Every count above is then a lower bound, and no verified claim may rest
    /// on them. Set only by the linkable histogram: `distinct_rows_all_columns` going
    /// absent does not make the joint figures incomplete.
    pub measurement_incomplete: bool,
    /// What `unique_rows` would have been with each matched column dropped, ascending by
    /// that count, then by column index.
    ///
    /// The only figure here a reader can act on. `unique_rows` says how exposed the file is;
    /// this says which column to change to fix it, which is the difference between a report
    /// and an alarm.
    ///
    /// Empty when `drop_attribution_incomplete` is set, and also when the file has no matched
    /// column to drop. The two are told apart by that flag rather than by the emptiness of
    /// this list, because "we did not measure" and "there is nothing to drop" are opposite
    /// findings that would otherwise look identical.
    pub drop_column_effects: Vec<DropColumnEffect>,
    /// The attribution was not run, or was stopped, so `drop_column_effects` is empty for a
    /// reason other than there being nothing to say.
    ///
    /// Set when the joint measurement itself is incomplete (there is no baseline to compare
    /// against), when the file has more columns than the attribution tracks, or when the
    /// leave-one-out histograms outgrew their shared budget. Reported rather than hidden: a
    /// reader who is told nothing about which column to drop should know whether that is
    /// because no column would help or because nobody looked.
    pub drop_attribution_incomplete: bool,
}

/// What dropping one column would do to the count of rows that stand alone.
///
/// Exact, not an estimate: the count is read off a second equivalence-class histogram built
/// over the same rows with this column's contribution removed, in the same single pass. That
/// matters because the intuitive estimate is wrong in both directions — dropping a column
/// whose projection is nearly constant changes almost nothing however revealing the column
/// looks, and dropping one of two correlated columns can change almost nothing either.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DropColumnEffect {
    /// `ColumnMetadata::index`, as everywhere else.
    pub column_index: usize,
    /// Rows that would still be alone in their class with this column dropped, every other
    /// column unchanged. Compare against `RowUniquenessSummary::unique_rows`; it can never
    /// be larger, since removing a column only ever merges classes.
    pub unique_rows_without: usize,
}

/// One column the joint measure read, and what it was matched on.
///
/// The pairing is the point, and it must not be split back into two lists of column
/// indices — value-carrying and format-only. Two categories cannot express three kinds of
/// contribution, and the missing one is the partial match, the most common kind on a
/// pseudonymized file: a report built from two lists tells the reader that rows "share their
/// combination of birth_date, email" when what they share is a decade and a domain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchedColumn {
    /// `ColumnMetadata::index`, which is how every other report structure names a column.
    pub column_index: usize,
    pub matched_on: MatchedPart,
    /// Whether every measured row actually carried `matched_on`, or only some of them did.
    ///
    /// `matched_on` is decided once per column, from the column's strategy and detected type
    /// alone — no cell value can change it. The values can still disagree with it: a cell
    /// that does not fit its column's detected shape is pseudonymized generically, and the
    /// projection returns nothing for that row. So a `Timestamp` column where one value in a
    /// hundred parses is `DateDecadeAndTime`, and the finding said the rows "share the decade
    /// and time of birth_date" of ninety-nine rows that carry no decade.
    ///
    /// A `bool` and not a count, because the report needs to know only whether to qualify the
    /// phrase. Quoting "matched on 1 of 100 rows" would invite the reader to weigh a number
    /// that is not the one that matters: those rows were counted as sharing nothing there,
    /// so the arithmetic is already right and only the wording was over-claiming.
    pub matched_every_row: bool,
}

/// What survived a column that an outsider holding the original could match against.
///
/// Written so each variant completes the sentence "rows share **…** with each other", which
/// is what forces the distinction: `WholeValue` licenses naming the column bare, and
/// nothing else does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MatchedPart {
    /// The released cell is the original, or a skeleton of it anyone can derive. The column
    /// may be named on its own.
    WholeValue,
    /// Everything from the last `@`: the employer, not the person.
    EmailDomain,
    /// The decade of the released date, and the time of day exactly. Approximate on the date
    /// half by construction — see `crate::uniqueness::LinkableProjection` — and named as an
    /// approximation so a reader is not told their rows share a birth date when they share a
    /// decade.
    DateDecadeAndTime,
    /// No part of the value, only a format property: a digit count, a separator layout, a
    /// number of name parts, a mask's word and letter counts. Counted like any other,
    /// because a joint measure is where individually weak signals combine, but never named
    /// as though it were the value.
    SurvivingFormat,
    /// Not the cell at all — only whether it was blank, and with which blank token.
    ///
    /// A cell the engine reads as empty is written through verbatim before any strategy
    /// runs, so even a redacted column publishes its missingness pattern, and someone
    /// holding the original record knows which of its fields were blank. Named apart because
    /// "the blank-cell pattern of address" and "address" are wildly different claims, and
    /// because the remedy differs too: no strategy fixes this one.
    BlankPattern,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyReport {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detection_run_summary: Option<DetectionRunSummary>,
    pub direct_identifiers: usize,
    pub quasi_identifiers: usize,
    pub pseudonymized_columns: usize,
    pub smart_replacement_columns: usize,
    pub opaque_token_columns: usize,
    pub masked_columns: usize,
    #[serde(default)]
    pub labelled_columns: usize,
    #[serde(default)]
    pub redacted_columns: usize,
    pub pass_through_columns: usize,
    pub unique_pseudonym_values: usize,
    pub reused_pseudonym_values: usize,
    pub collisions_avoided: usize,
    pub exhausted_pseudonym_pools: usize,
    pub opaque_token_values: usize,
    #[serde(default)]
    pub keyed_token_values: usize,
    #[serde(default)]
    pub keyed_token_columns: Vec<usize>,
    pub smart_replacement_values: usize,
    #[serde(default)]
    pub smart_replacement_rejections: usize,
    #[serde(default)]
    pub smart_replacement_rejection_reasons: Vec<SmartReplacementRejectionCount>,
    pub smart_replacement_fallbacks: usize,
    #[serde(default)]
    pub shape_fallback_values: usize,
    #[serde(default)]
    pub readiness: ReleaseReadiness,
    #[serde(default)]
    pub evidence: Vec<ReleaseEvidenceItem>,
    #[serde(default)]
    pub column_reports: Vec<ColumnReleaseReport>,
    #[serde(default)]
    pub column_value_distributions: Vec<ColumnValueDistribution>,
    #[serde(default)]
    pub row_uniqueness: Option<RowUniquenessSummary>,
    #[serde(default)]
    pub utility_metrics: Vec<UtilityMetric>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseReadiness {
    pub status: ReleaseReadinessStatus,
    pub blockers: Vec<String>,
    pub review_items: Vec<String>,
    pub verified_items: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReleaseReadinessStatus {
    Verified,
    #[default]
    Review,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseEvidenceItem {
    pub id: String,
    pub label: String,
    pub status: ReleaseEvidenceStatus,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReleaseEvidenceStatus {
    Verified,
    Review,
    Blocked,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnReleaseReport {
    pub column_index: usize,
    pub column_name: String,
    pub selected: bool,
    pub detected_type: DataType,
    pub pii_risk: PiiRisk,
    pub strategy: AnonymizationStrategy,
    pub action: String,
    pub status: ReleaseEvidenceStatus,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UtilityMetric {
    pub label: String,
    pub value: String,
    pub status: ReleaseEvidenceStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[cfg(test)]
mod tests;
