use crate::error::{AnonymizerError, Result};
use crate::random::random_string;
use crate::smart::{SmartReplacementMap, value_identity_key};
use crate::types::{AnonymizationStrategy, ColumnValueDistribution, TransformReport};
use rand::Rng;
use std::collections::HashMap;

const GENERATED_ATTEMPT_LIMIT: usize = 512;
pub(super) const TOKEN_CHARSET: &str = "abcdefghijklmnopqrstuvwxyz0123456789";
pub(super) const LETTER_CHARSET: &str = "abcdefghijklmnopqrstuvwxyz";

#[derive(Debug, Clone, Default)]
pub struct TransformState {
    mappers: HashMap<PseudonymDomain, PseudonymMapper>,
    ledgers: HashMap<usize, ColumnValueLedger>,
    smart_replacements: SmartReplacementMap,
    report: TransformReport,
    /// Live entry count across every map in [`Self::mappers`] and [`Self::ledgers`].
    ///
    /// Maintained as the maps are written rather than summed on demand, because the
    /// budget has to be checked once per row: summing `len()` over the ledgers and
    /// both directions of every domain's map is cheap per call and quadratic over a
    /// run, and the figure it produces is one this state can just as easily keep.
    ///
    /// Counted in *entries* rather than distinct values because entries are what the
    /// memory is proportional to, and a distinct value costs a different number of
    /// them per strategy — see [`Self::mapping_entries_per_distinct_value`].
    mapping_entries: usize,
}

impl TransformState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_smart_replacements(smart_replacements: SmartReplacementMap) -> Self {
        let smart_replacement_values = smart_replacements.len();
        let smart_replacement_requests = smart_replacements.requested_values();
        let smart_replacement_rejections = smart_replacements.rejected_values();
        let smart_replacement_rejection_reasons = smart_replacements.rejection_reasons();
        Self {
            mappers: HashMap::new(),
            ledgers: HashMap::new(),
            smart_replacements,
            report: TransformReport {
                smart_replacement_requests,
                smart_replacement_values,
                smart_replacement_rejections,
                smart_replacement_rejection_reasons,
                ..TransformReport::default()
            },
            mapping_entries: 0,
        }
    }

    pub fn report(&self) -> TransformReport {
        TransformReport {
            // Held in the ledgers rather than accumulated into `report` as values
            // arrive, because a distribution is not a running total: distinct and
            // singleton counts are only correct once the last row has been seen.
            column_value_distributions: self.column_value_distributions(),
            ..self.report.clone()
        }
    }

    /// Bytes of resident memory one mapping entry costs, measured.
    ///
    /// Measured on Linux with `VmHWM` read at the end of a one-column,
    /// 1,000,000-row transform — the harness is
    /// `strategies::tests::mapping_budget`, whose ignored tests print these figures
    /// and say how to re-run them. The streaming floor is subtracted, so what
    /// remains is the mapping's own cost:
    ///
    /// | Run | Peak RSS | Entries | Bytes per entry |
    /// | --- | --- | --- | --- |
    /// | Redact, all distinct | 11 MiB | 0 | — (floor) |
    /// | Label, all distinct | 162 MiB | 1,000,000 | 158 |
    /// | Pseudonymize, 250,000 distinct | 127 MiB | 750,000 | 162 |
    /// | Pseudonymize, all distinct | 477 MiB | 3,000,000 | 163 |
    ///
    /// 160 is the middle of that 158–163 band. The band is narrow across two
    /// structures with different value types — a ledger entry is a `String` key with
    /// two `usize`s, a mapper entry is a `String` key with a `String` value — because
    /// at these sizes the cost is dominated by the allocator and hash-table overhead
    /// per entry rather than by the payload, which is what makes one figure per
    /// *entry* meaningful at all.
    ///
    /// Range the data supports: keys of about 16 bytes, entry counts from 750,000 to
    /// 3,000,000, on 64-bit Linux with the system allocator. Not tested: other
    /// platforms or allocators, 32-bit targets, long values (a 200-byte cell pays its
    /// own bytes on top of this overhead, twice over on a pseudonymizing strategy
    /// since the value is also a key of the reverse map), or entry counts far above
    /// 3,000,000, where the figure could drift with hash-table growth steps.
    pub(crate) const APPROXIMATE_BYTES_PER_MAPPING_ENTRY: usize = 160;

    /// Mapping entries a single run may hold before it is refused.
    ///
    /// At [`Self::APPROXIMATE_BYTES_PER_MAPPING_ENTRY`] this is about 5.1 GB, and it
    /// is chosen from both ends:
    ///
    /// - It must not refuse work the app does today. The largest run this project has
    ///   measured is four all-distinct columns of a 63 MB input at about 1.9 GB, which
    ///   is 4 × 1,000,000 × 3 = 12,000,000 entries. The ceiling sits 2.7× above that.
    ///   A single all-distinct pseudonymized column of 1,000,000 rows — the README's
    ///   worst measured single-column case, 477 MiB — is 3,000,000 entries, under a
    ///   tenth of it.
    /// - It must fire before the machine dies. 5.1 GB of mapping still leaves room on
    ///   the 8 GB floor of a current desktop, where the alternative is the OOM killer
    ///   taking the process with no message at all.
    ///
    /// Not tested: machines with less than 8 GB of RAM, and 32-bit builds, where 5.1 GB
    /// is unreachable and this ceiling can never be the thing that fires.
    ///
    /// Smart replacement's own value map is resident alongside this and is not counted
    /// here, but it needs no ceiling of its own: `smart::insert_unique_smart_value`
    /// stops collecting at `SMART_REPLACEMENT_VALUE_CAP_PER_COLUMN`, so it holds at most
    /// 200 values per Local AI column however varied the file is. What a Local AI column
    /// past that cap costs *is* counted here, because every value beyond it falls back to
    /// the pseudonymizing transformers and registers in the mapping like any other.
    ///
    /// Deliberately not a silent cap. Dropping entries at this point would break the
    /// guarantee that repeated source values keep one replacement for the whole run
    /// and would make the privacy report's post-run distribution figures wrong, both
    /// invisibly. Refusing says so.
    pub(crate) const MAPPING_ENTRY_CEILING: usize = 32_000_000;

    /// Entries one distinct value costs on `strategy`.
    ///
    /// The projection in `service::preflight` needs this before a run, and only this
    /// state knows it. Read off the transform paths:
    ///
    /// - `Label` records the value in the column ledger and nothing else: 1.
    /// - `Tokenize`, `Pseudonymize` and `Auto` record the ledger entry and then
    ///   register a pseudonym, which is stored in both directions so a collision can
    ///   be detected from the output side: 3.
    /// - `LocalAi` costs 1 on the path where the model's replacement is used and 3 on
    ///   the fallback, which lands on the pseudonymizing transformers. Reported as 3,
    ///   the upper bound, because a projection that assumed the cheap path would
    ///   under-state exactly the run that then failed.
    /// - `Mask`, `Redact` and `PassThrough` keep no mapping: 0.
    ///
    /// Approximate in two directions, both of which make it an over-estimate for the
    /// columns it is wrong about. `Auto` and `Pseudonymize` on a type that defaults to
    /// pass-through keep nothing, so callers gate on `keeps_consistent_mapping` in
    /// `service::controls` first. And the name transformers map name *tokens*, not
    /// whole values, so a column of full names holds far fewer mapper entries than
    /// distinct values — while a two-token name that misses can touch two domains and
    /// reach 5.
    pub(crate) const fn mapping_entries_per_distinct_value(
        strategy: AnonymizationStrategy,
    ) -> usize {
        match strategy {
            AnonymizationStrategy::Label => 1,
            AnonymizationStrategy::Tokenize
            | AnonymizationStrategy::LocalAi
            | AnonymizationStrategy::Auto
            | AnonymizationStrategy::Pseudonymize => 3,
            AnonymizationStrategy::Mask
            | AnonymizationStrategy::Redact
            | AnonymizationStrategy::PassThrough => 0,
        }
    }

    /// Resident bytes `entries` mapping entries are expected to cost.
    ///
    /// One conversion shared by the mid-run error and the pre-run projection, so the
    /// two cannot quote different memory for the same entry count.
    pub(crate) fn approximate_mapping_bytes(entries: usize) -> u64 {
        (entries as u64).saturating_mul(Self::APPROXIMATE_BYTES_PER_MAPPING_ENTRY as u64)
    }

    /// [`Self::approximate_mapping_bytes`] in whole MB, for messages.
    ///
    /// Decimal MB rather than MiB to agree with the README's memory table, and whole
    /// numbers because the input is a 160-bytes-per-entry estimate: a figure with
    /// decimals would claim a precision the measurement does not have.
    pub(crate) fn approximate_mapping_megabytes(entries: usize) -> u64 {
        Self::approximate_mapping_bytes(entries) / 1_000_000
    }

    /// Entries currently held across every pseudonym domain and every column ledger.
    ///
    /// Public for the same reason [`Self::check_mapping_budget`] is: this state is
    /// handed out to callers that drive [`super::transform_row_with_state`] over their
    /// own input, and the memory it accumulates is theirs to watch.
    pub fn mapping_entries(&self) -> usize {
        self.mapping_entries
    }

    /// Refuses the run once the mapping has outgrown the crate-internal
    /// `MAPPING_ENTRY_CEILING`, whose documentation carries the measurements behind it.
    ///
    /// Called once per row by the run loop in `crate::csv_io`, which is the only place
    /// that can stop the growth: by the time a single row has been transformed the
    /// entries it added are already resident, so this reports the ceiling being
    /// *passed* rather than preventing it — one row's worth of entries beyond the
    /// ceiling is well inside the estimate's own error.
    ///
    /// Public rather than `pub(crate)` because the loop is not always this crate's.
    /// `transform_row_with_state` and `transform_value_with_state` are public and take
    /// this state, so a caller streaming its own rows through them accumulates exactly
    /// the same unbounded mapping and needs the same check available.
    pub fn check_mapping_budget(&self) -> Result<()> {
        self.check_mapping_budget_against(Self::MAPPING_ENTRY_CEILING)
    }

    /// [`Self::check_mapping_budget`] against an arbitrary ceiling.
    ///
    /// Exists so the refusal can be tested without building the several gigabytes of
    /// mapping the real ceiling stands for. It cannot change production behaviour:
    /// the ceiling is an argument rather than a setting, so there is no global for a
    /// test to install and nothing for it to leave behind, and the only caller outside
    /// the tests is [`Self::check_mapping_budget`], which passes the constant.
    pub(crate) fn check_mapping_budget_against(&self, ceiling: usize) -> Result<()> {
        if self.mapping_entries <= ceiling {
            return Ok(());
        }
        Err(AnonymizerError::MappingBudgetExceeded {
            reached: self.mapping_entries,
            ceiling,
            approximate_megabytes: Self::approximate_mapping_megabytes(self.mapping_entries),
        })
    }

    fn mapper_mut(&mut self, domain: PseudonymDomain) -> &mut PseudonymMapper {
        self.mappers.entry(domain).or_default()
    }

    /// Records that `value` was consistently pseudonymized in this column, and
    /// returns the column's 1-based ordinal for it.
    ///
    /// Callers that only need the side effect ignore the return value; a labelled
    /// placeholder uses it to name the value the same way on every row it appears.
    ///
    /// Only the paths that produce a *consistent* pseudonym call this, which is
    /// what makes [`Self::column_value_distributions`] mean what it says. Mask rewrites
    /// each value independently, redact collapses the column to one token, and
    /// pass-through does not transform at all — none of them leaks a distribution,
    /// and counting them would report a cardinality risk against columns that
    /// carry none.
    pub(super) fn record_pseudonymized_value(&mut self, column_index: usize, value: &str) -> usize {
        let ledger = self.ledgers.entry(column_index).or_default();
        // Read the count before the insert: `entry` would have already added this
        // value by the time the closure runs, making the first value ordinal 2.
        let next_ordinal = ledger.values.len() + 1;
        let entry = ledger
            .values
            .entry(value_identity_key(value))
            .or_insert(LedgerEntry {
                ordinal: next_ordinal,
                occurrences: 0,
            });
        entry.occurrences += 1;
        let ordinal = entry.ordinal;
        // A repeat leaves the map the size it was, which is the whole point of the
        // ledger; only a value never seen in this column costs an entry.
        let grew = ledger.values.len() == next_ordinal;
        if grew {
            self.mapping_entries += 1;
        }
        ordinal
    }

    /// The distribution each consistently pseudonymized column exposed, ordered by
    /// column index so a report reads in column order.
    fn column_value_distributions(&self) -> Vec<ColumnValueDistribution> {
        let mut stats = self
            .ledgers
            .iter()
            .map(|(&column_index, ledger)| ColumnValueDistribution {
                column_index,
                distinct_values: ledger.values.len(),
                total_values: ledger.values.values().map(|entry| entry.occurrences).sum(),
                singleton_values: ledger
                    .values
                    .values()
                    .filter(|entry| entry.occurrences == 1)
                    .count(),
                doubleton_values: ledger
                    .values
                    .values()
                    .filter(|entry| entry.occurrences == 2)
                    .count(),
                max_value_occurrences: ledger
                    .values
                    .values()
                    .map(|entry| entry.occurrences)
                    .max()
                    .unwrap_or(0),
            })
            .collect::<Vec<_>>();
        stats.sort_by_key(|item| item.column_index);
        stats
    }

    pub(super) fn assign_from_pool(
        &mut self,
        domain: PseudonymDomain,
        value: &str,
        candidates: &[&str],
        excluded_tokens: &[&str],
    ) -> String {
        let source_key = value_identity_key(value);
        if let Some(existing) = self
            .mapper_mut(domain)
            .source_to_output
            .get(&source_key)
            .cloned()
        {
            self.report.reused_pseudonym_values += 1;
            return existing;
        }

        let start_index = rand::thread_rng().gen_range(0..candidates.len());
        let mut collided = false;

        for offset in 0..candidates.len() {
            let candidate = candidates[(start_index + offset) % candidates.len()];
            if excluded_tokens
                .iter()
                .any(|token| candidate.eq_ignore_ascii_case(token.trim()))
            {
                continue;
            }
            if self.output_is_used_by_other_source(domain, candidate, &source_key) {
                collided = true;
                continue;
            }

            return self.register_assignment(domain, &source_key, candidate.to_string(), collided);
        }

        self.report.exhausted_pseudonym_pools += 1;
        for attempt in 0..GENERATED_ATTEMPT_LIMIT {
            let base = candidates[(start_index + attempt) % candidates.len()];
            let suffix = generated_name_suffix();
            let candidate = format!("{base}{suffix}");
            if excluded_tokens
                .iter()
                .any(|token| candidate.eq_ignore_ascii_case(token.trim()))
            {
                continue;
            }
            if !self.output_is_used_by_other_source(domain, &candidate, &source_key) {
                return self.register_assignment(domain, &source_key, candidate, collided);
            }
        }

        let fallback = format!("{}{}", candidates[start_index], generated_name_suffix());
        self.register_exhausted_assignment(domain, &source_key, fallback)
    }

    /// Maps `source_key` to a freshly generated output, reusing the existing
    /// mapping if this source was already seen.
    ///
    /// `generate` is called repeatedly until it produces a non-empty value that
    /// no other source already owns. It takes no arguments: every generator is
    /// random, so retrying is what breaks a collision, and none of them ever
    /// needed to know which attempt they were on.
    pub(super) fn assign_generated(
        &mut self,
        domain: PseudonymDomain,
        source_key: &str,
        mut generate: impl FnMut() -> String,
    ) -> String {
        if let Some(existing) = self
            .mapper_mut(domain)
            .source_to_output
            .get(source_key)
            .cloned()
        {
            self.report.reused_pseudonym_values += 1;
            return existing;
        }

        let mut collided = false;
        for _ in 0..GENERATED_ATTEMPT_LIMIT {
            let candidate = generate();
            if candidate.is_empty() {
                continue;
            }
            if self.output_is_used_by_other_source(domain, &candidate, source_key) {
                collided = true;
                continue;
            }

            return self.register_assignment(domain, source_key, candidate, collided);
        }

        self.report.exhausted_pseudonym_pools += 1;
        self.register_exhausted_assignment(domain, source_key, generate())
    }

    fn output_is_used_by_other_source(
        &mut self,
        domain: PseudonymDomain,
        candidate: &str,
        source_key: &str,
    ) -> bool {
        self.mapper_mut(domain)
            .output_to_source
            .get(candidate)
            .is_some_and(|owner| owner != source_key)
    }

    fn register_assignment(
        &mut self,
        domain: PseudonymDomain,
        source_key: &str,
        output: String,
        collided: bool,
    ) -> String {
        let mapper = self.mapper_mut(domain);
        let entries_before = mapper.entries();
        mapper
            .source_to_output
            .insert(source_key.to_string(), output.clone());
        mapper
            .output_to_source
            .insert(output.clone(), source_key.to_string());
        let entries_added = mapper.entries() - entries_before;
        self.mapping_entries += entries_added;
        self.report.unique_pseudonym_values += 1;
        if collided {
            self.report.collisions_avoided += 1;
        }
        if domain == PseudonymDomain::OpaqueToken {
            self.report.opaque_token_values += 1;
        }
        output
    }

    fn register_exhausted_assignment(
        &mut self,
        domain: PseudonymDomain,
        source_key: &str,
        output: String,
    ) -> String {
        let mapper = self.mapper_mut(domain);
        let entries_before = mapper.entries();
        mapper
            .source_to_output
            .insert(source_key.to_string(), output.clone());
        mapper
            .output_to_source
            .entry(output.clone())
            .or_insert_with(|| source_key.to_string());
        let entries_added = mapper.entries() - entries_before;
        self.mapping_entries += entries_added;
        self.report.unique_pseudonym_values += 1;
        if domain == PseudonymDomain::OpaqueToken {
            self.report.opaque_token_values += 1;
        }
        output
    }

    pub(super) fn smart_replacement(&mut self, column_index: usize, value: &str) -> Option<String> {
        self.smart_replacements
            .get(column_index, value)
            .map(ToString::to_string)
    }

    pub(super) fn record_smart_fallback(&mut self) {
        self.report.smart_replacement_fallbacks += 1;
    }

    pub(super) fn record_shape_fallback(&mut self) {
        self.report.shape_fallback_values += 1;
    }
}

#[derive(Debug, Clone, Default)]
struct PseudonymMapper {
    source_to_output: HashMap<String, String>,
    output_to_source: HashMap<String, String>,
}

impl PseudonymMapper {
    /// Entries held in both directions.
    ///
    /// Both, because both are resident: the reverse map is what lets an assignment
    /// tell "this output is already mine" from "this output belongs to another
    /// source", so it is not an index that could be dropped. Measured as a length
    /// rather than assumed to be twice the forward map, because an exhausted pool
    /// falls back to an output that may already be owned, leaving the two sides
    /// different sizes.
    fn entries(&self) -> usize {
        self.source_to_output.len() + self.output_to_source.len()
    }
}

/// What a column's consistent pseudonyms reveal about its source values.
///
/// A consistent pseudonym preserves equality, which is the point — it keeps a
/// dataset joinable. The cost is that it also preserves the *shape* of the value
/// distribution, and the shape is enough to attack the mapping. A column with
/// few distinct values can be relabelled by anyone who knows how the real field
/// is distributed, and a value occurring exactly once yields a pseudonym covering
/// exactly one row, which singles that record out however opaque the token looks.
///
/// Keyed separately from [`PseudonymMapper`] because the mappers are keyed by
/// domain, not by column: `PseudonymDomain::GenericString` deliberately shares one
/// key space across every generic column, so no per-column count can be recovered
/// from it.
#[derive(Debug, Clone, Default)]
struct ColumnValueLedger {
    values: HashMap<String, LedgerEntry>,
}

#[derive(Debug, Clone, Copy)]
struct LedgerEntry {
    /// 1-based order of first appearance within the column. 1-based because it is
    /// user-facing in a labelled placeholder, where `[NOTES_1]` reads as the first
    /// distinct value and `[NOTES_0]` reads as a bug.
    ordinal: usize,
    occurrences: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum PseudonymDomain {
    EmailLocal,
    Uuid,
    Timestamp,
    NumericId,
    NumericValue,
    Phone,
    FirstName,
    LastName,
    GenericString,
    OpaqueToken,
}

fn generated_name_suffix() -> String {
    random_string(4, LETTER_CHARSET)
}
