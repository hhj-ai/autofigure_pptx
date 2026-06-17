use std::collections::BTreeMap;
use std::time::Duration;

use methodfig::schema::{
    CanvasAspect, Component, ComponentRole, Edge, EdgeImportance, EdgeSemantic, EdgeStyle,
    FigurePlan, LayoutRegion, StyleName, VisualWeight,
};
use methodfig::style::style_by_name;
use methodfig::tools::pptx_codegen::generate_typescript;
use methodfig::tools::render::{default_renderer_root, run_node_renderer};

#[test]
fn renderer_places_llm_component_ids_inside_declared_regions() {
    let temp = tempfile::tempdir().expect("tempdir");
    let round_dir = temp.path().join("round_000");
    let renderer_root = default_renderer_root().expect("renderer root");
    let mut plan = FigurePlan::mock_from_method(
        "Teacher guides a student with latent residuals and separates training from inference.",
        StyleName::WpsClean,
        CanvasAspect::PaperWide,
        85,
    );
    plan.layout.regions = vec![
        LayoutRegion {
            id: "r_input".to_string(),
            bbox: [0.06, 0.34, 0.18, 0.62],
        },
        LayoutRegion {
            id: "r_teacher".to_string(),
            bbox: [0.24, 0.12, 0.42, 0.36],
        },
        LayoutRegion {
            id: "r_residual".to_string(),
            bbox: [0.47, 0.20, 0.60, 0.42],
        },
        LayoutRegion {
            id: "r_student".to_string(),
            bbox: [0.66, 0.34, 0.84, 0.62],
        },
        LayoutRegion {
            id: "r_loss".to_string(),
            bbox: [0.68, 0.70, 0.86, 0.88],
        },
    ];
    plan.components = vec![
        Component::new(
            "c_input",
            "Input",
            ComponentRole::Input,
            VisualWeight::Normal,
            "r_input",
        ),
        Component::new(
            "c_teacher",
            "Teacher LM",
            ComponentRole::Context,
            VisualWeight::Muted,
            "r_teacher",
        ),
        Component::new(
            "c_residual",
            "Latent residual",
            ComponentRole::Main,
            VisualWeight::Strong,
            "r_residual",
        ),
        Component::new(
            "c_student",
            "Student LM",
            ComponentRole::Main,
            VisualWeight::Strong,
            "r_student",
        ),
        Component::new(
            "c_loss",
            "Task loss",
            ComponentRole::Loss,
            VisualWeight::Normal,
            "r_loss",
        ),
    ];
    plan.edges = vec![
        Edge::new(
            "e_input_to_teacher",
            "c_input",
            "c_teacher",
            "same input",
            EdgeSemantic::DataFlow,
            EdgeStyle::Solid,
            EdgeImportance::Main,
        ),
        Edge::new(
            "e_teacher_to_residual",
            "c_teacher",
            "c_residual",
            "teacher residual",
            EdgeSemantic::Supervision,
            EdgeStyle::Dash,
            EdgeImportance::Main,
        ),
        Edge::new(
            "e_residual_to_student",
            "c_residual",
            "c_student",
            "latent signal",
            EdgeSemantic::Supervision,
            EdgeStyle::Dash,
            EdgeImportance::Main,
        ),
        Edge::new(
            "e_student_to_loss",
            "c_student",
            "c_loss",
            "task objective",
            EdgeSemantic::Loss,
            EdgeStyle::Solid,
            EdgeImportance::Main,
        ),
    ];
    plan.assets.clear();
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

    let layout_map: serde_json::Value = serde_json::from_slice(
        &std::fs::read(round_dir.join("layout_map.json")).expect("layout map should exist"),
    )
    .expect("layout map should parse");
    assert_box_inside(
        object_bbox(&layout_map, "c_teacher"),
        [0.24, 0.12, 0.42, 0.36],
    );
    assert_box_inside(
        object_bbox(&layout_map, "c_student"),
        [0.66, 0.34, 0.84, 0.62],
    );
    assert_box_inside(object_bbox(&layout_map, "c_loss"), [0.68, 0.70, 0.86, 0.88]);
}

#[test]
fn renderer_tracks_large_muted_caption_component_as_annotation() {
    let temp = tempfile::tempdir().expect("tempdir");
    let round_dir = temp.path().join("round_000");
    let renderer_root = default_renderer_root().expect("renderer root");
    let mut plan = FigurePlan::mock_from_method(
        "Teacher guides a student with latent residuals.",
        StyleName::WpsClean,
        CanvasAspect::PaperWide,
        85,
    );
    plan.layout.regions.push(LayoutRegion {
        id: "r_inference".to_string(),
        bbox: [0.0, 0.0, 1.0, 1.0],
    });
    plan.components.push(Component::new(
        "c_inference_label",
        "Inference: Student Only",
        ComponentRole::Module,
        VisualWeight::Muted,
        "r_inference",
    ));
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

    let layout_map: serde_json::Value = serde_json::from_slice(
        &std::fs::read(round_dir.join("layout_map.json")).expect("layout map should exist"),
    )
    .expect("layout map should parse");
    let object = layout_map["objects"]
        .as_array()
        .expect("objects is array")
        .iter()
        .find(|object| object["id"] == "c_inference_label")
        .expect("caption object exists");
    assert_eq!(object["kind"], "annotation");
    let bbox = object_bbox_any_kind(&layout_map, "c_inference_label");
    assert!(bbox[2] - bbox[0] <= 0.35, "{bbox:?} is too wide");
    assert!(bbox[3] - bbox[1] <= 0.12, "{bbox:?} is too tall");
}

fn object_bbox(layout_map: &serde_json::Value, id: &str) -> [f64; 4] {
    let object = layout_map["objects"]
        .as_array()
        .expect("objects is array")
        .iter()
        .find(|object| object["id"] == id && object["kind"] == "component")
        .unwrap_or_else(|| panic!("component {id} exists"));
    let bbox = object["bbox"].as_array().expect("bbox is array");
    [
        bbox[0].as_f64().unwrap(),
        bbox[1].as_f64().unwrap(),
        bbox[2].as_f64().unwrap(),
        bbox[3].as_f64().unwrap(),
    ]
}

fn object_bbox_any_kind(layout_map: &serde_json::Value, id: &str) -> [f64; 4] {
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

fn assert_box_inside(inner: [f64; 4], outer: [f64; 4]) {
    let epsilon = 0.001;
    assert!(
        inner[0] >= outer[0] - epsilon,
        "{inner:?} is left of {outer:?}"
    );
    assert!(
        inner[1] >= outer[1] - epsilon,
        "{inner:?} is above {outer:?}"
    );
    assert!(
        inner[2] <= outer[2] + epsilon,
        "{inner:?} is right of {outer:?}"
    );
    assert!(
        inner[3] <= outer[3] + epsilon,
        "{inner:?} is below {outer:?}"
    );
}
