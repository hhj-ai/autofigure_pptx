use std::collections::BTreeMap;
use std::time::Duration;

use methodfig::schema::{
    AssetSpec, CanvasAspect, Component, ComponentRole, Edge, EdgeImportance, EdgeSemantic,
    EdgeStyle, FigurePlan, ImageProviderKind, LayoutRegion, StyleName, Template, VisualWeight,
};
use methodfig::style::style_by_name;
use methodfig::tools::canonicalize::canonicalize_plan_for_render;
use methodfig::tools::pptx_codegen::generate_typescript;
use methodfig::tools::render::{default_renderer_root, run_node_renderer};
use methodfig::tools::review::render_quality_issues;

#[test]
fn canonicalizes_teacher_student_plan_to_stable_regions_without_image_assets() {
    let mut plan = drifting_teacher_student_plan();

    canonicalize_plan_for_render(&mut plan, ImageProviderKind::None);

    assert_region(&plan, "ts_input", [0.06, 0.44, 0.17, 0.60]);
    assert_region(&plan, "ts_teacher", [0.24, 0.10, 0.42, 0.28]);
    assert_region(&plan, "ts_student", [0.24, 0.48, 0.42, 0.68]);
    assert_region(&plan, "ts_residual", [0.43, 0.24, 0.60, 0.44]);
    assert_region(&plan, "ts_task_loss", [0.44, 0.58, 0.60, 0.74]);
    assert_region(&plan, "ts_output", [0.76, 0.58, 0.94, 0.76]);
    assert_region(&plan, "ts_inference_student", [0.62, 0.28, 0.82, 0.48]);
    assert_component_region(&plan, "teacher_big", "ts_teacher");
    assert_component_region(&plan, "student_model", "ts_student");
    assert_component_region(&plan, "latent_residual", "ts_residual");
    assert_component_region(&plan, "task_loss", "ts_task_loss");
    assert_component_region(&plan, "final_output", "ts_output");
    assert_component_region(&plan, "c_inference_student", "ts_inference_student");
    assert_component_absent(&plan, "inference_student");
    assert_component_absent(&plan, "title_component");
    assert_component_absent(&plan, "student_inference_branch");
    assert_component_absent(&plan, "train_label_component");
    assert_component_absent(&plan, "legend_dashed");
    assert_component_absent(&plan, "legend_solid");
    assert_component_absent(&plan, "ann_task_loss");
    assert!(plan.annotations.is_empty());

    assert!(plan.assets.is_empty());
    assert!(plan
        .components
        .iter()
        .all(|component| component.allowed_asset_id.is_none()));
    assert!(plan.edges.iter().any(|edge| edge.from == "latent_residual"
        && edge.to == "task_loss"
        && edge.style == EdgeStyle::Dash));
    assert!(plan.edges.iter().any(|edge| edge.from == "teacher_big"
        && edge.to == "student_model"
        && edge.semantic == EdgeSemantic::Supervision
        && edge.style == EdgeStyle::Dash));
    assert!(plan.edges.iter().any(|edge| edge.from == "student_model"
        && edge.to == "latent_residual"
        && edge.semantic == EdgeSemantic::DataFlow
        && edge.style == EdgeStyle::Solid
        && edge.label.is_empty()));
    assert!(plan
        .edges
        .iter()
        .all(|edge| !(edge.from == "latent_residual" && edge.to == "student_model")));
    assert!(plan
        .edges
        .iter()
        .any(|edge| edge.from == "c_inference_student" && edge.to == "final_output"));
    assert!(plan
        .edges
        .iter()
        .all(|edge| edge.from != "title_component" && edge.to != "title_component"));
    assert!(plan.edges.iter().all(
        |edge| edge.from != "student_inference_branch" && edge.to != "student_inference_branch"
    ));
    assert!(plan
        .edges
        .iter()
        .all(|edge| !(edge.from == "task_loss" && edge.to == "student_model")));
}

#[test]
fn canonicalized_teacher_student_keeps_residual_and_final_output_distinct() {
    let mut plan = FigurePlan::mock_from_method(
        "A teacher model supervises a compact student with latent residuals and task loss.",
        StyleName::WpsClean,
        CanvasAspect::PaperWide,
        85,
    );
    plan.layout.template = Template::TeacherStudent;
    plan.components = vec![
        Component::new(
            "student_input",
            "Task Input",
            ComponentRole::Input,
            VisualWeight::Normal,
            "main_lane",
        ),
        Component::new(
            "teacher_module",
            "Teacher LM",
            ComponentRole::Module,
            VisualWeight::Normal,
            "main_lane",
        ),
        Component::new(
            "teacher_output",
            "Latent Representation",
            ComponentRole::Output,
            VisualWeight::Normal,
            "main_lane",
        ),
        Component::new(
            "student_module",
            "Student Model",
            ComponentRole::Main,
            VisualWeight::Strong,
            "main_lane",
        ),
        Component::new(
            "student_output",
            "Final Answer",
            ComponentRole::Output,
            VisualWeight::Normal,
            "main_lane",
        ),
        Component::new(
            "task_loss",
            "Task Loss",
            ComponentRole::Loss,
            VisualWeight::Normal,
            "main_lane",
        ),
    ];

    canonicalize_plan_for_render(&mut plan, ImageProviderKind::None);

    assert_component_region(&plan, "teacher_output", "ts_residual");
    assert_component_region(&plan, "student_output", "ts_output");
    assert_component_region(&plan, "c_inference_student", "ts_inference_student");
    assert!(plan
        .edges
        .iter()
        .any(|edge| edge.from == "c_inference_student" && edge.to == "student_output"));
    assert!(plan.edges.iter().all(|edge| !(edge.from == "student_module"
        && edge.to == "teacher_output"
        && edge.label == "prediction")));
}

#[test]
fn canonicalized_teacher_student_synthesizes_prediction_when_final_output_is_missing() {
    let mut plan = FigurePlan::mock_from_method(
        "A teacher model supervises a compact student with latent residuals and task loss.",
        StyleName::WpsClean,
        CanvasAspect::PaperWide,
        85,
    );
    plan.layout.template = Template::TeacherStudent;
    plan.components = vec![
        Component::new(
            "student_input",
            "Task Input",
            ComponentRole::Input,
            VisualWeight::Normal,
            "main_lane",
        ),
        Component::new(
            "teacher_module",
            "Teacher LM",
            ComponentRole::Module,
            VisualWeight::Normal,
            "main_lane",
        ),
        Component::new(
            "student_module",
            "Student Model",
            ComponentRole::Main,
            VisualWeight::Strong,
            "main_lane",
        ),
        Component::new(
            "latent_residual",
            "Latent Residual",
            ComponentRole::Loss,
            VisualWeight::Normal,
            "main_lane",
        ),
        Component::new(
            "task_loss",
            "Task Loss",
            ComponentRole::Loss,
            VisualWeight::Normal,
            "main_lane",
        ),
        Component::new(
            "inference_note",
            "Inference: student only",
            ComponentRole::Output,
            VisualWeight::Normal,
            "main_lane",
        ),
    ];

    canonicalize_plan_for_render(&mut plan, ImageProviderKind::None);

    let output = plan
        .components
        .iter()
        .find(|component| component.region == "ts_output")
        .expect("canonicalizer should synthesize a final output component");
    assert_eq!(output.id, "ts_prediction");
    assert_eq!(output.label, "Prediction");
    assert!(plan
        .edges
        .iter()
        .any(|edge| edge.from == "c_inference_student" && edge.to == output.id));
}

#[test]
fn canonicalized_teacher_student_render_passes_local_geometry_gate() {
    let temp = tempfile::tempdir().expect("tempdir");
    let round_dir = temp.path().join("round_000");
    let renderer_root = default_renderer_root().expect("renderer root");
    let mut plan = drifting_teacher_student_plan();
    canonicalize_plan_for_render(&mut plan, ImageProviderKind::None);

    let style = style_by_name(StyleName::WpsClean);
    let code = generate_typescript(&plan, &style, &round_dir, &renderer_root, &BTreeMap::new())
        .expect("deterministic code should generate");
    run_node_renderer(
        &code,
        &round_dir,
        &renderer_root,
        Duration::from_secs(20),
        false,
    )
    .expect("renderer should complete");

    let issues =
        render_quality_issues(&round_dir.join("layout_map.json")).expect("quality gate should run");
    assert!(issues.is_empty(), "{issues:#?}");

    let layout_map: serde_json::Value = serde_json::from_slice(
        &std::fs::read(round_dir.join("layout_map.json")).expect("layout map should exist"),
    )
    .expect("layout map should parse");
    object_bbox(&layout_map, "c_inference_student");
    object_bbox(&layout_map, "e_ts_residual_to_loss");
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
            "inference_student",
            "Student (inference only)",
            ComponentRole::Module,
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
        Component::new(
            "train_label_component",
            "Training",
            ComponentRole::Context,
            VisualWeight::Muted,
            "missing_title_region",
        ),
        Component::new(
            "legend_dashed",
            "Latent Residual Supervision",
            ComponentRole::Context,
            VisualWeight::Muted,
            "legend_region",
        ),
        Component::new(
            "legend_solid",
            "Data Flow",
            ComponentRole::Context,
            VisualWeight::Muted,
            "legend_region",
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
            "e_input_to_student",
            "input_text",
            "student_model",
            "input",
            EdgeSemantic::DataFlow,
            EdgeStyle::Solid,
            EdgeImportance::Main,
        ),
        Edge::new(
            "e_teacher_to_residual",
            "teacher_big",
            "latent_residual",
            "teacher hidden",
            EdgeSemantic::DataFlow,
            EdgeStyle::Solid,
            EdgeImportance::Normal,
        ),
        Edge::new(
            "e_student_to_residual",
            "student_model",
            "latent_residual",
            "student hidden",
            EdgeSemantic::DataFlow,
            EdgeStyle::Solid,
            EdgeImportance::Normal,
        ),
        Edge::new(
            "e_residual_to_student",
            "latent_residual",
            "student_model",
            "supervision",
            EdgeSemantic::Supervision,
            EdgeStyle::Dash,
            EdgeImportance::Main,
        ),
        Edge::new(
            "e_loss_feedback_to_student",
            "task_loss",
            "student_model",
            "feedback",
            EdgeSemantic::Feedback,
            EdgeStyle::Solid,
            EdgeImportance::Normal,
        ),
        Edge::new(
            "e_title_to_output",
            "title_component",
            "final_output",
            "noise",
            EdgeSemantic::DataFlow,
            EdgeStyle::Solid,
            EdgeImportance::Normal,
        ),
        Edge::new(
            "e_inference_branch_to_output",
            "student_inference_branch",
            "final_output",
            "duplicate inference path",
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

fn assert_component_absent(plan: &FigurePlan, id: &str) {
    assert!(
        plan.components.iter().all(|component| component.id != id),
        "component {id} should be pruned"
    );
}

fn object_bbox(layout_map: &serde_json::Value, id: &str) -> [f64; 4] {
    let object = layout_map["objects"]
        .as_array()
        .expect("objects is array")
        .iter()
        .find(|object| object["id"] == id)
        .unwrap_or_else(|| panic!("object {id} exists"));
    let bbox = object["bbox"].as_array().expect("bbox is array");
    [
        bbox[0].as_f64().unwrap(),
        bbox[1].as_f64().unwrap(),
        bbox[2].as_f64().unwrap(),
        bbox[3].as_f64().unwrap(),
    ]
}
