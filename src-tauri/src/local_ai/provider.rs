use csv_anonymizer_core::{
    AnonymizationStrategy, AnonymizerError, ColumnControl, Result as CoreResult, SmartReplacement,
    SmartReplacementProvider, SmartReplacementRequest,
};
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::json;

use super::prompt::{replacement_schema, smart_replacement_prompt};
use super::types::LocalAiRequest;
use super::{DEFAULT_OLLAMA_ENDPOINT, client};

#[derive(Debug, Clone)]
pub struct OllamaSmartReplacementProvider {
    client: Client,
    endpoint: String,
    model: String,
}

#[derive(Debug, Clone, Deserialize)]
struct OllamaGenerateResponse {
    response: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ReplacementPayload {
    replacements: Vec<ReplacementItem>,
}

#[derive(Debug, Clone, Deserialize)]
struct ReplacementItem {
    original: String,
    replacement: String,
}

impl OllamaSmartReplacementProvider {
    fn new(model: String) -> CoreResult<Self> {
        Ok(Self {
            client: client().map_err(AnonymizerError::SmartReplacement)?,
            endpoint: DEFAULT_OLLAMA_ENDPOINT.to_string(),
            model,
        })
    }
}

impl SmartReplacementProvider for OllamaSmartReplacementProvider {
    fn generate_replacements(
        &mut self,
        request: SmartReplacementRequest<'_>,
    ) -> CoreResult<Vec<SmartReplacement>> {
        let prompt = smart_replacement_prompt(request);
        let options = json!({
            "temperature": 0.35
        });
        let body = json!({
            "model": self.model,
            "prompt": prompt,
            "stream": false,
            "format": replacement_schema(),
            "options": options
        });
        // Accepted limitation: this blocking request cannot be interrupted by
        // job cancellation; cancel takes effect between batches, so a slow
        // model can delay cancellation by up to the client timeout (120s).
        let response = self
            .client
            .post(format!("{}/api/generate", self.endpoint))
            .json(&body)
            .send()
            .map_err(|error| {
                AnonymizerError::SmartReplacement(format!("Local AI request failed: {error}"))
            })?
            .error_for_status()
            .map_err(|error| {
                AnonymizerError::SmartReplacement(format!("Local AI request failed: {error}"))
            })?
            .json::<OllamaGenerateResponse>()
            .map_err(|error| {
                AnonymizerError::SmartReplacement(format!(
                    "Local AI response was not valid: {error}"
                ))
            })?;
        let parsed =
            serde_json::from_str::<ReplacementPayload>(&response.response).map_err(|error| {
                AnonymizerError::SmartReplacement(format!(
                    "Local AI returned replacement data that could not be parsed: {error}"
                ))
            })?;
        Ok(parsed
            .replacements
            .into_iter()
            .map(|item| SmartReplacement {
                original: item.original,
                replacement: item.replacement,
            })
            .collect())
    }
}

pub fn smart_provider_for_request(
    request: Option<LocalAiRequest>,
    controls: &[ColumnControl],
    selected_columns: &[usize],
    local_ai_enabled: bool,
) -> Result<Option<OllamaSmartReplacementProvider>, String> {
    if !controls.iter().any(|control| {
        selected_columns.contains(&control.column_index)
            && control.strategy == AnonymizationStrategy::LocalAi
    }) {
        return Ok(None);
    }

    smart_provider_for_enabled_request(request, local_ai_enabled)
}

pub fn smart_provider_for_strategy(
    request: Option<LocalAiRequest>,
    strategy: AnonymizationStrategy,
    local_ai_enabled: bool,
) -> Result<Option<OllamaSmartReplacementProvider>, String> {
    if strategy != AnonymizationStrategy::LocalAi {
        return Ok(None);
    }

    smart_provider_for_enabled_request(request, local_ai_enabled)
}

fn smart_provider_for_enabled_request(
    request: Option<LocalAiRequest>,
    local_ai_enabled: bool,
) -> Result<Option<OllamaSmartReplacementProvider>, String> {
    let Some(request) = request.filter(|request| request.enabled) else {
        return Ok(None);
    };
    if !local_ai_enabled {
        return Err(
            "Local AI is off. Enable it in Settings before choosing Smart replacement.".to_string(),
        );
    }
    OllamaSmartReplacementProvider::new(request.model_name())
        .map(Some)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local_ai_request() -> LocalAiRequest {
        LocalAiRequest {
            enabled: true,
            model: "gemma3:4b".to_string(),
        }
    }

    fn local_ai_control() -> ColumnControl {
        ColumnControl {
            column_index: 0,
            type_override: None,
            strategy: AnonymizationStrategy::LocalAi,
        }
    }

    #[test]
    fn rejects_request_enabled_when_persisted_local_ai_consent_is_off() {
        let error = smart_provider_for_request(
            Some(local_ai_request()),
            &[local_ai_control()],
            &[0],
            false,
        )
        .unwrap_err();

        assert!(error.contains("Local AI is off"));
    }

    #[test]
    fn ignores_persisted_local_ai_consent_for_non_local_ai_controls() {
        let provider = smart_provider_for_request(
            Some(local_ai_request()),
            &[ColumnControl {
                column_index: 0,
                type_override: None,
                strategy: AnonymizationStrategy::Mask,
            }],
            &[0],
            false,
        )
        .unwrap();

        assert!(provider.is_none());
    }

    #[test]
    fn ignores_unselected_local_ai_controls() {
        let provider = smart_provider_for_request(
            Some(local_ai_request()),
            &[local_ai_control()],
            &[1],
            false,
        )
        .unwrap();

        assert!(provider.is_none());
    }

    #[test]
    fn creates_provider_when_request_and_persisted_consent_are_enabled() {
        let provider = smart_provider_for_strategy(
            Some(local_ai_request()),
            AnonymizationStrategy::LocalAi,
            true,
        )
        .unwrap();

        assert!(provider.is_some());
    }
}
