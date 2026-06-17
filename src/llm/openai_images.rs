use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use base64::Engine;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

use crate::config::ImageConfig;
use crate::llm::{GeneratedImage, ImageProvider, ImageRequest};

pub struct OpenAiImagesProvider {
    config: ImageConfig,
    client: Client,
}

impl OpenAiImagesProvider {
    pub fn new(config: ImageConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }
}

#[async_trait]
impl ImageProvider for OpenAiImagesProvider {
    async fn generate(&self, request: ImageRequest) -> Result<GeneratedImage> {
        let api_key = self
            .config
            .api_key
            .as_ref()
            .ok_or_else(|| anyhow!("missing OpenAI Images API key"))?;
        let model = self
            .config
            .model
            .as_ref()
            .ok_or_else(|| anyhow!("missing OpenAI Images model"))?;
        let url = format!(
            "{}/images/generations",
            self.config.base_url.trim_end_matches('/')
        );
        let response = self
            .client
            .post(url)
            .bearer_auth(api_key)
            .json(&json!({
                "model": model,
                "prompt": request.prompt,
                "size": "1024x1024",
                "response_format": "b64_json"
            }))
            .send()
            .await
            .context("OpenAI Images request failed")?
            .error_for_status()
            .context("OpenAI Images request returned error status")?
            .json::<OpenAiImageResponse>()
            .await
            .context("OpenAI Images response was not valid JSON")?;

        let b64 = response
            .data
            .first()
            .and_then(|item| item.b64_json.as_deref())
            .ok_or_else(|| anyhow!("OpenAI Images response had no b64_json"))?;
        Ok(GeneratedImage {
            mime_type: "image/png".to_string(),
            bytes: base64::engine::general_purpose::STANDARD.decode(b64)?,
        })
    }
}

#[derive(Debug, Deserialize)]
struct OpenAiImageResponse {
    data: Vec<OpenAiImageItem>,
}

#[derive(Debug, Deserialize)]
struct OpenAiImageItem {
    b64_json: Option<String>,
}
