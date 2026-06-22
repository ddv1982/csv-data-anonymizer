use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, AnonymizerError>;

#[derive(Debug, Error)]
pub enum AnonymizerError {
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("CSV parse error{row_text}: {message}")]
    CsvParse {
        message: String,
        row: Option<u64>,
        row_text: String,
    },

    #[error("Column index {index} is out of range. Valid range: 0-{max_index}")]
    ColumnOutOfRange { index: usize, max_index: usize },

    #[error("Output file already exists: {0}")]
    OutputExists(PathBuf),

    #[error("Output directory is not writable: {0}")]
    OutputDirectoryNotWritable(PathBuf),

    #[error("Processing canceled")]
    Canceled,

    #[error("Smart replacement error: {0}")]
    SmartReplacement(String),

    #[error("Privacy release error: {0}")]
    Privacy(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl AnonymizerError {
    pub fn csv_parse(message: impl Into<String>, row: Option<u64>) -> Self {
        let row_text = row.map(|row| format!(" at row {row}")).unwrap_or_default();
        Self::CsvParse {
            message: message.into(),
            row,
            row_text,
        }
    }
}

pub(crate) fn csv_error(error: csv::Error) -> AnonymizerError {
    let row = error.position().map(|position| position.line());
    AnonymizerError::csv_parse(error.to_string(), row)
}
