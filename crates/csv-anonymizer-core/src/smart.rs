use crate::csv_io::validate_file;
use crate::detection::is_empty_value;
use crate::error::{AnonymizerError, Result, csv_error};
use crate::process_control::check_canceled;
use crate::types::{
    ColumnMetadata, ProcessControl, SmartReplacementEntry, SmartReplacementRejectionCount,
    SmartReplacementRejectionReason,
};
use csv::{ReaderBuilder, Trim};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

const SMART_REPLACEMENT_BATCH_SIZE: usize = 20;
pub(crate) const SMART_REPLACEMENT_VALUE_CAP_PER_COLUMN: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartReplacement {
    pub original: String,
    pub replacement: String,
}

#[derive(Debug, Clone, Copy)]
pub struct SmartReplacementRequest<'a> {
    pub column: &'a ColumnMetadata,
    pub values: &'a [String],
}

pub trait SmartReplacementProvider {
    fn generate_replacements(
        &mut self,
        request: SmartReplacementRequest<'_>,
    ) -> Result<Vec<SmartReplacement>>;
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SmartReplacementMap {
    replacements: HashMap<SmartReplacementKey, StoredSmartReplacement>,
    requested_values: usize,
    rejected_values: usize,
    rejection_counts: BTreeMap<SmartReplacementRejectionReason, usize>,
}

impl SmartReplacementMap {
    pub fn len(&self) -> usize {
        self.replacements.len()
    }

    pub fn is_empty(&self) -> bool {
        self.replacements.is_empty()
    }

    pub fn has_activity(&self) -> bool {
        !self.replacements.is_empty() || self.requested_values > 0 || self.rejected_values > 0
    }

    /// This map when it carries something a run has to account for, `None` otherwise.
    ///
    /// Activity is more than "holds replacements": a map that only *requested* values,
    /// or only had them rejected, still has to reach the report, or a run whose Local
    /// AI output the leak guard refused would be described as having used no Local AI
    /// at all. Every caller that decides whether to carry a map asks the question this
    /// way, so it is asked in one place — spelled out at each site, one site could
    /// keep the old, narrower test and quietly drop the rejection counts.
    pub(crate) fn if_active(self) -> Option<Self> {
        self.has_activity().then_some(self)
    }

    pub fn requested_values(&self) -> usize {
        self.requested_values
    }

    pub fn rejected_values(&self) -> usize {
        self.rejected_values
    }

    pub fn rejection_reasons(&self) -> Vec<SmartReplacementRejectionCount> {
        self.rejection_counts
            .iter()
            .map(|(reason, count)| SmartReplacementRejectionCount {
                reason: *reason,
                count: *count,
            })
            .collect()
    }

    pub fn insert(&mut self, column_index: usize, original: &str, replacement: impl Into<String>) {
        self.replacements.insert(
            SmartReplacementKey::new(column_index, original),
            StoredSmartReplacement {
                column_index,
                original: original.to_string(),
                replacement: replacement.into(),
            },
        );
    }

    pub fn contains(&self, column_index: usize, value: &str) -> bool {
        self.replacements
            .contains_key(&SmartReplacementKey::new(column_index, value))
    }

    pub fn get(&self, column_index: usize, value: &str) -> Option<&str> {
        self.replacements
            .get(&SmartReplacementKey::new(column_index, value))
            .map(|replacement| replacement.replacement.as_str())
    }

    pub fn from_entries(entries: &[SmartReplacementEntry]) -> Self {
        let mut entries_by_column = BTreeMap::<usize, Vec<SmartReplacement>>::new();
        for entry in entries {
            entries_by_column
                .entry(entry.column_index)
                .or_default()
                .push(SmartReplacement {
                    original: entry.original.clone(),
                    replacement: entry.replacement.clone(),
                });
        }

        let mut map = Self::default();
        for (column_index, replacements) in entries_by_column {
            let expected_values = replacements
                .iter()
                .map(|replacement| replacement.original.clone())
                .collect::<Vec<_>>();
            let column_source_keys = source_keys(&expected_values);
            let mut used_outputs = BTreeSet::new();
            let validation = validated_replacements(
                &expected_values,
                &column_source_keys,
                replacements,
                &mut used_outputs,
            );
            map.record_request_batch(expected_values.len(), &validation.rejection_reasons);
            for (original, replacement) in validation.accepted {
                map.insert(column_index, &original, replacement);
            }
        }
        map
    }

    pub fn to_entries(&self) -> Vec<SmartReplacementEntry> {
        let mut entries = self
            .replacements
            .values()
            .map(|replacement| SmartReplacementEntry {
                column_index: replacement.column_index,
                original: replacement.original.clone(),
                replacement: replacement.replacement.clone(),
            })
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| {
            left.column_index
                .cmp(&right.column_index)
                .then_with(|| left.original.cmp(&right.original))
        });
        entries
    }

    fn record_request_batch(
        &mut self,
        requested: usize,
        rejection_reasons: &[SmartReplacementRejectionReason],
    ) {
        self.requested_values += requested;
        self.rejected_values += rejection_reasons.len();
        for reason in rejection_reasons {
            *self.rejection_counts.entry(*reason).or_default() += 1;
        }
    }

    fn output_keys_for_column(&self, column_index: usize) -> BTreeSet<String> {
        self.replacements
            .values()
            .filter(|replacement| replacement.column_index == column_index)
            .map(|replacement| value_identity_key(&replacement.replacement))
            .collect()
    }

    /// The source values this map already holds for `column_index`, identity-keyed.
    ///
    /// These come from a preview that ran earlier in the same session, so they are
    /// real values of the same column and belong in the cross-value leak set even
    /// though the current request will not ask about them again: a replacement
    /// generated now that carries one of them still publishes that person's value
    /// against another record. Leaving them out would make the guard weaker the more
    /// work the preview had already done, which is exactly backwards.
    fn source_keys_for_column(&self, column_index: usize) -> BTreeSet<String> {
        self.replacements
            .values()
            .filter(|replacement| replacement.column_index == column_index)
            .map(|replacement| value_identity_key(&replacement.original))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StoredSmartReplacement {
    column_index: usize,
    original: String,
    replacement: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SmartReplacementKey {
    column_index: usize,
    normalized_value: String,
}

impl SmartReplacementKey {
    fn new(column_index: usize, value: &str) -> Self {
        Self {
            column_index,
            normalized_value: value_identity_key(value),
        }
    }
}

/// The replacements a preview already produced, when the pending run can reuse them.
///
/// Two conditions, and both are load-bearing. The entries have to carry activity, or
/// an empty map would stand in for "preview produced nothing" and suppress the
/// generation the run still needs. And the selection has to still contain a Local AI
/// column, or replacements computed for a column the user has since deselected would
/// be carried into a run that no longer has anywhere to apply them.
///
/// Asked in one place because the file run, the paste runs and preflight all have to
/// reach the same verdict: preflight decides on this basis whether the run needs Local
/// AI at all, so a preflight that answered differently from the run would clear a run
/// that then demands a model the user was told it would not need.
pub(crate) fn reusable_preview_smart_replacements(
    preview_smart_replacements: &[SmartReplacementEntry],
    selected_metadata: &[ColumnMetadata],
) -> Option<SmartReplacementMap> {
    let replacements = SmartReplacementMap::from_entries(preview_smart_replacements);
    if !has_smart_replacement_columns(selected_metadata) {
        return None;
    }
    replacements.if_active()
}

pub fn has_smart_replacement_columns(columns: &[ColumnMetadata]) -> bool {
    columns.iter().any(|column| {
        column.is_selected && column.strategy == crate::types::AnonymizationStrategy::LocalAi
    })
}

pub fn prepare_smart_replacements_from_rows(
    rows: &[Vec<String>],
    columns: &[ColumnMetadata],
    existing: Option<&SmartReplacementMap>,
    provider: Option<&mut dyn SmartReplacementProvider>,
) -> Result<SmartReplacementMap> {
    let batches = collect_unique_values_from_rows(rows, columns);
    build_replacement_map(columns, batches, existing, provider)
}

pub fn prepare_smart_replacements_from_csv(
    file_path: &Path,
    columns: &[ColumnMetadata],
    control: Option<&mut ProcessControl<'_>>,
    existing: Option<&SmartReplacementMap>,
    provider: Option<&mut dyn SmartReplacementProvider>,
) -> Result<SmartReplacementMap> {
    validate_file(file_path)?;
    let batches = collect_unique_values_from_csv(file_path, columns, control)?;
    build_replacement_map(columns, batches, existing, provider)
}

pub fn missing_smart_replacement_values_from_csv(
    file_path: &Path,
    columns: &[ColumnMetadata],
    existing: Option<&SmartReplacementMap>,
) -> Result<bool> {
    validate_file(file_path)?;
    let batches = collect_unique_values_from_csv(file_path, columns, None)?;
    Ok(has_missing_smart_replacement_values(batches, existing))
}

/// Accumulates the distinct non-empty values of each Local AI column.
///
/// The row source differs between the paste path (rows already in memory) and
/// the file path (streamed records), but which values are collected must not:
/// the preview and the final run have to request replacements for the same set,
/// or the run discards the preview's work and re-queries the model.
struct SmartValueCollector {
    values_by_column: BTreeMap<usize, BTreeSet<String>>,
}

impl SmartValueCollector {
    fn new(columns: &[ColumnMetadata]) -> Self {
        Self {
            values_by_column: selected_smart_columns(columns)
                .map(|column| (column.index, BTreeSet::new()))
                .collect(),
        }
    }

    fn is_empty(&self) -> bool {
        self.values_by_column.is_empty()
    }

    fn push_row<'a>(&mut self, cell_at: impl Fn(usize) -> Option<&'a str>) {
        for (column_index, values) in &mut self.values_by_column {
            let Some(value) = cell_at(*column_index) else {
                continue;
            };
            if !is_empty_value(value) {
                insert_unique_smart_value(values, value);
            }
        }
    }

    fn into_batches(self) -> BTreeMap<usize, Vec<String>> {
        self.values_by_column
            .into_iter()
            .map(|(index, values)| (index, values.into_iter().collect()))
            .collect()
    }
}

fn collect_unique_values_from_rows(
    rows: &[Vec<String>],
    columns: &[ColumnMetadata],
) -> BTreeMap<usize, Vec<String>> {
    let mut collector = SmartValueCollector::new(columns);
    if collector.is_empty() {
        return BTreeMap::new();
    }

    for row in rows {
        collector.push_row(|index| row.get(index).map(String::as_str));
    }

    collector.into_batches()
}

fn collect_unique_values_from_csv(
    file_path: &Path,
    columns: &[ColumnMetadata],
    mut control: Option<&mut ProcessControl<'_>>,
) -> Result<BTreeMap<usize, Vec<String>>> {
    let mut collector = SmartValueCollector::new(columns);
    if collector.is_empty() {
        return Ok(BTreeMap::new());
    }

    let mut reader = ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .trim(Trim::All)
        .from_path(file_path)
        .map_err(csv_error)?;
    let mut header_processed = false;

    for result in reader.records() {
        let record = result.map_err(csv_error)?;
        if !header_processed {
            header_processed = true;
            continue;
        }

        check_canceled(&mut control)?;
        if record.iter().all(|value| value.trim().is_empty()) {
            continue;
        }

        collector.push_row(|index| record.get(index));
    }

    Ok(collector.into_batches())
}

fn build_replacement_map(
    columns: &[ColumnMetadata],
    batches: BTreeMap<usize, Vec<String>>,
    existing: Option<&SmartReplacementMap>,
    mut provider: Option<&mut dyn SmartReplacementProvider>,
) -> Result<SmartReplacementMap> {
    if batches.is_empty() {
        return Ok(SmartReplacementMap::default());
    }

    let mut map = existing.cloned().unwrap_or_default();
    for (column_index, values) in batches {
        let missing_values = values
            .into_iter()
            .filter(|value| !map.contains(column_index, value))
            .collect::<Vec<_>>();
        if missing_values.is_empty() {
            continue;
        }
        let Some(column) = find_column_by_index(column_index, columns) else {
            continue;
        };
        let Some(provider) = provider.as_deref_mut() else {
            return Err(AnonymizerError::SmartReplacement(
                "Smart replacement needs Local AI to be ready. Enable Local AI, make sure Ollama is running, and download Gemma 3 4B before trying again."
                    .to_string(),
            ));
        };
        let mut used_outputs = map.output_keys_for_column(column_index);
        // Built once for the whole column, not per chunk. A column carries up to
        // `SMART_REPLACEMENT_VALUE_CAP_PER_COLUMN` values and is asked about in
        // batches of `SMART_REPLACEMENT_BATCH_SIZE`, so a chunk-scoped leak set never
        // compared the eleventh value's replacement against the first value's source:
        // a model that answered "Sophie" with "Emma" — a real name it had been shown
        // in an earlier prompt — was accepted, publishing Emma's name against
        // Sophie's record. The set is honest but not exhaustive: values dropped by
        // the per-column cap were never collected, so a replacement echoing one of
        // those is not detectable here.
        let mut column_source_keys = map.source_keys_for_column(column_index);
        column_source_keys.extend(source_keys(&missing_values));

        for chunk in missing_values.chunks(SMART_REPLACEMENT_BATCH_SIZE) {
            let requested = chunk.len();
            let replacements = provider.generate_replacements(SmartReplacementRequest {
                column,
                values: chunk,
            })?;
            let validation =
                validated_replacements(chunk, &column_source_keys, replacements, &mut used_outputs);
            map.record_request_batch(requested, &validation.rejection_reasons);
            for (original, replacement) in validation.accepted {
                map.insert(column_index, &original, replacement);
            }
        }
    }

    Ok(map)
}

fn has_missing_smart_replacement_values(
    batches: BTreeMap<usize, Vec<String>>,
    existing: Option<&SmartReplacementMap>,
) -> bool {
    batches.into_iter().any(|(column_index, values)| {
        values
            .iter()
            .any(|value| !existing.is_some_and(|map| map.contains(column_index, value)))
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ValidatedSmartReplacements {
    accepted: Vec<(String, String)>,
    rejection_reasons: Vec<SmartReplacementRejectionReason>,
}

/// Checks one prompt's answers, against two deliberately different sets.
///
/// `expected_values` is what *this* prompt asked about and decides whether a returned
/// original is one of them: an answer naming a value from another prompt is a model
/// error and stays `UnexpectedOriginal`, so this set must stay request-scoped.
/// `column_source_keys` is what a replacement is *checked against* for the cross-value
/// leak, and covers the whole column, because a value's real danger is being echoed
/// into a row the model was asked about at some other time. Conflating the two either
/// blinds the leak check to earlier chunks or starts accepting originals nobody asked
/// for.
fn validated_replacements(
    expected_values: &[String],
    column_source_keys: &BTreeSet<String>,
    replacements: Vec<SmartReplacement>,
    used_outputs: &mut BTreeSet<String>,
) -> ValidatedSmartReplacements {
    let expected_by_key = expected_values
        .iter()
        .map(|value| (value_identity_key(value), value.clone()))
        .collect::<HashMap<_, _>>();
    let mut seen_expected_originals = BTreeSet::new();
    let mut accepted_originals = BTreeSet::new();
    let mut accepted = Vec::new();
    let mut rejection_reasons = Vec::new();

    for replacement in replacements {
        let original_key = value_identity_key(&replacement.original);
        let Some(original) = expected_by_key.get(&original_key) else {
            rejection_reasons.push(SmartReplacementRejectionReason::UnexpectedOriginal);
            continue;
        };
        seen_expected_originals.insert(original_key.clone());
        if accepted_originals.contains(&original_key) {
            rejection_reasons.push(SmartReplacementRejectionReason::DuplicateOriginal);
            continue;
        }
        let cleaned = replacement.replacement.trim();
        if let Some(reason) = invalid_replacement_reason(original, cleaned, column_source_keys) {
            rejection_reasons.push(reason);
            continue;
        }
        let output_key = value_identity_key(cleaned);
        if !used_outputs.insert(output_key) {
            rejection_reasons.push(SmartReplacementRejectionReason::DuplicateOutput);
            continue;
        }
        accepted_originals.insert(original_key);
        accepted.push((original.clone(), cleaned.to_string()));
    }

    for value in expected_values {
        let key = value_identity_key(value);
        if !accepted_originals.contains(&key) && !seen_expected_originals.contains(&key) {
            rejection_reasons.push(SmartReplacementRejectionReason::MissingOutput);
        }
    }

    ValidatedSmartReplacements {
        accepted,
        rejection_reasons,
    }
}

/// Why `replacement` may not stand in for `original`, if it may not.
///
/// `column_source_keys` is every source value known for the whole column, keyed by
/// [`value_identity_key`]. It is needed because the dangerous case is not only a
/// replacement that echoes its *own* original — it is one that carries a *different*
/// row's value, which would publish that person's data against the wrong record. A
/// model handed a column's values across several prompts can copy any of them into
/// any slot, and the reverse-map dedup in `strategies::state` cannot see it: each
/// source still maps to a distinct output, so nothing downstream looks wrong.
///
/// Self-comparisons are ordered first so the more specific reason wins: a replacement
/// equal to its own original reports `SameAsOriginal`, never `MatchesOtherOriginal`.
fn invalid_replacement_reason(
    original: &str,
    replacement: &str,
    column_source_keys: &BTreeSet<String>,
) -> Option<SmartReplacementRejectionReason> {
    if replacement.is_empty() {
        return Some(SmartReplacementRejectionReason::EmptyOutput);
    }

    let original_key = value_identity_key(original);
    let replacement_key = value_identity_key(replacement);
    // Compared as identity keys rather than with `eq_ignore_ascii_case`, so that a
    // replacement differing from its original only in non-ASCII case or in internal
    // spacing is still recognized as the original coming straight back.
    if replacement_key == original_key {
        return Some(SmartReplacementRejectionReason::SameAsOriginal);
    }
    if replacement
        .chars()
        .any(|character| character.is_control() && character != '\t')
    {
        return Some(SmartReplacementRejectionReason::ControlCharacter);
    }

    if contains_source_value(&replacement_key, &original_key) {
        return Some(SmartReplacementRejectionReason::ContainsOriginal);
    }

    if column_source_keys
        .iter()
        .filter(|key| *key != &original_key)
        .any(|key| contains_source_value(&replacement_key, key))
    {
        return Some(SmartReplacementRejectionReason::MatchesOtherOriginal);
    }

    None
}

/// Whether `replacement_key` carries `source_key`, both already identity-keyed.
///
/// Exact equality counts at any length, because a two-character value reproduced
/// whole is still that value — even for a closed domain like country or gender, where
/// it costs utility, republishing one row's real value against another row is a
/// genuine quasi-identifier leak and the pseudonymization fallback is the right
/// answer.
///
/// Containment is the arm that has to be careful, because it now runs against every
/// source value of the column rather than against a single original: a bare substring
/// test at a three-character floor rejects the honest `Janneke Visser` because some
/// other row of the same column happens to be called `Jan`. So containment only counts
/// when the match sits on a token boundary, which keeps the cases that matter —
/// `alice@corp.com` inside a longer address, a name inside `Anne-Marie` or `O'Brien`,
/// an id inside `ID-AB12345-X` — while letting a short source key that is merely a
/// prefix of a longer word pass. The three-character floor stays for the same reason
/// it was introduced: below it even a whole token like `id` or `nl` is coincidence
/// more often than leak.
fn contains_source_value(replacement_key: &str, source_key: &str) -> bool {
    if source_key.is_empty() {
        return false;
    }
    if replacement_key == source_key {
        return true;
    }
    source_key.chars().count() >= 3 && contains_at_token_boundary(replacement_key, source_key)
}

/// Whether `haystack` contains `needle` with both ends aligned to a token boundary.
///
/// A boundary is the start or end of the string, or a neighbouring character that is
/// not alphanumeric — which makes `-`, `'`, `@`, `.` and whitespace all separators, so
/// hyphenated and apostrophed names and the parts of an address or an id are matched
/// as whole tokens. An edge of `needle` that is itself non-alphanumeric needs no
/// boundary on that side: `@corp.com` is already delimited by its own `@`.
fn contains_at_token_boundary(haystack: &str, needle: &str) -> bool {
    let needle_starts_open = needle.chars().next().is_some_and(char::is_alphanumeric);
    let needle_ends_open = needle
        .chars()
        .next_back()
        .is_some_and(char::is_alphanumeric);

    haystack.match_indices(needle).any(|(start, matched)| {
        let before_is_boundary = !needle_starts_open
            || haystack[..start]
                .chars()
                .next_back()
                .is_none_or(|character| !character.is_alphanumeric());
        let after_is_boundary = !needle_ends_open
            || haystack[start + matched.len()..]
                .chars()
                .next()
                .is_none_or(|character| !character.is_alphanumeric());
        before_is_boundary && after_is_boundary
    })
}

/// The identity keys of `values`, the form the cross-value leak check compares against.
fn source_keys(values: &[String]) -> BTreeSet<String> {
    values
        .iter()
        .map(|value| value_identity_key(value))
        .collect()
}

fn selected_smart_columns(columns: &[ColumnMetadata]) -> impl Iterator<Item = &ColumnMetadata> {
    columns.iter().filter(|column| {
        column.is_selected && column.strategy == crate::types::AnonymizationStrategy::LocalAi
    })
}

fn find_column_by_index(index: usize, columns: &[ColumnMetadata]) -> Option<&ColumnMetadata> {
    columns.iter().find(|column| column.index == index)
}

fn insert_unique_smart_value(values: &mut BTreeSet<String>, value: &str) {
    let trimmed = value.trim();
    if values.len() < SMART_REPLACEMENT_VALUE_CAP_PER_COLUMN || values.contains(trimmed) {
        values.insert(trimmed.to_string());
    }
}

/// The key a source value is remembered under for the duration of a run.
///
/// This is what makes `Ada Lovelace`, `  Ada Lovelace  ` and `ada lovelace` one
/// value rather than three: a run gives them one replacement, so the output stays
/// internally consistent for a person the input spelled inconsistently.
///
/// Every per-run replacement map keys on this — the Local AI map in this module and
/// the pseudonym maps in `strategies::state`. They hold separate key spaces and
/// never query each other, so a divergence would not cause a cross-map miss; each
/// map would simply stop recognizing its own values as repeats. That is the quieter
/// failure, which is why the rule lives in one place rather than being restated per
/// map: a map that stopped trimming still returns a perfectly plausible replacement,
/// just a second one for a value it has already seen. For Local AI it does not even
/// look like a miss, only an unexplained bump in `smart_replacement_fallbacks`.
///
/// The folding must stay Unicode-aware, and it collapses internal whitespace runs.
/// ASCII-only case folding would leave `MÜLLER` and `Müller` as two different values,
/// which is a leak and not merely an inconsistency: the smart-replacement checks
/// compare on this key, so a source value echoed back with different non-ASCII casing
/// would match neither the equality nor the containment arm and be published. Two
/// genuinely different source values cannot be merged by either rule — they can only differ by case or by the
/// width of a whitespace run, and a person written `Jan  de Vries` in one row and
/// `Jan de Vries` in the next is one person, which is the same judgement trimming
/// already makes at the ends of the value.
pub(crate) fn value_identity_key(value: &str) -> String {
    let mut key = String::with_capacity(value.len());
    let mut pending_separator = false;
    for character in value.trim().chars() {
        if character.is_whitespace() {
            pending_separator = true;
            continue;
        }
        if pending_separator {
            key.push(' ');
            pending_separator = false;
        }
        key.extend(character.to_lowercase());
    }
    key
}
