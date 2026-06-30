use super::*;
use crate::smart::{SmartReplacement, SmartReplacementProvider, SmartReplacementRequest};
use crate::types::{AnonymizationStrategy, ColumnControl, DataType};
use std::{fs, path::PathBuf};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

mod analysis_preview;
mod anonymize;
mod preflight;
mod smart_replacement;
