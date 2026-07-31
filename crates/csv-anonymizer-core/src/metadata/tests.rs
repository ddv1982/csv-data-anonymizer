use super::*;
use crate::detection::{
    Candidate, CandidateBatch, CandidateBatchResult, CandidateDetectionCoverage, CandidateDetector,
    CandidateDetectorRunStatus, CandidateKind, CandidateRejectionReason,
};
use crate::types::DataType;

#[test]
fn builds_metadata_for_all_columns() {
    let headers = vec!["email".to_string(), "id".to_string(), "country".to_string()];
    let samples = vec![
        vec![
            "john@example.com".to_string(),
            "1001".to_string(),
            "US".to_string(),
        ],
        vec![
            "jane@test.org".to_string(),
            "1002".to_string(),
            "GB".to_string(),
        ],
    ];

    let metadata = build_column_metadata(&headers, &samples);

    assert_eq!(metadata.len(), 3);
    assert_eq!(metadata[0].detected_type, DataType::Email);
    assert_eq!(metadata[1].detected_type, DataType::NumericId);
    assert_eq!(metadata[2].detected_type, DataType::CountryCode);
}

#[test]
fn uuid_shape_is_a_persistent_record_identifier_unless_device_context_supports_more() {
    let headers = vec![
        "custom_reference".to_string(),
        "location_id".to_string(),
        "device_id".to_string(),
    ];
    let samples = (0..3)
        .map(|row| {
            vec![
                format!("00000000-0000-4000-8000-{row:012}"),
                format!("10000000-0000-4000-8000-{row:012}"),
                format!("20000000-0000-4000-8000-{row:012}"),
            ]
        })
        .collect::<Vec<_>>();

    let metadata = build_column_metadata(&headers, &samples);

    for column in &metadata[..2] {
        assert_eq!(column.detected_type, DataType::Uuid);
        assert_eq!(column.pii_risk, PiiRisk::Medium);
        assert_eq!(column.strategy, AnonymizationStrategy::Redact);
        assert!(column.privacy_evidence.iter().any(|evidence| {
            evidence.kind == crate::types::PrivacyFindingKind::RecordIdentifier
        }));
        assert!(column.privacy_evidence.iter().all(|evidence| {
            evidence.kind != crate::types::PrivacyFindingKind::NetworkOrDeviceId
        }));
        assert_eq!(
            column.evidence_profile.semantic_decision.kind,
            "recordIdentifier"
        );
        assert_eq!(
            column.evidence_profile.redaction_decision.placeholder,
            format!("[{}]", column.name.to_uppercase())
        );
        assert!(!column.evidence_profile.redaction_decision.is_typed);
        assert!(
            !column
                .evidence_profile
                .redaction_decision
                .preserves_equality
        );
    }
    assert!(
        metadata[2].privacy_evidence.iter().any(|evidence| {
            evidence.kind == crate::types::PrivacyFindingKind::NetworkOrDeviceId
        })
    );
    assert_eq!(
        metadata[2].evidence_profile.redaction_decision.placeholder,
        "[NETWORK_ID]"
    );
    assert!(metadata[2].evidence_profile.redaction_decision.is_typed);
}

#[test]
fn detects_name_types_from_header_context() {
    let headers = vec![
        "first_name".to_string(),
        "last_name".to_string(),
        "full_name".to_string(),
        "name".to_string(),
    ];
    let samples = vec![
        vec![
            "Alice".to_string(),
            "Smith".to_string(),
            "Alice Smith".to_string(),
            "Alice".to_string(),
        ],
        vec![
            "Bob".to_string(),
            "Jones".to_string(),
            "Bob Jones".to_string(),
            "Bob".to_string(),
        ],
        vec![
            "Carol".to_string(),
            "O'Neil".to_string(),
            "Carol O'Neil".to_string(),
            "Carol".to_string(),
        ],
    ];

    let metadata = build_column_metadata(&headers, &samples);

    assert_eq!(metadata[0].detected_type, DataType::FirstName);
    assert_eq!(metadata[1].detected_type, DataType::LastName);
    assert_eq!(metadata[2].detected_type, DataType::FullName);
    assert_eq!(metadata[3].detected_type, DataType::FirstName);
}

#[test]
fn does_not_detect_names_without_header_context() {
    let headers = vec!["status".to_string()];
    let samples = vec![
        vec!["Alice".to_string()],
        vec!["Bob".to_string()],
        vec!["Carol".to_string()],
    ];

    let metadata = build_column_metadata(&headers, &samples);

    assert_eq!(metadata[0].detected_type, DataType::String);
}

#[test]
fn applies_column_selection_without_mutating_source() {
    let metadata = vec![ColumnMetadata {
        pii_risk: PiiRisk::High,
        ..crate::test_support::column(0, "email", DataType::Email, AnonymizationStrategy::Auto)
    }];

    let selected = apply_column_selection(&metadata, &[0]);

    assert!(selected[0].is_selected);
    assert!(!metadata[0].is_selected);
}

#[test]
fn auto_selection_tracks_current_pii_risk_contract() {
    let headers = vec![
        "email".to_string(),
        "id".to_string(),
        "country".to_string(),
        "status".to_string(),
    ];
    let samples = vec![
        vec![
            "john@example.com".to_string(),
            "1001".to_string(),
            "US".to_string(),
            "active".to_string(),
        ],
        vec![
            "jane@example.com".to_string(),
            "1002".to_string(),
            "GB".to_string(),
            "inactive".to_string(),
        ],
        vec![
            "jo@example.com".to_string(),
            "1003".to_string(),
            "DE".to_string(),
            "pending".to_string(),
        ],
    ];

    let metadata = build_column_metadata(&headers, &samples);
    let metadata = auto_select_pii_columns(&metadata);

    assert!(metadata[0].is_selected);
    assert!(metadata[1].is_selected);
    assert!(!metadata[2].is_selected);
    assert!(!metadata[3].is_selected);
}

#[test]
fn should_auto_select_requires_samples_and_detected_risk() {
    let high_risk = column_metadata(PiiRisk::High, vec!["person@example.com".to_string()]);
    let medium_risk = column_metadata(PiiRisk::Medium, vec!["10001".to_string()]);
    let low_risk = column_metadata(PiiRisk::Low, vec!["active".to_string()]);
    let empty_high_risk = column_metadata(PiiRisk::High, vec![]);

    assert!(should_auto_select_column(&high_risk));
    assert!(should_auto_select_column(&medium_risk));
    assert!(!should_auto_select_column(&low_risk));
    assert!(!should_auto_select_column(&empty_high_risk));
}

#[test]
fn default_strategy_redacts_medium_and_high_risk_columns() {
    let headers = vec![
        "email".to_string(),
        "date_of_birth".to_string(),
        "country".to_string(),
        "status".to_string(),
    ];
    let samples = vec![
        vec![
            "john@example.com".to_string(),
            "1980-01-02".to_string(),
            "US".to_string(),
            "active".to_string(),
        ],
        vec![
            "jane@example.com".to_string(),
            "1991-03-04".to_string(),
            "GB".to_string(),
            "inactive".to_string(),
        ],
        vec![
            "jo@example.com".to_string(),
            "1975-05-06".to_string(),
            "DE".to_string(),
            "pending".to_string(),
        ],
    ];

    let metadata = build_column_metadata(&headers, &samples);

    assert_eq!(metadata[0].pii_risk, PiiRisk::High);
    assert_eq!(metadata[0].strategy, AnonymizationStrategy::Redact);
    assert_eq!(metadata[1].pii_risk, PiiRisk::Medium);
    assert_eq!(metadata[1].strategy, AnonymizationStrategy::Redact);
    assert_eq!(metadata[2].pii_risk, PiiRisk::Low);
    assert_eq!(metadata[2].strategy, AnonymizationStrategy::Auto);
    assert_eq!(metadata[3].pii_risk, PiiRisk::Low);
    assert_eq!(metadata[3].strategy, AnonymizationStrategy::Auto);
}

/// An unselected generic-text column, varied on the two things auto-selection reads.
///
/// `DataType::String` rather than the `Default` of `Unknown` because these tests ask
/// whether a column is auto-selected on its *risk and values* alone, and a type that
/// carries a finding of its own would answer for them.
fn column_metadata(pii_risk: PiiRisk, sample_values: Vec<String>) -> ColumnMetadata {
    ColumnMetadata {
        pii_risk,
        sample_values,
        ..crate::test_support::column(0, "field", DataType::String, AnonymizationStrategy::Auto)
    }
}

#[test]
fn auto_selection_includes_sensitive_new_types_only() {
    let headers = vec![
        "ip".to_string(),
        "tax_id".to_string(),
        "zip".to_string(),
        "street_address".to_string(),
        "website".to_string(),
        "mac".to_string(),
        "active".to_string(),
        "price".to_string(),
        "discount".to_string(),
    ];
    let samples = vec![
        vec![
            "192.168.1.1".to_string(),
            "123-45-6789".to_string(),
            "94105".to_string(),
            "123 Main St".to_string(),
            "https://example.com".to_string(),
            "00:1A:2B:3C:4D:5E".to_string(),
            "true".to_string(),
            "$1200.00".to_string(),
            "10%".to_string(),
        ],
        vec![
            "10.0.0.2".to_string(),
            "987-65-4321".to_string(),
            "10001".to_string(),
            "44 Market Road".to_string(),
            "www.example.org".to_string(),
            "00-1A-2B-3C-4D-5F".to_string(),
            "false".to_string(),
            "$999.99".to_string(),
            "25%".to_string(),
        ],
    ];

    let metadata = auto_select_pii_columns(&build_column_metadata(&headers, &samples));

    assert_eq!(metadata[0].detected_type, DataType::IpAddress);
    assert_eq!(metadata[1].detected_type, DataType::TaxId);
    assert_eq!(metadata[2].detected_type, DataType::PostalCode);
    assert_eq!(metadata[3].detected_type, DataType::Address);
    assert_eq!(metadata[4].detected_type, DataType::Url);
    assert_eq!(metadata[5].detected_type, DataType::MacAddress);
    assert_eq!(metadata[6].detected_type, DataType::Boolean);
    assert_eq!(metadata[7].detected_type, DataType::Currency);
    assert_eq!(metadata[8].detected_type, DataType::Percentage);
    for column in metadata.iter().take(6) {
        assert!(column.is_selected);
    }
    for column in metadata.iter().take(9).skip(6) {
        assert!(!column.is_selected);
    }
}

#[test]
fn metadata_lifts_embedded_span_findings_into_column_evidence() {
    let headers = vec!["notes".to_string()];
    let samples = vec![
        vec!["contact ada@example.com".to_string()],
        vec!["contact grace@example.com".to_string()],
        vec!["contact alan@example.com".to_string()],
    ];

    let metadata = build_column_metadata(&headers, &samples);
    let column = &metadata[0];

    assert_eq!(column.detected_type, DataType::String);
    assert_eq!(column.pii_risk, PiiRisk::High);
    assert_eq!(column.strategy, AnonymizationStrategy::Redact);
    assert_eq!(column.privacy_evidence[0].match_count, 3);
    assert_eq!(column.privacy_findings[0].start, "contact ".len());
}

#[test]
fn metadata_adds_header_evidence_for_private_dates_and_secrets() {
    let headers = vec!["date_of_birth".to_string(), "api_token".to_string()];
    let samples = vec![
        vec!["1990-01-01".to_string(), "abc123secret".to_string()],
        vec!["1982-06-29".to_string(), "def456secret".to_string()],
    ];

    let metadata = build_column_metadata(&headers, &samples);

    assert!(
        metadata[0]
            .privacy_evidence
            .iter()
            .any(
                |summary| summary.kind == crate::types::PrivacyFindingKind::PrivateDate
                    && summary.confidence == crate::types::Confidence::Medium
            )
    );
    assert!(
        metadata[1]
            .privacy_evidence
            .iter()
            .any(
                |summary| summary.kind == crate::types::PrivacyFindingKind::CredentialOrSecret
                    && summary.match_count == 2
            )
    );
}

#[test]
fn metadata_auto_selects_multilingual_pii_columns() {
    let headers = vec![
        "voornaam".to_string(),
        "achternaam".to_string(),
        "teléfono".to_string(),
        "adresse".to_string(),
        "geboortedatum".to_string(),
        "status".to_string(),
    ];
    let samples = vec![
        vec![
            "Renée".to_string(),
            "Jansen".to_string(),
            "+34 612 345 678".to_string(),
            "12 Rue de Rivoli".to_string(),
            "1980-01-02".to_string(),
            "active".to_string(),
        ],
        vec![
            "Søren".to_string(),
            "Müller".to_string(),
            "+34 611 111 111".to_string(),
            "5 Avenue Victor Hugo".to_string(),
            "1991-03-04".to_string(),
            "inactive".to_string(),
        ],
    ];

    let metadata = auto_select_pii_columns(&build_column_metadata(&headers, &samples));

    assert_eq!(metadata[0].detected_type, DataType::FirstName);
    assert_eq!(metadata[1].detected_type, DataType::LastName);
    assert_eq!(metadata[2].detected_type, DataType::Phone);
    assert_eq!(metadata[3].detected_type, DataType::Address);
    assert_eq!(metadata[4].detected_type, DataType::Timestamp);
    assert_eq!(metadata[5].detected_type, DataType::String);

    for column in metadata.iter().take(5) {
        assert!(
            column.is_selected,
            "column {} should be selected",
            column.name
        );
    }
    assert!(!metadata[5].is_selected);

    assert!(
        metadata[2]
            .detection_trace
            .as_ref()
            .is_some_and(|trace| trace.selected_reason.contains("Header taxonomy term"))
    );
    assert!(metadata[4].privacy_evidence.iter().any(|evidence| {
        evidence
            .detectors
            .contains(&"header:taxonomy:private-date".to_string())
    }));
}

#[test]
fn metadata_uses_iban_validator_without_english_header_context() {
    let headers = vec!["rekening".to_string()];
    let samples = vec![
        vec!["GB82 WEST 1234 5698 7654 32".to_string()],
        vec!["NL91ABNA0417164300".to_string()],
    ];

    let metadata = auto_select_pii_columns(&build_column_metadata(&headers, &samples));
    let column = &metadata[0];

    assert_eq!(column.detected_type, DataType::String);
    assert_eq!(column.pii_risk, PiiRisk::High);
    assert_eq!(column.strategy, AnonymizationStrategy::Redact);
    assert!(column.is_selected);
    assert!(column.privacy_evidence.iter().any(|evidence| {
        evidence
            .reason
            .contains("IBAN account identifier passed checksum validation")
    }));
}

#[test]
fn metadata_promotes_headerless_vat_values_to_tax_id() {
    let headers = vec!["business_number".to_string()];
    let samples = vec![
        vec!["NL000099998B57".to_string()],
        vec!["DE111111125".to_string()],
        vec!["FR61954506077".to_string()],
    ];

    let metadata = auto_select_pii_columns(&build_column_metadata(&headers, &samples));
    let column = &metadata[0];

    assert_eq!(column.detected_type, DataType::TaxId);
    assert_eq!(column.pii_risk, PiiRisk::High);
    assert_eq!(column.strategy, AnonymizationStrategy::Redact);
    assert!(column.is_selected);
    assert!(
        column
            .privacy_evidence
            .iter()
            .any(|evidence| evidence.detectors.contains(&"validator:vat".to_string()))
    );
}

#[test]
fn locale_context_flows_from_iban_column_to_detection() {
    // One IBAN column establishes NL context; this test only asserts the
    // plumbing compiles end-to-end and detection still classifies the IBAN
    // column. Behavioral use of the context lands in later tasks.
    let headers = vec!["iban".to_string(), "note".to_string()];
    let rows: Vec<Vec<String>> = (0..12)
        .map(|_| vec!["NL91ABNA0417164300".to_string(), "hello".to_string()])
        .collect();
    let metadata = build_column_metadata(&headers, &rows);
    assert_eq!(metadata.len(), 2);
}

#[test]
fn low_confidence_date_evidence_does_not_auto_select_column() {
    let headers = vec!["event_notes".to_string()];
    let samples = vec![vec!["created 2026-06-29".to_string()]];

    let metadata = auto_select_pii_columns(&build_column_metadata(&headers, &samples));
    let column = &metadata[0];

    assert_eq!(column.detected_type, DataType::String);
    assert_eq!(column.pii_risk, PiiRisk::Low);
    assert!(!column.is_selected);
    assert!(column.privacy_evidence.iter().any(|summary| summary.kind
        == crate::types::PrivacyFindingKind::PrivateDate
        && summary.confidence == crate::types::Confidence::Low));
}

#[test]
fn dutch_postcodes_detected_via_iban_locale_context() {
    let headers = vec!["c1".to_string(), "c2".to_string()];
    let rows: Vec<Vec<String>> = [
        ("NL91ABNA0417164300", "1012 AB"),
        ("NL02RABO0123456789", "2511 CV"),
        ("NL91ABNA0417164300", "3011 ED"),
        ("NL02RABO0123456789", "9711 LM"),
        ("NL91ABNA0417164300", "5611 EM"),
        ("NL02RABO0123456789", "6511 KL"),
        ("NL91ABNA0417164300", "7511 JE"),
        ("NL02RABO0123456789", "8011 NW"),
        ("NL91ABNA0417164300", "4811 DJ"),
        ("NL02RABO0123456789", "1071 XX"),
        ("NL91ABNA0417164300", "2312 EZ"),
        ("NL02RABO0123456789", "3512 JE"),
    ]
    .iter()
    .map(|(a, b)| vec![a.to_string(), b.to_string()])
    .collect();
    let metadata = build_column_metadata(&headers, &rows);
    assert_eq!(metadata[1].detected_type, DataType::PostalCode);
}

/// Only the columns that actually share a label are flagged. Detection stage, not
/// strategy stage, because this is the one place the whole column set is visible.
#[test]
fn duplicated_headers_are_flagged_and_unique_ones_are_left_alone() {
    let headers = vec![
        "notes".to_string(),
        "email".to_string(),
        "notes".to_string(),
    ];
    let samples = vec![vec![
        "alpha".to_string(),
        "john@example.com".to_string(),
        "zulu".to_string(),
    ]];

    let metadata = build_column_metadata(&headers, &samples);

    assert!(metadata[0].header_label_is_ambiguous);
    assert!(!metadata[1].header_label_is_ambiguous);
    assert!(metadata[2].header_label_is_ambiguous);
}

/// The comparison is on the label, not the raw header, because that is where the
/// collision happens: casing and punctuation are dropped on the way to a label, so
/// headers that differ only in those still collide.
#[test]
fn headers_differing_only_in_case_or_punctuation_are_ambiguous() {
    let headers = vec![
        "Notes".to_string(),
        "notes!".to_string(),
        "customer notes".to_string(),
    ];
    let samples = vec![vec![
        "alpha".to_string(),
        "beta".to_string(),
        "gamma".to_string(),
    ]];

    let metadata = build_column_metadata(&headers, &samples);

    assert!(metadata[0].header_label_is_ambiguous);
    assert!(metadata[1].header_label_is_ambiguous);
    // A longer header is a different label, not a collision.
    assert!(!metadata[2].header_label_is_ambiguous);
}

/// Unnamed columns already fall back to their position, so their labels differ
/// without any qualifier and flagging them would add a redundant one.
#[test]
fn unnamed_columns_are_not_ambiguous_because_their_labels_already_differ() {
    let headers = vec![String::new(), "  ".to_string(), "---".to_string()];
    let samples = vec![vec![
        "alpha".to_string(),
        "beta".to_string(),
        "gamma".to_string(),
    ]];

    let metadata = build_column_metadata(&headers, &samples);

    for column in &metadata {
        assert!(
            !column.header_label_is_ambiguous,
            "column {} was flagged",
            column.index
        );
    }
}

struct FakeCandidateDetector {
    result: std::result::Result<CandidateBatchResult, String>,
}

impl CandidateDetector for FakeCandidateDetector {
    fn detector_id(&self) -> &str {
        "fake"
    }

    fn detect(
        &mut self,
        _batch: &CandidateBatch<'_>,
    ) -> std::result::Result<CandidateBatchResult, String> {
        self.result.clone()
    }
}

fn candidate_result(candidates: Vec<Candidate>) -> CandidateBatchResult {
    CandidateBatchResult {
        model_version: Some("test-1".to_string()),
        coverage: CandidateDetectionCoverage::complete(1),
        candidates,
    }
}

#[test]
fn disabled_candidate_detection_preserves_the_existing_metadata() {
    let headers = vec!["misc".to_string()];
    let samples = vec![vec!["Ada Lovelace".to_string()]];

    let expected = build_column_metadata(&headers, &samples);
    let (actual, status) = build_column_metadata_with_candidate_detector(&headers, &samples, None);

    assert_eq!(actual, expected);
    assert_eq!(status, CandidateDetectorRunStatus::Disabled);
}

#[test]
fn detector_failure_preserves_deterministic_metadata() {
    let headers = vec!["email".to_string()];
    let samples = vec![vec!["ada@example.com".to_string()]];
    let expected = build_column_metadata(&headers, &samples);
    let mut detector = FakeCandidateDetector {
        result: Err("model unavailable".to_string()),
    };

    let (actual, status) =
        build_column_metadata_with_candidate_detector(&headers, &samples, Some(&mut detector));

    assert_eq!(actual, expected);
    assert!(matches!(
        status,
        CandidateDetectorRunStatus::Failed { message, .. } if message == "model unavailable"
    ));
}

#[test]
fn valid_candidate_adds_privacy_evidence_without_retyping_the_column() {
    let headers = vec!["misc".to_string()];
    let samples = vec![vec!["Ada Lovelace".to_string()]];
    let mut detector = FakeCandidateDetector {
        result: Ok(candidate_result(vec![Candidate {
            column_index: 0,
            row_index: 0,
            start_byte: 0,
            end_byte: "Ada Lovelace".len(),
            kind: CandidateKind::PersonName,
            score_basis_points: 9_000,
        }])),
    };

    let (metadata, status) =
        build_column_metadata_with_candidate_detector(&headers, &samples, Some(&mut detector));

    assert_eq!(metadata[0].detected_type, DataType::String);
    assert_eq!(metadata[0].pii_risk, PiiRisk::Low);
    assert!(!metadata[0].is_selected);
    assert_eq!(metadata[0].strategy, AnonymizationStrategy::Auto);
    assert_eq!(
        metadata[0].review_reasons,
        [ColumnReviewReason::AmbiguousContext]
    );
    assert!(
        metadata[0]
            .privacy_evidence
            .iter()
            .any(|evidence| evidence.detectors == ["local-ner:fake"])
    );
    assert!(matches!(
        status,
        CandidateDetectorRunStatus::Completed {
            accepted_candidates: 1,
            ..
        }
    ));
}

#[test]
fn partial_detector_coverage_is_reported_as_incomplete() {
    let headers = vec!["misc".to_string()];
    let samples = vec![vec!["Ada Lovelace".to_string()]];
    let mut detector = FakeCandidateDetector {
        result: Ok(CandidateBatchResult {
            model_version: Some("test-1".to_string()),
            coverage: CandidateDetectionCoverage {
                total_cells: 10,
                examined_cells: 1,
                skipped_oversized_cells: 2,
            },
            candidates: Vec::new(),
        }),
    };

    let (_, status) =
        build_column_metadata_with_candidate_detector(&headers, &samples, Some(&mut detector));

    assert!(matches!(
        status,
        CandidateDetectorRunStatus::Incomplete {
            total_cells: 10,
            examined_cells: 1,
            skipped_oversized_cells: 2,
            ..
        }
    ));
}

#[test]
fn malformed_candidates_are_rejected_without_changing_metadata() {
    let headers = vec!["misc".to_string()];
    let samples = vec![vec!["Renée".to_string()]];
    let expected = build_column_metadata(&headers, &samples);
    let mut detector = FakeCandidateDetector {
        result: Ok(candidate_result(vec![
            Candidate {
                column_index: 1,
                row_index: 0,
                start_byte: 0,
                end_byte: 1,
                kind: CandidateKind::PersonName,
                score_basis_points: 9_000,
            },
            Candidate {
                column_index: 0,
                row_index: 0,
                start_byte: 3,
                end_byte: 4,
                kind: CandidateKind::PersonName,
                score_basis_points: 9_000,
            },
            Candidate {
                column_index: 0,
                row_index: 0,
                start_byte: 0,
                end_byte: 1,
                kind: CandidateKind::PersonName,
                score_basis_points: 10_001,
            },
        ])),
    };

    let (actual, status) =
        build_column_metadata_with_candidate_detector(&headers, &samples, Some(&mut detector));

    assert_eq!(actual, expected);
    let CandidateDetectorRunStatus::Completed { rejections, .. } = status else {
        panic!("expected completed detector status");
    };
    assert!(
        rejections
            .iter()
            .any(|item| item.reason == CandidateRejectionReason::UnknownCell)
    );
    assert!(
        rejections
            .iter()
            .any(|item| item.reason == CandidateRejectionReason::InvalidSpan)
    );
    assert!(
        rejections
            .iter()
            .any(|item| item.reason == CandidateRejectionReason::ScoreOutOfRange)
    );
}

#[test]
fn candidate_cannot_replace_overlapping_deterministic_evidence() {
    let headers = vec!["email".to_string()];
    let samples = vec![vec!["ada@example.com".to_string()]];
    let expected = build_column_metadata(&headers, &samples);
    let mut detector = FakeCandidateDetector {
        result: Ok(candidate_result(vec![Candidate {
            column_index: 0,
            row_index: 0,
            start_byte: 0,
            end_byte: "ada@example.com".len(),
            kind: CandidateKind::PersonName,
            score_basis_points: 9_500,
        }])),
    };

    let (actual, status) =
        build_column_metadata_with_candidate_detector(&headers, &samples, Some(&mut detector));

    assert_eq!(actual, expected);
    let CandidateDetectorRunStatus::Completed { rejections, .. } = status else {
        panic!("expected completed detector status");
    };
    assert!(
        rejections
            .iter()
            .any(|item| { item.reason == CandidateRejectionReason::OverlapsDeterministicEvidence })
    );
}

#[test]
fn overlapping_model_candidates_are_resolved_before_replay() {
    let headers = vec!["misc".to_string()];
    let samples = vec![vec!["Ada Lovelace lives here".to_string()]];
    let mut detector = FakeCandidateDetector {
        result: Ok(candidate_result(vec![
            Candidate {
                column_index: 0,
                row_index: 0,
                start_byte: 0,
                end_byte: "Ada Lovelace".len(),
                kind: CandidateKind::PersonName,
                score_basis_points: 9_000,
            },
            Candidate {
                column_index: 0,
                row_index: 0,
                start_byte: 4,
                end_byte: "Ada Lovelace lives".len(),
                kind: CandidateKind::PrivateAddress,
                score_basis_points: 9_000,
            },
        ])),
    };

    let (metadata, status) =
        build_column_metadata_with_candidate_detector(&headers, &samples, Some(&mut detector));

    assert_eq!(
        metadata[0]
            .privacy_findings
            .iter()
            .filter(|finding| finding.detector == "local-ner:fake")
            .count(),
        1
    );
    let CandidateDetectorRunStatus::Completed {
        accepted_candidates,
        rejections,
        ..
    } = status
    else {
        panic!("expected completed detector status");
    };
    assert_eq!(accepted_candidates, 1);
    assert!(rejections.iter().any(|item| {
        item.reason == CandidateRejectionReason::OverlapsCandidateEvidence && item.count == 1
    }));
}
