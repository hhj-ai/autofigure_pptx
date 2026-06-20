use std::fs;
use std::time::Duration;

use methodfig::pipeline::{run_pipeline, RunOptions};
use methodfig::schema::{CanvasAspect, ImageProviderKind, StyleName};

#[test]
fn mock_pipeline_writes_agentic_workspace_and_draw_plan_artifacts() {
    let temp = tempfile::tempdir().expect("tempdir");
    let method_path = temp.path().join("method.md");
    let out_dir = temp.path().join("run");
    fs::write(
        &method_path,
        "# Method\n\nA teacher model supervises a compact student with latent residuals.",
    )
    .expect("write method");

    let result = run_pipeline(RunOptions {
        method_path,
        out_dir: out_dir.clone(),
        style: StyleName::WpsClean,
        aspect: CanvasAspect::PaperWide,
        target_width_mm: 85,
        max_iterations: 2,
        max_cost_usd: 3.0,
        max_minutes: 20,
        image_provider: ImageProviderKind::None,
        mock_models: true,
        keep_intermediate: true,
        renderer_timeout: Duration::from_secs(20),
    })
    .expect("mock pipeline should complete");

    assert!(result.accepted);
    let round_dir = out_dir.join("round_000");
    assert!(round_dir.join("workspace/manifest.json").exists());
    assert!(round_dir
        .join("workspace/writable/design_brief.md")
        .exists());
    assert!(round_dir.join("workspace/writable/draw_plan.json").exists());
    assert!(round_dir.join("workspace/writable/code/figure.ts").exists());
    assert!(round_dir
        .join("workspace/readable/method_templates.json")
        .exists());
    assert!(round_dir.join("draw_plan.json").exists());
    assert!(round_dir.join("figure.ts").exists());
    assert!(round_dir.join("validation_report.json").exists());
    assert!(round_dir.join("renderer_status.json").exists());
    assert!(out_dir.join("final/draw_plan.json").exists());
    assert!(out_dir.join("final/figure.ts").exists());
    assert!(out_dir.join("final/renderer_status.json").exists());

    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(round_dir.join("workspace/manifest.json")).unwrap())
            .unwrap();
    assert!(manifest["writable"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["path"] == "writable/draw_plan.json"));
    assert!(manifest["writable"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["path"] == "writable/code/figure.ts"));
    assert!(manifest["readable"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["path"] == "readable/method_templates.json"));
    let method_templates =
        fs::read_to_string(round_dir.join("workspace/readable/method_templates.json")).unwrap();
    assert!(method_templates.contains("simclr_contrastive_y_branch"));
    assert!(method_templates.contains("derived_from_pdf_vector_page"));

    let renderer_status: serde_json::Value =
        serde_json::from_slice(&fs::read(round_dir.join("renderer_status.json")).unwrap()).unwrap();
    assert_eq!(renderer_status["source"], "mock_generated_code");
    assert_eq!(renderer_status["used_fallback"], false);
}

#[test]
fn mock_pipeline_revises_generated_code_from_previous_round_context() {
    let temp = tempfile::tempdir().expect("tempdir");
    let method_path = temp.path().join("method.md");
    let out_dir = temp.path().join("run");
    fs::write(
        &method_path,
        "# Method\n\nA teacher model supervises a compact student with latent residuals.",
    )
    .expect("write method");

    let result = run_pipeline(RunOptions {
        method_path,
        out_dir: out_dir.clone(),
        style: StyleName::WpsClean,
        aspect: CanvasAspect::PaperWide,
        target_width_mm: 85,
        max_iterations: 2,
        max_cost_usd: 3.0,
        max_minutes: 20,
        image_provider: ImageProviderKind::None,
        mock_models: true,
        keep_intermediate: true,
        renderer_timeout: Duration::from_secs(20),
    })
    .expect("mock pipeline should complete");

    assert!(result.accepted);
    let round_000_code = fs::read_to_string(out_dir.join("round_000/figure.ts")).unwrap();
    let round_001_code = fs::read_to_string(out_dir.join("round_001/figure.ts")).unwrap();
    let round_000_draw_plan = fs::read_to_string(out_dir.join("round_000/draw_plan.json")).unwrap();
    let round_001_draw_plan = fs::read_to_string(out_dir.join("round_001/draw_plan.json")).unwrap();
    assert_ne!(
        round_000_code, round_001_code,
        "second round should revise generated code instead of replaying the same output"
    );
    assert_ne!(
        round_000_draw_plan, round_001_draw_plan,
        "second round should revise DrawPlan geometry from previous review, not just wrap the same plan in new code"
    );
    assert!(out_dir
        .join("round_001/workspace/readable/previous_code/figure.ts")
        .exists());
    assert!(out_dir
        .join("round_001/workspace/writable/code/figure.ts")
        .exists());
    assert!(!out_dir.join("round_000/patch_plan.json").exists());
    assert!(!out_dir.join("round_001/patch_plan.json").exists());
}
