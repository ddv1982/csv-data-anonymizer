use serde::{Deserialize, Serialize};
/// The largest detection basis any entry point accepts, for any input kind.
///
/// One limit rather than one per workflow, because the figure comes from one
/// setting. "Sample rows" is not per-workflow: a user who raises it to work on a
/// large CSV file has raised it for the paste workflow too, so a value that reaches
/// the setting has to be a value every entry point will honour.
///
/// Enforced twice, and both sites read this: `settings::sanitize_settings` clamps
/// what can be stored, and the paste entry points reject an oversized request
/// outright, since they are reachable by callers that never went through settings.
pub const MAX_SAMPLE_ROW_COUNT: usize = 10_000;

/// The largest display window any entry point accepts, for any input kind. One
/// limit for the same reason as [`MAX_SAMPLE_ROW_COUNT`] — it comes from the
/// "Preview rows" setting, which is likewise not per-workflow.
pub const MAX_PREVIEW_SAMPLE_COUNT: usize = 100;

/// The smallest detection basis any entry point classifies on, for any input kind.
///
/// A floor rather than a default: `service::detection_sample_rows` and
/// `direct_input::shared::paste_detection_sample_rows` both raise a caller's "Sample
/// rows" to at least this, so the setting can only ask for more evidence than the
/// default, never less.
///
/// One constant rather than one per workflow, because the file and paste workflows
/// promise the user that they classify on the same basis. Two literals held that
/// promise by coincidence: changing either alone left a pasted CSV and the same file
/// on disk detecting different types, and so being offered different strategies, with
/// nothing failing to say so.
pub(crate) const DETECTION_SAMPLE_ROW_FLOOR: usize = 100;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedSample {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// Data rows read while building the sample. This is the input's full
    /// data-row count only when `scanned_entire_input` is true; it is always
    /// >= `rows.len()`, because a spread sample thins what it keeps.
    pub data_rows_scanned: usize,
    /// Whether every data row was read. False only for a head window that hit
    /// its cap; detection samples always scan the whole input.
    pub scanned_entire_input: bool,
}

/// What the two figures in a detection-coverage disclosure count.
///
/// The unit is carried rather than assumed because "rows" is only true for the two
/// tabular entry points. A field-based paste — JSON, YAML, XML, or free text scanned
/// for privacy spans — has no rows to sample: `direct_input::shared::detection_coverage`
/// counts the values of the busiest field, and `PasteAnalyzeData::row_count` is derived
/// separately and can disagree outright. A pasted `{"users": [500 objects]}` shows one
/// row in the UI, and free text shows a match count that is neither its line count nor
/// its row count, so a disclosure hard-coded to "rows" states a figure the user cannot
/// find anywhere on screen and cannot check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DetectionCoverageUnit {
    Rows,
    Values,
}

impl DetectionCoverageUnit {
    /// The noun a disclosure sentence uses for these figures.
    pub(crate) fn plural_noun(self) -> &'static str {
        match self {
            Self::Rows => "rows",
            Self::Values => "values",
        }
    }
}

/// How much of an input detection actually classified.
///
/// Detection votes on a bounded sample, so a value occurring in few rows can be
/// missed — and a column whose sensitive values were all missed is never
/// auto-selected, so it is written unchanged. That is a deliberate trade for
/// bounded memory on inputs of any size, but it is only an honest one if the user
/// is told the verdict rests on a sample. This carries the figures needed to say so
/// and the unit they are counted in.
///
/// Not a DTO. It is `pub(crate)` on purpose: [`Self::new`] is the only place the
/// `examined <= total` invariant is established, and a `Deserialize` impl would let
/// a wire value walk straight past it and report a sample larger than the input it
/// was drawn from. What crosses the IPC boundary is [`DetectionCoverageSummary`],
/// a flat snapshot taken after clamping, plus the report notes and preflight review
/// item built from this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DetectionCoverage {
    /// Rows or values kept for classification.
    examined: usize,
    /// Rows or values the input holds.
    total: usize,
    unit: DetectionCoverageUnit,
}

impl DetectionCoverage {
    /// Coverage of the sample a detection pass actually kept.
    ///
    /// Read off the sample rather than recomputed from the requested row count,
    /// because the request is a ceiling and the sample is what happened: a file with
    /// fewer rows than the request is fully covered, and asking the request would
    /// claim otherwise.
    pub(crate) fn from_detection_sample(sample: &ParsedSample) -> Self {
        Self::rows(sample.rows.len(), sample.data_rows_scanned)
    }

    /// Coverage for an input that has nothing to sample.
    ///
    /// Both figures are zero, which is not a missing figure but the only truthful
    /// pair here: quick-generate has no source input, so no row went unexamined and
    /// none was examined either. Only [`Self::is_partial`] is consulted for such an
    /// input, and it answers false — the disclosure stays silent, which is right,
    /// because there is no sampling to disclose. Nothing may print
    /// [`Self::examined`] and [`Self::total`] without gating on
    /// [`Self::is_partial`] first, or this constructor renders as "0 of 0".
    pub(crate) fn complete() -> Self {
        Self::rows(0, 0)
    }

    /// Coverage counted in data rows: the two tabular entry points, CSV file and
    /// pasted CSV text.
    pub(crate) fn rows(examined: usize, total: usize) -> Self {
        Self::new(examined, total, DetectionCoverageUnit::Rows)
    }

    /// Coverage counted in field values: JSON, YAML, XML and free-text pastes, which
    /// have fields rather than rows. See [`DetectionCoverageUnit`].
    pub(crate) fn values(examined: usize, total: usize) -> Self {
        Self::new(examined, total, DetectionCoverageUnit::Values)
    }

    fn new(examined: usize, total: usize, unit: DetectionCoverageUnit) -> Self {
        Self {
            examined: examined.min(total),
            total,
            unit,
        }
    }

    /// Whether some of the input went unclassified.
    pub(crate) fn is_partial(self) -> bool {
        self.examined < self.total
    }

    pub(crate) fn examined(self) -> usize {
        self.examined
    }

    pub(crate) fn total(self) -> usize {
        self.total
    }

    pub(crate) fn unit(self) -> DetectionCoverageUnit {
        self.unit
    }

    /// The IPC-facing snapshot of this coverage.
    pub(crate) fn summary(self) -> DetectionCoverageSummary {
        DetectionCoverageSummary {
            examined: self.examined,
            total: self.total,
            unit: self.unit,
            is_partial: self.is_partial(),
        }
    }
}

/// What detection classified, as the paste analyze result reports it.
///
/// This exists so a paste user learns that detection sampled before choosing
/// columns, not after the output already exists. The file workflow gets the same
/// disclosure from preflight; the paste workflow has no preflight, so until this
/// crossed the boundary the only place it appeared was the post-transform privacy
/// report — advice ("raise Sample rows") that by then costs a whole second run.
///
/// Counts only. No value, column name or excerpt goes on the wire here, so the
/// disclosure cannot itself leak what detection missed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectionCoverageSummary {
    pub examined: usize,
    pub total: usize,
    pub unit: DetectionCoverageUnit,
    /// `examined < total`, decided here rather than in the client.
    ///
    /// The comparison is trivial, which is exactly why it would drift: a client that
    /// re-derives it is free to write `<=` or to compare against a row count from a
    /// different field, and either mistake silences the warning rather than
    /// producing a visible error.
    pub is_partial: bool,
}
