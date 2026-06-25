use reqwest::blocking::Client;
use serde::Deserialize;

use super::types::{LocalAiRequest, LocalAiStatus};
use super::{DEFAULT_OLLAMA_ENDPOINT, OLLAMA_UNAVAILABLE_MESSAGE, client};

#[derive(Debug, Clone, Deserialize)]
struct OllamaVersion {
    version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct OllamaTags {
    models: Vec<OllamaModel>,
}

#[derive(Debug, Clone, Deserialize)]
struct OllamaModel {
    name: Option<String>,
    model: Option<String>,
}

pub fn local_ai_status(request: LocalAiRequest) -> Result<LocalAiStatus, String> {
    local_ai_status_with_endpoint(request, DEFAULT_OLLAMA_ENDPOINT)
}

pub fn ensure_ollama_runtime_available() -> Result<(), String> {
    ensure_runtime_available(DEFAULT_OLLAMA_ENDPOINT)
}

fn local_ai_status_with_endpoint(
    request: LocalAiRequest,
    endpoint: &str,
) -> Result<LocalAiStatus, String> {
    let model = request.model_name();
    let client = client()?;
    let version = ollama_version(&client, endpoint);
    let Ok(version) = version else {
        return Ok(LocalAiStatus {
            enabled: request.enabled,
            provider: "ollama".to_string(),
            model,
            available_models: Vec::new(),
            endpoint: endpoint.to_string(),
            runtime_available: false,
            model_installed: false,
            ready: false,
            runtime_version: None,
            message: OLLAMA_UNAVAILABLE_MESSAGE.to_string(),
        });
    };

    let tags = client
        .get(format!("{endpoint}/api/tags"))
        .send()
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.json::<OllamaTags>())
        .map_err(|error| format!("Could not inspect Ollama models: {error}"))?;
    let available_models = installed_model_names(&tags.models);
    let model_installed = is_model_installed(&tags.models, &model);
    let ready = request.enabled && model_installed;
    let message = if !request.enabled {
        "Local AI is off. Enable it before choosing Smart replacement.".to_string()
    } else if model_installed {
        "Local AI is ready. CSV values stay on this device and are sent only to Ollama on localhost."
            .to_string()
    } else {
        format!("{model} is not downloaded in Ollama yet.")
    };

    Ok(LocalAiStatus {
        enabled: request.enabled,
        provider: "ollama".to_string(),
        model,
        available_models,
        endpoint: endpoint.to_string(),
        runtime_available: true,
        model_installed,
        ready,
        runtime_version: version.version,
        message,
    })
}

fn ensure_runtime_available(endpoint: &str) -> Result<(), String> {
    let client = client()?;
    ollama_version(&client, endpoint)
        .map(|_| ())
        .map_err(|_| OLLAMA_UNAVAILABLE_MESSAGE.to_string())
}

fn ollama_version(client: &Client, endpoint: &str) -> Result<OllamaVersion, reqwest::Error> {
    client
        .get(format!("{endpoint}/api/version"))
        .send()
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.json::<OllamaVersion>())
}

fn installed_model_names(models: &[OllamaModel]) -> Vec<String> {
    let mut names = models
        .iter()
        .filter_map(|installed| {
            [installed.name.as_deref(), installed.model.as_deref()]
                .into_iter()
                .flatten()
                .find(|name| !name.trim().is_empty())
                .map(str::to_string)
        })
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    names
}

fn is_model_installed(models: &[OllamaModel], model: &str) -> bool {
    models.iter().any(|installed| {
        installed.name.as_deref() == Some(model) || installed.model.as_deref() == Some(model)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn installed_model_names_are_sorted_deduped_and_fallback_to_model() {
        let names = installed_model_names(&[
            OllamaModel {
                name: Some("llama3.2:latest".to_string()),
                model: Some("llama3.2:latest".to_string()),
            },
            OllamaModel {
                name: Some("".to_string()),
                model: Some("gemma3:4b".to_string()),
            },
            OllamaModel {
                name: Some("llama3.2:latest".to_string()),
                model: None,
            },
        ]);

        assert_eq!(names, vec!["gemma3:4b", "llama3.2:latest"]);
    }

    #[test]
    fn is_model_installed_checks_name_and_model_fields() {
        let models = [OllamaModel {
            name: Some("llama3.2".to_string()),
            model: Some("llama3.2:latest".to_string()),
        }];

        assert!(is_model_installed(&models, "llama3.2"));
        assert!(is_model_installed(&models, "llama3.2:latest"));
        assert!(!is_model_installed(&models, "gemma3:4b"));
    }

    #[test]
    fn local_ai_status_reports_friendly_message_when_ollama_is_unavailable() {
        let status = local_ai_status_with_endpoint(
            LocalAiRequest {
                enabled: true,
                model: "gemma3:4b".to_string(),
            },
            &unused_loopback_endpoint(),
        )
        .expect("local ai status should be returned even when ollama is unavailable");

        assert!(!status.runtime_available);
        assert!(!status.model_installed);
        assert!(!status.ready);
        assert_eq!(status.message, OLLAMA_UNAVAILABLE_MESSAGE);
    }

    #[test]
    fn runtime_preflight_returns_friendly_message_when_ollama_is_unavailable() {
        let error = ensure_runtime_available(&unused_loopback_endpoint())
            .expect_err("runtime preflight should fail without ollama");

        assert_eq!(error, OLLAMA_UNAVAILABLE_MESSAGE);
    }

    fn unused_loopback_endpoint() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("loopback port should bind");
        let address = listener
            .local_addr()
            .expect("loopback address should be available");
        drop(listener);
        format!("http://{address}")
    }
}
