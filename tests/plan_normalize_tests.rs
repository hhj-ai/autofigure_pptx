use methodfig::schema::{CanvasAspect, FigurePlan, StyleName};
use methodfig::tools::validate::normalize_plan_for_render;

#[test]
fn normalizes_grid_region_boxes_and_unbounded_safe_margin_from_model_plan() {
    let mut plan = FigurePlan::mock_from_method(
        "Teacher guides student with latent residuals.",
        StyleName::WpsClean,
        CanvasAspect::PaperWide,
        85,
    );
    plan.canvas.safe_margin = 3.0;
    plan.layout.grid.columns = 5;
    plan.layout.grid.rows = 2;
    plan.layout.regions[0].bbox = [3.0, 0.0, 2.0, 1.0];

    normalize_plan_for_render(&mut plan);

    assert_eq!(plan.canvas.safe_margin, 0.06);
    assert_eq!(plan.layout.regions[0].bbox, [0.6, 0.0, 1.0, 0.5]);
}

#[test]
fn expands_zero_height_regions_at_canvas_edge() {
    let mut plan = FigurePlan::mock_from_method(
        "Teacher guides student with latent residuals.",
        StyleName::WpsClean,
        CanvasAspect::PaperWide,
        85,
    );
    plan.layout.regions[0].bbox = [0.2, 1.0, 0.8, 1.0];

    normalize_plan_for_render(&mut plan);

    assert_eq!(plan.layout.regions[0].bbox, [0.2, 0.94, 0.8, 1.0]);
}

#[test]
fn converts_literal_backslash_n_in_editable_labels_to_real_line_breaks() {
    let mut plan = FigurePlan::mock_from_method(
        "Teacher guides student with latent residuals.",
        StyleName::WpsClean,
        CanvasAspect::PaperWide,
        85,
    );
    plan.components[0].label = "Teacher\\n(Large LM)".to_string();
    plan.edges[0].label = "latent\\nsupervision".to_string();
    plan.annotations.push(methodfig::schema::Annotation {
        id: "ann_phase".to_string(),
        label: "Training\\nphase".to_string(),
        target_id: None,
        bbox: Some([0.1, 0.1, 0.3, 0.2]),
    });

    normalize_plan_for_render(&mut plan);

    assert_eq!(plan.components[0].label, "Teacher\n(Large LM)");
    assert_eq!(plan.edges[0].label, "latent\nsupervision");
    assert_eq!(plan.annotations[0].label, "Training\nphase");
}
