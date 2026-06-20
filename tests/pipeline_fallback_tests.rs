use std::collections::BTreeMap;

use methodfig::pipeline::build_fallback_renderer_code;
use methodfig::schema::{CanvasAspect, FigurePlan, StyleName};
use methodfig::style::style_by_name;
use methodfig::tools::draw_plan::draw_plan_from_figure_plan;
use methodfig::tools::render::default_renderer_root;

#[test]
fn pipeline_fallback_renderer_uses_current_draw_plan_not_legacy_figure_plan() {
    let temp = tempfile::tempdir().expect("tempdir");
    let round_dir = temp.path().join("round_000");
    let renderer_root = default_renderer_root().expect("renderer root");
    let style = style_by_name(StyleName::WpsClean);
    let plan = FigurePlan::mock_from_method(
        "Teacher guides a student with latent residuals.",
        StyleName::WpsClean,
        CanvasAspect::PaperWide,
        85,
    );
    let draw_plan = draw_plan_from_figure_plan(&plan, &style);

    let fallback_code = build_fallback_renderer_code(
        &plan,
        &draw_plan,
        &style,
        &round_dir,
        &renderer_root,
        &BTreeMap::new(),
    )
    .expect("fallback code should build");

    assert!(fallback_code.contains("createDrawPlanRuntime"));
    assert!(!fallback_code.contains("createFigureRuntime"));
}
