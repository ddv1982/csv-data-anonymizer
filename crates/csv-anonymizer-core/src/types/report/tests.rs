use super::*;
fn distribution(distinct: usize, total: usize) -> ColumnValueDistribution {
    ColumnValueDistribution {
        column_index: 0,
        distinct_values: distinct,
        total_values: total,
        // No singletons, so coverage is 1.0 and the ratio test is reachable.
        // The shapes that make coverage matter are pinned separately below.
        singleton_values: 0,
        doubleton_values: 0,
        max_value_occurrences: 0,
    }
}

/// The floor is the whole reason this predicate is not just `distinct < 10`.
/// `distinct_values <= total_values`, so below the floor the absolute test is
/// vacuously true and every short column — including unique-key columns — looks
/// low-cardinality. Measured: at 5 rows all 7 columns of `large.csv` trip it.
#[test]
fn cardinality_risk_ignores_columns_with_too_few_values_to_judge() {
    // The vacuous case: every value distinct, yet fewer than the constant.
    assert!(!distribution(5, 5).risks_frequency_inversion());
    assert!(!distribution(49, 49).risks_frequency_inversion());
    // Genuinely low cardinality, but still not enough rows to claim it.
    assert!(!distribution(4, 49).risks_frequency_inversion());
    // One value more and the same shape is judgeable.
    assert!(distribution(4, 50).risks_frequency_inversion());
}

#[test]
fn cardinality_risk_flags_few_distinct_values_at_the_boundary() {
    assert!(distribution(9, 60).risks_frequency_inversion());
    // Ten distinct over sixty is 0.167, above the ratio threshold too, so this
    // pins both terms at once: neither may fire here.
    assert!(!distribution(10, 60).risks_frequency_inversion());
}

/// The ratio term exists for columns the absolute term cannot reach. Measured on
/// `large.csv name`: 100 distinct over 10500 values.
#[test]
fn cardinality_risk_flags_a_large_column_by_ratio() {
    assert!(distribution(100, 10_500).risks_frequency_inversion());
    // Same distinct count, far fewer rows: 0.1 is above the threshold, and the
    // absolute term does not reach 100 either.
    assert!(!distribution(100, 1_000).risks_frequency_inversion());
}

/// Below 200 values the ratio term is subsumed by the absolute one, which is why
/// 0.05 was chosen: small and medium inputs are judged on the statistic that
/// saturates rather than on one that keeps drifting as more rows are read.
#[test]
fn the_ratio_term_is_inert_below_two_hundred_values() {
    for total in [50usize, 100, 199] {
        for distinct in 10..=total {
            let subject = distribution(distinct, total);
            assert!(
                !subject.risks_frequency_inversion(),
                "{distinct} distinct over {total} fired without the absolute term"
            );
        }
    }
}

/// Counts the values a transform would count: empties skipped, case and padding
/// folded. A pre-run warning that measured differently from the run would
/// contradict the report built from the same data.
#[test]
fn distribution_from_values_matches_the_transform_value_identity() {
    let values = [
        "Sales".to_string(),
        "  sales  ".to_string(),
        "SALES".to_string(),
        "Legal".to_string(),
        String::new(),
        "null".to_string(),
        "NULL".to_string(),
        "   ".to_string(),
    ];

    let subject = ColumnValueDistribution::from_values(3, &values);

    assert_eq!(subject.column_index, 3);
    assert_eq!(subject.distinct_values, 2);
    assert_eq!(subject.total_values, 4);
    assert_eq!(subject.singleton_values, 1);
    assert_eq!(subject.doubleton_values, 0);
    assert_eq!(subject.max_value_occurrences, 3);
}

/// Singletons and doubletons are counted after identity folding too, since they
/// are what [`ColumnValueDistribution::estimated_distinct_values`] reads.
#[test]
fn distribution_counts_singletons_and_doubletons() {
    let values = ["a", "a", "b", "b", "c", "d", "d", "d"]
        .map(str::to_string)
        .to_vec();

    let subject = ColumnValueDistribution::from_values(0, &values);

    assert_eq!(subject.distinct_values, 4);
    assert_eq!(subject.singleton_values, 1, "c");
    assert_eq!(subject.doubleton_values, 2, "a and b");
}

/// A sampled column of a large file is the case the ratio test could not see. The
/// numbers are the measured ones for 30 departments over 5000 rows.
#[test]
fn a_sampled_distribution_is_judged_against_the_columns_real_size() {
    let sampled = ColumnValueDistribution {
        column_index: 1,
        distinct_values: 29,
        total_values: 100,
        singleton_values: 5,
        doubleton_values: 4,
        max_value_occurrences: 8,
    };

    // Against the sample's own size the ratio is 0.29 and the absolute term does
    // not reach 29 either, so judged this way the column looks safe.
    assert!(!sampled.risks_frequency_inversion());
    // Against the file it was drawn from, it is 30-odd values over 5000 rows.
    assert!(sampled.frequency_inversion_risk_in(5_000).is_some());
}

/// The counterweight, and why the ratio test cannot simply divide by the row
/// count: a sample of a hundred cannot look like a million distinct values, so a
/// fully unique column scores 0.005 on the ratio alone. Coverage is what
/// distinguishes "few distinct values" from "a sample that learned nothing".
#[test]
fn a_sample_that_learned_nothing_cannot_raise_a_warning() {
    let every_value_new = ColumnValueDistribution {
        column_index: 0,
        distinct_values: 100,
        total_values: 100,
        singleton_values: 100,
        doubleton_values: 0,
        max_value_occurrences: 1,
    };

    assert!(
        every_value_new
            .frequency_inversion_risk_in(1_000_000)
            .is_none()
    );
}

/// Coverage gates the ratio term only. A column with few enough distinct values
/// is invertible whatever the coverage figure says, so the absolute term has to
/// answer first.
#[test]
fn the_absolute_term_is_not_gated_by_coverage() {
    let sparse_but_unsaturated = ColumnValueDistribution {
        column_index: 0,
        distinct_values: 9,
        total_values: 60,
        // Coverage 0.0, far below the gate.
        singleton_values: 60,
        doubleton_values: 0,
        max_value_occurrences: 1,
    };

    assert!(
        sparse_but_unsaturated
            .frequency_inversion_risk_in(1_000_000)
            .is_some()
    );
}

/// Chao1 estimates the groups a sample missed from how many it saw exactly once.
/// Pinned on the measured shape rather than the formula, so a rewrite that keeps
/// the behaviour passes and one that changes it fails.
#[test]
fn the_estimated_distinct_count_allows_for_unseen_values() {
    let sampled = ColumnValueDistribution {
        column_index: 0,
        distinct_values: 29,
        total_values: 100,
        singleton_values: 5,
        doubleton_values: 4,
        max_value_occurrences: 8,
    };
    // 29 + 25/8 = 32, against a true count of 30.
    assert_eq!(sampled.estimated_distinct_values(), 32);

    // Nothing seen twice: the bias-corrected form, and no division by zero.
    let no_doubletons = ColumnValueDistribution {
        distinct_values: 10,
        total_values: 100,
        singleton_values: 4,
        doubleton_values: 0,
        ..ColumnValueDistribution::default()
    };
    assert_eq!(no_doubletons.estimated_distinct_values(), 10 + 6);

    // Every value repeated: nothing was missed.
    let fully_measured = ColumnValueDistribution {
        distinct_values: 30,
        total_values: 5_000,
        singleton_values: 0,
        doubleton_values: 0,
        ..ColumnValueDistribution::default()
    };
    assert_eq!(fully_measured.estimated_distinct_values(), 30);
}

#[test]
fn distribution_from_no_usable_values_is_empty_rather_than_risky() {
    let values = [String::new(), "null".to_string(), "  ".to_string()];

    let subject = ColumnValueDistribution::from_values(0, &values);

    assert_eq!(subject.total_values, 0);
    assert_eq!(subject.max_value_occurrences, 0);
    assert!(!subject.risks_frequency_inversion());
}

/// A default distribution is what a caller that never measured produces, so it
/// must never be able to raise a warning it has no evidence for.
#[test]
fn a_defaulted_distribution_cannot_raise_a_warning() {
    assert!(!ColumnValueDistribution::default().risks_frequency_inversion());
}

/// One dominant value over forty-nine near-unique ones.
///
/// Not invented: these are the exact figures the detection sample computes for
/// `tests/fixtures/dominant-value.csv`, pinned in
/// `service::tests::cardinality::the_dominant_value_fixture_has_the_shape_these_tests_depend_on`,
/// so the unit tests here and the integration tests there are reasoning about one
/// column rather than two that happen to resemble each other.
fn dominant_value_sample() -> ColumnValueDistribution {
    ColumnValueDistribution {
        column_index: 1,
        distinct_values: 50,
        total_values: 100,
        singleton_values: 49,
        doubleton_values: 0,
        max_value_occurrences: 51,
    }
}

/// The gap this term closes. A column that is *diverse* — here 50 distinct values in
/// the sample and thousands in the file — but whose most common value covers half the
/// rows leaks half the column the moment one pseudonym is matched, and until this term
/// existed it drew no warning at all: 50 distinct values is far above the absolute
/// limit, and the Chao1 ratio is high precisely because the column is diverse.
///
/// Both older terms are asserted against as well as the verdict. Without them a change
/// that made the *absolute* term start firing at 50 distinct values would leave this
/// test green while flagging most columns in the corpus.
#[test]
fn a_dominant_value_is_flagged_however_diverse_the_rest_of_the_column_is() {
    let subject = dominant_value_sample();

    assert!(subject.distinct_values >= MAX_INVERTIBLE_DISTINCT_VALUES);
    assert!(subject.sample_coverage() < MIN_SAMPLE_COVERAGE);

    assert!(subject.frequency_inversion_risk_in(5_000_000).is_some());
}

/// The same column with the dominance taken out of it, and nothing else changed.
///
/// Half the rows moved off the dominant value onto values of their own, so the column
/// is *more* diverse than the one above while holding the same 100 values. It has to
/// go silent, or the term is reading something other than dominance.
#[test]
fn the_same_column_without_a_dominant_value_stays_silent() {
    let spread_out = ColumnValueDistribution {
        distinct_values: 76,
        singleton_values: 75,
        max_value_occurrences: 25,
        ..dominant_value_sample()
    };

    assert!(spread_out.frequency_inversion_risk_in(5_000_000).is_none());
}

/// The dominant-value term is deliberately ahead of the coverage gate, and this is
/// the reason: the shape it catches is singleton-heavy, so its coverage is low. The
/// fixture's coverage is 0.51 against a 0.75 gate. Behind the gate this term would have
/// been silent on almost every column it was added for: of 2400 measured columns whose
/// top value covered half the rows, none reached coverage 0.75, and of 2400 covering
/// three fifths, six did.
#[test]
fn the_dominant_value_term_is_not_gated_by_coverage() {
    let subject = dominant_value_sample();

    assert!(subject.sample_coverage() < MIN_SAMPLE_COVERAGE);
    assert!(subject.frequency_inversion_risk_in(5_000_000).is_some());
}

/// A share and not a count, which is the whole point of the constant: the identical
/// shape has to answer the same way at every file size. A count-based rule would
/// have to be either silent on the small file or noisy on the large one.
#[test]
fn the_dominant_value_verdict_does_not_move_with_the_files_size() {
    for population in [100usize, 5_000, 1_000_000, 5_000_000] {
        assert!(
            dominant_value_sample()
                .frequency_inversion_risk_in(population)
                .is_some(),
            "silent at {population} values"
        );
    }
}

/// The boundary, measured on a fully counted column so no estimate is involved:
/// 20 of 60 is exactly a third and fires, 19 of 60 does not.
///
/// The surrounding counts are chosen so neither other term can answer — 15 distinct
/// clears the absolute limit, and Chao1 over the column's own 60 values is 0.67,
/// nowhere near the 0.05 ratio limit — so the verdict here is the dominant-value
/// term's alone.
#[test]
fn the_dominant_share_boundary_sits_at_one_third() {
    let at_the_boundary = ColumnValueDistribution {
        column_index: 0,
        distinct_values: 15,
        total_values: 60,
        singleton_values: 10,
        doubleton_values: 2,
        max_value_occurrences: 20,
    };
    assert!(at_the_boundary.risks_frequency_inversion());

    let one_row_short = ColumnValueDistribution {
        max_value_occurrences: 19,
        ..at_the_boundary
    };
    assert!(!one_row_short.risks_frequency_inversion());
}

/// A genuinely unique column has to stay silent, and the dominant-value term must
/// not be what breaks that. Its most common value covers one row, so its share is
/// `1 / total_values` — below a third for any column past the floor, which is why
/// the term needs no special case for it. Pinned at four sizes because a share is
/// exactly the kind of quantity that misbehaves at the small end.
#[test]
fn a_unique_column_cannot_trip_the_dominant_value_term() {
    for total in [50usize, 60, 5_000, 1_000_000] {
        let unique = ColumnValueDistribution {
            column_index: 0,
            distinct_values: total,
            total_values: total,
            singleton_values: total,
            doubleton_values: 0,
            max_value_occurrences: 1,
        };

        assert!(
            unique.frequency_inversion_risk_in(total).is_none(),
            "{total} unique values were flagged"
        );
    }
}

/// The false-positive case the constant was calibrated against. This is a measured
/// 100-value sample of a Zipf column with exponent 1.0 over 1000 labels — the
/// ordinary shape of real categorical data, whose top value takes a seventh of the
/// rows. Over 4000 such samples no draw reached a third, and a threshold low enough
/// to catch this shape would fire on most text columns in most files.
#[test]
fn a_mildly_skewed_high_cardinality_column_stays_silent() {
    let ordinary_skew = ColumnValueDistribution {
        column_index: 0,
        distinct_values: 68,
        total_values: 100,
        singleton_values: 55,
        doubleton_values: 8,
        max_value_occurrences: 14,
    };

    assert!(ordinary_skew.frequency_inversion_risk_in(100_000).is_none());
}
