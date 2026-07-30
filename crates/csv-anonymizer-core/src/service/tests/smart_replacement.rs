use super::*;
use crate::smart::{SMART_REPLACEMENT_VALUE_CAP_PER_COLUMN, SmartReplacementMap};
use crate::types::{
    SmartReplacementEntry, SmartReplacementRejectionCount, SmartReplacementRejectionReason,
};

/// Mirrors `smart::SMART_REPLACEMENT_BATCH_SIZE`, which is private to that module.
/// A test that spans a chunk boundary has to know where the boundary is.
const SMART_REPLACEMENT_BATCH_SIZE_FOR_TESTS: usize = 20;

#[derive(Default)]
struct MockSmartProvider;

impl SmartReplacementProvider for MockSmartProvider {
    fn generate_replacements(
        &mut self,
        request: SmartReplacementRequest<'_>,
    ) -> Result<Vec<SmartReplacement>> {
        Ok(request
            .values
            .iter()
            .enumerate()
            .map(|(index, value)| SmartReplacement {
                original: value.clone(),
                replacement: format!("Local AI {} {}", request.column.index, index + 1),
            })
            .collect())
    }
}

struct RecordingSmartProvider {
    prefix: &'static str,
    requests: Vec<Vec<String>>,
    next_index: usize,
}

impl RecordingSmartProvider {
    fn new(prefix: &'static str) -> Self {
        Self {
            prefix,
            requests: Vec::new(),
            next_index: 0,
        }
    }
}

#[derive(Default)]
struct CrossChunkDuplicateProvider {
    next_index: usize,
}

/// Answers each value with the *next* source value in the same batch.
///
/// The failure this reproduces is a model copying one row's value into another
/// row's slot, which the per-pair checks could not see: every source still gets a
/// distinct output, so no dedup downstream has anything to object to.
#[derive(Default)]
struct SwappingSmartProvider;

impl SmartReplacementProvider for SwappingSmartProvider {
    fn generate_replacements(
        &mut self,
        request: SmartReplacementRequest<'_>,
    ) -> Result<Vec<SmartReplacement>> {
        let values = request.values;
        Ok(values
            .iter()
            .enumerate()
            .map(|(index, value)| SmartReplacement {
                original: value.clone(),
                replacement: values[(index + 1) % values.len()].clone(),
            })
            .collect())
    }
}

/// Answers a later chunk with a source value it was shown in an *earlier* chunk.
///
/// A column is asked about in batches of twenty, so this is the leak a chunk-scoped
/// check could not see: the second prompt knows nothing about the first prompt's
/// values, and the answer looks like a perfectly ordinary name.
#[derive(Default)]
struct CrossChunkLeakProvider {
    leaked_value: Option<String>,
    next_index: usize,
}

impl SmartReplacementProvider for CrossChunkLeakProvider {
    fn generate_replacements(
        &mut self,
        request: SmartReplacementRequest<'_>,
    ) -> Result<Vec<SmartReplacement>> {
        let leaked_value = self.leaked_value.clone();
        self.leaked_value
            .get_or_insert_with(|| request.values[0].clone());
        Ok(request
            .values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                self.next_index += 1;
                let replacement = match (index, leaked_value.as_ref()) {
                    (0, Some(leaked)) => leaked.clone(),
                    _ => format!("Unique Local AI {}", self.next_index),
                };
                SmartReplacement {
                    original: value.clone(),
                    replacement,
                }
            })
            .collect())
    }
}

#[derive(Default)]
struct RejectingSmartProvider;

impl SmartReplacementProvider for RejectingSmartProvider {
    fn generate_replacements(
        &mut self,
        request: SmartReplacementRequest<'_>,
    ) -> Result<Vec<SmartReplacement>> {
        Ok(request
            .values
            .iter()
            .map(|value| SmartReplacement {
                original: value.clone(),
                replacement: value.clone(),
            })
            .collect())
    }
}

impl SmartReplacementProvider for RecordingSmartProvider {
    fn generate_replacements(
        &mut self,
        request: SmartReplacementRequest<'_>,
    ) -> Result<Vec<SmartReplacement>> {
        self.requests.push(request.values.to_vec());
        let replacements = request
            .values
            .iter()
            .map(|value| {
                self.next_index += 1;
                SmartReplacement {
                    original: value.clone(),
                    replacement: format!(
                        "{} {} {}",
                        self.prefix, request.column.index, self.next_index
                    ),
                }
            })
            .collect();
        Ok(replacements)
    }
}

impl SmartReplacementProvider for CrossChunkDuplicateProvider {
    fn generate_replacements(
        &mut self,
        request: SmartReplacementRequest<'_>,
    ) -> Result<Vec<SmartReplacement>> {
        Ok(request
            .values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                self.next_index += 1;
                SmartReplacement {
                    original: value.clone(),
                    replacement: if index == 0 {
                        "Repeated Local AI Name".to_string()
                    } else {
                        format!("Unique Local AI {}", self.next_index)
                    },
                }
            })
            .collect())
    }
}

#[test]
fn preview_uses_local_ai_provider_for_smart_replacement_columns() {
    let workspace = Workspace::new();
    let input_path = workspace.write_input("smart-preview.csv", "name\nAlice Smith\nBob Stone\n");
    let mut provider = MockSmartProvider;

    let preview = workspace
        .service
        .preview_anonymization_with_smart_provider(
            PreviewParams {
                controls: vec![typed_control(
                    0,
                    DataType::FullName,
                    AnonymizationStrategy::LocalAi,
                )],
                sample_count: 2,
                ..preview_params(input_path, vec![0])
            },
            Some(&mut provider),
        )
        .unwrap();

    assert_eq!(preview.previews[0].samples.len(), 2);
    assert_eq!(preview.previews[0].samples[0].anonymized, "Local AI 0 1");
    assert!(
        preview
            .warnings
            .iter()
            .any(|warning| warning.message.contains("Local AI"))
    );
}

#[test]
fn anonymize_uses_local_ai_provider_and_reports_smart_replacements() {
    let workspace = Workspace::new();
    let input_path = workspace.path("smart.csv");
    let output_path = workspace.path("smart-output.csv");
    fs::write(&input_path, "name\nAlice Smith\nAlice Smith\nBob Stone\n").unwrap();
    let mut provider = MockSmartProvider;

    let result = workspace
        .service
        .anonymize_csv_with_sample_rows_and_control_and_smart_provider(
            AnonymizeParams {
                controls: vec![typed_control(
                    0,
                    DataType::FullName,
                    AnonymizationStrategy::LocalAi,
                )],
                ..anonymize_params(input_path, output_path.clone(), vec![0])
            },
            10,
            None,
            Some(&mut provider),
        )
        .unwrap();

    let output = read_sample(&output_path, 10).unwrap();
    assert_eq!(output.rows[0][0], output.rows[1][0]);
    assert_ne!(output.rows[0][0], output.rows[2][0]);
    assert_eq!(result.privacy_report.smart_replacement_columns, 1);
    assert_eq!(result.privacy_report.smart_replacement_values, 2);
    assert_eq!(result.privacy_report.smart_replacement_fallbacks, 0);
}

#[test]
fn anonymize_reuses_preview_smart_replacements_and_generates_missing_values() {
    let workspace = Workspace::new();
    let input_path = workspace.path("smart-preview-reuse.csv");
    let output_path = workspace.path("smart-preview-reuse-output.csv");
    fs::write(&input_path, "name\nAlice Smith\nBob Stone\nCharlie Ray\n").unwrap();
    let controls = vec![typed_control(
        0,
        DataType::FullName,
        AnonymizationStrategy::LocalAi,
    )];
    let mut preview_provider = RecordingSmartProvider::new("Preview");

    let preview = workspace
        .service
        .preview_anonymization_with_smart_provider(
            PreviewParams {
                controls: controls.clone(),
                sample_count: 1,
                ..preview_params(input_path.clone(), vec![0])
            },
            Some(&mut preview_provider),
        )
        .unwrap();
    let mut final_provider = RecordingSmartProvider::new("Final");

    workspace
        .service
        .anonymize_csv_with_sample_rows_and_control_and_smart_provider(
            AnonymizeParams {
                file_path: input_path,
                output_path: output_path.clone(),
                columns: vec![0],
                controls,
                force: false,
                preview_smart_replacements: preview.smart_replacements.clone(),
            },
            10,
            None,
            Some(&mut final_provider),
        )
        .unwrap();

    let output = read_sample(&output_path, 10).unwrap();
    assert_eq!(preview.smart_replacements.len(), 2);
    assert_eq!(preview.previews[0].samples[0].anonymized, output.rows[0][0]);
    assert_eq!(output.rows[1][0], "Preview 0 2");
    assert_eq!(output.rows[2][0], "Final 0 1");
    assert_eq!(
        final_provider.requests,
        vec![vec!["Charlie Ray".to_string()]]
    );
}

#[test]
fn anonymize_rejects_invalid_preview_smart_replacements_and_generates_missing_values() {
    let workspace = Workspace::new();
    let input_path = workspace.path("smart-invalid-preview.csv");
    let output_path = workspace.path("smart-invalid-preview-output.csv");
    fs::write(&input_path, "name\nAlice Smith\n").unwrap();
    let mut provider = RecordingSmartProvider::new("Generated");

    let result = workspace
        .service
        .anonymize_csv_with_sample_rows_and_control_and_smart_provider(
            AnonymizeParams {
                controls: vec![typed_control(
                    0,
                    DataType::FullName,
                    AnonymizationStrategy::LocalAi,
                )],
                preview_smart_replacements: vec![SmartReplacementEntry {
                    column_index: 0,
                    original: "Alice Smith".to_string(),
                    replacement: "Alice Smith".to_string(),
                }],
                ..anonymize_params(input_path, output_path.clone(), vec![0])
            },
            10,
            None,
            Some(&mut provider),
        )
        .unwrap();

    let output = read_sample(&output_path, 10).unwrap();
    assert_eq!(output.rows[0][0], "Generated 0 1");
    assert_eq!(provider.requests, vec![vec!["Alice Smith".to_string()]]);
    assert_eq!(result.privacy_report.smart_replacement_values, 1);
    assert_eq!(result.privacy_report.smart_replacement_rejections, 1);
    assert_eq!(
        result.privacy_report.smart_replacement_rejection_reasons,
        vec![SmartReplacementRejectionCount {
            reason: SmartReplacementRejectionReason::SameAsOriginal,
            count: 1,
        }]
    );
}

#[test]
fn anonymize_rejects_smart_replacements_carrying_another_rows_value() {
    let workspace = Workspace::new();
    let input_path = workspace.path("smart-cross-value.csv");
    let output_path = workspace.path("smart-cross-value-output.csv");
    fs::write(&input_path, "name\nAlice Smith\nBob Stone\n").unwrap();
    let mut provider = SwappingSmartProvider;

    let result = workspace
        .service
        .anonymize_csv_with_sample_rows_and_control_and_smart_provider(
            AnonymizeParams {
                controls: vec![typed_control(
                    0,
                    DataType::FullName,
                    AnonymizationStrategy::LocalAi,
                )],
                ..anonymize_params(input_path, output_path.clone(), vec![0])
            },
            10,
            None,
            Some(&mut provider),
        )
        .unwrap();

    let output = read_sample(&output_path, 10).unwrap();
    // Neither source value may appear anywhere in the output: not in its own row,
    // which the old per-pair check already covered, and not in the other row, which
    // is what this test exists for.
    let written = output
        .rows
        .iter()
        .map(|row| row[0].clone())
        .collect::<Vec<_>>();
    assert!(!written.iter().any(|value| value == "Alice Smith"));
    assert!(!written.iter().any(|value| value == "Bob Stone"));
    assert_eq!(result.privacy_report.smart_replacement_values, 0);
    assert_eq!(result.privacy_report.smart_replacement_fallbacks, 2);
    assert_eq!(
        result.privacy_report.smart_replacement_rejection_reasons,
        vec![SmartReplacementRejectionCount {
            reason: SmartReplacementRejectionReason::MatchesOtherOriginal,
            count: 2,
        }]
    );
}

#[test]
fn preview_supplied_smart_replacements_cannot_carry_another_rows_value() {
    let map = SmartReplacementMap::from_entries(&[
        SmartReplacementEntry {
            column_index: 0,
            original: "alice@corp.example".to_string(),
            // A real address belonging to the other row of the same column.
            replacement: "bob@corp.example".to_string(),
        },
        SmartReplacementEntry {
            column_index: 0,
            original: "bob@corp.example".to_string(),
            replacement: "carol@example.test".to_string(),
        },
    ]);

    assert_eq!(map.get(0, "alice@corp.example"), None);
    assert_eq!(map.get(0, "bob@corp.example"), Some("carol@example.test"));
    assert_eq!(
        map.rejection_reasons(),
        vec![SmartReplacementRejectionCount {
            reason: SmartReplacementRejectionReason::MatchesOtherOriginal,
            count: 1,
        }]
    );
}

#[test]
fn short_source_values_reject_only_on_an_exact_match() {
    // Two-character sources: `bo` occurs inside "Bosworth Clay" by coincidence, and
    // rejecting on that would push honest replacements onto the fallback path. An
    // exact reproduction of the same short value is still a leak and still refused.
    let coincidence = SmartReplacementMap::from_entries(&[
        SmartReplacementEntry {
            column_index: 0,
            original: "Al".to_string(),
            replacement: "Bosworth Clay".to_string(),
        },
        SmartReplacementEntry {
            column_index: 0,
            original: "Bo".to_string(),
            replacement: "Alistair Fenn".to_string(),
        },
    ]);

    assert_eq!(coincidence.get(0, "Al"), Some("Bosworth Clay"));
    assert_eq!(coincidence.get(0, "Bo"), Some("Alistair Fenn"));
    assert_eq!(coincidence.rejection_reasons(), vec![]);

    let exact = SmartReplacementMap::from_entries(&[
        SmartReplacementEntry {
            column_index: 0,
            original: "Al".to_string(),
            replacement: "Bo".to_string(),
        },
        SmartReplacementEntry {
            column_index: 0,
            original: "Bo".to_string(),
            replacement: "Alistair Fenn".to_string(),
        },
    ]);

    assert_eq!(exact.get(0, "Al"), None);
    assert_eq!(
        exact.rejection_reasons(),
        vec![SmartReplacementRejectionCount {
            reason: SmartReplacementRejectionReason::MatchesOtherOriginal,
            count: 1,
        }]
    );
}

#[test]
fn anonymize_rejects_smart_replacements_carrying_an_earlier_chunks_value() {
    let workspace = Workspace::new();
    let input_path = workspace.path("smart-cross-chunk-leak.csv");
    let output_path = workspace.path("smart-cross-chunk-leak-output.csv");
    // More values than one provider request holds, so the leak crosses a chunk
    // boundary: the value echoed back was shown to the model in the first prompt and
    // is answered into a row asked about in the second.
    let mut csv = String::from("name\n");
    for index in 0..(SMART_REPLACEMENT_BATCH_SIZE_FOR_TESTS + 1) {
        csv.push_str(&format!("Person {index:02}\n"));
    }
    fs::write(&input_path, csv).unwrap();
    let mut provider = CrossChunkLeakProvider::default();

    let result = workspace
        .service
        .anonymize_csv_with_sample_rows_and_control_and_smart_provider(
            AnonymizeParams {
                controls: vec![typed_control(
                    0,
                    DataType::FullName,
                    AnonymizationStrategy::LocalAi,
                )],
                ..anonymize_params(input_path, output_path.clone(), vec![0])
            },
            30,
            None,
            Some(&mut provider),
        )
        .unwrap();

    let output = read_sample(&output_path, 30).unwrap();
    let leaked = provider.leaked_value.clone().unwrap();
    assert!(!output.rows.iter().any(|row| row[0] == leaked));
    assert_eq!(
        result.privacy_report.smart_replacement_values,
        SMART_REPLACEMENT_BATCH_SIZE_FOR_TESTS
    );
    assert_eq!(result.privacy_report.smart_replacement_fallbacks, 1);
    assert_eq!(
        result.privacy_report.smart_replacement_rejection_reasons,
        vec![SmartReplacementRejectionCount {
            reason: SmartReplacementRejectionReason::MatchesOtherOriginal,
            count: 1,
        }]
    );
}

#[test]
fn short_source_values_reject_whole_tokens_but_not_word_prefixes() {
    // `Jan` is a real Dutch first name in the column, and the leak check now runs a
    // replacement against every source value of the column rather than against one.
    // A bare substring test would refuse the honest `Janneke Visser` for a row that
    // has nothing to do with Jan, and refusing it costs a real replacement.
    let prefix_only = SmartReplacementMap::from_entries(&[
        SmartReplacementEntry {
            column_index: 0,
            original: "Jan".to_string(),
            replacement: "Bram Kuiper".to_string(),
        },
        SmartReplacementEntry {
            column_index: 0,
            original: "Sophie de Wit".to_string(),
            replacement: "Janneke Visser".to_string(),
        },
    ]);

    assert_eq!(prefix_only.get(0, "Sophie de Wit"), Some("Janneke Visser"));
    assert_eq!(prefix_only.rejection_reasons(), vec![]);

    // The same three characters as a whole token is the leak the check exists for,
    // whether they stand alone or sit behind a hyphen.
    for leaking_replacement in ["Jan", "Anne-Jan Bakker"] {
        let leak = SmartReplacementMap::from_entries(&[
            SmartReplacementEntry {
                column_index: 0,
                original: "Jan".to_string(),
                replacement: "Bram Kuiper".to_string(),
            },
            SmartReplacementEntry {
                column_index: 0,
                original: "Sophie de Wit".to_string(),
                replacement: leaking_replacement.to_string(),
            },
        ]);

        assert_eq!(leak.get(0, "Sophie de Wit"), None);
        assert_eq!(
            leak.rejection_reasons(),
            vec![SmartReplacementRejectionCount {
                reason: SmartReplacementRejectionReason::MatchesOtherOriginal,
                count: 1,
            }]
        );
    }
}

#[test]
fn non_ascii_source_values_are_recognized_across_casing() {
    // ASCII-only case folding left `MÜLLER` and `müller` as different strings, so a
    // source value handed back in another row's slot with ordinary casing passed both
    // the equality and the containment arm and reached the output.
    let map = SmartReplacementMap::from_entries(&[
        SmartReplacementEntry {
            column_index: 0,
            original: "MÜLLER".to_string(),
            replacement: "Anna Bakker".to_string(),
        },
        SmartReplacementEntry {
            column_index: 0,
            original: "Sophie de Wit".to_string(),
            replacement: "Anna Müller".to_string(),
        },
    ]);

    assert_eq!(map.get(0, "MÜLLER"), Some("Anna Bakker"));
    assert_eq!(map.get(0, "Sophie de Wit"), None);
    assert_eq!(
        map.rejection_reasons(),
        vec![SmartReplacementRejectionCount {
            reason: SmartReplacementRejectionReason::MatchesOtherOriginal,
            count: 1,
        }]
    );
}

#[test]
fn anonymize_reports_all_rejected_smart_replacement_batches() {
    let workspace = Workspace::new();
    let input_path = workspace.path("smart-all-rejected.csv");
    let output_path = workspace.path("smart-all-rejected-output.csv");
    fs::write(&input_path, "name\nAlice Smith\nBob Stone\n").unwrap();
    let mut provider = RejectingSmartProvider;

    let result = workspace
        .service
        .anonymize_csv_with_sample_rows_and_control_and_smart_provider(
            AnonymizeParams {
                controls: vec![typed_control(
                    0,
                    DataType::FullName,
                    AnonymizationStrategy::LocalAi,
                )],
                ..anonymize_params(input_path, output_path.clone(), vec![0])
            },
            10,
            None,
            Some(&mut provider),
        )
        .unwrap();

    let output = read_sample(&output_path, 10).unwrap();
    assert_ne!(output.rows[0][0], "Alice Smith");
    assert_ne!(output.rows[1][0], "Bob Stone");
    assert_eq!(result.privacy_report.smart_replacement_values, 0);
    assert_eq!(result.privacy_report.smart_replacement_rejections, 2);
    assert_eq!(result.privacy_report.smart_replacement_fallbacks, 2);
    assert_eq!(
        result.privacy_report.smart_replacement_rejection_reasons,
        vec![SmartReplacementRejectionCount {
            reason: SmartReplacementRejectionReason::SameAsOriginal,
            count: 2,
        }]
    );
}

#[test]
fn anonymize_caps_local_ai_unique_values_and_falls_back_for_excess_values() {
    let workspace = Workspace::new();
    let input_path = workspace.path("smart-high-cardinality.csv");
    let output_path = workspace.path("smart-high-cardinality-output.csv");
    let mut csv = String::from("name\n");
    for index in 0..(SMART_REPLACEMENT_VALUE_CAP_PER_COLUMN + 2) {
        csv.push_str(&format!("Person {index}\n"));
    }
    fs::write(&input_path, csv).unwrap();
    let mut provider = RecordingSmartProvider::new("Capped");

    let result = workspace
        .service
        .anonymize_csv_with_sample_rows_and_control_and_smart_provider(
            AnonymizeParams {
                file_path: input_path,
                output_path,
                columns: vec![0],
                controls: vec![typed_control(
                    0,
                    DataType::FullName,
                    AnonymizationStrategy::LocalAi,
                )],
                force: false,
                preview_smart_replacements: vec![],
            },
            10,
            None,
            Some(&mut provider),
        )
        .unwrap();
    let requested_values = provider.requests.iter().map(Vec::len).sum::<usize>();

    assert_eq!(requested_values, SMART_REPLACEMENT_VALUE_CAP_PER_COLUMN);
    assert_eq!(
        result.privacy_report.smart_replacement_values,
        SMART_REPLACEMENT_VALUE_CAP_PER_COLUMN
    );
    assert_eq!(result.privacy_report.smart_replacement_fallbacks, 2);
}

#[test]
fn anonymize_rejects_duplicate_smart_outputs_across_provider_chunks() {
    let workspace = Workspace::new();
    let input_path = workspace.path("smart-cross-chunk-duplicates.csv");
    let output_path = workspace.path("smart-cross-chunk-duplicates-output.csv");
    let mut csv = String::from("name\n");
    for index in 0..21 {
        csv.push_str(&format!("Person {index}\n"));
    }
    fs::write(&input_path, csv).unwrap();
    let mut provider = CrossChunkDuplicateProvider::default();

    let result = workspace
        .service
        .anonymize_csv_with_sample_rows_and_control_and_smart_provider(
            AnonymizeParams {
                file_path: input_path,
                output_path,
                columns: vec![0],
                controls: vec![typed_control(
                    0,
                    DataType::FullName,
                    AnonymizationStrategy::LocalAi,
                )],
                force: false,
                preview_smart_replacements: vec![],
            },
            30,
            None,
            Some(&mut provider),
        )
        .unwrap();

    assert_eq!(result.privacy_report.smart_replacement_values, 20);
    assert_eq!(result.privacy_report.smart_replacement_rejections, 1);
    assert_eq!(result.privacy_report.smart_replacement_fallbacks, 1);
    assert_eq!(
        result.privacy_report.smart_replacement_rejection_reasons,
        vec![SmartReplacementRejectionCount {
            reason: SmartReplacementRejectionReason::DuplicateOutput,
            count: 1,
        }]
    );
}

#[test]
fn local_ai_strategy_requires_provider_before_processing() {
    let workspace = Workspace::new();
    let input_path = workspace.write_input("smart-missing-provider.csv", "name\nAlice Smith\n");

    let error = workspace
        .service
        .preview_anonymization(PreviewParams {
            controls: vec![typed_control(
                0,
                DataType::FullName,
                AnonymizationStrategy::LocalAi,
            )],
            sample_count: 1,
            ..preview_params(input_path, vec![0])
        })
        .unwrap_err();

    assert!(error.to_string().contains("Local AI"));
}

/// Whether the report explained a wholesale fallback rather than only counting it.
fn leak_guard_note(report: &crate::types::PrivacyReport) -> Option<&String> {
    report
        .notes
        .iter()
        .find(|note| note.contains("exactly matched another row's real value"))
}

/// A closed-domain column of 8 repeated values across 60 rows.
///
/// This is the shape the leak guard cannot help but reject in full: with eight
/// countries in the column, every realistic replacement for one row is another row's
/// real value. 60 rows clears `CARDINALITY_FLOOR`, so the transform ledger's exact
/// distribution reports `FewDistinctValues` and the note can name the column.
fn closed_domain_csv() -> String {
    const COUNTRIES: [&str; 8] = [
        "Netherlands",
        "Belgium",
        "Germany",
        "France",
        "Spain",
        "Italy",
        "Portugal",
        "Austria",
    ];
    let mut content = String::from("country\n");
    for row in 0..60 {
        content.push_str(COUNTRIES[row % COUNTRIES.len()]);
        content.push('\n');
    }
    content
}

/// A closed-domain column rejected wholesale is replaced, not released, and the note
/// explains it.
///
/// The regression this pins: a repeated-country column detects as `Enum`, `Enum` is a
/// `uses_default_pass_through` type, and the Local AI fallback used to fall into the
/// shared pass-through gate — so every value the leak guard refused was written out
/// verbatim. `Netherlands` appeared in the output of a run the user had asked to
/// anonymize with Smart replacement, at a near-100% rate, because a closed domain is
/// exactly what makes the guard reject everything. The source values must not appear.
#[test]
fn a_local_ai_column_rejected_wholesale_releases_no_source_value() {
    let workspace = Workspace::new();
    let input_path = workspace.path("closed-domain.csv");
    let output_path = workspace.path("closed-domain-output.csv");
    fs::write(&input_path, closed_domain_csv()).unwrap();
    let mut provider = SwappingSmartProvider;

    let result = workspace
        .service
        .anonymize_csv_with_sample_rows_and_control_and_smart_provider(
            AnonymizeParams {
                controls: vec![control(0, AnonymizationStrategy::LocalAi)],
                ..anonymize_params(input_path, output_path.clone(), vec![0])
            },
            100,
            None,
            Some(&mut provider),
        )
        .unwrap();

    assert_eq!(result.privacy_report.smart_replacement_values, 0);
    let output = fs::read_to_string(&output_path).unwrap();
    for country in [
        "Netherlands",
        "Belgium",
        "Germany",
        "France",
        "Spain",
        "Italy",
        "Portugal",
        "Austria",
    ] {
        assert!(
            !output.contains(country),
            "a value the leak guard refused was released verbatim: {country}"
        );
    }

    let note = leak_guard_note(&result.privacy_report)
        .expect("a wholesale leak-guard fallback should be explained");
    assert!(
        note.contains("small closed set of repeated values"),
        "the note should name the likely cause: {note:?}"
    );
    assert!(
        note.contains("Refused values in country") && note.contains("never written through"),
        "the note should say the refused values were replaced: {note:?}"
    );
    assert!(
        !note.contains("written out unchanged"),
        "nothing is written through any more: {note:?}"
    );

    // And the column report no longer promises a fallback it does not deliver.
    let column_report = result
        .privacy_report
        .column_reports
        .iter()
        .find(|entry| entry.column_index == 0)
        .expect("the selected column should be reported");
    assert!(
        column_report
            .detail
            .contains("fell back to rule-based replacement rather than the original value"),
        "{:?}",
        column_report.detail
    );
}

/// Auto and Pseudonymize keep passing these types through.
///
/// The Local AI fix exempts one path from the pass-through gate, and the risk of such
/// an exemption is that it widens: a user who never asked for Smart replacement must
/// not silently find their country column rewritten, because pass-through for closed
/// domains is a deliberate utility choice — replacing `NL` with another country code
/// buys no privacy and destroys the column.
#[test]
fn pass_through_types_are_still_unchanged_under_auto_and_pseudonymize() {
    for strategy in [
        AnonymizationStrategy::Auto,
        AnonymizationStrategy::Pseudonymize,
    ] {
        let workspace = Workspace::new();
        let input_path = workspace.path("closed-domain.csv");
        let output_path = workspace.path("closed-domain-output.csv");
        fs::write(&input_path, closed_domain_csv()).unwrap();

        workspace
            .service
            .anonymize_csv_with_sample_rows_and_control(
                AnonymizeParams {
                    controls: vec![control(0, strategy)],
                    ..anonymize_params(input_path, output_path.clone(), vec![0])
                },
                100,
                None,
            )
            .unwrap();

        let output = fs::read_to_string(&output_path).unwrap();
        assert!(
            output.contains("Netherlands"),
            "{strategy:?} started transforming a pass-through type"
        );
    }
}

/// The note names the column without inventing a shape for it.
///
/// Two rows is below `CARDINALITY_FLOOR`, so no distribution proves a closed set — the
/// note names the column but quotes no figures it did not measure.
#[test]
fn wholesale_leak_guard_fallback_is_explained_without_naming_unproven_columns() {
    let workspace = Workspace::new();
    let input_path = workspace.path("tiny-cross-value.csv");
    let output_path = workspace.path("tiny-cross-value-output.csv");
    fs::write(&input_path, "name\nAlice Smith\nBob Stone\n").unwrap();
    let mut provider = SwappingSmartProvider;

    let result = workspace
        .service
        .anonymize_csv_with_sample_rows_and_control_and_smart_provider(
            AnonymizeParams {
                controls: vec![typed_control(
                    0,
                    DataType::FullName,
                    AnonymizationStrategy::LocalAi,
                )],
                ..anonymize_params(input_path, output_path.clone(), vec![0])
            },
            10,
            None,
            Some(&mut provider),
        )
        .unwrap();

    let note = leak_guard_note(&result.privacy_report)
        .expect("a wholesale leak-guard fallback should be explained");
    assert!(
        !note.contains("distinct of"),
        "no distribution proves a closed set here: {note:?}"
    );
    assert!(
        note.contains("Refused values in name fell back to rule-based replacement"),
        "the note should say the fallback replaced these values: {note:?}"
    );
    assert!(
        !note.contains("written out unchanged"),
        "nothing is released here: {note:?}"
    );
}

/// Silent on a run whose replacements were accepted.
///
/// The note claims Local AI "appears to have left the column as opaque tokens",
/// which is false of a run that used the model's output, and a caveat that fires
/// when it does not apply is what trains people to skip the notes that do.
#[test]
fn accepted_local_ai_replacements_draw_no_wholesale_fallback_note() {
    let workspace = Workspace::new();
    let input_path = workspace.path("closed-domain-accepted.csv");
    let output_path = workspace.path("closed-domain-accepted-output.csv");
    fs::write(&input_path, closed_domain_csv()).unwrap();
    let mut provider = MockSmartProvider;

    let result = workspace
        .service
        .anonymize_csv_with_sample_rows_and_control_and_smart_provider(
            AnonymizeParams {
                controls: vec![control(0, AnonymizationStrategy::LocalAi)],
                ..anonymize_params(input_path, output_path.clone(), vec![0])
            },
            100,
            None,
            Some(&mut provider),
        )
        .unwrap();

    assert!(result.privacy_report.smart_replacement_values > 0);
    assert_eq!(leak_guard_note(&result.privacy_report), None);
}
