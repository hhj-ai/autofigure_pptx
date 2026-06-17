use anyhow::Result;
use async_trait::async_trait;
use base64::Engine;

use crate::llm::{ChatProvider, ChatRequest, GeneratedImage, ImageProvider, ImageRequest};

pub struct MockChatProvider;

#[async_trait]
impl ChatProvider for MockChatProvider {
    async fn complete(&self, _request: ChatRequest) -> Result<String> {
        Ok("{}".to_string())
    }
}

pub struct MockImageProvider;

#[async_trait]
impl ImageProvider for MockImageProvider {
    async fn generate(&self, _request: ImageRequest) -> Result<GeneratedImage> {
        let bytes = base64::engine::general_purpose::STANDARD.decode(
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGPQz5//HwAESwI9NWkaoQAAAABJRU5ErkJggg==",
        )?;
        Ok(GeneratedImage {
            mime_type: "image/png".to_string(),
            bytes,
        })
    }
}
