use std::fs;
use std::time::Duration;

use methodfig::pipeline::{resume_pipeline, run_pipeline, RunOptions};
use methodfig::schema::{CanvasAspect, ImageProviderKind, StyleName};

#[test]
fn mock_pipeline_fails_once_patches_then_writes_final_artifacts() {
    let temp = tempfile::tempdir().expect("tempdir");
    let method_path = temp.path().join("method.md");
    let out_dir = temp.path().join("run");
    fs::write(
        &method_path,
        "# Method\n\nA teacher model supervises a compact student model with latent residuals.",
    )
    .expect("write method");

    let result = run_pipeline(RunOptions {
        method_path,
        out_dir: out_dir.clone(),
        style: StyleName::WpsClean,
        aspect: CanvasAspect::PaperWide,
        target_width_mm: 85,
        max_iterations: 3,
        max_cost_usd: 3.0,
        max_minutes: 20,
        image_provider: ImageProviderKind::None,
        mock_models: true,
        keep_intermediate: true,
        renderer_timeout: Duration::from_secs(20),
    })
    .expect("mock pipeline should complete");

    assert!(result.accepted);
    assert_eq!(result.rounds, 2);
    assert!(out_dir.join("round_000/review.json").exists());
    assert!(out_dir.join("round_001/patch_plan.json").exists());
    assert!(out_dir.join("final/figure.pptx").exists());
    assert!(out_dir.join("final/figure.pdf").exists());
    assert!(out_dir.join("final/figure.png").exists());
    assert!(out_dir.join("final/figure_plan.json").exists());
    assert!(out_dir.join("final/review.json").exists());
}

#[test]
fn resume_pipeline_uses_existing_run_directory() {
    let temp = tempfile::tempdir().expect("tempdir");
    let method_path = temp.path().join("method.md");
    let out_dir = temp.path().join("run");
    fs::write(
        &method_path,
        "# Method\n\nTwo encoders fuse image and text features before classification.",
    )
    .expect("write method");

    run_pipeline(RunOptions {
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
    .expect("initial run");

    let resumed = resume_pipeline(out_dir.clone()).expect("resume should inspect existing run");
    assert!(resumed.accepted);
    assert_eq!(resumed.rounds, 2);
    assert_eq!(resumed.run_dir, out_dir);
}

#[test]
fn mock_pipeline_writes_rejected_final_when_iteration_cap_reached() {
    let temp = tempfile::tempdir().expect("tempdir");
    let method_path = temp.path().join("method.md");
    let out_dir = temp.path().join("run");
    fs::write(
        &method_path,
        "# Method\n\nA teacher supervises a student, but one iteration is not enough.",
    )
    .expect("write method");

    let result = run_pipeline(RunOptions {
        method_path,
        out_dir: out_dir.clone(),
        style: StyleName::WpsClean,
        aspect: CanvasAspect::PaperWide,
        target_width_mm: 85,
        max_iterations: 1,
        max_cost_usd: 3.0,
        max_minutes: 20,
        image_provider: ImageProviderKind::None,
        mock_models: true,
        keep_intermediate: true,
        renderer_timeout: Duration::from_secs(20),
    })
    .expect("mock pipeline should produce best available final");

    assert!(!result.accepted);
    assert!(out_dir.join("final/figure.pptx").exists());
    let status: serde_json::Value =
        serde_json::from_slice(&fs::read(out_dir.join("final/status.json")).unwrap()).unwrap();
    assert_eq!(status["accepted"], false);
}
