use methodfig::schema::{
    figure_plan_schema_json, validate_stable_ids, CanvasAspect, FigurePlan, StyleName, Template,
};

#[test]
fn figure_plan_roundtrips_with_required_goal_fields() {
    let json = serde_json::json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Student model learns compact residuals from a teacher.",
            "visual_focus": ["teacher", "student"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [{"id": "main_lane", "bbox": [0.08, 0.25, 0.92, 0.72]}]
        },
        "components": [
            {
                "id": "teacher",
                "label": "Teacher LM",
                "role": "context",
                "visual_weight": "muted",
                "region": "main_lane",
                "allowed_asset_id": null
            },
            {
                "id": "student",
                "label": "Student LM",
                "role": "main",
                "visual_weight": "strong",
                "region": "main_lane",
                "allowed_asset_id": "student_icon"
            }
        ],
        "edges": [
            {
                "id": "latent_supervision",
                "from": "teacher",
                "to": "student",
                "label": "latent residual",
                "semantic": "supervision",
                "style": "dash",
                "importance": "main"
            }
        ],
        "annotations": [],
        "assets": [
            {
                "id": "student_icon",
                "type": "generated_icon",
                "prompt": "minimal student model pictogram",
                "negative_prompt": "text, letters, numbers, watermark, signature, photorealistic clutter",
                "usage": "inside_component",
                "size": "small",
                "transparent_background": true,
                "style_constraints": {
                    "flat": true,
                    "no_text": true,
                    "match_palette": "wps-clean"
                },
                "status": "missing"
            }
        ],
        "design": {
            "style": "wps-clean",
            "max_colors": 4,
            "font_policy": "wps_friendly",
            "avoid_arrow_crossing": true,
            "prefer_native_shapes": true
        }
    });

    let plan: FigurePlan = serde_json::from_value(json).expect("FigurePlan should deserialize");
    assert_eq!(plan.canvas.aspect, CanvasAspect::PaperWide);
    assert_eq!(plan.layout.template, Template::TeacherStudent);
    assert_eq!(plan.design.style, StyleName::WpsClean);
    validate_stable_ids(&plan).expect("all ids are stable");

    let encoded = serde_json::to_string(&plan).expect("FigurePlan should serialize");
    assert!(encoded.contains("teacher_student"));
}

#[test]
fn schema_command_payload_contains_figure_plan_definition() {
    let schema = figure_plan_schema_json().expect("schema json should be printable");
    assert!(schema.contains("FigurePlan"));
    assert!(schema.contains("teacher_student"));
    assert!(schema.contains("wps-clean"));
}

#[test]
fn figure_plan_defaults_missing_version_to_current_schema_version() {
    let mut json = serde_json::to_value(FigurePlan::mock_from_method(
        "Teacher guides student with latent residuals.",
        StyleName::WpsClean,
        CanvasAspect::PaperWide,
        85,
    ))
    .expect("mock plan should serialize");
    json.as_object_mut()
        .expect("FigurePlan should serialize as object")
        .remove("version");

    let plan: FigurePlan =
        serde_json::from_value(json).expect("missing version should use current default");

    assert_eq!(plan.version, "0.1");
}

#[test]
fn stable_id_validator_rejects_empty_and_duplicate_ids() {
    let mut plan = FigurePlan::mock_from_method(
        "Teacher guides student with latent residuals.",
        StyleName::WpsClean,
        CanvasAspect::PaperWide,
        85,
    );
    plan.components[1].id = plan.components[0].id.clone();
    let err = validate_stable_ids(&plan).expect_err("duplicate ids should fail");
    assert!(err.to_string().contains("duplicate"));
}
