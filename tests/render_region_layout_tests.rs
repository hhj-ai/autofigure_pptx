use std::collections::BTreeMap;
use std::process::Command;
use std::time::Duration;

use methodfig::schema::{
    CanvasAspect, Component, ComponentRole, Edge, EdgeImportance, EdgeSemantic, EdgeStyle,
    FigurePlan, LayoutRegion, StyleName, VisualWeight,
};
use methodfig::style::style_by_name;
use methodfig::tools::draw_plan::generate_draw_plan_typescript;
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

#[test]
fn draw_plan_renderer_draws_connector_labels_above_boxes() {
    let temp = tempfile::tempdir().expect("tempdir");
    let round_dir = temp.path().join("round_000");
    let renderer_root = default_renderer_root().expect("renderer root");
    let style = style_by_name(StyleName::WpsClean);
    let draw_plan = methodfig::schema::DrawPlan {
        version: "0.2".to_string(),
        canvas: methodfig::schema::Canvas {
            aspect: CanvasAspect::PaperWide,
            target_width_mm: 85,
            safe_margin: 0.06,
        },
        style_tokens: BTreeMap::new(),
        objects: vec![
            methodfig::schema::DrawObject::Connector {
                id: "e1".to_string(),
                points: vec![[0.1, 0.5], [0.9, 0.5]],
                from: None,
                to: None,
                style: "main_flow".to_string(),
                label: Some(methodfig::schema::DrawLabel {
                    text: "label".to_string(),
                    bbox: [0.42, 0.46, 0.58, 0.54],
                }),
                z: 10,
            },
            methodfig::schema::DrawObject::Box {
                id: "b1".to_string(),
                bbox: [0.35, 0.35, 0.65, 0.65],
                text: "Box".to_string(),
                role: "main".to_string(),
                style: "primary_module".to_string(),
                z: 20,
            },
        ],
    };
    let code = generate_draw_plan_typescript(
        &draw_plan,
        &style,
        &round_dir,
        &renderer_root,
        &BTreeMap::new(),
    )
    .expect("draw plan code should generate");

    run_node_renderer(
        &code,
        &round_dir,
        &renderer_root,
        Duration::from_secs(20),
        false,
    )
    .expect("renderer should complete");

    let slide_xml = Command::new("unzip")
        .arg("-p")
        .arg(round_dir.join("figure.pptx"))
        .arg("ppt/slides/slide1.xml")
        .output()
        .expect("unzip should run");
    assert!(slide_xml.status.success());
    let slide_xml = String::from_utf8(slide_xml.stdout).expect("slide xml should be utf8");
    let box_index = slide_xml
        .find(">Box<")
        .expect("box text should be in slide xml");
    let label_index = slide_xml
        .find(">label<")
        .expect("label text should be in slide xml");
    assert!(
        label_index > box_index,
        "edge label should be drawn after boxes so it stays visible"
    );

    let layout_map: serde_json::Value = serde_json::from_slice(
        &std::fs::read(round_dir.join("layout_map.json")).expect("layout map should exist"),
    )
    .expect("layout map should parse");
    let edge = layout_map["objects"]
        .as_array()
        .expect("objects is array")
        .iter()
        .find(|object| object["id"] == "e1" && object["kind"] == "edge")
        .expect("edge layout entry exists");
    assert_eq!(
        edge["points"],
        serde_json::json!([[0.1, 0.5], [0.9, 0.5]]),
        "DrawPlan renderer should preserve connector points for local geometry gates"
    );
}

#[test]
fn draw_plan_renderer_applies_style_tokens_and_tracks_edge_endpoints() {
    let temp = tempfile::tempdir().expect("tempdir");
    let round_dir = temp.path().join("round_000");
    let renderer_root = default_renderer_root().expect("renderer root");
    let style = style_by_name(StyleName::WpsClean);
    let draw_plan = methodfig::schema::DrawPlan {
        version: "0.2".to_string(),
        canvas: methodfig::schema::Canvas {
            aspect: CanvasAspect::PaperWide,
            target_width_mm: 85,
            safe_margin: 0.06,
        },
        style_tokens: BTreeMap::from([
            ("primary".to_string(), "AA0000".to_string()),
            ("accent".to_string(), "00AA00".to_string()),
            ("neutral_fill".to_string(), "F0F0F0".to_string()),
            ("text".to_string(), "111111".to_string()),
        ]),
        objects: vec![
            methodfig::schema::DrawObject::Box {
                id: "main".to_string(),
                bbox: [0.10, 0.25, 0.28, 0.48],
                text: "Main".to_string(),
                role: "main".to_string(),
                style: "primary_module".to_string(),
                z: 20,
            },
            methodfig::schema::DrawObject::Box {
                id: "loss".to_string(),
                bbox: [0.42, 0.25, 0.60, 0.48],
                text: "Loss".to_string(),
                role: "loss".to_string(),
                style: "accent_module".to_string(),
                z: 21,
            },
            methodfig::schema::DrawObject::Box {
                id: "neutral".to_string(),
                bbox: [0.70, 0.25, 0.88, 0.48],
                text: "Neutral".to_string(),
                role: "context".to_string(),
                style: "regular_module".to_string(),
                z: 22,
            },
            methodfig::schema::DrawObject::Connector {
                id: "e_main_loss".to_string(),
                points: vec![[0.28, 0.36], [0.42, 0.36]],
                from: Some("main".to_string()),
                to: Some("loss".to_string()),
                style: "dash_supervision".to_string(),
                label: None,
                z: 10,
            },
        ],
    };
    let code = generate_draw_plan_typescript(
        &draw_plan,
        &style,
        &round_dir,
        &renderer_root,
        &BTreeMap::new(),
    )
    .expect("draw plan code should generate");

    run_node_renderer(
        &code,
        &round_dir,
        &renderer_root,
        Duration::from_secs(20),
        false,
    )
    .expect("renderer should complete");

    let slide_xml = Command::new("unzip")
        .arg("-p")
        .arg(round_dir.join("figure.pptx"))
        .arg("ppt/slides/slide1.xml")
        .output()
        .expect("unzip should run");
    assert!(slide_xml.status.success());
    let slide_xml = String::from_utf8(slide_xml.stdout).expect("slide xml should be utf8");
    assert!(
        slide_xml.contains("AA0000"),
        "primary token should reach PPTX XML"
    );
    assert!(
        slide_xml.contains("00AA00"),
        "accent token should reach PPTX XML"
    );
    assert!(
        slide_xml.contains("F0F0F0"),
        "neutral_fill token should reach PPTX XML"
    );
    assert!(
        slide_xml.contains("111111"),
        "text token should reach PPTX XML"
    );

    let layout_map: serde_json::Value = serde_json::from_slice(
        &std::fs::read(round_dir.join("layout_map.json")).expect("layout map should exist"),
    )
    .expect("layout map should parse");
    let edge = layout_map["objects"]
        .as_array()
        .expect("objects is array")
        .iter()
        .find(|object| object["id"] == "e_main_loss" && object["kind"] == "edge")
        .expect("edge layout entry exists");
    assert_eq!(edge["from"], "main");
    assert_eq!(edge["to"], "loss");
}

#[test]
fn draw_plan_renderer_tracks_text_metrics_and_scales_font_for_target_width() {
    let temp = tempfile::tempdir().expect("tempdir");
    let round_dir = temp.path().join("round_000");
    let renderer_root = default_renderer_root().expect("renderer root");
    let style = style_by_name(StyleName::WpsClean);
    let draw_plan = methodfig::schema::DrawPlan {
        version: "0.2".to_string(),
        canvas: methodfig::schema::Canvas {
            aspect: CanvasAspect::PaperWide,
            target_width_mm: 85,
            safe_margin: 0.06,
        },
        style_tokens: BTreeMap::new(),
        objects: vec![methodfig::schema::DrawObject::Box {
            id: "main".to_string(),
            bbox: [0.18, 0.25, 0.58, 0.55],
            text: "Student LM".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 20,
        }],
    };
    let code = generate_draw_plan_typescript(
        &draw_plan,
        &style,
        &round_dir,
        &renderer_root,
        &BTreeMap::new(),
    )
    .expect("draw plan code should generate");

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
        .find(|object| object["id"] == "main" && object["kind"] == "component")
        .expect("component exists");
    assert_eq!(object["text"], "Student LM");
    let font_size = object["font_size_pt"]
        .as_f64()
        .expect("font_size_pt should be recorded");
    let margin = object["margin_in"]
        .as_f64()
        .expect("margin_in should be recorded");
    assert!(
        font_size > style.font_sizes.module_label * 1.3,
        "font should be scaled for 85mm target width: {font_size}"
    );
    assert!(
        margin < 0.05,
        "component margin should be adaptive instead of fixed 0.05in: {margin}"
    );

    let slide_xml = Command::new("unzip")
        .arg("-p")
        .arg(round_dir.join("figure.pptx"))
        .arg("ppt/slides/slide1.xml")
        .output()
        .expect("unzip should run");
    assert!(slide_xml.status.success());
    let slide_xml = String::from_utf8(slide_xml.stdout).expect("slide xml should be utf8");
    let expected_sz = format!("sz=\"{}\"", (font_size * 100.0).round() as i64);
    assert!(
        slide_xml.contains(&expected_sz),
        "PPTX XML should contain scaled font size {expected_sz}"
    );
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
