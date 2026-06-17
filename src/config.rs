use std::env;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub reasoner: RoleConfig,
    pub coder: RoleConfig,
    pub vision: RoleConfig,
    pub image: ImageConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoleConfig {
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: Option<String>,
}

impl RoleConfig {
    pub fn is_configured(&self) -> bool {
        self.api_key.as_ref().is_some_and(|value| !value.is_empty())
            && self.model.as_ref().is_some_and(|value| !value.is_empty())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageConfig {
    pub provider: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub modalities: Vec<String>,
}

impl ImageConfig {
    pub fn is_configured(&self) -> bool {
        self.api_key.as_ref().is_some_and(|value| !value.is_empty())
            && self.model.as_ref().is_some_and(|value| !value.is_empty())
    }
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();
        Ok(Self {
            reasoner: role_from_env("METHODFIG_REASONER", "https://api.openai.com/v1"),
            coder: role_from_env("METHODFIG_CODER", "https://api.openai.com/v1"),
            vision: role_from_env("METHODFIG_VISION", "https://api.openai.com/v1"),
            image: ImageConfig {
                provider: env::var("METHODFIG_IMAGE_PROVIDER")
                    .unwrap_or_else(|_| "openrouter".to_string()),
                base_url: env::var("METHODFIG_IMAGE_BASE_URL")
                    .unwrap_or_else(|_| "https://openrouter.ai/api/v1".to_string()),
                api_key: env::var("METHODFIG_IMAGE_API_KEY").ok(),
                model: env::var("METHODFIG_IMAGE_MODEL").ok(),
                modalities: parse_modalities(
                    &env::var("METHODFIG_IMAGE_MODALITIES").unwrap_or_else(|_| "image".to_string()),
                ),
            },
        })
    }
}

fn role_from_env(prefix: &str, default_base_url: &str) -> RoleConfig {
    RoleConfig {
        base_url: env::var(format!("{prefix}_BASE_URL"))
            .unwrap_or_else(|_| default_base_url.to_string()),
        api_key: env::var(format!("{prefix}_API_KEY")).ok(),
        model: env::var(format!("{prefix}_MODEL")).ok(),
    }
}

fn parse_modalities(value: &str) -> Vec<String> {
    let modalities = value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if modalities.is_empty() {
        vec!["image".to_string()]
    } else {
        modalities
    }
}
