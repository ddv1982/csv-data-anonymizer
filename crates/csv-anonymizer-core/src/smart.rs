use crate::csv_io::validate_file;
use crate::detection::is_empty_value;
use crate::error::{AnonymizerError, Result, csv_error};
use crate::types::{ColumnMetadata, ProcessControl, SmartReplacementEntry};
use csv::{ReaderBuilder, Trim};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

const SMART_REPLACEMENT_BATCH_SIZE: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartReplacement {
    pub original: String,
    pub replacement: String,
}

#[derive(Debug, Clone, Copy)]
pub struct SmartReplacementRequest<'a> {
    pub column: &'a ColumnMetadata,
    pub values: &'a [String],
    pub deterministic: bool,
    pub seed: &'a str,
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
}

impl SmartReplacementMap {
    pub fn len(&self) -> usize {
        self.replacements.len()
    }

    pub fn is_empty(&self) -> bool {
        self.replacements.is_empty()
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
        let mut map = Self::default();
        for entry in entries {
            map.insert(
                entry.column_index,
                &entry.original,
                entry.replacement.clone(),
            );
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
    deterministic: bool,
    seed: &str,
    provider: Option<&mut dyn SmartReplacementProvider>,
) -> Result<SmartReplacementMap> {
    let batches = collect_unique_values_from_rows(rows, columns);
    build_replacement_map(columns, batches, deterministic, seed, None, provider)
}

pub fn prepare_smart_replacements_from_csv(
    file_path: &Path,
    columns: &[ColumnMetadata],
    deterministic: bool,
    seed: &str,
    control: Option<&mut ProcessControl<'_>>,
    existing: Option<&SmartReplacementMap>,
    provider: Option<&mut dyn SmartReplacementProvider>,
) -> Result<SmartReplacementMap> {
    validate_file(file_path)?;
    let batches = collect_unique_values_from_csv(file_path, columns, control)?;
    build_replacement_map(columns, batches, deterministic, seed, existing, provider)
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
                values.insert(value.trim().to_string());
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
                values.insert(value.trim().to_string());
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
    deterministic: bool,
    seed: &str,
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

        for chunk in missing_values.chunks(SMART_REPLACEMENT_BATCH_SIZE) {
            let replacements = provider.generate_replacements(SmartReplacementRequest {
                column,
                values: chunk,
                deterministic,
                seed,
            })?;
            for (original, replacement) in validated_replacements(chunk, replacements) {
                map.insert(column_index, &original, replacement);
            }
        }
    }

    Ok(map)
}

fn validated_replacements(
    expected_values: &[String],
    replacements: Vec<SmartReplacement>,
) -> Vec<(String, String)> {
    let expected_by_key = expected_values
        .iter()
        .map(|value| (normalized_value_key(value), value.clone()))
        .collect::<HashMap<_, _>>();
    let mut used_outputs = BTreeSet::new();
    let mut valid = Vec::new();

    for replacement in replacements {
        let original_key = normalized_value_key(&replacement.original);
        let Some(original) = expected_by_key.get(&original_key) else {
            continue;
        };
        let cleaned = replacement.replacement.trim();
        if !is_valid_replacement(original, cleaned) {
            continue;
        }
        let output_key = normalized_value_key(cleaned);
        if !used_outputs.insert(output_key) {
            continue;
        }
        valid.push((original.clone(), cleaned.to_string()));
    }

    valid
}

fn is_valid_replacement(original: &str, replacement: &str) -> bool {
    if replacement.is_empty() || replacement.eq_ignore_ascii_case(original) {
        return false;
    }
    if replacement
        .chars()
        .any(|character| character.is_control() && character != '\t')
    {
        return false;
    }

    let original_key = normalized_value_key(original);
    if original_key.len() >= 3 && normalized_value_key(replacement).contains(&original_key) {
        return false;
    }

    true
}

fn selected_smart_columns(columns: &[ColumnMetadata]) -> impl Iterator<Item = &ColumnMetadata> {
    columns.iter().filter(|column| {
        column.is_selected && column.strategy == crate::types::AnonymizationStrategy::LocalAi
    })
}

fn find_column_by_index(index: usize, columns: &[ColumnMetadata]) -> Option<&ColumnMetadata> {
    columns.iter().find(|column| column.index == index)
}

fn normalized_value_key(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn check_canceled(control: &mut Option<&mut ProcessControl<'_>>) -> Result<()> {
    let Some(control) = control.as_deref_mut() else {
        return Ok(());
    };
    let Some(should_cancel) = control.should_cancel else {
        return Ok(());
    };
    if should_cancel() {
        Err(AnonymizerError::Canceled)
    } else {
        Ok(())
    }
}
