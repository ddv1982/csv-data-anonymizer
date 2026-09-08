use super::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProcessOptions<'a> {
    pub smart_replacements: Option<&'a crate::smart::SmartReplacementMap>,
    /// Validated run-only secret. The key type cannot be serialized.
    pub tokenization_key: Option<&'a crate::strategies::TokenizationKey>,
    /// Mapping entries this run may hold before it refuses to continue, or `None` for
    /// `TransformState::MAPPING_ENTRY_CEILING`.
    ///
    /// A run option rather than a constant read at the point of use because the
    /// ceiling is a resource limit, and a resource limit that cannot be set to a
    /// reachable value cannot be tested: the real one stands for about 5 GB of
    /// mapping, so the only way to show that the run loop *consults* it — and that
    /// refusing leaves no partial output — is to hand the loop a smaller one.
    pub mapping_entry_ceiling: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessProgress {
    pub rows_processed: usize,
}

pub struct ProcessControl<'a> {
    pub on_progress: Option<&'a mut dyn FnMut(ProcessProgress)>,
    pub should_cancel: Option<&'a dyn Fn() -> bool>,
}

impl ProcessControl<'_> {
    pub fn none() -> Self {
        Self {
            on_progress: None,
            should_cancel: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessResult {
    pub row_count: usize,
    pub output_path: PathBuf,
    pub duration_ms: u128,
    pub transform_report: TransformReport,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransformReport {
    pub unique_pseudonym_values: usize,
    pub reused_pseudonym_values: usize,
    pub collisions_avoided: usize,
    pub exhausted_pseudonym_pools: usize,
    pub opaque_token_values: usize,
    pub keyed_token_values: usize,
    pub keyed_token_columns: Vec<usize>,
    pub smart_replacement_requests: usize,
    pub smart_replacement_values: usize,
    pub smart_replacement_rejections: usize,
    pub smart_replacement_rejection_reasons: Vec<SmartReplacementRejectionCount>,
    pub smart_replacement_fallbacks: usize,
    pub shape_fallback_values: usize,
    /// Selected values that a strategy claiming to transform returned byte-for-byte
    /// unchanged after trimming. This is the exact residual-leak guard; it is not a
    /// heuristic scan for merely realistic-looking replacement data.
    pub unchanged_sensitive_values: usize,
    pub unchanged_sensitive_columns: Vec<usize>,
    /// Unique selected high/medium-risk source values fingerprinted for the broad
    /// residual audit, and released cell fingerprints compared against them.
    pub residual_audit_source_values: usize,
    pub residual_audit_output_values: usize,
    pub residual_audit_matches: usize,
    /// True when either fingerprint set reached its independent memory ceiling.
    pub residual_audit_incomplete: bool,
    pub column_value_distributions: Vec<ColumnValueDistribution>,
    pub row_uniqueness: Option<RowUniquenessSummary>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransformContext<'a> {
    pub column_name: &'a str,
    pub column_index: usize,
    pub row_index: usize,
    pub empty_format: EmptyFormat,
}

impl<'a> TransformContext<'a> {
    /// The context for one cell of `column`.
    ///
    /// Every caller wants exactly this: the column's own name, index and empty
    /// format, plus where the value sits. Assembled by hand at each site, the fields
    /// are three same-shaped values a mistake can silently swap — a context built
    /// with another column's index makes the transform key its mapping under the
    /// wrong column, so two columns share replacements and a value redacted in one
    /// reappears in the other.
    pub fn for_column(column: &'a ColumnMetadata, row_index: usize) -> Self {
        Self {
            column_name: &column.name,
            column_index: column.index,
            row_index,
            empty_format: column.empty_format,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartReplacementEntry {
    pub column_index: usize,
    pub original: String,
    pub replacement: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SmartReplacementRejectionReason {
    UnexpectedOriginal,
    MissingOutput,
    EmptyOutput,
    SameAsOriginal,
    ContainsOriginal,
    /// The replacement was, or contained, a source value belonging to a *different*
    /// row of the same column.
    ///
    /// Kept apart from [`Self::ContainsOriginal`] because the two describe different
    /// events and only one of them moves a person's data between rows.
    /// `ContainsOriginal` means the model echoed back the value it was asked to
    /// replace, which wastes the request; this means it emitted somebody else's real
    /// value, which would have published that value against the wrong record. A
    /// report that merged them could not say which had happened.
    MatchesOtherOriginal,
    ControlCharacter,
    DuplicateOriginal,
    DuplicateOutput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartReplacementRejectionCount {
    pub reason: SmartReplacementRejectionReason,
    pub count: usize,
}
