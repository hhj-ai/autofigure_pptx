use std::path::Path;

use anyhow::{anyhow, Context, Result};

use crate::config::AppConfig;
use crate::llm::openai_compatible::OpenAiCompatibleProvider;
use crate::llm::{ChatMessage, ChatProvider, ChatRequest};
use crate::prompts::{
    CODER_PPTXGENJS_GENERATOR, REASON_INITIAL_PLANNER, REASON_PATCH_PLANNER, VISION_REVIEWER,
};
use crate::schema::{CanvasAspect, FigurePlan, PatchPlan, Review, StyleName, VisualWeight};
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
    let request = ChatRequest {
        temperature: 0.1,
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: REASON_INITIAL_PLANNER.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: format!(
                    "Style: {:?}\nAspect: {:?}\nTarget width mm: {target_width_mm}\nMethod Markdown:\n{method}\nReturn FigurePlan JSON only.",
                    style, aspect
                ),
            },
        ],
    };
    let text = block_on(provider.complete(request))?;
    let plan: FigurePlan = serde_json::from_str(strip_json_fence(&text))
        .with_context(|| "reasoner did not return valid FigurePlan JSON")?;
    Ok(plan)
}

pub fn create_typescript_code(
    plan: &FigurePlan,
    style: &StyleSpec,
    round_dir: &Path,
    renderer_root: &Path,
    asset_paths: &std::collections::BTreeMap<String, std::path::PathBuf>,
    config: &AppConfig,
    mock_models: bool,
) -> Result<String> {
    let deterministic = generate_typescript(plan, style, round_dir, renderer_root, asset_paths)?;
    if mock_models {
        return Ok(deterministic);
    }
    let provider = OpenAiCompatibleProvider::new(config.coder.clone());
    let request = ChatRequest {
        temperature: 0.0,
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: CODER_PPTXGENJS_GENERATOR.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: format!(
                    "Generate TypeScript for this FigurePlan using the local renderer runtime only.\nFigurePlan:\n{}\nStyle:\n{}\nAsset paths:\n{}\nRenderer root: {}\nRound dir: {}\nA deterministic safe reference implementation follows; preserve its import/runtime contract unless you can improve layout while keeping safety:\n{}",
                    serde_json::to_string_pretty(plan)?,
                    serde_json::to_string_pretty(style)?,
                    serde_json::to_string_pretty(asset_paths)?,
                    renderer_root.display(),
                    round_dir.display(),
                    deterministic
                ),
            },
        ],
    };
    let text = block_on(provider.complete(request))?;
    let code = strip_code_fence(&text).to_string();
    scan_generated_typescript(&code)?;
    Ok(code)
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
    let user_text = format!(
        "Review this paper method figure. Return Review JSON only.\nFigurePlan:\n{plan_json}\nlayout_map.json:\n{layout_map}"
    );
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
    let review: Review = serde_json::from_str(strip_json_fence(&text))
        .with_context(|| "vision model did not return valid Review JSON")?;
    Ok(review)
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
                content: REASON_PATCH_PLANNER.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: format!(
                    "Previous FigurePlan:\n{}\nReview:\n{}\nReturn PatchPlan JSON only.",
                    serde_json::to_string_pretty(plan)?,
                    serde_json::to_string_pretty(review)?
                ),
            },
        ],
    };
    let text = block_on(provider.complete(request))?;
    let patch: PatchPlan = serde_json::from_str(strip_json_fence(&text))
        .with_context(|| "reasoner did not return valid PatchPlan JSON")?;
    Ok(patch)
}

pub fn apply_patch_plan_to_figure(plan: &mut FigurePlan) {
    for component in &mut plan.components {
        let role_is_main = matches!(component.role, crate::schema::ComponentRole::Main);
        if role_is_main {
            component.visual_weight = VisualWeight::Strong;
            if !component.label.contains("Core") && component.label == "Method" {
                component.label = "Method Core".to_string();
            }
        }
    }

    plan.story
        .visual_focus
        .push("main contribution path emphasized".to_string());
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

fn strip_code_fence(text: &str) -> &str {
    let trimmed = text.trim();
    if let Some(rest) = trimmed.strip_prefix("```") {
        if let Some((_, body)) = rest.split_once('\n') {
            if let Some((code, _)) = body.rsplit_once("```") {
                return code.trim();
            }
        }
    }
    trimmed
}

fn block_on<T>(future: impl std::future::Future<Output = Result<T>>) -> Result<T> {
    tokio::runtime::Runtime::new()
        .context("failed to create Tokio runtime for model call")?
        .block_on(future)
}
