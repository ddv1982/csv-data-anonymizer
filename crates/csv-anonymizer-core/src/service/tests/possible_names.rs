use super::*;

/// The clause the review warning is recognised by.
const REVIEW_CLAUSE: &str = "look like names";

fn name_column_file(
    directory: &std::path::Path,
    header: &str,
    values: &[&str],
) -> std::path::PathBuf {
    let path = directory.join(format!("{header}.csv"));
    let mut text = format!("row_id,{header}\n");
    for row in 0..60 {
        text.push_str(&format!("{row},{}\n", values[row % values.len()]));
    }
    std::fs::write(&path, text).unwrap();
    path
}

/// Columns drawing the review warning, under the app's own default selection.
///
/// `columns` and `controls` are both empty on purpose: that is the state a user is in
/// when the column table first appears, and the whole point of this warning is what it
/// says about a column the defaults did *not* pick up.
fn reviewed_columns(header: &str, values: &[&str]) -> Vec<String> {
    let temp_dir = tempfile::tempdir().unwrap();
    AnonymizerService::new("test-version")
        .preview_anonymization(PreviewParams {
            file_path: name_column_file(temp_dir.path(), header, values),
            columns: vec![],
            controls: vec![],
            sample_count: 5,
            sample_row_count: 100,
        })
        .unwrap()
        .warnings
        .iter()
        .filter(|warning| warning.message.contains(REVIEW_CLAUSE))
        .map(|warning| warning.column_name.clone())
        .collect()
}

const PEOPLE: &[&str] = &["Ada Lovelace", "Grace Hopper", "Alan Turing", "Jean Bartik"];

/// The leak this closes.
///
/// `agent_name` reads exactly like what it holds, and the taxonomy enumerates
/// `<word> name` compounds one at a time, so it was typed `String`/Low and left
/// unselected — a user accepting the app's own defaults wrote sixty people's names out
/// unchanged with nothing anywhere saying so. Detection still does not claim these are
/// people, because it cannot; it reports that it could not tell.
#[test]
fn a_plainly_named_person_column_is_surfaced_for_review() {
    for header in [
        "agent_name",
        "employee_name",
        "reviewer_name",
        "patient_name",
        "assignee_name",
    ] {
        assert_eq!(
            reviewed_columns(header, PEOPLE),
            vec![header.to_string()],
            "{header} was not surfaced for review"
        );
    }
}

/// The precision counterweight, and the reason `NON_PERSON_NAME_TOKENS` exists.
///
/// These headers match the same `name` suffix. Only the values separate them, and
/// without that separation this warning would fire on most `<word> name` columns in
/// most files and stop being read.
#[test]
fn an_organisation_name_column_is_not_surfaced() {
    for values in [
        &[
            "Acme Corporation",
            "Globex Industries",
            "Initech Limited",
            "Umbrella Group",
        ],
        &[
            "Platform Engineering",
            "Customer Success",
            "Data Science",
            "Site Reliability",
        ],
    ] {
        assert!(
            reviewed_columns("company_name", values).is_empty(),
            "an organisation column was surfaced as possible people: {values:?}"
        );
    }
}

/// A column with no name-ish header draws nothing, however its values are shaped.
#[test]
fn a_column_without_a_name_header_is_not_surfaced() {
    assert!(reviewed_columns("order_ref", PEOPLE).is_empty());
}

/// The honest limit, pinned so it cannot be mistaken for an oversight.
///
/// `New York` and `Grace Hopper` are the same shape, and the vocabulary that rejects
/// organisations deliberately holds no place words because English surnames are largely
/// toponymic — `Park`, `Hill`, `Ford` and `Brooks` all name people. So a city column
/// *is* surfaced for review. Under this design that is the correct outcome rather than a
/// false positive: the warning says detection could not tell, the column is not
/// selected, and nothing is redacted. Escalating instead would destroy a column of
/// cities by default to protect a column of people.
#[test]
fn a_place_name_column_is_surfaced_because_nothing_here_can_rule_it_out() {
    assert_eq!(
        reviewed_columns(
            "city_name",
            &["New York", "San Francisco", "Los Angeles", "Kansas City"]
        ),
        vec!["city_name".to_string()]
    );
}

/// Surfacing must not become selecting. If this ever changes, a column of city names is
/// redacted by default, which is the outcome the review tier exists to avoid.
#[test]
fn a_surfaced_column_is_still_neither_selected_nor_redacted() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = name_column_file(temp_dir.path(), "agent_name", PEOPLE);
    let column = AnonymizerService::new("test-version")
        .analyze_csv(path)
        .unwrap()
        .columns
        .remove(1);

    assert_eq!(column.pii_risk, PiiRisk::Low);
    assert!(!column.is_selected);
    assert_eq!(column.strategy, AnonymizationStrategy::Auto);
    // Not the detected type itself — four names over sixty rows land on `Enum` and a
    // more varied column would land on `String`, and neither is the point. The point is
    // that the review tier did not quietly become a *classification*: a person type here
    // would carry High risk through `classify_pii_risk` and undo all three assertions
    // above.
    assert_ne!(column.detected_type, DataType::FullName);
    assert_ne!(column.detected_type, DataType::FirstName);
    assert_ne!(column.detected_type, DataType::LastName);
}
