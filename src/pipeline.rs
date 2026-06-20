use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use crate::agent::{
    create_initial_code_bundle, create_initial_plan, create_revised_code_bundle,
    review_rendered_figure, revise_draw_plan_from_feedback, validate_required_model_config,
};
use crate::config::AppConfig;
use crate::schema::{
    validate_draw_plan, CanvasAspect, DrawPlan, FigurePlan, ImageProviderKind, Review, StyleName,
};
use crate::style::style_by_name;
use crate::tools::asset_gen::materialize_assets;
use crate::tools::canonicalize::canonicalize_plan_for_render;
use crate::tools::cost::{
    CostTracker, EST_CODER_USD, EST_IMAGE_ASSET_USD, EST_REASONER_INITIAL_USD, EST_VISION_USD,
};
use crate::tools::draw_plan::{
    draw_plan_from_figure_plan, generate_draw_plan_typescript,
    polish_model_draw_plan_geometry_with_figure_plan, repair_draw_plan_geometry_with_figure_plan,
};
use crate::tools::export::export_round;
use crate::tools::generated_code::{
    write_generated_code_bundle, GeneratedCodeBundle, ENTRYPOINT_WORKSPACE_PATH,
    HELPER_WORKSPACE_PATH,
};
use crate::tools::render::{
    default_renderer_root, run_node_renderer, run_node_renderer_with_fallback,
};
use crate::tools::review::{apply_plan_geometry_gate, apply_render_quality_gate};
use crate::tools::template_library::method_template_pack_json;
use crate::tools::validate::{normalize_plan_for_render, validate_plan_for_render};
use crate::tools::workspace::{
    AgentWorkspace, WorkspaceFile, WorkspaceFileFormat, WorkspaceManifest,
};

#[derive(Clone, Debug)]
pub struct RunOptions {
    pub method_path: PathBuf,
    pub out_dir: PathBuf,
    pub style: StyleName,
    pub aspect: CanvasAspect,
    pub target_width_mm: u32,
    pub max_iterations: u32,
    pub max_cost_usd: f64,
    pub max_minutes: u32,
    pub image_provider: ImageProviderKind,
    pub mock_models: bool,
    pub keep_intermediate: bool,
    pub renderer_timeout: Duration,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PipelineResult {
    pub accepted: bool,
    pub rounds: u32,
    pub run_dir: PathBuf,
    pub final_dir: PathBuf,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ConfigSnapshot {
    style: StyleName,
    aspect: CanvasAspect,
    target_width_mm: u32,
    max_iterations: u32,
    max_cost_usd: f64,
    max_minutes: u32,
    image_provider: ImageProviderKind,
    mock_models: bool,
    keep_intermediate: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RoundValidationReport {
    warnings: Vec<String>,
    errors: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RendererStatus {
    source: String,
    used_fallback: bool,
}

pub fn run_pipeline(options: RunOptions) -> Result<PipelineResult> {
    run_pipeline_inner(options, None)
}

#[derive(Clone, Debug)]
struct ResumeState {
    method_text: String,
    plan: FigurePlan,
    best_round: Option<(u32, Review)>,
    previous_review: Option<Review>,
    next_round_index: u32,
    rounds_completed: u32,
}

fn run_pipeline_inner(
    options: RunOptions,
    resume_state: Option<ResumeState>,
) -> Result<PipelineResult> {
    let start = Instant::now();
    fs::create_dir_all(&options.out_dir)?;

    let config = AppConfig::from_env()?;
    validate_required_model_config(&config, options.mock_models)?;
    let mut cost = CostTracker::new(options.max_cost_usd)?;
    let style = style_by_name(options.style);
    let renderer_root = default_renderer_root()?;
    let mut cap_reason: Option<String> = None;
    let (
        method_text,
        mut plan,
        mut best_round,
        mut previous_review,
        mut rounds_completed,
        mut round_index,
    ) = match resume_state {
        Some(state) => {
            append_log(
                &options.out_dir,
                &format!("run resumed at round {}", state.next_round_index),
            )?;
            (
                state.method_text,
                state.plan,
                state.best_round,
                state.previous_review,
                state.rounds_completed,
                state.next_round_index,
            )
        }
        None => {
            let method_text = fs::read_to_string(&options.method_path).with_context(|| {
                format!(
                    "failed to read method file {}",
                    options.method_path.display()
                )
            })?;
            fs::write(options.out_dir.join("input.md"), &method_text)?;
            write_json(
                &options.out_dir.join("config_snapshot.json"),
                &ConfigSnapshot {
                    style: options.style,
                    aspect: options.aspect,
                    target_width_mm: options.target_width_mm,
                    max_iterations: options.max_iterations,
                    max_cost_usd: options.max_cost_usd,
                    max_minutes: options.max_minutes,
                    image_provider: options.image_provider,
                    mock_models: options.mock_models,
                    keep_intermediate: options.keep_intermediate,
                },
            )?;
            append_log(&options.out_dir, "run started")?;
            if !options.mock_models {
                cost.reserve("reasoner initial plan", EST_REASONER_INITIAL_USD)?;
            }
            let plan = create_initial_plan(
                &method_text,
                options.style,
                options.aspect,
                options.target_width_mm,
                &config,
                options.mock_models,
            )?;
            (method_text, plan, None, None, 0, 0)
        }
    };

    let mut rounds_this_invocation = 0;
    loop {
        if options.max_iterations > 0 && rounds_this_invocation >= options.max_iterations {
            break;
        }
        if options.max_minutes > 0
            && start.elapsed() > Duration::from_secs(u64::from(options.max_minutes) * 60)
        {
            append_log(&options.out_dir, "time cap reached")?;
            cap_reason = Some("time cap reached".to_string());
            break;
        }

        let round_dir = options.out_dir.join(format!("round_{round_index:03}"));
        let revision_source_index =
            revision_source_round_index(round_index, best_round.as_ref().map(|(index, _)| *index));
        let revision_round_dir = revision_source_index
            .map(|source_index| options.out_dir.join(format!("round_{source_index:03}")));
        let revision_review = match revision_source_index {
            None => None,
            Some(source_index) => best_round
                .as_ref()
                .and_then(|(best_index, best_review)| {
                    (*best_index == source_index).then_some(best_review.clone())
                })
                .or_else(|| {
                    if source_index + 1 == round_index {
                        previous_review.clone()
                    } else {
                        None
                    }
                }),
        };
        if let Some(source_index) = revision_source_index {
            if revision_review.is_none() {
                return Err(anyhow!(
                    "missing review for revision source round {source_index}"
                ));
            }
            if source_index + 1 != round_index {
                append_log(
                    &options.out_dir,
                    &format!("round {round_index} revising from best-so-far round {source_index}"),
                )?;
            }
        }
        fs::create_dir_all(round_dir.join("assets"))?;
        canonicalize_plan_for_render(&mut plan, options.image_provider);
        normalize_plan_for_render(&mut plan);
        validate_plan_for_render(&plan, &style)?;
        write_json(&round_dir.join("figure_plan.json"), &plan)?;
        let mut draw_plan = if let (Some(revision_round_dir), Some(revision_review)) =
            (revision_round_dir.as_deref(), revision_review.as_ref())
        {
            if !options.mock_models {
                if let Err(error) = cost.reserve("vision DrawPlan optimization", EST_VISION_USD) {
                    let reason = error.to_string();
                    append_log(&options.out_dir, &reason)?;
                    cap_reason = Some(reason);
                    break;
                }
            }
            let previous_draw_plan: DrawPlan =
                read_json(&revision_round_dir.join("draw_plan.json"))
                    .context("failed to read revision source draw_plan.json")?;
            revise_draw_plan_from_feedback(
                &previous_draw_plan,
                revision_review,
                &read_text_or_empty(&revision_round_dir.join("layout_map.json"))?,
                &read_text_or_empty(&revision_round_dir.join("validation_report.json"))?,
                &previous_overlay_path(revision_round_dir, options.target_width_mm),
                &config,
                options.mock_models,
                round_index,
            )?
        } else {
            draw_plan_from_figure_plan(&plan, &style)
        };
        if options.mock_models {
            repair_draw_plan_geometry_with_figure_plan(&mut draw_plan, &plan);
        } else {
            polish_model_draw_plan_geometry_with_figure_plan(&mut draw_plan, &plan);
        }
        validate_draw_plan(&draw_plan)?;
        write_json(&round_dir.join("draw_plan.json"), &draw_plan)?;
        write_json(
            &round_dir.join("validation_report.json"),
            &RoundValidationReport {
                warnings: vec![],
                errors: vec![],
            },
        )?;
        let workspace = create_round_workspace(
            &round_dir,
            round_index,
            &method_text,
            revision_round_dir.as_deref(),
        )?;
        workspace.write_declared(
            "writable/design_brief.md",
            build_design_brief(&plan).as_bytes(),
        )?;
        workspace.write_declared(
            "writable/figure_plan.json",
            &serde_json::to_vec_pretty(&plan)?,
        )?;
        workspace.write_declared(
            "writable/draw_plan.json",
            &serde_json::to_vec_pretty(&draw_plan)?,
        )?;
        workspace.write_declared(
            "writable/asset_requests.json",
            &serde_json::to_vec_pretty(&plan.assets)?,
        )?;
        workspace.write_declared(
            "writable/renderer_notes.md",
            b"DrawPlan is the primary editable rendering contract for this round.\n",
        )?;

        if !options.mock_models && !matches!(options.image_provider, ImageProviderKind::None) {
            let asset_estimate = EST_IMAGE_ASSET_USD * plan.assets.len() as f64;
            if let Err(error) = cost.reserve("image asset generation", asset_estimate) {
                let reason = error.to_string();
                append_log(&options.out_dir, &reason)?;
                cap_reason = Some(reason);
                break;
            }
        }
        let asset_paths = materialize_assets(
            &plan,
            &options.out_dir,
            &round_dir,
            options.image_provider,
            &config,
            options.mock_models,
        )?;
        let request_generated_code_bundle =
            renderer_uses_generated_code_bundle(options.mock_models);
        if request_generated_code_bundle && !options.mock_models {
            if let Err(error) = cost.reserve("coder TypeScript generation", EST_CODER_USD) {
                let reason = error.to_string();
                append_log(&options.out_dir, &reason)?;
                cap_reason = Some(reason);
                break;
            }
        }
        let fallback_code = build_fallback_renderer_code(
            &plan,
            &draw_plan,
            &style,
            &round_dir,
            &renderer_root,
            &asset_paths,
        )?;
        let code_bundle = if !request_generated_code_bundle {
            append_log(
                &options.out_dir,
                "coder code generation skipped; deterministic DrawPlan runtime is primary renderer contract",
            )?;
            GeneratedCodeBundle::single_figure(
                fallback_code.clone(),
                "deterministic DrawPlan runtime selected as primary renderer contract; coder model skipped",
            )
        } else if let (Some(revision_round_dir), Some(revision_review)) =
            (revision_round_dir.as_deref(), revision_review.as_ref())
        {
            create_revised_code_bundle(
                &fs::read_to_string(revision_round_dir.join("figure.ts"))
                    .context("failed to read revision source generated figure.ts")?,
                &draw_plan,
                revision_review,
                &read_text_or_empty(&revision_round_dir.join("layout_map.json"))?,
                &read_text_or_empty(&revision_round_dir.join("validation_report.json"))?,
                &style,
                &round_dir,
                &renderer_root,
                &asset_paths,
                &config,
                options.mock_models,
                round_index,
            )?
        } else {
            create_initial_code_bundle(
                &draw_plan,
                &style,
                &round_dir,
                &renderer_root,
                &asset_paths,
                &config,
                options.mock_models,
            )?
        };
        let coder_used_deterministic_fallback =
            code_bundle_used_deterministic_fallback(&code_bundle);
        let written_code = write_generated_code_bundle(&workspace, &round_dir, &code_bundle)?;
        let renderer_code = select_renderer_code(
            options.mock_models,
            &written_code.entrypoint_code,
            &fallback_code,
        );
        let renderer_used_fallback = if let Some(fallback_code) = renderer_code.fallback_code {
            run_node_renderer_with_fallback(
                renderer_code.primary_code,
                fallback_code,
                &round_dir,
                &renderer_root,
                options.renderer_timeout,
                options.mock_models,
            )?;
            round_dir.join("figure.model_error.log").exists()
        } else {
            run_node_renderer(
                renderer_code.primary_code,
                &round_dir,
                &renderer_root,
                options.renderer_timeout,
                options.mock_models,
            )?;
            false
        };
        let used_fallback = renderer_used_fallback || coder_used_deterministic_fallback;
        write_json(
            &round_dir.join("renderer_status.json"),
            &RendererStatus {
                source: renderer_source(renderer_code.source_on_success, used_fallback).to_string(),
                used_fallback,
            },
        )?;
        export_round(&round_dir, options.target_width_mm, options.mock_models)?;

        if !options.mock_models {
            if let Err(error) = cost.reserve("vision review", EST_VISION_USD) {
                let reason = error.to_string();
                append_log(&options.out_dir, &reason)?;
                cap_reason = Some(reason);
                break;
            }
        }
        let mut review =
            review_rendered_figure(&plan, &round_dir, &config, options.mock_models, round_index)?;
        apply_plan_geometry_gate(&plan, &mut review);
        apply_render_quality_gate(&mut review, &round_dir.join("layout_map.json"))?;
        if used_fallback {
            reject_fallback_round(&mut review);
        }
        write_json(&round_dir.join("review.json"), &review)?;
        write_json(
            &round_dir.join("validation_report.json"),
            &RoundValidationReport {
                warnings: vec![],
                errors: review.blocking_issues.clone(),
            },
        )?;
        rounds_completed = count_rounds(&options.out_dir)?;
        if should_replace_best_review(best_round.as_ref().map(|(_, review)| review), &review) {
            best_round = Some((round_index, review.clone()));
        }

        if review.passed {
            finalize_from_round(&options.out_dir, &round_dir, &review, true, "accepted")?;
            append_log(&options.out_dir, "run accepted")?;
            return Ok(PipelineResult {
                accepted: true,
                rounds: count_rounds(&options.out_dir)?,
                run_dir: options.out_dir.clone(),
                final_dir: options.out_dir.join("final"),
                reason: "accepted".to_string(),
            });
        }
        previous_review = Some(review);
        round_index += 1;
        rounds_this_invocation += 1;
    }

    let (round_index, review) = best_round.ok_or_else(|| anyhow!("no round was produced"))?;
    let round_dir = options.out_dir.join(format!("round_{round_index:03}"));
    let reason = cap_reason.unwrap_or_else(|| "cap reached before acceptance".to_string());
    finalize_from_round(&options.out_dir, &round_dir, &review, false, &reason)?;
    Ok(PipelineResult {
        accepted: false,
        rounds: rounds_completed,
        run_dir: options.out_dir.clone(),
        final_dir: options.out_dir.join("final"),
        reason,
    })
}

pub fn resume_pipeline(run_dir: PathBuf) -> Result<PipelineResult> {
    let final_status = run_dir.join("final/status.json");
    if final_status.exists() {
        let status: FinalStatus = read_json(&final_status)?;
        if status.accepted {
            return Ok(PipelineResult {
                accepted: status.accepted,
                rounds: count_rounds(&run_dir)?,
                run_dir: run_dir.clone(),
                final_dir: run_dir.join("final"),
                reason: status.reason,
            });
        }
    }

    let config: ConfigSnapshot = read_json(&run_dir.join("config_snapshot.json"))?;
    let input = run_dir.join("input.md");
    let state = load_resume_state(&run_dir)?;
    run_pipeline_inner(
        RunOptions {
            method_path: input,
            out_dir: run_dir,
            style: config.style,
            aspect: config.aspect,
            target_width_mm: config.target_width_mm,
            max_iterations: config.max_iterations,
            max_cost_usd: config.max_cost_usd,
            max_minutes: config.max_minutes,
            image_provider: config.image_provider,
            mock_models: config.mock_models,
            keep_intermediate: config.keep_intermediate,
            renderer_timeout: Duration::from_secs(60),
        },
        Some(state),
    )
}

fn load_resume_state(run_dir: &Path) -> Result<ResumeState> {
    let method_text = fs::read_to_string(run_dir.join("input.md"))
        .with_context(|| format!("failed to read {}", run_dir.join("input.md").display()))?;
    let completed_indices = completed_round_indices(run_dir)?;
    if completed_indices.is_empty() {
        return Err(anyhow!(
            "cannot resume {}; no completed round with review.json was found",
            run_dir.display()
        ));
    }

    let mut best_round: Option<(u32, Review)> = None;
    let mut previous_review: Option<Review> = None;
    let mut latest_completed_index = 0;
    for index in completed_indices {
        let review: Review = read_json(&run_dir.join(format!("round_{index:03}/review.json")))?;
        if previous_review.is_none() || index > latest_completed_index {
            latest_completed_index = index;
            previous_review = Some(review.clone());
        }
        if should_replace_best_review(best_round.as_ref().map(|(_, review)| review), &review) {
            best_round = Some((index, review));
        }
    }

    let best_index = best_round
        .as_ref()
        .map(|(index, _)| *index)
        .ok_or_else(|| {
            anyhow!(
                "cannot resume {}; no best round was found",
                run_dir.display()
            )
        })?;
    let plan: FigurePlan =
        read_json(&run_dir.join(format!("round_{best_index:03}/figure_plan.json"))).with_context(
            || format!("failed to read best round figure_plan.json from round {best_index}"),
        )?;

    Ok(ResumeState {
        method_text,
        plan,
        best_round,
        previous_review,
        next_round_index: next_round_index(run_dir)?,
        rounds_completed: count_rounds(run_dir)?,
    })
}

pub fn build_fallback_renderer_code(
    _plan: &FigurePlan,
    draw_plan: &DrawPlan,
    style: &crate::style::StyleSpec,
    round_dir: &Path,
    renderer_root: &Path,
    asset_paths: &std::collections::BTreeMap<String, PathBuf>,
) -> Result<String> {
    generate_draw_plan_typescript(draw_plan, style, round_dir, renderer_root, asset_paths)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RendererCodeSelection<'a> {
    pub primary_code: &'a str,
    pub fallback_code: Option<&'a str>,
    pub source_on_success: &'static str,
}

pub fn select_renderer_code<'a>(
    mock_models: bool,
    generated_code: &'a str,
    deterministic_draw_plan_code: &'a str,
) -> RendererCodeSelection<'a> {
    if mock_models {
        RendererCodeSelection {
            primary_code: generated_code,
            fallback_code: Some(deterministic_draw_plan_code),
            source_on_success: "mock_generated_code",
        }
    } else {
        RendererCodeSelection {
            primary_code: generated_code,
            fallback_code: Some(deterministic_draw_plan_code),
            source_on_success: "model_generated_code",
        }
    }
}

pub fn renderer_uses_generated_code_bundle(_mock_models: bool) -> bool {
    true
}

pub fn should_replace_best_review(current: Option<&Review>, candidate: &Review) -> bool {
    let Some(current) = current else {
        return true;
    };
    review_rank(candidate) > review_rank(current)
}

pub fn revision_source_round_index(
    current_round_index: u32,
    best_round_index: Option<u32>,
) -> Option<u32> {
    if current_round_index == 0 {
        None
    } else {
        best_round_index.or(Some(current_round_index - 1))
    }
}

fn review_rank(review: &Review) -> (u8, u8, i32, u32, u8, u8, u8, u8, u8) {
    (
        u8::from(review.passed),
        u8::from(review.blocking_issues.is_empty()),
        -(review.blocking_issues.len() as i32),
        review_score_sum(review),
        review.scores.semantic_fidelity,
        review.scores.story_clarity,
        review.scores.layout_cleanliness,
        review.scores.arrow_routing,
        review.scores.wps_editability,
    )
}

fn review_score_sum(review: &Review) -> u32 {
    u32::from(review.scores.semantic_fidelity)
        + u32::from(review.scores.story_clarity)
        + u32::from(review.scores.visual_hierarchy)
        + u32::from(review.scores.paper_readability)
        + u32::from(review.scores.layout_cleanliness)
        + u32::from(review.scores.arrow_routing)
        + u32::from(review.scores.color_semantics)
        + u32::from(review.scores.aesthetic_quality)
        + u32::from(review.scores.wps_editability)
}

fn finalize_from_round(
    run_dir: &Path,
    round_dir: &Path,
    review: &Review,
    accepted: bool,
    reason: &str,
) -> Result<()> {
    let final_dir = run_dir.join("final");
    if final_dir.exists() {
        fs::remove_dir_all(&final_dir)?;
    }
    fs::create_dir_all(final_dir.join("assets"))?;
    for file in [
        "figure.pptx",
        "figure.pdf",
        "figure.png",
        "figure.ts",
        "helpers.ts",
        "figure_plan.json",
        "draw_plan.json",
        "review.json",
        "layout_map.json",
        "validation_report.json",
        "renderer_status.json",
    ] {
        let src = round_dir.join(file);
        if src.exists() {
            fs::copy(src, final_dir.join(file))?;
        }
    }
    let assets_dir = round_dir.join("assets");
    if assets_dir.exists() {
        for entry in fs::read_dir(assets_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                fs::copy(
                    entry.path(),
                    final_dir.join("assets").join(entry.file_name()),
                )?;
            }
        }
    }
    write_json(
        &final_dir.join("status.json"),
        &FinalStatus {
            accepted,
            reason: reason.to_string(),
            review_passed: review.passed,
        },
    )?;
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct FinalStatus {
    accepted: bool,
    reason: String,
    review_passed: bool,
}

fn count_rounds(run_dir: &Path) -> Result<u32> {
    Ok(round_indices(run_dir)?.len() as u32)
}

fn next_round_index(run_dir: &Path) -> Result<u32> {
    Ok(round_indices(run_dir)?
        .into_iter()
        .max()
        .map(|index| index + 1)
        .unwrap_or(0))
}

fn completed_round_indices(run_dir: &Path) -> Result<Vec<u32>> {
    let mut indices = Vec::new();
    for index in round_indices(run_dir)? {
        if run_dir
            .join(format!("round_{index:03}/review.json"))
            .exists()
        {
            indices.push(index);
        }
    }
    Ok(indices)
}

fn round_indices(run_dir: &Path) -> Result<Vec<u32>> {
    let mut indices = Vec::new();
    if !run_dir.exists() {
        return Ok(indices);
    }
    for entry in fs::read_dir(run_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Some(index) = name
            .strip_prefix("round_")
            .and_then(|raw| raw.parse::<u32>().ok())
        else {
            continue;
        };
        indices.push(index);
    }
    indices.sort_unstable();
    Ok(indices)
}

fn read_text_or_empty(path: &Path) -> Result<String> {
    if path.exists() {
        Ok(fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?)
    } else {
        Ok(String::new())
    }
}

fn renderer_source(source_on_success: &'static str, used_fallback: bool) -> &'static str {
    if used_fallback {
        "deterministic_fallback"
    } else {
        source_on_success
    }
}

fn code_bundle_used_deterministic_fallback(bundle: &GeneratedCodeBundle) -> bool {
    bundle.notes.contains("deterministic DrawPlan runtime used")
}

fn previous_overlay_path(previous_round_dir: &Path, target_width_mm: u32) -> PathBuf {
    let overlay = previous_round_dir.join("figure_review_overlay.png");
    if overlay.exists() {
        overlay
    } else {
        previous_round_dir.join(format!("figure_{target_width_mm}mm_preview.png"))
    }
}

fn reject_fallback_round(review: &mut Review) {
    let issue =
        "model generated TypeScript failed; deterministic fallback rendered this round".to_string();
    if !review.blocking_issues.contains(&issue) {
        review.blocking_issues.push(issue);
    }
    review.passed = false;
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

fn create_round_workspace(
    round_dir: &Path,
    round_index: u32,
    method_text: &str,
    previous_round_dir: Option<&Path>,
) -> Result<AgentWorkspace> {
    let mut readable = vec![
        workspace_file(
            "readable/input.md",
            "Original method description for this run.",
            WorkspaceFileFormat::Markdown,
            256_000,
        ),
        workspace_file(
            "readable/method_templates.json",
            "Classic method-overview template pack derived from extracted PDF/SVG figures.",
            WorkspaceFileFormat::Json,
            512_000,
        ),
    ];
    if round_index > 0 {
        readable.extend([
            workspace_file(
                "readable/previous_figure_plan.json",
                "Previous round semantic figure plan.",
                WorkspaceFileFormat::Json,
                512_000,
            ),
            workspace_file(
                "readable/previous_draw_plan.json",
                "Previous round drawable primitive plan.",
                WorkspaceFileFormat::Json,
                512_000,
            ),
            workspace_file(
                "readable/previous_layout_map.json",
                "Previous round rendered object bounding boxes.",
                WorkspaceFileFormat::Json,
                512_000,
            ),
            workspace_file(
                "readable/previous_review.json",
                "Previous round vision review.",
                WorkspaceFileFormat::Json,
                512_000,
            ),
            workspace_file(
                "readable/previous_validation_report.json",
                "Previous round local validation report.",
                WorkspaceFileFormat::Json,
                512_000,
            ),
            workspace_file(
                "readable/previous_code/figure.ts",
                "Previous round generated renderer entrypoint.",
                WorkspaceFileFormat::Typescript,
                1_000_000,
            ),
        ]);
        if previous_round_dir
            .map(|dir| dir.join("helpers.ts").exists())
            .unwrap_or(false)
        {
            readable.push(workspace_file(
                "readable/previous_code/helpers.ts",
                "Previous round generated renderer helper module.",
                WorkspaceFileFormat::Typescript,
                1_000_000,
            ));
        }
    }
    let writable = vec![
        workspace_file(
            "writable/design_brief.md",
            "Model-authored design rationale and reading order for this round.",
            WorkspaceFileFormat::Markdown,
            256_000,
        ),
        workspace_file(
            "writable/figure_plan.json",
            "Model-authored semantic FigurePlan for this round.",
            WorkspaceFileFormat::Json,
            512_000,
        ),
        workspace_file(
            "writable/draw_plan.json",
            "Model-authored or agent-derived editable primitive DrawPlan.",
            WorkspaceFileFormat::Json,
            512_000,
        ),
        workspace_file(
            "writable/asset_requests.json",
            "Small local image asset requests only.",
            WorkspaceFileFormat::Json,
            256_000,
        ),
        workspace_file(
            "writable/renderer_notes.md",
            "Renderer assumptions, fallback notes, and repair hints.",
            WorkspaceFileFormat::Markdown,
            128_000,
        ),
        workspace_file(
            ENTRYPOINT_WORKSPACE_PATH,
            "Generated renderer entrypoint for this round.",
            WorkspaceFileFormat::Typescript,
            1_000_000,
        ),
        workspace_file(
            HELPER_WORKSPACE_PATH,
            "Optional generated renderer helper module for this round.",
            WorkspaceFileFormat::Typescript,
            1_000_000,
        ),
    ];
    let workspace = AgentWorkspace::create(round_dir, WorkspaceManifest { readable, writable })?;
    workspace.write_readable("readable/input.md", method_text.as_bytes())?;
    workspace.write_readable(
        "readable/method_templates.json",
        method_template_pack_json()?.as_bytes(),
    )?;
    if let Some(previous_round_dir) = previous_round_dir {
        write_previous_readable(
            &workspace,
            previous_round_dir,
            "figure_plan.json",
            "readable/previous_figure_plan.json",
        )?;
        write_previous_readable(
            &workspace,
            previous_round_dir,
            "draw_plan.json",
            "readable/previous_draw_plan.json",
        )?;
        write_previous_readable(
            &workspace,
            previous_round_dir,
            "layout_map.json",
            "readable/previous_layout_map.json",
        )?;
        write_previous_readable(
            &workspace,
            previous_round_dir,
            "review.json",
            "readable/previous_review.json",
        )?;
        write_previous_readable(
            &workspace,
            previous_round_dir,
            "validation_report.json",
            "readable/previous_validation_report.json",
        )?;
        write_previous_readable(
            &workspace,
            previous_round_dir,
            "figure.ts",
            "readable/previous_code/figure.ts",
        )?;
        if previous_round_dir.join("helpers.ts").exists() {
            write_previous_readable(
                &workspace,
                previous_round_dir,
                "helpers.ts",
                "readable/previous_code/helpers.ts",
            )?;
        }
    }
    Ok(workspace)
}

fn write_previous_readable(
    workspace: &AgentWorkspace,
    previous_round_dir: &Path,
    source_name: &str,
    workspace_path: &str,
) -> Result<()> {
    let bytes = fs::read(previous_round_dir.join(source_name)).with_context(|| {
        format!(
            "failed to read previous round artifact {}",
            previous_round_dir.join(source_name).display()
        )
    })?;
    workspace.write_readable(workspace_path, &bytes)
}

fn workspace_file(
    path: &str,
    purpose: &str,
    format: WorkspaceFileFormat,
    max_bytes: u64,
) -> WorkspaceFile {
    WorkspaceFile {
        path: path.to_string(),
        purpose: purpose.to_string(),
        format,
        max_bytes,
    }
}

fn build_design_brief(plan: &FigurePlan) -> String {
    format!(
        "# Design Brief\n\nMain message: {}\n\nReading order: {:?}\n\nVisual focus:\n{}\n",
        plan.story.main_message,
        plan.story.reading_order,
        plan.story
            .visual_focus
            .iter()
            .map(|focus| format!("- {focus}"))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let bytes = fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn append_log(run_dir: &Path, message: &str) -> Result<()> {
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(run_dir.join("run.log"))?;
    writeln!(file, "{message}")?;
    Ok(())
}
