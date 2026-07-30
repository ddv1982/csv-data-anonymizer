use super::*;
use crate::test_support::column;
use crate::types::{DropColumnEffect, MatchedColumn};

/// A column matched on `matched_on` in every measured row.
///
/// The partial case is spelled out inline by the one test that turns on it, so that the
/// qualification is visible where it matters rather than hidden behind a flag.
fn matched(column_index: usize, matched_on: MatchedPart) -> MatchedColumn {
    MatchedColumn {
        column_index,
        matched_on,
        matched_every_row: true,
    }
}

/// The unmeasured arm's wording, which no report-layer test touched.
///
/// It is the arm that must never read as a pass, so it is the arm most worth pinning: a
/// future editor softening "is not the same as measured clean" has nothing standing in the
/// way.
#[test]
fn the_unmeasured_arm_says_absent_and_not_clean() {
    let summary = RowUniquenessSummary {
        rows_measured: 2_000_001,
        matched_columns: vec![matched(0, MatchedPart::WholeValue)],
        measurement_incomplete: true,
        ..Default::default()
    };

    let item = row_uniqueness_evidence(&[], &summary);

    assert_eq!(item.status, ReleaseEvidenceStatus::Review);
    assert_eq!(
        item.detail,
        "Not measured: this file holds more distinct combinations than the check keeps. 2000001 \
         row(s) were read before it stopped."
    );
}

/// A column name containing a comma must not read as two columns.
///
/// The names are joined with commas, so `city, state` would be counted as two entries by a
/// reader checking the list against the "N columns" label, with nothing to say which was
/// wrong.
#[test]
fn a_column_name_holding_a_comma_is_quoted() {
    let columns = vec![column(
        0,
        "city, state",
        DataType::String,
        AnonymizationStrategy::PassThrough,
    )];
    let summary = RowUniquenessSummary {
        rows_measured: 10,
        matched_columns: vec![matched(0, MatchedPart::WholeValue)],
        unique_rows: 1,
        // The attribution is irrelevant to what this test asserts, and an unrun one is the
        // only state whose `drop_column_effects` may honestly be empty beside a matched column.
        drop_column_effects: vec![],
        drop_attribution_incomplete: true,
        ..Default::default()
    };

    let item = row_uniqueness_evidence(&columns, &summary);

    assert!(
        item.detail.contains("unique on \"city, state\""),
        "a comma inside a name has to be quoted, got: {}",
        item.detail
    );
}

/// A singleton in a large file must not round away to nothing.
#[test]
fn a_rare_singleton_is_not_reported_as_zero_percent() {
    let summary = RowUniquenessSummary {
        rows_measured: 10_000,
        matched_columns: vec![matched(0, MatchedPart::WholeValue)],
        unique_rows: 1,
        // The attribution is irrelevant to what this test asserts, and an unrun one is the
        // only state whose `drop_column_effects` may honestly be empty beside a matched column.
        drop_column_effects: vec![],
        drop_attribution_incomplete: true,
        ..Default::default()
    };

    let item = row_uniqueness_evidence(&[], &summary);

    // One row in ten thousand is 0.01%, which one decimal place printed as "0.0%" — a
    // headline reading "none" on a file where a person is individually identifiable.
    assert!(
        item.detail.contains("(under 0.1%)"),
        "a real singleton must not read as zero, got: {}",
        item.detail
    );

    // And a ceiling, which the guard did not have: widening its threshold from 0.05 to 50.0
    // left every test green while a 10% exposure rate printed as "under 0.1%".
    let common = RowUniquenessSummary {
        rows_measured: 20,
        unique_rows: 2,
        ..summary
    };
    let item = row_uniqueness_evidence(&[], &common);
    assert!(
        item.detail.contains("(10.0%)"),
        "an ordinary share must print as itself, got: {}",
        item.detail
    );
}

/// A header can be empty or all spaces, and such a column used to print as nothing at all.
#[test]
fn a_blank_header_is_named_by_its_position() {
    let columns = vec![column(
        3,
        "   ",
        DataType::String,
        AnonymizationStrategy::PassThrough,
    )];
    let summary = RowUniquenessSummary {
        rows_measured: 8,
        matched_columns: vec![matched(3, MatchedPart::WholeValue)],
        unique_rows: 8,
        // The attribution is irrelevant to what this test asserts, and an unrun one is the
        // only state whose `drop_column_effects` may honestly be empty beside a matched column.
        drop_column_effects: vec![],
        drop_attribution_incomplete: true,
        ..Default::default()
    };

    let item = row_uniqueness_evidence(&columns, &summary);

    // "8 of 8 released row(s) (100.0%) are unique on , city" told the reader they were
    // unique on a comma.
    assert!(
        item.detail.contains("unique on column 3"),
        "a nameless column still has to be identifiable, got: {}",
        item.detail
    );
}

/// Quoting a comma-bearing name is not enough on its own; the quotes have to be escaped.
#[test]
fn a_column_name_holding_a_quote_is_escaped() {
    let columns = vec![column(
        0,
        "he said \"hi\", ok",
        DataType::String,
        AnonymizationStrategy::PassThrough,
    )];
    let summary = RowUniquenessSummary {
        rows_measured: 8,
        matched_columns: vec![matched(0, MatchedPart::WholeValue)],
        unique_rows: 8,
        // The attribution is irrelevant to what this test asserts, and an unrun one is the
        // only state whose `drop_column_effects` may honestly be empty beside a matched column.
        drop_column_effects: vec![],
        drop_attribution_incomplete: true,
        ..Default::default()
    };

    let item = row_uniqueness_evidence(&columns, &summary);

    // Wrapped bare, this name closed its own quote halfway through and was ambiguous under
    // exactly the reading the quoting was added to prevent.
    assert!(
        item.detail.contains("\"he said \\\"hi\\\", ok\""),
        "an embedded quote must be escaped, got: {}",
        item.detail
    );
}

/// A part only some rows carry is said to be only some rows, rather than claimed of all.
///
/// `matched_on` comes from the column's strategy and detected type and no cell can change it,
/// so a `Timestamp` column where one value in a hundred parses is still `DateDecadeAndTime`
/// and the finding read "share the decade and time of birth_date" — of ninety-nine rows with
/// no decade in them. The caveat also has to say the counts already account for it, or it
/// reads as doubt about a figure that is sound.
#[test]
fn a_part_only_some_rows_carry_is_not_claimed_of_all_of_them() {
    let columns = vec![column(
        2,
        "birth_date",
        DataType::Timestamp,
        AnonymizationStrategy::Auto,
    )];
    let summary = RowUniquenessSummary {
        rows_measured: 100,
        matched_columns: vec![MatchedColumn {
            column_index: 2,
            matched_on: MatchedPart::DateDecadeAndTime,
            matched_every_row: false,
        }],
        unique_rows: 10,
        drop_attribution_incomplete: true,
        ..Default::default()
    };

    let item = row_uniqueness_evidence(&columns, &summary);

    assert!(
        item.detail
            .contains("Only some of the released rows carry what birth_date was matched on"),
        "a part the rows do not all carry has to be named as partial, got: {}",
        item.detail
    );
    // Without this the caveat reads as "and therefore the number above is doubtful", which is
    // the opposite of true: those rows were hashed as sharing nothing on that column, which is
    // what an outsider holding the originals also gets.
    assert!(
        item.detail
            .contains("already count those as sharing nothing there"),
        "the caveat must say the arithmetic accounts for it, got: {}",
        item.detail
    );
}

/// A column every row carries gets no caveat, so the caveat means something when it appears.
#[test]
fn a_part_every_row_carries_is_stated_without_qualification() {
    let columns = vec![column(
        1,
        "city",
        DataType::String,
        AnonymizationStrategy::PassThrough,
    )];
    let summary = RowUniquenessSummary {
        rows_measured: 100,
        matched_columns: vec![matched(1, MatchedPart::WholeValue)],
        unique_rows: 10,
        drop_attribution_incomplete: true,
        ..Default::default()
    };

    let item = row_uniqueness_evidence(&columns, &summary);

    assert!(
        !item.detail.contains("Only some of the released rows"),
        "nothing here is partial, got: {}",
        item.detail
    );
}

/// The advice names the column and both figures, so a reader can act without opening the file.
#[test]
fn the_finding_says_which_column_to_drop() {
    let columns = vec![column(
        2,
        "birth_date",
        DataType::String,
        AnonymizationStrategy::PassThrough,
    )];
    let summary = RowUniquenessSummary {
        rows_measured: 1000,
        matched_columns: vec![matched(2, MatchedPart::WholeValue)],
        unique_rows: 412,
        drop_column_effects: vec![DropColumnEffect {
            column_index: 2,
            unique_rows_without: 3,
        }],
        ..Default::default()
    };

    let item = row_uniqueness_evidence(&columns, &summary);

    assert!(
        item.detail.contains(
            "Removing birth_date from the file would leave 3 of them unique instead of 412"
        ),
        "the one actionable sentence has to name the column and both counts, got: {}",
        item.detail
    );
    // The bound on that number, which nothing pinned: this arm hands over an actionable
    // figure, and rewording its tail away used to leave every test green. The Verified arm
    // is the only other place the scope is stated, and a reviewed file never reaches it.
    assert!(
        item.detail
            .contains("counted over the same columns as the figures above and no others"),
        "the counterfactual is over the measured columns only and has to say so, got: {}",
        item.detail
    );
    assert!(
        item.detail
            .contains("group sizes behind it are not re-measured"),
        "removing the column can clear the singletons and still leave pairs, got: {}",
        item.detail
    );
}

/// No column being decisive is a finding of its own, not an absence of one.
///
/// The instruction differs: uniqueness spread across the combination cannot be cleared by
/// changing one column, and a reader shown nothing here would try exactly that.
#[test]
fn no_single_column_carrying_it_is_said_out_loud() {
    let summary = RowUniquenessSummary {
        rows_measured: 40,
        matched_columns: vec![matched(0, MatchedPart::WholeValue)],
        unique_rows: 40,
        // Dropping the column leaves every row still unique, which is the measured null
        // result this test is about — not an absence of measurement.
        drop_column_effects: vec![DropColumnEffect {
            column_index: 0,
            unique_rows_without: 40,
        }],
        ..Default::default()
    };

    let item = row_uniqueness_evidence(&[], &summary);

    assert!(
        item.detail.contains("No single column carries it"),
        "a measured null result has to be stated, got: {}",
        item.detail
    );
    assert!(
        !item.detail.contains("Removing"),
        "and must not read as advice to drop a column that would not help, got: {}",
        item.detail
    );
}

/// An unmeasured attribution says so rather than going quiet.
///
/// Silence is indistinguishable from "no column would help", which is the opposite finding.
#[test]
fn an_unmeasured_attribution_is_distinguished_from_no_column_helping() {
    let summary = RowUniquenessSummary {
        rows_measured: 40,
        matched_columns: vec![matched(0, MatchedPart::WholeValue)],
        unique_rows: 40,
        drop_attribution_incomplete: true,
        ..Default::default()
    };

    let item = row_uniqueness_evidence(&[], &summary);

    assert!(
        item.detail.contains("was not measured on this file"),
        "an unrun attribution has to be visible, got: {}",
        item.detail
    );
    assert!(
        !item.detail.contains("No single column carries it"),
        "and must not be reported as a measured null result, got: {}",
        item.detail
    );
}

/// A file with no unique rows gets no advice about clearing them.
#[test]
fn a_grouped_file_is_given_no_column_to_drop() {
    let summary = RowUniquenessSummary {
        rows_measured: 100,
        matched_columns: vec![matched(0, MatchedPart::WholeValue)],
        unique_rows: 0,
        smallest_class: 2,
        fifth_percentile_class_size: 2,
        // Spelled out although this arm never reads it: an attribution that *is* present is
        // what makes the silence a decision about `unique_rows` rather than an accident of
        // there being nothing to say.
        drop_column_effects: vec![DropColumnEffect {
            column_index: 0,
            unique_rows_without: 0,
        }],
        ..Default::default()
    };

    let item = row_uniqueness_evidence(&[], &summary);

    assert!(
        !item.detail.contains("Removing") && !item.detail.contains("No single column"),
        "there is nothing to clear, so there is nothing to advise, got: {}",
        item.detail
    );
}

/// A column matched only on its blank-cell pattern is named, and named as that.
///
/// The regression test for a variant that fell out of the sentence entirely. `BlankPattern`
/// was added to `MatchedPart` when the missingness leak was made visible, and
/// `counted_column_names` built its groups from an array of four variants that nobody updated
/// — so such a column was counted into the class arithmetic and then named nowhere. With every
/// matched column a blank pattern the list came back empty and the finding read "are unique on
/// ." Nothing caught it, because no test in the crate put a `BlankPattern` through the report.
#[test]
fn a_blank_cell_pattern_is_named_as_what_it_is() {
    let columns = vec![
        column(
            0,
            "address",
            DataType::String,
            AnonymizationStrategy::Redact,
        ),
        column(1, "notes", DataType::String, AnonymizationStrategy::Redact),
    ];
    let summary = RowUniquenessSummary {
        rows_measured: 40,
        matched_columns: vec![
            matched(0, MatchedPart::BlankPattern),
            matched(1, MatchedPart::BlankPattern),
        ],
        unique_rows: 12,
        drop_column_effects: vec![DropColumnEffect {
            column_index: 0,
            unique_rows_without: 2,
        }],
        ..Default::default()
    };

    let item = row_uniqueness_evidence(&columns, &summary);

    assert!(
        item.detail
            .contains("unique on which cells are blank in address, notes"),
        "a blank-pattern column has to be named, and named as a blank pattern, got: {}",
        item.detail
    );
    // The shape of the original defect: an empty list left the sentence trailing off.
    assert!(
        !item.detail.contains("unique on ."),
        "the column list must never come back empty, got: {}",
        item.detail
    );
}

/// Every `MatchedPart` reaches the sentence, so no variant can be silently dropped again.
///
/// The array this replaced compiled fine while missing a variant. `group_order` is exhaustive,
/// so the *build* now catches an unclassified variant — but a variant classified and then not
/// rendered would still slip through, which is what this covers.
#[test]
fn every_matched_part_is_named_in_the_finding() {
    let parts = [
        (MatchedPart::WholeValue, "postcode"),
        (MatchedPart::EmailDomain, "the domain of"),
        (MatchedPart::DateDecadeAndTime, "the decade and time of"),
        (MatchedPart::SurvivingFormat, "the surviving format of"),
        (MatchedPart::BlankPattern, "which cells are blank in"),
    ];

    for (part, expected) in parts {
        let columns = vec![column(
            0,
            "postcode",
            DataType::String,
            AnonymizationStrategy::PassThrough,
        )];
        let summary = RowUniquenessSummary {
            rows_measured: 40,
            matched_columns: vec![matched(0, part)],
            unique_rows: 40,
            drop_attribution_incomplete: true,
            ..Default::default()
        };

        let item = row_uniqueness_evidence(&columns, &summary);
        assert!(
            item.detail.contains(expected),
            "{part:?} must reach the sentence as {expected:?}, got: {}",
            item.detail
        );
    }
}

/// The groups come out strongest claim first, whatever order the columns sit in.
///
/// `group_order` documents that ordering and nothing checked it: giving `BlankPattern` the
/// same slot as `WholeValue` left every test green, because grouping is by variant and only
/// the reading order moves. Order is the point of that function, though — a reader who stops
/// after the first clause should have read the strongest claim, not the weakest.
#[test]
fn matched_groups_are_ordered_strongest_claim_first() {
    let named = |index: usize, name: &str| {
        column(
            index,
            name,
            DataType::String,
            AnonymizationStrategy::PassThrough,
        )
    };
    let columns = vec![
        named(0, "blanks"),
        named(1, "shape"),
        named(2, "birth"),
        named(3, "mail"),
        named(4, "postcode"),
    ];
    // Deliberately listed weakest first, so the assertion cannot pass by echoing the input.
    let summary = RowUniquenessSummary {
        rows_measured: 40,
        matched_columns: vec![
            matched(0, MatchedPart::BlankPattern),
            matched(1, MatchedPart::SurvivingFormat),
            matched(2, MatchedPart::DateDecadeAndTime),
            matched(3, MatchedPart::EmailDomain),
            matched(4, MatchedPart::WholeValue),
        ],
        unique_rows: 40,
        drop_attribution_incomplete: true,
        ..Default::default()
    };

    let item = row_uniqueness_evidence(&columns, &summary);

    assert!(
        item.detail.contains(
            "unique on postcode; the domain of mail; the decade and time of birth; \
             the surviving format of shape; which cells are blank in blanks"
        ),
        "groups have to read strongest claim first, got: {}",
        item.detail
    );
}
