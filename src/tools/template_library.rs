use anyhow::{Context, Result};

const METHOD_TEMPLATE_PACK: &str =
    include_str!("../../templates/method_overview/method_templates.json");

pub fn method_template_pack_json() -> Result<String> {
    let value: serde_json::Value = serde_json::from_str(METHOD_TEMPLATE_PACK)
        .context("method template pack is invalid JSON")?;
    Ok(serde_json::to_string_pretty(&value)?)
}
