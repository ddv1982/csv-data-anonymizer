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
            let mut used_outputs = BTreeSet::new();
            let validation =
                validated_replacements(&expected_values, replacements, &mut used_outputs);
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
            .map(|replacement| normalized_value_key(&replacement.replacement))
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
            normalized_value: normalized_value_key(value),
        }
    }
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

fn collect_unique_values_from_rows(
    rows: &[Vec<String>],
    columns: &[ColumnMetadata],
) -> BTreeMap<usize, Vec<String>> {
    let mut values_by_column = selected_smart_columns(columns)
        .map(|column| (column.index, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();

    if values_by_column.is_empty() {
        return BTreeMap::new();
    }

    for row in rows {
        for (column_index, values) in &mut values_by_column {
            let Some(value) = row.get(*column_index) else {
                continue;
            };
            if !is_empty_value(value) {
                insert_unique_smart_value(values, value);
            }
        }
    }

    values_by_column
        .into_iter()
        .map(|(index, values)| (index, values.into_iter().collect()))
        .collect()
}

fn collect_unique_values_from_csv(
    file_path: &Path,
    columns: &[ColumnMetadata],
    mut control: Option<&mut ProcessControl<'_>>,
) -> Result<BTreeMap<usize, Vec<String>>> {
    let mut values_by_column = selected_smart_columns(columns)
        .map(|column| (column.index, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();

    if values_by_column.is_empty() {
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

        for (column_index, values) in &mut values_by_column {
            let Some(value) = record.get(*column_index) else {
                continue;
            };
            if !is_empty_value(value) {
                insert_unique_smart_value(values, value);
            }
        }
    }

    Ok(values_by_column
        .into_iter()
        .map(|(index, values)| (index, values.into_iter().collect()))
        .collect())
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

        for chunk in missing_values.chunks(SMART_REPLACEMENT_BATCH_SIZE) {
            let requested = chunk.len();
            let replacements = provider.generate_replacements(SmartReplacementRequest {
                column,
                values: chunk,
            })?;
            let validation = validated_replacements(chunk, replacements, &mut used_outputs);
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

fn validated_replacements(
    expected_values: &[String],
    replacements: Vec<SmartReplacement>,
    used_outputs: &mut BTreeSet<String>,
) -> ValidatedSmartReplacements {
    let expected_by_key = expected_values
        .iter()
        .map(|value| (normalized_value_key(value), value.clone()))
        .collect::<HashMap<_, _>>();
    let mut seen_expected_originals = BTreeSet::new();
    let mut accepted_originals = BTreeSet::new();
    let mut accepted = Vec::new();
    let mut rejection_reasons = Vec::new();

    for replacement in replacements {
        let original_key = normalized_value_key(&replacement.original);
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
        if let Some(reason) = invalid_replacement_reason(original, cleaned) {
            rejection_reasons.push(reason);
            continue;
        }
        let output_key = normalized_value_key(cleaned);
        if !used_outputs.insert(output_key) {
            rejection_reasons.push(SmartReplacementRejectionReason::DuplicateOutput);
            continue;
        }
        accepted_originals.insert(original_key);
        accepted.push((original.clone(), cleaned.to_string()));
    }

    for value in expected_values {
        let key = normalized_value_key(value);
        if !accepted_originals.contains(&key) && !seen_expected_originals.contains(&key) {
            rejection_reasons.push(SmartReplacementRejectionReason::MissingOutput);
        }
    }

    ValidatedSmartReplacements {
        accepted,
        rejection_reasons,
    }
}

fn invalid_replacement_reason(
    original: &str,
    replacement: &str,
) -> Option<SmartReplacementRejectionReason> {
    if replacement.is_empty() {
        return Some(SmartReplacementRejectionReason::EmptyOutput);
    }
    if replacement.eq_ignore_ascii_case(original) {
        return Some(SmartReplacementRejectionReason::SameAsOriginal);
    }
    if replacement
        .chars()
        .any(|character| character.is_control() && character != '\t')
    {
        return Some(SmartReplacementRejectionReason::ControlCharacter);
    }

    let original_key = normalized_value_key(original);
    if original_key.len() >= 3 && normalized_value_key(replacement).contains(&original_key) {
        return Some(SmartReplacementRejectionReason::ContainsOriginal);
    }

    None
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

fn normalized_value_key(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}
