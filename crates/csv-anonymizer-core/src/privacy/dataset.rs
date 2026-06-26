use crate::csv_io::{normalize_data_row, record_to_vec, strip_bom, write_csv_output_record};
use crate::error::{AnonymizerError, Result, csv_error};
use crate::file_ops::write_csv_file_atomically;
use crate::types::ProcessControl;
use csv::{ReaderBuilder, Trim};
use std::path::{Path, PathBuf};

pub(super) use crate::process_control::{check_canceled, report_progress};

pub(super) const MAX_IN_MEMORY_DATA_ROWS: usize = 1_000_000;

#[derive(Debug, Clone)]
pub(super) struct CsvDataset {
    pub(super) headers: Vec<String>,
    pub(super) rows: Vec<DataRow>,
}

impl CsvDataset {
    pub(super) fn data_row_count(&self) -> usize {
        self.rows.iter().filter(|row| !row.is_blank).count()
    }
}

#[derive(Debug, Clone)]
pub(super) struct DataRow {
    pub(super) values: Vec<String>,
    pub(super) is_blank: bool,
}

pub(super) fn read_dataset(
    input_path: &Path,
    mut control: Option<&mut ProcessControl<'_>>,
) -> Result<CsvDataset> {
    let mut reader = ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .trim(Trim::All)
        .from_path(input_path)
        .map_err(csv_error)?;
    let mut headers = Vec::new();
    let mut rows = Vec::new();
    let mut header_processed = false;

    check_canceled(&mut control)?;
    for result in reader.records() {
        let record = result.map_err(csv_error)?;
        let mut row = record_to_vec(&record);

        if !header_processed {
            if let Some(first) = row.first_mut() {
                *first = strip_bom(first).to_string();
            }
            if row.is_empty() {
                return Err(AnonymizerError::csv_parse(
                    "CSV file is empty or has no valid headers",
                    None,
                ));
            }
            headers = row;
            header_processed = true;
            continue;
        }

        let is_blank = row.iter().all(|value| value.trim().is_empty());
        row = normalize_data_row(row, headers.len(), record.position().map(|pos| pos.line()))?;
        rows.push(DataRow {
            values: row,
            is_blank,
        });
        if rows.len() > MAX_IN_MEMORY_DATA_ROWS {
            return Err(AnonymizerError::Privacy(format!(
                "privacy release inputs are limited to {MAX_IN_MEMORY_DATA_ROWS} data row(s) because this release mode materializes the dataset in memory"
            )));
        }
        check_canceled(&mut control)?;
    }

    if headers.is_empty() {
        return Err(AnonymizerError::csv_parse(
            "CSV file is empty or has no valid headers",
            None,
        ));
    }

    Ok(CsvDataset { headers, rows })
}

pub(super) fn write_atomically(
    output_path: &Path,
    mut control: Option<&mut ProcessControl<'_>>,
    write: impl FnOnce(
        &mut csv::Writer<std::fs::File>,
        &mut Option<&mut ProcessControl<'_>>,
    ) -> Result<()>,
) -> Result<PathBuf> {
    write_csv_file_atomically(output_path, |writer| {
        write(writer, &mut control)?;
        Ok(())
    })
}

pub(super) fn write_record<'a>(
    writer: &mut csv::Writer<std::fs::File>,
    record: impl IntoIterator<Item = &'a str>,
) -> Result<()> {
    write_csv_output_record(writer, record)
}
