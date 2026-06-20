use methodfig::schema::{
    AssetSpec, CanvasAspect, Component, ComponentRole, Edge, EdgeImportance, EdgeSemantic,
    EdgeStyle, FigurePlan, ImageProviderKind, LayoutRegion, StyleName, Template, VisualWeight,
};
use methodfig::tools::canonicalize::canonicalize_plan_for_render;

#[test]
fn canonicalize_preserves_model_authored_teacher_student_layout() {
    let mut plan = drifting_teacher_student_plan();
    let original_regions = plan.layout.regions.clone();
    let original_components = plan.components.clone();
    let original_edges = plan.edges.clone();
    let original_annotations = plan.annotations.clone();

    canonicalize_plan_for_render(&mut plan, ImageProviderKind::None);

    assert_eq!(plan.layout.regions, original_regions);
    assert_eq!(plan.components.len(), original_components.len());
    assert!(original_components
        .iter()
        .all(|component| plan.components.iter().any(|kept| kept.id == component.id)));
    assert_eq!(plan.edges, original_edges);
    assert_eq!(plan.annotations, original_annotations);
}

#[test]
fn canonicalize_strips_image_expectations_without_rewriting_design() {
    let mut plan = drifting_teacher_student_plan();

    canonicalize_plan_for_render(&mut plan, ImageProviderKind::None);

    assert!(plan.assets.is_empty());
    assert!(plan
        .components
        .iter()
        .all(|component| component.allowed_asset_id.is_none()));
    assert_component_region(&plan, "student_model", "random_middle");
    assert_component_region(&plan, "latent_residual", "random_middle");
    assert!(plan
        .components
        .iter()
        .any(|component| component.id == "student_inference_branch"));
}

#[test]
fn canonicalize_keeps_asset_expectations_when_image_provider_is_enabled() {
    let mut plan = drifting_teacher_student_plan();

    canonicalize_plan_for_render(&mut plan, ImageProviderKind::OpenRouter);

    assert_eq!(plan.assets.len(), 2);
    assert_eq!(
        plan.components
            .iter()
            .find(|component| component.id == "student_model")
            .and_then(|component| component.allowed_asset_id.as_deref()),
        Some("student_icon")
    );
    assert_region(&plan, "full_canvas", [0.0, 0.0, 1.0, 1.0]);
    assert_region(&plan, "random_middle", [0.3, 0.1, 0.9, 0.9]);
}

fn drifting_teacher_student_plan() -> FigurePlan {
    let mut plan = FigurePlan::mock_from_method(
        "A teacher model supervises a compact student with latent residuals and task loss.",
        StyleName::WpsClean,
        CanvasAspect::PaperWide,
        85,
    );
    plan.layout.template = Template::TeacherStudent;
    plan.layout.regions = vec![
        LayoutRegion {
            id: "full_canvas".to_string(),
            bbox: [0.0, 0.0, 1.0, 1.0],
        },
        LayoutRegion {
            id: "random_middle".to_string(),
            bbox: [0.3, 0.1, 0.9, 0.9],
        },
    ];
    plan.components = vec![
        Component::new(
            "title_component",
            "Teacher-Student Residual Overview",
            ComponentRole::Context,
            VisualWeight::Normal,
            "full_canvas",
        ),
        Component::new(
            "input_text",
            "Task Input x",
            ComponentRole::Input,
            VisualWeight::Normal,
            "full_canvas",
        ),
        Component::new(
            "teacher_big",
            "Teacher Model (large)",
            ComponentRole::Context,
            VisualWeight::Normal,
            "full_canvas",
        ),
        Component::new(
            "student_model",
            "Student Model (compact)",
            ComponentRole::Main,
            VisualWeight::Strong,
            "random_middle",
        )
        .with_asset("student_icon"),
        Component::new(
            "latent_residual",
            "Latent Residual r = h_T - h_S",
            ComponentRole::Loss,
            VisualWeight::Normal,
            "random_middle",
        )
        .with_asset("residual_icon"),
        Component::new(
            "task_loss",
            "Task Loss",
            ComponentRole::Loss,
            VisualWeight::Normal,
            "full_canvas",
        ),
        Component::new(
            "final_output",
            "Predicted Answer",
            ComponentRole::Output,
            VisualWeight::Normal,
            "full_canvas",
        ),
        Component::new(
            "student_inference_branch",
            "Student (inference only)",
            ComponentRole::Main,
            VisualWeight::Normal,
            "full_canvas",
        ),
    ];
    plan.edges = vec![
        Edge::new(
            "e_input_to_teacher",
            "input_text",
            "teacher_big",
            "input",
            EdgeSemantic::DataFlow,
            EdgeStyle::Solid,
            EdgeImportance::Normal,
        ),
        Edge::new(
            "e_student_to_output",
            "student_model",
            "final_output",
            "prediction",
            EdgeSemantic::DataFlow,
            EdgeStyle::Solid,
            EdgeImportance::Main,
        ),
    ];
    plan.assets = vec![
        AssetSpec::generated_icon("student_icon", "student model"),
        AssetSpec::generated_icon("residual_icon", "latent residual"),
    ];
    plan
}

fn assert_region(plan: &FigurePlan, id: &str, expected: [f64; 4]) {
    let region = plan
        .layout
        .regions
        .iter()
        .find(|region| region.id == id)
        .unwrap_or_else(|| panic!("region {id} exists"));
    assert_eq!(region.bbox, expected);
}

fn assert_component_region(plan: &FigurePlan, id: &str, expected_region: &str) {
    let component = plan
        .components
        .iter()
        .find(|component| component.id == id)
        .unwrap_or_else(|| panic!("component {id} exists"));
    assert_eq!(component.region, expected_region);
}
