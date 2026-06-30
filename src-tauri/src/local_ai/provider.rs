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
) -> Result<Option<OllamaSmartReplacementProvider>, String> {
    if !controls
        .iter()
        .any(|control| control.strategy == AnonymizationStrategy::LocalAi)
    {
        return Ok(None);
    }

    smart_provider_for_enabled_request(request)
}

pub fn smart_provider_for_strategy(
    request: Option<LocalAiRequest>,
    strategy: AnonymizationStrategy,
) -> Result<Option<OllamaSmartReplacementProvider>, String> {
    if strategy != AnonymizationStrategy::LocalAi {
        return Ok(None);
    }

    smart_provider_for_enabled_request(request)
}

fn smart_provider_for_enabled_request(
    request: Option<LocalAiRequest>,
) -> Result<Option<OllamaSmartReplacementProvider>, String> {
    let Some(request) = request.filter(|request| request.enabled) else {
        return Ok(None);
    };
    OllamaSmartReplacementProvider::new(request.model_name())
        .map(Some)
        .map_err(|error| error.to_string())
}
