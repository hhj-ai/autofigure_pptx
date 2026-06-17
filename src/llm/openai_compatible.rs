use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use base64::Engine;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

use crate::config::RoleConfig;
use crate::llm::{ChatProvider, ChatRequest};

pub struct OpenAiCompatibleProvider {
    config: RoleConfig,
    client: Client,
}

impl OpenAiCompatibleProvider {
    pub async fn complete_vision(
        config: RoleConfig,
        system_prompt: &str,
        user_text: &str,
        image_path: &Path,
    ) -> Result<String> {
        let api_key = config
            .api_key
            .as_ref()
            .ok_or_else(|| anyhow!("missing API key for OpenAI-compatible vision provider"))?;
        let model = config
            .model
            .as_ref()
            .ok_or_else(|| anyhow!("missing model for OpenAI-compatible vision provider"))?;
        let bytes = fs::read(image_path)
            .with_context(|| format!("failed to read review image {}", image_path.display()))?;
        let mime = mime_guess::from_path(image_path)
            .first_or_octet_stream()
            .to_string();
        let data_url = format!(
            "data:{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        );
        let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));
        let response = Client::new()
            .post(url)
            .bearer_auth(api_key)
            .json(&json!({
                "model": model,
                "messages": [
                    {"role": "system", "content": system_prompt},
                    {
                        "role": "user",
                        "content": [
                            {"type": "text", "text": user_text},
                            {"type": "image_url", "image_url": {"url": data_url}}
                        ]
                    }
                ],
                "temperature": 0.0,
                "stream": false
            }))
            .send()
            .await
            .context("OpenAI-compatible vision request failed")?
            .error_for_status()
            .context("OpenAI-compatible vision request returned error status")?
            .json::<ChatCompletionResponse>()
            .await
            .context("OpenAI-compatible vision response was not valid JSON")?;

        response
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message.content)
            .ok_or_else(|| anyhow!("OpenAI-compatible vision response had no choices"))
    }
}

impl OpenAiCompatibleProvider {
    pub fn new(config: RoleConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }
}

#[async_trait]
impl ChatProvider for OpenAiCompatibleProvider {
    async fn complete(&self, request: ChatRequest) -> Result<String> {
        let api_key = self
            .config
            .api_key
            .as_ref()
            .ok_or_else(|| anyhow!("missing API key for OpenAI-compatible provider"))?;
        let model = self
            .config
            .model
            .as_ref()
            .ok_or_else(|| anyhow!("missing model for OpenAI-compatible provider"))?;
        let url = format!(
            "{}/chat/completions",
            self.config.base_url.trim_end_matches('/')
        );
        let response = self
            .client
            .post(url)
            .bearer_auth(api_key)
            .json(&json!({
                "model": model,
                "messages": request.messages,
                "temperature": request.temperature,
                "stream": false
            }))
            .send()
            .await
            .context("OpenAI-compatible request failed")?
            .error_for_status()
            .context("OpenAI-compatible request returned error status")?
            .json::<ChatCompletionResponse>()
            .await
            .context("OpenAI-compatible response was not valid JSON")?;

        response
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message.content)
            .ok_or_else(|| anyhow!("OpenAI-compatible response had no choices"))
    }
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: AssistantMessage,
}

#[derive(Debug, Deserialize)]
struct AssistantMessage {
    #[serde(default)]
    content: String,
}
