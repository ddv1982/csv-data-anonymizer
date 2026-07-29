use super::*;
use crate::service::cardinality_warning_for_column;
use crate::types::ColumnValueDistribution;

/// A value with no meaning of its own: every assertion here turns on the column's
/// strategy and type, never on what the cell happens to contain.
const VALUE: &str = "Alice Smith";

/// The column's real size, which is what the ratio term in the cardinality test
/// divides by. Kept equal to the distribution's own total so the fixture below
/// describes a fully measured column rather than a sample.
const POPULATION_VALUES: usize = 60;

/// Five values repeated across sixty rows, with nothing seen once.
///
/// Engineered to trip [`ColumnValueDistribution::frequency_inversion_risk_in`] on
/// its absolute term alone — sixty clears the floor and five distinct values is
/// under the invertibility limit — so a `None` from the warning means the strategy
/// was ruled out, not that the numbers happened to be harmless. Testing agreement
/// against a benign distribution would pass no matter which way either side
/// answered.
fn invertible_distribution() -> ColumnValueDistribution {
    ColumnValueDistribution {
        column_index: 0,
        distinct_values: 5,
        total_values: POPULATION_VALUES,
        singleton_values: 0,
        doubleton_values: 0,
        max_value_occurrences: 12,
    }
}

/// A replacement for [`VALUE`], seeded for every strategy rather than only for
/// Local AI.
///
/// Local AI's recording path is behind `state.smart_replacement(..)`, so without a
/// seeded map the branch under test is unreachable offline and the variant would be
/// measured on its fallback path instead — which records for a different reason and
/// would hide a disagreement about the hit path. Every other strategy ignores the
/// map, so seeding it unconditionally keeps one state builder for the whole walk.
fn seeded_state() -> TransformState {
    let mut replacements = SmartReplacementMap::default();
    replacements.insert(0, VALUE, "Maya Carter");
    TransformState::with_smart_replacements(replacements)
}

/// The transform ledger and the cardinality warning have to name the same set of
/// strategies.
///
/// Two exhaustive matches decide it independently: `transform_value_with_state`
/// records a value into the ledger on the paths that hand out a consistent
/// pseudonym, and `cardinality_warning_for_column` decides from its own
/// `keeps_consistent_mapping` predicate whether such a column can be relabelled by
/// frequency. Neither has a wildcard arm, so a new strategy cannot be added without
/// visiting both — but nothing makes the two answers match, and a strategy the
/// author routes through the ledger while leaving out of `keeps_consistent_mapping`
/// preserves linkage and draws no warning at all.
///
/// That is the direction worth a test, because it fails open: the user keeps a
/// column whose mapping can be inverted from how often each value occurs, and the
/// preview that exists to tell them before the run stays silent. The reverse
/// mistake produces a warning about a risk the column lacks, which is noise a user
/// can see and dismiss.
///
/// `keeps_consistent_mapping` is now shared with the preflight memory projection, so
/// a strategy left out of it also goes uncounted towards the mapping ceiling. That
/// widens what this test protects rather than changing it.
///
/// Both types are walked because the two matches also share a *second* decision:
/// under Auto or Pseudonymize a pass-through type is returned unchanged and exposes
/// nothing, and each side reaches that conclusion through its own call to
/// `uses_default_pass_through`. A strategy that consults the predicate on one side
/// only would agree on ordinary types and drift on exactly the types where the
/// column is left alone.
#[test]
fn the_ledger_and_the_cardinality_warning_agree_on_every_strategy() {
    let mut disagreements = Vec::new();

    for strategy in all_strategies() {
        for detected_type in [DataType::FullName, DataType::CountryCode] {
            let mut subject = column(detected_type);
            subject.strategy = strategy;
            subject.sample_value_distribution = invertible_distribution();

            let mut state = seeded_state();
            transform_value_with_state(VALUE, &subject, &context(), &mut state);
            let recorded = !state.report().column_value_distributions.is_empty();

            let warned = cardinality_warning_for_column(&subject, POPULATION_VALUES).is_some();

            if recorded != warned {
                disagreements.push(format!(
                    "{strategy:?} on {detected_type:?}: the ledger {} but the warning {}",
                    if recorded {
                        "recorded a distribution"
                    } else {
                        "recorded nothing"
                    },
                    if warned { "fired" } else { "stayed silent" },
                ));
            }
        }
    }

    assert!(
        disagreements.is_empty(),
        "a strategy whose ledger and warning disagree either leaks a frequency-invertible \
         mapping with no warning, or warns about a column it did not pseudonymize:\n  {}",
        disagreements.join("\n  ")
    );
}
