use crate::error::{AnonymizerError, Result, csv_error};
use crate::types::{ProcessControl, ProcessProgress};
use csv::{ReaderBuilder, StringRecord, Trim, WriterBuilder};
use std::fs;
use std::path::{Path, PathBuf};

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
        if row.len() < headers.len() {
            row.resize(headers.len(), String::new());
        }
        rows.push(DataRow {
            values: row,
            is_blank,
        });
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
    let temporary_output_path = temporary_output_path(output_path);
    let write_result = (|| {
        let mut writer = WriterBuilder::new()
            .has_headers(false)
            .from_path(&temporary_output_path)
            .map_err(csv_error)?;
        write(&mut writer, &mut control)?;
        writer.flush()?;
        Ok(())
    })();

    match write_result {
        Ok(()) => {
            fs::rename(&temporary_output_path, output_path)?;
            Ok(output_path.to_path_buf())
        }
        Err(error) => {
            let _ = fs::remove_file(&temporary_output_path);
            Err(error)
        }
    }
}

pub(super) fn check_canceled(control: &mut Option<&mut ProcessControl<'_>>) -> Result<()> {
    let Some(control) = control.as_deref_mut() else {
        return Ok(());
    };
    let Some(should_cancel) = control.should_cancel else {
        return Ok(());
    };
    if should_cancel() {
        Err(AnonymizerError::Canceled)
    } else {
        Ok(())
    }
}

pub(super) fn report_progress(
    control: &mut Option<&mut ProcessControl<'_>>,
    rows_processed: usize,
) {
    let Some(control) = control.as_deref_mut() else {
        return;
    };
    let Some(on_progress) = control.on_progress.as_deref_mut() else {
        return;
    };
    on_progress(ProcessProgress { rows_processed });
}

fn record_to_vec(record: &StringRecord) -> Vec<String> {
    record.iter().map(ToString::to_string).collect()
}

fn strip_bom(value: &str) -> &str {
    value.strip_prefix('\u{feff}').unwrap_or(value)
}

fn temporary_output_path(output_path: &Path) -> PathBuf {
    let parent = output_path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = output_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("output.csv");
    let suffix = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default()
    );
    parent.join(format!(".{file_name}.{suffix}.tmp"))
}
