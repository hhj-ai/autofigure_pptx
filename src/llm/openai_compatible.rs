use std::fs;
use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use base64::Engine;
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use serde_json::json;
use serde_json::Value;
use tokio::time::sleep;

use crate::config::RoleConfig;
use crate::llm::{ChatProvider, ChatRequest};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(180);
const MAX_REQUEST_ATTEMPTS: usize = 3;

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
        Self::complete_vision_with_images(config, system_prompt, user_text, &[image_path]).await
    }

    pub async fn complete_vision_with_images(
        config: RoleConfig,
        system_prompt: &str,
        user_text: &str,
        image_paths: &[&Path],
    ) -> Result<String> {
        let api_key = config
            .api_key
            .as_ref()
            .ok_or_else(|| anyhow!("missing API key for OpenAI-compatible vision provider"))?;
        let model = config
            .model
            .as_ref()
            .ok_or_else(|| anyhow!("missing model for OpenAI-compatible vision provider"))?;
        if image_paths.is_empty() {
            return Err(anyhow!("vision request requires at least one image"));
        }
        let mut content = vec![json!({"type": "text", "text": user_text})];
        for image_path in image_paths {
            let bytes = fs::read(image_path)
                .with_context(|| format!("failed to read review image {}", image_path.display()))?;
            let mime = mime_guess::from_path(image_path)
                .first_or_octet_stream()
                .to_string();
            let data_url = format!(
                "data:{mime};base64,{}",
                base64::engine::general_purpose::STANDARD.encode(bytes)
            );
            content.push(json!({"type": "image_url", "image_url": {"url": data_url}}));
        }
        let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));
        let body = json!({
                "model": model,
                "messages": [
                    {"role": "system", "content": system_prompt},
                    {
                        "role": "user",
                        "content": content
                    }
                ],
                "temperature": 0.0,
                "stream": false
        });
        let response = send_chat_completion_with_retries(
            &timeout_client(),
            &url,
            api_key,
            body,
            "OpenAI-compatible vision request",
        )
        .await?;

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
            client: timeout_client(),
        }
    }
}

fn timeout_client() -> Client {
    Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .unwrap_or_else(|_| Client::new())
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
        let body = json!({
                "model": model,
                "messages": request.messages,
                "temperature": request.temperature,
                "stream": false
        });
        let response = send_chat_completion_with_retries(
            &self.client,
            &url,
            api_key,
            body,
            "OpenAI-compatible request",
        )
        .await?;

        response
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message.content)
            .ok_or_else(|| anyhow!("OpenAI-compatible response had no choices"))
    }
}

async fn send_chat_completion_with_retries(
    client: &Client,
    url: &str,
    api_key: &str,
    body: Value,
    context: &str,
) -> Result<ChatCompletionResponse> {
    let mut last_error: Option<anyhow::Error> = None;
    for attempt in 0..MAX_REQUEST_ATTEMPTS {
        let response = match client
            .post(url)
            .bearer_auth(api_key)
            .json(&body)
            .send()
            .await
        {
            Ok(response) => response,
            Err(error) => {
                last_error = Some(anyhow!(error).context(format!("{context} failed")));
                if attempt + 1 < MAX_REQUEST_ATTEMPTS {
                    sleep(retry_delay(attempt)).await;
                    continue;
                }
                break;
            }
        };

        let status = response.status();
        if status.is_success() {
            return response
                .json::<ChatCompletionResponse>()
                .await
                .with_context(|| format!("{context} response was not valid JSON"));
        }

        let body_text = response.text().await.unwrap_or_default();
        let error = anyhow!(
            "{context} returned error status {status}: {}",
            truncate_for_error(body_text)
        );
        if is_retryable_status(status) && attempt + 1 < MAX_REQUEST_ATTEMPTS {
            last_error = Some(error);
            sleep(retry_delay(attempt)).await;
            continue;
        }
        return Err(error);
    }

    Err(last_error.unwrap_or_else(|| anyhow!("{context} failed without response")))
}

fn is_retryable_status(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

fn retry_delay(attempt: usize) -> Duration {
    #[cfg(test)]
    {
        let _ = attempt;
        Duration::from_millis(1)
    }
    #[cfg(not(test))]
    {
        Duration::from_millis(400 * (attempt as u64 + 1))
    }
}

fn truncate_for_error(value: String) -> String {
    let value = value.trim();
    const LIMIT: usize = 240;
    if value.len() <= LIMIT {
        value.to_string()
    } else {
        format!("{}...", &value[..LIMIT])
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
