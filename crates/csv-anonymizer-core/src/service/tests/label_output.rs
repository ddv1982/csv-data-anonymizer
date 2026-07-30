use super::*;

/// A labelled column, with detection left to decide the type on its own.
///
/// `Label` short-circuits before the type-dispatched transformers, so the detected
/// type does not change what these tests observe — but a control that overrode the
/// type would hide it if that ever stopped being true, so nothing here overrides one.
fn label_control(column_index: usize) -> ColumnControl {
    control(column_index, AnonymizationStrategy::Label)
}

/// The ordinal a placeholder carries, or `None` if the cell is not one.
///
/// Only the shape is decoded here; every test that cares about *which* value a cell
/// held asserts the whole string instead.
fn placeholder_ordinal(cell: &str, label: &str) -> Option<usize> {
    cell.strip_prefix(&format!("[{label}_"))?
        .strip_suffix(']')?
        .parse()
        .ok()
}

fn anonymize_to(input_path: &Path, output_path: &Path, columns: Vec<usize>) {
    let controls = columns.iter().copied().map(label_control).collect();
    AnonymizerService::new("test-version")
        .anonymize_csv(AnonymizeParams {
            controls,
            ..anonymize_params(input_path.to_path_buf(), output_path.to_path_buf(), columns)
        })
        .unwrap();
}

/// The first end-to-end evidence that a labelled placeholder is what lands on disk.
///
/// Every existing `Label` test drives `transform_value_with_state` directly, which
/// proves the transform and nothing about the four stages between a user's choice and
/// the file: the control has to reach the metadata, the metadata has to survive column
/// selection, the streaming writer has to carry one `TransformState` across all rows,
/// and the placeholder has to escape spreadsheet-formula neutralization intact. A
/// break in any of those leaves the unit tests green and writes something other than
/// `[CUSTOMER_NOTES_1]` into the file the user hands over.
#[test]
fn a_labelled_column_reaches_the_written_file_with_one_placeholder_per_distinct_value() {
    let workspace = Workspace::new();
    let input_path = workspace.write_input(
        "notes.csv",
        "ticket,customer notes\n\
         T-1,escalated to billing\n\
         T-2,customer asked for a refund\n\
         T-3,escalated to billing\n\
         T-4,\n\
         T-5,  ESCALATED TO BILLING  \n",
    );
    let output_path = workspace.path("notes-anonymized.csv");

    // Pins the reason this column is a fair subject: no validator claims free-form
    // prose, so `String` is what a real free-text column detects as, and the header
    // is genuinely the only surviving evidence about what the cells held.
    let columns = workspace.service.analyze_csv(&input_path).unwrap().columns;
    assert_eq!(columns[1].name, "customer notes");
    assert_eq!(columns[1].detected_type, DataType::String);

    anonymize_to(&input_path, &output_path, vec![1]);
    let rows = written_rows(&output_path);

    // Headers are never transformed, which is what makes naming the column in the
    // cell disclose nothing row 1 does not already say.
    assert_eq!(rows[0], vec!["ticket", "customer notes"]);
    // The unselected column proves the run wrote the whole file, not just the
    // labelled column.
    assert_eq!(rows[1][0], "T-1");

    assert_eq!(rows[1][1], "[CUSTOMER_NOTES_1]");
    assert_eq!(rows[2][1], "[CUSTOMER_NOTES_2]");
    // The property the strategy exists for: two rows that held the same value are
    // visibly the same value in the output.
    assert_eq!(rows[3][1], "[CUSTOMER_NOTES_1]");
    // And two rows that did not are visibly different.
    assert_ne!(rows[1][1], rows[2][1]);
    // An empty cell is preserved rather than labelled: detection treated it as
    // absent, so labelling it would assert a value that was never there.
    assert_eq!(rows[4][1], "");
    // Value identity is trimmed and case-folded, so a padded, shouted duplicate is
    // the same value — the alternative is a second ordinal for one source value,
    // which reads as a distinction the data does not make.
    assert_eq!(rows[5][1], "[CUSTOMER_NOTES_1]");

    for row in rows.iter().skip(1) {
        let cell = &row[1];
        assert!(
            cell.is_empty() || placeholder_ordinal(cell, "CUSTOMER_NOTES").is_some(),
            "{cell:?} is neither empty nor a [CUSTOMER_NOTES_n] placeholder"
        );
    }
}

/// Rows of unique free text, one value per row, in a file far larger than any sample.
///
/// Unique values are what make the ordinal readable as a row number: row `n` holds the
/// `n`-th distinct value, so the run must write `[NOTES_n+1]` there and any
/// disagreement about *which rows were seen in what order* shows up as an arithmetic
/// mismatch rather than as a coincidence. 5000 rows is deliberate — see
/// [`the_preview_and_the_run_agree_on_what_each_placeholder_stands_for`].
fn unique_notes_file(workspace: &Workspace) -> PathBuf {
    let mut text = String::from("row_id,notes\n");
    for row in 0..5_000 {
        text.push_str(&format!("{row},note {row}\n"));
    }
    workspace.write_input("unique-notes.csv", &text)
}

/// The invariant no other test pins: a placeholder means the same source value in the
/// preview as it does in the file.
///
/// The two views reach it by different routes and agree only by construction. The
/// preview displays a *head* window (`csv_io::read_sample`) while detection samples a
/// *spread* over the whole file, and the run streams every row in order — so the
/// preview numbers the file's opening distinct values, and so does the run. Making the
/// preview "consistent with detection" by displaying the spread sample is an obvious
/// tidy-up and it silently breaks the promise the preview makes: the spread sample's
/// first row here is row 48, so it would be shown as `[NOTES_1]` while the file calls
/// it `[NOTES_49]`, and the user approves a mapping the run does not apply.
///
/// The file is 5000 rows so no spread sample can reproduce the head window by
/// accident. `SpreadSampler::spread` ranks rows by a fixed hash of their position, so
/// which rows a given sample size keeps is deterministic: at 5000 rows the smallest
/// position kept is 558 for a 10-row sample and 48 for a 100-row one — the two sizes
/// the preview has to hand — and neither is row 0.
#[test]
fn the_preview_and_the_run_agree_on_what_each_placeholder_stands_for() {
    let workspace = Workspace::new();
    let input_path = unique_notes_file(&workspace);
    let output_path = workspace.path("unique-notes-anonymized.csv");

    let preview = workspace
        .service
        .preview_anonymization(PreviewParams {
            controls: vec![label_control(1)],
            ..preview_params(input_path.clone(), vec![1])
        })
        .unwrap();
    anonymize_to(&input_path, &output_path, vec![1]);
    let rows = written_rows(&output_path);

    let samples = &preview.previews[0].samples;
    assert_eq!(samples.len(), 5, "{samples:?}");
    for sample in samples {
        // Which row the previewed value lives on, read out of the value itself, so
        // this holds whichever rows the preview chose to show.
        let row = sample
            .original
            .strip_prefix("note ")
            .and_then(|number| number.parse::<usize>().ok())
            .unwrap_or_else(|| panic!("unexpected previewed value {:?}", sample.original));

        assert_eq!(
            rows[row + 1][1],
            sample.anonymized,
            "the preview showed {:?} as {} but the run wrote {} for the same value: the \
             placeholder a user approved stands for a different source value in the file",
            sample.original,
            sample.anonymized,
            rows[row + 1][1],
        );
    }
}

/// The preview's half of the agreement, stated on its own so a failure says which
/// side moved.
///
/// The preview is a window on the input's *opening* rows — that is what a user reading
/// it assumes, and it is also what makes its ordinals match the run's. Both halves are
/// asserted as literal strings: the values shown are rows 0 to 4, and their
/// placeholders are the first five ordinals.
#[test]
fn the_preview_numbers_the_files_opening_rows() {
    let workspace = Workspace::new();
    let input_path = unique_notes_file(&workspace);

    let preview = workspace
        .service
        .preview_anonymization(PreviewParams {
            controls: vec![label_control(1)],
            ..preview_params(input_path, vec![1])
        })
        .unwrap();

    let shown: Vec<(String, String)> = preview.previews[0]
        .samples
        .iter()
        .map(|sample| (sample.original.clone(), sample.anonymized.clone()))
        .collect();
    let expected: Vec<(String, String)> = (0..5)
        .map(|row| (format!("note {row}"), format!("[NOTES_{}]", row + 1)))
        .collect();

    assert_eq!(shown, expected);
}

/// Two columns whose headers reduce to the same label must stay distinguishable in
/// the written file.
///
/// The two halves of duplicate-header handling meeting: detection flags the columns, the
/// strategy qualifies their labels. Composed rather than assumed, because each half is
/// inert on its own — a flag nobody reads, or a qualifier nobody sets.
///
/// The qualifier is decided in `build_column_metadata`, which is the only stage that
/// sees the whole column set, and consumed in `labelled_placeholder`, which is handed
/// one column at a time. Between them lie the control application, column selection
/// and the streaming writer, and every one of them clones or rebuilds
/// `ColumnMetadata` — so `header_label_is_ambiguous` reaching the strategy is
/// plumbing, not arithmetic, and plumbing is what a unit test that sets the flag by
/// hand cannot check.
///
/// Losing it is not cosmetic. Both columns would open at `[NOTES_1]` while holding
/// unrelated values, and a reader comparing those two cells would read an equality
/// nothing ever measured.
#[test]
fn duplicate_headers_stay_index_qualified_in_the_written_file() {
    let workspace = Workspace::new();
    // `notes` and `Notes!` are different headers that reduce to the same label:
    // case is folded away and the trailing punctuation contributes nothing.
    let input_path = workspace.write_input(
        "duplicate-headers.csv",
        "notes,Notes!\n\
         alpha,zulu\n\
         beta,zulu\n\
         alpha,yankee\n",
    );
    let output_path = workspace.path("duplicate-headers-anonymized.csv");

    anonymize_to(&input_path, &output_path, vec![0, 1]);
    let rows = written_rows(&output_path);

    assert_eq!(rows[0], vec!["notes", "Notes!"]);
    // The position qualifies the label; the ordinal still counts each column's own
    // distinct values, and repeats still keep their label.
    assert_eq!(rows[1], vec!["[NOTES_0_1]", "[NOTES_1_1]"]);
    assert_eq!(rows[2], vec!["[NOTES_0_2]", "[NOTES_1_1]"]);
    assert_eq!(rows[3], vec!["[NOTES_0_1]", "[NOTES_1_2]"]);

    for row in rows.iter().skip(1) {
        for cell in row {
            assert!(
                placeholder_ordinal(cell, "NOTES").is_none(),
                "{cell:?} is an unqualified label, so both columns number their own \
                 unrelated values from [NOTES_1]"
            );
        }
    }
}
