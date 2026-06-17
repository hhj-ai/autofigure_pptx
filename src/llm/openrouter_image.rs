use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use base64::Engine;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::config::ImageConfig;
use crate::llm::{GeneratedImage, ImageProvider, ImageRequest};

pub struct OpenRouterImageProvider {
    config: ImageConfig,
    client: Client,
}

impl OpenRouterImageProvider {
    pub fn new(config: ImageConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }
}

#[async_trait]
impl ImageProvider for OpenRouterImageProvider {
    async fn generate(&self, request: ImageRequest) -> Result<GeneratedImage> {
        let api_key = self
            .config
            .api_key
            .as_ref()
            .ok_or_else(|| anyhow!("missing OpenRouter image API key"))?;
        let model = self
            .config
            .model
            .as_ref()
            .ok_or_else(|| anyhow!("missing OpenRouter image model"))?;
        let url = format!(
            "{}/chat/completions",
            self.config.base_url.trim_end_matches('/')
        );
        let response = self
            .client
            .post(url)
            .bearer_auth(api_key)
            .json(&build_openrouter_image_payload(
                model,
                &request,
                &self.config.modalities,
            ))
            .send()
            .await
            .context("OpenRouter image request failed")?
            .error_for_status()
            .context("OpenRouter image request returned error status")?
            .json::<OpenRouterImageResponse>()
            .await
            .context("OpenRouter image response was not valid JSON")?;

        let image_url = response
            .choices
            .first()
            .and_then(|choice| choice.message.images.first())
            .map(|image| image.image_url.url.as_str())
            .ok_or_else(|| anyhow!("OpenRouter image response had no image_url"))?;

        decode_image_url(&self.client, image_url).await
    }
}

pub fn build_openrouter_image_payload(
    model: &str,
    request: &ImageRequest,
    modalities: &[String],
) -> Value {
    let modalities = if modalities.is_empty() {
        vec!["image".to_string()]
    } else {
        modalities.to_vec()
    };
    json!({
        "model": model,
        "messages": [{"role": "user", "content": request.prompt}],
        "modalities": modalities,
        "stream": false,
        "image_config": {
            "aspect_ratio": request.aspect_ratio,
            "image_size": request.image_size
        }
    })
}

async fn decode_image_url(client: &Client, image_url: &str) -> Result<GeneratedImage> {
    if let Some((meta, data)) = image_url.split_once(',') {
        if meta.starts_with("data:") {
            let mime_type = meta
                .strip_prefix("data:")
                .and_then(|value| value.split(';').next())
                .unwrap_or("image/png")
                .to_string();
            return Ok(GeneratedImage {
                mime_type,
                bytes: base64::engine::general_purpose::STANDARD.decode(data)?,
            });
        }
    }

    let bytes = client
        .get(image_url)
        .send()
        .await
        .context("failed to download OpenRouter image URL")?
        .error_for_status()
        .context("OpenRouter image URL returned error status")?
        .bytes()
        .await?
        .to_vec();
    Ok(GeneratedImage {
        mime_type: mime_guess::from_path(image_url)
            .first_or_octet_stream()
            .to_string(),
        bytes,
    })
}

#[derive(Debug, Deserialize)]
struct OpenRouterImageResponse {
    choices: Vec<ImageChoice>,
}

#[derive(Debug, Deserialize)]
struct ImageChoice {
    message: ImageMessage,
}

#[derive(Debug, Deserialize)]
struct ImageMessage {
    #[serde(default)]
    images: Vec<ImageItem>,
}

#[derive(Debug, Deserialize)]
struct ImageItem {
    image_url: ImageUrl,
}

#[derive(Debug, Deserialize)]
struct ImageUrl {
    url: String,
}
