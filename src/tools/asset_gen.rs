use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use sha2::{Digest, Sha256};

use crate::config::AppConfig;
use crate::llm::mock::MockImageProvider;
use crate::llm::openai_images::OpenAiImagesProvider;
use crate::llm::openrouter_image::OpenRouterImageProvider;
use crate::llm::replicate::ReplicateImageProvider;
use crate::llm::{ImageProvider, ImageRequest};
use crate::schema::{AssetSpec, FigurePlan, ImageProviderKind, StyleName};

pub fn asset_cache_key(spec: &AssetSpec) -> Result<String> {
    let payload = serde_json::to_vec(spec)?;
    let mut hasher = Sha256::new();
    hasher.update(payload);
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn build_asset_prompt(spec: &AssetSpec, style: StyleName) -> String {
    format!(
        "Generate a small local asset for a paper method figure: {}. \
minimal flat vector-style pictogram, no text, no letters, no numbers, \
no watermark, no signature, transparent background if possible, \
simple silhouette, paper figure style, clean geometry, limited palette matching {:?}, \
high contrast, centered object. This is not a full pipeline diagram and must not contain arrows carrying meaning.",
        spec.prompt, style
    )
}

pub fn materialize_assets(
    plan: &FigurePlan,
    run_dir: &Path,
    round_dir: &Path,
    provider_kind: ImageProviderKind,
    config: &AppConfig,
    mock_models: bool,
) -> Result<BTreeMap<String, PathBuf>> {
    let mut paths = BTreeMap::new();
    if matches!(provider_kind, ImageProviderKind::None) {
        return Ok(paths);
    }

    let cache_dir = run_dir.join("asset_cache");
    let round_asset_dir = round_dir.join("assets");
    fs::create_dir_all(&cache_dir)?;
    fs::create_dir_all(&round_asset_dir)?;

    for spec in &plan.assets {
        let hash = asset_cache_key(spec)?;
        let cache_path = cache_dir.join(format!("{hash}.png"));
        let round_path = round_asset_dir.join(format!("{}.png", spec.id));
        if !cache_path.exists() {
            let image =
                generate_asset_bytes(spec, plan.design.style, provider_kind, config, mock_models)
                    .with_context(|| format!("failed to generate asset {}", spec.id))?;
            fs::write(&cache_path, image)?;
        }
        fs::copy(&cache_path, &round_path)?;
        paths.insert(
            spec.id.clone(),
            round_path
                .canonicalize()
                .context("failed to canonicalize generated asset path")?,
        );
    }

    Ok(paths)
}

fn generate_asset_bytes(
    spec: &AssetSpec,
    style: StyleName,
    provider_kind: ImageProviderKind,
    config: &AppConfig,
    mock_models: bool,
) -> Result<Vec<u8>> {
    let request = ImageRequest {
        prompt: build_asset_prompt(spec, style),
        aspect_ratio: "1:1".to_string(),
        image_size: "1K".to_string(),
    };
    if mock_models {
        return block_on(MockImageProvider.generate(request)).map(|image| image.bytes);
    }

    if !config.image.is_configured() {
        return Err(anyhow!("image model is not configured; set METHODFIG_IMAGE_API_KEY and METHODFIG_IMAGE_MODEL, choose --image-provider none, or use --mock-models"));
    }

    match provider_kind {
        ImageProviderKind::OpenRouter => {
            block_on(OpenRouterImageProvider::new(config.image.clone()).generate(request))
                .map(|image| image.bytes)
        }
        ImageProviderKind::OpenAiImages => {
            block_on(OpenAiImagesProvider::new(config.image.clone()).generate(request))
                .map(|image| image.bytes)
        }
        ImageProviderKind::Replicate => {
            block_on(ReplicateImageProvider::new(config.image.clone()).generate(request))
                .map(|image| image.bytes)
        }
        ImageProviderKind::None => Ok(vec![]),
    }
}

fn block_on<T>(future: impl std::future::Future<Output = Result<T>>) -> Result<T> {
    tokio::runtime::Runtime::new()
        .context("failed to create Tokio runtime for image generation")?
        .block_on(future)
}
