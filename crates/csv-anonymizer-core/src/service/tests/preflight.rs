use super::*;
use crate::types::{
    PreflightMode, PreflightParams, ReleaseEvidenceStatus, ReleaseReadinessStatus,
    SmartReplacementEntry,
};

#[test]
fn preflight_preview_does_not_require_output_path() {
    let workspace = Workspace::new();
    let input_path = workspace.write_input("preview.csv", "email\nada@example.com\n");

    let result = workspace
        .service
        .preflight_anonymization(preflight_params(
            input_path,
            PreflightMode::Preview,
            vec![0],
        ))
        .unwrap();

    assert!(result.readiness.blockers.is_empty());
    assert!(
        result
            .readiness
            .verified_items
            .iter()
            .any(|item| item.contains("Preview does not require an output path"))
    );
}

#[test]
fn preflight_anonymize_blocks_missing_output_path() {
    let workspace = Workspace::new();
    let input_path = workspace.write_input("missing-output.csv", "email\nada@example.com\n");

    let result = workspace
        .service
        .preflight_anonymization(preflight_params(
            input_path,
            PreflightMode::Anonymize,
            vec![0],
        ))
        .unwrap();

    assert_eq!(result.readiness.status, ReleaseReadinessStatus::Blocked);
    assert!(
        result
            .readiness
            .blockers
            .iter()
            .any(|item| item.contains("Choose an output path"))
    );
}

#[test]
fn preflight_allows_local_ai_anonymize_when_preview_replacements_cover_values() {
    let workspace = Workspace::new();
    let input_path = workspace.path("smart-covered.csv");
    let output_path = workspace.path("smart-covered-output.csv");
    fs::write(&input_path, "name\nAlice Smith\nBob Stone\n").unwrap();

    let result = workspace
        .service
        .preflight_anonymization(PreflightParams {
            output_path: Some(output_path),
            controls: vec![typed_control(
                0,
                DataType::FullName,
                AnonymizationStrategy::LocalAi,
            )],
            preview_smart_replacements: vec![
                SmartReplacementEntry {
                    column_index: 0,
                    original: "Alice Smith".to_string(),
                    replacement: "Preview Alice".to_string(),
                },
                SmartReplacementEntry {
                    column_index: 0,
                    original: "Bob Stone".to_string(),
                    replacement: "Preview Bob".to_string(),
                },
            ],
            local_ai_message: Some("Local AI is unavailable.".to_string()),
            ..preflight_params(input_path, PreflightMode::Anonymize, vec![0])
        })
        .unwrap();

    assert!(
        !result
            .readiness
            .blockers
            .iter()
            .any(|item| item.contains("Local AI"))
    );
    assert!(
        result
            .readiness
            .verified_items
            .iter()
            .any(|item| item.contains("Preview Smart replacements cover"))
    );
}

#[test]
fn preflight_blocks_local_ai_anonymize_when_preview_replacements_are_incomplete() {
    let workspace = Workspace::new();
    let input_path = workspace.path("smart-incomplete.csv");
    let output_path = workspace.path("smart-incomplete-output.csv");
    fs::write(&input_path, "name\nAlice Smith\nBob Stone\n").unwrap();

    let result = workspace
        .service
        .preflight_anonymization(PreflightParams {
            output_path: Some(output_path),
            controls: vec![typed_control(
                0,
                DataType::FullName,
                AnonymizationStrategy::LocalAi,
            )],
            preview_smart_replacements: vec![SmartReplacementEntry {
                column_index: 0,
                original: "Alice Smith".to_string(),
                replacement: "Preview Alice".to_string(),
            }],
            local_ai_message: Some("Local AI is unavailable.".to_string()),
            ..preflight_params(input_path, PreflightMode::Anonymize, vec![0])
        })
        .unwrap();

    assert_eq!(result.readiness.status, ReleaseReadinessStatus::Blocked);
    assert!(
        result
            .readiness
            .blockers
            .iter()
            .any(|item| item.contains("Local AI is unavailable"))
    );
}

#[test]
fn preflight_blocks_output_path_equal_to_input() {
    let workspace = Workspace::new();
    let input_path = workspace.write_input("data.csv", "email\nada@example.com\n");

    let result = workspace
        .service
        .preflight_anonymization(PreflightParams {
            output_path: Some(input_path.clone()),
            force: true,
            ..preflight_params(input_path, PreflightMode::Anonymize, vec![0])
        })
        .unwrap();

    assert_eq!(result.readiness.status, ReleaseReadinessStatus::Blocked);
    assert!(
        result
            .readiness
            .blockers
            .iter()
            .any(|blocker| blocker.contains("must differ from the input")),
        "blockers: {:?}",
        result.readiness.blockers
    );
}

#[test]
fn preflight_keeps_late_privacy_evidence_after_type_override() {
    let workspace = Workspace::new();
    let input_path = workspace.path("late-privacy-evidence.csv");
    fs::write(
        &input_path,
        "safe,value\n1,alpha\n2,bravo\n3,charlie\n4,delta\n5,echo\n6,late@example.com\n",
    )
    .unwrap();

    let result = workspace
        .service
        .preflight_anonymization(PreflightParams {
            controls: vec![typed_control(
                1,
                DataType::String,
                AnonymizationStrategy::Redact,
            )],
            ..preflight_params(input_path, PreflightMode::Preview, vec![0])
        })
        .unwrap();

    let detector_risk = result
        .evidence
        .iter()
        .find(|item| item.id == "detector-risk")
        .expect("detector-risk evidence");
    assert_eq!(detector_risk.status, ReleaseEvidenceStatus::Review);
    assert!(detector_risk.detail.contains("value"));
    assert!(
        result
            .readiness
            .review_items
            .iter()
            .any(|item| item.contains("value"))
    );
}

/// Rows past the detection floor, so the sample cannot cover the file.
///
/// `detection_sample_rows` floors every entry point at 100 rows regardless of what
/// the caller asks for, so a partial-coverage fixture has to exceed that floor
/// rather than merely exceed `sample_row_count`.
fn write_csv_with_row_count(path: &std::path::Path, data_rows: usize) {
    let mut content = String::from("id,notes\n");
    for row in 1..=data_rows {
        content.push_str(&format!("{row},order shipped ok\n"));
    }
    fs::write(path, content).unwrap();
}

#[test]
fn preflight_reviews_partial_detection_coverage_without_blocking() {
    let workspace = Workspace::new();
    let input_path = workspace.path("partial-coverage.csv");
    write_csv_with_row_count(&input_path, 500);

    let result = workspace
        .service
        .preflight_anonymization(preflight_params(
            input_path,
            PreflightMode::Preview,
            vec![0],
        ))
        .unwrap();

    assert!(
        result
            .readiness
            .review_items
            .iter()
            .any(|item| item.contains("Detection examined 100 of 500 rows")),
        "review items were {:?}",
        result.readiness.review_items
    );
    // Informing the decision, not refusing it: every large-file run samples.
    assert!(result.readiness.blockers.is_empty());
    assert_eq!(result.readiness.status, ReleaseReadinessStatus::Review);
    assert!(result.evidence.iter().any(
        |item| item.id == "detection-coverage" && item.status == ReleaseEvidenceStatus::Review
    ));
}

/// The reported figure is the requested sample, not the floor.
///
/// Every other coverage test asks for 10 rows, which `detection_sample_rows` raises
/// to the 100-row floor — so all of them pass against a hard-coded default, and a
/// preflight that ignored "Sample rows" entirely would ship green. 300 is above the
/// floor, so only a preflight that actually reads the request can report it.
#[test]
fn preflight_reports_a_raised_sample_size_as_the_examined_row_count() {
    let workspace = Workspace::new();
    let input_path = workspace.path("raised-sample-coverage.csv");
    write_csv_with_row_count(&input_path, 500);

    let result = workspace
        .service
        .preflight_anonymization(PreflightParams {
            sample_row_count: 300,
            ..preflight_params(input_path, PreflightMode::Preview, vec![0])
        })
        .unwrap();

    assert!(
        result
            .readiness
            .review_items
            .iter()
            .any(|item| item.contains("Detection examined 300 of 500 rows")),
        "review items were {:?}",
        result.readiness.review_items
    );
    assert!(result.evidence.iter().any(|item| {
        item.id == "detection-coverage" && item.detail.contains("300 of 500 data row(s)")
    }));
}

#[test]
fn preflight_confirms_complete_detection_coverage() {
    let workspace = Workspace::new();
    let input_path = workspace.path("complete-coverage.csv");
    write_csv_with_row_count(&input_path, 20);

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
            .readiness
            .review_items
            .iter()
            .any(|item| item.contains("Detection examined")),
        "review items were {:?}",
        result.readiness.review_items
    );
    assert!(
        result
            .readiness
            .verified_items
            .iter()
            .any(|item| item.contains("Every row was examined for detection"))
    );
    assert!(
        !result
            .evidence
            .iter()
            .any(|item| item.id == "detection-coverage")
    );
}

/// The consequence the review item asserts has to be one that can actually happen.
///
/// With every column selected, "stays unselected" describes nothing: no column
/// was left out, so the sentence was simply false and the item was making a claim
/// the run contradicts. The disclosure is not suppressed — a type misread from the
/// examined rows still picks the strategy about to be applied — it just has to say
/// that instead.
///
/// Paired with `anonymize::anonymize_states_the_mistyping_risk_when_no_column_was_left_unselected`,
/// which pins the same sentence on the run's own report. One wording, two call paths:
/// this one reaches it through preflight's review items, and only this one can catch a
/// preflight that stops routing the disclosure into them.
#[test]
fn preflight_states_the_mistyping_risk_when_no_column_was_left_unselected() {
    let workspace = Workspace::new();
    let input_path = workspace.path("all-selected-coverage.csv");
    write_csv_with_row_count(&input_path, 500);

    let result = workspace
        .service
        .preflight_anonymization(preflight_params(
            input_path,
            PreflightMode::Preview,
            vec![0, 1],
        ))
        .unwrap();

    let item = result
        .readiness
        .review_items
        .iter()
        .find(|item| item.starts_with("Detection examined"))
        .expect("partial coverage should still be disclosed");
    assert!(item.contains("100 of 500 rows"), "item was {item:?}");
    assert!(
        !item.contains("stays unselected"),
        "nothing was left unselected, so this claim is false: {item:?}"
    );
    assert!(
        item.contains("strategy that wrong type implies"),
        "the residual mis-typing risk should still be stated: {item:?}"
    );
}
