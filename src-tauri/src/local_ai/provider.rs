use csv_anonymizer_core::{
    AnonymizationStrategy, AnonymizerError, ColumnControl, Result as CoreResult, SmartReplacement,
    SmartReplacementProvider, SmartReplacementRequest,
};
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::json;

use super::prompt::{PreparedPrompt, replacement_schema, smart_replacement_prompt};
use super::types::LocalAiRequest;
use super::{DEFAULT_OLLAMA_ENDPOINT, client, ensure_obviously_local_model};

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
        ensure_obviously_local_model(&model).map_err(AnonymizerError::SmartReplacement)?;
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
        let prepared = smart_replacement_prompt(request);
        if prepared.values.is_empty() {
            // Every value in this batch was withheld as implausibly long for the
            // detected type, so there is nothing to ask about. Returning an empty
            // answer rather than an error keeps the batch on the ordinary path: the
            // caller records each value as `MissingOutput` and falls back, exactly as
            // it would for a value the model declined to replace.
            return Ok(Vec::new());
        }
        let PreparedPrompt {
            prompt,
            skipped_values,
            ..
        } = prepared;
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
            // A withheld value was never described to the model, so an answer naming
            // one is the model reconstructing it from something else in the prompt —
            // most plausibly from injection text it decided to quote. Dropping it here
            // keeps that text out of the replacement map; the value then falls back
            // like any other unanswered one.
            .filter(|item| !skipped_values.contains(&item.original.as_str()))
            .map(|item| SmartReplacement {
                original: item.original,
                replacement: item.replacement,
            })
            .collect())
    }
}

/// The refusal a caller gets for asking Local AI to run while the persisted consent is off.
///
/// Shared so preflight predicts exactly what the run would say, rather than a paraphrase of it:
/// a preflight that warns in different words than the failure is a preflight users learn to skim.
pub const LOCAL_AI_DISABLED_MESSAGE: &str =
    "Local AI is off. Enable it in Settings before choosing Smart replacement.";

/// Whether this selection would send any value to Local AI.
///
/// One definition for both callers. Preflight uses it to decide whether to warn, and the run
/// itself to decide whether to build a provider; if the two ever disagreed, the direction that
/// matters is preflight saying no while the run says yes — that reports a file as staying local
/// and then sends it off the box.
pub fn selection_requires_local_ai(controls: &[ColumnControl], selected_columns: &[usize]) -> bool {
    controls.iter().any(|control| {
        selected_columns.contains(&control.column_index)
            && control.strategy == AnonymizationStrategy::LocalAi
    })
}

pub fn smart_provider_for_request(
    request: Option<LocalAiRequest>,
    controls: &[ColumnControl],
    selected_columns: &[usize],
    local_ai_enabled: bool,
) -> Result<Option<OllamaSmartReplacementProvider>, String> {
    if !selection_requires_local_ai(controls, selected_columns) {
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
        return Err(LOCAL_AI_DISABLED_MESSAGE.to_string());
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

    /// The predicate preflight warns on and the predicate the run gates on are now the same
    /// function, so this pins the answer once for both. The case that matters is the last one:
    /// a Local AI control on a column the user did not select must read as false, or every run
    /// carrying a leftover control would be announced as leaving the machine when it does not —
    /// and the case above it, where the selected column does use Local AI, must read as true,
    /// because that is the warning that stops a file being sent off the box unannounced.
    #[test]
    fn selection_requires_local_ai_follows_the_selected_columns() {
        assert!(selection_requires_local_ai(&[local_ai_control()], &[0]));
        assert!(!selection_requires_local_ai(&[local_ai_control()], &[]));
        assert!(!selection_requires_local_ai(&[local_ai_control()], &[1]));
        assert!(!selection_requires_local_ai(&[], &[0]));
        assert!(!selection_requires_local_ai(
            &[ColumnControl {
                column_index: 0,
                type_override: None,
                strategy: AnonymizationStrategy::Mask,
            }],
            &[0]
        ));
        // One selected Local AI column among others is still a yes.
        assert!(selection_requires_local_ai(
            &[
                ColumnControl {
                    column_index: 0,
                    type_override: None,
                    strategy: AnonymizationStrategy::Mask,
                },
                ColumnControl {
                    column_index: 3,
                    type_override: None,
                    strategy: AnonymizationStrategy::LocalAi,
                },
            ],
            &[0, 3]
        ));
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

    #[test]
    fn rejects_cloud_model_before_constructing_generation_provider() {
        let error = smart_provider_for_strategy(
            Some(LocalAiRequest {
                enabled: true,
                model: "glm-4.7:cloud".to_string(),
            }),
            AnonymizationStrategy::LocalAi,
            true,
        )
        .unwrap_err();

        assert!(error.contains("Cloud-backed Ollama models are not allowed"));
    }
}
