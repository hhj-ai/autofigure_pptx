use std::path::Path;

use anyhow::{anyhow, Context, Result};

use crate::config::AppConfig;
use crate::llm::openai_compatible::OpenAiCompatibleProvider;
use crate::llm::{ChatMessage, ChatProvider, ChatRequest};
use crate::prompts::{
    REASON_INITIAL_PLANNER, REASON_PATCH_PLANNER, TOP_TIER_FIGURE_DIRECTIVE, VISION_REVIEWER,
};
use crate::schema::{
    figure_plan_schema_json, patch_plan_schema_json, review_schema_json, CanvasAspect, FigurePlan,
    LayoutRegion, PatchOperation, PatchOperationType, PatchPlan, PatchStopReason, Review,
    StyleName, VisualWeight,
};
use crate::style::StyleSpec;
use crate::tools::pptx_codegen::generate_typescript;
use crate::tools::render::scan_generated_typescript;

pub fn validate_required_model_config(config: &AppConfig, mock_models: bool) -> Result<()> {
    if mock_models {
        return Ok(());
    }
    if !config.reasoner.is_configured() {
        return Err(anyhow!("reasoner model is not configured; set METHODFIG_REASONER_API_KEY and METHODFIG_REASONER_MODEL or use --mock-models"));
    }
    if !config.coder.is_configured() {
        return Err(anyhow!("coder model is not configured; set METHODFIG_CODER_API_KEY and METHODFIG_CODER_MODEL or use --mock-models"));
    }
    if !config.vision.is_configured() {
        return Err(anyhow!("vision model is not configured; set METHODFIG_VISION_API_KEY and METHODFIG_VISION_MODEL or use --mock-models"));
    }
    Ok(())
}

pub fn create_initial_plan(
    method: &str,
    style: StyleName,
    aspect: CanvasAspect,
    target_width_mm: u32,
    config: &AppConfig,
    mock_models: bool,
) -> Result<FigurePlan> {
    if mock_models {
        return Ok(FigurePlan::mock_from_method(
            method,
            style,
            aspect,
            target_width_mm,
        ));
    }
    let provider = OpenAiCompatibleProvider::new(config.reasoner.clone());
    let user_prompt = build_initial_plan_prompt(method, style, aspect, target_width_mm)?;
    let request = ChatRequest {
        temperature: 0.1,
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: format!("{REASON_INITIAL_PLANNER}\n\n{TOP_TIER_FIGURE_DIRECTIVE}"),
            },
            ChatMessage {
                role: "user".to_string(),
                content: user_prompt,
            },
        ],
    };
    let text = block_on(provider.complete(request))?;
    let plan: FigurePlan = serde_json::from_str(strip_json_fence(&text))
        .with_context(|| "reasoner did not return valid FigurePlan JSON")?;
    Ok(plan)
}

pub fn build_initial_plan_prompt(
    method: &str,
    style: StyleName,
    aspect: CanvasAspect,
    target_width_mm: u32,
) -> Result<String> {
    let schema = figure_plan_schema_json()?;
    Ok(format!(
        "Create exactly one FigurePlan object for the method below.\n\
Return JSON only: no markdown, no prose, no schema wrapper, no comments.\n\
Required top-level keys: version, canvas, story, layout, components, edges, annotations, assets, design.\n\
Every stable id must be globally unique across layout.regions, components, edges, annotations, and assets; do not reuse a component id as an edge or asset id.\n\
Set version to \"0.1\". Set canvas.aspect to \"{}\" and canvas.target_width_mm to {target_width_mm}.\n\
Use design.style \"{}\". Keep generated assets small and local only; semantic labels must remain editable PPTX text.\n\n\
{}\n\n\
FigurePlan JSON Schema:\n{schema}\n\n\
Method Markdown:\n{method}",
        aspect_json_name(aspect),
        style_json_name(style),
        TOP_TIER_FIGURE_DIRECTIVE
    ))
}

pub fn create_typescript_code(
    plan: &FigurePlan,
    style: &StyleSpec,
    round_dir: &Path,
    renderer_root: &Path,
    asset_paths: &std::collections::BTreeMap<String, std::path::PathBuf>,
    _config: &AppConfig,
    mock_models: bool,
) -> Result<String> {
    let deterministic = generate_typescript(plan, style, round_dir, renderer_root, asset_paths)?;
    if mock_models {
        return Ok(deterministic);
    }
    scan_generated_typescript(&deterministic)?;
    Ok(deterministic)
}

pub fn review_rendered_figure(
    plan: &FigurePlan,
    round_dir: &Path,
    config: &AppConfig,
    mock_models: bool,
    round_index: u32,
) -> Result<Review> {
    if mock_models {
        return Ok(crate::tools::review::mock_review(round_index));
    }
    if !config.vision.is_configured() {
        return Err(anyhow!("vision model is not configured; set METHODFIG_VISION_API_KEY and METHODFIG_VISION_MODEL or use --mock-models"));
    }
    let plan_json = serde_json::to_string_pretty(plan)?;
    let layout_map = std::fs::read_to_string(round_dir.join("layout_map.json"))
        .unwrap_or_else(|_| "{}".to_string());
    let user_text = build_review_prompt(&plan_json, &layout_map)?;
    let image_path = round_dir.join(format!(
        "figure_{}mm_preview.png",
        plan.canvas.target_width_mm
    ));
    let text = block_on(OpenAiCompatibleProvider::complete_vision(
        config.vision.clone(),
        VISION_REVIEWER,
        &user_text,
        &image_path,
    ))?;
    let review = parse_review_text(&text).or_else(|first_error| {
        let retry_user_text = build_review_retry_prompt(&plan_json, &layout_map)?;
        let retry_text = block_on(OpenAiCompatibleProvider::complete_vision(
            config.vision.clone(),
            VISION_REVIEWER,
            &retry_user_text,
            &image_path,
        ))?;
        parse_review_text(&retry_text).with_context(|| {
            format!(
                "vision model did not return valid Review JSON after retry; first parse error: {first_error}"
            )
        })
    })?;
    Ok(review)
}

pub fn build_review_prompt(plan_json: &str, layout_map: &str) -> Result<String> {
    let schema = review_schema_json()?;
    Ok(format!(
        "Review this paper method figure. Return exactly one Review object.\n\
Return JSON only: no markdown, no prose, no comments.\n\
Scores must include every field from the schema, including semantic_fidelity and wps_editability, each as an integer from 1 to 10.\n\
Keep string values short and do not embed quotation marks inside string content.\n\n\
Review JSON Schema:\n{schema}\n\n\
FigurePlan:\n{plan_json}\n\n\
layout_map.json:\n{layout_map}"
    ))
}

pub fn build_review_retry_prompt(plan_json: &str, layout_map: &str) -> Result<String> {
    let schema = review_schema_json()?;
    Ok(format!(
        "The previous answer was not valid JSON. Return the same Review object again.\n\
Return strict JSON only: no markdown, no prose, no comments, and no code fences.\n\
Keep each string value short and avoid embedded quotation marks inside the text.\n\
Scores must include every field from the schema, including semantic_fidelity and wps_editability, each as an integer from 1 to 10.\n\n\
Review JSON Schema:\n{schema}\n\n\
FigurePlan:\n{plan_json}\n\n\
layout_map.json:\n{layout_map}"
    ))
}

pub fn create_patch_plan(
    plan: &FigurePlan,
    review: &Review,
    config: &AppConfig,
    mock_models: bool,
) -> Result<PatchPlan> {
    if mock_models {
        return Ok(crate::tools::review::mock_patch_plan());
    }
    if !config.reasoner.is_configured() {
        return Err(anyhow!(
            "reasoner model is not configured for patch planning"
        ));
    }
    let provider = OpenAiCompatibleProvider::new(config.reasoner.clone());
    let request = ChatRequest {
        temperature: 0.1,
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: format!("{REASON_PATCH_PLANNER}\n\n{TOP_TIER_FIGURE_DIRECTIVE}"),
            },
            ChatMessage {
                role: "user".to_string(),
                content: build_patch_prompt(
                    &serde_json::to_string_pretty(plan)?,
                    &serde_json::to_string_pretty(review)?,
                )?,
            },
        ],
    };
    let text = block_on(provider.complete(request))?;
    let patch = parse_patch_plan_text(&text)
        .with_context(|| "reasoner did not return valid PatchPlan JSON")?;
    if patch_plan_has_unexecutable_layout_patch(&patch) {
        let retry_request = ChatRequest {
            temperature: 0.0,
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: format!(
                        "{REASON_PATCH_PLANNER}\n\n{TOP_TIER_FIGURE_DIRECTIVE}"
                    ),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: format!(
                        "{}\n\nThe previous PatchPlan was rejected because at least one layout_patch did not include final bbox coordinates. Return a corrected PatchPlan JSON only. Every layout_patch action must include final bbox [x1,y1,x2,y2] arrays next to the exact target id.\n\nRejected PatchPlan:\n{}",
                        build_patch_prompt(
                            &serde_json::to_string_pretty(plan)?,
                            &serde_json::to_string_pretty(review)?,
                        )?,
                        serde_json::to_string_pretty(&patch)?
                    ),
                },
            ],
        };
        let retry_text = block_on(provider.complete(retry_request))?;
        let retry_patch = parse_patch_plan_text(&retry_text)
            .with_context(|| "reasoner retry did not return valid PatchPlan JSON")?;
        return Ok(retry_patch);
    }
    Ok(patch)
}

pub fn parse_patch_plan_text(text: &str) -> Result<PatchPlan> {
    let mut value: serde_json::Value = serde_json::from_str(strip_json_fence(text))?;
    if value.get("stop_reason").is_none() {
        value["stop_reason"] = serde_json::json!(PatchStopReason::Continue);
    }
    Ok(serde_json::from_value(value)?)
}

pub fn patch_plan_has_unexecutable_layout_patch(patch: &PatchPlan) -> bool {
    patch.operations.iter().any(|operation| {
        matches!(operation.operation_type, PatchOperationType::LayoutPatch)
            && find_patch_bbox_literal(&operation.action).is_none()
    })
}

pub fn build_patch_prompt(plan_json: &str, review_json: &str) -> Result<String> {
    let schema = patch_plan_schema_json()?;
    Ok(format!(
        "Create a repair plan for the reviewed figure. Return exactly one PatchPlan object.\n\
Return JSON only: no markdown, no prose, no comments.\n\
Required top-level keys: operations, stop_reason. Use operations as an array, even when empty.\n\
Prefer layout, style, routing, and concise text edits before regenerating assets.\n\
Every layout_patch must be executable: include the exact stable target id and the final bbox [x1,y1,x2,y2] for each changed region, annotation, or component-created region.\n\
For layout patches that affect multiple regions, write each id next to its final bbox, for example r_student to [0.20,0.18,0.55,0.62], r_loss to [0.55,0.62,0.78,0.86].\n\
Do not use only relative instructions such as \"move up\", \"start at y=0.66\", or \"shrink y_max\" without final bbox arrays. Operations without executable coordinates are ignored by the local patch executor.\n\
For text_patch, quote the exact replacement label. For edge_reroute, keep the target edge id exact and specify solid/dash/long_dash when style must change.\n\n\
{}\n\n\
PatchPlan JSON Schema:\n{schema}\n\n\
Previous FigurePlan:\n{plan_json}\n\n\
Review:\n{review_json}"
        ,
        TOP_TIER_FIGURE_DIRECTIVE
    ))
}

pub fn apply_patch_plan_to_figure(plan: &mut FigurePlan, patch: &PatchPlan) {
    for operation in &patch.operations {
        match operation.operation_type {
            PatchOperationType::LayoutPatch => {
                apply_layout_patch(plan, operation);
            }
            PatchOperationType::StylePatch => {
                apply_style_patch(plan, operation);
            }
            PatchOperationType::TextPatch => {
                apply_text_patch(plan, operation);
            }
            PatchOperationType::EdgeReroute => {
                apply_edge_patch(plan, operation);
            }
            PatchOperationType::AssetRegeneration => {}
        }
    }

    for component in &mut plan.components {
        let role_is_main = matches!(component.role, crate::schema::ComponentRole::Main);
        if role_is_main {
            component.visual_weight = VisualWeight::Strong;
            if !component.label.contains("Core") && component.label == "Method" {
                component.label = "Method Core".to_string();
            }
        }
    }

    let focus = "main contribution path emphasized".to_string();
    if !plan.story.visual_focus.contains(&focus) {
        plan.story.visual_focus.push(focus);
    }
}

fn apply_layout_patch(plan: &mut FigurePlan, operation: &PatchOperation) {
    if let Some(annotation) = plan
        .annotations
        .iter_mut()
        .find(|annotation| annotation.id == operation.target_id)
    {
        if let Some(bbox) = find_patch_bbox_literal(&operation.action) {
            annotation.bbox = Some(bbox);
        }
        if operation
            .action
            .to_lowercase()
            .contains("target_id to null")
        {
            annotation.target_id = None;
        }
        return;
    }

    if let Some(region) = plan
        .layout
        .regions
        .iter_mut()
        .find(|region| region.id == operation.target_id)
    {
        if let Some(bbox) = find_patch_bbox_literal(&operation.action) {
            region.bbox = bbox;
        }
    }

    for region in &mut plan.layout.regions {
        if let Some(bbox) = find_bbox_after_label(&operation.action, &region.id) {
            region.bbox = bbox;
        }
    }

    let mut component_region_updates = Vec::new();
    for component in &plan.components {
        if component.id == operation.target_id {
            if let Some(bbox) = find_patch_bbox_literal(&operation.action) {
                component_region_updates.push((
                    component.id.clone(),
                    component_region_id(&operation.action, &component.id),
                    bbox,
                ));
            }
            continue;
        }

        if let Some(bbox) = find_bbox_after_standalone_label(&operation.action, &component.id) {
            component_region_updates.push((
                component.id.clone(),
                component_region_id(&operation.action, &component.id),
                bbox,
            ));
        }
    }

    for (component_id, region_id, bbox) in component_region_updates {
        upsert_region(plan, &region_id, bbox);
        if let Some(component) = plan
            .components
            .iter_mut()
            .find(|component| component.id == component_id)
        {
            component.region = region_id;
        }
    }
}

fn apply_style_patch(plan: &mut FigurePlan, operation: &PatchOperation) {
    let action = operation.action.to_lowercase();
    if let Some(component) = plan
        .components
        .iter_mut()
        .find(|component| component.id == operation.target_id)
    {
        if action.contains("strong") || action.contains("emphasis") || action.contains("main") {
            component.visual_weight = VisualWeight::Strong;
        } else if action.contains("muted") || action.contains("de-emphasis") {
            component.visual_weight = VisualWeight::Muted;
        } else if action.contains("normal") {
            component.visual_weight = VisualWeight::Normal;
        }
    }
}

fn apply_text_patch(plan: &mut FigurePlan, operation: &PatchOperation) {
    let Some(label) = find_quoted_value(&operation.action) else {
        return;
    };
    if let Some(edge) = plan
        .edges
        .iter_mut()
        .find(|edge| edge.id == operation.target_id)
    {
        edge.label = label;
        return;
    }
    if let Some(component) = plan
        .components
        .iter_mut()
        .find(|component| component.id == operation.target_id)
    {
        component.label = label;
        return;
    }
    if let Some(annotation) = plan
        .annotations
        .iter_mut()
        .find(|annotation| annotation.id == operation.target_id)
    {
        annotation.label = label;
    }
}

fn apply_edge_patch(plan: &mut FigurePlan, operation: &PatchOperation) {
    let Some(edge) = plan
        .edges
        .iter_mut()
        .find(|edge| edge.id == operation.target_id)
    else {
        return;
    };
    let action = operation.action.to_lowercase();
    if action.contains("long_dash") || action.contains("long dash") {
        edge.style = crate::schema::EdgeStyle::LongDash;
    } else if action.contains("dash") || action.contains("dashed") {
        edge.style = crate::schema::EdgeStyle::Dash;
    } else if action.contains("solid") {
        edge.style = crate::schema::EdgeStyle::Solid;
    }

    if action.contains("main") || action.contains("thick") || action.contains("emphasis") {
        edge.importance = crate::schema::EdgeImportance::Main;
    } else if action.contains("aux") || action.contains("secondary") {
        edge.importance = crate::schema::EdgeImportance::Aux;
    } else if action.contains("normal") {
        edge.importance = crate::schema::EdgeImportance::Normal;
    }
}

fn find_bbox_after_label(text: &str, label: &str) -> Option<[f64; 4]> {
    let start = text.find(label)?;
    find_patch_bbox_literal(&text[start + label.len()..])
}

fn find_bbox_after_standalone_label(text: &str, label: &str) -> Option<[f64; 4]> {
    let start = find_standalone_label_start(text, label)?;
    find_patch_bbox_literal(&text[start + label.len()..])
}

fn find_patch_bbox_literal(text: &str) -> Option<[f64; 4]> {
    for (start, _) in text.match_indices(" to ") {
        if let Some(bbox) = find_bbox_literal(&text[start + 4..]) {
            return Some(bbox);
        }
    }
    find_bbox_literal(text)
}

fn find_bbox_literal(text: &str) -> Option<[f64; 4]> {
    let start = text.find('[')?;
    let rest = &text[start + 1..];
    let end = rest.find(']')?;
    let inner = &rest[..end];
    let values = inner
        .split(|character: char| character == ',' || character == ';' || character.is_whitespace())
        .filter(|part| !part.trim().is_empty())
        .map(|part| part.trim().parse::<f64>())
        .collect::<std::result::Result<Vec<_>, _>>()
        .ok()?;
    if values.len() != 4 || values.iter().any(|value| !value.is_finite()) {
        return None;
    }
    let x1 = values[0].clamp(0.0, 1.0);
    let y1 = values[1].clamp(0.0, 1.0);
    let x2 = values[2].clamp(0.0, 1.0);
    let y2 = values[3].clamp(0.0, 1.0);
    let (x1, x2) = expand_patch_axis_to_min_size(x1.min(x2), x1.max(x2), 0.04);
    let (y1, y2) = expand_patch_axis_to_min_size(y1.min(y2), y1.max(y2), 0.06);
    Some([x1, y1, x2, y2])
}

fn find_quoted_value(text: &str) -> Option<String> {
    let double_quote = text.find('"').map(|index| (index, '"'));
    let single_quote = text.find('\'').map(|index| (index, '\''));
    let (start, quote) = match (double_quote, single_quote) {
        (Some(double_quote), Some(single_quote)) => {
            if double_quote.0 <= single_quote.0 {
                double_quote
            } else {
                single_quote
            }
        }
        (Some(double_quote), None) => double_quote,
        (None, Some(single_quote)) => single_quote,
        (None, None) => return None,
    };
    let rest = &text[start + 1..];
    let end = rest.find(quote)?;
    let value = rest[..end].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn component_region_id(_action: &str, component_id: &str) -> String {
    format!("{component_id}_region")
}

fn upsert_region(plan: &mut FigurePlan, region_id: &str, bbox: [f64; 4]) {
    if let Some(region) = plan
        .layout
        .regions
        .iter_mut()
        .find(|region| region.id == region_id)
    {
        region.bbox = bbox;
    } else {
        plan.layout.regions.push(LayoutRegion {
            id: region_id.to_string(),
            bbox,
        });
    }
}

fn find_standalone_label_start(text: &str, label: &str) -> Option<usize> {
    let mut search_start = 0;
    while let Some(relative_start) = text[search_start..].find(label) {
        let start = search_start + relative_start;
        let end = start + label.len();
        let before = text[..start].chars().next_back();
        let after = text[end..].chars().next();
        if before.map_or(true, |character| !is_stable_id_character(character))
            && after.map_or(true, |character| !is_stable_id_character(character))
        {
            return Some(start);
        }
        search_start = end;
    }
    None
}

fn is_stable_id_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

fn expand_patch_axis_to_min_size(start: f64, end: f64, min_size: f64) -> (f64, f64) {
    if end - start >= min_size {
        return (start, end);
    }
    let center = (start + end) / 2.0;
    let mut expanded_start = center - min_size / 2.0;
    let mut expanded_end = center + min_size / 2.0;
    if expanded_start < 0.0 {
        expanded_end = (expanded_end - expanded_start).min(1.0);
        expanded_start = 0.0;
    }
    if expanded_end > 1.0 {
        expanded_start = (expanded_start - (expanded_end - 1.0)).max(0.0);
        expanded_end = 1.0;
    }
    (expanded_start, expanded_end)
}

fn strip_json_fence(text: &str) -> &str {
    let trimmed = text.trim();
    if let Some(rest) = trimmed.strip_prefix("```") {
        if let Some((_, body)) = rest.split_once('\n') {
            if let Some((json, _)) = body.rsplit_once("```") {
                return json.trim();
            }
        }
    }
    trimmed
}

fn parse_review_text(text: &str) -> Result<Review> {
    serde_json::from_str(strip_json_fence(text))
        .with_context(|| "vision model did not return valid Review JSON")
}

fn aspect_json_name(aspect: CanvasAspect) -> &'static str {
    match aspect {
        CanvasAspect::PaperWide => "paper-wide",
        CanvasAspect::SingleColumn => "single-column",
        CanvasAspect::DoubleColumn => "double-column",
        CanvasAspect::SixteenNine => "16:9",
    }
}

fn style_json_name(style: StyleName) -> &'static str {
    match style {
        StyleName::WpsClean => "wps-clean",
        StyleName::CvprClean => "cvpr-clean",
        StyleName::NeuripsMinimal => "neurips-minimal",
    }
}

fn block_on<T>(future: impl std::future::Future<Output = Result<T>>) -> Result<T> {
    tokio::runtime::Runtime::new()
        .context("failed to create Tokio runtime for model call")?
        .block_on(future)
}
