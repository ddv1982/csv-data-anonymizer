use super::*;

/// A released column, named after its position.
///
/// Selected, because an unselected column is written through unchanged and
/// [`LinkableProjection::for_column`] reads that as `WholeValue` whatever the strategy
/// says — so an unselected fixture would silently measure a different column from the one
/// its `strategy` argument describes. The tests that mean an unselected column clear the
/// flag themselves, where it is visible.
fn column(
    index: usize,
    detected_type: DataType,
    strategy: AnonymizationStrategy,
) -> ColumnMetadata {
    crate::test_support::selected_column(index, &format!("column{index}"), detected_type, strategy)
}

/// Columns matched on some part of their released value, by column index.
///
/// The summary carries one entry per column paired with *what* was matched, because a report
/// that only knows "linkable" cannot tell a released postcode from a shifted date. These two
/// helpers recover the coarser split the class arithmetic turns on, which is all most of
/// these tests care about; the tests that care about the pairing assert `matched_on` directly.
fn value_columns(summary: &RowUniquenessSummary) -> Vec<usize> {
    summary
        .matched_columns
        .iter()
        .filter(|matched| matched.matched_on != MatchedPart::SurvivingFormat)
        .map(|matched| matched.column_index)
        .collect()
}

fn format_columns(summary: &RowUniquenessSummary) -> Vec<usize> {
    summary
        .matched_columns
        .iter()
        .filter(|matched| matched.matched_on == MatchedPart::SurvivingFormat)
        .map(|matched| matched.column_index)
        .collect()
}

fn record(rows: &[&[&str]], columns: &[ColumnMetadata]) -> RowUniquenessSummary {
    let mut tracker = RowUniquenessTracker::default();
    for row in rows {
        let owned = row
            .iter()
            .map(|value| (*value).to_string())
            .collect::<Vec<_>>();
        tracker.record_row(&owned, columns);
    }
    tracker
        .summary()
        .expect("rows were recorded, so a summary must exist")
}

/// The arithmetic, against a class structure written out by hand.
///
/// Ten rows in classes of 4, 3, 2 and 1. Asserting every field of one known case is what
/// makes the percentile and the singleton count checkable at all — each is a one-line
/// expression whose off-by-one would otherwise pass every less specific test.
#[test]
fn class_structure_is_counted_exactly() {
    let columns = vec![column(
        0,
        DataType::String,
        AnonymizationStrategy::PassThrough,
    )];
    let rows: Vec<&[&str]> = vec![
        &["a"],
        &["a"],
        &["a"],
        &["a"],
        &["b"],
        &["b"],
        &["b"],
        &["c"],
        &["c"],
        &["d"],
    ];

    let summary = record(&rows, &columns);

    assert_eq!(summary.rows_measured, 10);
    assert_eq!(summary.distinct_classes, 4);
    assert_eq!(summary.unique_rows, 1);
    assert_eq!(summary.smallest_class, 1);
    // 5% of 10 rows rounds up to 1, and the smallest class holds that row.
    assert_eq!(summary.fifth_percentile_class_size, 1);
    assert_eq!(value_columns(&summary), vec![0]);
}

/// The percentile has to disagree with the floor, or it is not earning its place.
///
/// Twenty rows: one alone, the rest in a class of nineteen. The floor says 1 and the
/// population is in fact comfortably grouped — which is the whole reason both figures are
/// reported rather than just the smaller one.
#[test]
fn fifth_percentile_is_not_dragged_by_one_freak_row() {
    let columns = vec![column(
        0,
        DataType::String,
        AnonymizationStrategy::PassThrough,
    )];
    let mut rows = vec![vec!["shared"]; 19];
    rows.push(vec!["alone"]);
    let rows = rows.iter().map(Vec::as_slice).collect::<Vec<_>>();

    let summary = record(&rows, &columns);

    assert_eq!(summary.smallest_class, 1);
    assert_eq!(summary.unique_rows, 1);
    // 5% of 20 is 1 row, which is the lone one, so the percentile agrees here...
    assert_eq!(summary.fifth_percentile_class_size, 1);

    // ...and stops agreeing as soon as the lone row is under 5% of the population.
    let mut rows = vec![vec!["shared"]; 99];
    rows.push(vec!["alone"]);
    let rows = rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let summary = record(&rows, &columns);
    assert_eq!(summary.smallest_class, 1);
    assert_eq!(summary.fifth_percentile_class_size, 99);
}

/// The regression test for the subset rule, and the one most likely to be broken by a
/// later well-meaning change.
///
/// A tokenized primary key is distinct on every row. Counting it would make every row
/// unique and the metric would fire on every file that has a primary key — which is
/// nearly all of them — so the number would stop being read. It contributes nothing here
/// because an attacker cannot derive the token from anything they know.
#[test]
fn opaque_token_column_does_not_make_rows_unique() {
    let columns = vec![
        column(0, DataType::Uuid, AnonymizationStrategy::Tokenize),
        column(1, DataType::Enum, AnonymizationStrategy::PassThrough),
    ];
    let rows: Vec<&[&str]> = vec![
        &["tok-1", "north"],
        &["tok-2", "north"],
        &["tok-3", "south"],
    ];

    let summary = record(&rows, &columns);

    assert_eq!(value_columns(&summary), vec![1]);
    assert_eq!(summary.distinct_classes, 2);
    assert_eq!(summary.unique_rows, 1);
    // The all-column figure still sees the token, and still says every row is distinct.
    // The two disagreeing is the intended behaviour, not a bug: they answer different
    // questions, and their gap is the measure of how much the subset rule is doing.
    assert_eq!(summary.distinct_rows_all_columns, Some(3));
}

/// Labels and redactions are equally unmatchable, for the same reason as tokens: an
/// ordinal and a constant are not things an outsider can compute from a person's data.
#[test]
fn labelled_and_redacted_columns_are_excluded() {
    let columns = vec![
        column(0, DataType::FullName, AnonymizationStrategy::Label),
        column(1, DataType::Address, AnonymizationStrategy::Redact),
    ];
    let rows: Vec<&[&str]> = vec![&["name 1", "[redacted]"], &["name 2", "[redacted]"]];

    let summary = record(&rows, &columns);

    assert!(value_columns(&summary).is_empty());
    // No linkable column means one class holding everyone. That is not a finding of
    // safety and Phase 2's wording must not read as one, but it is the correct count.
    assert_eq!(summary.distinct_classes, 1);
    assert_eq!(summary.unique_rows, 0);
    assert_eq!(summary.smallest_class, 2);
}

/// A masked value is unreadable and still perfectly matchable: anyone who knows the name
/// "Jan de Vries" can write down `*** ** *****` and filter on it.
///
/// Counted on the whole cell, and reported as a *format*. Both halves matter. It used to be
/// classified `WholeValue`, which licenses naming the column bare, and the report then said
/// "Every released row shares full_name, city with at least 7 other(s)" about a file whose
/// names had all been replaced by stars — telling the reader the names were in the file.
#[test]
fn masked_column_is_linkable_by_its_skeleton() {
    let columns = vec![column(0, DataType::FullName, AnonymizationStrategy::Mask)];
    let rows: Vec<&[&str]> = vec![&["*** ** *****"], &["*** ** *****"], &["**** *****"]];

    let summary = record(&rows, &columns);

    assert_eq!(format_columns(&summary), vec![0]);
    assert!(value_columns(&summary).is_empty());
    assert_eq!(summary.distinct_classes, 2);
    assert_eq!(summary.unique_rows, 1);
}

/// A column whose part only some rows carry is marked as such, and the counts stay put.
///
/// `matched_on` is decided once per column, from its strategy and detected type, and no cell
/// can talk it out of that. A `Timestamp` column where one value parses and the rest were
/// pseudonymized generically is still `DateDecadeAndTime`, so the finding said the rows
/// "share the decade and time of column0" of three rows carrying no decade at all.
///
/// The arithmetic was never the problem, and this pins that too: the non-parsing rows project
/// to nothing and land in one class together, which is exactly what an outsider holding the
/// originals gets. Only the sentence over-claimed.
#[test]
fn a_column_only_some_rows_carry_is_marked_as_partial() {
    let columns = vec![column(0, DataType::Timestamp, AnonymizationStrategy::Auto)];
    let rows: Vec<&[&str]> = vec![
        &["1984-02-11T09:15:22Z"],
        &["qwertyuiopasdf"],
        &["Zk9bQm2Rw4Lp"],
        &["nothing-date-like"],
    ];

    let summary = record(&rows, &columns);

    assert_eq!(
        summary.matched_columns,
        vec![MatchedColumn {
            column_index: 0,
            matched_on: MatchedPart::DateDecadeAndTime,
            matched_every_row: false,
        }]
    );
    // One class for the parsed date, one for the three that projected to nothing.
    assert_eq!(summary.distinct_classes, 2);
    assert_eq!(summary.unique_rows, 1);
}

/// An empty projection is not always an absent one, and conflating them got this wrong.
///
/// `BlankCellPattern` returns the empty string for a cell with something in it — "not blank"
/// is the projection succeeding, and the pattern of which cells are blank is reproducible
/// from the original on every row. The first version of the partial-match flag tested
/// `!projected.is_empty()` and so reported every blank-pattern column in the suite as
/// matching only some of its rows. `WholeValue` has the same shape: a genuinely empty
/// released cell is the whole value, not a missing one.
#[test]
fn an_empty_projection_that_means_something_is_not_a_partial_match() {
    let columns = vec![
        column(0, DataType::Email, AnonymizationStrategy::Redact),
        column(1, DataType::String, AnonymizationStrategy::PassThrough),
    ];
    // Column 0 is redacted, so it is matched on its blank-cell pattern and every row here has
    // something in it. Column 1 passes through, and one of its cells is genuinely empty.
    let rows: Vec<&[&str]> = vec![&["kept", "alpha"], &["kept", ""], &["kept", "beta"]];

    let summary = record(&rows, &columns);

    assert!(
        summary
            .matched_columns
            .iter()
            .all(|matched| matched.matched_every_row),
        "no column here fails to apply on any row, got: {:?}",
        summary.matched_columns
    );
}

/// Pseudonymization keeps the domain after `@`, so the column sorts rows by employer and
/// nothing finer. Hashing the whole cell instead would report every row as unique on a
/// column that in truth only separates two companies.
#[test]
fn pseudonymized_email_links_on_its_domain_only() {
    let columns = vec![column(0, DataType::Email, AnonymizationStrategy::Auto)];
    let rows: Vec<&[&str]> = vec![
        &["user1@corp.com"],
        &["user2@corp.com"],
        &["user3@other.example"],
    ];

    let summary = record(&rows, &columns);

    assert_eq!(
        summary.matched_columns,
        vec![MatchedColumn {
            column_index: 0,
            matched_on: MatchedPart::EmailDomain,
            matched_every_row: true,
        }]
    );
    assert_eq!(summary.distinct_classes, 2);
    assert_eq!(summary.unique_rows, 1);
    assert_eq!(summary.distinct_rows_all_columns, Some(3));
}

/// The time of day is kept exactly, sub-second digits included, so two events at the same
/// instant in the same decade stay in one class however far apart their released dates land
/// inside it.
#[test]
fn pseudonymized_timestamp_links_on_its_time_of_day() {
    let columns = vec![column(0, DataType::Timestamp, AnonymizationStrategy::Auto)];
    let rows: Vec<&[&str]> = vec![
        &["2021-03-04T09:15:22.481Z"],
        &["2024-11-30T09:15:22.481Z"],
        &["2020-01-01T23:59:00Z"],
    ];

    let summary = record(&rows, &columns);

    // Same decade and same instant for the first two; the third differs on its time.
    assert_eq!(summary.distinct_classes, 2);
    assert_eq!(summary.unique_rows, 1);
}

/// A date with no time is the case the exact-reproducibility rule got wrong, and the reason
/// one projection is allowed to be approximate.
///
/// Under that rule a date-only birth date projected to the empty string on every row: the
/// column was scored at zero, and a file of twenty distinct birth dates was reported as
/// every row sharing its combination with nineteen others. The textbook quasi-identifier,
/// measured as carrying no information at all.
///
/// The decade is what an attacker holding the original can actually filter on, given a
/// shift of up to a year in either direction. So rows a decade apart separate, and rows
/// inside one decade do not — which is neither the "everything is unique" a released year
/// would have produced nor the "nothing counts" it replaced.
#[test]
fn date_only_values_link_on_their_decade_and_not_on_nothing() {
    let columns = vec![column(0, DataType::Timestamp, AnonymizationStrategy::Auto)];
    let rows: Vec<&[&str]> = vec![
        &["1984-02-11"],
        &["1987-09-30"],
        &["1989-01-01"],
        &["1994-06-15"],
    ];

    let summary = record(&rows, &columns);

    // Counted, and named as what it is: a decade, not a date. The report reads that pairing
    // directly, which is what stops it asserting rows share a birth date when they share a
    // decade.
    assert_eq!(
        summary.matched_columns,
        vec![MatchedColumn {
            column_index: 0,
            matched_on: MatchedPart::DateDecadeAndTime,
            matched_every_row: true,
        }]
    );
    // Three in the eighties, one in the nineties.
    assert_eq!(summary.distinct_classes, 2);
    assert_eq!(summary.unique_rows, 1);
    assert_eq!(summary.smallest_class, 1);
}

/// A cell that did not match its column's detected shape is replaced by a generic
/// pseudonym, so a timestamp column can hold a value with no date in it. Slicing that at
/// the ten-character boundary anyway would hash arbitrary characters as if they were a
/// time; an empty projection is the honest reading, since there is nothing left to link on.
#[test]
fn shape_fallback_value_projects_to_nothing() {
    assert_eq!(
        LinkableProjection::TimestampDecadeAndTimeOfDay.apply("qwertyuiopasdf"),
        None
    );
    assert_eq!(
        LinkableProjection::EmailDomain.apply("no-at-sign-here"),
        None
    );
    assert_eq!(
        LinkableProjection::TimestampDecadeAndTimeOfDay
            .apply("2021-03-04T09:15:22Z")
            .as_deref(),
        Some("202|T09:15:22Z")
    );
    // The phone layout is the projection that used to have no such guard, and the one where
    // its absence was visible: this is a generic-string pseudonym, and digit-masking it
    // published a class key made of leftover random letters.
    assert_eq!(LinkableProjection::PhoneDialLayout.apply("Qk7bZm2"), None);
    // Both halves of `is_phone_shaped`, separately. The first fails on digit count alone; the
    // second has eleven digits and fails only on the letters, which is the half a fixture of
    // short pseudonyms never reaches — so deleting the character-set clause used to leave
    // every one of these tests green.
    assert_eq!(LinkableProjection::PhoneDialLayout.apply("06-1234"), None);
    assert_eq!(
        LinkableProjection::PhoneDialLayout.apply("Qk7bZm2Rw9Lp4"),
        None
    );
}

/// A year outside `0..=9999` is still a date.
///
/// `chrono` writes one in expanded form — `+10000-01-01` — which a fixed ten-character prefix
/// rejected, so such a row projected to nothing and merged with every unparseable row in the
/// file. Reachable only from a source date at the end of the supported range plus a positive
/// shift, and worth fixing anyway because merging under-states risk, which is the one
/// direction this module may not fail in.
#[test]
fn an_expanded_year_is_read_as_a_date_and_not_as_noise() {
    assert_eq!(
        LinkableProjection::TimestampDecadeAndTimeOfDay
            .apply("+10000-01-01T00:00:00")
            .as_deref(),
        Some("+1000|T00:00:00")
    );
    // A negative year keeps its sign in the decade, so 1980 BCE cannot share a class with
    // 1980 CE.
    assert_eq!(
        LinkableProjection::TimestampDecadeAndTimeOfDay
            .apply("-1984-02-11")
            .as_deref(),
        Some("-198|")
    );
    assert_ne!(
        LinkableProjection::TimestampDecadeAndTimeOfDay.apply("-1984-02-11"),
        LinkableProjection::TimestampDecadeAndTimeOfDay.apply("1984-02-11")
    );
    // And the guard still holds: a value with no date in it projects to nothing rather than
    // being sliced at some offset the parser guessed.
    for noise in [
        "",
        "2024",
        "20-06-15",
        "qwertyuiopasdf",
        "2024-06-1é5",
        "日本語です",
    ] {
        assert_eq!(
            LinkableProjection::TimestampDecadeAndTimeOfDay.apply(noise),
            None,
            "{noise:?} holds no date"
        );
    }
}

/// A redrawn UUID with no letters in it is upper case and lower case at once.
///
/// Folding it into either would merge it with values whose original case is known, which
/// under-states risk about one value in seven million. Its own class over-states instead, and
/// is also the honest reading: an outsider filtering on case cannot place such a value either.
#[test]
fn a_uuid_with_no_letters_is_neither_case() {
    assert_eq!(
        LinkableProjection::UuidLetterCase
            .apply("55008400-2290-4104-1716-446655440000")
            .as_deref(),
        Some("no-letters")
    );
    assert_eq!(
        LinkableProjection::UuidLetterCase
            .apply("550E8400-E29B-41D4-A716-446655440000")
            .as_deref(),
        Some("upper")
    );
    assert_eq!(
        LinkableProjection::UuidLetterCase
            .apply("550e8400-e29b-41d4-a716-446655440000")
            .as_deref(),
        Some("other")
    );
}

/// The tracker resolves its columns from the first row and cannot be rebound.
///
/// Correct for every caller — a run's column metadata is decided before its first row — and
/// silently wrong for one that varied it, since every recorded position would then describe a
/// different column than the value being read. Asserted in debug rather than defended against,
/// because the honest repair for a varying shape is a second tracker.
#[test]
#[should_panic(expected = "cannot be rebound")]
fn rebinding_the_columns_mid_run_is_caught() {
    let first = vec![column(
        0,
        DataType::String,
        AnonymizationStrategy::PassThrough,
    )];
    let second = vec![column(
        7,
        DataType::String,
        AnonymizationStrategy::PassThrough,
    )];
    let mut tracker = RowUniquenessTracker::default();

    tracker.record_row(&["a".to_string()], &first);
    tracker.record_row(&["b".to_string()], &second);
}

/// The digit placeholder must be a character `is_phone_shaped` rejects, or the projection is
/// not injective on the layouts it exists to separate.
///
/// `'#'` was the placeholder and is an accepted phone character — the DTMF hash key — so a
/// ten-digit number ending in `#` and an eleven-digit number collapsed to the same key. Two
/// separately-filterable dial layouts in one class makes classes look larger and the file
/// look safer, which is the direction `hash_fields` was widened to 128 bits to avoid.
#[test]
fn the_digit_placeholder_cannot_occur_in_a_phone_value() {
    // The property itself, not one value of it. The first version of this test asserted
    // `!is_phone_shaped("\0\0\0\0\0\0\0")`, which passes for *every* character — seven
    // non-digits fail the digit-count gate before the character set is ever consulted — so
    // `'('`, `'x'` or a space as the placeholder would have reintroduced the collision and
    // kept the test green. Seven digits alongside the placeholder is what reaches the clause
    // that matters.
    assert!(
        !is_phone_shaped(&format!("1234567{DIGIT_PLACEHOLDER}")),
        "the placeholder must be a character no phone-shaped value can contain"
    );
    // And the collision it was chosen to prevent: `'#'` is accepted as the DTMF hash key, so
    // a ten-digit number ending in `#` and an eleven-digit number both masked to eleven
    // identical characters.
    assert!(
        is_phone_shaped("0612345678#"),
        "the fixture must be phone-shaped"
    );
    assert_ne!(
        LinkableProjection::PhoneDialLayout.apply("0612345678#"),
        LinkableProjection::PhoneDialLayout.apply("06123456785")
    );
}

/// A projection that never yields anything must leave its column out of the finding.
///
/// Every value here missed the phone shape and was pseudonymized generically, so nothing
/// reproducible survived the column at all. It used to be named anyway — the column lists
/// were fixed from strategy and type at activation, never from what was extracted — so the
/// empty-subset guard never fired and the file went on to earn a verified claim about a
/// column that had contributed nothing to a single class.
#[test]
fn a_column_that_yields_nothing_is_named_nowhere() {
    let columns = vec![column(0, DataType::Phone, AnonymizationStrategy::Auto)];

    let summary = record(&[&["Qk7bZm2"], &["Xr4pLd9"], &["Bn2vTs6"]], &columns);

    assert!(value_columns(&summary).is_empty());
    assert!(format_columns(&summary).is_empty());
    // One class holding everyone, which is the correct count over an empty subset. What
    // must not happen is a claim about `column0`, and the empty lists are what stop it.
    assert_eq!(summary.distinct_classes, 1);
    assert_eq!(summary.smallest_class, 3);
}

/// A column that contributes on some rows and not others is still a column that contributes.
#[test]
fn contributing_once_is_enough_to_be_named() {
    let columns = vec![column(0, DataType::Phone, AnonymizationStrategy::Auto)];

    let summary = record(&[&["Qk7bZm2"], &["06-12345678"], &["Xr4pLd9"]], &columns);

    assert_eq!(format_columns(&summary), vec![0]);
}

/// A refused Local AI replacement lands on the pseudonymizing transformers, so the column
/// leaks exactly what those leak — and the measure has to report that, because it cannot
/// tell per row whether the model answered.
///
/// This arm read `None`, on the reasoning that a rejected value "is no more reproducible
/// than a pseudonym is" — which is true and is an argument for the opposite conclusion,
/// since every projection in this module exists because pseudonyms are reproducible in
/// part. With no provider configured the whole column takes this path, so the effect was to
/// under-report a whole column's risk.
#[test]
fn local_ai_columns_are_measured_by_what_their_fallback_leaks() {
    let columns = vec![
        column(0, DataType::Email, AnonymizationStrategy::LocalAi),
        // A closed value domain, and the one type whose Local AI fallback differs from its
        // `Pseudonymize` behaviour: the pass-through gate is skipped on this path precisely
        // so a refused value is not published verbatim, so what it gets is a generic-string
        // pseudonym and nothing reproducible survives.
        column(1, DataType::CountryCode, AnonymizationStrategy::LocalAi),
    ];
    let rows: Vec<&[&str]> = vec![
        &["user1@corp.com", "Xq7"],
        &["user2@corp.com", "Bt2"],
        &["user3@other.example", "Lp9"],
    ];

    let summary = record(&rows, &columns);

    assert_eq!(value_columns(&summary), vec![0]);
    assert!(format_columns(&summary).is_empty());
    assert_eq!(summary.distinct_classes, 2);
    assert_eq!(summary.unique_rows, 1);
}

/// An unselected column is written through untouched whatever its `strategy` field says,
/// so it must be read as pass-through and not as the strategy that never ran. Getting
/// this backwards would exclude the one class of column that is guaranteed to hold
/// original values.
#[test]
fn unselected_column_counts_as_released_verbatim() {
    let mut unselected = column(0, DataType::FullName, AnonymizationStrategy::Redact);
    unselected.is_selected = false;

    let summary = record(&[&["Jan de Vries"], &["Anna Bakker"]], &[unselected]);

    assert_eq!(value_columns(&summary), vec![0]);
    assert_eq!(summary.unique_rows, 2);
}

/// Field boundaries have to survive hashing: `["ab", "c"]` and `["a", "bc"]` are different
/// rows and must land in different classes. Without a length prefix they would collide,
/// which merges two rows into one class and under-states how exposed the file is — the one
/// direction this measure must not be wrong in.
#[test]
fn field_boundaries_are_not_hashable_away() {
    let columns = vec![
        column(0, DataType::String, AnonymizationStrategy::PassThrough),
        column(1, DataType::String, AnonymizationStrategy::PassThrough),
    ];

    let summary = record(&[&["ab", "c"], &["a", "bc"]], &columns);

    assert_eq!(summary.distinct_classes, 2);
    assert_eq!(summary.unique_rows, 2);
}

/// The two histograms must not share keys. They hash the same field list whenever every
/// column is linkable, and a shared key space would have them counting into each other.
#[test]
fn the_two_histograms_are_domain_separated() {
    let fields = ["same", "fields"];
    assert_ne!(
        hash_fields(1, fields.iter().copied()),
        hash_fields(2, fields.iter().copied())
    );
}

/// Nothing recorded means nothing measured, and that has to be distinguishable from a
/// file that measured clean. `None` is what the unstructured-text and single-value paths
/// report, and a zeroed summary there would read as a result rather than an absence.
#[test]
fn a_tracker_that_never_saw_a_row_reports_nothing() {
    assert!(RowUniquenessTracker::default().summary().is_none());
}

/// Past the ceiling the measurement stops, the flag is set, and the counts are not
/// reported as though they were the file's. The run itself continues — the caller is in
/// the middle of writing an output file, and a report figure is not worth failing it for.
#[test]
fn passing_the_class_ceiling_stops_measuring_and_says_so() {
    let columns = vec![column(
        0,
        DataType::String,
        AnonymizationStrategy::PassThrough,
    )];
    let mut tracker = RowUniquenessTracker::default();
    for row in 0..=CLASS_CEILING + 1 {
        tracker.record_row(&[format!("value{row}")], &columns);
    }

    let summary = tracker.summary().expect("rows were recorded");

    assert!(summary.measurement_incomplete);
    assert_eq!(summary.distinct_classes, 0);
    assert_eq!(summary.unique_rows, 0);
    // The subset is still reported: which columns were being watched is knowable even
    // when the counts over them are not.
    assert_eq!(value_columns(&summary), vec![0]);
}

/// The all-column histogram outgrowing its ceiling must not take the joint measure with it.
///
/// It fills faster than the other one by construction — its classes are whole rows, the
/// other's are projections of a subset — so a single shared flag meant that every file with
/// more than two million distinct rows reported "not measured" while holding a perfectly
/// good joint measure. The figure that is actually unavailable is the one that overflowed,
/// and it goes absent rather than reading zero.
#[test]
fn the_all_column_histogram_overflows_on_its_own() {
    let columns = vec![
        // One value for everyone, so the joint measure holds a single class however many
        // rows arrive.
        column(0, DataType::Enum, AnonymizationStrategy::PassThrough),
        // Distinct per row and unmatchable, so it fills the all-column map and no other.
        column(1, DataType::Uuid, AnonymizationStrategy::Tokenize),
    ];
    let mut tracker = RowUniquenessTracker::default();
    for row in 0..=CLASS_CEILING + 1 {
        tracker.record_row(&["north".to_string(), format!("tok-{row}")], &columns);
    }

    let summary = tracker.summary().expect("rows were recorded");

    assert!(!summary.measurement_incomplete);
    assert_eq!(summary.distinct_classes, 1);
    assert_eq!(summary.unique_rows, 0);
    assert_eq!(summary.rows_measured, CLASS_CEILING + 2);
    assert_eq!(summary.distinct_rows_all_columns, None);
}

/// The correction's whole justification, in one fixture.
///
/// Two pseudonymized columns whose surviving shapes are a phone dial layout and a count of
/// name parts. Neither singles anybody out alone — the layouts split the file 3/2 and the
/// part counts split it 2/3, and no group on either is smaller than two. Their
/// intersection holds one row.
///
/// The test proves the premise rather than asserting it: each column is measured on its
/// own first, and each reports zero unique rows. The earlier rule dropped both of these
/// columns for having "too few effective values", and so reported this file as having no
/// unique rows at all — a prediction about the size of an effect, made instead of the
/// measurement that would have found it.
#[test]
fn shape_signals_too_weak_alone_still_combine() {
    let phone = column(0, DataType::Phone, AnonymizationStrategy::Auto);
    let name = column(1, DataType::FullName, AnonymizationStrategy::Auto);
    let rows: Vec<&[&str]> = vec![
        &["06-12345678", "Anna Bakker"],
        &["06-23456789", "Piet Jansen"],
        &["06-34567890", "Femke de Wit"],
        &["+31 6 1234 5678", "Jan van Berg"],
        &["+31 6 2345 6789", "Sanne de Vries"],
    ];

    for (position, single) in [(0, &phone), (1, &name)] {
        let alone = rows
            .iter()
            .map(|row| vec![row[position]])
            .collect::<Vec<_>>();
        let alone = alone.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let summary = record(&alone, std::slice::from_ref(single));
        assert_eq!(
            summary.unique_rows, 0,
            "column {position} must single nobody out on its own, or the fixture proves nothing"
        );
    }

    let summary = record(&rows, &[phone, name]);

    // Neither column is reported as carrying a value: what survived is format only.
    assert!(value_columns(&summary).is_empty());
    assert_eq!(format_columns(&summary), vec![0, 1]);
    assert_eq!(summary.distinct_classes, 3);
    assert_eq!(summary.unique_rows, 1);
}

/// Digit count and leading zeros survive `transform_numeric_id_candidate` exactly, so an
/// outsider knowing the original id knows which released cells it could not have become.
#[test]
fn pseudonymized_numeric_id_links_on_its_width() {
    let columns = vec![column(0, DataType::NumericId, AnonymizationStrategy::Auto)];

    let summary = record(&[&["4471"], &["8823"], &["00042"], &["123456"]], &columns);

    assert_eq!(format_columns(&summary), vec![0]);
    // Two four-wide values share a class; the padded five-wide and the six-wide stand out.
    assert_eq!(summary.distinct_classes, 3);
    assert_eq!(summary.unique_rows, 2);
}

/// The one exclusion left on the pseudonymizing side, and it rests on the transform rather
/// than on a judgment about how much the signal is worth: a generic-string pseudonym is
/// 80–120% of the original's length, so nobody can say which released cells a known
/// original could have become. There is no filter to apply and so nothing to count.
#[test]
fn approximate_length_is_not_reproducible_and_stays_excluded() {
    for data_type in [
        DataType::Address,
        DataType::PostalCode,
        DataType::IpAddress,
        DataType::Url,
        DataType::MacAddress,
        DataType::TaxId,
        DataType::String,
        DataType::Unknown,
    ] {
        let columns = vec![column(0, data_type, AnonymizationStrategy::Auto)];
        let summary = record(&[&["one"], &["two"], &["three"]], &columns);
        assert!(
            value_columns(&summary).is_empty() && format_columns(&summary).is_empty(),
            "{data_type:?} survives only as an approximate length and must not be counted"
        );
    }
}

/// Value-carrying and format-only columns are counted into the same classes and reported
/// as two lists. Merging them would tell a reader that a customer id singled their row
/// out when all it contributed was its width.
#[test]
fn value_and_shape_columns_are_counted_together_and_reported_apart() {
    let columns = vec![
        column(0, DataType::PostalCode, AnonymizationStrategy::PassThrough),
        column(1, DataType::NumericId, AnonymizationStrategy::Auto),
    ];
    let rows: Vec<&[&str]> = vec![
        &["1011AB", "4471"],
        &["1011AB", "123456"],
        &["2033CD", "55"],
    ];

    let summary = record(&rows, &columns);

    assert_eq!(value_columns(&summary), vec![0]);
    assert_eq!(format_columns(&summary), vec![1]);
    // The two rows sharing a postcode are split by their id widths, which is the whole
    // contribution the earlier rule could not see.
    assert_eq!(summary.distinct_classes, 3);
    assert_eq!(summary.unique_rows, 3);
}

/// A column whose projection never changes separates nobody from anybody, and must not be
/// named as though it did.
///
/// Four projections never return an empty string — an empty cell reads as `0:0` under
/// `NumericIdWidth` and as `0` under `NameTokenCount` — so "yielded something" was satisfied
/// by three all-blank columns, and a verified finding named all three: "shares the surviving
/// format of column0, column1, column2". The stronger question is whether the projection ever
/// varied.
#[test]
fn a_column_whose_projection_never_changes_is_named_nowhere() {
    let columns = vec![
        column(0, DataType::NumericId, AnonymizationStrategy::Auto),
        column(1, DataType::PostalCode, AnonymizationStrategy::PassThrough),
    ];

    // Same id width on every row, and a postcode that does vary.
    let summary = record(
        &[
            &["4471", "1011AB"],
            &["8823", "2033CD"],
            &["1234", "3055EF"],
        ],
        &columns,
    );

    assert!(
        format_columns(&summary).is_empty(),
        "a constant digit width distinguishes nobody"
    );
    assert_eq!(value_columns(&summary), vec![1]);
}

/// Below two rows nothing can vary, so the weaker test applies there.
///
/// Dropping every column on a one-row file would report "no released column carries anything
/// an outsider could match" about a row that is trivially unique — a reassurance, from the
/// arm whose whole job is not to be one.
#[test]
fn a_single_row_still_names_the_columns_it_released() {
    let columns = vec![column(
        0,
        DataType::PostalCode,
        AnonymizationStrategy::PassThrough,
    )];

    let summary = record(&[&["1011AB"]], &columns);

    assert_eq!(value_columns(&summary), vec![0]);
    assert_eq!(summary.unique_rows, 1);
}

/// The one-row fallback is "did this column yield anything", not "does this column exist".
///
/// Below two rows the measure cannot ask whether a projection *varied*, so it asks the weaker
/// question instead — and the weaker question still has to be answered honestly. A `Timestamp`
/// column holding a value with no date in it projects to nothing, and naming it would tell the
/// reader an outsider can match on a cell that carries nothing matchable.
///
/// Found by mutation: making a constant projection count as yielded left the whole suite
/// green, because every other one-row test uses a projection that does yield.
#[test]
fn a_single_row_does_not_name_a_column_whose_projection_is_empty() {
    let columns = vec![column(
        0,
        DataType::Timestamp,
        AnonymizationStrategy::Pseudonymize,
    )];

    let summary = record(&[&["not-a-timestamp"]], &columns);

    assert!(summary.matched_columns.is_empty());
}

/// A blank cell is written through verbatim before any strategy runs, so even a fully
/// redacted column publishes which of its rows were empty.
///
/// The measure scored that at zero and the report then said "No released column carries
/// anything an outsider could match against data they already hold" about a file whose four
/// released rows were each distinguishable by their null pattern alone. Someone holding the
/// original record knows which of its fields were blank, which is the same rule every other
/// projection here answers to.
#[test]
fn a_published_blank_pattern_is_counted() {
    let columns = vec![
        column(0, DataType::Address, AnonymizationStrategy::Redact),
        column(1, DataType::Address, AnonymizationStrategy::Redact),
    ];
    let rows: Vec<&[&str]> = vec![
        &["NULL", "x"],
        &["x", "NULL"],
        &["x", "x"],
        &["NULL", "NULL"],
    ];

    let summary = record(&rows, &columns);

    assert_eq!(
        summary.matched_columns,
        vec![
            MatchedColumn {
                column_index: 0,
                matched_on: MatchedPart::BlankPattern,
                matched_every_row: true,
            },
            MatchedColumn {
                column_index: 1,
                matched_on: MatchedPart::BlankPattern,
                matched_every_row: true,
            },
        ]
    );
    assert_eq!(summary.distinct_classes, 4);
    assert_eq!(summary.unique_rows, 4);
}

/// ...and a column with no blanks in it costs the reader nothing.
#[test]
fn a_column_without_blanks_is_not_named_for_its_blanks() {
    let columns = vec![column(0, DataType::Address, AnonymizationStrategy::Redact)];

    let summary = record(&[&["[ADDRESS]"], &["[ADDRESS]"]], &columns);

    assert!(summary.matched_columns.is_empty());
}

/// Each projection reads back exactly the property its transformer preserves.
#[test]
fn projections_read_the_property_their_transformer_keeps() {
    let masked = |layout: &str| layout.replace('#', &DIGIT_PLACEHOLDER.to_string());
    assert_eq!(
        LinkableProjection::MaskedSkeleton
            .apply("*** ** *****")
            .as_deref(),
        Some("*** ** *****")
    );
    assert_eq!(
        LinkableProjection::PhoneDialLayout
            .apply("+31 (0)6-12345678")
            .as_deref(),
        Some(masked("+## (#)#-########").as_str())
    );
    // Five wide, three leading zeros.
    assert_eq!(
        LinkableProjection::NumericIdWidth.apply("00042").as_deref(),
        Some("5:3")
    );
    // The width is the transformer's own `len`, and every id it emits is ASCII digits, so
    // the two measures cannot come apart on any released value.
    assert_eq!(
        LinkableProjection::NumericIdWidth.apply("12345").as_deref(),
        Some("5:0")
    );
    assert_eq!(
        LinkableProjection::NumericValueWidth
            .apply("-12.50")
            .as_deref(),
        Some("-2:0.2")
    );
    assert_eq!(
        LinkableProjection::NumericValueWidth.apply("42").as_deref(),
        Some("2:0")
    );
    // The integer part's leading zeros, which `generate_numeric_component` reproduces byte
    // for byte and returns verbatim when the whole component is zero. Reading only the widths
    // put `0.8` and `6.3` in one class, while anyone holding the originals `0.5` and `4.2`
    // could tell them apart on the first character.
    assert_eq!(
        LinkableProjection::NumericValueWidth
            .apply("0.8")
            .as_deref(),
        Some("1:1.1")
    );
    assert_ne!(
        LinkableProjection::NumericValueWidth.apply("0.8"),
        LinkableProjection::NumericValueWidth.apply("6.3")
    );
    assert_eq!(
        LinkableProjection::NumericValueWidth
            .apply("0016")
            .as_deref(),
        Some("4:2")
    );
    assert_eq!(
        LinkableProjection::NameTokenCount
            .apply("Jan van der Berg")
            .as_deref(),
        Some("4")
    );
    assert_eq!(
        LinkableProjection::UuidLetterCase
            .apply("550E8400-E29B-41D4-A716-446655440000")
            .as_deref(),
        Some("upper")
    );
    assert_eq!(
        LinkableProjection::UuidLetterCase
            .apply("550e8400-e29b-41d4-a716-446655440000")
            .as_deref(),
        Some("other")
    );
}

/// The attribution against a table whose leave-one-out answers can be read off by eye.
///
/// Five rows, three pass-through columns, every row unique on the three together. Column 2
/// holds a distinct value per row and is the whole reason for that; columns 0 and 1 sort the
/// rows into a pair, a pair and a singleton. So dropping column 2 leaves only the fifth row
/// alone, while dropping either of the others leaves all five unique — column 2 separates
/// them on its own.
///
/// Written out by hand rather than derived, because an attribution checked against a figure
/// computed the same way it is would pass whatever it did.
#[test]
fn dropping_a_column_is_counted_against_a_table_worked_by_hand() {
    let columns = vec![
        column(0, DataType::String, AnonymizationStrategy::PassThrough),
        column(1, DataType::String, AnonymizationStrategy::PassThrough),
        column(2, DataType::String, AnonymizationStrategy::PassThrough),
    ];
    let rows: Vec<&[&str]> = vec![
        &["a", "x", "1"],
        &["a", "x", "2"],
        &["a", "y", "3"],
        &["a", "y", "4"],
        &["b", "z", "5"],
    ];
    let summary = record(&rows, &columns);

    assert_eq!(summary.unique_rows, 5);
    assert!(!summary.drop_attribution_incomplete);
    // Best first: column 2 leaves one unique row, the other two leave all five.
    assert_eq!(
        summary.drop_column_effects,
        vec![
            DropColumnEffect {
                column_index: 2,
                unique_rows_without: 1,
            },
            DropColumnEffect {
                column_index: 0,
                unique_rows_without: 5,
            },
            DropColumnEffect {
                column_index: 1,
                unique_rows_without: 5,
            },
        ]
    );
}

/// Removing a column can only merge classes, so no effect may exceed the baseline.
///
/// The invariant that makes the reported sentence safe to word as a reduction. A leave-one-out
/// histogram keyed on anything other than the row's own projections could break it — and
/// would break it silently, since a count larger than `unique_rows` still prints as a number.
///
/// The fixture is built so the assertion has somewhere to fail. An earlier one made every row
/// distinct, which put `unique_rows` at the row count and left the bound unreachable: each map
/// can hold at most one class per row, so `n <= n` held whatever `record_attribution` did —
/// including keying every row into its own class. Here two pairs collide, so the baseline is
/// 2 of 6 and an implementation that fails to merge anything reports 6 and is caught.
#[test]
fn no_dropped_column_raises_the_unique_count() {
    let columns = vec![
        column(0, DataType::Email, AnonymizationStrategy::Pseudonymize),
        column(1, DataType::Timestamp, AnonymizationStrategy::Pseudonymize),
        column(2, DataType::NumericId, AnonymizationStrategy::Pseudonymize),
        column(3, DataType::String, AnonymizationStrategy::PassThrough),
    ];
    // Rows 1 and 2 share every projection, as do 3 and 4; 5 and 6 differ only in the seconds
    // of their timestamp, which is exact and so survives.
    let rows: Vec<&[&str]> = vec![
        &["a@one.com", "1984-01-02T09:00:00", "001", "p"],
        &["b@one.com", "1985-03-04T09:00:00", "002", "p"],
        &["c@two.com", "1995-05-06T10:30:00", "03", "q"],
        &["d@two.com", "1996-07-08T10:30:00", "04", "q"],
        &["e@one.com", "2001-09-10T23:59:59", "5", "r"],
        &["f@one.com", "2002-11-12T23:59:58", "6", "r"],
    ];
    let summary = record(&rows, &columns);

    assert!(!summary.drop_attribution_incomplete);
    assert!(!summary.drop_column_effects.is_empty());
    // The bound is only a bound if the baseline is below the row count.
    assert_eq!(summary.rows_measured, 6);
    assert_eq!(summary.unique_rows, 2);

    for effect in &summary.drop_column_effects {
        assert!(
            effect.unique_rows_without <= summary.unique_rows,
            "dropping column {} raised the unique count from {} to {}",
            effect.column_index,
            summary.unique_rows,
            effect.unique_rows_without
        );
    }

    // And the other side of it: an implementation that merged *everything* would satisfy the
    // bound trivially, so at least one column has to be shown actually carrying the pair.
    // Dropping the timestamp merges rows 5 and 6, which nothing else separates.
    assert!(
        summary
            .drop_column_effects
            .iter()
            .any(|effect| effect.unique_rows_without < summary.unique_rows),
        "no column reduced the count on a fixture built so one does: {:?}",
        summary.drop_column_effects
    );
}

/// Every effect names a column the reader was also told was matched.
///
/// The two lists are filtered by one predicate precisely so this holds. If they drift, the
/// report pairs an effect with a column it never named, and advises dropping a column its
/// own finding does not rest on.
#[test]
fn every_effect_names_a_matched_column() {
    let columns = vec![
        column(0, DataType::String, AnonymizationStrategy::PassThrough),
        // Constant on every row, so it varies nowhere and may not be named.
        column(1, DataType::String, AnonymizationStrategy::PassThrough),
        // Redacted with no blanks, so its only projection is constant too.
        column(2, DataType::String, AnonymizationStrategy::Redact),
    ];
    let rows: Vec<&[&str]> = vec![
        &["a", "same", "x"],
        &["b", "same", "y"],
        &["c", "same", "z"],
    ];
    let summary = record(&rows, &columns);

    let matched = summary
        .matched_columns
        .iter()
        .map(|matched| matched.column_index)
        .collect::<Vec<_>>();
    assert_eq!(matched, vec![0]);
    assert_eq!(
        summary
            .drop_column_effects
            .iter()
            .map(|effect| effect.column_index)
            .collect::<Vec<_>>(),
        matched
    );
}

/// A file wider than the cap keeps its joint measure and loses only the advice — and a file
/// *at* the cap keeps both.
///
/// The two have to fail apart. Tying them together would drop the finding along with the
/// footnote about it on exactly the widest files, which are the ones a joint measure has the
/// most to say about.
///
/// Both sides of the boundary, and both written as literals. Deriving the fixture width from
/// `ATTRIBUTION_COLUMN_CAP` — which is what this test did — makes it scale with the constant
/// and prove only that cap+1 exceeds cap, a tautology that stays green if the constant is
/// changed to 1000. The literals are what pin the value; the boundary case is what pins `>`
/// against `>=`, which would otherwise silently deny the attribution to every file of exactly
/// this width.
#[test]
fn the_attribution_column_cap_is_where_it_says_it_is() {
    assert_eq!(ATTRIBUTION_COLUMN_CAP, 24);

    let measure_columns = |count: usize| {
        let columns = (0..count)
            .map(|index| column(index, DataType::String, AnonymizationStrategy::PassThrough))
            .collect::<Vec<_>>();
        let first = (0..count)
            .map(|index| index.to_string())
            .collect::<Vec<_>>();
        let second = first
            .iter()
            .map(|value| format!("{value}!"))
            .collect::<Vec<_>>();

        let mut tracker = RowUniquenessTracker::default();
        tracker.record_row(&first, &columns);
        tracker.record_row(&second, &columns);
        tracker.summary().expect("two rows were recorded")
    };

    let at_cap = measure_columns(24);
    assert!(!at_cap.measurement_incomplete);
    assert!(
        !at_cap.drop_attribution_incomplete,
        "a file of exactly the cap width still gets its attribution"
    );
    assert_eq!(at_cap.drop_column_effects.len(), 24);

    let over_cap = measure_columns(25);
    assert!(!over_cap.measurement_incomplete);
    assert_eq!(over_cap.unique_rows, 2);
    assert!(over_cap.drop_attribution_incomplete);
    assert!(over_cap.drop_column_effects.is_empty());
}

/// Tripping the joint ceiling stops the attribution as well, and says so.
///
/// Driven through `record_row` rather than by setting the flags. The test that used to cover
/// this set `linkable_stopped` and called `stop_attribution` by hand and then asserted the
/// state those two calls produce, which proves `summary` reads a flag and not that anything
/// ever sets it: deleting the `stop_attribution` call from `record_row` left it green while
/// the tracker went on growing leave-one-out maps, for the whole rest of a run, holding
/// exactly the memory the shared ceiling exists to cap for figures the summary suppresses.
#[test]
fn passing_the_joint_ceiling_stops_the_attribution_too() {
    let columns = vec![column(
        0,
        DataType::String,
        AnonymizationStrategy::PassThrough,
    )];
    let mut tracker = RowUniquenessTracker::default();
    for row in 0..=CLASS_CEILING + 1 {
        tracker.record_row(&[format!("value{row}")], &columns);
    }

    // The flag, and then the thing the flag is supposed to be a consequence of.
    assert!(tracker.attribution_stopped);
    assert!(
        tracker.attribution.is_empty(),
        "the maps have to be released, or the ceiling is a reporting rule and not a memory bound"
    );
    assert_eq!(tracker.attribution_classes, 0);

    let summary = tracker.summary().expect("rows were recorded");
    assert!(summary.measurement_incomplete);
    assert!(summary.drop_attribution_incomplete);
    assert!(summary.drop_column_effects.is_empty());
}

/// The attribution's own budget stops it, independently of the joint ceiling.
///
/// Untested until a review round pointed out that the constant could be deleted, set to
/// `usize::MAX`, or never incremented towards, with every test still green — the shared 4M
/// bound existed only as a doc comment. Its sibling `CLASS_CEILING` has had a test since
/// Phase 1.
///
/// Sixteen columns, so each row adds sixteen leave-one-out classes against the joint
/// histogram's one, and the attribution's budget is reached at a sixteenth of the rows. Two
/// columns is what this fixture had first, and at that width the two ceilings fall on the same
/// row: the joint measure stopped in the same iteration and the test proved nothing about
/// which budget had fired. That the joint measure is *still running* here is the half worth
/// asserting — the two budgets are separate, and the file keeps its finding.
#[test]
fn the_attribution_budget_stops_it_while_the_joint_measure_continues() {
    const WIDTH: usize = 16;
    let columns = (0..WIDTH)
        .map(|index| column(index, DataType::String, AnonymizationStrategy::PassThrough))
        .collect::<Vec<_>>();
    let mut tracker = RowUniquenessTracker::default();
    for row in 0..=ATTRIBUTION_CLASS_CEILING / WIDTH + 1 {
        let values = (0..WIDTH)
            .map(|index| format!("c{index}r{row}"))
            .collect::<Vec<_>>();
        tracker.record_row(&values, &columns);
    }

    assert!(tracker.attribution_stopped);
    assert!(tracker.attribution.is_empty());
    // The joint histogram is well under its own ceiling and has not stopped, so the finding
    // survives and only the advice is lost.
    assert!(!tracker.linkable_stopped);

    let summary = tracker.summary().expect("rows were recorded");
    assert!(!summary.measurement_incomplete);
    assert!(summary.unique_rows > 0);
    assert!(summary.drop_attribution_incomplete);
    assert!(summary.drop_column_effects.is_empty());
}

/// A run with no columns at all is marked as unmeasured, not as measured-and-clear.
#[test]
fn a_column_less_run_reports_no_attribution() {
    let mut tracker = RowUniquenessTracker::default();
    tracker.record_row(&[], &[]);

    let summary = tracker.summary().expect("a row was recorded");

    // `false` here would publish "we looked and no column helps" about a run with nothing to
    // look at, which is the reading the flag exists to prevent.
    assert!(summary.drop_attribution_incomplete);
    assert!(summary.drop_column_effects.is_empty());
}

/// Two columns holding the same projection must not cancel each other out.
///
/// The leave-one-out keys are sums of per-column components, and a sum forgets order. Without
/// the position hashed into each component, `["a", "b"]` and `["b", "a"]` would share a class
/// — two rows merged, which makes the file look safer, the one direction this module may not
/// be wrong in.
#[test]
fn position_is_part_of_each_component() {
    assert_ne!(component_hash(0, "a"), component_hash(1, "a"));
    let swapped = component_hash(0, "a").wrapping_add(component_hash(1, "b"));
    let original = component_hash(0, "b").wrapping_add(component_hash(1, "a"));
    assert_ne!(swapped, original);

    // Boundaries too, at the component level. `hash_fields` has its own test for this; the
    // additive path needed one of its own, since nothing else exercises `component_hash`.
    assert_ne!(
        component_hash(0, "ab").wrapping_add(component_hash(1, "c")),
        component_hash(0, "a").wrapping_add(component_hash(1, "bc"))
    );

    let columns = vec![
        column(0, DataType::String, AnonymizationStrategy::PassThrough),
        column(1, DataType::String, AnonymizationStrategy::PassThrough),
    ];
    let rows: Vec<&[&str]> = vec![&["a", "b"], &["b", "a"]];
    let summary = record(&rows, &columns);

    assert_eq!(summary.distinct_classes, 2);
    assert_eq!(summary.unique_rows, 2);
    // `distinct_classes` and `unique_rows` come out of the *sequential* histogram, which
    // `component_hash` has nothing to do with — so on their own they leave the additive path
    // untested and the file-level half of this test was decoration. These two rows have equal
    // totals if the position is dropped, so each leave-one-out map holds one class of two and
    // the effects read zero: "removing column0 would leave 0 of them unique instead of 2",
    // falsely reassuring, from a measure that had merged two distinct rows.
    assert_eq!(
        summary
            .drop_column_effects
            .iter()
            .map(|effect| effect.unique_rows_without)
            .collect::<Vec<_>>(),
        vec![2, 2]
    );
}

/// A file with no unique rows still gets its attribution, and every effect reads zero.
///
/// Not because the columns do not matter — dropping either merges the pairs further — but
/// because `unique_rows_without` counts singletons and there are none to lose. The report says
/// nothing at all on such a file: `drop_column_advice` is reached only from the arm that has
/// already established `unique_rows > 0`, so the "no single column carries it" sentence cannot
/// be produced here. This test's docstring used to claim it could.
#[test]
fn a_file_with_no_unique_rows_reports_effects_that_reduce_nothing() {
    let columns = vec![
        column(0, DataType::String, AnonymizationStrategy::PassThrough),
        column(1, DataType::String, AnonymizationStrategy::PassThrough),
    ];
    let rows: Vec<&[&str]> = vec![&["a", "x"], &["a", "x"], &["b", "y"], &["b", "y"]];
    let summary = record(&rows, &columns);

    assert_eq!(summary.unique_rows, 0);
    assert!(!summary.drop_attribution_incomplete);
    // Both columns, by index, and their values — rather than `.all()` over a list that is
    // vacuously satisfied when empty. An implementation returning `Vec::new()` unconditionally
    // passed the previous version of this test, which is the whole behaviour it names.
    assert_eq!(
        summary.drop_column_effects,
        vec![
            DropColumnEffect {
                column_index: 0,
                unique_rows_without: 0,
            },
            DropColumnEffect {
                column_index: 1,
                unique_rows_without: 0,
            },
        ]
    );
}

/// Two columns that help equally are ordered by column index, not by where they sit in the row.
///
/// The tie-break was decoration as tested: effects are built in `counted` order, `sort_by_key`
/// is stable, and every other fixture in this file gives its columns an `index` equal to its
/// position — so dropping `column_index` from the sort key changed nothing anywhere and the
/// mutation survived. `CountedColumn` keeps `position` and `column_index` apart precisely
/// because metadata is not required to be a dense ascending prefix of the row, and this is the
/// input where that distinction is observable: without the tie-break the report would advise
/// whichever of two equally-good columns happened to sit earlier in the row.
#[test]
fn equally_helpful_columns_are_ordered_by_index_not_position() {
    let mut first = column(7, DataType::String, AnonymizationStrategy::PassThrough);
    first.index = 7;
    let mut second = column(3, DataType::String, AnonymizationStrategy::PassThrough);
    second.index = 3;
    let columns = vec![first, second];

    // Both rows unique, and dropping either column leaves both rows unique — a genuine tie.
    let rows: Vec<&[&str]> = vec![&["a", "x"], &["b", "y"]];
    let summary = record(&rows, &columns);

    assert_eq!(summary.unique_rows, 2);
    assert_eq!(
        summary.drop_column_effects,
        vec![
            DropColumnEffect {
                column_index: 3,
                unique_rows_without: 2,
            },
            DropColumnEffect {
                column_index: 7,
                unique_rows_without: 2,
            },
        ],
        "a tie has to break on the column index, not on the row position"
    );
}
