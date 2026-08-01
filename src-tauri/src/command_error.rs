use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: CommandErrorCode,
    pub message: String,
    pub remedy: Option<String>,
    pub retryable: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommandErrorCode {
    InvalidInput,
    PathNotAuthorized,
    StaleAnalysis,
    InternalError,
}

impl CommandError {
    pub fn new(
        code: CommandErrorCode,
        message: impl Into<String>,
        remedy: Option<&str>,
        retryable: bool,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            remedy: remedy.map(str::to_string),
            retryable,
        }
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(CommandErrorCode::InvalidInput, message, None, false)
    }

    pub fn path_not_authorized(message: impl Into<String>) -> Self {
        Self::new(
            CommandErrorCode::PathNotAuthorized,
            message,
            Some("Choose the file or output location again."),
            false,
        )
    }

    pub fn stale_analysis(message: impl Into<String>) -> Self {
        Self::new(
            CommandErrorCode::StaleAnalysis,
            message,
            Some("Analyze the source again."),
            false,
        )
    }
}

impl From<String> for CommandError {
    fn from(message: String) -> Self {
        Self::new(CommandErrorCode::InternalError, message, None, false)
    }
}

impl From<&str> for CommandError {
    fn from(message: &str) -> Self {
        message.to_string().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_analysis_has_a_stable_code_and_remedy() {
        let error = CommandError::stale_analysis("Analyze the source again: fingerprint changed");
        assert!(matches!(error.code, CommandErrorCode::StaleAnalysis));
        assert_eq!(error.remedy.as_deref(), Some("Analyze the source again."));
        assert!(!error.retryable);
    }
}
