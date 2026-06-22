use methodfig::schema::{
    validate_stable_ids, CanvasAspect, Edge, EdgeImportance, EdgeSemantic, EdgeStyle, FigurePlan,
    LayoutRegion, StyleName,
};
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

#[test]
fn normalizes_duplicate_region_component_ids_and_updates_references() {
    let mut plan = FigurePlan::mock_from_method(
        "Teacher guides student with latent residuals.",
        StyleName::WpsClean,
        CanvasAspect::PaperWide,
        85,
    );
    plan.layout.regions[0].id = "teacher".to_string();
    plan.components[0].id = "teacher".to_string();
    plan.components[0].region = "teacher".to_string();

    normalize_plan_for_render(&mut plan);

    assert_ne!(plan.layout.regions[0].id, "teacher");
    assert_eq!(plan.components[0].region, plan.layout.regions[0].id);
    validate_stable_ids(&plan).expect("normalization should remove duplicate stable ids");
}

#[test]
fn repairs_edges_that_reference_regions_and_drops_unresolvable_endpoints() {
    let mut plan = FigurePlan::mock_from_method(
        "Teacher guides student with latent residuals.",
        StyleName::WpsClean,
        CanvasAspect::PaperWide,
        85,
    );
    let source_id = plan.components[0].id.clone();
    let target_id = plan.components[1].id.clone();
    plan.layout.regions.push(LayoutRegion {
        id: "latent_residual_region".to_string(),
        bbox: [0.45, 0.35, 0.6, 0.55],
    });
    plan.components[1].region = "latent_residual_region".to_string();
    plan.edges = vec![
        Edge::new(
            "e_region_endpoint",
            &source_id,
            "latent_residual_region",
            "residual",
            EdgeSemantic::Supervision,
            EdgeStyle::Dash,
            EdgeImportance::Normal,
        ),
        Edge::new(
            "e_missing_endpoint",
            &source_id,
            "missing_component",
            "bad",
            EdgeSemantic::Supervision,
            EdgeStyle::Dash,
            EdgeImportance::Normal,
        ),
    ];

    normalize_plan_for_render(&mut plan);

    assert_eq!(plan.edges.len(), 1);
    assert_eq!(plan.edges[0].id, "e_region_endpoint");
    assert_eq!(plan.edges[0].to, target_id);
    validate_stable_ids(&plan).expect("normalization should leave only valid edge endpoints");
}
