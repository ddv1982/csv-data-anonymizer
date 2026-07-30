use super::*;
use crate::smart::{SmartReplacement, SmartReplacementProvider, SmartReplacementRequest};
use crate::types::{
    AnonymizationStrategy, ColumnControl, DataType, PiiRisk, PreflightMode, PreflightParams,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

/// The arrange half of a service test: a service and a scratch directory to write into.
///
/// Every test on this path needs both, and they are not independent — the directory has to
/// outlive every path handed out of it, which is why it is a field here rather than a
/// `tempfile::tempdir()` binding a caller has to remember to keep alive. A `let _ =` on that
/// binding deletes the directory immediately and the failure surfaces as a missing input
/// file three calls later.
///
/// Generalised from the local versions in [`label_output`] and [`cardinality`], which
/// solved the same preamble one file at a time.
struct Workspace {
    service: AnonymizerService,
    directory: tempfile::TempDir,
}

impl Workspace {
    fn new() -> Self {
        Self {
            service: AnonymizerService::new("test-version"),
            directory: tempfile::tempdir().unwrap(),
        }
    }

    /// A path inside the scratch directory. Nothing is created, so this is how an output
    /// path — or an input a test means to be absent — is named.
    fn path(&self, name: &str) -> PathBuf {
        self.directory.path().join(name)
    }

    /// Writes `text` to `name` in the scratch directory and returns its path.
    fn write_input(&self, name: &str, text: &str) -> PathBuf {
        let path = self.path(name);
        fs::write(&path, text).unwrap();
        path
    }
}

/// A control that leaves the detected type alone and only sets the strategy.
///
/// The type override stays `None` because a control that pinned it would decide the
/// transform the test is asking detection to choose — see [`typed_control`] for the tests
/// that mean to override it.
fn control(column_index: usize, strategy: AnonymizationStrategy) -> ColumnControl {
    ColumnControl {
        column_index,
        type_override: None,
        strategy,
    }
}

/// A control that overrides the detected type as well.
fn typed_control(
    column_index: usize,
    type_override: DataType,
    strategy: AnonymizationStrategy,
) -> ColumnControl {
    ColumnControl {
        column_index,
        type_override: Some(type_override),
        strategy,
    }
}

/// A run of `columns` over `file_path`, with detection left to pick every strategy.
///
/// A builder rather than a `Default` impl, and deliberately so: [`AnonymizeParams`] is
/// deserialized from the frontend, and a `Default` on it would turn a field the frontend
/// stopped sending into a silent `false` — `force`, say — instead of into the
/// deserialization error that would have caught it. This function is `#[cfg(test)]` and
/// cannot be reached from the IPC path.
///
/// `force: false` and an empty `controls` are the values a test that does not mention them
/// means. A test that varies either states it, which is what `..anonymize_params(..)` at
/// the call sites is for.
fn anonymize_params(
    file_path: PathBuf,
    output_path: PathBuf,
    columns: Vec<usize>,
) -> AnonymizeParams {
    AnonymizeParams {
        file_path,
        output_path,
        columns,
        controls: vec![],
        force: false,
        preview_smart_replacements: vec![],
    }
}

/// A preview of `columns` over `file_path`, showing five rows and classifying on a hundred.
///
/// Those two figures are the app's own preview sizes, and they are the ones the assertions
/// in this directory are calibrated against — a test that turns on a different sample size
/// says so with `..preview_params(..)`, because the size is then part of what it is
/// testing. Not a `Default` impl, for the reason given on [`anonymize_params`].
fn preview_params(file_path: PathBuf, columns: Vec<usize>) -> PreviewParams {
    PreviewParams {
        file_path,
        columns,
        controls: vec![],
        sample_count: 5,
        sample_row_count: 100,
    }
}

/// A preflight of `columns` over `file_path` in `mode`, with nothing else asked for.
///
/// The three fields left at their most restrictive: no output path (which is what
/// `PreflightMode::Anonymize` blocks on, and what `Preview` is allowed to omit), `force`
/// off, and Local AI not ready. Each is a gate this path can refuse on, so a test that
/// wants one open says so rather than inheriting it. Not a `Default` impl, for the reason
/// given on [`anonymize_params`].
fn preflight_params(
    file_path: PathBuf,
    mode: PreflightMode,
    columns: Vec<usize>,
) -> PreflightParams {
    PreflightParams {
        mode,
        file_path,
        output_path: None,
        columns,
        controls: vec![],
        force: false,
        sample_row_count: 10,
        preview_smart_replacements: vec![],
        local_ai_ready: false,
        local_ai_message: None,
    }
}

/// Reads a written output file back as fields, header row first.
///
/// Parsed as CSV rather than split on commas: these tests assert on exact cell
/// contents, and a quoting change would otherwise show up as a mangled expectation
/// rather than as the quoting change it is.
fn written_rows(path: &Path) -> Vec<Vec<String>> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_path(path)
        .unwrap();
    reader
        .records()
        .map(|record| record.unwrap().iter().map(ToString::to_string).collect())
        .collect()
}

mod analysis_preview;
mod anonymize;
mod cardinality;
mod label_output;
mod possible_names;
mod preflight;
mod smart_replacement;
