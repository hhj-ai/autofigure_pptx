use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::tools::render::scan_generated_typescript;
use crate::tools::workspace::AgentWorkspace;

pub const ENTRYPOINT_WORKSPACE_PATH: &str = "writable/code/figure.ts";
pub const HELPER_WORKSPACE_PATH: &str = "writable/code/helpers.ts";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GeneratedCodeBundle {
    pub files: Vec<GeneratedCodeFile>,
    #[serde(default)]
    pub notes: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GeneratedCodeFile {
    pub path: String,
    pub content: String,
}

#[derive(Clone, Debug)]
pub struct WrittenCodeBundle {
    pub entrypoint_code: String,
    pub entrypoint_path: PathBuf,
}

impl GeneratedCodeBundle {
    pub fn single_figure(content: String, notes: impl Into<String>) -> Self {
        Self {
            files: vec![GeneratedCodeFile {
                path: ENTRYPOINT_WORKSPACE_PATH.to_string(),
                content,
            }],
            notes: notes.into(),
        }
    }

    pub fn schema_json() -> Result<String> {
        Ok(serde_json::to_string_pretty(&schemars::schema_for!(
            GeneratedCodeBundle
        ))?)
    }
}

pub fn write_generated_code_bundle(
    workspace: &AgentWorkspace,
    round_dir: &Path,
    bundle: &GeneratedCodeBundle,
) -> Result<WrittenCodeBundle> {
    let files = normalize_and_validate_files(bundle)?;
    fs::create_dir_all(round_dir)?;

    let mut entrypoint_code = None;
    for file in files {
        scan_generated_typescript(&file.content)?;
        workspace.write_declared(&file.path, file.content.as_bytes())?;
        let local_name = local_code_filename(&file.path)?;
        fs::write(round_dir.join(local_name), file.content.as_bytes())
            .with_context(|| format!("failed to write generated code {}", file.path))?;
        if file.path == ENTRYPOINT_WORKSPACE_PATH {
            entrypoint_code = Some(file.content);
        }
    }

    let entrypoint_code = entrypoint_code
        .ok_or_else(|| anyhow!("GeneratedCodeBundle must include {ENTRYPOINT_WORKSPACE_PATH}"))?;
    Ok(WrittenCodeBundle {
        entrypoint_code,
        entrypoint_path: round_dir.join("figure.ts"),
    })
}

pub fn normalize_generated_code_bundle(
    bundle: &GeneratedCodeBundle,
) -> Result<GeneratedCodeBundle> {
    let files = normalize_and_validate_files(bundle)?;
    Ok(GeneratedCodeBundle {
        files,
        notes: bundle.notes.clone(),
    })
}

fn normalize_and_validate_files(bundle: &GeneratedCodeBundle) -> Result<Vec<GeneratedCodeFile>> {
    if bundle.files.is_empty() {
        return Err(anyhow!("GeneratedCodeBundle.files must not be empty"));
    }

    let mut normalized = Vec::with_capacity(bundle.files.len());
    let mut saw_entrypoint = false;
    for file in &bundle.files {
        let path = normalize_code_path(&file.path)?;
        if path == ENTRYPOINT_WORKSPACE_PATH {
            saw_entrypoint = true;
        }
        if normalized
            .iter()
            .any(|existing: &GeneratedCodeFile| existing.path == path)
        {
            return Err(anyhow!("duplicate generated code path: {path}"));
        }
        if file.content.trim().is_empty() {
            return Err(anyhow!("generated code file is empty: {path}"));
        }
        normalized.push(GeneratedCodeFile {
            path,
            content: file.content.clone(),
        });
    }

    if !saw_entrypoint {
        return Err(anyhow!(
            "GeneratedCodeBundle must include {ENTRYPOINT_WORKSPACE_PATH}"
        ));
    }
    Ok(normalized)
}

fn normalize_code_path(raw_path: &str) -> Result<String> {
    let path = raw_path.trim().trim_start_matches("./");
    let path = if path.starts_with("writable/code/") {
        path.to_string()
    } else if path.starts_with("code/") {
        format!("writable/{path}")
    } else if !path.contains('/') {
        format!("writable/code/{path}")
    } else {
        return Err(anyhow!("unsafe generated code path: {raw_path}"));
    };

    let local_name = local_code_filename(&path)?;
    if !matches!(
        path.as_str(),
        ENTRYPOINT_WORKSPACE_PATH | HELPER_WORKSPACE_PATH
    ) {
        return Err(anyhow!(
            "generated code path is not declared in the shared workspace manifest: {path}"
        ));
    }
    if local_name == ".env" || !local_name.ends_with(".ts") {
        return Err(anyhow!("unsafe generated code path: {raw_path}"));
    }
    Ok(path)
}

fn local_code_filename(workspace_path: &str) -> Result<&str> {
    let Some(local_name) = workspace_path.strip_prefix("writable/code/") else {
        return Err(anyhow!("unsafe generated code path: {workspace_path}"));
    };
    if local_name.is_empty()
        || local_name.contains('/')
        || local_name.contains('\\')
        || local_name.contains("..")
    {
        return Err(anyhow!("unsafe generated code path: {workspace_path}"));
    }
    Ok(local_name)
}
