use super::PrivacyProcessResult;
use super::dataset::{
    CsvDataset, check_canceled, read_dataset, report_progress, write_atomically, write_record,
};
use super::generalization::generalize_value;
use super::roles::{RolePlan, build_role_plan, validate_common_config};
use crate::error::{AnonymizerError, Result};
use crate::hash::{deterministic_number, deterministic_uuid};
use crate::release_report::{
    ReportContext, build_column_reports, build_evidence, build_readiness, build_utility_metrics,
};
use crate::types::{
    ColumnMetadata, ColumnRole, DataType, PrivacyConfig, PrivacyModel, PrivacyModelReport,
    PrivacyReport, ProcessControl, ReleaseMode, SyntheticDataConfig,
};
use std::path::{Path, PathBuf};
use std::time::Instant;

struct SyntheticWritePlan<'a> {
    columns: &'a [ColumnMetadata],
    role_plan: &'a RolePlan,
    row_count: usize,
    seed: &'a str,
}

pub(super) fn process_synthetic_data(
    input_path: &Path,
    output_path: &Path,
    columns: &[ColumnMetadata],
    config: &PrivacyConfig,
    _deterministic: bool,
    seed: &str,
    mut control: Option<&mut ProcessControl<'_>>,
) -> Result<PrivacyProcessResult> {
    validate_common_config(columns, config)?;
    validate_synthetic_config(&config.synthetic)?;
    let start_time = Instant::now();
    let dataset = read_dataset(input_path, control.as_deref_mut())?;
    let role_plan = build_role_plan(columns, config)?;
    let requested_rows = config
        .synthetic
        .row_count
        .unwrap_or_else(|| dataset.data_row_count());
    let write_plan = SyntheticWritePlan {
        columns,
        role_plan: &role_plan,
        row_count: requested_rows,
        seed,
    };
    let output_path = write_synthetic_release(output_path, &dataset, write_plan, control)?;
    let formal_models = vec![PrivacyModelReport {
        model: PrivacyModel::SyntheticData,
        satisfied: true,
        actual: format!("{requested_rows} generated row(s)"),
        threshold: format!("{requested_rows} requested row(s)"),
        message: "Generated rows are sampled independently from column distributions and direct identifiers are replaced."
            .to_string(),
    }];
    let report_context = ReportContext {
        roles: Some(&role_plan.roles),
        synthetic_rows: Some(requested_rows),
        ..ReportContext::default()
    };

    Ok(PrivacyProcessResult {
        row_count: requested_rows,
        output_path,
        duration_ms: start_time.elapsed().as_millis(),
        columns_anonymized: columns.iter().filter(|column| column.is_selected).count(),
        privacy_report: PrivacyReport {
            release_mode: ReleaseMode::SyntheticData,
            direct_identifiers: role_plan.role_count(ColumnRole::DirectIdentifier),
            quasi_identifiers: role_plan.role_count(ColumnRole::QuasiIdentifier),
            sensitive_columns: role_plan.role_count(ColumnRole::Sensitive),
            pseudonymized_columns: role_plan.role_count(ColumnRole::DirectIdentifier)
                + role_plan.role_count(ColumnRole::Sensitive),
            smart_replacement_columns: 0,
            opaque_token_columns: 0,
            masked_columns: 0,
            redacted_columns: 0,
            generalized_columns: role_plan.role_count(ColumnRole::QuasiIdentifier),
            pass_through_columns: role_plan.role_count(ColumnRole::Attribute),
            suppressed_rows: 0,
            synthetic_rows: requested_rows,
            dp_epsilon: None,
            dp_budget: None,
            unique_pseudonym_values: 0,
            reused_pseudonym_values: 0,
            collisions_avoided: 0,
            exhausted_pseudonym_pools: 0,
            opaque_token_values: 0,
            smart_replacement_values: 0,
            smart_replacement_rejections: 0,
            smart_replacement_rejection_reasons: Vec::new(),
            smart_replacement_fallbacks: 0,
            formal_models,
            readiness: build_readiness(
                ReleaseMode::SyntheticData,
                columns,
                Some(config),
                &report_context,
            ),
            evidence: build_evidence(ReleaseMode::SyntheticData, columns, &report_context),
            column_reports: build_column_reports(
                ReleaseMode::SyntheticData,
                columns,
                report_context.roles,
            ),
            utility_metrics: build_utility_metrics(
                ReleaseMode::SyntheticData,
                columns,
                &report_context,
            ),
            notes: synthetic_notes(),
        },
    })
}

pub(super) fn validate_synthetic_config(config: &SyntheticDataConfig) -> Result<()> {
    if let Some(row_count) = config.row_count
        && row_count > 1_000_000
    {
        return Err(AnonymizerError::Privacy(
            "synthetic data row count is limited to 1,000,000 rows".to_string(),
        ));
    }
    if config.epsilon.is_some() {
        return Err(AnonymizerError::Privacy(
            "synthetic DP epsilon is not supported by this generator; clear epsilon until a DP synthetic-data generator is implemented"
                .to_string(),
        ));
    }
    Ok(())
}

fn write_synthetic_release(
    output_path: &Path,
    dataset: &CsvDataset,
    plan: SyntheticWritePlan<'_>,
    control: Option<&mut ProcessControl<'_>>,
) -> Result<PathBuf> {
    write_atomically(output_path, control, |writer, control| {
        write_record(writer, dataset.headers.iter().map(String::as_str))?;
        for row_index in 0..plan.row_count {
            check_canceled(control)?;
            let row = plan
                .columns
                .iter()
                .map(|column| {
                    let role = plan
                        .role_plan
                        .roles
                        .get(column.index)
                        .copied()
                        .unwrap_or(ColumnRole::Attribute);
                    synthetic_value(column, role, row_index, plan.seed)
                })
                .collect::<Vec<_>>();
            write_record(writer, row.iter().map(String::as_str))?;
            report_progress(control, row_index + 1);
        }
        Ok(())
    })
}

fn synthetic_value(
    column: &ColumnMetadata,
    role: ColumnRole,
    row_index: usize,
    seed: &str,
) -> String {
    if matches!(role, ColumnRole::DirectIdentifier | ColumnRole::Exclude) {
        return synthetic_identifier(column, role, row_index, seed);
    }
    if role == ColumnRole::Sensitive {
        return synthetic_sensitive_value(column, role, row_index, seed);
    }
    if role == ColumnRole::QuasiIdentifier {
        let value = synthetic_attribute_value(column, role, row_index, seed);
        generalize_value(&value, column.detected_type, 1)
    } else {
        synthetic_attribute_value(column, role, row_index, seed)
    }
}

fn synthetic_identifier(
    column: &ColumnMetadata,
    role: ColumnRole,
    row_index: usize,
    seed: &str,
) -> String {
    let suffix = synthetic_sequence_number(column, role, row_index, seed, "identifier", 1_000_000);
    let key = synthetic_value_key(column, role, row_index, "identifier");
    match column.detected_type {
        DataType::Email => format!("person{suffix}@example.invalid"),
        DataType::Phone => format!("555-010-{:04}", suffix % 10_000),
        DataType::FirstName => format!("First{suffix}"),
        DataType::LastName => format!("Last{suffix}"),
        DataType::FullName => format!("Person {suffix}"),
        DataType::TaxId => format!("TAX-{:06}", suffix % 1_000_000),
        DataType::Address => format!("{suffix} Example Street"),
        DataType::Uuid => deterministic_uuid(&key, seed),
        _ => format!("synthetic-{suffix}"),
    }
}

fn synthetic_sensitive_value(
    column: &ColumnMetadata,
    role: ColumnRole,
    row_index: usize,
    seed: &str,
) -> String {
    let key = synthetic_value_key(column, role, row_index, "sensitive");
    let suffix = synthetic_sequence_number(column, role, row_index, seed, "sensitive", 1_000_000);
    match column.detected_type {
        DataType::NumericId
        | DataType::NumericValue
        | DataType::Currency
        | DataType::Percentage => deterministic_number(&key, seed, 1, 100).to_string(),
        DataType::Boolean => {
            if deterministic_number(&key, seed, 0, 1) == 0 {
                "false".to_string()
            } else {
                "true".to_string()
            }
        }
        DataType::Timestamp => synthetic_timestamp(&key, seed),
        _ => format!("synthetic-sensitive-{suffix}"),
    }
}

fn synthetic_attribute_value(
    column: &ColumnMetadata,
    role: ColumnRole,
    row_index: usize,
    seed: &str,
) -> String {
    let key = synthetic_value_key(column, role, row_index, "attribute");
    let suffix = synthetic_sequence_number(column, role, row_index, seed, "attribute", 1_000_000);
    match column.detected_type {
        DataType::Email => format!("attribute{suffix}@example.invalid"),
        DataType::Phone => format!("555-020-{:04}", suffix % 10_000),
        DataType::FirstName => format!("AttrFirst{suffix}"),
        DataType::LastName => format!("AttrLast{suffix}"),
        DataType::FullName => format!("Attribute Person {suffix}"),
        DataType::TaxId => format!("ATTR-{:06}", suffix % 1_000_000),
        DataType::Address => format!("{suffix} Attribute Avenue"),
        DataType::Uuid => deterministic_uuid(&key, seed),
        DataType::NumericId | DataType::NumericValue | DataType::Currency => {
            deterministic_number(&key, seed, 1, 10_000).to_string()
        }
        DataType::Percentage => deterministic_number(&key, seed, 0, 100).to_string(),
        DataType::Boolean => {
            if deterministic_number(&key, seed, 0, 1) == 0 {
                "false".to_string()
            } else {
                "true".to_string()
            }
        }
        DataType::Timestamp => synthetic_timestamp(&key, seed),
        DataType::CountryCode => format!("ZZ{:02}", suffix % 100),
        DataType::Enum => format!("synthetic-enum-{suffix}"),
        DataType::PostalCode => format!("000{:02}", suffix % 100),
        DataType::IpAddress => format!("192.0.2.{}", (suffix % 254) + 1),
        DataType::Url => format!("https://example.invalid/item/{suffix}"),
        DataType::MacAddress => synthetic_mac_address(&key, seed),
        DataType::String | DataType::Unknown => format!("synthetic-attribute-{suffix}"),
    }
}

fn synthetic_timestamp(key: &str, seed: &str) -> String {
    let year = deterministic_number(&format!("{key}:year"), seed, 2000, 2029);
    let month = deterministic_number(&format!("{key}:month"), seed, 1, 12);
    let day = deterministic_number(&format!("{key}:day"), seed, 1, 28);
    let hour = deterministic_number(&format!("{key}:hour"), seed, 0, 23);
    let minute = deterministic_number(&format!("{key}:minute"), seed, 0, 59);
    let second = deterministic_number(&format!("{key}:second"), seed, 0, 59);

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn synthetic_mac_address(key: &str, seed: &str) -> String {
    let octet_3 = deterministic_number(&format!("{key}:mac:3"), seed, 0, 255);
    let octet_4 = deterministic_number(&format!("{key}:mac:4"), seed, 0, 255);
    let octet_5 = deterministic_number(&format!("{key}:mac:5"), seed, 0, 255);
    let octet_6 = deterministic_number(&format!("{key}:mac:6"), seed, 0, 255);

    format!("02:00:{octet_3:02x}:{octet_4:02x}:{octet_5:02x}:{octet_6:02x}")
}

fn synthetic_sequence_number(
    column: &ColumnMetadata,
    role: ColumnRole,
    row_index: usize,
    seed: &str,
    namespace: &str,
    modulus: usize,
) -> usize {
    let column_key = synthetic_column_key(column, role, namespace);
    let offset = deterministic_number(&column_key, seed, 0, modulus.saturating_sub(1) as i64);
    ((offset as usize + row_index) % modulus) + 1
}

fn synthetic_value_key(
    column: &ColumnMetadata,
    role: ColumnRole,
    row_index: usize,
    namespace: &str,
) -> String {
    format!(
        "{}:row={}",
        synthetic_column_key(column, role, namespace),
        row_index + 1
    )
}

fn synthetic_column_key(column: &ColumnMetadata, role: ColumnRole, namespace: &str) -> String {
    format!(
        "synthetic:v2:{namespace}:role={}:type={}:column={}:name={}",
        synthetic_role_key(role),
        synthetic_data_type_key(column.detected_type),
        column.index,
        column.name
    )
}

fn synthetic_role_key(role: ColumnRole) -> &'static str {
    match role {
        ColumnRole::Auto => "auto",
        ColumnRole::DirectIdentifier => "direct-identifier",
        ColumnRole::QuasiIdentifier => "quasi-identifier",
        ColumnRole::Sensitive => "sensitive",
        ColumnRole::Attribute => "attribute",
        ColumnRole::Exclude => "exclude",
    }
}

fn synthetic_data_type_key(data_type: DataType) -> &'static str {
    match data_type {
        DataType::Email => "email",
        DataType::Uuid => "uuid",
        DataType::Timestamp => "timestamp",
        DataType::NumericId => "numeric-id",
        DataType::NumericValue => "numeric-value",
        DataType::PostalCode => "postal-code",
        DataType::Address => "address",
        DataType::IpAddress => "ip-address",
        DataType::Url => "url",
        DataType::MacAddress => "mac-address",
        DataType::TaxId => "tax-id",
        DataType::Boolean => "boolean",
        DataType::Currency => "currency",
        DataType::Percentage => "percentage",
        DataType::CountryCode => "country-code",
        DataType::Phone => "phone",
        DataType::FirstName => "first-name",
        DataType::LastName => "last-name",
        DataType::FullName => "full-name",
        DataType::Enum => "enum",
        DataType::String => "string",
        DataType::Unknown => "unknown",
    }
}

fn synthetic_notes() -> Vec<String> {
    vec![
        "Synthetic data mode generates new rows from simple per-column distributions and does not make the source data anonymous by itself."
            .to_string(),
        "Direct identifier columns are replaced with generated placeholders instead of sampled source values."
            .to_string(),
        "Sensitive and Attribute columns are replaced with generated placeholders instead of sampled source values."
            .to_string(),
    ]
}
