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

    #[error("{format} parse error: {message}")]
    InputParse { format: String, message: String },

    #[error("Column index {index} is out of range. Valid range: 0-{max_index}")]
    ColumnOutOfRange { index: usize, max_index: usize },

    #[error("Output file already exists: {0}")]
    OutputExists(PathBuf),

    #[error("Output path must differ from the input file: {0}")]
    OutputSameAsInput(PathBuf),

    #[error("Output directory is not writable: {0}")]
    OutputDirectoryNotWritable(PathBuf),

    #[error("Processing canceled")]
    Canceled,

    /// The run held more consistent-replacement mapping than the ceiling allows.
    ///
    /// Names all three things a user needs: how far it got, where the limit is, and
    /// what to do instead. The remedy is specific because the generic advice — "use a
    /// smaller file" — is wrong here: the mapping grows with the number of *distinct*
    /// values in the selected columns, so a smaller file with the same variety fails
    /// the same way, while the same file on Redact or Mask does not grow at all.
    ///
    /// Says the output was not written because that is what happens. Every file run
    /// goes through `file_ops::replace_file_atomically`, which writes a temporary file
    /// beside the destination and only renames it into place once the closure returns
    /// `Ok`; any `Err` deletes the temporary and leaves the destination as it was. So
    /// this error behaves like the mid-run CSV parse error that `csv_io` already
    /// raises, and neither leaves a half-written output for someone to mistake for a
    /// finished one.
    #[error(
        "Consistent replacement needs more memory than this run is allowed: {reached} mapping \
         entries held (about {approximate_megabytes} MB), over the ceiling of {ceiling}. No \
         output was written. Strategies that keep repeated values linkable have to remember \
         every distinct value until the run ends, so this grows with the number of distinct \
         values in the columns you selected, not with the size of the file. Put the columns with \
         the most distinct values on Redact or Mask, which hold no mapping and stay flat at any \
         cardinality, or select fewer columns."
    )]
    MappingBudgetExceeded {
        reached: usize,
        ceiling: usize,
        approximate_megabytes: u64,
    },

    #[error("Smart replacement error: {0}")]
    SmartReplacement(String),

    #[error("Tokenization key must contain exactly 64 hexadecimal characters.")]
    InvalidTokenizationKey,

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

    pub fn input_parse(format: impl Into<String>, message: impl Into<String>) -> Self {
        Self::InputParse {
            format: format.into(),
            message: message.into(),
        }
    }
}

pub(crate) fn csv_error(error: csv::Error) -> AnonymizerError {
    let row = error.position().map(|position| position.line());
    AnonymizerError::csv_parse(error.to_string(), row)
}
