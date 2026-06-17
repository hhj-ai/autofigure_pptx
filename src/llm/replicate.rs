use anyhow::{anyhow, Result};
use async_trait::async_trait;

use crate::config::ImageConfig;
use crate::llm::{GeneratedImage, ImageProvider, ImageRequest};

pub struct ReplicateImageProvider {
    #[allow(dead_code)]
    config: ImageConfig,
}

impl ReplicateImageProvider {
    pub fn new(config: ImageConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl ImageProvider for ReplicateImageProvider {
    async fn generate(&self, _request: ImageRequest) -> Result<GeneratedImage> {
        Err(anyhow!(
            "Replicate image provider is declared for CLI compatibility but not implemented in this MVP"
        ))
    }
}
