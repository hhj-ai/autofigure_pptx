use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use crate::agent::{
    create_initial_code_bundle, create_initial_plan, create_reference_selection,
    create_revised_code_bundle, create_round_improvement_plan, review_rendered_figure,
    revise_draw_plan_from_feedback, validate_required_model_config,
};
use crate::config::AppConfig;
use crate::schema::{
    validate_draw_plan, CanvasAspect, DrawObject, DrawPlan, EdgeStyle, FigurePlan,
    ImageProviderKind, ReferencePreviewMode, ReferenceSelection, Review, StyleName,
};
use crate::style::style_by_name;
use crate::tools::asset_gen::materialize_assets;
use crate::tools::canonicalize::canonicalize_plan_for_render;
use crate::tools::cost::{
    CostTracker, EST_CODER_USD, EST_IMAGE_ASSET_USD, EST_REASONER_INITIAL_USD,
    EST_REASONER_PATCH_USD, EST_VISION_USD,
};
use crate::tools::draw_plan::{
    draw_plan_from_figure_plan, draw_plan_material_changes, generate_draw_plan_typescript,
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
use crate::tools::review::{
    apply_plan_geometry_gate, build_quality_report, QualityIssue, QualityReport,
};
use crate::tools::template_library::{method_template_pack_json, select_reference_for_method};
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
    pub reference_previews: ReferencePreviewMode,
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
    #[serde(default = "default_reference_previews")]
    reference_previews: ReferencePreviewMode,
    mock_models: bool,
    keep_intermediate: bool,
}

fn default_reference_previews() -> ReferencePreviewMode {
    ReferencePreviewMode::Auto
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

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RepairReport {
    version: String,
    round_index: u32,
    source_round_index: Option<u32>,
    material_changes: Vec<String>,
    notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RegressionReport {
    version: String,
    round_index: u32,
    source_round_index: Option<u32>,
    status: String,
    source_quality_score: Option<u32>,
    quality_score: u32,
    score_delta: Option<i32>,
    blocking_delta: Option<i32>,
    major_delta: Option<i32>,
    issue_delta: Option<i32>,
    regressed_issue_types: Vec<String>,
    resolved_issue_types: Vec<String>,
    budget: RegressionBudget,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RegressionBudget {
    min_quality_score: u32,
    max_blocking_issues: usize,
    max_major_issues: usize,
    requirement: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct IssueBinding {
    version: String,
    round_index: u32,
    entries: Vec<IssueBindingEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct IssueBindingEntry {
    issue_id: String,
    issue_type: String,
    severity: String,
    source: String,
    target_ids: Vec<String>,
    evidence: String,
    suggested_action: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct IssueHistory {
    version: String,
    round_index: u32,
    issues: Vec<TrackedIssue>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TrackedIssue {
    issue_key: String,
    issue_type: String,
    target_ids: Vec<String>,
    first_seen_round: u32,
    last_seen_round: u32,
    occurrences: u32,
    latest_evidence: String,
    latest_suggested_action: String,
}

pub fn run_pipeline(options: RunOptions) -> Result<PipelineResult> {
    run_pipeline_inner(options, None)
}

#[derive(Clone, Debug)]
struct ResumeState {
    method_text: String,
    plan: FigurePlan,
    reference_selection: ReferenceSelection,
    best_round: Option<BestRound>,
    previous_review: Option<Review>,
    next_round_index: u32,
    rounds_completed: u32,
    rounds_this_invocation_offset: u32,
}

#[derive(Clone, Debug)]
struct BestRound {
    index: u32,
    review: Review,
    quality_report: QualityReport,
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
        reference_selection,
        mut best_round,
        mut previous_review,
        mut rounds_completed,
        mut round_index,
        mut rounds_this_invocation,
    ) = match resume_state {
        Some(state) => {
            append_log(
                &options.out_dir,
                &format!("run resumed at round {}", state.next_round_index),
            )?;
            (
                state.method_text,
                state.plan,
                state.reference_selection,
                state.best_round,
                state.previous_review,
                state.rounds_completed,
                state.next_round_index,
                state.rounds_this_invocation_offset,
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
                    reference_previews: options.reference_previews,
                    mock_models: options.mock_models,
                    keep_intermediate: options.keep_intermediate,
                },
            )?;
            append_log(&options.out_dir, "run started")?;
            if !options.mock_models {
                cost.reserve("reasoner reference selection", EST_REASONER_PATCH_USD)?;
            }
            let reference_selection = create_reference_selection(
                &method_text,
                options.reference_previews,
                &config,
                options.mock_models,
            )?;
            write_json(
                &options.out_dir.join("reference_selection.json"),
                &reference_selection,
            )?;
            if !options.mock_models {
                cost.reserve("reasoner initial plan", EST_REASONER_INITIAL_USD)?;
            }
            let plan = create_initial_plan(
                &method_text,
                options.style,
                options.aspect,
                options.target_width_mm,
                &reference_selection,
                &config,
                options.mock_models,
            )?;
            (method_text, plan, reference_selection, None, None, 0, 0, 0)
        }
    };
    let reference_preview_path =
        resolve_reference_preview_path(&reference_selection, options.reference_previews)?;

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
            revision_source_round_index(round_index, best_round.as_ref().map(|best| best.index));
        let revision_round_dir = revision_source_index
            .map(|source_index| options.out_dir.join(format!("round_{source_index:03}")));
        let latest_attempt_index = round_index.checked_sub(1);
        let latest_attempt_round_dir = latest_attempt_index
            .filter(|latest_index| Some(*latest_index) != revision_source_index)
            .map(|latest_index| options.out_dir.join(format!("round_{latest_index:03}")));
        let revision_regression_context = build_revision_regression_context(
            revision_source_index,
            revision_round_dir.as_deref(),
            latest_attempt_index,
            latest_attempt_round_dir.as_deref(),
        )?;
        let revision_review = match revision_source_index {
            None => None,
            Some(source_index) => best_round
                .as_ref()
                .and_then(|best| (best.index == source_index).then_some(best.review.clone()))
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
        write_json(
            &round_dir.join("reference_selection.json"),
            &reference_selection,
        )?;
        let revision_source_draw_plan =
            if let Some(revision_round_dir) = revision_round_dir.as_deref() {
                Some(
                    read_json::<DrawPlan>(&revision_round_dir.join("draw_plan.json"))
                        .context("failed to read revision source draw_plan.json")?,
                )
            } else {
                None
            };
        let mut draw_plan =
            if let (Some(revision_round_dir), Some(revision_review), Some(previous_draw_plan)) = (
                revision_round_dir.as_deref(),
                revision_review.as_ref(),
                revision_source_draw_plan.as_ref(),
            ) {
                if !options.mock_models {
                    if let Err(error) = cost.reserve("vision DrawPlan optimization", EST_VISION_USD)
                    {
                        let reason = error.to_string();
                        append_log(&options.out_dir, &reason)?;
                        cap_reason = Some(reason);
                        break;
                    }
                }
                let previous_improvement_plan = select_revision_improvement_plan(
                    revision_round_dir,
                    latest_attempt_round_dir.as_deref(),
                )?;
                revise_draw_plan_from_feedback(
                    previous_draw_plan,
                    revision_review,
                    &read_text_or_empty(&revision_round_dir.join("layout_map.json"))?,
                    &read_text_or_empty(&revision_round_dir.join("validation_report.json"))?,
                    &read_text_or_empty(&revision_round_dir.join("quality_report.json"))?,
                    &read_text_or_empty(&revision_round_dir.join("issue_history.json"))?,
                    &read_text_or_empty(&revision_round_dir.join("issue_binding.json"))?,
                    &revision_regression_context,
                    &read_text_or_empty(&revision_round_dir.join("figure.ts"))?,
                    &previous_overlay_path(revision_round_dir, options.target_width_mm),
                    &reference_selection,
                    &previous_improvement_plan,
                    reference_preview_path.as_deref(),
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
        let repair_report = build_repair_report(
            round_index,
            revision_source_index,
            revision_source_draw_plan.as_ref(),
            &draw_plan,
        );
        write_json(&round_dir.join("repair_report.json"), &repair_report)?;
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
            &reference_selection,
            revision_round_dir.as_deref(),
            latest_attempt_round_dir.as_deref(),
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
                &read_text_or_empty(&revision_round_dir.join("quality_report.json"))?,
                &read_text_or_empty(&revision_round_dir.join("issue_history.json"))?,
                &read_text_or_empty(&revision_round_dir.join("issue_binding.json"))?,
                &revision_regression_context,
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
        let mut review = review_rendered_figure(
            &plan,
            &round_dir,
            &reference_selection,
            reference_preview_path.as_deref(),
            &config,
            options.mock_models,
            round_index,
        )?;
        apply_plan_geometry_gate(&plan, &mut review);
        let mut quality_report = build_quality_report(&round_dir.join("layout_map.json"))?;
        inject_draw_plan_semantic_quality_issues(&mut quality_report, &plan, &draw_plan);
        write_json(&round_dir.join("quality_report.json"), &quality_report)?;
        let source_quality_report = revision_round_dir
            .as_deref()
            .map(|dir| read_json::<QualityReport>(&dir.join("quality_report.json")))
            .transpose()?;
        let regression_report = build_regression_report(
            round_index,
            revision_source_index,
            source_quality_report.as_ref(),
            &quality_report,
        );
        write_json(
            &round_dir.join("regression_report.json"),
            &regression_report,
        )?;
        apply_quality_report_gate(&mut review, &quality_report);
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
        let issue_binding = build_issue_binding(round_index, &review, &quality_report);
        write_json(&round_dir.join("issue_binding.json"), &issue_binding)?;
        let issue_history =
            build_issue_history(round_index, revision_round_dir.as_deref(), &issue_binding)?;
        write_json(&round_dir.join("issue_history.json"), &issue_history)?;
        if !options.mock_models {
            if let Err(error) =
                cost.reserve("reasoner round improvement plan", EST_REASONER_PATCH_USD)
            {
                let reason = error.to_string();
                append_log(&options.out_dir, &reason)?;
                cap_reason = Some(reason);
                break;
            }
        }
        let improvement_plan = create_round_improvement_plan(
            &review,
            &read_text_or_empty(&round_dir.join("layout_map.json"))?,
            &read_text_or_empty(&round_dir.join("validation_report.json"))?,
            &read_text_or_empty(&round_dir.join("quality_report.json"))?,
            &read_text_or_empty(&round_dir.join("issue_history.json"))?,
            &read_text_or_empty(&round_dir.join("issue_binding.json"))?,
            &read_text_or_empty(&round_dir.join("regression_report.json"))?,
            &reference_selection,
            &config,
            options.mock_models,
            round_index,
        )?;
        write_json(&round_dir.join("improvement_plan.json"), &improvement_plan)?;
        rounds_completed = count_rounds(&options.out_dir)?;
        let candidate_best_round = BestRound {
            index: round_index,
            review: review.clone(),
            quality_report: quality_report.clone(),
        };
        if should_replace_best_round(best_round.as_ref(), &candidate_best_round) {
            best_round = Some(candidate_best_round);
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

    let best_round = best_round.ok_or_else(|| anyhow!("no round was produced"))?;
    let round_dir = options
        .out_dir
        .join(format!("round_{:03}", best_round.index));
    let reason = cap_reason.unwrap_or_else(|| "cap reached before acceptance".to_string());
    finalize_from_round(
        &options.out_dir,
        &round_dir,
        &best_round.review,
        false,
        &reason,
    )?;
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
    let resume_after_final = final_status.exists();
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
    let state = load_resume_state(&run_dir, resume_after_final)?;
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
            reference_previews: config.reference_previews,
            mock_models: config.mock_models,
            keep_intermediate: config.keep_intermediate,
            renderer_timeout: Duration::from_secs(60),
        },
        Some(state),
    )
}

fn load_resume_state(run_dir: &Path, resume_after_final: bool) -> Result<ResumeState> {
    let method_text = fs::read_to_string(run_dir.join("input.md"))
        .with_context(|| format!("failed to read {}", run_dir.join("input.md").display()))?;
    let completed_indices = completed_round_indices(run_dir)?;
    if completed_indices.is_empty() {
        return Err(anyhow!(
            "cannot resume {}; no completed round with review.json was found",
            run_dir.display()
        ));
    }

    let mut best_round: Option<BestRound> = None;
    let mut previous_review: Option<Review> = None;
    let mut latest_completed_index = 0;
    for index in completed_indices {
        let round_dir = run_dir.join(format!("round_{index:03}"));
        let review: Review = read_json(&round_dir.join("review.json"))?;
        let quality_report: QualityReport = read_json(&round_dir.join("quality_report.json"))
            .with_context(|| {
                format!("failed to read quality_report.json from completed round {index}")
            })?;
        if previous_review.is_none() || index > latest_completed_index {
            latest_completed_index = index;
            previous_review = Some(review.clone());
        }
        let candidate_best_round = BestRound {
            index,
            review,
            quality_report,
        };
        if should_replace_best_round(best_round.as_ref(), &candidate_best_round) {
            best_round = Some(candidate_best_round);
        }
    }

    let best_index = best_round.as_ref().map(|best| best.index).ok_or_else(|| {
        anyhow!(
            "cannot resume {}; no best round was found",
            run_dir.display()
        )
    })?;
    let plan: FigurePlan =
        read_json(&run_dir.join(format!("round_{best_index:03}/figure_plan.json"))).with_context(
            || format!("failed to read best round figure_plan.json from round {best_index}"),
        )?;
    let reference_selection = if run_dir.join("reference_selection.json").exists() {
        read_json(&run_dir.join("reference_selection.json"))?
    } else {
        select_reference_for_resume(&method_text)?
    };

    let rounds_completed = count_rounds(run_dir)?;
    Ok(ResumeState {
        method_text,
        plan,
        reference_selection,
        best_round,
        previous_review,
        next_round_index: next_round_index(run_dir)?,
        rounds_completed,
        rounds_this_invocation_offset: if resume_after_final {
            0
        } else {
            rounds_completed
        },
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

pub fn should_replace_best_round_quality(
    current: Option<(&Review, &QualityReport)>,
    candidate: (&Review, &QualityReport),
) -> bool {
    let Some((current_review, current_quality)) = current else {
        return true;
    };
    best_round_rank(candidate.0, candidate.1) > best_round_rank(current_review, current_quality)
}

fn should_replace_best_round(current: Option<&BestRound>, candidate: &BestRound) -> bool {
    should_replace_best_round_quality(
        current.map(|best| (&best.review, &best.quality_report)),
        (&candidate.review, &candidate.quality_report),
    )
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

fn build_revision_regression_context(
    revision_source_index: Option<u32>,
    revision_round_dir: Option<&Path>,
    latest_attempt_index: Option<u32>,
    latest_attempt_round_dir: Option<&Path>,
) -> Result<String> {
    let Some(revision_round_dir) = revision_round_dir else {
        return Ok(String::new());
    };
    let source_report =
        read_json_value_if_exists(&revision_round_dir.join("regression_report.json"))?;
    let source_renderer_error =
        read_text_or_empty(&revision_round_dir.join("figure.model_error.log"))?;
    let Some(latest_attempt_round_dir) = latest_attempt_round_dir else {
        return Ok(serde_json::to_string_pretty(&serde_json::json!({
            "version": "0.1",
            "revision_source_round_index": revision_source_index,
            "revision_source_regression_report": source_report,
            "revision_source_renderer_error": source_renderer_error,
            "instruction": "Treat revision_source_regression_report.budget as the hard floor. If revision_source_renderer_error is non-empty, fix the generated TypeScript by using only createDrawPlanRuntimeFromEnv() and runtime.renderDrawPlan(); do not repeat unsupported runtime API calls."
        }))?);
    };

    // 回滚到 best-so-far 时，仍把最新失败尝试作为负例证据放进上下文，避免下一轮重复同一套坏策略。
    serde_json::to_string_pretty(&serde_json::json!({
        "version": "0.1",
        "revision_source_round_index": revision_source_index,
        "latest_attempt_round_index": latest_attempt_index,
        "revision_source_regression_report": source_report,
        "revision_source_renderer_error": source_renderer_error,
        "latest_attempt_review": read_json_value_if_exists(&latest_attempt_round_dir.join("review.json"))?,
        "latest_attempt_quality_report": read_json_value_if_exists(&latest_attempt_round_dir.join("quality_report.json"))?,
        "latest_attempt_regression_report": read_json_value_if_exists(&latest_attempt_round_dir.join("regression_report.json"))?,
        "latest_attempt_issue_binding": read_json_value_if_exists(&latest_attempt_round_dir.join("issue_binding.json"))?,
        "latest_attempt_issue_history": read_json_value_if_exists(&latest_attempt_round_dir.join("issue_history.json"))?,
        "latest_attempt_improvement_plan": read_json_value_if_exists(&latest_attempt_round_dir.join("improvement_plan.json"))?,
        "latest_attempt_renderer_error": read_text_or_empty(&latest_attempt_round_dir.join("figure.model_error.log"))?,
        "latest_attempt_figure_ts": read_text_or_empty(&latest_attempt_round_dir.join("figure.ts"))?,
        "instruction": "Treat revision_source_regression_report.budget as the hard floor. Treat latest_attempt_* as negative evidence: identify what changed in the latest attempt, do not repeat its regressed issue types, and keep only local fixes that directly address current unresolved target ids. Never fix a local issue by adding edge_crossing, annotation_in_main_corridor, duplicate input lanes, or a standalone inference lane. If any renderer_error field is non-empty, fix the generated TypeScript by using only createDrawPlanRuntimeFromEnv() and runtime.renderDrawPlan(); do not repeat unsupported runtime API calls."
    }))
    .context("failed to build revision regression context")
}

fn read_json_value_if_exists(path: &Path) -> Result<serde_json::Value> {
    if !path.exists() {
        return Ok(serde_json::Value::Null);
    }
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read JSON context {}", path.display()))?;
    serde_json::from_str(&text)
        .with_context(|| format!("failed to parse JSON context {}", path.display()))
}

fn best_round_rank(
    review: &Review,
    quality_report: &QualityReport,
) -> (
    u8,
    i32,
    i32,
    u32,
    i32,
    (u8, u8, i32, u32, u8, u8, u8, u8, u8),
) {
    (
        u8::from(quality_report.passed),
        -(quality_issue_count(quality_report, "blocking") as i32),
        -(quality_issue_count(quality_report, "major") as i32),
        quality_report.score,
        -(quality_report.issues.len() as i32),
        review_rank(review),
    )
}

fn build_regression_report(
    round_index: u32,
    source_round_index: Option<u32>,
    source: Option<&QualityReport>,
    current: &QualityReport,
) -> RegressionReport {
    let source_score = source.map(|report| report.score);
    let score_delta = source.map(|report| current.score as i32 - report.score as i32);
    let blocking_delta = source.map(|report| {
        quality_issue_count(current, "blocking") as i32
            - quality_issue_count(report, "blocking") as i32
    });
    let major_delta = source.map(|report| {
        quality_issue_count(current, "major") as i32 - quality_issue_count(report, "major") as i32
    });
    let issue_delta = source.map(|report| current.issues.len() as i32 - report.issues.len() as i32);
    let status = match source {
        None => "initial".to_string(),
        Some(_) if blocking_delta.unwrap_or_default() > 0 => "regressed".to_string(),
        Some(_) if major_delta.unwrap_or_default() > 0 => "regressed".to_string(),
        Some(_) if score_delta.unwrap_or_default() < 0 => "regressed".to_string(),
        Some(_)
            if blocking_delta.unwrap_or_default() < 0
                || major_delta.unwrap_or_default() < 0
                || score_delta.unwrap_or_default() > 0
                || issue_delta.unwrap_or_default() < 0 =>
        {
            "improved".to_string()
        }
        Some(_) => "same".to_string(),
    };
    let (regressed_issue_types, resolved_issue_types) = source
        .map(|report| issue_type_deltas(report, current))
        .unwrap_or_default();
    let min_quality_score = source.map(|report| report.score).unwrap_or(current.score);
    let max_blocking_issues = source
        .map(|report| quality_issue_count(report, "blocking"))
        .unwrap_or_else(|| quality_issue_count(current, "blocking"));
    let max_major_issues = source
        .map(|report| quality_issue_count(report, "major"))
        .unwrap_or_else(|| quality_issue_count(current, "major"));

    RegressionReport {
        version: "0.1".to_string(),
        round_index,
        source_round_index,
        status,
        source_quality_score: source_score,
        quality_score: current.score,
        score_delta,
        blocking_delta,
        major_delta,
        issue_delta,
        regressed_issue_types,
        resolved_issue_types,
        budget: RegressionBudget {
            min_quality_score,
            max_blocking_issues,
            max_major_issues,
            requirement:
                "Next round must not reduce quality_report.score or increase blocking/major issue counts relative to the revision source; if this round regressed, revert to the source geometry and apply only local fixes for unresolved target ids."
                    .to_string(),
        },
    }
}

fn issue_type_deltas(
    source: &QualityReport,
    current: &QualityReport,
) -> (Vec<String>, Vec<String>) {
    let source_counts = issue_type_counts(source);
    let current_counts = issue_type_counts(current);
    let mut keys = source_counts
        .keys()
        .chain(current_counts.keys())
        .cloned()
        .collect::<Vec<_>>();
    keys.sort();
    keys.dedup();
    let mut regressed = Vec::new();
    let mut resolved = Vec::new();
    for key in keys {
        let before = source_counts.get(&key).copied().unwrap_or(0);
        let after = current_counts.get(&key).copied().unwrap_or(0);
        if after > before {
            regressed.push(key);
        } else if after < before {
            resolved.push(key);
        }
    }
    (regressed, resolved)
}

fn issue_type_counts(report: &QualityReport) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for issue in report
        .issues
        .iter()
        .filter(|issue| issue.severity == "blocking" || issue.severity == "major")
    {
        *counts.entry(issue.issue_type.clone()).or_insert(0) += 1;
    }
    counts
}

fn quality_issue_count(quality_report: &QualityReport, severity: &str) -> usize {
    quality_report
        .issues
        .iter()
        .filter(|issue| issue.severity == severity)
        .count()
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
        "reference_selection.json",
        "improvement_plan.json",
        "quality_report.json",
        "regression_report.json",
        "issue_binding.json",
        "issue_history.json",
        "repair_report.json",
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
    Ok(completed_round_indices(run_dir)?.len() as u32)
}

fn next_round_index(run_dir: &Path) -> Result<u32> {
    Ok(completed_round_indices(run_dir)?
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

fn resolve_reference_preview_path(
    selection: &ReferenceSelection,
    mode: ReferencePreviewMode,
) -> Result<Option<PathBuf>> {
    if matches!(mode, ReferencePreviewMode::Off) {
        return Ok(None);
    }
    let Some(path) = selection.preview_path.as_ref().map(PathBuf::from) else {
        if matches!(mode, ReferencePreviewMode::Required) {
            return Err(anyhow!(
                "reference preview is required but selected reference has no preview_path"
            ));
        }
        return Ok(None);
    };
    if path.exists() {
        Ok(Some(path))
    } else if matches!(mode, ReferencePreviewMode::Required) {
        Err(anyhow!(
            "reference preview is required but {} does not exist; run scripts/extract_reference_previews.sh first",
            path.display()
        ))
    } else {
        Ok(None)
    }
}

fn select_reference_for_resume(method_text: &str) -> Result<ReferenceSelection> {
    select_reference_for_method(method_text, ReferencePreviewMode::Auto)
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

fn build_repair_report(
    round_index: u32,
    source_round_index: Option<u32>,
    previous_draw_plan: Option<&DrawPlan>,
    current_draw_plan: &DrawPlan,
) -> RepairReport {
    let material_changes = previous_draw_plan
        .map(|previous| draw_plan_material_changes(previous, current_draw_plan))
        .unwrap_or_default();
    let notes = if previous_draw_plan.is_some() {
        if material_changes.is_empty() {
            vec!["No material DrawPlan change detected after geometry repair.".to_string()]
        } else {
            vec![
                "Round is an incremental repair from the selected source round.".to_string(),
                "Material changes are object-level evidence for the next coder/reasoner pass."
                    .to_string(),
            ]
        }
    } else {
        vec!["Initial DrawPlan generated from FigurePlan.".to_string()]
    };
    RepairReport {
        version: "0.1".to_string(),
        round_index,
        source_round_index,
        material_changes,
        notes,
    }
}

fn inject_draw_plan_semantic_quality_issues(
    quality_report: &mut QualityReport,
    figure_plan: &FigurePlan,
    draw_plan: &DrawPlan,
) {
    let draw_connector_styles = draw_plan
        .objects
        .iter()
        .filter_map(|object| match object {
            DrawObject::Connector { id, style, .. } => Some((id.as_str(), style.as_str())),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();

    for edge in &figure_plan.edges {
        let Some(draw_style) = draw_connector_styles.get(edge.id.as_str()) else {
            continue;
        };
        let expected_dash = figure_edge_style_uses_dash(edge.style);
        let actual_dash = draw_style_uses_dash(draw_style);
        if expected_dash == actual_dash {
            continue;
        }
        append_quality_issue(
            quality_report,
            "edge_style_mismatch",
            "major",
            vec![edge.id.clone()],
            format!(
                "render quality failed: connector {} uses draw style '{}' but FigurePlan specifies {}",
                edge.id,
                draw_style,
                figure_edge_style_name(edge.style)
            ),
            "Change the DrawPlan connector style to match the FigurePlan edge style; preserve semantic endpoints and routing while fixing only the stroke style.",
        );
    }
    recompute_quality_report_status(quality_report);
}

fn apply_quality_report_gate(review: &mut Review, quality_report: &QualityReport) {
    for issue in &quality_report.issues {
        if issue.severity != "blocking" && issue.severity != "major" {
            continue;
        }
        if !review.blocking_issues.contains(&issue.evidence) {
            review.blocking_issues.push(issue.evidence.clone());
        }
    }
    review.passed = crate::tools::review::review_passes_threshold(review);
}

fn append_quality_issue(
    quality_report: &mut QualityReport,
    issue_type: &str,
    severity: &str,
    target_ids: Vec<String>,
    evidence: String,
    suggested_action: &str,
) {
    let issue_id = format!("quality_{:03}", quality_report.issues.len() + 1);
    quality_report.issues.push(QualityIssue {
        issue_id,
        issue_type: issue_type.to_string(),
        severity: severity.to_string(),
        target_ids,
        evidence,
        suggested_action: suggested_action.to_string(),
    });
}

fn recompute_quality_report_status(quality_report: &mut QualityReport) {
    let penalty = quality_report
        .issues
        .iter()
        .map(|issue| match issue.severity.as_str() {
            "blocking" => 12,
            "major" => 6,
            "minor" => 2,
            _ => 1,
        })
        .sum::<u32>();
    quality_report.passed = !quality_report
        .issues
        .iter()
        .any(|issue| issue.severity == "blocking" || issue.severity == "major");
    quality_report.score = 100_u32.saturating_sub(penalty);
}

fn figure_edge_style_uses_dash(style: EdgeStyle) -> bool {
    matches!(style, EdgeStyle::Dash | EdgeStyle::LongDash)
}

fn draw_style_uses_dash(style: &str) -> bool {
    let style = style.to_ascii_lowercase();
    style.contains("dash") || style.contains("supervision")
}

fn figure_edge_style_name(style: EdgeStyle) -> &'static str {
    match style {
        EdgeStyle::Solid => "solid",
        EdgeStyle::Dash => "dash",
        EdgeStyle::LongDash => "long_dash",
    }
}

fn build_issue_binding(
    round_index: u32,
    review: &Review,
    quality_report: &QualityReport,
) -> IssueBinding {
    let mut entries = Vec::new();
    for issue in &quality_report.issues {
        entries.push(IssueBindingEntry {
            issue_id: issue.issue_id.clone(),
            issue_type: issue.issue_type.clone(),
            severity: issue.severity.clone(),
            source: "quality_report".to_string(),
            target_ids: issue.target_ids.clone(),
            evidence: issue.evidence.clone(),
            suggested_action: issue.suggested_action.clone(),
        });
    }
    for (index, issue) in review.localized_issues.iter().enumerate() {
        entries.push(IssueBindingEntry {
            issue_id: format!("review_localized_{:03}", index + 1),
            issue_type: "localized_review".to_string(),
            severity: issue_severity_name(issue.severity).to_string(),
            source: "vision_review".to_string(),
            target_ids: vec![issue.target_id.clone()],
            evidence: issue.evidence.clone(),
            suggested_action: issue.suggested_direction.clone(),
        });
    }
    for (index, issue) in review.blocking_issues.iter().enumerate() {
        entries.push(IssueBindingEntry {
            issue_id: format!("review_blocking_{:03}", index + 1),
            issue_type: "blocking_review".to_string(),
            severity: "blocking".to_string(),
            source: "vision_review".to_string(),
            target_ids: target_ids_from_issue_text(issue),
            evidence: issue.clone(),
            suggested_action:
                "Bind this blocking issue to concrete DrawPlan ids before changing code."
                    .to_string(),
        });
    }
    IssueBinding {
        version: "0.1".to_string(),
        round_index,
        entries,
    }
}

fn build_issue_history(
    round_index: u32,
    previous_round_dir: Option<&Path>,
    binding: &IssueBinding,
) -> Result<IssueHistory> {
    let previous = previous_round_dir
        .map(|dir| dir.join("issue_history.json"))
        .filter(|path| path.exists())
        .map(|path| read_json::<IssueHistory>(&path))
        .transpose()?;
    let previous_by_key = previous
        .map(|history| {
            history
                .issues
                .into_iter()
                .map(|issue| (issue.issue_key.clone(), issue))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let issues = binding
        .entries
        .iter()
        .map(|entry| {
            let key = issue_history_key(entry);
            let previous = previous_by_key.get(&key);
            TrackedIssue {
                issue_key: key,
                issue_type: entry.issue_type.clone(),
                target_ids: normalized_target_ids(&entry.target_ids),
                first_seen_round: previous
                    .map(|issue| issue.first_seen_round)
                    .unwrap_or(round_index),
                last_seen_round: round_index,
                occurrences: previous
                    .map(|issue| issue.occurrences.saturating_add(1))
                    .unwrap_or(1),
                latest_evidence: entry.evidence.clone(),
                latest_suggested_action: entry.suggested_action.clone(),
            }
        })
        .collect();
    Ok(IssueHistory {
        version: "0.1".to_string(),
        round_index,
        issues,
    })
}

fn issue_severity_name(severity: crate::schema::IssueSeverity) -> &'static str {
    match severity {
        crate::schema::IssueSeverity::Blocking => "blocking",
        crate::schema::IssueSeverity::Major => "major",
        crate::schema::IssueSeverity::Minor => "minor",
    }
}

fn issue_history_key(entry: &IssueBindingEntry) -> String {
    let target_ids = normalized_target_ids(&entry.target_ids).join("+");
    let evidence = normalize_issue_text(&entry.evidence);
    format!("{}|{}|{}", entry.issue_type, target_ids, evidence)
}

fn normalized_target_ids(target_ids: &[String]) -> Vec<String> {
    let mut ids = target_ids
        .iter()
        .filter(|id| !id.trim().is_empty())
        .cloned()
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
}

fn normalize_issue_text(text: &str) -> String {
    text.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .take(16)
        .collect::<Vec<_>>()
        .join("_")
}

fn target_ids_from_issue_text(text: &str) -> Vec<String> {
    let mut ids = text
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'))
        .filter(|token| token.contains('_') || token.starts_with('e'))
        .map(str::to_string)
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    if ids.is_empty() {
        ids.push("global_layout".to_string());
    }
    ids
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

fn select_revision_improvement_plan(
    revision_round_dir: &Path,
    latest_attempt_round_dir: Option<&Path>,
) -> Result<String> {
    if let Some(latest_attempt_round_dir) = latest_attempt_round_dir {
        let latest_plan_path = latest_attempt_round_dir.join("improvement_plan.json");
        if latest_plan_path.exists() {
            return read_text_or_empty(&latest_plan_path);
        }
    }
    read_text_or_empty(&revision_round_dir.join("improvement_plan.json"))
}

fn create_round_workspace(
    round_dir: &Path,
    round_index: u32,
    method_text: &str,
    reference_selection: &ReferenceSelection,
    previous_round_dir: Option<&Path>,
    latest_attempt_round_dir: Option<&Path>,
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
        workspace_file(
            "readable/reference_selection.json",
            "Selected read-only visual reference grammar and anti-patterns for this run.",
            WorkspaceFileFormat::Json,
            128_000,
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
                "readable/previous_quality_report.json",
                "Previous round structured render quality report with target object ids.",
                WorkspaceFileFormat::Json,
                512_000,
            ),
            workspace_file(
                "readable/previous_regression_report.json",
                "Previous round quality regression report and next-round regression budget.",
                WorkspaceFileFormat::Json,
                256_000,
            ),
            workspace_file(
                "readable/previous_issue_binding.json",
                "Previous round issue-to-object binding used for incremental repair.",
                WorkspaceFileFormat::Json,
                512_000,
            ),
            workspace_file(
                "readable/previous_issue_history.json",
                "Active repeated issue history from previous rounds.",
                WorkspaceFileFormat::Json,
                512_000,
            ),
            workspace_file(
                "readable/previous_repair_report.json",
                "Previous round material DrawPlan changes and repair notes.",
                WorkspaceFileFormat::Json,
                256_000,
            ),
            workspace_file(
                "readable/previous_code/figure.ts",
                "Previous round generated renderer entrypoint.",
                WorkspaceFileFormat::Typescript,
                1_000_000,
            ),
        ]);
        if previous_round_dir
            .map(|dir| dir.join("improvement_plan.json").exists())
            .unwrap_or(false)
        {
            readable.push(workspace_file(
                "readable/previous_improvement_plan.json",
                "Previous round concrete improvement plan.",
                WorkspaceFileFormat::Json,
                256_000,
            ));
        }
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
        if previous_round_dir
            .map(|dir| dir.join("figure.model_error.log").exists())
            .unwrap_or(false)
        {
            readable.push(workspace_file(
                "readable/previous_renderer_error.txt",
                "Previous round model-generated renderer error; use it to avoid repeating invalid TypeScript/runtime API calls.",
                WorkspaceFileFormat::Text,
                256_000,
            ));
        }
    }
    if latest_attempt_round_dir.is_some() {
        readable.extend([
            workspace_file(
                "readable/latest_attempt_draw_plan.json",
                "Most recent attempted DrawPlan, included when revision source rolls back to an earlier best round.",
                WorkspaceFileFormat::Json,
                512_000,
            ),
            workspace_file(
                "readable/latest_attempt_review.json",
                "Most recent attempted review, used to avoid repeating a reverted failed strategy.",
                WorkspaceFileFormat::Json,
                512_000,
            ),
            workspace_file(
                "readable/latest_attempt_quality_report.json",
                "Most recent attempted structured render quality report.",
                WorkspaceFileFormat::Json,
                512_000,
            ),
            workspace_file(
                "readable/latest_attempt_regression_report.json",
                "Most recent attempted regression report against its revision source.",
                WorkspaceFileFormat::Json,
                256_000,
            ),
            workspace_file(
                "readable/latest_attempt_issue_binding.json",
                "Most recent attempted issue-to-object binding.",
                WorkspaceFileFormat::Json,
                512_000,
            ),
            workspace_file(
                "readable/latest_attempt_code/figure.ts",
                "Most recent attempted renderer entrypoint, used only as failure evidence when distinct from the revision source.",
                WorkspaceFileFormat::Typescript,
                1_000_000,
            ),
        ]);
        if latest_attempt_round_dir
            .map(|dir| dir.join("improvement_plan.json").exists())
            .unwrap_or(false)
        {
            readable.push(workspace_file(
                "readable/latest_attempt_improvement_plan.json",
                "Most recent attempted improvement plan, used to avoid repeating a reverted failed strategy.",
                WorkspaceFileFormat::Json,
                256_000,
            ));
        }
        if latest_attempt_round_dir
            .map(|dir| dir.join("figure.model_error.log").exists())
            .unwrap_or(false)
        {
            readable.push(workspace_file(
                "readable/latest_attempt_renderer_error.txt",
                "Most recent attempted model-generated renderer error; use it as negative evidence when revising code.",
                WorkspaceFileFormat::Text,
                256_000,
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
    workspace.write_readable(
        "readable/reference_selection.json",
        &serde_json::to_vec_pretty(reference_selection)?,
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
        write_previous_readable_if_exists(
            &workspace,
            previous_round_dir,
            "quality_report.json",
            "readable/previous_quality_report.json",
        )?;
        write_previous_readable_if_exists(
            &workspace,
            previous_round_dir,
            "regression_report.json",
            "readable/previous_regression_report.json",
        )?;
        write_previous_readable_if_exists(
            &workspace,
            previous_round_dir,
            "issue_binding.json",
            "readable/previous_issue_binding.json",
        )?;
        write_previous_readable_if_exists(
            &workspace,
            previous_round_dir,
            "issue_history.json",
            "readable/previous_issue_history.json",
        )?;
        write_previous_readable_if_exists(
            &workspace,
            previous_round_dir,
            "repair_report.json",
            "readable/previous_repair_report.json",
        )?;
        if previous_round_dir.join("improvement_plan.json").exists() {
            write_previous_readable(
                &workspace,
                previous_round_dir,
                "improvement_plan.json",
                "readable/previous_improvement_plan.json",
            )?;
        }
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
        write_previous_readable_if_exists(
            &workspace,
            previous_round_dir,
            "figure.model_error.log",
            "readable/previous_renderer_error.txt",
        )?;
    }
    if let Some(latest_attempt_round_dir) = latest_attempt_round_dir {
        write_previous_readable_if_exists(
            &workspace,
            latest_attempt_round_dir,
            "draw_plan.json",
            "readable/latest_attempt_draw_plan.json",
        )?;
        write_previous_readable_if_exists(
            &workspace,
            latest_attempt_round_dir,
            "review.json",
            "readable/latest_attempt_review.json",
        )?;
        write_previous_readable_if_exists(
            &workspace,
            latest_attempt_round_dir,
            "quality_report.json",
            "readable/latest_attempt_quality_report.json",
        )?;
        write_previous_readable_if_exists(
            &workspace,
            latest_attempt_round_dir,
            "regression_report.json",
            "readable/latest_attempt_regression_report.json",
        )?;
        write_previous_readable_if_exists(
            &workspace,
            latest_attempt_round_dir,
            "issue_binding.json",
            "readable/latest_attempt_issue_binding.json",
        )?;
        write_previous_readable_if_exists(
            &workspace,
            latest_attempt_round_dir,
            "improvement_plan.json",
            "readable/latest_attempt_improvement_plan.json",
        )?;
        write_previous_readable_if_exists(
            &workspace,
            latest_attempt_round_dir,
            "figure.ts",
            "readable/latest_attempt_code/figure.ts",
        )?;
        write_previous_readable_if_exists(
            &workspace,
            latest_attempt_round_dir,
            "figure.model_error.log",
            "readable/latest_attempt_renderer_error.txt",
        )?;
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

fn write_previous_readable_if_exists(
    workspace: &AgentWorkspace,
    previous_round_dir: &Path,
    source_name: &str,
    workspace_path: &str,
) -> Result<()> {
    if previous_round_dir.join(source_name).exists() {
        write_previous_readable(workspace, previous_round_dir, source_name, workspace_path)?;
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revision_improvement_plan_prefers_latest_attempt_plan_when_revising_from_best_source() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("round_001");
        let latest = temp.path().join("round_002");
        fs::create_dir_all(&source).expect("source dir");
        fs::create_dir_all(&latest).expect("latest dir");
        fs::write(
            source.join("improvement_plan.json"),
            r#"{"summary":"stale source plan"}"#,
        )
        .expect("source plan");
        fs::write(
            latest.join("improvement_plan.json"),
            r#"{"summary":"latest failed attempt postmortem"}"#,
        )
        .expect("latest plan");

        let selected = select_revision_improvement_plan(&source, Some(&latest))
            .expect("revision plan should be selected");

        assert!(selected.contains("latest failed attempt postmortem"));
        assert!(!selected.contains("stale source plan"));
    }

    #[test]
    fn quality_report_injects_draw_plan_edge_style_mismatch() {
        let mut figure_plan = FigurePlan::mock_from_method(
            "teacher student distillation",
            StyleName::WpsClean,
            CanvasAspect::PaperWide,
            85,
        );
        figure_plan.edges[0].id = "teacher_to_alignment".to_string();
        figure_plan.edges[0].style = EdgeStyle::Solid;

        let draw_plan = DrawPlan {
            version: "0.2".to_string(),
            canvas: figure_plan.canvas.clone(),
            style_tokens: BTreeMap::new(),
            objects: vec![DrawObject::Connector {
                id: "teacher_to_alignment".to_string(),
                points: vec![[0.1, 0.2], [0.4, 0.2]],
                from: Some("teacher".to_string()),
                to: Some("student".to_string()),
                style: "dashed_supervision".to_string(),
                label: None,
                z: 1,
            }],
        };
        let mut quality_report = QualityReport {
            version: "0.1".to_string(),
            passed: true,
            score: 100,
            issues: vec![],
        };

        inject_draw_plan_semantic_quality_issues(&mut quality_report, &figure_plan, &draw_plan);

        let issue = quality_report
            .issues
            .iter()
            .find(|issue| issue.issue_type == "edge_style_mismatch")
            .expect("draw connector style should match the FigurePlan edge style");
        assert_eq!(issue.severity, "major");
        assert_eq!(issue.target_ids, vec!["teacher_to_alignment".to_string()]);
        assert!(!quality_report.passed);
        assert_eq!(quality_report.score, 94);
    }
}
