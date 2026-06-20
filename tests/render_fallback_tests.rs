use std::collections::BTreeMap;
use std::time::Duration;

use methodfig::schema::{CanvasAspect, FigurePlan, StyleName};
use methodfig::style::style_by_name;
use methodfig::tools::draw_plan::{draw_plan_from_figure_plan, generate_draw_plan_typescript};
use methodfig::tools::pptx_codegen::generate_typescript;
use methodfig::tools::render::{
    default_renderer_root, run_node_renderer, run_node_renderer_with_fallback,
};

#[test]
fn renderer_retries_deterministic_fallback_when_model_code_does_not_compile() {
    let temp = tempfile::tempdir().expect("tempdir");
    let round_dir = temp.path().join("round_000");
    let renderer_root = default_renderer_root().expect("renderer root");
    let plan = FigurePlan::mock_from_method(
        "Teacher guides student with latent residuals.",
        StyleName::WpsClean,
        CanvasAspect::PaperWide,
        85,
    );
    let style = style_by_name(StyleName::WpsClean);
    let fallback = generate_typescript(&plan, &style, &round_dir, &renderer_root, &BTreeMap::new())
        .expect("fallback code should generate");

    run_node_renderer_with_fallback(
        r#"const broken = "unterminated"#,
        &fallback,
        &round_dir,
        &renderer_root,
        Duration::from_secs(20),
        false,
    )
    .expect("fallback renderer should recover from invalid model code");

    assert!(round_dir.join("figure.pptx").exists());
    assert!(round_dir.join("layout_map.json").exists());
    assert!(round_dir.join("figure.ts.log").exists());
}

#[test]
fn renderer_forces_current_round_out_dir_over_model_payload() {
    let temp = tempfile::tempdir().expect("tempdir");
    let round_dir = temp.path().join("round_000");
    let stale_payload_dir = temp.path().join("round_999");
    let renderer_root = default_renderer_root().expect("renderer root");
    let plan = FigurePlan::mock_from_method(
        "Teacher guides student with latent residuals.",
        StyleName::WpsClean,
        CanvasAspect::PaperWide,
        85,
    );
    let style = style_by_name(StyleName::WpsClean);
    let draw_plan = draw_plan_from_figure_plan(&plan, &style);
    let model_code = generate_draw_plan_typescript(
        &draw_plan,
        &style,
        &stale_payload_dir,
        &renderer_root,
        &BTreeMap::new(),
    )
    .expect("model-like code should generate");

    run_node_renderer(
        &model_code,
        &round_dir,
        &renderer_root,
        Duration::from_secs(20),
        false,
    )
    .expect("trusted runtime should ignore stale model payload output directory");

    assert!(round_dir.join("figure.pptx").exists());
    assert!(round_dir.join("layout_map.json").exists());
    assert!(
        !stale_payload_dir.join("figure.pptx").exists(),
        "model code must not be able to redirect renderer output to another round"
    );
}
