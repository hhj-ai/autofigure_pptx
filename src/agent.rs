use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

use crate::config::AppConfig;
use crate::llm::openai_compatible::OpenAiCompatibleProvider;
use crate::llm::{ChatMessage, ChatProvider, ChatRequest};
use crate::prompts::{
    CODER_DRAW_PLAN_INITIAL, CODER_DRAW_PLAN_REVISION, DRAW_PLAN_OPTIMIZER, REASON_INITIAL_PLANNER,
    REASON_PATCH_PLANNER, REFERENCE_SELECTOR, ROUND_IMPROVEMENT_PLANNER, TOP_TIER_FIGURE_DIRECTIVE,
    VISION_REVIEWER,
};
use crate::schema::{
    draw_plan_schema_json, figure_plan_schema_json, patch_plan_schema_json,
    reference_selection_schema_json, review_schema_json, round_improvement_plan_schema_json,
    validate_draw_plan, CanvasAspect, DrawObject, DrawPlan, FigurePlan, ImprovementAction,
    LayoutRegion, PatchOperation, PatchOperationType, PatchPlan, PatchStopReason,
    ReferencePreviewMode, ReferenceSelection, Review, RoundImprovementPlan, StyleName,
    VisualWeight,
};
use crate::style::StyleSpec;
use crate::tools::draw_plan::{
    generate_draw_plan_typescript, has_material_draw_plan_change, normalize_draw_plan_bounds,
    preserve_semantic_draw_objects,
};
use crate::tools::generated_code::{normalize_generated_code_bundle, GeneratedCodeBundle};
use crate::tools::pptx_codegen::generate_typescript;
use crate::tools::render::scan_generated_typescript;
use crate::tools::template_library::{
    method_template_pack_json, reference_pack_json, select_reference_for_method,
    selected_reference_json,
};

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

pub fn create_reference_selection(
    method: &str,
    preview_mode: ReferencePreviewMode,
    config: &AppConfig,
    mock_models: bool,
) -> Result<ReferenceSelection> {
    if mock_models {
        return select_reference_for_method(method, preview_mode);
    }
    if !config.reasoner.is_configured() {
        return Err(anyhow!(
            "reasoner model is not configured for reference selection"
        ));
    }
    let provider = OpenAiCompatibleProvider::new(config.reasoner.clone());
    let request = ChatRequest {
        temperature: 0.0,
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: REFERENCE_SELECTOR.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: build_reference_selection_prompt(method, preview_mode)?,
            },
        ],
    };
    let text = block_on(provider.complete(request))?;
    let mut selection: ReferenceSelection = serde_json::from_str(strip_json_fence(&text))
        .with_context(|| "reasoner did not return valid ReferenceSelection JSON")?;
    selection.preview_mode = preview_mode;
    Ok(selection)
}

pub fn build_reference_selection_prompt(
    method: &str,
    preview_mode: ReferencePreviewMode,
) -> Result<String> {
    let schema = reference_selection_schema_json()?;
    let pack = reference_pack_json()?;
    Ok(format!(
        "Select exactly one reference from reference_figures.json for the method below.\n\
Return strict ReferenceSelection JSON only: no markdown, no prose, no schema wrapper.\n\
Set version to \"0.1\" and preview_mode to \"{}\".\n\
Use the reference as read-only layout/style evidence. Do not copy source artwork and do not ask the renderer to use preview images as output assets.\n\n\
ReferenceSelection JSON Schema:\n{schema}\n\n\
reference_figures.json:\n{pack}\n\n\
Method Markdown:\n{method}",
        reference_preview_mode_name(preview_mode)
    ))
}

pub fn create_initial_plan(
    method: &str,
    style: StyleName,
    aspect: CanvasAspect,
    target_width_mm: u32,
    reference_selection: &ReferenceSelection,
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
    let user_prompt =
        build_initial_plan_prompt(method, style, aspect, target_width_mm, reference_selection)?;
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
    reference_selection: &ReferenceSelection,
) -> Result<String> {
    let schema = figure_plan_schema_json()?;
    let selected_reference = selected_reference_json(reference_selection)?;
    Ok(format!(
        "Create exactly one FigurePlan object for the method below.\n\
Return JSON only: no markdown, no prose, no schema wrapper, no comments.\n\
Required top-level keys: version, canvas, story, layout, components, edges, annotations, assets, design.\n\
Every stable id must be globally unique across layout.regions, components, edges, annotations, and assets; do not reuse a component id as an edge or asset id.\n\
Set version to \"0.1\". Set canvas.aspect to \"{}\" and canvas.target_width_mm to {target_width_mm}.\n\
Use design.style \"{}\". Keep generated assets small and local only; semantic labels must remain editable PPTX text.\n\
Use the Selected visual reference as layout grammar and quality target. Adapt its slots, flows, anti_patterns, and quality_targets. Do not copy source artwork. If preview_path is present, treat it as a read-only preview only. The renderer must not use reference preview images as assets.\n\n\
{}\n\n\
Selected visual reference (ReferenceSelection):\n{}\n\n\
FigurePlan JSON Schema:\n{schema}\n\n\
Method Markdown:\n{method}",
        aspect_json_name(aspect),
        style_json_name(style),
        TOP_TIER_FIGURE_DIRECTIVE,
        selected_reference
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

pub fn create_initial_code_bundle(
    draw_plan: &DrawPlan,
    style: &StyleSpec,
    round_dir: &Path,
    renderer_root: &Path,
    asset_paths: &BTreeMap<String, PathBuf>,
    config: &AppConfig,
    mock_models: bool,
) -> Result<GeneratedCodeBundle> {
    let deterministic =
        generate_draw_plan_typescript(draw_plan, style, round_dir, renderer_root, asset_paths)?;
    if mock_models {
        return Ok(GeneratedCodeBundle::single_figure(
            deterministic,
            "mock initial code generated from DrawPlan",
        ));
    }
    if !config.coder.is_configured() {
        return Err(anyhow!("coder model is not configured for generated code"));
    }
    let provider = OpenAiCompatibleProvider::new(config.coder.clone());
    let request = ChatRequest {
        temperature: 0.1,
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: CODER_DRAW_PLAN_INITIAL.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: build_initial_code_prompt(
                    draw_plan,
                    style,
                    renderer_root,
                    asset_paths,
                    &deterministic,
                )?,
            },
        ],
    };
    code_bundle_or_draw_plan_runtime(deterministic, block_on(provider.complete(request)), "coder")
}

pub fn create_revised_code_bundle(
    previous_code: &str,
    draw_plan: &DrawPlan,
    review: &Review,
    layout_map: &str,
    validation_report: &str,
    style: &StyleSpec,
    round_dir: &Path,
    renderer_root: &Path,
    asset_paths: &BTreeMap<String, PathBuf>,
    config: &AppConfig,
    mock_models: bool,
    round_index: u32,
) -> Result<GeneratedCodeBundle> {
    let deterministic =
        generate_draw_plan_typescript(draw_plan, style, round_dir, renderer_root, asset_paths)?;
    if mock_models {
        let feedback = review
            .blocking_issues
            .first()
            .map(String::as_str)
            .unwrap_or("mock revision after previous review");
        return Ok(GeneratedCodeBundle::single_figure(
            format!(
                "// methodfig mock coder revision round {round_index}\n// feedback: {}\n{}",
                sanitize_ts_comment(feedback),
                deterministic
            ),
            "mock revised code generated from previous feedback",
        ));
    }
    if !config.coder.is_configured() {
        return Err(anyhow!("coder model is not configured for code revision"));
    }
    let provider = OpenAiCompatibleProvider::new(config.coder.clone());
    let request = ChatRequest {
        temperature: 0.1,
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: CODER_DRAW_PLAN_REVISION.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: build_revised_code_prompt(
                    previous_code,
                    draw_plan,
                    review,
                    layout_map,
                    validation_report,
                    style,
                    renderer_root,
                    asset_paths,
                    &deterministic,
                )?,
            },
        ],
    };
    code_bundle_or_draw_plan_runtime(
        deterministic,
        block_on(provider.complete(request)),
        "revised coder",
    )
}

pub fn revise_draw_plan_from_feedback(
    previous_draw_plan: &DrawPlan,
    review: &Review,
    layout_map: &str,
    validation_report: &str,
    overlay_path: &Path,
    reference_selection: &ReferenceSelection,
    improvement_plan_json: &str,
    reference_preview_path: Option<&Path>,
    config: &AppConfig,
    mock_models: bool,
    round_index: u32,
) -> Result<DrawPlan> {
    if mock_models {
        let revised = mock_revise_draw_plan(previous_draw_plan, round_index);
        validate_draw_plan(&revised)?;
        return Ok(revised);
    }
    if !config.vision.is_configured() {
        return Err(anyhow!(
            "vision model is not configured for DrawPlan optimization"
        ));
    }
    let previous_draw_plan_json = serde_json::to_string_pretty(previous_draw_plan)?;
    let review_json = serde_json::to_string_pretty(review)?;
    let prompt = build_draw_plan_revision_prompt(
        &previous_draw_plan_json,
        layout_map,
        &review_json,
        validation_report,
        reference_selection,
        improvement_plan_json,
    )?;
    let vision_images = vision_images_with_optional_reference(overlay_path, reference_preview_path);
    let text = block_on(OpenAiCompatibleProvider::complete_vision_with_images(
        config.vision.clone(),
        DRAW_PLAN_OPTIMIZER,
        &prompt,
        &vision_images,
    ))?;
    let mut draw_plan: DrawPlan = serde_json::from_str(strip_json_fence(&text))
        .with_context(|| "DrawPlan optimizer did not return valid DrawPlan JSON")?;
    preserve_semantic_draw_objects(previous_draw_plan, &mut draw_plan);
    normalize_draw_plan_bounds(&mut draw_plan);
    validate_draw_plan(&draw_plan)?;
    if !has_material_draw_plan_change(previous_draw_plan, &draw_plan) {
        let retry_prompt = format!(
            "{prompt}\n\nThe previous DrawPlan optimizer output was rejected because it made no material change to bbox, connector points, label bbox, text, style, object additions, or object removals. Return a corrected DrawPlan that implements the RoundImprovementPlan with visible changes."
        );
        let retry_text = block_on(OpenAiCompatibleProvider::complete_vision_with_images(
            config.vision.clone(),
            DRAW_PLAN_OPTIMIZER,
            &retry_prompt,
            &vision_images,
        ))?;
        draw_plan = serde_json::from_str(strip_json_fence(&retry_text))
            .with_context(|| "DrawPlan optimizer retry did not return valid DrawPlan JSON")?;
        preserve_semantic_draw_objects(previous_draw_plan, &mut draw_plan);
        normalize_draw_plan_bounds(&mut draw_plan);
        validate_draw_plan(&draw_plan)?;
        if !has_material_draw_plan_change(previous_draw_plan, &draw_plan) {
            return Err(anyhow!(
                "DrawPlan optimizer returned no material visible change after retry"
            ));
        }
    }
    Ok(draw_plan)
}

pub fn build_draw_plan_revision_prompt(
    previous_draw_plan_json: &str,
    layout_map: &str,
    review_json: &str,
    validation_report_json: &str,
    reference_selection: &ReferenceSelection,
    improvement_plan_json: &str,
) -> Result<String> {
    let schema = draw_plan_schema_json()?;
    let selected_reference = selected_reference_json(reference_selection)?;
    Ok(format!(
        "Revise the editable primitive DrawPlan for the next round.\n\
Return exactly one DrawPlan object. Do not return TypeScript, SVG, markdown, prose, comments, or a schema wrapper.\n\
The current rendered overlay image is attached. Treat it like AutoFigure-Edit's optimizer evidence: compare the visual overlay, layout_map coordinates, local validation errors, reviewer feedback, and current DrawPlan code/state.\n\n\
Use the Selected visual reference as read-only layout/style evidence. If reference preview evidence is attached after the current overlay, use it only to judge composition quality; do not copy the source artwork and do not add it as a DrawPlan image. These references are derived from extracted PDF/SVG method-overview figures from classic or ML conference award papers. Treat the selected reference anti_patterns as hard: if the current figure contains one, remove or redesign it instead of merely moving it.\n\n\
You must implement the RoundImprovementPlan. The returned DrawPlan must materially change at least one bbox, connector point list, connector label bbox, text, style, object addition, or object removal unless the Review already passes.\n\n\
You are a visual optimizer, not a semantic replanner. Do not invent new semantic modules, duplicate outputs, extra loss boxes, or new branches that are absent from the current semantic state. Do not expand an inference note into a separate inference subgraph unless such boxes/connectors already exist in the DrawPlan.\n\n\
Do not add an output-to-student task-loss feedback edge when a task_loss box or output-to-loss edge already exists. If the semantic state contains teacher-to-student latent residual supervision as an edge, prefer a direct dashed residual edge instead of creating a separate residual box.\n\n\
Please carefully compare and optimize these TWO MAJOR ASPECTS:\n\
## ASPECT 1: POSITION\n\
1. Components and groups: resize oversized empty boxes and move crowded items into clean whitespace.\n\
2. Text positions: move labels away from connector strokes and away from other labels.\n\
3. Arrows and connectors: use orthogonal or clean polyline routes for non-branching flows; avoid diagonal wandering.\n\
4. Lines and borders: align starts/ends to box edges and avoid overlaps.\n\n\
## ASPECT 2: STYLE\n\
5. Component hierarchy: keep the main contribution visually strongest without huge empty containers.\n\
6. Text style: keep labels short, readable at paper width, and editable.\n\
7. Arrow style: preserve semantic styles such as dashed supervision and solid data flow.\n\
8. Line/border style: keep restrained high-contrast WPS-friendly styling.\n\n\
Keep stable ids for objects that remain semantic parts of the figure. Remove only redundant or marginal explanatory text objects explicitly flagged by the review.\n\
All bbox values and connector points must be normalized [0,1]. Keep objects inside the canvas safe area.\n\
Do not add full-slide raster images; semantic content must remain native shapes/text/connectors.\n\n\
DrawPlan JSON Schema:\n{schema}\n\n\
Selected visual reference (ReferenceSelection):\n{selected_reference}\n\n\
RoundImprovementPlan:\n{improvement_plan_json}\n\n\
Current DrawPlan JSON:\n{previous_draw_plan_json}\n\n\
layout_map.json:\n{layout_map}\n\n\
Review JSON:\n{review_json}\n\n\
validation_report.json:\n{validation_report_json}"
    ))
}

fn build_initial_code_prompt(
    draw_plan: &DrawPlan,
    style: &StyleSpec,
    renderer_root: &Path,
    asset_paths: &BTreeMap<String, PathBuf>,
    deterministic_code: &str,
) -> Result<String> {
    let method_templates = method_template_pack_json()?;
    Ok(format!(
        "Return a GeneratedCodeBundle JSON object for this figure renderer.\n\
Allowed paths are writable/code/figure.ts and writable/code/helpers.ts. Include writable/code/figure.ts.\n\
method_templates.json is provided as layout evidence derived from extracted PDF/SVG figures. Do not copy those source figures; render the current DrawPlan as native PPTX shapes/text/connectors.\n\
GeneratedCodeBundle JSON Schema:\n{}\n\n\
Renderer runtime path:\n{}\n\n\
method_templates.json:\n{}\n\n\
StyleSpec JSON:\n{}\n\n\
Asset paths JSON:\n{}\n\n\
DrawPlan JSON:\n{}\n\n\
Reference code that already renders the DrawPlan; you may improve structure, helper extraction, and small renderer-level choices but must keep the same output contract:\n```ts\n{}\n```",
        GeneratedCodeBundle::schema_json()?,
        renderer_root.join("src/runtime.ts").display(),
        method_templates,
        serde_json::to_string_pretty(style)?,
        serde_json::to_string_pretty(asset_paths)?,
        serde_json::to_string_pretty(draw_plan)?,
        deterministic_code
    ))
}

fn build_revised_code_prompt(
    previous_code: &str,
    draw_plan: &DrawPlan,
    review: &Review,
    layout_map: &str,
    validation_report: &str,
    style: &StyleSpec,
    renderer_root: &Path,
    asset_paths: &BTreeMap<String, PathBuf>,
    deterministic_code: &str,
) -> Result<String> {
    let method_templates = method_template_pack_json()?;
    Ok(format!(
        "Revise the generated renderer code. Return a GeneratedCodeBundle JSON object only.\n\
Allowed paths are writable/code/figure.ts and writable/code/helpers.ts. Include writable/code/figure.ts.\n\
method_templates.json is provided as layout evidence derived from extracted PDF/SVG figures. Do not copy those source figures; implement the current DrawPlan as native PPTX shapes/text/connectors.\n\
GeneratedCodeBundle JSON Schema:\n{}\n\n\
Renderer runtime path:\n{}\n\n\
method_templates.json:\n{}\n\n\
StyleSpec JSON:\n{}\n\n\
Asset paths JSON:\n{}\n\n\
Current DrawPlan JSON:\n{}\n\n\
Previous figure.ts:\n```ts\n{}\n```\n\n\
Previous review JSON:\n{}\n\n\
Previous layout_map.json:\n{}\n\n\
Previous validation_report.json:\n{}\n\n\
Reference code that renders the current DrawPlan if the previous code is too broken:\n```ts\n{}\n```",
        GeneratedCodeBundle::schema_json()?,
        renderer_root.join("src/runtime.ts").display(),
        method_templates,
        serde_json::to_string_pretty(style)?,
        serde_json::to_string_pretty(asset_paths)?,
        serde_json::to_string_pretty(draw_plan)?,
        previous_code,
        serde_json::to_string_pretty(review)?,
        layout_map,
        validation_report,
        deterministic_code
    ))
}

fn parse_generated_code_bundle_text(text: &str) -> Result<GeneratedCodeBundle> {
    let bundle: GeneratedCodeBundle = serde_json::from_str(strip_json_fence(text))?;
    normalize_generated_code_bundle(&bundle)
}

pub fn code_bundle_or_draw_plan_runtime(
    deterministic_code: String,
    coder_result: Result<String>,
    context: &str,
) -> Result<GeneratedCodeBundle> {
    match coder_result {
        Ok(text) => parse_generated_code_bundle_text(&text).or_else(|error| {
            Ok(GeneratedCodeBundle::single_figure(
                deterministic_code,
                format!(
                    "{context} returned unusable output; deterministic DrawPlan runtime used: {error}"
                ),
            ))
        }),
        Err(error) => Ok(GeneratedCodeBundle::single_figure(
            deterministic_code,
            format!("{context} failed; deterministic DrawPlan runtime used: {error}"),
        )),
    }
}

fn mock_revise_draw_plan(previous_draw_plan: &DrawPlan, round_index: u32) -> DrawPlan {
    let mut revised = previous_draw_plan.clone();
    revised
        .objects
        .retain(|object| !matches!(object, DrawObject::Text { id, .. } if id.starts_with("a_")));

    for object in &mut revised.objects {
        match object {
            DrawObject::Box { id, bbox, .. } => {
                if id == "inference_tag" {
                    *bbox = [0.72, 0.72, 0.94, 0.80];
                } else if box_height(*bbox) > 0.48 {
                    *bbox = shrink_box_height(*bbox, 0.34);
                }
            }
            DrawObject::Connector { points, label, .. } => {
                if points.len() == 2 && points[0][0] != points[1][0] && points[0][1] != points[1][1]
                {
                    let start = points[0];
                    let end = points[1];
                    points.insert(1, [end[0], start[1]]);
                }
                if let Some(label) = label {
                    label.bbox = offset_label_box(label.bbox, round_index);
                }
            }
            DrawObject::Text { bbox, .. } => {
                *bbox = clamp_bbox(*bbox);
            }
            DrawObject::Image { bbox, .. } | DrawObject::Group { bbox, .. } => {
                *bbox = clamp_bbox(*bbox);
            }
        }
    }
    revised
}

fn offset_label_box(bbox: [f64; 4], round_index: u32) -> [f64; 4] {
    let dy = 0.08 + f64::from(round_index.min(3)) * 0.01;
    clamp_bbox([bbox[0], bbox[1] - dy, bbox[2], bbox[3] - dy])
}

fn shrink_box_height(bbox: [f64; 4], target_height: f64) -> [f64; 4] {
    let [x1, y1, x2, y2] = clamp_bbox(bbox);
    let center_y = (y1 + y2) / 2.0;
    let half = target_height / 2.0;
    clamp_bbox([x1, center_y - half, x2, center_y + half])
}

fn clamp_bbox(bbox: [f64; 4]) -> [f64; 4] {
    let x1 = bbox[0].clamp(0.0, 1.0);
    let y1 = bbox[1].clamp(0.0, 1.0);
    let x2 = bbox[2].clamp(0.0, 1.0);
    let y2 = bbox[3].clamp(0.0, 1.0);
    [x1.min(x2), y1.min(y2), x1.max(x2), y1.max(y2)]
}

fn box_height(bbox: [f64; 4]) -> f64 {
    let bbox = clamp_bbox(bbox);
    bbox[3] - bbox[1]
}

pub fn review_rendered_figure(
    plan: &FigurePlan,
    round_dir: &Path,
    reference_selection: &ReferenceSelection,
    reference_preview_path: Option<&Path>,
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
    let draw_plan_json = std::fs::read_to_string(round_dir.join("draw_plan.json"))
        .unwrap_or_else(|_| "{}".to_string());
    let layout_map = std::fs::read_to_string(round_dir.join("layout_map.json"))
        .unwrap_or_else(|_| "{}".to_string());
    let user_text = build_review_prompt(
        &plan_json,
        &draw_plan_json,
        &layout_map,
        reference_selection,
    )?;
    let image_path = round_dir.join(format!(
        "figure_{}mm_preview.png",
        plan.canvas.target_width_mm
    ));
    let vision_images = vision_images_with_optional_reference(&image_path, reference_preview_path);
    let text = block_on(OpenAiCompatibleProvider::complete_vision_with_images(
        config.vision.clone(),
        VISION_REVIEWER,
        &user_text,
        &vision_images,
    ))?;
    let review = parse_review_text(&text).or_else(|first_error| {
        let retry_user_text = build_review_retry_prompt(
            &plan_json,
            &draw_plan_json,
            &layout_map,
            reference_selection,
        )?;
        let retry_text = block_on(OpenAiCompatibleProvider::complete_vision_with_images(
            config.vision.clone(),
            VISION_REVIEWER,
            &retry_user_text,
            &vision_images,
        ))?;
        parse_review_text(&retry_text).with_context(|| {
            format!(
                "vision model did not return valid Review JSON after retry; first parse error: {first_error}"
            )
        })
    })?;
    Ok(review)
}

pub fn build_review_prompt(
    plan_json: &str,
    draw_plan_json: &str,
    layout_map: &str,
    reference_selection: &ReferenceSelection,
) -> Result<String> {
    let schema = review_schema_json()?;
    let selected_reference = selected_reference_json(reference_selection)?;
    Ok(format!(
        "Review this paper method figure. Return exactly one Review object.\n\
Return JSON only: no markdown, no prose, no comments.\n\
Scores must include every field from the schema, including semantic_fidelity and wps_editability, each as an integer from 1 to 10.\n\
Keep string values short and do not embed quotation marks inside string content.\n\n\
DrawPlan is the rendered source of truth for visible editable objects. Use FigurePlan for semantic intent, but judge object presence, removal, routing, and placement against DrawPlan and layout_map.\n\
Do not report FigurePlan annotations as missing when they are absent from DrawPlan; assume redundant or marginal annotations may be intentionally removed before rendering.\n\n\
Use the Selected visual reference as the quality target. Respect its reading order and anti-patterns when the current DrawPlan is trying to follow it. If an optional reference preview image is attached, it is read-only evidence and must not be treated as an output asset.\n\n\
Every rejection must include localized_issues or at least one blocking issue that names concrete target ids from DrawPlan/layout_map. The suggested_direction must be actionable and describe a visible bbox, connector route, label, spacing, or style change.\n\n\
For wps_editability, inspect DrawPlan/layout_map rather than guessing from the PNG. If the visible semantic figure is composed of native boxes, text, and connectors with no full-slide raster image, assign wps_editability 9 or 10 unless there is concrete evidence of non-editable text, missing objects, or invalid geometry.\n\n\
Review JSON Schema:\n{schema}\n\n\
Selected visual reference (ReferenceSelection):\n{selected_reference}\n\n\
FigurePlan:\n{plan_json}\n\n\
DrawPlan:\n{draw_plan_json}\n\n\
layout_map.json:\n{layout_map}"
    ))
}

pub fn build_review_retry_prompt(
    plan_json: &str,
    draw_plan_json: &str,
    layout_map: &str,
    reference_selection: &ReferenceSelection,
) -> Result<String> {
    let schema = review_schema_json()?;
    let selected_reference = selected_reference_json(reference_selection)?;
    Ok(format!(
        "The previous answer was not valid JSON. Return the same Review object again.\n\
Return strict JSON only: no markdown, no prose, no comments, and no code fences.\n\
Keep each string value short and avoid embedded quotation marks inside the text.\n\
Scores must include every field from the schema, including semantic_fidelity and wps_editability, each as an integer from 1 to 10.\n\n\
DrawPlan is the rendered source of truth for visible editable objects. Use FigurePlan for semantic intent, but judge object presence, removal, routing, and placement against DrawPlan and layout_map.\n\
Do not report FigurePlan annotations as missing when they are absent from DrawPlan.\n\n\
Use the Selected visual reference as the quality target. Every rejection must include localized_issues or concrete target ids in blocking_issues, with actionable suggested_direction text.\n\n\
For wps_editability, inspect DrawPlan/layout_map rather than guessing from the PNG. If the visible semantic figure is composed of native boxes, text, and connectors with no full-slide raster image, assign wps_editability 9 or 10 unless there is concrete evidence of non-editable text, missing objects, or invalid geometry.\n\n\
Review JSON Schema:\n{schema}\n\n\
Selected visual reference (ReferenceSelection):\n{selected_reference}\n\n\
FigurePlan:\n{plan_json}\n\n\
DrawPlan:\n{draw_plan_json}\n\n\
layout_map.json:\n{layout_map}"
    ))
}

pub fn create_round_improvement_plan(
    review: &Review,
    layout_map: &str,
    validation_report: &str,
    reference_selection: &ReferenceSelection,
    config: &AppConfig,
    mock_models: bool,
    round_index: u32,
) -> Result<RoundImprovementPlan> {
    if mock_models {
        return Ok(mock_round_improvement_plan(
            review,
            reference_selection,
            round_index,
        ));
    }
    if !config.reasoner.is_configured() {
        return Err(anyhow!(
            "reasoner model is not configured for round improvement planning"
        ));
    }
    let provider = OpenAiCompatibleProvider::new(config.reasoner.clone());
    let request = ChatRequest {
        temperature: 0.0,
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: ROUND_IMPROVEMENT_PLANNER.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: build_round_improvement_prompt(
                    &serde_json::to_string_pretty(review)?,
                    layout_map,
                    validation_report,
                    reference_selection,
                    round_index,
                )?,
            },
        ],
    };
    let text = block_on(provider.complete(request))?;
    let plan: RoundImprovementPlan = serde_json::from_str(strip_json_fence(&text))
        .with_context(|| "reasoner did not return valid RoundImprovementPlan JSON")?;
    validate_round_improvement_plan(&plan, review)?;
    Ok(plan)
}

pub fn build_round_improvement_prompt(
    review_json: &str,
    layout_map: &str,
    validation_report: &str,
    reference_selection: &ReferenceSelection,
    round_index: u32,
) -> Result<String> {
    let schema = round_improvement_plan_schema_json()?;
    let selected_reference = selected_reference_json(reference_selection)?;
    Ok(format!(
        "Create the next-round improvement plan for round {round_index}.\n\
Return exactly one RoundImprovementPlan JSON object. Use actions as an array.\n\
If Review.passed is false, actions must be non-empty. Each action must name a target_id from layout_map when possible, or use change_type \"reference_replan\" for a template-level change.\n\
Do not give vague advice. Each expected_visible_effect must state what will visibly change in bbox, connector points, label bbox, text, style, object addition, or object removal.\n\n\
RoundImprovementPlan JSON Schema:\n{schema}\n\n\
Selected visual reference (ReferenceSelection):\n{selected_reference}\n\n\
Review JSON:\n{review_json}\n\n\
layout_map.json:\n{layout_map}\n\n\
validation_report.json:\n{validation_report}"
    ))
}

fn mock_round_improvement_plan(
    review: &Review,
    reference_selection: &ReferenceSelection,
    round_index: u32,
) -> RoundImprovementPlan {
    let mut actions = Vec::new();
    if !review.passed {
        for issue in &review.localized_issues {
            actions.push(ImprovementAction {
                target_id: Some(issue.target_id.clone()),
                change_type: "localized_geometry_or_style".to_string(),
                issue: issue.issue.clone(),
                expected_visible_effect: issue.suggested_direction.clone(),
                success_check: format!(
                    "{} no longer appears in review/layout_map issues",
                    issue.target_id
                ),
            });
        }
        if actions.is_empty() {
            for issue in &review.blocking_issues {
                actions.push(ImprovementAction {
                    target_id: Some("global_layout".to_string()),
                    change_type: "reference_replan".to_string(),
                    issue: issue.clone(),
                    expected_visible_effect: "Apply selected reference anti-patterns to make a visible layout or routing change".to_string(),
                    success_check: "next DrawPlan material diff is non-empty and the blocking issue is reduced".to_string(),
                });
            }
        }
    }
    RoundImprovementPlan {
        version: "0.1".to_string(),
        round_index,
        reference_id: reference_selection.selected_reference_id.clone(),
        summary: if review.passed {
            "Review passed; preserve current design.".to_string()
        } else {
            "Concrete next-round fixes derived from review and selected reference.".to_string()
        },
        actions,
        preserve: reference_selection.quality_targets.clone(),
        rejected_as_unusable: false,
    }
}

fn validate_round_improvement_plan(plan: &RoundImprovementPlan, review: &Review) -> Result<()> {
    if !review.passed && plan.actions.is_empty() {
        return Err(anyhow!(
            "RoundImprovementPlan for rejected review must include at least one action"
        ));
    }
    for action in &plan.actions {
        if action
            .target_id
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
            && action.change_type != "reference_replan"
        {
            return Err(anyhow!(
                "RoundImprovementPlan action must include target_id or reference_replan"
            ));
        }
        if action.expected_visible_effect.trim().is_empty()
            || action.success_check.trim().is_empty()
        {
            return Err(anyhow!(
                "RoundImprovementPlan action must include visible effect and success check"
            ));
        }
    }
    Ok(())
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

fn sanitize_ts_comment(text: &str) -> String {
    text.replace('\n', " ")
        .replace('\r', " ")
        .replace("*/", "* /")
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

fn reference_preview_mode_name(mode: ReferencePreviewMode) -> &'static str {
    match mode {
        ReferencePreviewMode::Auto => "auto",
        ReferencePreviewMode::Off => "off",
        ReferencePreviewMode::Required => "required",
    }
}

fn vision_images_with_optional_reference<'a>(
    primary: &'a Path,
    reference: Option<&'a Path>,
) -> Vec<&'a Path> {
    let mut images = vec![primary];
    if let Some(reference) = reference {
        if reference.exists() {
            images.push(reference);
        }
    }
    images
}

fn block_on<T>(future: impl std::future::Future<Output = Result<T>>) -> Result<T> {
    tokio::runtime::Runtime::new()
        .context("failed to create Tokio runtime for model call")?
        .block_on(future)
}
