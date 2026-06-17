use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use crate::agent::{
    apply_patch_plan_to_figure, create_initial_plan, create_patch_plan, create_typescript_code,
    review_rendered_figure, validate_required_model_config,
};
use crate::config::AppConfig;
use crate::schema::{
    CanvasAspect, ImageProviderKind, PatchPlan, PatchStopReason, Review, StyleName,
};
use crate::style::style_by_name;
use crate::tools::asset_gen::materialize_assets;
use crate::tools::canonicalize::canonicalize_plan_for_render;
use crate::tools::cost::{
    CostTracker, EST_CODER_USD, EST_IMAGE_ASSET_USD, EST_REASONER_INITIAL_USD,
    EST_REASONER_PATCH_USD, EST_VISION_USD,
};
use crate::tools::export::export_round;
use crate::tools::pptx_codegen::generate_typescript;
use crate::tools::render::{default_renderer_root, run_node_renderer_with_fallback};
use crate::tools::review::{apply_plan_geometry_gate, apply_render_quality_gate};
use crate::tools::validate::{normalize_plan_for_render, validate_plan_for_render};

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

pub fn run_pipeline(options: RunOptions) -> Result<PipelineResult> {
    if options.max_iterations == 0 {
        return Err(anyhow!("max_iterations must be greater than 0"));
    }
    let start = Instant::now();
    fs::create_dir_all(&options.out_dir)?;
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

    let config = AppConfig::from_env()?;
    validate_required_model_config(&config, options.mock_models)?;
    let mut cost = CostTracker::new(options.max_cost_usd)?;
    if !options.mock_models {
        cost.reserve("reasoner initial plan", EST_REASONER_INITIAL_USD)?;
    }
    let style = style_by_name(options.style);
    let renderer_root = default_renderer_root()?;
    let mut best_round: Option<(u32, Review)> = None;
    let mut cap_reason: Option<String> = None;
    let mut plan = create_initial_plan(
        &method_text,
        options.style,
        options.aspect,
        options.target_width_mm,
        &config,
        options.mock_models,
    )?;
    let mut pending_patch: Option<PatchPlan> = None;

    for round_index in 0..options.max_iterations {
        if start.elapsed() > Duration::from_secs(u64::from(options.max_minutes) * 60) {
            append_log(&options.out_dir, "time cap reached")?;
            break;
        }

        if let Some(patch) = pending_patch.take() {
            apply_patch_plan_to_figure(&mut plan, &patch);
        }

        let round_dir = options.out_dir.join(format!("round_{round_index:03}"));
        fs::create_dir_all(round_dir.join("assets"))?;
        canonicalize_plan_for_render(&mut plan, options.image_provider);
        normalize_plan_for_render(&mut plan);
        validate_plan_for_render(&plan, &style)?;
        write_json(&round_dir.join("figure_plan.json"), &plan)?;

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
        if !options.mock_models {
            if let Err(error) = cost.reserve("coder TypeScript generation", EST_CODER_USD) {
                let reason = error.to_string();
                append_log(&options.out_dir, &reason)?;
                cap_reason = Some(reason);
                break;
            }
        }
        let code = create_typescript_code(
            &plan,
            &style,
            &round_dir,
            &renderer_root,
            &asset_paths,
            &config,
            options.mock_models,
        )?;
        let fallback_code =
            generate_typescript(&plan, &style, &round_dir, &renderer_root, &asset_paths)?;
        run_node_renderer_with_fallback(
            &code,
            &fallback_code,
            &round_dir,
            &renderer_root,
            options.renderer_timeout,
            options.mock_models,
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
        write_json(&round_dir.join("review.json"), &review)?;
        best_round = Some((round_index, review.clone()));

        if review.passed {
            let patch = PatchPlan {
                operations: vec![],
                stop_reason: PatchStopReason::Accepted,
            };
            write_json(&round_dir.join("patch_plan.json"), &patch)?;
            finalize_from_round(&options.out_dir, &round_dir, &review, true, "accepted")?;
            append_log(&options.out_dir, "run accepted")?;
            return Ok(PipelineResult {
                accepted: true,
                rounds: round_index + 1,
                run_dir: options.out_dir.clone(),
                final_dir: options.out_dir.join("final"),
                reason: "accepted".to_string(),
            });
        }

        if !options.mock_models {
            if let Err(error) = cost.reserve("reasoner patch plan", EST_REASONER_PATCH_USD) {
                let reason = error.to_string();
                append_log(&options.out_dir, &reason)?;
                cap_reason = Some(reason);
                break;
            }
        }
        let patch = create_patch_plan(&plan, &review, &config, options.mock_models)?;
        write_json(&round_dir.join("patch_plan.json"), &patch)?;
        pending_patch = Some(patch);
    }

    let (round_index, review) = best_round.ok_or_else(|| anyhow!("no round was produced"))?;
    let round_dir = options.out_dir.join(format!("round_{round_index:03}"));
    let reason = cap_reason.unwrap_or_else(|| "cap reached before acceptance".to_string());
    finalize_from_round(&options.out_dir, &round_dir, &review, false, &reason)?;
    Ok(PipelineResult {
        accepted: false,
        rounds: round_index + 1,
        run_dir: options.out_dir.clone(),
        final_dir: options.out_dir.join("final"),
        reason,
    })
}

pub fn resume_pipeline(run_dir: PathBuf) -> Result<PipelineResult> {
    let final_status = run_dir.join("final/status.json");
    if final_status.exists() {
        let status: FinalStatus = read_json(&final_status)?;
        return Ok(PipelineResult {
            accepted: status.accepted,
            rounds: count_rounds(&run_dir)?,
            run_dir: run_dir.clone(),
            final_dir: run_dir.join("final"),
            reason: status.reason,
        });
    }

    let config: ConfigSnapshot = read_json(&run_dir.join("config_snapshot.json"))?;
    let input = run_dir.join("input.md");
    run_pipeline(RunOptions {
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
    })
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
        "figure_plan.json",
        "review.json",
        "layout_map.json",
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
    let mut count = 0;
    for entry in fs::read_dir(run_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() && entry.file_name().to_string_lossy().starts_with("round_")
        {
            count += 1;
        }
    }
    Ok(count)
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
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
