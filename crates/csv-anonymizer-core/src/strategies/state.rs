use crate::error::{AnonymizerError, Result};
use crate::random::random_string;
use crate::smart::{SmartReplacementMap, value_identity_key};
use crate::types::{
    AnonymizationStrategy, ColumnMetadata, ColumnValueDistribution, TransformReport,
};
use crate::uniqueness::RowUniquenessTracker;
use rand::Rng;
use std::collections::{HashMap, HashSet, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};

const GENERATED_ATTEMPT_LIMIT: usize = 512;
pub(super) const TOKEN_CHARSET: &str = "abcdefghijklmnopqrstuvwxyz0123456789";
pub(super) const LETTER_CHARSET: &str = "abcdefghijklmnopqrstuvwxyz";

/// Validated run-only key. No serde implementation is intentional: this type cannot
/// be written into settings, prepared snapshots, job status, or privacy reports.
#[derive(Clone, PartialEq, Eq)]
pub struct TokenizationKey([u8; 32]);

impl std::fmt::Debug for TokenizationKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("TokenizationKey([REDACTED])")
    }
}

impl TokenizationKey {
    pub fn parse_hex(value: &str) -> Result<Self> {
        if value.len() != 64 {
            return Err(AnonymizerError::InvalidTokenizationKey);
        }
        let mut bytes = [0_u8; 32];
        for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
            let pair =
                std::str::from_utf8(pair).map_err(|_| AnonymizerError::InvalidTokenizationKey)?;
            bytes[index] = u8::from_str_radix(pair, 16)
                .map_err(|_| AnonymizerError::InvalidTokenizationKey)?;
        }
        Ok(Self(bytes))
    }

    pub(super) fn token_for(&self, column_index: usize, column_name: &str, value: &str) -> String {
        let mut hasher = blake3::Hasher::new_keyed(&self.0);
        hasher.update(b"csv-anonymizer/keyed-token/v1\0");
        hasher.update(&column_index.to_le_bytes());
        hasher.update(column_name.as_bytes());
        hasher.update(b"\0");
        hasher.update(value_identity_key(value).as_bytes());
        format!("tok_{}", &hasher.finalize().to_hex()[..24])
    }
}

fn insert_bounded_fingerprint(
    set: &mut HashSet<u128>,
    value: &str,
    ceiling: usize,
    incomplete: &mut bool,
) {
    let fingerprint = value_fingerprint(value);
    if set.contains(&fingerprint) {
        return;
    }
    if set.len() >= ceiling {
        *incomplete = true;
        return;
    }
    set.insert(fingerprint);
}

fn value_fingerprint(value: &str) -> u128 {
    fn half(value: &str, domain: u8) -> u64 {
        let mut hasher = DefaultHasher::new();
        domain.hash(&mut hasher);
        value.hash(&mut hasher);
        hasher.finish()
    }
    u128::from(half(value, 0)) << 64 | u128::from(half(value, 1))
}

#[derive(Debug, Clone, Default)]
pub struct TransformState {
    mappers: HashMap<PseudonymDomain, PseudonymMapper>,
    ledgers: HashMap<usize, ColumnValueLedger>,
    smart_replacements: SmartReplacementMap,
    /// Deliberately outside the `mapping_entries` budget below. Its entries are real
    /// memory and are bounded, but by a ceiling of its own with a softer failure: see
    /// [`Self::record_released_row`].
    row_uniqueness: RowUniquenessTracker,
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
    residual_source_fingerprints: HashSet<u128>,
    residual_output_fingerprints: HashSet<u128>,
    residual_audit_incomplete: bool,
    tokenization_key: Option<TokenizationKey>,
}

impl TransformState {
    pub fn new() -> Self {
        Self::default()
    }

    /// A state carrying `smart_replacements` only when they hold something to carry.
    ///
    /// Every transform builds its state this way, so what counts as worth carrying —
    /// [`SmartReplacementMap::if_active`] — is decided once. Split across the call
    /// sites, a state built from an inert map would open a Local AI section in the
    /// report of a run that used none, and one built from a map with rejections
    /// dropped would omit the guard's refusals from a run that had them.
    pub fn with_smart_replacements_if_active(smart_replacements: SmartReplacementMap) -> Self {
        smart_replacements
            .if_active()
            .map_or_else(Self::new, Self::with_smart_replacements)
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
            row_uniqueness: RowUniquenessTracker::default(),
            report: TransformReport {
                smart_replacement_requests,
                smart_replacement_values,
                smart_replacement_rejections,
                smart_replacement_rejection_reasons,
                ..TransformReport::default()
            },
            mapping_entries: 0,
            residual_source_fingerprints: HashSet::new(),
            residual_output_fingerprints: HashSet::new(),
            residual_audit_incomplete: false,
            tokenization_key: None,
        }
    }

    pub fn with_tokenization_key(mut self, key: Option<TokenizationKey>) -> Self {
        self.tokenization_key = key;
        self
    }

    pub(super) fn keyed_token(
        &self,
        column_index: usize,
        column_name: &str,
        value: &str,
    ) -> Option<String> {
        self.tokenization_key
            .as_ref()
            .map(|key| key.token_for(column_index, column_name, value))
    }

    pub(super) fn record_keyed_token(&mut self, column_index: usize, value: &str) {
        // `record_pseudonymized_value` runs immediately before token generation and
        // already owns the per-column distinct-value ledger. Reuse it instead of
        // retaining a second unbounded set outside the mapping budget.
        let is_first_occurrence = self
            .ledgers
            .get(&column_index)
            .and_then(|ledger| ledger.values.get(&value_identity_key(value)))
            .is_some_and(|entry| entry.occurrences == 1);
        if is_first_occurrence {
            self.report.opaque_token_values += 1;
            self.report.keyed_token_values += 1;
            if !self.report.keyed_token_columns.contains(&column_index) {
                self.report.keyed_token_columns.push(column_index);
            }
        }
    }

    pub fn report(&self) -> TransformReport {
        let residual_audit_matches = self
            .residual_source_fingerprints
            .intersection(&self.residual_output_fingerprints)
            .count();
        TransformReport {
            // Held in the ledgers rather than accumulated into `report` as values
            // arrive, because a distribution is not a running total: distinct and
            // singleton counts are only correct once the last row has been seen.
            column_value_distributions: self.column_value_distributions(),
            // `None` until a row has actually been recorded, which is how the paths with
            // no rows to speak of — unstructured text, a single pasted value — report an
            // absent measurement rather than a clean one.
            row_uniqueness: self.row_uniqueness.summary(),
            residual_audit_source_values: self.residual_source_fingerprints.len(),
            residual_audit_output_values: self.residual_output_fingerprints.len(),
            residual_audit_matches,
            residual_audit_incomplete: self.residual_audit_incomplete,
            ..self.report.clone()
        }
    }

    /// Records one transformed row against the joint re-identifiability measure.
    ///
    /// Separate from `record_pseudonymized_value` and called once per row rather
    /// than once per value, because the whole point of this figure is that it cannot be
    /// computed a column at a time.
    ///
    /// Its memory is bounded by a ceiling of its own rather than by
    /// `MAPPING_ENTRY_CEILING`, for two reasons. Folding it in would start
    /// refusing runs that stream fine today — a redact-only run costs zero mapping
    /// entries, and charging it one per row would make a large redaction fail in the name
    /// of a report. And the two have opposite failure modes: dropping mapping entries
    /// silently corrupts the output, so that ceiling must refuse the run, while dropping
    /// uniqueness classes costs only a figure, so this one stops measuring, says so
    /// through `measurement_incomplete`, and lets the run finish.
    pub fn record_released_row(&mut self, released: &[String], columns: &[ColumnMetadata]) {
        self.row_uniqueness.record_row(released, columns);
    }

    pub fn record_unchanged_sensitive_values(
        &mut self,
        source: &[String],
        released: &[String],
        columns: &[ColumnMetadata],
    ) {
        for (index, ((source, released), column)) in
            source.iter().zip(released).zip(columns).enumerate()
        {
            if !column.is_selected
                || !column.pii_risk.is_elevated()
                || column.strategy == AnonymizationStrategy::PassThrough
                || (matches!(
                    column.strategy,
                    AnonymizationStrategy::Auto | AnonymizationStrategy::Pseudonymize
                ) && column.detected_type.uses_default_pass_through())
                || crate::detection::is_empty_value(source.trim())
                || source.trim() != released.trim()
            {
                continue;
            }
            self.report.unchanged_sensitive_values += 1;
            if !self.report.unchanged_sensitive_columns.contains(&index) {
                self.report.unchanged_sensitive_columns.push(index);
            }
        }
    }

    /// Broad post-transform audit over value fingerprints. Unlike the exact same-cell
    /// guard above, this catches a protected source value surviving in any output column.
    /// It retains no source text and stops collecting rather than growing without bound.
    pub fn record_residual_audit(
        &mut self,
        source: &[String],
        released: &[String],
        columns: &[ColumnMetadata],
    ) {
        const VALUE_CEILING: usize = 1_000_000;
        for (value, column) in source.iter().zip(columns) {
            if column.is_selected
                && column.pii_risk.is_elevated()
                && column.strategy != AnonymizationStrategy::PassThrough
                && !(matches!(
                    column.strategy,
                    AnonymizationStrategy::Auto | AnonymizationStrategy::Pseudonymize
                ) && column.detected_type.uses_default_pass_through())
                && !crate::detection::is_empty_value(value)
            {
                insert_bounded_fingerprint(
                    &mut self.residual_source_fingerprints,
                    value.trim(),
                    VALUE_CEILING,
                    &mut self.residual_audit_incomplete,
                );
            }
        }
        for value in released {
            if !crate::detection::is_empty_value(value) {
                insert_bounded_fingerprint(
                    &mut self.residual_output_fingerprints,
                    value.trim(),
                    VALUE_CEILING,
                    &mut self.residual_audit_incomplete,
                );
            }
        }
    }

    /// Bytes of resident memory one mapping entry costs, measured.
    ///
    /// Peak-RSS measurements over one-column, 1,000,000-row transforms put the cost in
    /// a 158–163 byte band; 160 is its middle. Measured only on 64-bit Linux with the
    /// system allocator, short keys, and 750,000–3,000,000 entries — other platforms,
    /// allocators and long values are not covered.
    /// See docs/calibration.md#approximate-bytes-per-mapping-entry for the measurements behind this.
    pub(crate) const APPROXIMATE_BYTES_PER_MAPPING_ENTRY: usize = 160;

    /// Mapping entries a single run may hold before it is refused.
    ///
    /// About 5.1 GB at [`Self::APPROXIMATE_BYTES_PER_MAPPING_ENTRY`]: 2.7× above the
    /// largest run this project has measured, and still under the 8 GB floor of a
    /// current desktop. Not tested on machines with less than 8 GB, or on 32-bit
    /// builds where 5.1 GB is unreachable and this ceiling can never fire.
    /// See docs/calibration.md#mapping-entry-ceiling for the measurements behind this.
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

    /// A conservative per-machine ceiling: at most 20% of installed RAM and never
    /// above the calibrated absolute ceiling. Installed rather than currently free
    /// memory keeps preflight and execution stable while other processes fluctuate.
    pub(crate) fn runtime_mapping_entry_ceiling() -> usize {
        let system = sysinfo::System::new_with_specifics(
            sysinfo::RefreshKind::nothing().with_memory(sysinfo::MemoryRefreshKind::everything()),
        );
        let total_memory = system.total_memory();
        if total_memory == 0 {
            return Self::MAPPING_ENTRY_CEILING;
        }
        let memory_budget = total_memory / 5;
        let entries = memory_budget / Self::APPROXIMATE_BYTES_PER_MAPPING_ENTRY as u64;
        usize::try_from(entries)
            .unwrap_or(Self::MAPPING_ENTRY_CEILING)
            .clamp(1, Self::MAPPING_ENTRY_CEILING)
    }

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
    /// Test-only: production reads the count through [`Self::check_mapping_budget_against`],
    /// which is the only place the number is acted on rather than merely observed.
    #[cfg(test)]
    pub(crate) fn mapping_entries(&self) -> usize {
        self.mapping_entries
    }

    /// Refuses the run once the mapping has outgrown the crate-internal
    /// `MAPPING_ENTRY_CEILING`, whose documentation points at the measurements behind it.
    ///
    /// Test-only. The run loop in `crate::csv_io` calls
    /// [`Self::check_mapping_budget_against`] with the constant directly; this exists so a
    /// test can assert the real ceiling is the one wired up, without naming the figure twice.
    #[cfg(test)]
    pub(crate) fn check_mapping_budget(&self) -> Result<()> {
        self.check_mapping_budget_against(Self::MAPPING_ENTRY_CEILING)
    }

    /// Refuses the run once the mapping has outgrown `ceiling`.
    ///
    /// This is the production entry point: the run loop in `crate::csv_io` calls it once per
    /// row with [`Self::MAPPING_ENTRY_CEILING`]. That loop is the only place that can stop the
    /// growth, and even it reports the ceiling being *passed* rather than preventing it — by
    /// the time a row has been transformed its entries are already resident, which is well
    /// inside the estimate's own error.
    ///
    /// The ceiling is an argument rather than a constant read here so the refusal can be
    /// tested without building the several gigabytes the real one stands for. That cannot
    /// change production behaviour: there is no global for a test to install and nothing for
    /// it to leave behind.
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
