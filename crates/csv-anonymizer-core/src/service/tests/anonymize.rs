use super::*;
use crate::types::{MatchedPart, ReleaseEvidenceStatus, ReleaseReadinessStatus};
use std::io::Write;

#[test]
fn anonymizes_selected_columns_without_web_runtime() {
    let workspace = Workspace::new();
    let output_path = workspace.path("sample-anonymized.csv");

    let result = workspace
        .service
        .anonymize_csv(anonymize_params(
            fixture("sample.csv"),
            output_path.clone(),
            vec![1],
        ))
        .unwrap();

    assert_eq!(result.output_path, output_path);
    assert_eq!(result.row_count, 5);
    assert_eq!(result.columns_anonymized, 1);
}

/// End-to-end guard for the sampling-window leak.
///
/// Detection samples the file; the transform streams all of it. When detection
/// only looked at the file's opening rows, a column whose PII started later
/// looked benign, so `should_auto_select_column` never offered it, the user never
/// selected it, and every real value was copied verbatim into the file labelled
/// "anonymized". Detection-level coverage is not enough to prove that is fixed —
/// this checks the bytes that actually reach disk.
#[test]
fn late_starting_pii_is_offered_for_selection_and_not_written_to_the_output() {
    let workspace = Workspace::new();
    let input_path = workspace.path("late-pii.csv");
    let output_path = workspace.path("late-pii-anonymized.csv");

    // The first 100 rows — exactly one default sample window — are benign.
    let mut content = String::from("flag\n");
    for row in 0..1_000 {
        if row < 100 {
            content.push_str(if row % 2 == 0 { "true\n" } else { "false\n" });
        } else {
            content.push_str(&format!("user{row}@example.com\n"));
        }
    }
    fs::write(&input_path, &content).unwrap();

    // Select exactly the columns the app would offer to anonymize by default.
    let headers = workspace.service.analyze_csv(&input_path).unwrap();
    let auto_selected: Vec<usize> = headers
        .columns
        .iter()
        .filter(|column| crate::should_auto_select_column(column))
        .map(|column| column.index)
        .collect();
    assert_eq!(
        auto_selected,
        vec![0],
        "the email column must be offered for anonymization"
    );

    workspace
        .service
        .anonymize_csv(anonymize_params(
            input_path,
            output_path.clone(),
            auto_selected,
        ))
        .unwrap();

    let output = fs::read_to_string(&output_path).unwrap();
    let leaked = (100..1_000)
        .filter(|row| output.contains(&format!("user{row}@example.com")))
        .count();
    assert_eq!(leaked, 0, "{leaked} original addresses reached the output");
}

#[test]
fn one_rare_validated_identifier_is_anchored_in_the_detection_basis() {
    let workspace = Workspace::new();
    let input_path = workspace.path("rare-email.csv");
    let mut content = String::from("contact\n");
    for row in 0..10_000 {
        if row == 9_999 {
            content.push_str("rare.person@example.com\n");
        } else {
            content.push_str("ordinary-value\n");
        }
    }
    fs::write(&input_path, content).unwrap();

    let headers = workspace.service.analyze_csv(&input_path).unwrap();
    let column = &headers.columns[0];
    assert_eq!(column.pii_risk, crate::types::PiiRisk::High);
    assert_eq!(
        column.evidence_disposition,
        crate::types::EvidenceDisposition::DetectedSensitive
    );
    assert!(crate::should_auto_select_column(column));
}

#[test]
fn anonymize_csv_with_control_reports_progress() {
    let workspace = Workspace::new();
    let output_path = workspace.path("sample-anonymized.csv");
    let mut progress_events = Vec::new();
    let result = {
        let mut on_progress = |progress: crate::types::ProcessProgress| {
            progress_events.push(progress.rows_processed);
        };
        let mut control = ProcessControl {
            on_progress: Some(&mut on_progress),
            should_cancel: None,
        };

        workspace
            .service
            .anonymize_csv_with_control(
                anonymize_params(fixture("sample.csv"), output_path.clone(), vec![1]),
                &mut control,
            )
            .unwrap()
    };

    assert_eq!(result.row_count, 5);
    assert_eq!(progress_events, vec![1, 2, 3, 4, 5]);
}

#[test]
fn selected_sample_empty_columns_transform_later_values() {
    let workspace = Workspace::new();
    let input_path = workspace.path("sparse.csv");
    let output_path = workspace.path("sparse-anonymized.csv");
    fs::write(&input_path, "id,secret\n1,\n2,\n3,late-secret\n").unwrap();

    let result = workspace
        .service
        .anonymize_csv_with_sample_rows(
            anonymize_params(input_path, output_path.clone(), vec![1]),
            2,
        )
        .unwrap();

    let output = read_sample(&output_path, 10).unwrap();

    assert_eq!(result.row_count, 3);
    assert_eq!(output.rows[2][0], "3");
    assert_ne!(output.rows[2][1], "late-secret");
    assert!(!output.rows[2][1].is_empty());
}

#[test]
fn anonymize_preserves_numeric_shapes_in_output_file() {
    let workspace = Workspace::new();
    let input_path = workspace.path("numeric-shapes.csv");
    let output_path = workspace.path("numeric-shapes-anonymized.csv");
    fs::write(
        &input_path,
        "id,code,padded,amount,sparse\n1,7,0001,-12.50,\n2,8,0002,0.00,null\n3,9,0010,42.75,123\n",
    )
    .unwrap();

    workspace
        .service
        .anonymize_csv(AnonymizeParams {
            controls: vec![
                control(0, AnonymizationStrategy::Auto),
                control(1, AnonymizationStrategy::Auto),
                control(2, AnonymizationStrategy::Auto),
                control(3, AnonymizationStrategy::Auto),
                control(4, AnonymizationStrategy::Auto),
            ],
            ..anonymize_params(input_path, output_path.clone(), vec![0, 1, 2, 3, 4])
        })
        .unwrap();

    let output = read_sample(&output_path, 10).unwrap();

    assert_eq!(output.rows[0][0].len(), 1);
    assert!(
        output.rows[0][0]
            .chars()
            .all(|character| character.is_ascii_digit())
    );
    assert_eq!(output.rows[0][1].len(), 1);
    assert!(
        output.rows[0][1]
            .chars()
            .all(|character| character.is_ascii_digit())
    );
    assert_eq!(output.rows[0][2].len(), 4);
    assert!(output.rows[0][2].starts_with("000"));
    assert_eq!(output.rows[0][3].len(), "-12.50".len());
    assert!(output.rows[0][3].starts_with('-'));
    assert!(!output.rows[0][3].starts_with("'"));
    assert_eq!(output.rows[0][3].split_once('.').unwrap().1.len(), 2);
    assert_eq!(output.rows[0][4], "");
    assert_eq!(output.rows[1][4], "null");
    assert_eq!(output.rows[2][4].len(), 3);
    assert!(
        output.rows[2][4]
            .chars()
            .all(|character| character.is_ascii_digit())
    );
}

#[test]
fn anonymize_reuses_repeated_name_sources_in_random_mode() {
    let workspace = Workspace::new();
    let input_path = workspace.path("repeated-names.csv");
    let output_path = workspace.path("repeated-names-output.csv");
    fs::write(&input_path, "first_name\nAlice\nAlice\nBianca\n").unwrap();

    workspace
        .service
        .anonymize_csv(AnonymizeParams {
            controls: vec![typed_control(
                0,
                DataType::FirstName,
                AnonymizationStrategy::Auto,
            )],
            ..anonymize_params(input_path, output_path.clone(), vec![0])
        })
        .unwrap();

    let output = read_sample(&output_path, 10).unwrap();
    assert_eq!(output.rows[0][0], output.rows[1][0]);
    assert_ne!(output.rows[0][0], output.rows[2][0]);
}

#[test]
fn anonymize_random_mode_avoids_duplicate_names_for_distinct_sources() {
    let workspace = Workspace::new();
    let input_path = workspace.path("distinct-random-names.csv");
    let output_path = workspace.path("distinct-random-names-output.csv");
    fs::write(
        &input_path,
        "first_name\nAlice\nBianca\nCeline\nDaphne\nElise\nFreya\nGemma\nHelena\nIris\nJenna\nKeira\nLena\n",
    )
    .unwrap();

    workspace
        .service
        .anonymize_csv(AnonymizeParams {
            controls: vec![typed_control(
                0,
                DataType::FirstName,
                AnonymizationStrategy::Auto,
            )],
            ..anonymize_params(input_path, output_path.clone(), vec![0])
        })
        .unwrap();

    let output = read_sample(&output_path, 20).unwrap();
    let names = output
        .rows
        .iter()
        .map(|row| row[0].clone())
        .collect::<Vec<_>>();
    let unique_names = names.iter().collect::<std::collections::HashSet<_>>();

    assert_eq!(unique_names.len(), names.len());
}

#[test]
fn anonymize_reuses_repeated_values_in_single_output() {
    let workspace = Workspace::new();
    let input_path = workspace.path("repeated-values.csv");
    let output_path = workspace.path("repeated-values-output.csv");
    fs::write(
        &input_path,
        "first_name,last_name,email\nAlice,Smith,alice@example.com\nBianca,Jones,bianca@example.com\nAlice,Smith,alice@example.com\n",
    )
    .unwrap();

    workspace
        .service
        .anonymize_csv(AnonymizeParams {
            controls: vec![
                typed_control(0, DataType::FirstName, AnonymizationStrategy::Auto),
                typed_control(1, DataType::LastName, AnonymizationStrategy::Auto),
                typed_control(2, DataType::Email, AnonymizationStrategy::Auto),
            ],
            ..anonymize_params(input_path.clone(), output_path.clone(), vec![0, 1, 2])
        })
        .unwrap();

    let output = read_sample(&output_path, 10).unwrap();
    assert_eq!(output.rows[0][0], output.rows[2][0]);
    assert_eq!(output.rows[0][1], output.rows[2][1]);
    assert_eq!(output.rows[0][2], output.rows[2][2]);
    assert_ne!(output.rows[0][0], output.rows[1][0]);
    assert_ne!(output.rows[0][1], output.rows[1][1]);
    assert_ne!(output.rows[0][2], output.rows[1][2]);
}

#[test]
fn anonymize_applies_pass_through_control() {
    let workspace = Workspace::new();
    let input_path = workspace.path("pass-through.csv");
    let output_path = workspace.path("pass-through-output.csv");
    fs::write(&input_path, "email\nuser@example.com\n").unwrap();

    let result = workspace
        .service
        .anonymize_csv(AnonymizeParams {
            controls: vec![control(0, AnonymizationStrategy::PassThrough)],
            ..anonymize_params(input_path, output_path.clone(), vec![0])
        })
        .unwrap();

    assert_eq!(result.columns_anonymized, 0);
    let output = read_sample(&output_path, 10).unwrap();
    assert_eq!(output.rows[0][0], "user@example.com");
}

#[test]
fn anonymize_does_not_count_auto_noop_selected_columns() {
    let workspace = Workspace::new();
    let input_path = workspace.path("noop-count.csv");
    let output_path = workspace.path("noop-count-output.csv");
    fs::write(
        &input_path,
        "email,country,status\nuser@example.com,US,active\n",
    )
    .unwrap();

    let result = workspace
        .service
        .anonymize_csv(AnonymizeParams {
            controls: vec![
                typed_control(0, DataType::Email, AnonymizationStrategy::PassThrough),
                typed_control(1, DataType::CountryCode, AnonymizationStrategy::Auto),
                typed_control(2, DataType::String, AnonymizationStrategy::Mask),
            ],
            ..anonymize_params(input_path, output_path.clone(), vec![0, 1, 2])
        })
        .unwrap();

    let output = read_sample(&output_path, 10).unwrap();

    assert_eq!(result.columns_anonymized, 1);
    assert_eq!(output.rows[0][0], "user@example.com");
    assert_eq!(output.rows[0][1], "US");
    assert_ne!(output.rows[0][2], "active");
}

#[test]
fn anonymize_rejects_output_path_equal_to_input_even_with_force() {
    let workspace = Workspace::new();
    let input_path = workspace.write_input("data.csv", "email\nada@example.com\n");

    let error = workspace
        .service
        .anonymize_csv(AnonymizeParams {
            force: true,
            ..anonymize_params(input_path.clone(), input_path.clone(), vec![0])
        })
        .unwrap_err();

    assert!(
        error.to_string().contains("must differ from the input"),
        "unexpected error: {error}"
    );
    let original = fs::read_to_string(&input_path).unwrap();
    assert_eq!(original, "email\nada@example.com\n");
}

#[test]
fn anonymize_rejects_absolute_input_matching_bare_relative_output() {
    let service = AnonymizerService::new("test-version");
    let mut input_file = tempfile::NamedTempFile::new_in(std::env::current_dir().unwrap()).unwrap();
    input_file.write_all(b"email\nada@example.com\n").unwrap();
    let input_path = input_file.path().canonicalize().unwrap();
    let output_path = input_path.file_name().unwrap().into();

    let error = service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path.clone(),
            output_path,
            columns: vec![0],
            controls: vec![],
            force: true,
            preview_smart_replacements: vec![],
        })
        .unwrap_err();

    assert!(
        error.to_string().contains("must differ from the input"),
        "unexpected error: {error}"
    );
    let original = fs::read_to_string(&input_path).unwrap();
    assert_eq!(original, "email\nada@example.com\n");
}

#[test]
fn anonymize_counts_iban_evidence_as_quasi_identifier() {
    let workspace = Workspace::new();
    let input_path = workspace.path("iban.csv");
    let output_path = workspace.path("iban-output.csv");
    fs::write(
        &input_path,
        "rekening\nGB82 WEST 1234 5698 7654 32\nNL91ABNA0417164300\n",
    )
    .unwrap();

    let result = workspace
        .service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path,
            columns: vec![0],
            controls: vec![],
            force: false,
            preview_smart_replacements: vec![],
        })
        .unwrap();

    assert_eq!(result.privacy_report.direct_identifiers, 0);
    assert_eq!(result.privacy_report.quasi_identifiers, 1);
    assert_eq!(result.privacy_report.redacted_columns, 1);
}

/// Whether the privacy report told the user its types came from a sample.
fn coverage_note(report: &crate::types::PrivacyReport) -> Option<&String> {
    report
        .notes
        .iter()
        .find(|note| note.starts_with("Detection examined"))
}

#[test]
fn anonymize_reports_partial_detection_coverage_in_privacy_notes() {
    let workspace = Workspace::new();
    let input_path = workspace.path("coverage-partial.csv");
    let output_path = workspace.path("coverage-partial-output.csv");
    let mut content = String::from("id,email\n");
    for row in 1..=400 {
        content.push_str(&format!("{row},user{row}@example.com\n"));
    }
    fs::write(&input_path, content).unwrap();

    let result = workspace
        .service
        .anonymize_csv_with_sample_rows(
            anonymize_params(input_path, output_path.clone(), vec![1]),
            10,
        )
        .unwrap();

    let note = coverage_note(&result.privacy_report).expect("coverage note should be present");
    // The 100-row statistical sample is supplemented by one strict email
    // evidence row instead of sacrificing a representative spread row.
    assert!(note.contains("101 of 400 rows"), "note was {note:?}");
}

#[test]
fn anonymize_omits_coverage_note_when_every_row_was_examined() {
    let workspace = Workspace::new();
    let input_path = workspace.path("coverage-complete.csv");
    let output_path = workspace.path("coverage-complete-output.csv");
    fs::write(
        &input_path,
        "id,email\n1,ada@example.com\n2,bob@example.com\n",
    )
    .unwrap();

    let result = workspace
        .service
        .anonymize_csv_with_sample_rows(
            anonymize_params(input_path, output_path.clone(), vec![1]),
            10,
        )
        .unwrap();

    assert_eq!(coverage_note(&result.privacy_report), None);
}

/// The privacy note's consequence clause tracks what the run actually did.
///
/// "Left unselected on evidence that missed them" cannot have happened on a run that
/// selected every column, so on that run the note has to state the risk that *can*
/// have happened: a type misread from the sampled rows chose the strategy.
///
/// Paired with `preflight::preflight_states_the_mistyping_risk_when_no_column_was_left_unselected`,
/// which pins the same sentence before the run. Kept separate from it because the two
/// reach the shared wording by different routes — a privacy-report note here, a
/// preflight review item there — and either route can be broken on its own.
#[test]
fn anonymize_states_the_mistyping_risk_when_no_column_was_left_unselected() {
    let workspace = Workspace::new();
    let input_path = workspace.path("coverage-all-selected.csv");
    let output_path = workspace.path("coverage-all-selected-output.csv");
    let mut content = String::from("id,email\n");
    for row in 1..=400 {
        content.push_str(&format!("{row},user{row}@example.com\n"));
    }
    fs::write(&input_path, content).unwrap();

    let result = workspace
        .service
        .anonymize_csv_with_sample_rows(
            anonymize_params(input_path, output_path.clone(), vec![0, 1]),
            10,
        )
        .unwrap();

    let note = coverage_note(&result.privacy_report).expect("coverage note should be present");
    assert!(note.contains("101 of 400 rows"), "note was {note:?}");
    assert!(
        !note.contains("left unselected"),
        "nothing was left unselected, so this claim is false: {note:?}"
    );
    assert!(
        note.contains("strategy that wrong type implies"),
        "the residual mis-typing risk should still be stated: {note:?}"
    );
}

/// Builds the file the joint measure exists for: one identifying column handled, three
/// quasi-identifiers released untouched, and two people singled out between them.
fn write_quasi_identifier_fixture(path: &std::path::Path) {
    let groups = [
        ("1011AB", "1984-02-11", "nurse"),
        ("2033CD", "1979-07-30", "driver"),
        ("3055EF", "1991-12-02", "teacher"),
    ];
    let mut content = String::from("full_name,postal_code,birth_date,job_title\n");
    for (group, (postcode, birth_date, job)) in groups.iter().enumerate() {
        for repeat in 0..6 {
            content.push_str(&format!(
                "Person {group}{repeat},{postcode},{birth_date},{job}\n"
            ));
        }
    }
    content.push_str("Alone One,9099ZZ,1962-01-05,archivist\n");
    content.push_str("Alone Two,9088YY,1955-06-19,harbourmaster\n");
    fs::write(path, content).unwrap();
}

/// The report has to say the thing the per-column checks cannot.
///
/// Only the name column is selected, so every per-column verdict is satisfied and the
/// release would previously have read as handled. The three released columns still make
/// two rows unique, and this is the item that says so — sitting in the same evidence list
/// as the per-column verdict it qualifies.
#[test]
fn the_release_report_states_how_many_rows_the_released_columns_single_out() {
    let workspace = Workspace::new();
    let input_path = workspace.path("quasi-identifiers.csv");
    let output_path = workspace.path("quasi-identifiers-anonymized.csv");
    write_quasi_identifier_fixture(&input_path);

    let headers = workspace.service.analyze_csv(&input_path).unwrap();
    let name_index = headers
        .columns
        .iter()
        .find(|column| column.name == "full_name")
        .expect("the fixture has a full_name column")
        .index;

    let result = workspace
        .service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path,
            columns: vec![name_index],
            controls: vec![control(name_index, AnonymizationStrategy::Redact)],
            force: false,
            preview_smart_replacements: vec![],
        })
        .unwrap();

    let summary = result
        .privacy_report
        .row_uniqueness
        .as_ref()
        .expect("a CSV run measures its released rows");
    assert_eq!(summary.rows_measured, 20);
    assert_eq!(summary.unique_rows, 2);

    let item = result
        .privacy_report
        .evidence
        .iter()
        .find(|item| item.id == "row-uniqueness")
        .expect("row-uniqueness evidence");
    assert_eq!(item.status, ReleaseEvidenceStatus::Review);
    assert!(item.detail.contains("2 of 20"));
    // The consequence clause, not just the count. This arm reports individually identifiable
    // people and was pinned only by the substring "2 of 20" — a reviewer replaced "however
    // each column reads on its own" with "which is fine" and all 477 tests stayed green.
    assert!(
        item.detail.contains(
            "Anyone holding those fields for a person finds that person's row, however each \
             column reads on its own."
        ),
        "the arm that reports identifiable people must keep saying what that means, got: {}",
        item.detail
    );
    // Named columns, not indices: "unique on columns 1, 2, 3" answers the reader's
    // question only if they have the file open beside the report.
    for name in ["postal_code", "birth_date", "job_title"] {
        assert!(
            item.detail.contains(name),
            "the finding must name the columns it is about, got: {}",
            item.detail
        );
    }

    // And it has to reach the readiness list, in the same words. A finding that lives
    // only in the evidence table is one a reader skimming the summary never sees.
    assert!(
        result
            .privacy_report
            .readiness
            .review_items
            .contains(&item.detail)
    );
}

/// The measure must not claim anything before the run it measures.
///
/// Preflight happens ahead of the transform, so it has no released rows to read. An item
/// appearing here would be a claim about a file that does not exist yet.
#[test]
fn preflight_makes_no_joint_re_identifiability_claim() {
    let workspace = Workspace::new();
    let input_path = workspace.path("quasi-identifiers-preflight.csv");
    write_quasi_identifier_fixture(&input_path);

    let result = workspace
        .service
        .preflight_anonymization(preflight_params(
            input_path,
            PreflightMode::Preview,
            vec![0],
        ))
        .unwrap();

    assert!(
        !result
            .evidence
            .iter()
            .any(|item| item.id == "row-uniqueness"),
        "preflight has no released rows and must not report a measurement of them"
    );
}

/// Redacting everything leaves nothing matchable, and that must read as information
/// rather than as a clean bill of health: a column this check excludes can still be
/// revealing, and the wording is the only thing keeping the two apart.
#[test]
fn a_fully_redacted_release_reports_no_applicable_measure_rather_than_a_pass() {
    let workspace = Workspace::new();
    let input_path = workspace.path("all-redacted.csv");
    let output_path = workspace.path("all-redacted-anonymized.csv");
    write_quasi_identifier_fixture(&input_path);

    let headers = workspace.service.analyze_csv(&input_path).unwrap();
    let all: Vec<usize> = headers.columns.iter().map(|column| column.index).collect();
    let controls = all
        .iter()
        .map(|index| control(*index, AnonymizationStrategy::Redact))
        .collect();

    let result = workspace
        .service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path,
            columns: all,
            controls,
            force: false,
            preview_smart_replacements: vec![],
        })
        .unwrap();

    let item = result
        .privacy_report
        .evidence
        .iter()
        .find(|item| item.id == "row-uniqueness")
        .expect("row-uniqueness evidence");
    assert_eq!(item.status, ReleaseEvidenceStatus::Info);
    assert!(item.detail.contains("not a finding"));
    // Not promoted into the verified list, where it would be read as an assurance.
    assert!(
        !result
            .privacy_report
            .readiness
            .verified_items
            .iter()
            .any(|entry| entry.contains("matchable against outside data"))
    );
}

/// The finding must not credit a format survivor with what a released value did.
///
/// `customer_id` is pseudonymized, so only its digit count survives; `postal_code` is
/// released as it stands. Both are counted — that is the point of a joint measure — but a
/// reader told their rows were unique "on postal_code, customer_id" would reasonably remove
/// the customer id and change nothing. The wording has to distinguish them.
#[test]
fn the_finding_separates_released_values_from_surviving_formats() {
    let workspace = Workspace::new();
    let input_path = workspace.path("mixed-linkability.csv");
    let output_path = workspace.path("mixed-linkability-anonymized.csv");

    let mut content = String::from("customer_id,postal_code\n");
    for row in 0..8 {
        // Two postcodes, because a column whose projection never changes separates nobody and
        // is no longer named — correctly, but it would leave this fixture proving nothing.
        let postcode = if row % 2 == 0 { "1011AB" } else { "2033CD" };
        content.push_str(&format!("{},{postcode}\n", 1000 + row));
    }
    // Wider id, same postcode: the width is the only thing that can single this row out.
    content.push_str("123456789,1011AB\n");
    fs::write(&input_path, content).unwrap();

    let headers = workspace.service.analyze_csv(&input_path).unwrap();
    let customer_id = headers
        .columns
        .iter()
        .find(|column| column.name == "customer_id")
        .expect("the fixture has a customer_id column")
        .index;

    let result = workspace
        .service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path,
            columns: vec![customer_id],
            controls: vec![typed_control(
                customer_id,
                DataType::NumericId,
                AnonymizationStrategy::Pseudonymize,
            )],
            force: false,
            preview_smart_replacements: vec![],
        })
        .unwrap();

    let summary = result
        .privacy_report
        .row_uniqueness
        .as_ref()
        .expect("a CSV run measures its released rows");
    let matched = summary
        .matched_columns
        .iter()
        .find(|matched| matched.column_index == customer_id)
        .expect("the pseudonymized id is counted");
    assert_eq!(matched.matched_on, MatchedPart::SurvivingFormat);

    let item = result
        .privacy_report
        .evidence
        .iter()
        .find(|item| item.id == "row-uniqueness")
        .expect("row-uniqueness evidence");
    assert!(
        item.detail.contains("the surviving format of customer_id"),
        "the finding must mark a format-only contributor as one, got: {}",
        item.detail
    );
    // And the released postcode is named as itself, not lumped in with it. Semicolon, not
    // comma: with commas inside a group, a comma between groups marked only the first
    // boundary, so the second and later names of a multi-name last group read as bare
    // columns released as they stand.
    assert!(
        item.detail
            .contains("postal_code; the surviving format of customer_id"),
        "the group boundary must be unambiguous, got: {}",
        item.detail
    );
}

/// Writes `groups` × `per_group` rows over one released column, plus `strays` rows in a
/// class of their own size.
///
/// `city` is left unselected, so it is released verbatim and is the whole linkable subset.
/// That makes the class structure of the file exactly the class structure of the measure,
/// which is what lets these tests state an expected verdict rather than discover one.
fn write_grouped_city_fixture(path: &std::path::Path, groups: &[(&str, usize)]) {
    let mut content = String::from("full_name,city\n");
    let mut row = 0;
    for (city, count) in groups {
        for _ in 0..*count {
            content.push_str(&format!("Person {row},{city}\n"));
            row += 1;
        }
    }
    fs::write(path, content).unwrap();
}

fn city_fixture_uniqueness(
    input_path: &std::path::Path,
    output_path: &std::path::Path,
) -> crate::types::AnonymizeData {
    let service = AnonymizerService::new("test-version");
    let headers = service.analyze_csv(input_path).unwrap();
    let name_index = headers
        .columns
        .iter()
        .find(|column| column.name == "full_name")
        .expect("the fixture has a full_name column")
        .index;

    service
        .anonymize_csv(AnonymizeParams {
            controls: vec![control(name_index, AnonymizationStrategy::Redact)],
            ..anonymize_params(
                input_path.to_path_buf(),
                output_path.to_path_buf(),
                vec![name_index],
            )
        })
        .unwrap()
}

/// The branch that issues a green tick, which nothing covered.
///
/// Every other test here pins a review, an info or the wording of a column list, so all
/// three defects that shipped in the verified arm — a reachable `k = 2`, an inverted
/// percentile, and a claim about a column that had contributed nothing — passed a fully
/// green gate. On a privacy tool the pass is the branch that most needs a test, because it
/// is the one a reader acts on.
#[test]
fn a_comfortably_grouped_release_verifies_and_says_what_it_measured() {
    let workspace = Workspace::new();
    let input_path = workspace.path("grouped.csv");
    let output_path = workspace.path("grouped-anonymized.csv");
    // Deliberately unequal, and that is the whole design of this fixture. An earlier version
    // used four groups of six, which made `smallest_class` and `fifth_percentile_class_size`
    // both 6 — so swapping the two figures in the sentence left every test green, on the one
    // test whose job is to catch a confusion between them.
    write_grouped_city_fixture(
        &input_path,
        &[("Delfzijl", 5), ("Assen", 50), ("Emmen", 50)],
    );

    let result = city_fixture_uniqueness(&input_path, &output_path);

    let summary = result
        .privacy_report
        .row_uniqueness
        .as_ref()
        .expect("a CSV run measures its released rows");
    assert_eq!(summary.rows_measured, 105);
    assert_eq!(summary.unique_rows, 0);
    // The two figures the sentence quotes, and they must differ or the assertions below
    // cannot tell which is which.
    assert_eq!(summary.smallest_class, 5);
    assert_eq!(summary.fifth_percentile_class_size, 50);

    let item = result
        .privacy_report
        .evidence
        .iter()
        .find(|item| item.id == "row-uniqueness")
        .expect("row-uniqueness evidence");
    assert_eq!(item.status, ReleaseEvidenceStatus::Verified);
    assert!(item.detail.contains("with at least 4 other(s)"));
    // "or fewer", never "or more". The percentile is an upper bound on how small the most
    // exposed twentieth's groups are, and the verified arm used to invert it — turning the
    // one figure that qualifies the claim into a second reassurance.
    assert!(
        item.detail.contains("groups of 50 or fewer"),
        "the percentile reads as an upper bound everywhere else, got: {}",
        item.detail
    );
    // A pass still has to say what it did not look at. This was the only arm without that
    // caveat, and it is the arm where its absence matters.
    assert!(
        item.detail.contains("no others"),
        "a verified claim must bound itself to the columns measured, got: {}",
        item.detail
    );
    assert!(
        result
            .privacy_report
            .readiness
            .verified_items
            .contains(&item.detail)
    );
}

/// The best case a file can reach still does not get a green tick, and that is deliberate.
///
/// `build_readiness` used to compute its status as `if review_items.is_empty() { Verified }`,
/// which reads as a decision about the file. It is not one: the "not a formal anonymity
/// guarantee" caveat is pushed unconditionally, so the list is never empty and the Verified
/// arm was unreachable — replacing the whole conditional with the constant `Review` left all
/// 501 tests green. This test pins the stance so the constant stays honest: if anyone ever
/// makes that caveat conditional, they have to come back here and decide, out loud, whether
/// this tool may now certify a file as anonymous.
#[test]
fn the_best_case_release_still_reviews_and_never_certifies() {
    let workspace = Workspace::new();
    let input_path = workspace.path("grouped.csv");
    let output_path = workspace.path("grouped-anonymized.csv");
    // The same fixture the verified-arm test above uses, so this is the most favourable
    // measurement the suite produces: no unique rows, comfortable group sizes, and a
    // row-uniqueness item that lands in `verified_items`.
    write_grouped_city_fixture(
        &input_path,
        &[("Delfzijl", 5), ("Assen", 50), ("Emmen", 50)],
    );

    let result = city_fixture_uniqueness(&input_path, &output_path);
    let readiness = &result.privacy_report.readiness;

    assert!(
        !readiness.verified_items.is_empty(),
        "this fixture is chosen because it does verify its row-uniqueness item"
    );
    assert_eq!(readiness.status, ReleaseReadinessStatus::Review);
    assert!(
        readiness.review_items.iter().any(|item| item
            == "CSV transforms reduce exposure but are not a formal anonymity guarantee."),
        "the caveat is what makes the status a constant, got: {:?}",
        readiness.review_items
    );
}

/// Two people in one group is not anonymity, and the verified arm used to say it was.
///
/// The floor was tested against the fifth percentile only. Here the percentile is 99 and
/// the smallest group holds two, so a file with a uniquely-identifiable pair passed straight
/// to a green tick — while the review arm one line above it says in as many words that a
/// group this small is not anonymity. Two branches of one function disagreeing about the
/// same file, and the disagreement resolved in favour of the reassuring answer.
#[test]
fn a_pair_alone_in_its_group_is_reviewed_and_not_verified() {
    let workspace = Workspace::new();
    let input_path = workspace.path("lone-pair.csv");
    let output_path = workspace.path("lone-pair-anonymized.csv");
    write_grouped_city_fixture(&input_path, &[("Delfzijl", 2), ("Amsterdam", 99)]);

    let result = city_fixture_uniqueness(&input_path, &output_path);

    let summary = result
        .privacy_report
        .row_uniqueness
        .as_ref()
        .expect("a CSV run measures its released rows");
    // No row stands alone, so the singleton count cannot catch this and the floor has to.
    assert_eq!(summary.unique_rows, 0);
    assert_eq!(summary.smallest_class, 2);
    assert_eq!(summary.fifth_percentile_class_size, 99);

    let item = result
        .privacy_report
        .evidence
        .iter()
        .find(|item| item.id == "row-uniqueness")
        .expect("row-uniqueness evidence");
    assert_eq!(item.status, ReleaseEvidenceStatus::Review);
    // The whole sentence, not a fragment of it. `contains("holds 2 row(s)")` was the only
    // assertion on this arm in the repo, and it is satisfied by a sentence that drops the
    // percentile, drops the floor, inverts "or fewer" into "or more" and ends "which is fine"
    // — verified by a reviewer who wrote exactly that and watched every test pass.
    assert_eq!(
        item.detail,
        "No released row stands alone, but the smallest group on city holds 2 row(s), under the \
         floor of 5, and the most exposed 5% sit in groups of 99 or fewer. A group that small is \
         not anonymity."
    );
    assert!(
        result
            .privacy_report
            .readiness
            .review_items
            .contains(&item.detail)
    );
}

/// A partial match must not be reported as though the whole value were released.
///
/// Both columns here are pseudonymized, so what survives is a domain and a decade — and the
/// report used to name them bare and then assert that rows "share their combination of
/// birth_date, email". Every released row in this fixture is distinct on those two cells as
/// written to disk, so that sentence was false while carrying a green tick.
#[test]
fn a_partial_match_is_named_as_the_part_it_matched() {
    let workspace = Workspace::new();
    let input_path = workspace.path("partial-match.csv");
    let output_path = workspace.path("partial-match-anonymized.csv");

    // Two decades, each entered mid-year so a ±365-day shift cannot carry a date across a
    // boundary, and two domains. Both columns therefore vary — a column whose projection
    // never changes is named nowhere — and the class structure is the same on every run.
    // An earlier fixture used 1980–1987, where `1980-04-10` is day 100 of its decade: about
    // a third of runs shifted it into 1979 and split the class, so the arm this test reached
    // was decided by a dice roll.
    let mut content = String::from("birth_date,email\n");
    for row in 0..4 {
        content.push_str(&format!("1984-06-1{row},person{row}@corp.example\n"));
    }
    for row in 0..4 {
        content.push_str(&format!("1994-06-1{row},person{row}@other.example\n"));
    }
    fs::write(&input_path, content).unwrap();

    let headers = workspace.service.analyze_csv(&input_path).unwrap();
    let indices: Vec<usize> = headers.columns.iter().map(|column| column.index).collect();
    let controls = headers
        .columns
        .iter()
        .map(|column| {
            typed_control(
                column.index,
                if column.name == "birth_date" {
                    DataType::Timestamp
                } else {
                    DataType::Email
                },
                AnonymizationStrategy::Pseudonymize,
            )
        })
        .collect();

    let result = workspace
        .service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path,
            columns: indices,
            controls,
            force: false,
            preview_smart_replacements: vec![],
        })
        .unwrap();

    let item = result
        .privacy_report
        .evidence
        .iter()
        .find(|item| item.id == "row-uniqueness")
        .expect("row-uniqueness evidence");

    assert!(
        item.detail.contains("the domain of email"),
        "an email pseudonym keeps its domain and nothing else, got: {}",
        item.detail
    );
    assert!(
        item.detail.contains("the decade and time of birth_date"),
        "a shifted date is matchable at decade resolution, got: {}",
        item.detail
    );
    // Neither column may be named bare, which is what the old wording did and what would now
    // be a false claim about both.
    for bare in [
        "shares birth_date",
        "shares email",
        ", email",
        ", birth_date",
    ] {
        assert!(
            !item.detail.contains(bare),
            "no sentence may name {bare:?} as released as it stands, got: {}",
            item.detail
        );
    }
    // Four rows in each decade, so this is the below-floor arm — which quotes a group size,
    // and so takes the caveat worded for one. Deterministic on this fixture.
    let summary = result
        .privacy_report
        .row_uniqueness
        .as_ref()
        .expect("a CSV run measures its released rows");
    assert_eq!(summary.smallest_class, 4);
    assert_eq!(item.status, ReleaseEvidenceStatus::Review);
    assert!(
        item.detail
            .contains("treat this group size as an upper bound"),
        "a decade-level match must be disclosed beside the figure, got: {}",
        item.detail
    );
    assert!(
        item.detail.contains("move between runs"),
        "the shift is redrawn per value, so the figure is not stable, got: {}",
        item.detail
    );
}
