use std::fs;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use methodfig::pipeline::{run_pipeline, RunOptions};
use methodfig::schema::{CanvasAspect, ImageProviderKind, StyleName};

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[test]
fn non_mock_pipeline_requires_configured_reasoner() {
    let _guard = env_lock().lock().expect("env lock");
    for key in [
        "METHODFIG_REASONER_API_KEY",
        "METHODFIG_REASONER_MODEL",
        "METHODFIG_CODER_API_KEY",
        "METHODFIG_CODER_MODEL",
        "METHODFIG_VISION_API_KEY",
        "METHODFIG_VISION_MODEL",
    ] {
        std::env::remove_var(key);
    }

    let temp = tempfile::tempdir().expect("tempdir");
    let method_path = temp.path().join("method.md");
    fs::write(&method_path, "A pipeline method with a central module.").expect("write method");

    let err = run_pipeline(RunOptions {
        method_path,
        out_dir: temp.path().join("run"),
        style: StyleName::WpsClean,
        aspect: CanvasAspect::PaperWide,
        target_width_mm: 85,
        max_iterations: 1,
        max_cost_usd: 3.0,
        max_minutes: 20,
        image_provider: ImageProviderKind::None,
        mock_models: false,
        keep_intermediate: true,
        renderer_timeout: Duration::from_secs(20),
    })
    .expect_err("non-mock run should require configured models");

    assert!(err.to_string().contains("reasoner"));
}

#[test]
fn non_mock_pipeline_requires_configured_coder_before_api_calls() {
    let _guard = env_lock().lock().expect("env lock");
    std::env::set_var("METHODFIG_REASONER_API_KEY", "reasoner-key");
    std::env::set_var("METHODFIG_REASONER_MODEL", "reasoner-model");
    std::env::remove_var("METHODFIG_CODER_API_KEY");
    std::env::remove_var("METHODFIG_CODER_MODEL");
    std::env::set_var("METHODFIG_VISION_API_KEY", "vision-key");
    std::env::set_var("METHODFIG_VISION_MODEL", "vision-model");

    let temp = tempfile::tempdir().expect("tempdir");
    let method_path = temp.path().join("method.md");
    fs::write(&method_path, "A pipeline method with a central module.").expect("write method");

    let err = run_pipeline(RunOptions {
        method_path,
        out_dir: temp.path().join("run"),
        style: StyleName::WpsClean,
        aspect: CanvasAspect::PaperWide,
        target_width_mm: 85,
        max_iterations: 1,
        max_cost_usd: 3.0,
        max_minutes: 20,
        image_provider: ImageProviderKind::None,
        mock_models: false,
        keep_intermediate: true,
        renderer_timeout: Duration::from_secs(20),
    })
    .expect_err("non-mock run should require configured coder");

    assert!(err.to_string().contains("coder"));
}

#[test]
fn non_mock_pipeline_enforces_cost_cap_before_external_calls() {
    let _guard = env_lock().lock().expect("env lock");
    std::env::set_var("METHODFIG_REASONER_API_KEY", "reasoner-key");
    std::env::set_var("METHODFIG_REASONER_MODEL", "reasoner-model");
    std::env::set_var("METHODFIG_CODER_API_KEY", "coder-key");
    std::env::set_var("METHODFIG_CODER_MODEL", "coder-model");
    std::env::set_var("METHODFIG_VISION_API_KEY", "vision-key");
    std::env::set_var("METHODFIG_VISION_MODEL", "vision-model");

    let temp = tempfile::tempdir().expect("tempdir");
    let method_path = temp.path().join("method.md");
    fs::write(&method_path, "A pipeline method with a central module.").expect("write method");

    let err = run_pipeline(RunOptions {
        method_path,
        out_dir: temp.path().join("run"),
        style: StyleName::WpsClean,
        aspect: CanvasAspect::PaperWide,
        target_width_mm: 85,
        max_iterations: 1,
        max_cost_usd: 0.01,
        max_minutes: 20,
        image_provider: ImageProviderKind::None,
        mock_models: false,
        keep_intermediate: true,
        renderer_timeout: Duration::from_secs(20),
    })
    .expect_err("non-mock run should stop before exceeding cost cap");

    assert!(err.to_string().contains("cost cap"));
}
