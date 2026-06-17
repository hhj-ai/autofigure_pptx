use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    pub temperature: f32,
}

#[async_trait]
pub trait ChatProvider: Send + Sync {
    async fn complete(&self, request: ChatRequest) -> Result<String>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageRequest {
    pub prompt: String,
    pub aspect_ratio: String,
    pub image_size: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeneratedImage {
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

#[async_trait]
pub trait ImageProvider: Send + Sync {
    async fn generate(&self, request: ImageRequest) -> Result<GeneratedImage>;
}
