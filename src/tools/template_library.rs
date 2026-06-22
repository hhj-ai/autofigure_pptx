use anyhow::{anyhow, Context, Result};

use crate::schema::{ReferencePreviewMode, ReferenceSelection};

const METHOD_TEMPLATE_PACK: &str =
    include_str!("../../templates/method_overview/method_templates.json");
const REFERENCE_FIGURE_PACK: &str =
    include_str!("../../templates/method_overview/reference_figures.json");

pub fn method_template_pack_json() -> Result<String> {
    let value: serde_json::Value = serde_json::from_str(METHOD_TEMPLATE_PACK)
        .context("method template pack is invalid JSON")?;
    Ok(serde_json::to_string_pretty(&value)?)
}

pub fn reference_pack_json() -> Result<String> {
    let value: serde_json::Value = serde_json::from_str(REFERENCE_FIGURE_PACK)
        .context("reference figure pack is invalid JSON")?;
    Ok(serde_json::to_string_pretty(&value)?)
}

pub fn selected_reference_json(selection: &ReferenceSelection) -> Result<String> {
    Ok(serde_json::to_string_pretty(selection)?)
}

pub fn complete_reference_selection_from_pack(
    selection: &mut ReferenceSelection,
    preview_mode: ReferencePreviewMode,
) -> Result<()> {
    let value: serde_json::Value = serde_json::from_str(REFERENCE_FIGURE_PACK)
        .context("reference figure pack is invalid JSON")?;
    let references = value["references"]
        .as_array()
        .ok_or_else(|| anyhow!("reference figure pack has no references array"))?;
    let Some(entry) = references
        .iter()
        .find(|entry| entry["id"].as_str() == Some(selection.selected_reference_id.as_str()))
    else {
        return Ok(());
    };
    let canonical = reference_selection_from_entry(entry, &selection.why_fit, preview_mode)?;
    selection.preview_mode = preview_mode;
    if selection.selected_reference_name.trim().is_empty() {
        selection.selected_reference_name = canonical.selected_reference_name;
    }
    if selection.source_paper.trim().is_empty() {
        selection.source_paper = canonical.source_paper;
    }
    if selection.source_url.trim().is_empty() {
        selection.source_url = canonical.source_url;
    }
    if selection
        .preview_path
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        selection.preview_path = canonical.preview_path;
    }
    if selection.adaptation_rules.is_empty() {
        selection.adaptation_rules = canonical.adaptation_rules;
    }
    if selection.anti_patterns.is_empty() {
        selection.anti_patterns = canonical.anti_patterns;
    }
    if selection.quality_targets.is_empty() {
        selection.quality_targets = canonical.quality_targets;
    }
    Ok(())
}

pub fn select_reference_for_method(
    method: &str,
    preview_mode: ReferencePreviewMode,
) -> Result<ReferenceSelection> {
    let value: serde_json::Value = serde_json::from_str(REFERENCE_FIGURE_PACK)
        .context("reference figure pack is invalid JSON")?;
    let references = value["references"]
        .as_array()
        .ok_or_else(|| anyhow!("reference figure pack has no references array"))?;
    let lower_method = method.to_lowercase();
    let mut iter = references.iter();
    let mut selected = iter
        .next()
        .ok_or_else(|| anyhow!("reference figure pack is empty"))?;
    let mut selected_score = reference_score(selected, &lower_method);
    for entry in iter {
        let score = reference_score(entry, &lower_method);
        if score > selected_score {
            selected = entry;
            selected_score = score;
        }
    }
    reference_selection_from_entry(selected, method, preview_mode)
}

fn reference_score(entry: &serde_json::Value, lower_method: &str) -> usize {
    let method_words = lower_method
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    entry["selection_tags"]
        .as_array()
        .map(|tags| {
            tags.iter()
                .filter_map(|tag| tag.as_str())
                .map(|tag| tag.to_lowercase())
                .map(|tag| tag_match_score(&tag, lower_method, &method_words))
                .sum()
        })
        .unwrap_or(0)
}

fn tag_match_score(tag: &str, lower_method: &str, method_words: &[&str]) -> usize {
    if lower_method.contains(tag) {
        return tag.split_whitespace().count().max(1) * 2;
    }
    let tag_words = tag
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    if tag_words.is_empty() {
        return 0;
    }
    if tag_words.iter().all(|tag_word| {
        method_words.iter().any(|method_word| {
            method_word.starts_with(tag_word) || tag_word.starts_with(method_word)
        })
    }) {
        tag_words.len()
    } else {
        0
    }
}

fn reference_selection_from_entry(
    entry: &serde_json::Value,
    method: &str,
    preview_mode: ReferencePreviewMode,
) -> Result<ReferenceSelection> {
    let id = required_str(entry, "id")?;
    let source = &entry["source"];
    let source_url = source["pdf_url"]
        .as_str()
        .or_else(|| source["award_url"].as_str())
        .unwrap_or_default()
        .to_string();
    Ok(ReferenceSelection {
        version: "0.1".to_string(),
        selected_reference_id: id.to_string(),
        selected_reference_name: required_str(entry, "name")?.to_string(),
        source_paper: source["paper"].as_str().unwrap_or_default().to_string(),
        source_url,
        preview_path: entry["preview"]["local_path"].as_str().map(str::to_string),
        preview_mode,
        why_fit: format!(
            "Selected by keyword overlap with method: {}",
            summarize_for_selection(method)
        ),
        adaptation_rules: string_array(entry, "style_grammar"),
        anti_patterns: string_array(entry, "anti_patterns"),
        quality_targets: string_array(entry, "quality_rubric"),
    })
}

fn required_str<'a>(entry: &'a serde_json::Value, key: &str) -> Result<&'a str> {
    entry[key]
        .as_str()
        .ok_or_else(|| anyhow!("reference entry missing string field {key}"))
}

fn string_array(entry: &serde_json::Value, key: &str) -> Vec<String> {
    entry[key]
        .as_array()
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn summarize_for_selection(method: &str) -> String {
    let compact = method.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() > 100 {
        compact.chars().take(97).collect::<String>() + "..."
    } else {
        compact
    }
}
