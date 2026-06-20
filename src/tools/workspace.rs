use std::fs;
use std::path::{Component as PathComponent, Path, PathBuf};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceManifest {
    pub readable: Vec<WorkspaceFile>,
    pub writable: Vec<WorkspaceFile>,
}

impl WorkspaceManifest {
    pub fn validate(&self) -> Result<()> {
        for file in &self.readable {
            validate_manifest_path(&file.path, "readable")?;
        }
        for file in &self.writable {
            validate_manifest_path(&file.path, "writable")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceFile {
    pub path: String,
    pub purpose: String,
    pub format: WorkspaceFileFormat,
    pub max_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkspaceFileFormat {
    #[serde(rename = "markdown")]
    Markdown,
    #[serde(rename = "json")]
    Json,
    #[serde(rename = "png")]
    Png,
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "typescript")]
    Typescript,
}

#[derive(Clone, Debug)]
pub struct AgentWorkspace {
    root: PathBuf,
    manifest: WorkspaceManifest,
}

impl AgentWorkspace {
    pub fn create(round_dir: &Path, manifest: WorkspaceManifest) -> Result<Self> {
        manifest.validate()?;
        let root = round_dir.join("workspace");
        fs::create_dir_all(root.join("readable"))?;
        fs::create_dir_all(root.join("writable"))?;
        fs::write(
            root.join("manifest.json"),
            serde_json::to_vec_pretty(&manifest)?,
        )?;
        Ok(Self { root, manifest })
    }

    pub fn write_declared(&self, relative_path: &str, bytes: &[u8]) -> Result<()> {
        validate_manifest_path(relative_path, "writable")?;
        let Some(entry) = self
            .manifest
            .writable
            .iter()
            .find(|entry| entry.path == relative_path)
        else {
            return Err(anyhow!(
                "workspace path is not declared writable: {relative_path}"
            ));
        };
        if bytes.len() as u64 > entry.max_bytes {
            return Err(anyhow!(
                "workspace write exceeds max_bytes for {relative_path}: {} > {}",
                bytes.len(),
                entry.max_bytes
            ));
        }
        let target = self.root.join(relative_path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(target, bytes)?;
        Ok(())
    }

    pub fn write_readable(&self, relative_path: &str, bytes: &[u8]) -> Result<()> {
        validate_manifest_path(relative_path, "readable")?;
        let Some(entry) = self
            .manifest
            .readable
            .iter()
            .find(|entry| entry.path == relative_path)
        else {
            return Err(anyhow!(
                "workspace path is not declared readable: {relative_path}"
            ));
        };
        if bytes.len() as u64 > entry.max_bytes {
            return Err(anyhow!(
                "workspace readable file exceeds max_bytes for {relative_path}: {} > {}",
                bytes.len(),
                entry.max_bytes
            ));
        }
        let target = self.root.join(relative_path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(target, bytes)?;
        Ok(())
    }
}

fn validate_manifest_path(path: &str, required_root: &str) -> Result<()> {
    let path_ref = Path::new(path);
    if path_ref.is_absolute() {
        return Err(anyhow!("unsafe workspace path: {path}"));
    }

    let mut components = path_ref.components();
    let Some(first) = components.next() else {
        return Err(anyhow!("unsafe workspace path: {path}"));
    };
    if !matches!(first, PathComponent::Normal(value) if value == required_root) {
        return Err(anyhow!("unsafe workspace path: {path}"));
    }

    let mut saw_leaf = false;
    for component in components {
        match component {
            PathComponent::Normal(value) => {
                if value == ".env" {
                    return Err(anyhow!("unsafe workspace path: {path}"));
                }
                saw_leaf = true;
            }
            _ => return Err(anyhow!("unsafe workspace path: {path}")),
        }
    }
    if !saw_leaf {
        return Err(anyhow!("unsafe workspace path: {path}"));
    }
    Ok(())
}
