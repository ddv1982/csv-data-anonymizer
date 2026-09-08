use super::*;
use serde_json::json;
use std::path::PathBuf;
#[test]
fn column_metadata_serializes_frontend_contract_shape() {
    let column = ColumnMetadata {
        header_label_is_ambiguous: false,
        name: "email".to_string(),
        source_path: Some("$.user.email".to_string()),
        index: 2,
        detected_type: DataType::Email,
        confidence: Confidence::High,
        detection_trace: Some(DetectionTrace {
            summary: "email evidence".to_string(),
            selected_reason: "value matched email".to_string(),
            total_non_empty: 1,
            candidates: vec![DetectionTraceItem {
                data_type: DataType::Email,
                reason: "valid email".to_string(),
                match_count: 1,
                total_considered: 1,
                confidence: Confidence::High,
                accepted: true,
            }],
        }),
        privacy_findings: Vec::new(),
        privacy_evidence: vec![PrivacyEvidenceSummary {
            kind: PrivacyFindingKind::Contact,
            data_type: DataType::Email,
            confidence: Confidence::High,
            match_count: 1,
            sample_count: 1,
            score: 100,
            detector: "email".to_string(),
            reason: "Column contains contact details.".to_string(),
            detectors: vec!["email".to_string()],
        }],
        review_reasons: vec![ColumnReviewReason::AmbiguousContext],
        evidence_disposition: EvidenceDisposition::DetectedSensitive,
        evidence_profile: Default::default(),
        pii_risk: PiiRisk::High,
        sample_values: vec!["ada@example.com".to_string()],
        sample_value_distribution: Default::default(),
        empty_format: EmptyFormat::EmptyString,
        is_selected: true,
        strategy: AnonymizationStrategy::Redact,
    };

    let value = serde_json::to_value(&column).unwrap();

    assert_eq!(value["detectedType"], json!("email"));
    assert_eq!(value["detectionTrace"]["totalNonEmpty"], json!(1));
    assert_eq!(value["privacyEvidence"][0]["matchCount"], json!(1));
    assert_eq!(value["reviewReasons"], json!(["ambiguousContext"]));
    assert_eq!(value["piiRisk"], json!("high"));
    assert_eq!(value["sampleValues"], json!(["ada@example.com"]));
    assert_eq!(value["emptyFormat"], json!("emptyString"));
    assert_eq!(value["isSelected"], json!(true));
    assert_eq!(value["strategy"], json!("redact"));
    assert!(value.get("detected_type").is_none());
    assert!(value.get("pii_risk").is_none());

    let round_trip: ColumnMetadata = serde_json::from_value(value).unwrap();
    assert_eq!(round_trip, column);
}

#[test]
fn preflight_params_serialize_optional_and_default_contract_fields() {
    let params = PreflightParams {
        mode: PreflightMode::Anonymize,
        file_path: PathBuf::from("/data/input.csv"),
        output_path: None,
        columns: vec![0, 2],
        controls: Vec::new(),
        force: false,
        sample_row_count: 10,
        preview_smart_replacements: Vec::new(),
        local_ai_ready: false,
        local_ai_message: None,
    };

    let value = serde_json::to_value(&params).unwrap();

    assert_eq!(value["mode"], json!("anonymize"));
    assert_eq!(value["sampleRowCount"], json!(10));
    assert_eq!(value["previewSmartReplacements"], json!([]));
    assert_eq!(value["localAiReady"], json!(false));
    assert!(value.get("outputPath").is_none());
    assert!(value.get("localAiMessage").is_none());
    assert!(value.get("sample_row_count").is_none());

    let minimal = json!({
        "mode": "preview",
        "filePath": "/data/input.csv",
        "columns": [1],
        "force": false,
        "sampleRowCount": 5,
        "localAiReady": true
    });
    let decoded: PreflightParams = serde_json::from_value(minimal).unwrap();
    assert_eq!(decoded.mode, PreflightMode::Preview);
    assert_eq!(decoded.controls, Vec::<ColumnControl>::new());
    assert_eq!(
        decoded.preview_smart_replacements,
        Vec::<SmartReplacementEntry>::new()
    );
    assert_eq!(decoded.output_path, None);
    assert_eq!(decoded.local_ai_message, None);
}

#[test]
fn privacy_report_serializes_nested_release_and_smart_replacement_fields() {
    let report = PrivacyReport {
        detection_run_summary: None,
        direct_identifiers: 1,
        quasi_identifiers: 2,
        pseudonymized_columns: 1,
        smart_replacement_columns: 1,
        opaque_token_columns: 0,
        masked_columns: 0,
        labelled_columns: 0,
        redacted_columns: 1,
        pass_through_columns: 0,
        unique_pseudonym_values: 3,
        reused_pseudonym_values: 0,
        collisions_avoided: 0,
        exhausted_pseudonym_pools: 0,
        opaque_token_values: 0,
        keyed_token_values: 0,
        keyed_token_columns: Vec::new(),
        smart_replacement_values: 2,
        smart_replacement_rejections: 1,
        smart_replacement_rejection_reasons: vec![SmartReplacementRejectionCount {
            reason: SmartReplacementRejectionReason::ContainsOriginal,
            count: 1,
        }],
        smart_replacement_fallbacks: 1,
        shape_fallback_values: 2,
        readiness: ReleaseReadiness {
            status: ReleaseReadinessStatus::Review,
            blockers: Vec::new(),
            review_items: vec!["Review Smart replacement output.".to_string()],
            verified_items: Vec::new(),
        },
        evidence: vec![ReleaseEvidenceItem {
            id: "local-ai".to_string(),
            label: "Local AI".to_string(),
            status: ReleaseEvidenceStatus::Review,
            detail: "Review generated values.".to_string(),
        }],
        column_reports: vec![ColumnReleaseReport {
            column_index: 2,
            column_name: "email".to_string(),
            selected: true,
            detected_type: DataType::Email,
            pii_risk: PiiRisk::High,
            strategy: AnonymizationStrategy::LocalAi,
            action: "Smart replacement".to_string(),
            status: ReleaseEvidenceStatus::Review,
            detail: "Generated replacements.".to_string(),
        }],
        column_value_distributions: vec![ColumnValueDistribution {
            column_index: 2,
            distinct_values: 3,
            total_values: 10,
            singleton_values: 1,
            doubleton_values: 0,
            max_value_occurrences: 6,
        }],
        row_uniqueness: Some(RowUniquenessSummary {
            rows_measured: 10,
            matched_columns: vec![
                MatchedColumn {
                    column_index: 0,
                    matched_on: MatchedPart::WholeValue,
                    matched_every_row: true,
                },
                MatchedColumn {
                    column_index: 2,
                    matched_on: MatchedPart::DateDecadeAndTime,
                    matched_every_row: true,
                },
                MatchedColumn {
                    column_index: 3,
                    matched_on: MatchedPart::SurvivingFormat,
                    matched_every_row: true,
                },
            ],
            distinct_classes: 7,
            unique_rows: 4,
            smallest_class: 1,
            fifth_percentile_class_size: 1,
            distinct_rows_all_columns: Some(10),
            measurement_incomplete: false,
            drop_column_effects: vec![
                DropColumnEffect {
                    column_index: 2,
                    unique_rows_without: 1,
                },
                DropColumnEffect {
                    column_index: 0,
                    unique_rows_without: 3,
                },
            ],
            drop_attribution_incomplete: false,
        }),
        utility_metrics: vec![UtilityMetric {
            label: "Rows".to_string(),
            value: "10".to_string(),
            status: ReleaseEvidenceStatus::Info,
            detail: Some("sample".to_string()),
        }],
        notes: vec!["Review generated replacements.".to_string()],
    };

    let value = serde_json::to_value(&report).unwrap();

    assert_eq!(value["directIdentifiers"], json!(1));
    assert_eq!(value["smartReplacementColumns"], json!(1));
    assert_eq!(
        value["smartReplacementRejectionReasons"][0]["reason"],
        json!("containsOriginal")
    );
    assert_eq!(value["readiness"]["status"], json!("review"));
    assert_eq!(value["evidence"][0]["status"], json!("review"));
    assert_eq!(value["columnReports"][0]["detectedType"], json!("email"));
    assert_eq!(value["utilityMetrics"][0]["status"], json!("info"));
    assert_eq!(
        value["columnValueDistributions"][0]["distinctValues"],
        json!(3)
    );
    assert_eq!(
        value["columnValueDistributions"][0]["singletonValues"],
        json!(1)
    );
    assert!(value.get("direct_identifiers").is_none());
    assert!(value.get("smart_replacement_columns").is_none());

    let round_trip: PrivacyReport = serde_json::from_value(value).unwrap();
    assert_eq!(round_trip, report);
}

#[test]
fn privacy_report_accepts_defaulted_newer_fields_when_deserializing() {
    let value = json!({
        "directIdentifiers": 1,
        "quasiIdentifiers": 0,
        "pseudonymizedColumns": 1,
        "smartReplacementColumns": 0,
        "opaqueTokenColumns": 0,
        "maskedColumns": 0,
        "passThroughColumns": 0,
        "uniquePseudonymValues": 1,
        "reusedPseudonymValues": 0,
        "collisionsAvoided": 0,
        "exhaustedPseudonymPools": 0,
        "opaqueTokenValues": 0,
        "smartReplacementValues": 0,
        "smartReplacementFallbacks": 0,
        "notes": []
    });

    let report: PrivacyReport = serde_json::from_value(value).unwrap();

    assert_eq!(report.redacted_columns, 0);
    assert_eq!(report.smart_replacement_rejections, 0);
    assert_eq!(report.smart_replacement_rejection_reasons, Vec::new());
    assert_eq!(report.readiness, ReleaseReadiness::default());
    assert_eq!(report.evidence, Vec::new());
    assert_eq!(report.column_reports, Vec::new());
    assert_eq!(report.utility_metrics, Vec::new());
}

#[test]
fn selected_enums_use_camel_case_wire_values() {
    assert_eq!(
        serde_json::to_value(DataType::NumericId).unwrap(),
        json!("numericId")
    );
    assert_eq!(
        serde_json::to_value(PasteDataFormat::PlainText).unwrap(),
        json!("plainText")
    );
    assert_eq!(
        serde_json::to_value(AnonymizationStrategy::PassThrough).unwrap(),
        json!("passThrough")
    );
    assert_eq!(
        serde_json::to_value(ReleaseEvidenceStatus::Info).unwrap(),
        json!("info")
    );
}
