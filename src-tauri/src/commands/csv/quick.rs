use super::{load_local_ai_enabled, parse_tokenization_key};
use crate::command_error::CommandError;
use crate::commands::shared::run_blocking;
use crate::local_ai::{LocalAiRequest, smart_provider_for_strategy};
use crate::settings::SettingsStore;
use csv_anonymizer_core::{
    QuickGenerateParams, QuickTransformData, SmartReplacementProvider, TransformRuntime,
};
use serde::Deserialize;
use std::sync::Arc;
use tauri::State;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickGenerateRequest {
    #[serde(flatten)]
    pub params: QuickGenerateParams,
    pub local_ai: Option<LocalAiRequest>,
    #[serde(default)]
    pub tokenization_key: Option<String>,
}

#[tauri::command]
pub async fn generate_quick_values(
    settings: State<'_, Arc<SettingsStore>>,
    request: QuickGenerateRequest,
) -> Result<QuickTransformData, CommandError> {
    let local_ai_enabled = load_local_ai_enabled(&settings)?;
    let tokenization_key = parse_tokenization_key(request.tokenization_key.as_deref())
        .map_err(CommandError::invalid_input)?;
    run_blocking(move || {
        let mut provider = smart_provider_for_strategy(
            request.local_ai,
            request.params.strategy,
            local_ai_enabled,
        )?;
        let provider = provider
            .as_mut()
            .map(|provider| provider as &mut dyn SmartReplacementProvider);
        csv_anonymizer_core::direct_input::generate_quick_values(
            request.params,
            TransformRuntime {
                provider,
                tokenization_key: tokenization_key.as_ref(),
            },
        )
        .map_err(|error| error.to_string())
    })
    .await
    .map_err(Into::into)
}
