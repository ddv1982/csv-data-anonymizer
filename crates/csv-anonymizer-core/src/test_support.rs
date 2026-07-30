//! Fixtures shared by the crate's unit tests, and nothing else.
//!
//! `#[cfg(test)]` at the module declaration in `lib.rs`, so none of this is compiled into
//! a release build. It exists because [`crate::types::ColumnMetadata`] has fifteen fields
//! of which any one test varies two or three, and a hand-written literal per module means
//! a new field is a hand-edit per module — which is how a field ends up set to whatever
//! made the compiler stop complaining rather than to what the test meant.
//!
//! Only [`crate::types::ColumnMetadata`] is built here. The IPC parameter structs
//! (`AnonymizeParams`, `PreviewParams`, `PreflightParams` and their siblings) deliberately
//! have no `Default`: they are deserialized from the frontend, and a `Default` on a
//! privacy parameter would let a field the frontend forgot to send arrive as a silent
//! zero rather than as a deserialization error. Their fixtures are `#[cfg(test)]` builders
//! in `service::tests`, which cannot reach the wire.

use crate::types::{AnonymizationStrategy, ColumnMetadata, Confidence, DataType, PiiRisk};

/// A column as a unit test needs one: identified, typed, and given a strategy.
///
/// Those four are the parameters because they are what the code under test reads. The
/// rest come from [`ColumnMetadata`]'s `Default`, whose every field is the
/// least-privileged reading of "nobody decided this" — see the derive's doc comment.
///
/// Two of the defaults are overridden here, both towards *less* evidence rather than
/// more. `Confidence::High` says the type given is the type meant, so a test that names
/// `DataType::Email` is not silently answering `is_actionable` with a no. `PiiRisk::Medium`
/// is the milder of the two elevated levels: a fixture at `High` would carry the strongest
/// finding the app makes without any test having asked for it, and a caller that wants
/// `High` says so.
///
/// `is_selected` stays at the `Default` of false. Selection is read by
/// `release_report::build_readiness`, by `uniqueness::LinkableProjection::for_column` and
/// by the report notes, so it is never a field a fixture should decide silently — a caller
/// that releases the column sets it.
pub(crate) fn column(
    index: usize,
    name: &str,
    detected_type: DataType,
    strategy: AnonymizationStrategy,
) -> ColumnMetadata {
    ColumnMetadata {
        name: name.to_string(),
        index,
        detected_type,
        confidence: Confidence::High,
        pii_risk: PiiRisk::Medium,
        strategy,
        ..Default::default()
    }
}

/// The same column, released rather than held back.
///
/// Split out rather than made a fifth parameter because `is_selected` is a different kind
/// of fact from the other four: it says whether the column is in the file at all, and a
/// bare `true` at a call site says that far less clearly than the name does.
pub(crate) fn selected_column(
    index: usize,
    name: &str,
    detected_type: DataType,
    strategy: AnonymizationStrategy,
) -> ColumnMetadata {
    ColumnMetadata {
        is_selected: true,
        ..column(index, name, detected_type, strategy)
    }
}
