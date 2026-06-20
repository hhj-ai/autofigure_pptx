use std::collections::BTreeMap;

use methodfig::schema::{
    validate_draw_plan, Canvas, CanvasAspect, DrawLabel, DrawObject, DrawPlan, FigurePlan,
    StyleName,
};
use methodfig::style::style_by_name;
use methodfig::tools::draw_plan::{
    draw_plan_from_figure_plan, polish_model_draw_plan_geometry,
    polish_model_draw_plan_geometry_with_figure_plan, preserve_semantic_draw_objects,
    repair_draw_plan_geometry, repair_draw_plan_geometry_with_figure_plan,
};
use serde_json::json;

#[test]
fn draw_plan_roundtrips_box_connector_and_label_geometry() {
    let plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "c_student".to_string(),
            bbox: [0.28, 0.44, 0.46, 0.62],
            text: "Student".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 20,
        },
        DrawObject::Connector {
            id: "e_teacher_student".to_string(),
            points: vec![[0.37, 0.26], [0.37, 0.44]],
            from: Some("c_teacher".to_string()),
            to: Some("c_student".to_string()),
            style: "supervision_dash".to_string(),
            label: Some(DrawLabel {
                text: "distill".to_string(),
                bbox: [0.39, 0.32, 0.47, 0.37],
            }),
            z: 10,
        },
    ]);

    let encoded = serde_json::to_string(&plan).expect("draw plan should serialize");
    assert!(encoded.contains("\"kind\":\"box\""));
    assert!(encoded.contains("\"kind\":\"connector\""));
    let decoded: DrawPlan = serde_json::from_str(&encoded).expect("draw plan should deserialize");

    assert_eq!(decoded.objects.len(), 2);
    validate_draw_plan(&decoded).expect("valid draw plan should pass");
}

#[test]
fn model_draw_plan_polish_normalizes_out_of_bounds_model_geometry() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Text {
            id: "ann_task_eq".to_string(),
            bbox: [1.08, -0.04, 1.28, 0.06],
            text: "L_task = CE(y, y_hat)".to_string(),
            style: "caption".to_string(),
            z: 30,
        },
        DrawObject::Connector {
            id: "e_task_eq".to_string(),
            points: vec![[-0.10, 0.50], [1.15, 1.10]],
            from: None,
            to: None,
            style: "normal_flow".to_string(),
            label: Some(DrawLabel {
                text: "task loss".to_string(),
                bbox: [0.92, 1.04, 1.12, 1.12],
            }),
            z: 10,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    validate_draw_plan(&plan)
        .expect("model-authored out-of-bounds geometry should be normalized before validation");
    let ann_bbox = text_box(&plan, "ann_task_eq");
    assert!(
        ann_bbox[2] - ann_bbox[0] > 0.05 && ann_bbox[3] - ann_bbox[1] > 0.03,
        "normalization should shift boxes inside the canvas without collapsing them"
    );
}

#[test]
fn draw_plan_from_figure_plan_uses_accent_style_for_loss_components() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Task loss supervises the student.",
            "visual_focus": ["student", "task_loss"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "pipeline",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "student_region", "bbox": [0.20, 0.34, 0.42, 0.58]},
                {"id": "loss_region", "bbox": [0.62, 0.34, 0.84, 0.58]}
            ]
        },
        "components": [
            {"id": "student", "label": "Student", "role": "main", "visual_weight": "strong", "region": "student_region", "allowed_asset_id": null},
            {"id": "task_loss", "label": "Task Loss", "role": "loss", "visual_weight": "normal", "region": "loss_region", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "e_student_loss", "from": "student", "to": "task_loss", "label": "", "semantic": "loss", "style": "solid", "importance": "normal"}
        ],
        "annotations": [],
        "assets": [],
        "design": {
            "style": "wps-clean",
            "max_colors": 3,
            "font_policy": "wps_friendly",
            "avoid_arrow_crossing": true,
            "prefer_native_shapes": true
        }
    }))
    .expect("figure plan should deserialize");

    let draw_plan = draw_plan_from_figure_plan(&figure_plan, &style_by_name(StyleName::WpsClean));

    assert_eq!(
        box_style(&draw_plan, "task_loss"),
        "accent_module",
        "loss/alignment objectives should use accent styling even when visual_weight is normal"
    );
    assert_eq!(box_style(&draw_plan, "student"), "primary_module");
}

#[test]
fn model_draw_plan_polish_preserves_model_authored_teacher_student_geometry() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input".to_string(),
            bbox: [0.08, 0.42, 0.18, 0.58],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher".to_string(),
            bbox: [0.22, 0.18, 0.42, 0.34],
            text: "Teacher".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student".to_string(),
            bbox: [0.62, 0.46, 0.84, 0.62],
            text: "Student".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Connector {
            id: "teacher_to_student".to_string(),
            points: vec![[0.42, 0.26], [0.62, 0.54]],
            from: Some("teacher".to_string()),
            to: Some("student".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "distill".to_string(),
                bbox: [0.47, 0.39, 0.58, 0.45],
            }),
            z: 10,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.52, 0.72, 0.59, 0.79],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "neutral_module".to_string(),
            z: 23,
        },
        DrawObject::Text {
            id: "anno_training".to_string(),
            bbox: [0.22, 0.10, 0.34, 0.16],
            text: "Training".to_string(),
            style: "annotation".to_string(),
            z: 40,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    assert_eq!(object_box(&plan, "teacher"), [0.22, 0.18, 0.42, 0.34]);
    assert_eq!(object_box(&plan, "student"), [0.62, 0.46, 0.84, 0.62]);
    assert!(
        !text_object_exists(&plan, "anno_training"),
        "phase-only text should be folded into modules or removed, not floated over routes"
    );
    let task_loss = object_box(&plan, "task_loss");
    assert!(
        task_loss[2] - task_loss[0] >= 0.10 && task_loss[3] - task_loss[1] >= 0.10,
        "model polish should expand tiny semantic boxes enough for the local quality gate: {task_loss:?}"
    );
    let edge = connector(&plan, "teacher_to_student");
    assert!(
        edge.points.windows(2).all(|window| {
            (window[0][0] - window[1][0]).abs() < 0.0001
                || (window[0][1] - window[1][1]).abs() < 0.0001
        }),
        "polish may orthogonalize connectors but must not apply a local teacher-student template: {:?}",
        edge.points
    );
}

#[test]
fn model_draw_plan_polish_syncs_connector_style_from_current_figure_plan() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher-student distillation with latent residual supervision.",
            "visual_focus": ["teacher", "student", "latent_residual"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "teacher_region", "bbox": [0.56, 0.18, 0.72, 0.34]},
                {"id": "student_region", "bbox": [0.20, 0.38, 0.36, 0.54]},
                {"id": "latent_region", "bbox": [0.42, 0.20, 0.56, 0.32]}
            ]
        },
        "components": [
            {"id": "teacher", "label": "Teacher", "role": "context", "visual_weight": "normal", "region": "teacher_region", "allowed_asset_id": null},
            {"id": "student", "label": "Student", "role": "main", "visual_weight": "strong", "region": "student_region", "allowed_asset_id": null},
            {"id": "latent_residual", "label": "Latent residual", "role": "loss", "visual_weight": "normal", "region": "latent_region", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "e_residual_student", "from": "latent_residual", "to": "student", "label": "", "semantic": "supervision", "style": "dash", "importance": "main"}
        ],
        "annotations": [],
        "assets": [],
        "design": {
            "style": "wps-clean",
            "max_colors": 3,
            "font_policy": "wps_friendly",
            "avoid_arrow_crossing": true,
            "prefer_native_shapes": true
        }
    }))
    .expect("figure plan should deserialize");
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "teacher".to_string(),
            bbox: [0.58, 0.20, 0.72, 0.36],
            text: "Teacher".to_string(),
            role: "context".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student".to_string(),
            bbox: [0.20, 0.36, 0.34, 0.52],
            text: "Student".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "latent_residual".to_string(),
            bbox: [0.42, 0.19, 0.56, 0.29],
            text: "Latent residual".to_string(),
            role: "loss".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Connector {
            id: "e_residual_student".to_string(),
            points: vec![[0.42, 0.24], [0.38, 0.24], [0.38, 0.44], [0.34, 0.44]],
            from: Some("latent_residual".to_string()),
            to: Some("student".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut draw_plan, &figure_plan);

    assert_eq!(object_box(&draw_plan, "teacher"), [0.58, 0.20, 0.72, 0.36]);
    assert_eq!(object_box(&draw_plan, "student"), [0.20, 0.36, 0.34, 0.52]);
    assert_eq!(
        input_teacher_style(&draw_plan, "e_residual_student"),
        "dashed_supervision"
    );
}

#[test]
fn model_draw_plan_polish_honors_solid_supervision_style_from_figure_plan() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher residual supervises the student through an explicit residual block.",
            "visual_focus": ["teacher", "residual"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "teacher_region", "bbox": [0.18, 0.20, 0.34, 0.36]},
                {"id": "residual_region", "bbox": [0.48, 0.20, 0.64, 0.36]}
            ]
        },
        "components": [
            {"id": "teacher", "label": "Teacher", "role": "context", "visual_weight": "normal", "region": "teacher_region", "allowed_asset_id": null},
            {"id": "residual", "label": "Residual", "role": "loss", "visual_weight": "normal", "region": "residual_region", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "e_teacher_residual", "from": "teacher", "to": "residual", "label": "", "semantic": "supervision", "style": "solid", "importance": "normal"}
        ],
        "annotations": [],
        "assets": [],
        "design": {
            "style": "wps-clean",
            "max_colors": 3,
            "font_policy": "wps_friendly",
            "avoid_arrow_crossing": true,
            "prefer_native_shapes": true
        }
    }))
    .expect("figure plan should deserialize");
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "teacher".to_string(),
            bbox: [0.18, 0.20, 0.34, 0.36],
            text: "Teacher".to_string(),
            role: "context".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "residual".to_string(),
            bbox: [0.48, 0.20, 0.64, 0.36],
            text: "Residual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 21,
        },
        DrawObject::Connector {
            id: "e_teacher_residual".to_string(),
            points: vec![[0.34, 0.28], [0.48, 0.28]],
            from: Some("teacher".to_string()),
            to: Some("residual".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 10,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut draw_plan, &figure_plan);

    assert_eq!(
        input_teacher_style(&draw_plan, "e_teacher_residual"),
        "normal_flow"
    );
}

#[test]
fn model_draw_plan_polish_removes_connectors_absent_from_figure_plan() {
    let figure_plan = teacher_student_semantic_plan();
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "student_model".to_string(),
            bbox: [0.20, 0.42, 0.42, 0.58],
            text: "Student".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "task_output".to_string(),
            bbox: [0.66, 0.42, 0.78, 0.58],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.84, 0.42, 0.95, 0.58],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "neutral_module".to_string(),
            z: 22,
        },
        DrawObject::Connector {
            id: "output_to_loss".to_string(),
            points: vec![[0.78, 0.50], [0.84, 0.50]],
            from: Some("task_output".to_string()),
            to: Some("task_loss".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_student_to_taskloss".to_string(),
            points: vec![[0.42, 0.50], [0.84, 0.50]],
            from: Some("student_model".to_string()),
            to: Some("task_loss".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 11,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut draw_plan, &figure_plan);

    assert_eq!(
        connector_endpoints(&draw_plan, "output_to_loss"),
        (Some("task_output"), Some("task_loss"))
    );
    assert!(
        !connector_object_exists(&draw_plan, "e_student_to_taskloss"),
        "model polish should remove semantic connectors that connect FigurePlan components but are absent from FigurePlan edges"
    );
    assert_eq!(
        object_box(&draw_plan, "student_model"),
        [0.20, 0.42, 0.42, 0.58]
    );
}

#[test]
fn model_draw_plan_polish_adds_missing_connectors_declared_by_figure_plan() {
    let figure_plan = teacher_student_semantic_plan();
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "teacher_model".to_string(),
            bbox: [0.62, 0.30, 0.82, 0.46],
            text: "Teacher".to_string(),
            role: "context".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student_model".to_string(),
            bbox: [0.18, 0.30, 0.38, 0.46],
            text: "Student".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut draw_plan, &figure_plan);

    assert_eq!(
        connector_endpoints(&draw_plan, "teacher_latent_residual"),
        (Some("teacher_model"), Some("student_model")),
        "FigurePlan edges between existing boxes must be rendered even when the model omits them"
    );
    assert_eq!(
        input_teacher_style(&draw_plan, "teacher_latent_residual"),
        "dashed_supervision"
    );
    let residual = connector(&draw_plan, "teacher_latent_residual");
    assert!(
        residual.points.windows(2).all(|window| {
            (window[0][0] - window[1][0]).abs() < 0.0001
                || (window[0][1] - window[1][1]).abs() < 0.0001
        }),
        "synthesized semantic connector should use clean orthogonal geometry: {:?}",
        residual.points
    );
}

#[test]
fn model_draw_plan_polish_adds_missing_figure_plan_note_components() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Student branch is used for inference.",
            "visual_focus": ["student", "inference"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "student_region", "bbox": [0.58, 0.34, 0.82, 0.58]},
                {"id": "inference_note", "bbox": [0.66, 0.70, 0.94, 0.86]}
            ]
        },
        "components": [
            {"id": "student", "label": "Student", "role": "main", "visual_weight": "strong", "region": "student_region", "allowed_asset_id": null},
            {"id": "inference", "label": "Inference: Student only", "role": "output", "visual_weight": "normal", "region": "inference_note", "allowed_asset_id": null}
        ],
        "edges": [],
        "annotations": [],
        "assets": [],
        "design": {
            "style": "wps-clean",
            "max_colors": 3,
            "font_policy": "wps_friendly",
            "avoid_arrow_crossing": true,
            "prefer_native_shapes": true
        }
    }))
    .expect("figure plan should deserialize");
    let mut draw_plan = minimal_draw_plan(vec![DrawObject::Box {
        id: "student".to_string(),
        bbox: [0.62, 0.42, 0.82, 0.58],
        text: "Student".to_string(),
        role: "main".to_string(),
        style: "primary_module".to_string(),
        z: 20,
    }]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut draw_plan, &figure_plan);

    assert!(
        box_object_exists(&draw_plan, "inference"),
        "FigurePlan components must not disappear from the editable DrawPlan just because the model omitted them"
    );
    assert!(
        box_text(&draw_plan, "inference")
            .to_lowercase()
            .contains("student only"),
        "the restored note component should preserve the model's semantic label"
    );
    assert!(
        !draw_plan.objects.iter().any(|object| matches!(
            object,
            DrawObject::Connector { from, to, .. }
                if from.as_deref() == Some("student") && to.as_deref() == Some("inference")
        )),
        "orphan note-like FigurePlan components should remain visible as editable notes without adding clutter connectors"
    );
}

#[test]
fn model_draw_plan_polish_moves_outer_orphan_inference_component_near_student() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher-student distillation with student-only inference.",
            "visual_focus": ["comp_student", "comp_inference_note"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "student_region", "bbox": [0.28, 0.62, 0.42, 0.92]},
                {"id": "inference_note", "bbox": [0.06, 0.02, 0.24, 0.14]}
            ]
        },
        "components": [
            {"id": "comp_student", "label": "Student", "role": "main", "visual_weight": "strong", "region": "student_region", "allowed_asset_id": null},
            {"id": "comp_inference_note", "label": "Inference: student only", "role": "context", "visual_weight": "muted", "region": "inference_note", "allowed_asset_id": null}
        ],
        "edges": [],
        "annotations": [],
        "assets": [],
        "design": {
            "style": "wps-clean",
            "max_colors": 3,
            "font_policy": "wps_friendly",
            "avoid_arrow_crossing": true,
            "prefer_native_shapes": true
        }
    }))
    .expect("figure plan should deserialize");
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_student".to_string(),
            bbox: [0.28, 0.62, 0.42, 0.92],
            text: "Student".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "comp_inference_note".to_string(),
            bbox: [0.0768, 0.03, 0.2232, 0.13],
            text: "Inference: student only".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 21,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut draw_plan, &figure_plan);

    let student = object_box(&draw_plan, "comp_student");
    let note = object_box(&draw_plan, "comp_inference_note");
    assert!(
        !box_touches_test_margin(note),
        "orphan inference component should be pulled out of the page margin: {note:?}"
    );
    assert!(
        note[0] > student[2] && (center_y(note) - center_y(student)).abs() < 0.12,
        "orphan inference component should sit near the student branch without a clutter connector: note={note:?}, student={student:?}"
    );
}

#[test]
fn model_draw_plan_polish_moves_near_overlapping_inference_component_off_semantic_boxes() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Student-only inference is indicated without hiding the latent residual.",
            "visual_focus": ["student", "latent_residual", "inference_badge"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "student_region", "bbox": [0.74, 0.20, 0.90, 0.56]},
                {"id": "latent_region", "bbox": [0.36, 0.495, 0.64, 0.595]},
                {"id": "inference_region", "bbox": [0.48, 0.37, 0.688, 0.47]}
            ]
        },
        "components": [
            {"id": "student", "label": "Student", "role": "main", "visual_weight": "strong", "region": "student_region", "allowed_asset_id": null},
            {"id": "latent_residual", "label": "Latent Residual", "role": "loss", "visual_weight": "strong", "region": "latent_region", "allowed_asset_id": null},
            {"id": "inference_badge", "label": "Inference: student only", "role": "context", "visual_weight": "muted", "region": "inference_region", "allowed_asset_id": null}
        ],
        "edges": [],
        "annotations": [],
        "assets": [],
        "design": {
            "style": "wps-clean",
            "max_colors": 3,
            "font_policy": "wps_friendly",
            "avoid_arrow_crossing": true,
            "prefer_native_shapes": true
        }
    }))
    .expect("figure plan should deserialize");
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "student".to_string(),
            bbox: [0.74, 0.20, 0.90, 0.56],
            text: "Student".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "latent_residual".to_string(),
            bbox: [0.36, 0.495, 0.64, 0.595],
            text: "Latent Residual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "inference_badge".to_string(),
            bbox: [0.4804, 0.37, 0.688, 0.47],
            text: "Inference: student only".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 22,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut draw_plan, &figure_plan);

    let badge = object_box(&draw_plan, "inference_badge");
    let residual = object_box(&draw_plan, "latent_residual");
    let student = object_box(&draw_plan, "student");
    assert!(
        intersection_area([badge[0], badge[1], badge[2], badge[3] + 0.04], residual) == 0.0,
        "inference badge should not sit immediately above and horizontally over the latent residual box: badge={badge:?}, residual={residual:?}"
    );
    assert!(
        (center_y(badge) - center_y(student)).abs() < 0.18,
        "inference badge should remain visually associated with the student branch: badge={badge:?}, student={student:?}"
    );
}

#[test]
fn model_draw_plan_polish_moves_inference_note_off_main_flow_and_removes_duplicate_caption() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Student-only inference should be a compact side note, not an overlay on the main flow.",
            "visual_focus": ["comp_source_input", "comp_student_encoder", "comp_inference_note"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "input_region", "bbox": [0.02, 0.42, 0.12, 0.52]},
                {"id": "teacher_region", "bbox": [0.18, 0.38, 0.32, 0.48]},
                {"id": "student_region", "bbox": [0.56, 0.38, 0.70, 0.48]},
                {"id": "inference_region", "bbox": [0.30, 0.37, 0.52, 0.49]}
            ]
        },
        "components": [
            {"id": "comp_source_input", "label": "Input x", "role": "input", "visual_weight": "normal", "region": "input_region", "allowed_asset_id": null},
            {"id": "comp_teacher_encoder", "label": "Teacher Encoder", "role": "module", "visual_weight": "muted", "region": "teacher_region", "allowed_asset_id": null},
            {"id": "comp_student_encoder", "label": "Student Encoder", "role": "main", "visual_weight": "strong", "region": "student_region", "allowed_asset_id": null},
            {"id": "comp_inference_note", "label": "Inference: Student only", "role": "output", "visual_weight": "normal", "region": "inference_region", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "edge_input_to_teacher", "from": "comp_source_input", "to": "comp_teacher_encoder", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "edge_input_to_student", "from": "comp_source_input", "to": "comp_student_encoder", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"}
        ],
        "annotations": [],
        "assets": [],
        "design": {
            "style": "wps-clean",
            "max_colors": 3,
            "font_policy": "wps_friendly",
            "avoid_arrow_crossing": true,
            "prefer_native_shapes": true
        }
    }))
    .expect("figure plan should deserialize");
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_source_input".to_string(),
            bbox: [0.02, 0.42, 0.12, 0.52],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "comp_teacher_encoder".to_string(),
            bbox: [0.18, 0.38, 0.32, 0.48],
            text: "Teacher\nEncoder".to_string(),
            role: "module".to_string(),
            style: "muted_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "comp_student_encoder".to_string(),
            bbox: [0.56, 0.38, 0.70, 0.48],
            text: "Student\nEncoder".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Connector {
            id: "edge_input_to_student".to_string(),
            points: vec![[0.12, 0.43], [0.56, 0.43]],
            from: Some("comp_source_input".to_string()),
            to: Some("comp_student_encoder".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Box {
            id: "comp_inference_note".to_string(),
            bbox: [0.3048, 0.3724, 0.52, 0.4876],
            text: "Inference: Student only".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 23,
        },
        DrawObject::Text {
            id: "label_inference".to_string(),
            bbox: [0.76, 0.14, 0.98, 0.18],
            text: "Inference: student only".to_string(),
            style: "caption".to_string(),
            z: 24,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut draw_plan, &figure_plan);

    let note = object_box(&draw_plan, "comp_inference_note");
    let student = object_box(&draw_plan, "comp_student_encoder");
    let main_flow = connector(&draw_plan, "edge_input_to_student");
    assert!(
        !label_intersects_any_segment(note, &main_flow.points),
        "inference note should not cover the main input-to-student flow: note={note:?}, edge={:?}",
        main_flow.points
    );
    assert!(
        (center_y(note) - center_y(student)).abs() < 0.18,
        "inference note should remain associated with the student branch: note={note:?}, student={student:?}"
    );
    assert!(
        !text_object_exists(&draw_plan, "label_inference"),
        "duplicate inference caption should be removed when an editable inference note box already exists"
    );
}

#[test]
fn model_draw_plan_polish_moves_touching_task_loss_to_short_vertical_route() {
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_student_head".to_string(),
            bbox: [0.76, 0.22, 0.90, 0.32],
            text: "Student\nOutput Head".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "comp_task_loss".to_string(),
            bbox: [0.90, 0.22, 1.00, 0.32],
            text: "Task\nLoss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 21,
        },
        DrawObject::Connector {
            id: "edge_head_to_task_loss".to_string(),
            points: vec![[0.90, 0.27], [0.90, 0.31], [0.92, 0.31], [0.92, 0.27]],
            from: Some("comp_student_head".to_string()),
            to: Some("comp_task_loss".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
    ]);

    polish_model_draw_plan_geometry(&mut draw_plan);

    let head = object_box(&draw_plan, "comp_student_head");
    let loss = object_box(&draw_plan, "comp_task_loss");
    assert!(
        loss[1] > head[3] + 0.03,
        "touching task loss should move below the output head instead of forcing a tiny loop: head={head:?}, loss={loss:?}"
    );
    let edge = connector(&draw_plan, "edge_head_to_task_loss");
    assert_eq!(edge.points.len(), 2, "{:?}", edge.points);
    assert!(
        (edge.points[0][0] - edge.points[1][0]).abs() < 0.0001,
        "moved task loss should use a short vertical connector: {:?}",
        edge.points
    );
}

#[test]
fn model_draw_plan_polish_reroutes_residual_feedback_off_reverse_shared_segment() {
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "teacher_latent".to_string(),
            bbox: [0.22, 0.24, 0.42, 0.34],
            text: "Teacher\nLatent z_t".to_string(),
            role: "module".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "latent_residual".to_string(),
            bbox: [0.58, 0.26, 0.70, 0.36],
            text: "Latent Residual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student_encoder".to_string(),
            bbox: [0.22, 0.52, 0.42, 0.62],
            text: "Student\nEncoder".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Connector {
            id: "e_teacher_residual".to_string(),
            points: vec![[0.42, 0.31], [0.58, 0.31]],
            from: Some("teacher_latent".to_string()),
            to: Some("latent_residual".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_residual_student".to_string(),
            points: vec![[0.58, 0.31], [0.42, 0.31], [0.42, 0.52]],
            from: Some("latent_residual".to_string()),
            to: Some("student_encoder".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 11,
        },
    ]);

    polish_model_draw_plan_geometry(&mut draw_plan);

    let teacher_residual = connector(&draw_plan, "e_teacher_residual");
    let residual_student = connector(&draw_plan, "e_residual_student");
    assert!(
        !connectors_share_reversed_segment(&teacher_residual.points, &residual_student.points),
        "residual feedback should not backtrack over the teacher-to-residual segment: teacher={:?}, feedback={:?}",
        teacher_residual.points,
        residual_student.points
    );
    assert_point_close(residual_student.points[0], [0.58, 0.36]);
}

#[test]
fn model_draw_plan_polish_simplifies_diagonal_input_to_teacher_branch() {
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input_text".to_string(),
            bbox: [0.03, 0.42, 0.13, 0.52],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher_encoder".to_string(),
            bbox: [0.22, 0.08, 0.42, 0.18],
            text: "Teacher\nEncoder".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Connector {
            id: "e_input_teacher".to_string(),
            points: vec![[0.08, 0.42], [0.08, 0.30], [0.32, 0.30], [0.32, 0.18]],
            from: Some("input_text".to_string()),
            to: Some("teacher_encoder".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
    ]);

    polish_model_draw_plan_geometry(&mut draw_plan);

    let edge = connector(&draw_plan, "e_input_teacher");
    assert!(
        edge.points.len() <= 3,
        "diagonal input-to-teacher branch should use a short L-shape instead of a 4-point S-like path: {:?}",
        edge.points
    );
}

#[test]
fn model_draw_plan_polish_moves_inference_note_out_of_residual_student_gap() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Inference note should not sit between the student and residual objective.",
            "visual_focus": ["comp_student", "comp_latent_residual", "comp_inference_note"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "source_region", "bbox": [0.05, 0.77, 0.95, 0.87]},
                {"id": "student_region", "bbox": [0.06, 0.38, 0.34, 0.62]},
                {"id": "teacher_region", "bbox": [0.66, 0.38, 0.94, 0.62]},
                {"id": "residual_region", "bbox": [0.38, 0.315, 0.62, 0.435]},
                {"id": "task_loss_region", "bbox": [0.08, 0.16, 0.32, 0.28]},
                {"id": "output_region", "bbox": [0.08, 0.00, 0.32, 0.12]},
                {"id": "inference_region", "bbox": [0.38, 0.45, 0.60, 0.55]}
            ]
        },
        "components": [
            {"id": "comp_source_input", "label": "Task Input x", "role": "input", "visual_weight": "normal", "region": "source_region", "allowed_asset_id": null},
            {"id": "comp_student", "label": "Student", "role": "main", "visual_weight": "strong", "region": "student_region", "allowed_asset_id": null},
            {"id": "comp_teacher", "label": "Teacher", "role": "context", "visual_weight": "normal", "region": "teacher_region", "allowed_asset_id": null},
            {"id": "comp_latent_residual", "label": "Latent Residual", "role": "loss", "visual_weight": "strong", "region": "residual_region", "allowed_asset_id": null},
            {"id": "comp_task_loss", "label": "Task Loss", "role": "loss", "visual_weight": "normal", "region": "task_loss_region", "allowed_asset_id": null},
            {"id": "comp_output", "label": "ŷ", "role": "output", "visual_weight": "normal", "region": "output_region", "allowed_asset_id": null},
            {"id": "comp_inference_note", "label": "Inference: Student only", "role": "context", "visual_weight": "muted", "region": "inference_region", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "edge_input_to_student", "from": "comp_source_input", "to": "comp_student", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "edge_input_to_teacher", "from": "comp_source_input", "to": "comp_teacher", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "edge_teacher_to_residual", "from": "comp_teacher", "to": "comp_latent_residual", "label": "", "semantic": "supervision", "style": "solid", "importance": "normal"},
            {"id": "edge_residual_to_student", "from": "comp_latent_residual", "to": "comp_student", "label": "", "semantic": "supervision", "style": "dash", "importance": "main"},
            {"id": "edge_student_to_output", "from": "comp_student", "to": "comp_output", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "edge_student_to_task_loss", "from": "comp_student", "to": "comp_task_loss", "label": "", "semantic": "supervision", "style": "solid", "importance": "normal"}
        ],
        "annotations": [],
        "assets": [],
        "design": {
            "style": "wps-clean",
            "max_colors": 3,
            "font_policy": "wps_friendly",
            "avoid_arrow_crossing": true,
            "prefer_native_shapes": true
        }
    }))
    .expect("figure plan should deserialize");
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_source_input".to_string(),
            bbox: [0.05, 0.77, 0.95, 0.87],
            text: "Task Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "comp_student".to_string(),
            bbox: [0.06, 0.38, 0.34, 0.62],
            text: "Student".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "comp_teacher".to_string(),
            bbox: [0.66, 0.38, 0.94, 0.62],
            text: "Teacher".to_string(),
            role: "context".to_string(),
            style: "neutral_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "comp_latent_residual".to_string(),
            bbox: [0.38, 0.315, 0.62, 0.435],
            text: "Latent Residual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "comp_task_loss".to_string(),
            bbox: [0.08, 0.16, 0.32, 0.28],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "comp_output".to_string(),
            bbox: [0.08, 0.00, 0.32, 0.12],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 25,
        },
        DrawObject::Box {
            id: "comp_inference_note".to_string(),
            bbox: [0.38, 0.45, 0.60, 0.55],
            text: "Inference: Student only".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 26,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut draw_plan, &figure_plan);

    let note = object_box(&draw_plan, "comp_inference_note");
    let residual = object_box(&draw_plan, "comp_latent_residual");
    assert_eq!(
        intersection_area(note, expand_box(residual, 0.04)),
        0.0,
        "inference note should leave whitespace around the residual objective: note={note:?}, residual={residual:?}"
    );
}

#[test]
fn model_draw_plan_polish_moves_inference_note_out_of_input_student_corridor() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Inference note should stay outside the main input-to-student flow.",
            "visual_focus": ["c_input", "c_student_enc", "c_inference"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "input_source", "bbox": [0.42, 0.54, 0.58, 0.68]},
                {"id": "student_enc", "bbox": [0.08, 0.52, 0.22, 0.70]},
                {"id": "inference_note", "bbox": [0.26, 0.54, 0.42, 0.68]}
            ]
        },
        "components": [
            {"id": "c_input", "label": "Input x", "role": "input", "visual_weight": "normal", "region": "input_source", "allowed_asset_id": null},
            {"id": "c_student_enc", "label": "Student\nEncoder", "role": "main", "visual_weight": "strong", "region": "student_enc", "allowed_asset_id": null},
            {"id": "c_inference", "label": "Inference: student only", "role": "output", "visual_weight": "muted", "region": "inference_note", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "e_input_student", "from": "c_input", "to": "c_student_enc", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"}
        ],
        "annotations": [],
        "assets": [],
        "design": {
            "style": "wps-clean",
            "max_colors": 3,
            "font_policy": "wps_friendly",
            "avoid_arrow_crossing": true,
            "prefer_native_shapes": true
        }
    }))
    .expect("figure plan should deserialize");
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "c_input".to_string(),
            bbox: [0.42, 0.54, 0.58, 0.68],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "c_student_enc".to_string(),
            bbox: [0.08, 0.52, 0.22, 0.70],
            text: "Student\nEncoder".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "c_inference".to_string(),
            bbox: [0.26, 0.54, 0.42, 0.68],
            text: "Inference: student only".to_string(),
            role: "output".to_string(),
            style: "muted_module".to_string(),
            z: 22,
        },
        DrawObject::Connector {
            id: "e_input_student".to_string(),
            points: vec![[0.42, 0.61], [0.22, 0.61]],
            from: Some("c_input".to_string()),
            to: Some("c_student_enc".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut draw_plan, &figure_plan);

    let note = object_box(&draw_plan, "c_inference");
    let edge = connector(&draw_plan, "e_input_student");
    assert!(
        !label_intersects_any_segment(note, edge.points),
        "inference note should move out of the input-to-student connector corridor: note={note:?}, edge={:?}",
        edge.points
    );
    assert!(
        note[1] > object_box(&draw_plan, "c_student_enc")[3] + 0.02
            || note[3] < object_box(&draw_plan, "c_student_enc")[1] - 0.02,
        "inference note should sit outside the student encoder row: note={note:?}"
    );
}

#[test]
fn model_draw_plan_polish_moves_inference_annotation_away_from_student_label() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Inference note should stay editable without colliding with the student label.",
            "visual_focus": ["student_encoder", "ann_inference"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "student_label_region", "bbox": [0.62, 0.12, 0.90, 0.22]},
                {"id": "student_encoder_region", "bbox": [0.62, 0.22, 0.90, 0.34]},
                {"id": "student_latent_region", "bbox": [0.62, 0.40, 0.90, 0.50]}
            ]
        },
        "components": [
            {"id": "student_label", "label": "Student (trainable)", "role": "context", "visual_weight": "muted", "region": "student_label_region", "allowed_asset_id": null},
            {"id": "student_encoder", "label": "Student Encoder", "role": "main", "visual_weight": "strong", "region": "student_encoder_region", "allowed_asset_id": null},
            {"id": "student_latent", "label": "Latent h_S", "role": "data", "visual_weight": "normal", "region": "student_latent_region", "allowed_asset_id": null}
        ],
        "edges": [],
        "annotations": [
            {"id": "ann_inference", "label": "Inference: student only", "target_id": "student_encoder", "bbox": [0.56, 0.90, 0.94, 0.96]}
        ],
        "assets": [],
        "design": {
            "style": "wps-clean",
            "max_colors": 3,
            "font_policy": "wps_friendly",
            "avoid_arrow_crossing": true,
            "prefer_native_shapes": true
        }
    }))
    .expect("figure plan should deserialize");
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "student_label".to_string(),
            bbox: [0.62, 0.12, 0.90, 0.22],
            text: "Student (trainable)".to_string(),
            role: "annotation".to_string(),
            style: "annotation".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student_encoder".to_string(),
            bbox: [0.62, 0.22, 0.90, 0.34],
            text: "Student Encoder".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student_latent".to_string(),
            bbox: [0.62, 0.40, 0.90, 0.50],
            text: "Latent h_S".to_string(),
            role: "data".to_string(),
            style: "neutral_module".to_string(),
            z: 22,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut draw_plan, &figure_plan);

    let annotation = text_box(&draw_plan, "ann_inference");
    assert_eq!(
        intersection_area(annotation, object_box(&draw_plan, "student_label")),
        0.0,
        "inference annotation should not collide with the student label: {annotation:?}"
    );
    assert_eq!(
        intersection_area(annotation, object_box(&draw_plan, "student_encoder")),
        0.0,
        "inference annotation should not collide with the student encoder: {annotation:?}"
    );
    assert_eq!(
        intersection_area(annotation, object_box(&draw_plan, "student_latent")),
        0.0,
        "inference annotation should not collide with the student latent box: {annotation:?}"
    );
}

#[test]
fn model_draw_plan_polish_routes_prediction_to_task_loss_around_student_branch() {
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "student_encoder".to_string(),
            bbox: [0.62, 0.22, 0.90, 0.34],
            text: "Student Encoder".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student_latent".to_string(),
            bbox: [0.62, 0.40, 0.90, 0.50],
            text: "Latent h_S".to_string(),
            role: "data".to_string(),
            style: "neutral_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student_head".to_string(),
            bbox: [0.62, 0.56, 0.90, 0.66],
            text: "Output Head".to_string(),
            role: "output".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "prediction".to_string(),
            bbox: [0.62, 0.71, 0.90, 0.81],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "task_loss_node".to_string(),
            bbox: [0.62, 0.03, 0.90, 0.13],
            text: "L_task".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 24,
        },
        DrawObject::Connector {
            id: "e_pred_to_task_loss".to_string(),
            points: vec![[0.76, 0.71], [0.76, 0.13]],
            from: Some("prediction".to_string()),
            to: Some("task_loss_node".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
    ]);

    polish_model_draw_plan_geometry(&mut draw_plan);

    let edge = connector(&draw_plan, "e_pred_to_task_loss");
    let prediction = object_box(&draw_plan, "prediction");
    assert!(
        edge.points.len() >= 4 && edge.points.iter().any(|point| point[0] > prediction[2]),
        "prediction-to-task-loss edge should route around the right side, not straight through the student branch: {:?}",
        edge.points
    );
    for id in ["student_encoder", "student_latent", "student_head"] {
        assert!(
            !label_intersects_any_segment(object_box(&draw_plan, id), edge.points),
            "prediction-to-task-loss edge should not cross {id}: {:?}",
            edge.points
        );
    }
}

#[test]
fn model_draw_plan_polish_repairs_connector_with_missing_points() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Student latent is produced by the student.",
            "visual_focus": ["comp_student", "comp_student_latent"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "student_branch", "bbox": [0.20, 0.56, 0.46, 0.88]}
            ]
        },
        "components": [
            {"id": "comp_student", "label": "Student", "role": "main", "visual_weight": "strong", "region": "student_branch", "allowed_asset_id": null},
            {"id": "comp_student_latent", "label": "Student latent h_S", "role": "main", "visual_weight": "strong", "region": "student_branch", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "edge_student_to_latent", "from": "comp_student", "to": "comp_student_latent", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"}
        ],
        "annotations": [],
        "assets": [],
        "design": {
            "style": "wps-clean",
            "max_colors": 3,
            "font_policy": "wps_friendly",
            "avoid_arrow_crossing": true,
            "prefer_native_shapes": true
        }
    }))
    .expect("figure plan should deserialize");
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_student".to_string(),
            bbox: [0.24, 0.60, 0.40, 0.72],
            text: "Student".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "comp_student_latent".to_string(),
            bbox: [0.24, 0.76, 0.40, 0.86],
            text: "Student latent h_S".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Connector {
            id: "edge_student_to_latent".to_string(),
            points: vec![[0.32, 0.72]],
            from: Some("comp_student".to_string()),
            to: Some("comp_student_latent".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut draw_plan, &figure_plan);

    let edge = connector(&draw_plan, "edge_student_to_latent");
    assert!(
        edge.points.len() >= 2,
        "model polish should repair degenerate connector points before DrawPlan validation: {:?}",
        edge.points
    );
    validate_draw_plan(&draw_plan).expect("repaired connector should validate");
}

#[test]
fn model_draw_plan_polish_routes_near_vertical_input_branch_straight() {
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input_text".to_string(),
            bbox: [0.45, 0.77, 0.55, 0.87],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student_encoder".to_string(),
            bbox: [0.38, 0.18, 0.56, 0.32],
            text: "Student".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Connector {
            id: "e_input_to_student".to_string(),
            points: vec![[0.45, 0.82], [0.47, 0.82], [0.47, 0.32]],
            from: Some("input_text".to_string()),
            to: Some("student_encoder".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
    ]);

    polish_model_draw_plan_geometry(&mut draw_plan);

    let edge = connector(&draw_plan, "e_input_to_student");
    assert_eq!(edge.points.len(), 2, "{:?}", edge.points);
    assert_point_close(edge.points[0], [0.47, 0.77]);
    assert_point_close(edge.points[1], [0.47, 0.32]);
}

#[test]
fn model_draw_plan_polish_moves_input_branch_elbow_out_of_source_row() {
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input_text".to_string(),
            bbox: [0.45, 0.77, 0.55, 0.87],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher_encoder".to_string(),
            bbox: [0.06, 0.18, 0.24, 0.32],
            text: "Teacher".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 21,
        },
        DrawObject::Connector {
            id: "e_input_to_teacher".to_string(),
            points: vec![[0.45, 0.82], [0.15, 0.82], [0.15, 0.32]],
            from: Some("input_text".to_string()),
            to: Some("teacher_encoder".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
    ]);

    polish_model_draw_plan_geometry(&mut draw_plan);

    let edge = connector(&draw_plan, "e_input_to_teacher");
    assert!(edge.points.len() <= 3, "{:?}", edge.points);
    assert!(
        edge.points[1][1] < 0.70,
        "branch elbow should sit between source and target instead of hugging the input row: {:?}",
        edge.points
    );
}

#[test]
fn model_draw_plan_polish_aligns_output_box_with_source_for_straight_flow() {
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "student_latent".to_string(),
            bbox: [0.38, 0.42, 0.56, 0.54],
            text: "Latent h_S".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student_output".to_string(),
            bbox: [0.74, 0.24, 0.90, 0.36],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Connector {
            id: "e_student_to_output".to_string(),
            points: vec![[0.56, 0.48], [0.82, 0.48], [0.82, 0.36]],
            from: Some("student_latent".to_string()),
            to: Some("student_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
    ]);

    polish_model_draw_plan_geometry(&mut draw_plan);

    let latent = object_box(&draw_plan, "student_latent");
    let output = object_box(&draw_plan, "student_output");
    assert!(
        (center_y(latent) - center_y(output)).abs() < 0.001,
        "output should align with its source for a clean horizontal flow: output={output:?}"
    );
    let edge = connector(&draw_plan, "e_student_to_output");
    assert_eq!(edge.points.len(), 2, "{:?}", edge.points);
    assert!((edge.points[0][1] - edge.points[1][1]).abs() < 0.0001);
}

#[test]
fn model_draw_plan_polish_routes_objective_to_student_with_shorter_dogleg() {
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_latent_residual".to_string(),
            bbox: [0.42, 0.16, 0.58, 0.26],
            text: "Latent Residual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "comp_student_enc".to_string(),
            bbox: [0.62, 0.52, 0.88, 0.68],
            text: "Student\nEncoder".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Connector {
            id: "edge_latent_to_student".to_string(),
            points: vec![[0.5, 0.26], [0.5, 0.52], [0.75, 0.52]],
            from: Some("comp_latent_residual".to_string()),
            to: Some("comp_student_enc".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 10,
        },
    ]);

    polish_model_draw_plan_geometry(&mut draw_plan);

    let edge = connector(&draw_plan, "edge_latent_to_student");
    assert_eq!(edge.points.len(), 3, "{:?}", edge.points);
    assert_point_close(edge.points[0], [0.58, 0.21]);
    assert_point_close(edge.points[1], [0.62, 0.21]);
    assert_point_close(edge.points[2], [0.62, 0.52]);
}

#[test]
fn model_draw_plan_polish_routes_right_side_residual_feedback_to_student_edge() {
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_student".to_string(),
            bbox: [0.28, 0.62, 0.42, 0.92],
            text: "Student".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "comp_student_head".to_string(),
            bbox: [0.29, 0.38, 0.41, 0.48],
            text: "Student Latent".to_string(),
            role: "module".to_string(),
            style: "neutral_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "comp_latent_residual".to_string(),
            bbox: [0.53, 0.38, 0.65, 0.48],
            text: "Latent Residual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 22,
        },
        DrawObject::Connector {
            id: "edge_residual_to_student".to_string(),
            points: vec![[0.53, 0.43], [0.35, 0.43], [0.35, 0.62]],
            from: Some("comp_latent_residual".to_string()),
            to: Some("comp_student".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 10,
        },
    ]);

    polish_model_draw_plan_geometry(&mut draw_plan);

    let edge = connector(&draw_plan, "edge_residual_to_student");
    assert_eq!(edge.points.len(), 3, "{:?}", edge.points);
    assert_point_close(edge.points[1], [0.42, 0.43]);
    assert_point_close(edge.points[2], [0.42, 0.62]);
}

#[test]
fn model_draw_plan_polish_removes_duplicate_connectors_with_same_geometry() {
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.68, 0.72, 0.82, 0.82],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student_head".to_string(),
            bbox: [0.68, 0.52, 0.82, 0.62],
            text: "Student Head".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Connector {
            id: "e_task_loss".to_string(),
            points: vec![[0.75, 0.72], [0.75, 0.62]],
            from: Some("task_loss".to_string()),
            to: Some("student_head".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_task_back".to_string(),
            points: vec![[0.75, 0.72], [0.75, 0.62]],
            from: Some("task_loss".to_string()),
            to: Some("student_head".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 11,
        },
    ]);

    polish_model_draw_plan_geometry(&mut draw_plan);

    assert!(connector_object_exists(&draw_plan, "e_task_loss"));
    assert!(
        !connector_object_exists(&draw_plan, "e_task_back"),
        "duplicate connector with the same endpoints and geometry should be removed"
    );
}

#[test]
fn model_draw_plan_polish_folds_long_connected_inference_note_into_annotation() {
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "student_encoder".to_string(),
            bbox: [0.22, 0.52, 0.38, 0.62],
            text: "Student Encoder".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student_head".to_string(),
            bbox: [0.68, 0.52, 0.82, 0.62],
            text: "Student Head".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "task_output".to_string(),
            bbox: [0.87, 0.645, 0.97, 0.745],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "inference_note".to_string(),
            bbox: [0.85, 0.52, 0.95, 0.62],
            text: "Inference: student only".to_string(),
            role: "output".to_string(),
            style: "muted_module".to_string(),
            z: 23,
        },
        DrawObject::Connector {
            id: "e_output".to_string(),
            points: vec![[0.82, 0.57], [0.82, 0.695], [0.87, 0.695]],
            from: Some("student_head".to_string()),
            to: Some("task_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_student_encoder_inference_note".to_string(),
            points: vec![[0.38, 0.57], [0.85, 0.57]],
            from: Some("student_encoder".to_string()),
            to: Some("inference_note".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 11,
        },
    ]);

    polish_model_draw_plan_geometry(&mut draw_plan);

    assert!(!box_object_exists(&draw_plan, "inference_note"));
    assert!(!connector_object_exists(
        &draw_plan,
        "e_student_encoder_inference_note"
    ));
    assert!(
        !box_text(&draw_plan, "student_encoder")
            .to_lowercase()
            .contains("inference"),
        "long connected inference note should not be folded into the module label"
    );
    assert!(
        draw_plan.objects.iter().any(|object| matches!(
            object,
            DrawObject::Text { id, text, style, .. }
                if id == "ann_inference"
                    && style == "annotation"
                    && text.to_lowercase().contains("student only")
        )),
        "long connected inference note should be restored as a compact editable annotation"
    );
}

#[test]
fn model_draw_plan_polish_restores_missing_figure_plan_annotation_without_label_pollution() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher-student distillation with latent residual supervision.",
            "visual_focus": ["student_encoder", "student_head"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "student_encoder_region", "bbox": [0.20, 0.42, 0.38, 0.58]},
                {"id": "student_head_region", "bbox": [0.68, 0.42, 0.84, 0.58]},
                {"id": "output_region", "bbox": [0.86, 0.42, 0.96, 0.58]}
            ]
        },
        "components": [
            {"id": "student_encoder", "label": "Student Encoder", "role": "main", "visual_weight": "strong", "region": "student_encoder_region", "allowed_asset_id": null},
            {"id": "student_head", "label": "Student Head", "role": "main", "visual_weight": "strong", "region": "student_head_region", "allowed_asset_id": null},
            {"id": "task_output", "label": "ŷ", "role": "output", "visual_weight": "normal", "region": "output_region", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "e_output", "from": "student_head", "to": "task_output", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"}
        ],
        "annotations": [
            {"id": "ann_inference", "label": "Inference: student only", "target_id": "student_head", "bbox": [0.62, 0.08, 0.90, 0.14]}
        ],
        "assets": [],
        "design": {
            "style": "wps-clean",
            "max_colors": 3,
            "font_policy": "wps_friendly",
            "avoid_arrow_crossing": true,
            "prefer_native_shapes": true
        }
    }))
    .expect("figure plan should deserialize");
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "student_encoder".to_string(),
            bbox: [0.22, 0.52, 0.38, 0.62],
            text: "Student Encoder".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student_head".to_string(),
            bbox: [0.68, 0.52, 0.82, 0.62],
            text: "Student Head".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "task_output".to_string(),
            bbox: [0.87, 0.645, 0.97, 0.745],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "inference_note".to_string(),
            bbox: [0.85, 0.52, 0.95, 0.62],
            text: "Inference: student only".to_string(),
            role: "output".to_string(),
            style: "muted_module".to_string(),
            z: 23,
        },
        DrawObject::Connector {
            id: "e_output".to_string(),
            points: vec![[0.82, 0.57], [0.82, 0.695], [0.87, 0.695]],
            from: Some("student_head".to_string()),
            to: Some("task_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_student_encoder_inference_note".to_string(),
            points: vec![[0.38, 0.57], [0.85, 0.57]],
            from: Some("student_encoder".to_string()),
            to: Some("inference_note".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 11,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut draw_plan, &figure_plan);

    assert!(!box_object_exists(&draw_plan, "inference_note"));
    assert!(!connector_object_exists(
        &draw_plan,
        "e_student_encoder_inference_note"
    ));
    assert_eq!(box_text(&draw_plan, "student_encoder"), "Student Encoder");
    assert!(
        draw_plan.objects.iter().any(|object| matches!(
            object,
            DrawObject::Text { id, text, style, .. }
                if id == "ann_inference"
                    && text == "Inference: student only"
                    && style == "annotation"
        )),
        "FigurePlan annotation should be restored as editable text without polluting module labels"
    );
    let annotation = text_box(&draw_plan, "ann_inference");
    let target = object_box(&draw_plan, "student_head");
    assert!(
        target[1] - annotation[3] < 0.06,
        "restored inference annotation should be anchored near its FigurePlan target: {annotation:?} {target:?}"
    );
}

#[test]
fn model_draw_plan_polish_converts_note_text_components_to_connectable_boxes() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Student branch is used for inference.",
            "visual_focus": ["comp_student_branch", "comp_inference_note"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "pipeline",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "student_branch", "bbox": [0.10, 0.30, 0.30, 0.70]},
                {"id": "inference_note", "bbox": [0.72, 0.42, 0.90, 0.58]}
            ]
        },
        "components": [
            {"id": "comp_student_branch", "label": "Student", "role": "main", "visual_weight": "strong", "region": "student_branch", "allowed_asset_id": null},
            {"id": "comp_inference_note", "label": "Inference: Student only", "role": "output", "visual_weight": "normal", "region": "inference_note", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "edge_student_inference", "from": "comp_student_branch", "to": "comp_inference_note", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"}
        ],
        "annotations": [],
        "assets": [],
        "design": {
            "style": "wps-clean",
            "max_colors": 3,
            "font_policy": "wps_friendly",
            "avoid_arrow_crossing": true,
            "prefer_native_shapes": true
        }
    }))
    .expect("inference note fixture should deserialize");
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_student_branch".to_string(),
            bbox: [0.12, 0.34, 0.26, 0.68],
            text: "Student".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 20,
        },
        DrawObject::Text {
            id: "comp_inference_note".to_string(),
            bbox: [0.91, 0.45, 0.99, 0.53],
            text: "Inference:\nStudent only".to_string(),
            style: "neutral_module".to_string(),
            z: 27,
        },
        DrawObject::Connector {
            id: "edge_student_inference".to_string(),
            points: vec![[0.26, 0.51], [0.91, 0.51]],
            from: Some("comp_student_branch".to_string()),
            to: Some("comp_inference_note".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 16,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut draw_plan, &figure_plan);

    assert!(box_object_exists(&draw_plan, "comp_inference_note"));
    assert!(
        !text_object_exists(&draw_plan, "comp_inference_note"),
        "FigurePlan components must render as connectable component boxes, not floating text"
    );
    let note_box = object_box(&draw_plan, "comp_inference_note");
    assert!(
        note_box[2] <= 0.90,
        "note-like components should be moved out of the extreme right margin: {note_box:?}"
    );
    assert!(note_box[0] > object_box(&draw_plan, "comp_student_branch")[2]);
    let edge = connector(&draw_plan, "edge_student_inference");
    assert_point_close(
        edge.points.last().copied().expect("edge has an end point"),
        anchor_towards(note_box, object_box(&draw_plan, "comp_student_branch")),
    );
}

#[test]
fn model_draw_plan_polish_moves_top_edge_labels_into_safe_area() {
    let mut plan = minimal_draw_plan(vec![DrawObject::Connector {
        id: "edge_task_loss".to_string(),
        points: vec![[0.19, 0.13], [0.19, 0.07], [0.42, 0.07]],
        from: Some("comp_student_projection".to_string()),
        to: Some("comp_task_loss".to_string()),
        style: "main_flow".to_string(),
        label: Some(DrawLabel {
            text: "L_task".to_string(),
            bbox: [0.265, 0.0, 0.345, 0.04],
        }),
        z: 15,
    }]);

    polish_model_draw_plan_geometry(&mut plan);

    let edge = connector(&plan, "edge_task_loss");
    let label = edge.label.as_ref().expect("label should remain");
    assert!(
        label.bbox[1] >= 0.09,
        "top-margin labels should move below the nearby edge instead of hugging the canvas boundary: {:?}",
        label.bbox
    );
    assert!(!label_intersects_any_segment(label.bbox, edge.points));
}

#[test]
fn model_draw_plan_polish_separates_overlapping_semantic_boxes() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_student".to_string(),
            bbox: [0.34, 0.40, 0.54, 0.58],
            text: "Student".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "comp_output".to_string(),
            bbox: [0.0768, 0.03, 0.2832, 0.13],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "comp_task_loss".to_string(),
            bbox: [0.1568, 0.09, 0.2832, 0.19],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "neutral_module".to_string(),
            z: 22,
        },
        DrawObject::Connector {
            id: "e_student_output".to_string(),
            points: vec![[0.54, 0.49], [0.0768, 0.08]],
            from: Some("comp_student".to_string()),
            to: Some("comp_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_output_loss".to_string(),
            points: vec![[0.2832, 0.08], [0.1568, 0.14]],
            from: Some("comp_output".to_string()),
            to: Some("comp_task_loss".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 11,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let output_box = object_box(&plan, "comp_output");
    let loss_box = object_box(&plan, "comp_task_loss");
    assert_eq!(
        output_box,
        [0.0768, 0.03, 0.2832, 0.13],
        "primary output box should stay where the model placed it"
    );
    assert!(
        !component_overlap_gate_fails(output_box, loss_box),
        "model polish should separate overlapping semantic boxes without a local template: output={output_box:?}, loss={loss_box:?}"
    );
    assert_eq!(
        object_box(&plan, "comp_student"),
        [0.34, 0.40, 0.54, 0.58],
        "unrelated main modules must not be relaid out by a template"
    );
    let output_loss = connector(&plan, "e_output_loss");
    assert_eq!(
        output_loss.points.last().copied(),
        Some(anchor_towards(loss_box, output_box)),
        "connectors targeting a moved box should keep their endpoint attached to the new box"
    );
}

#[test]
fn model_draw_plan_polish_folds_standalone_inference_note_into_annotation() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "student_enc".to_string(),
            bbox: [0.42, 0.42, 0.56, 0.58],
            text: "Student\nEncoder".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "task_output".to_string(),
            bbox: [0.74, 0.42, 0.86, 0.58],
            text: "Task\nOutput".to_string(),
            role: "output".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "inference_note".to_string(),
            bbox: [0.40, 0.83, 0.56, 0.93],
            text: "Inference: Student only".to_string(),
            role: "context".to_string(),
            style: "neutral_module".to_string(),
            z: 22,
        },
        DrawObject::Connector {
            id: "e_student_output".to_string(),
            points: vec![[0.56, 0.50], [0.74, 0.50]],
            from: Some("student_enc".to_string()),
            to: Some("task_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    assert!(
        !box_object_exists(&plan, "inference_note"),
        "standalone inference note boxes should not remain as marginal explanatory notes"
    );
    assert!(
        !box_text(&plan, "student_enc")
            .to_lowercase()
            .contains("inference"),
        "inference-only semantics should not pollute the student module label"
    );
    assert!(
        draw_plan_has_text(&plan, "ann_inference", "student only"),
        "standalone inference note should become a compact editable annotation"
    );
}

#[test]
fn model_draw_plan_polish_removes_annotation_text_that_overlaps_connector_strokes() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "student_proj".to_string(),
            bbox: [0.42, 0.06, 0.56, 0.16],
            text: "Student\nProjection".to_string(),
            role: "module".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "task_output".to_string(),
            bbox: [0.74, 0.42, 0.86, 0.58],
            text: "Task\nOutput".to_string(),
            role: "output".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Connector {
            id: "e_student_proj_output".to_string(),
            points: vec![[0.56, 0.11], [0.65, 0.11], [0.65, 0.50], [0.74, 0.50]],
            from: Some("student_proj".to_string()),
            to: Some("task_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Text {
            id: "a_student_label".to_string(),
            bbox: [0.57, 0.44, 0.73, 0.50],
            text: "Student (trainable)".to_string(),
            style: "annotation".to_string(),
            z: 40,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    assert!(
        !text_object_exists(&plan, "a_student_label"),
        "floating annotation text that overlaps connector strokes should be removed"
    );
}

#[test]
fn model_draw_plan_polish_preserves_inference_text_annotation_outside_student_label() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_student_head".to_string(),
            bbox: [0.64, 0.54, 0.76, 0.65],
            text: "Student Head".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 20,
        },
        DrawObject::Text {
            id: "ann_inference_note".to_string(),
            bbox: [0.64, 0.47, 0.82, 0.525],
            text: "Inference only".to_string(),
            style: "annotation".to_string(),
            z: 40,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    assert!(
        text_object_exists(&plan, "ann_inference_note"),
        "inference-only text annotations are already editable external annotations"
    );
    assert!(
        !box_text(&plan, "comp_student_head")
            .to_lowercase()
            .contains("inference"),
        "inference-only text should not be folded into a student module"
    );
}

#[test]
fn model_draw_plan_polish_removes_asymmetric_student_branch_annotation() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_teacher_encoder".to_string(),
            bbox: [0.24, 0.76, 0.36, 0.87],
            text: "Teacher Encoder".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "comp_student_encoder".to_string(),
            bbox: [0.24, 0.54, 0.36, 0.65],
            text: "Student Encoder".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Text {
            id: "ann_student_branch".to_string(),
            bbox: [0.24, 0.47, 0.38, 0.525],
            text: "Student (trainable)".to_string(),
            style: "annotation".to_string(),
            z: 40,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    assert!(
        !text_object_exists(&plan, "ann_student_branch"),
        "single-sided branch annotations should be removed unless teacher/student are labeled symmetrically"
    );
}

#[test]
fn model_draw_plan_polish_removes_overlapping_branch_path_and_template_annotations() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_teacher_enc".to_string(),
            bbox: [0.05, 0.54, 0.25, 0.64],
            text: "Teacher Encoder".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "comp_student_enc".to_string(),
            bbox: [0.42, 0.54, 0.62, 0.64],
            text: "Student Encoder".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Text {
            id: "anno_template_ref".to_string(),
            bbox: [0.02, 0.02, 0.35, 0.10],
            text: "simclr_contrastive_y_branch adapted: shared source to dual branch".to_string(),
            style: "annotation".to_string(),
            z: 40,
        },
        DrawObject::Text {
            id: "anno_teacher_path".to_string(),
            bbox: [0.08, 0.56, 0.28, 0.64],
            text: "Teacher (training only)".to_string(),
            style: "annotation".to_string(),
            z: 41,
        },
        DrawObject::Text {
            id: "anno_student_path".to_string(),
            bbox: [0.40, 0.56, 0.60, 0.64],
            text: "Student (train + infer)".to_string(),
            style: "annotation".to_string(),
            z: 42,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    assert!(!text_object_exists(&plan, "anno_template_ref"));
    assert!(!text_object_exists(&plan, "anno_teacher_path"));
    assert!(!text_object_exists(&plan, "anno_student_path"));
}

#[test]
fn model_draw_plan_polish_removes_line_style_legend_annotations() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_residual".to_string(),
            bbox: [0.42, 0.35, 0.52, 0.45],
            text: "Residual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 20,
        },
        DrawObject::Text {
            id: "ann_residual_dash".to_string(),
            bbox: [0.50, 0.35, 0.54, 0.41],
            text: "dashed = residual supervision".to_string(),
            style: "annotation".to_string(),
            z: 40,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    assert!(!text_object_exists(&plan, "ann_residual_dash"));
}

#[test]
fn model_draw_plan_polish_syncs_connector_labels_and_snaps_them_to_routes() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Student output is compared with labels for task loss.",
            "visual_focus": ["student_out", "task_loss"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "student_out_region", "bbox": [0.08, 0.14, 0.24, 0.28]},
                {"id": "task_loss_region", "bbox": [0.08, 0.02, 0.24, 0.12]}
            ]
        },
        "components": [
            {"id": "student_out", "label": "ŷ", "role": "output", "visual_weight": "normal", "region": "student_out_region", "allowed_asset_id": null},
            {"id": "task_loss", "label": "Task Loss", "role": "loss", "visual_weight": "normal", "region": "task_loss_region", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "edge_student_out_to_task", "from": "student_out", "to": "task_loss", "label": "ŷ vs y", "semantic": "supervision", "style": "solid", "importance": "normal"}
        ],
        "annotations": [],
        "assets": [],
        "design": {
            "style": "wps-clean",
            "max_colors": 3,
            "font_policy": "wps_friendly",
            "avoid_arrow_crossing": true,
            "prefer_native_shapes": true
        }
    }))
    .expect("figure plan should deserialize");
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "student_out".to_string(),
            bbox: [0.08, 0.16, 0.24, 0.26],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.08, 0.00, 0.24, 0.10],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 21,
        },
        DrawObject::Connector {
            id: "edge_student_out_to_task".to_string(),
            points: vec![[0.16, 0.16], [0.16, 0.04]],
            from: Some("student_out".to_string()),
            to: Some("task_loss".to_string()),
            style: "main_flow".to_string(),
            label: Some(DrawLabel {
                text: "wrong".to_string(),
                bbox: [0.18, 0.50, 0.26, 0.54],
            }),
            z: 10,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut draw_plan, &figure_plan);

    let edge = connector(&draw_plan, "edge_student_out_to_task");
    let label = edge.label.as_ref().expect("FigurePlan label should exist");
    assert_eq!(label.text, "ŷ vs y");
    assert!(
        label.bbox[3] < 0.34,
        "label should be snapped near the final vertical edge, not left floating far away: {:?}",
        label.bbox
    );
}

#[test]
fn model_draw_plan_polish_resnaps_vertical_connector_label_to_final_route() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "student_head".to_string(),
            bbox: [0.3224, 0.3624, 0.4376, 0.5176],
            text: "Student Head".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.3224, 0.0824, 0.4376, 0.1976],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 21,
        },
        DrawObject::Connector {
            id: "e_student_task".to_string(),
            points: vec![[0.38, 0.3624], [0.38, 0.1976]],
            from: Some("student_head".to_string()),
            to: Some("task_loss".to_string()),
            style: "normal_flow".to_string(),
            label: Some(DrawLabel {
                text: "ŷ vs y".to_string(),
                bbox: [0.30, 0.6076, 0.46, 0.6576],
            }),
            z: 10,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let edge = connector(&plan, "e_student_task");
    let label = edge.label.as_ref().expect("label should remain");
    assert!(
        !label_intersects_any_segment(label.bbox, edge.points),
        "vertical connector label should sit beside the line, not on top of it: {:?} vs {:?}",
        label.bbox,
        edge.points
    );
    assert!(
        (center_y(label.bbox) - 0.28).abs() < 0.12,
        "stale label should be snapped back near the final vertical route: {:?}",
        label.bbox
    );
    assert_eq!(
        intersection_area(label.bbox, object_box(&plan, "student_head")),
        0.0
    );
    assert_eq!(
        intersection_area(label.bbox, object_box(&plan, "task_loss")),
        0.0
    );
}

#[test]
fn model_draw_plan_polish_resnaps_horizontal_connector_label_to_final_route() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "latent_residual".to_string(),
            bbox: [0.5424, 0.2024, 0.6576, 0.3576],
            text: "Latent Residual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher_head".to_string(),
            bbox: [0.768, 0.248, 0.912, 0.392],
            text: "Teacher Head".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 21,
        },
        DrawObject::Connector {
            id: "e_residual".to_string(),
            points: vec![[0.768, 0.28], [0.6576, 0.28]],
            from: Some("teacher_head".to_string()),
            to: Some("latent_residual".to_string()),
            style: "normal_flow".to_string(),
            label: Some(DrawLabel {
                text: "r = h_T - h_S".to_string(),
                bbox: [0.64, 0.395, 0.80, 0.445],
            }),
            z: 10,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let edge = connector(&plan, "e_residual");
    let label = edge.label.as_ref().expect("label should remain");
    assert!(
        !label_intersects_any_segment(label.bbox, edge.points),
        "horizontal connector label should not overlap the stroke: {:?} vs {:?}",
        label.bbox,
        edge.points
    );
    assert!(
        (center_y(label.bbox) - 0.28).abs() < 0.12,
        "horizontal connector label should stay close to the final route: label={:?}, points={:?}",
        label.bbox,
        edge.points
    );
}

#[test]
fn model_draw_plan_polish_resnaps_elbow_connector_label_to_final_segment() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "latent_residual".to_string(),
            bbox: [0.5424, 0.2024, 0.6576, 0.3576],
            text: "Latent Residual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student".to_string(),
            bbox: [0.3224, 0.6424, 0.4376, 0.8976],
            text: "Student".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Connector {
            id: "e_residual_student".to_string(),
            points: vec![[0.60, 0.28], [0.60, 0.44], [0.38, 0.44]],
            from: Some("latent_residual".to_string()),
            to: Some("student".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "residual supervision".to_string(),
                bbox: [0.41, 0.53, 0.57, 0.58],
            }),
            z: 10,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let edge = connector(&plan, "e_residual_student");
    let label = edge.label.as_ref().expect("label should remain");
    assert!(
        !label_intersects_any_segment(label.bbox, edge.points),
        "elbow connector label should sit beside a final segment: {:?} vs {:?}",
        label.bbox,
        edge.points
    );
    assert!(
        (center_y(label.bbox) - 0.44).abs() < 0.12,
        "elbow connector label should snap near its final horizontal segment: {:?}",
        label.bbox
    );
    assert_eq!(
        intersection_area(label.bbox, object_box(&plan, "latent_residual")),
        0.0
    );
    assert_eq!(
        intersection_area(label.bbox, object_box(&plan, "student")),
        0.0
    );
}

#[test]
fn model_draw_plan_polish_moves_shared_input_between_teacher_and_student_branches() {
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_input".to_string(),
            bbox: [0.08, 0.734, 0.18, 0.866],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "comp_teacher_enc".to_string(),
            bbox: [0.24, 0.14, 0.36, 0.38],
            text: "Teacher Encoder".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "comp_student_enc".to_string(),
            bbox: [0.64, 0.30, 0.76, 0.54],
            text: "Student Encoder".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Connector {
            id: "edge_input_teacher".to_string(),
            points: vec![[0.13, 0.734], [0.30, 0.38]],
            from: Some("comp_input".to_string()),
            to: Some("comp_teacher_enc".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "edge_input_student".to_string(),
            points: vec![[0.13, 0.734], [0.70, 0.54]],
            from: Some("comp_input".to_string()),
            to: Some("comp_student_enc".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 11,
        },
    ]);

    polish_model_draw_plan_geometry(&mut draw_plan);

    let input = object_box(&draw_plan, "comp_input");
    assert!(
        center_y(input) < 0.48,
        "shared input should sit between branch targets instead of below the whole diagram: {input:?}"
    );
    let edge = connector(&draw_plan, "edge_input_student");
    assert!(
        edge.points.iter().all(|point| point[1] < 0.58),
        "input-to-student route should not run as a long low horizontal bus: {:?}",
        edge.points
    );
}

#[test]
fn model_draw_plan_polish_moves_task_loss_below_output_for_short_vertical_edge() {
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_output".to_string(),
            bbox: [0.82, 0.61, 0.92, 0.71],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "comp_task_loss".to_string(),
            bbox: [0.634, 0.76, 0.766, 0.86],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 21,
        },
        DrawObject::Connector {
            id: "edge_task_loss".to_string(),
            points: vec![[0.87, 0.66], [0.87, 0.81], [0.70, 0.81]],
            from: Some("comp_output".to_string()),
            to: Some("comp_task_loss".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
    ]);

    polish_model_draw_plan_geometry(&mut draw_plan);

    let output = object_box(&draw_plan, "comp_output");
    let task_loss = object_box(&draw_plan, "comp_task_loss");
    assert!(
        (center_x(output) - center_x(task_loss)).abs() < 0.001,
        "task loss should align under output for a short readable loss edge: output={output:?}, task_loss={task_loss:?}"
    );
    let edge = connector(&draw_plan, "edge_task_loss");
    assert_eq!(edge.points.len(), 2, "{:?}", edge.points);
    assert!((edge.points[0][0] - edge.points[1][0]).abs() < 0.0001);
}

#[test]
fn model_draw_plan_polish_anchors_inference_annotation_to_figure_plan_target() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Student branch is used at inference time.",
            "visual_focus": ["comp_student_enc"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "student_region", "bbox": [0.64, 0.30, 0.76, 0.54]}
            ]
        },
        "components": [
            {"id": "comp_student_enc", "label": "Student Encoder", "role": "main", "visual_weight": "strong", "region": "student_region", "allowed_asset_id": null}
        ],
        "edges": [],
        "annotations": [
            {"id": "anno_inference", "label": "Inference: student only", "target_id": "comp_student_enc", "bbox": [0.62, 0.16, 0.78, 0.24]}
        ],
        "assets": [],
        "design": {
            "style": "wps-clean",
            "max_colors": 3,
            "font_policy": "wps_friendly",
            "avoid_arrow_crossing": true,
            "prefer_native_shapes": true
        }
    }))
    .expect("figure plan should deserialize");
    let mut draw_plan = minimal_draw_plan(vec![DrawObject::Box {
        id: "comp_student_enc".to_string(),
        bbox: [0.6424, 0.3024, 0.7576, 0.5376],
        text: "Student Encoder".to_string(),
        role: "main".to_string(),
        style: "primary_module".to_string(),
        z: 20,
    }]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut draw_plan, &figure_plan);

    let annotation = text_box(&draw_plan, "anno_inference");
    let target = object_box(&draw_plan, "comp_student_enc");
    assert!(
        annotation[3] <= target[1] - 0.005 && target[1] - annotation[3] < 0.04,
        "inference annotation should be tightly anchored above its target: annotation={annotation:?}, target={target:?}"
    );
}

#[test]
fn model_draw_plan_polish_moves_connector_labels_off_other_edges_and_boxes() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_student_latent".to_string(),
            bbox: [0.44, 0.54, 0.56, 0.65],
            text: "h_S".to_string(),
            role: "context".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "comp_residual_loss".to_string(),
            bbox: [0.44, 0.34, 0.56, 0.45],
            text: "L_resid".to_string(),
            role: "loss".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Connector {
            id: "edge_student_encode".to_string(),
            points: vec![[0.36, 0.595], [0.44, 0.595]],
            from: Some("comp_student_encoder".to_string()),
            to: Some("comp_student_latent".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "edge_latent_residual".to_string(),
            points: vec![[0.50, 0.76], [0.50, 0.45]],
            from: Some("comp_teacher_latent".to_string()),
            to: Some("comp_residual_loss".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "h_T - h_S".to_string(),
                bbox: [0.33, 0.60, 0.48, 0.64],
            }),
            z: 11,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let residual = connector(&plan, "edge_latent_residual");
    let label = residual.label.as_ref().expect("label remains");
    let student_encode = connector(&plan, "edge_student_encode");
    assert!(
        !label_intersects_any_segment(label.bbox, student_encode.points),
        "connector labels should be moved off other connector strokes"
    );
    assert_eq!(
        intersection_area(label.bbox, object_box(&plan, "comp_student_latent")),
        0.0,
        "connector labels should not overlap semantic boxes"
    );
}

#[test]
fn model_draw_plan_polish_simplifies_four_point_dogleg_connectors() {
    let mut plan = minimal_draw_plan(vec![DrawObject::Connector {
        id: "e_input_teacher".to_string(),
        points: vec![[0.11, 0.82], [0.19, 0.82], [0.19, 0.10], [0.28, 0.10]],
        from: Some("input".to_string()),
        to: Some("teacher".to_string()),
        style: "normal_flow".to_string(),
        label: None,
        z: 10,
    }]);

    polish_model_draw_plan_geometry(&mut plan);

    let edge = connector(&plan, "e_input_teacher");
    assert_eq!(
        edge.points.len(),
        3,
        "dogleg connector should collapse to a two-segment orthogonal route: {:?}",
        edge.points
    );
    assert_eq!(edge.points[0], [0.11, 0.82]);
    assert_eq!(edge.points[2], [0.28, 0.10]);
}

#[test]
fn model_draw_plan_polish_expands_degenerate_short_connectors() {
    let mut plan = minimal_draw_plan(vec![DrawObject::Connector {
        id: "e_teacher_latent".to_string(),
        points: vec![[0.36, 0.14], [0.36, 0.18]],
        from: Some("teacher".to_string()),
        to: Some("latent".to_string()),
        style: "normal_flow".to_string(),
        label: None,
        z: 10,
    }]);

    polish_model_draw_plan_geometry(&mut plan);

    let edge = connector(&plan, "e_teacher_latent");
    assert!(
        polyline_length(edge.points) >= 0.08,
        "short connector should be expanded enough to avoid local degenerate-edge gate: {:?}",
        edge.points
    );
    assert!(
        edge.points.windows(2).all(|window| {
            (window[0][0] - window[1][0]).abs() < 0.0001
                || (window[0][1] - window[1][1]).abs() < 0.0001
        }),
        "expanded short connector should stay orthogonal: {:?}",
        edge.points
    );
}

#[test]
fn draw_plan_validation_rejects_duplicate_object_ids() {
    let plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "dup".to_string(),
            bbox: [0.1, 0.1, 0.2, 0.2],
            text: "A".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 1,
        },
        DrawObject::Text {
            id: "dup".to_string(),
            bbox: [0.3, 0.1, 0.4, 0.2],
            text: "B".to_string(),
            style: "caption".to_string(),
            z: 2,
        },
    ]);

    let error = validate_draw_plan(&plan).expect_err("duplicate ids should fail");

    assert!(error.to_string().contains("duplicate draw object id"));
}

#[test]
fn draw_plan_validation_rejects_full_slide_raster_images() {
    let plan = minimal_draw_plan(vec![DrawObject::Image {
        id: "full_raster".to_string(),
        bbox: [0.0, 0.0, 1.0, 1.0],
        asset_id: "rendered_full_figure".to_string(),
        z: 1,
    }]);

    let error = validate_draw_plan(&plan).expect_err("full slide image should fail");

    assert!(error.to_string().contains("full-slide raster"));
}

#[test]
fn draw_plan_geometry_repair_removes_marginal_notes_and_routes_labels_off_edges() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input".to_string(),
            bbox: [0.05, 0.42, 0.17, 0.58],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher".to_string(),
            bbox: [0.28, 0.15, 0.52, 0.38],
            text: "Teacher".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "latent".to_string(),
            bbox: [0.62, 0.42, 0.74, 0.58],
            text: "Latent".to_string(),
            role: "module".to_string(),
            style: "accent_module".to_string(),
            z: 22,
        },
        DrawObject::Connector {
            id: "e_input_teacher".to_string(),
            points: vec![[0.17, 0.465], [0.28, 0.265]],
            from: Some("input".to_string()),
            to: Some("teacher".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_teacher_latent".to_string(),
            points: vec![[0.52, 0.265], [0.57, 0.265], [0.57, 0.465], [0.62, 0.465]],
            from: Some("teacher".to_string()),
            to: Some("latent".to_string()),
            style: "normal_flow".to_string(),
            label: Some(DrawLabel {
                text: "h_T".to_string(),
                bbox: [0.54, 0.245, 0.62, 0.295],
            }),
            z: 11,
        },
        DrawObject::Text {
            id: "anno_inference".to_string(),
            bbox: [0.33, 0.88, 0.47, 0.94],
            text: "Inference: Student only".to_string(),
            style: "annotation".to_string(),
            z: 40,
        },
        DrawObject::Text {
            id: "ann_training".to_string(),
            bbox: [0.35, 0.87, 0.55, 0.94],
            text: "Training".to_string(),
            style: "annotation".to_string(),
            z: 41,
        },
        DrawObject::Text {
            id: "a_training".to_string(),
            bbox: [0.35, 0.02, 0.45, 0.08],
            text: "Training".to_string(),
            style: "annotation".to_string(),
            z: 42,
        },
    ]);

    repair_draw_plan_geometry(&mut plan);

    assert!(
        !plan
            .objects
            .iter()
            .any(|object| matches!(object, DrawObject::Text { id, .. } if id == "anno_inference")),
        "marginal explanatory notes should be removed before rendering"
    );
    assert!(
        !plan
            .objects
            .iter()
            .any(|object| matches!(object, DrawObject::Text { id, .. } if id == "a_training")),
        "edge-of-canvas phase labels should be removed before rendering"
    );
    assert!(
        !plan
            .objects
            .iter()
            .any(|object| matches!(object, DrawObject::Text { id, .. } if id == "ann_training")),
        "ann_* edge-of-canvas phase labels from real optimizer output should be removed"
    );

    let input_edge = connector(&plan, "e_input_teacher");
    assert!(input_edge.points.len() >= 3, "{:?}", input_edge.points);
    assert!(
        input_edge.points.windows(2).all(|window| {
            (window[0][0] - window[1][0]).abs() < 0.0001
                || (window[0][1] - window[1][1]).abs() < 0.0001
        }),
        "simple flow should be repaired to orthogonal segments: {:?}",
        input_edge.points
    );

    let labeled_edge = connector(&plan, "e_teacher_latent");
    let label = labeled_edge.label.as_ref().expect("label remains");
    assert!(
        !label_intersects_any_segment(label.bbox, &labeled_edge.points),
        "label should be moved off the connector stroke: {:?} vs {:?}",
        label.bbox,
        labeled_edge.points
    );
}

#[test]
fn draw_plan_geometry_repair_preserves_teacher_student_lanes() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input".to_string(),
            bbox: [0.06, 0.42, 0.18, 0.58],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student".to_string(),
            bbox: [0.28, 0.35, 0.48, 0.65],
            text: "Student".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "teacher".to_string(),
            bbox: [0.70, 0.35, 0.90, 0.65],
            text: "Teacher".to_string(),
            role: "context".to_string(),
            style: "neutral_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "latent_residual".to_string(),
            bbox: [0.55, 0.08, 0.75, 0.18],
            text: "Latent Residual".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "output".to_string(),
            bbox: [0.30, 0.72, 0.46, 0.82],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.55, 0.72, 0.75, 0.82],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "neutral_module".to_string(),
            z: 25,
        },
        DrawObject::Box {
            id: "inference_student".to_string(),
            bbox: [0.85, 0.62, 0.95, 0.85],
            text: "Student".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 26,
        },
        DrawObject::Connector {
            id: "e_input_teacher".to_string(),
            points: vec![
                [0.18, 0.50],
                [0.18, 0.28],
                [0.70, 0.28],
                [0.75, 0.28],
                [0.75, 0.35],
                [0.80, 0.35],
            ],
            from: Some("input".to_string()),
            to: Some("teacher".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_teacher_residual".to_string(),
            points: vec![[0.80, 0.35], [0.80, 0.18]],
            from: Some("teacher".to_string()),
            to: Some("latent_residual".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_residual_student".to_string(),
            points: vec![[0.65, 0.18], [0.65, 0.35], [0.48, 0.35], [0.48, 0.50]],
            from: Some("latent_residual".to_string()),
            to: Some("student".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "e_student_taskloss".to_string(),
            points: vec![
                [0.59375, 0.25],
                [0.609375, 0.25],
                [0.609375, 0.8333333333333333],
                [0.625, 0.8333333333333333],
            ],
            from: Some("student".to_string()),
            to: Some("task_loss".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Connector {
            id: "e_task_student".to_string(),
            points: vec![[0.625, 0.1666], [0.396, 0.1666], [0.396, 0.62]],
            from: Some("task_loss".to_string()),
            to: Some("student".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 14,
        },
        DrawObject::Connector {
            id: "e_inference_input".to_string(),
            points: vec![[0.18, 0.50], [0.5, 0.50], [0.5, 0.735], [0.85, 0.735]],
            from: Some("input".to_string()),
            to: Some("inference_student".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 15,
        },
        DrawObject::Connector {
            id: "e_inference_output".to_string(),
            points: vec![[0.95, 0.735], [0.98, 0.735], [0.98, 0.50], [0.82, 0.50]],
            from: Some("inference_student".to_string()),
            to: Some("output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 16,
        },
        DrawObject::Text {
            id: "ann_inference".to_string(),
            bbox: [0.85, 0.87, 1.0, 0.945],
            text: "Inference Only".to_string(),
            style: "annotation".to_string(),
            z: 40,
        },
    ]);

    repair_draw_plan_geometry(&mut plan);

    let teacher = object_box(&plan, "teacher");
    let student = object_box(&plan, "student");
    let input = object_box(&plan, "input");
    let latent = object_box(&plan, "latent_residual");
    let output = object_box(&plan, "output");
    let loss = object_box(&plan, "task_loss");
    assert!(
        teacher[3] < student[1],
        "teacher should stay above the training student: {teacher:?} {student:?}"
    );
    assert!(
        latent[0] > student[2],
        "latent residual should sit to the right of the training lane"
    );
    assert!(
        output[0] > latent[2],
        "output should be right of latent residual"
    );
    assert!(
        loss[0] >= latent[0],
        "loss should stay near the residual/output side"
    );
    assert!(
        loss[0] > output[2],
        "task loss should use the right side of the paper-wide canvas"
    );
    assert!(
        loss[2] - input[0] > 0.85,
        "main semantic objects should use the paper-wide canvas instead of a narrow vertical stack"
    );

    let input_teacher = connector(&plan, "e_input_teacher");
    assert!(
        input_teacher.points.len() <= 4,
        "{:?}",
        input_teacher.points
    );
    assert!(input_teacher
        .points
        .windows(2)
        .all(|window| (window[0][0] - window[1][0]).abs() < 0.0001
            || (window[0][1] - window[1][1]).abs() < 0.0001));
    assert_eq!(input_teacher_style(&plan, "e_input_teacher"), "normal_flow");
    assert_eq!(
        input_teacher_style(&plan, "e_teacher_residual"),
        "dashed_supervision"
    );
    assert_eq!(
        input_teacher_style(&plan, "e_residual_student"),
        "dashed_supervision"
    );
    let residual_edge = connector(&plan, "e_residual_student");
    assert!(
        residual_edge.points.len() <= 3,
        "latent residual supervision should be a simple L-shape: {:?}",
        residual_edge.points
    );
    assert!(
        residual_edge.points[0][1] <= residual_edge.points.last().copied().unwrap()[1],
        "latent residual supervision should flow downward toward the training student, not backtrack: {:?}",
        residual_edge.points
    );
    assert!(
        !plan.objects.iter().any(|object| matches!(
            object,
            DrawObject::Connector { from, to, .. }
                if from.as_deref() == Some("student") && to.as_deref() == Some("task_loss")
        )),
        "task loss should consume the prediction node instead of a long direct student edge"
    );
    assert_eq!(
        connector_endpoints(&plan, "e_output_task_loss"),
        (Some("output"), Some("task_loss"))
    );
    let loss_edge = connector(&plan, "e_output_task_loss");
    assert_eq!(loss_edge.points[0], [output[2], center_y(output)]);
    assert_eq!(
        loss_edge.points.last().copied().unwrap(),
        [loss[0], center_y(loss)]
    );
    assert!(
        !plan.objects.iter().any(|object| matches!(
            object,
            DrawObject::Connector { from, to, .. }
                if from.as_deref() == Some("task_loss") && to.as_deref() == Some("student")
        )),
        "task-loss feedback should not add a long return edge when output-to-loss already carries the training signal"
    );
    assert!(
        box_text(&plan, "student").contains("training"),
        "training student should be labeled as the training phase"
    );
    assert!(
        !box_text(&plan, "student").contains("train / inference"),
        "training and inference should not be merged into one student label"
    );
    let inference_student = object_box(&plan, "inference_student");
    assert!(
        inference_student[0] > student[2] + 0.18
            && (center_y(inference_student) - center_y(student)).abs() < 0.08,
        "inference-only student should be a right-side continuation of the trained student: {inference_student:?} {student:?}"
    );
    assert!(box_text(&plan, "inference_student").contains("inference"));
    assert!(
        !plan.objects.iter().any(|object| matches!(
            object,
            DrawObject::Connector { from, to, .. }
                if from.as_deref() == Some("input") && to.as_deref() == Some("inference_student")
        )),
        "inference lane should not add a long input-to-inference detour"
    );
    assert!(
        plan.objects.iter().any(|object| matches!(
            object,
            DrawObject::Connector { from, to, .. }
                if from.as_deref() == Some("student") && to.as_deref() == Some("inference_student")
        )),
        "inference lane should be a short continuation from the trained student"
    );
    let inference_edge = connector(&plan, "e_student_inference");
    assert!(
        inference_edge.points.len() <= 3
            && inference_edge.points.iter().all(|point| point[1] <= student[1] + 0.02),
        "student-to-inference continuation should route above the training output row instead of through it: {:?}",
        inference_edge.points
    );
    assert!(
        !plan.objects.iter().any(|object| matches!(
            object,
            DrawObject::Text { id, .. }
                if id == "phase_training_label" || id == "phase_inference_label"
        )),
        "phase captions should be integrated into module text instead of floating near borders"
    );
    assert!(
        !plan
            .objects
            .iter()
            .any(|object| matches!(object, DrawObject::Text { id, .. } if id == "ann_inference")),
        "ann_* inference notes at the safe-area edge should be removed in teacher-student layouts"
    );
}

#[test]
fn draw_plan_geometry_repair_simplifies_overexpanded_teacher_student_optimizer_output() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "x_input".to_string(),
            bbox: [0.08, 0.42, 0.20, 0.58],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student".to_string(),
            bbox: [0.28, 0.62, 0.52, 0.85],
            text: "Student".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "y_hat".to_string(),
            bbox: [0.82, 0.42, 0.90, 0.58],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "task_loss_fn".to_string(),
            bbox: [0.62, 0.72, 0.76, 0.84],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "neutral_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "latent_loss".to_string(),
            bbox: [0.78, 0.72, 0.92, 0.84],
            text: "Latent Loss".to_string(),
            role: "loss".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "teacher".to_string(),
            bbox: [0.28, 0.15, 0.52, 0.38],
            text: "Teacher (frozen)".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 25,
        },
        DrawObject::Box {
            id: "h_teacher".to_string(),
            bbox: [0.56, 0.42, 0.66, 0.58],
            text: "h_t".to_string(),
            role: "module".to_string(),
            style: "muted_module".to_string(),
            z: 25,
        },
        DrawObject::Box {
            id: "h_student".to_string(),
            bbox: [0.38, 0.42, 0.48, 0.58],
            text: "h_s".to_string(),
            role: "module".to_string(),
            style: "muted_module".to_string(),
            z: 26,
        },
        DrawObject::Box {
            id: "residual".to_string(),
            bbox: [0.62, 0.42, 0.74, 0.58],
            text: "Residual".to_string(),
            role: "module".to_string(),
            style: "accent_module".to_string(),
            z: 27,
        },
        DrawObject::Box {
            id: "student_inf".to_string(),
            bbox: [0.68, 0.22, 0.88, 0.34],
            text: "Student".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 28,
        },
        DrawObject::Box {
            id: "y_out".to_string(),
            bbox: [0.86, 0.22, 0.96, 0.34],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 29,
        },
        DrawObject::Text {
            id: "phase_train".to_string(),
            bbox: [0.20, 0.88, 0.60, 0.95],
            text: "Training".to_string(),
            style: "phase_label".to_string(),
            z: 5,
        },
        DrawObject::Text {
            id: "phase_infer".to_string(),
            bbox: [0.68, 0.88, 0.96, 0.95],
            text: "Inference".to_string(),
            style: "phase_label".to_string(),
            z: 5,
        },
        DrawObject::Text {
            id: "only_student_annotation".to_string(),
            bbox: [0.80, 0.39, 0.95, 0.42],
            text: "Only Student".to_string(),
            style: "annotation".to_string(),
            z: 6,
        },
        DrawObject::Text {
            id: "inference_badge".to_string(),
            bbox: [0.78, 0.505, 0.90, 0.545],
            text: "Inference".to_string(),
            style: "badge".to_string(),
            z: 6,
        },
        DrawObject::Connector {
            id: "e_x_to_student".to_string(),
            points: vec![[0.17, 0.535], [0.28, 0.735]],
            from: Some("x_input".to_string()),
            to: Some("student".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_x_to_teacher".to_string(),
            points: vec![[0.17, 0.465], [0.28, 0.265]],
            from: Some("x_input".to_string()),
            to: Some("teacher".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_teacher_to_ht".to_string(),
            points: vec![[0.45, 0.265], [0.61, 0.42]],
            from: Some("teacher".to_string()),
            to: Some("h_teacher".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "e_ht_to_residual".to_string(),
            points: vec![[0.56, 0.5], [0.52, 0.5], [0.52, 0.28], [0.58, 0.28]],
            from: Some("h_teacher".to_string()),
            to: Some("residual".to_string()),
            style: "main_flow".to_string(),
            label: Some(DrawLabel {
                text: "h_t - h_s".to_string(),
                bbox: [0.51, 0.22, 0.59, 0.26],
            }),
            z: 13,
        },
        DrawObject::Connector {
            id: "e_student_to_hs".to_string(),
            points: vec![[0.38, 0.735], [0.43, 0.58]],
            from: Some("student".to_string()),
            to: Some("h_student".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 14,
        },
        DrawObject::Connector {
            id: "e_hs_to_residual".to_string(),
            points: vec![[0.48, 0.5], [0.5, 0.5], [0.5, 0.34]],
            from: Some("h_student".to_string()),
            to: Some("residual".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 15,
        },
        DrawObject::Connector {
            id: "e_residual_to_student".to_string(),
            points: vec![[0.62, 0.5], [0.52, 0.735]],
            from: Some("residual".to_string()),
            to: Some("student".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "supervise".to_string(),
                bbox: [0.42, 0.38, 0.50, 0.42],
            }),
            z: 16,
        },
        DrawObject::Connector {
            id: "e_student_to_output".to_string(),
            points: vec![[0.52, 0.735], [0.57, 0.735], [0.57, 0.5], [0.82, 0.5]],
            from: Some("student".to_string()),
            to: Some("y_hat".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 17,
        },
        DrawObject::Connector {
            id: "e_student_to_task_loss".to_string(),
            points: vec![[0.52, 0.735], [0.57, 0.735], [0.57, 0.78], [0.62, 0.78]],
            from: Some("student".to_string()),
            to: Some("task_loss_fn".to_string()),
            style: "normal_flow".to_string(),
            label: Some(DrawLabel {
                text: "ŷ".to_string(),
                bbox: [0.405, 0.785, 0.435, 0.825],
            }),
            z: 18,
        },
        DrawObject::Connector {
            id: "e_x_to_student_inf".to_string(),
            points: vec![[0.14, 0.535], [0.535, 0.535], [0.535, 0.28], [0.68, 0.28]],
            from: Some("x_input".to_string()),
            to: Some("student_inf".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 19,
        },
        DrawObject::Connector {
            id: "e_student_inf_to_yout".to_string(),
            points: vec![[0.88, 0.28], [0.90, 0.28]],
            from: Some("student_inf".to_string()),
            to: Some("y_out".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 20,
        },
        DrawObject::Connector {
            id: "e_ht_hs_to_residual".to_string(),
            points: vec![[0.84375, 0.784], [0.84375, 0.95]],
            from: Some("h_teacher".to_string()),
            to: Some("residual".to_string()),
            style: "main_flow".to_string(),
            label: Some(DrawLabel {
                text: "h_t - h_s".to_string(),
                bbox: [0.763, 0.714, 0.923, 0.764],
            }),
            z: 21,
        },
        DrawObject::Connector {
            id: "e_residual_to_latent_loss".to_string(),
            points: vec![[0.68, 0.50], [0.85, 0.50], [0.85, 0.78]],
            from: Some("residual".to_string()),
            to: Some("latent_loss".to_string()),
            style: "main_flow".to_string(),
            label: Some(DrawLabel {
                text: "L_latent".to_string(),
                bbox: [0.715, 0.44, 0.815, 0.48],
            }),
            z: 22,
        },
    ]);

    repair_draw_plan_geometry(&mut plan);

    for removed_id in [
        "h_teacher",
        "h_student",
        "student_inf",
        "y_out",
        "phase_train",
        "phase_infer",
        "only_student_annotation",
        "e_teacher_to_ht",
        "e_ht_to_residual",
        "e_student_to_hs",
        "e_hs_to_residual",
        "e_student_inf_to_yout",
        "e_ht_hs_to_residual",
        "e_loss_student",
    ] {
        assert!(
            !plan
                .objects
                .iter()
                .any(|object| draw_object_id(object) == removed_id),
            "{removed_id} should be removed by semantic simplification"
        );
    }
    assert!(plan.objects.iter().any(
        |object| matches!(object, DrawObject::Connector { from, to, style, .. }
            if from.as_deref() == Some("teacher")
                && to.as_deref() == Some("residual")
                && style == "dashed_supervision")
    ));
    let output = object_box(&plan, "y_hat");
    let student = object_box(&plan, "student");
    assert!(
        (center_y(output) - center_y(student)).abs() < 0.001,
        "output should align with the student lane for direct answer flow"
    );
    let output_edge = connector(&plan, "e_student_to_output");
    assert!(
        output_edge.label.is_none(),
        "prediction edge label is redundant with output node and should be removed"
    );
    assert!(
        (output_edge.points[0][1] - center_y(student)).abs() < 0.001,
        "prediction edge should be a straight lane from student to output: {:?}",
        output_edge.points
    );
    assert!(
        output_edge.points.windows(2).all(|window| {
            (window[0][0] - window[1][0]).abs() < 0.0001
                || (window[0][1] - window[1][1]).abs() < 0.0001
        }),
        "student-to-output route should stay orthogonal/direct: {:?}",
        output_edge.points
    );
    assert!(
        !plan.objects.iter().any(|object| matches!(
            object,
            DrawObject::Connector { from, to, .. }
                if from.as_deref() == Some("student") && to.as_deref() == Some("task_loss_fn")
        )),
        "task loss should consume the prediction node instead of a long direct student edge"
    );
    assert_eq!(
        connector_endpoints(&plan, "e_output_task_loss"),
        (Some("y_hat"), Some("task_loss_fn"))
    );
    assert_eq!(
        connector(&plan, "e_output_task_loss").points[0],
        [output[2], center_y(output)]
    );
    let teacher_residual = connector(&plan, "e_teacher_latent");
    assert!(
        teacher_residual.label.is_none(),
        "formula should be integrated into the residual box instead of floating on an edge"
    );
    assert!(
        box_text(&plan, "residual").contains("h_T - h_S"),
        "latent residual box should carry the compact formula text"
    );
    let residual_edge = connector(&plan, "e_residual_to_student");
    assert!(
        residual_edge.points.len() <= 3,
        "residual supervision should be a simple L-shaped route: {:?}",
        residual_edge.points
    );
    assert!(
        residual_edge.points.last().copied().unwrap()[1] <= student[1] + 0.001,
        "residual supervision should enter from the top edge instead of running through the student body: {:?}",
        residual_edge.points
    );
    assert!(
        connector(&plan, "e_output_task_loss").label.is_none(),
        "y-hat connector label is redundant with output node and should be removed"
    );
    assert!(
        !plan
            .objects
            .iter()
            .any(|object| matches!(object, DrawObject::Text { id, .. } if id == "training_badge")),
        "phase badges should not be inserted when they collide with top routes"
    );
    assert!(
        !plan
            .objects
            .iter()
            .any(|object| matches!(object, DrawObject::Box { id, .. } if id == "student_inf")),
        "raw optimizer inference boxes should be replaced by a controlled inference lane"
    );
    assert!(
        !box_text(&plan, "student").contains("train / inference"),
        "training and inference should not be merged into one student label"
    );
    let inference_student = object_box(&plan, "inference_student");
    assert!(
        inference_student[0] > student[2] + 0.18
            && (center_y(inference_student) - center_y(student)).abs() < 0.08,
        "inference student should be placed as a right-side continuation of the trained student"
    );
    assert!(
        box_text(&plan, "inference_student").contains("inference"),
        "controlled inference lane should keep explicit inference semantics"
    );
    assert!(
        !plan.objects.iter().any(|object| matches!(
            object,
            DrawObject::Connector { from, to, .. }
                if from.as_deref() == Some("x_input") && to.as_deref() == Some("inference_student")
        )),
        "controlled inference lane should not keep the long input detour"
    );
    assert!(
        plan.objects.iter().any(|object| matches!(
            object,
            DrawObject::Connector { from, to, .. }
                if from.as_deref() == Some("student") && to.as_deref() == Some("inference_student")
        )),
        "controlled inference lane should continue from the trained student"
    );
    let inference_edge = connector(&plan, "e_student_inference");
    assert!(
        inference_edge.points.len() <= 3
            && inference_edge
                .points
                .iter()
                .all(|point| point[1] <= student[1] + 0.02),
        "student-to-inference continuation should route above the training output row: {:?}",
        inference_edge.points
    );
    assert!(
        !plan.objects.iter().any(|object| matches!(
            object,
            DrawObject::Text { id, .. }
                if id == "phase_training_label" || id == "phase_inference_label"
        )),
        "floating phase captions should not be added when module labels already encode phases"
    );
    assert!(
        connector(&plan, "e_residual_to_latent_loss")
            .label
            .is_none(),
        "latent loss label is redundant with the loss box and should not sit on the line"
    );
    assert!(
        !plan
            .objects
            .iter()
            .any(|object| matches!(object, DrawObject::Text { id, .. } if id == "inference_badge")),
        "inference badge from model output should be removed when it intrudes into the main route"
    );
}

#[test]
fn draw_plan_geometry_repair_preserves_existing_prediction_node_from_optimizer_output() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input_text".to_string(),
            bbox: [0.04, 0.38, 0.15, 0.56],
            text: "Task Input".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher_module".to_string(),
            bbox: [0.22, 0.13, 0.42, 0.29],
            text: "Teacher LM".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student_module".to_string(),
            bbox: [0.22, 0.48, 0.42, 0.64],
            text: "Student".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "teacher_latent".to_string(),
            bbox: [0.29, 0.23, 0.63, 0.38],
            text: "Latent h_T".to_string(),
            role: "context".to_string(),
            style: "neutral_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "student_output".to_string(),
            bbox: [0.29, 0.77, 0.63, 0.96],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "latent_residual_signal".to_string(),
            bbox: [0.50, 0.31, 0.64, 0.47],
            text: "Latent Residual".to_string(),
            role: "module".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.86, 0.68, 0.97, 0.84],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Text {
            id: "a_inference".to_string(),
            bbox: [0.50, 0.88, 0.82, 0.94],
            text: "Inference only".to_string(),
            style: "annotation".to_string(),
            z: 40,
        },
        DrawObject::Connector {
            id: "e_input_to_teacher".to_string(),
            points: vec![[0.15, 0.43], [0.22, 0.21]],
            from: Some("input_text".to_string()),
            to: Some("teacher_module".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_input_to_student".to_string(),
            points: vec![[0.15, 0.51], [0.22, 0.56]],
            from: Some("input_text".to_string()),
            to: Some("student_module".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_teacher_to_latent".to_string(),
            points: vec![[0.42, 0.21], [0.50, 0.39]],
            from: Some("teacher_module".to_string()),
            to: Some("teacher_latent".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "e_student_output_extra".to_string(),
            points: vec![[0.32, 0.58], [0.32, 0.77]],
            from: Some("student_module".to_string()),
            to: Some("student_output".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "e_latent_to_student".to_string(),
            points: vec![[0.57, 0.47], [0.42, 0.48]],
            from: Some("latent_residual_signal".to_string()),
            to: Some("student_module".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Connector {
            id: "e_student_to_taskloss".to_string(),
            points: vec![[0.42, 0.61], [0.86, 0.76]],
            from: Some("student_module".to_string()),
            to: Some("task_loss".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 14,
        },
    ]);

    repair_draw_plan_geometry(&mut plan);

    let output = object_box(&plan, "student_output");
    let loss = object_box(&plan, "task_loss");
    assert!(
        output[0] > object_box(&plan, "latent_residual_signal")[2],
        "existing prediction node should sit to the right of the residual"
    );
    assert!(
        loss[0] > output[2],
        "task loss should sit after the existing prediction node"
    );
    assert!(
        !plan
            .objects
            .iter()
            .any(|object| draw_object_id(object) == "output_pred"),
        "repair should not create a synthetic output when the optimizer already provided one"
    );
    assert!(
        !plan.objects.iter().any(|object| matches!(
            object,
            DrawObject::Box { id, .. } if id == "teacher_latent"
        )),
        "intermediate hidden latent boxes from the optimizer should be removed"
    );
    assert!(
        !plan.objects.iter().any(|object| matches!(
            object,
            DrawObject::Connector { from, to, .. }
                if to.as_deref() == Some("teacher_latent")
        )),
        "connectors to removed hidden latent boxes should be removed"
    );
    assert!(
        !plan.objects.iter().any(|object| matches!(
            object,
            DrawObject::Connector { from, to, .. }
                if from.as_deref() == Some("student_module")
                    && to.as_deref() == Some("task_loss")
        )),
        "missing output should not force a long direct student-to-loss edge"
    );
    assert!(
        plan.objects.iter().any(|object| matches!(
            object,
            DrawObject::Connector { from, to, .. }
                if from.as_deref() == Some("student_module")
                    && to.as_deref() == Some("student_output")
        )),
        "training lane should preserve student-to-prediction flow"
    );
    assert!(
        plan.objects.iter().any(|object| matches!(
            object,
            DrawObject::Connector { from, to, .. }
                if from.as_deref() == Some("student_output")
                    && to.as_deref() == Some("task_loss")
        )),
        "task loss should consume the prediction node"
    );
    assert!(
        !plan
            .objects
            .iter()
            .any(|object| draw_object_id(object) == "inference_student"),
        "an inference annotation alone should not expand into a redundant inference subgraph"
    );
}

#[test]
fn draw_plan_geometry_repair_respects_existing_student_output_and_direct_residual_edge() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input_text".to_string(),
            bbox: [0.04, 0.34, 0.15, 0.52],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher_model".to_string(),
            bbox: [0.22, 0.10, 0.42, 0.26],
            text: "Teacher\n(frozen)".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student_model".to_string(),
            bbox: [0.22, 0.42, 0.42, 0.58],
            text: "Student\n(trainable)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "teacher_out".to_string(),
            bbox: [0.68, 0.14, 0.78, 0.30],
            text: "z_t".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "student_out".to_string(),
            bbox: [0.68, 0.42, 0.78, 0.58],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "task_label_y".to_string(),
            bbox: [0.04, 0.62, 0.14, 0.76],
            text: "y".to_string(),
            role: "data".to_string(),
            style: "neutral_module".to_string(),
            z: 25,
        },
        DrawObject::Connector {
            id: "e_input_teacher".to_string(),
            points: vec![[0.15, 0.43], [0.22, 0.18]],
            from: Some("input_text".to_string()),
            to: Some("teacher_model".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_input_student".to_string(),
            points: vec![[0.15, 0.51], [0.22, 0.50]],
            from: Some("input_text".to_string()),
            to: Some("student_model".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_teacher_latent".to_string(),
            points: vec![[0.42, 0.18], [0.68, 0.22]],
            from: Some("teacher_model".to_string()),
            to: Some("teacher_out".to_string()),
            style: "normal_flow".to_string(),
            label: Some(DrawLabel {
                text: "latent".to_string(),
                bbox: [0.52, 0.16, 0.62, 0.21],
            }),
            z: 12,
        },
        DrawObject::Connector {
            id: "e_latent_residual".to_string(),
            points: vec![[0.68, 0.22], [0.42, 0.50]],
            from: Some("teacher_out".to_string()),
            to: Some("student_model".to_string()),
            style: "main_flow".to_string(),
            label: Some(DrawLabel {
                text: "residual".to_string(),
                bbox: [0.52, 0.30, 0.66, 0.35],
            }),
            z: 13,
        },
        DrawObject::Connector {
            id: "e_student_pred".to_string(),
            points: vec![[0.42, 0.50], [0.68, 0.50]],
            from: Some("student_model".to_string()),
            to: Some("student_out".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 14,
        },
        DrawObject::Connector {
            id: "e_task_loss".to_string(),
            points: vec![[0.14, 0.69], [0.22, 0.50]],
            from: Some("task_label_y".to_string()),
            to: Some("student_model".to_string()),
            style: "main_flow".to_string(),
            label: Some(DrawLabel {
                text: "task loss".to_string(),
                bbox: [0.18, 0.46, 0.28, 0.51],
            }),
            z: 15,
        },
        DrawObject::Text {
            id: "anno_inference".to_string(),
            bbox: [0.58, 0.92, 0.92, 1.0],
            text: "Inference: student only".to_string(),
            style: "annotation".to_string(),
            z: 40,
        },
    ]);

    repair_draw_plan_geometry(&mut plan);

    for invented_id in [
        "latent_residual",
        "inference_student",
        "inference_output",
        "output_pred",
    ] {
        assert!(
            !plan
                .objects
                .iter()
                .any(|object| draw_object_id(object) == invented_id),
            "repair should not invent semantic node {invented_id} when the DrawPlan already has a direct semantic edge"
        );
    }
    let teacher_out = object_box(&plan, "teacher_out");
    let student_out = object_box(&plan, "student_out");
    assert!(
        teacher_out[3] < student_out[1],
        "teacher output and student prediction should remain distinct lanes: {teacher_out:?} {student_out:?}"
    );
    assert_eq!(
        connector_endpoints(&plan, "e_student_pred"),
        (Some("student_model"), Some("student_out")),
        "student prediction edge must target student_out, not teacher_out"
    );
    assert!(
        !plan.objects.iter().any(|object| matches!(
            object,
            DrawObject::Connector { from, to, .. }
                if from.as_deref() == Some("student_model")
                    && to.as_deref() == Some("teacher_out")
        )),
        "repair must not create a student-to-teacher-output shortcut"
    );
    assert_eq!(
        input_teacher_style(&plan, "e_latent_residual"),
        "dashed_supervision",
        "existing residual edge should carry dashed supervision semantics"
    );
    assert!(
        connector(&plan, "e_latent_residual").label.is_none(),
        "residual label should not sit on top of the connector"
    );
    assert!(
        !plan
            .objects
            .iter()
            .any(|object| matches!(object, DrawObject::Text { id, .. } if id == "anno_inference")),
        "inference annotation can be removed instead of expanding into a redundant subgraph"
    );
}

#[test]
fn draw_plan_geometry_repair_removes_section_labels_and_long_task_loss_feedback() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input".to_string(),
            bbox: [0.04, 0.34, 0.15, 0.52],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student".to_string(),
            bbox: [0.22, 0.42, 0.42, 0.58],
            text: "Student".to_string(),
            role: "main".to_string(),
            style: "primary_module_regular".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "teacher".to_string(),
            bbox: [0.22, 0.10, 0.42, 0.26],
            text: "Teacher".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "latent_residual".to_string(),
            bbox: [0.50, 0.26, 0.64, 0.42],
            text: "Latent Residual\nh_T - h_S".to_string(),
            role: "module".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "output".to_string(),
            bbox: [0.68, 0.42, 0.78, 0.58],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.84, 0.42, 0.95, 0.58],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "neutral_module".to_string(),
            z: 25,
        },
        DrawObject::Text {
            id: "training_label".to_string(),
            bbox: [0.38, 0.02, 0.62, 0.07],
            text: "Training".to_string(),
            style: "section_header".to_string(),
            z: 30,
        },
        DrawObject::Text {
            id: "inference_label".to_string(),
            bbox: [0.30, 0.62, 0.58, 0.67],
            text: "Inference: student only".to_string(),
            style: "section_subtitle".to_string(),
            z: 31,
        },
        DrawObject::Connector {
            id: "e_input_student".to_string(),
            points: vec![[0.15, 0.51], [0.22, 0.50]],
            from: Some("input".to_string()),
            to: Some("student".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_student_output".to_string(),
            points: vec![[0.42, 0.50], [0.68, 0.50]],
            from: Some("student".to_string()),
            to: Some("output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_teacher_latent".to_string(),
            points: vec![[0.42, 0.18], [0.50, 0.34]],
            from: Some("teacher".to_string()),
            to: Some("latent_residual".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "e_latent_residual".to_string(),
            points: vec![[0.57, 0.42], [0.42, 0.42]],
            from: Some("latent_residual".to_string()),
            to: Some("student".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Connector {
            id: "e_task_loss".to_string(),
            points: vec![[0.78, 0.50], [0.84, 0.50]],
            from: Some("output".to_string()),
            to: Some("task_loss".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 14,
        },
        DrawObject::Connector {
            id: "e_task_loss_back".to_string(),
            points: vec![
                [0.84, 0.50],
                [0.68, 0.50],
                [0.68, 0.64],
                [0.32, 0.64],
                [0.32, 0.58],
            ],
            from: Some("task_loss".to_string()),
            to: Some("student".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 15,
        },
        DrawObject::Connector {
            id: "task_loss_edge".to_string(),
            points: vec![[0.67, 0.38], [0.55, 0.38], [0.55, 0.36], [0.42, 0.36]],
            from: Some("output".to_string()),
            to: Some("student".to_string()),
            style: "aux_flow".to_string(),
            label: Some(DrawLabel {
                text: "Task Loss".to_string(),
                bbox: [0.49, 0.30, 0.59, 0.34],
            }),
            z: 16,
        },
    ]);

    repair_draw_plan_geometry(&mut plan);

    for removed_id in ["training_label", "inference_label"] {
        assert!(
            !plan
                .objects
                .iter()
                .any(|object| draw_object_id(object) == removed_id),
            "{removed_id} should be removed instead of floating around the diagram"
        );
    }
    assert!(
        plan.objects.iter().any(|object| matches!(
            object,
            DrawObject::Connector { from, to, .. }
                if from.as_deref() == Some("output") && to.as_deref() == Some("task_loss")
        )),
        "task loss should remain connected to the prediction output"
    );
    for removed_edge in ["e_task_loss_back", "task_loss_edge"] {
        assert!(
            !plan
                .objects
                .iter()
                .any(|object| draw_object_id(object) == removed_edge),
            "{removed_edge} should be removed when output-to-loss already carries the training signal"
        );
    }
    assert!(
        box_text(&plan, "teacher").contains("training"),
        "teacher text should carry the phase cue instead of a floating section label"
    );
    assert!(
        box_text(&plan, "student").contains("inference"),
        "student text should carry the inference cue instead of a floating section label"
    );
}

#[test]
fn draw_plan_geometry_repair_keeps_direct_residual_edge_when_no_residual_box_exists() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input_text".to_string(),
            bbox: [0.05, 0.42, 0.17, 0.58],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student_main".to_string(),
            bbox: [0.28, 0.62, 0.52, 0.85],
            text: "Student\n(prediction)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "teacher_latent".to_string(),
            bbox: [0.28, 0.15, 0.52, 0.38],
            text: "Teacher\n(latents)".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.62, 0.72, 0.76, 0.84],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "neutral_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "answer_out".to_string(),
            bbox: [0.78, 0.64, 0.86, 0.83],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Connector {
            id: "e_student_answer".to_string(),
            points: vec![[0.52, 0.735], [0.57, 0.735], [0.78, 0.735]],
            from: Some("student_main".to_string()),
            to: Some("answer_out".to_string()),
            style: "main_flow".to_string(),
            label: Some(DrawLabel {
                text: "predict".to_string(),
                bbox: [0.63, 0.74, 0.72, 0.78],
            }),
            z: 10,
        },
        DrawObject::Connector {
            id: "e_student_loss".to_string(),
            points: vec![[0.52, 0.735], [0.62, 0.78]],
            from: Some("student_main".to_string()),
            to: Some("task_loss".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_teacher_residual".to_string(),
            points: vec![[0.52, 0.265], [0.57, 0.265], [0.57, 0.62], [0.52, 0.62]],
            from: Some("teacher_latent".to_string()),
            to: Some("student_main".to_string()),
            style: "main_flow".to_string(),
            label: Some(DrawLabel {
                text: "residual".to_string(),
                bbox: [0.56, 0.46, 0.72, 0.51],
            }),
            z: 12,
        },
    ]);

    repair_draw_plan_geometry(&mut plan);

    assert!(plan.objects.iter().any(
        |object| matches!(object, DrawObject::Connector { from, to, style, .. }
            if from.as_deref() == Some("teacher_latent")
                && to.as_deref() == Some("student_main")
                && style == "dashed_supervision")
    ));
    assert!(
        !plan.objects.iter().any(|object| matches!(
            object,
            DrawObject::Box { id, .. } if id == "latent_residual"
        )),
        "repair should not invent a residual box when the model used a direct residual edge"
    );
    let answer_edge = connector(&plan, "e_student_answer");
    let answer_edge_y = answer_edge.points[0][1];
    let answer = object_box(&plan, "answer_out");
    let task_loss = object_box(&plan, "task_loss");
    assert!(
        (center_y(task_loss) - answer_edge_y).abs() < 0.04 && task_loss[0] > answer[2],
        "task loss should continue the prediction lane to the right: {task_loss:?}"
    );
    assert!(
        !plan
            .objects
            .iter()
            .any(|object| matches!(object, DrawObject::Text { id, .. } if id == "inference_badge")),
        "do not add a marginal inference badge when it would sit outside the main diagram"
    );
}

#[test]
fn draw_plan_geometry_repair_uses_figure_plan_as_semantic_source_of_truth() {
    let figure_plan = teacher_student_semantic_plan();
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "teacher_model".to_string(),
            bbox: [0.22, 0.10, 0.42, 0.26],
            text: "Teacher".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student_model".to_string(),
            bbox: [0.22, 0.42, 0.42, 0.58],
            text: "Student".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "task_input".to_string(),
            bbox: [0.04, 0.34, 0.15, 0.52],
            text: "x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "task_output".to_string(),
            bbox: [0.68, 0.42, 0.78, 0.58],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.84, 0.42, 0.95, 0.58],
            text: "L_task".to_string(),
            role: "loss".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "latent_residual".to_string(),
            bbox: [0.32, 0.28, 0.72, 0.64],
            text: "Latent Residual\n(Supervision)".to_string(),
            role: "loss".to_string(),
            style: "primary_module".to_string(),
            z: 25,
        },
        DrawObject::Box {
            id: "inference_student".to_string(),
            bbox: [0.22, 0.74, 0.42, 0.90],
            text: "Student\n(inference only)".to_string(),
            role: "main".to_string(),
            style: "primary_module_regular".to_string(),
            z: 26,
        },
        DrawObject::Box {
            id: "inference_output".to_string(),
            bbox: [0.50, 0.76, 0.60, 0.88],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 27,
        },
        DrawObject::Connector {
            id: "student_to_output".to_string(),
            points: vec![[0.42, 0.50], [0.68, 0.50]],
            from: Some("student_model".to_string()),
            to: Some("task_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "task_loss_edge".to_string(),
            points: vec![[0.67, 0.38], [0.55, 0.38], [0.42, 0.36]],
            from: Some("task_output".to_string()),
            to: Some("student_model".to_string()),
            style: "aux_flow".to_string(),
            label: Some(DrawLabel {
                text: "Task Loss".to_string(),
                bbox: [0.49, 0.30, 0.59, 0.34],
            }),
            z: 11,
        },
    ]);

    repair_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    for removed_id in ["latent_residual"] {
        assert!(
            !plan
                .objects
                .iter()
                .any(|object| draw_object_id(object) == removed_id),
            "{removed_id} is absent from FigurePlan components and should be removed"
        );
    }
    assert!(
        plan.objects.iter().any(|object| matches!(
            object,
            DrawObject::Connector { from, to, style, .. }
                if from.as_deref() == Some("teacher_model")
                    && to.as_deref() == Some("student_model")
                    && style == "dashed_supervision"
        )),
        "missing FigurePlan residual edge should be restored as a direct dashed connector"
    );
    assert!(
        !plan
            .objects
            .iter()
            .any(|object| draw_object_id(object) == "task_loss_edge"),
        "duplicate output-to-student task-loss feedback should be removed"
    );
}

#[test]
fn draw_plan_geometry_repair_honors_figure_plan_task_loss_feedback_edge() {
    let figure_plan = teacher_student_feedback_semantic_plan();
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "teacher".to_string(),
            bbox: [0.22, 0.10, 0.42, 0.26],
            text: "Teacher\n(Large LM)".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "input_data".to_string(),
            bbox: [0.04, 0.34, 0.15, 0.52],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student".to_string(),
            bbox: [0.22, 0.42, 0.42, 0.58],
            text: "Student\n(Compact)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "output_pred".to_string(),
            bbox: [0.68, 0.42, 0.78, 0.58],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.84, 0.42, 0.95, 0.58],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Connector {
            id: "e_input_student".to_string(),
            points: vec![[0.15, 0.51], [0.18, 0.51], [0.18, 0.50], [0.22, 0.50]],
            from: Some("input_data".to_string()),
            to: Some("student".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_student_output".to_string(),
            points: vec![[0.42, 0.50], [0.68, 0.50]],
            from: Some("student".to_string()),
            to: Some("output_pred".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_output_task_loss".to_string(),
            points: vec![[0.78, 0.50], [0.84, 0.50]],
            from: Some("output_pred".to_string()),
            to: Some("task_loss".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 12,
        },
    ]);

    repair_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    assert!(
        !plan
            .objects
            .iter()
            .any(|object| matches!(object, DrawObject::Box { id, .. } if id == "task_loss")),
        "unconnected task_loss component should not be rendered as a terminal box when FigurePlan declares output-to-student loss feedback"
    );
    assert!(
        !plan.objects.iter().any(|object| matches!(
            object,
            DrawObject::Connector { from, to, .. }
                if from.as_deref() == Some("task_loss") || to.as_deref() == Some("task_loss")
        )),
        "loss feedback mode should remove terminal loss-box connectors"
    );
    assert_eq!(
        connector_endpoints(&plan, "e_task_loss"),
        (Some("output_pred"), Some("student"))
    );

    let input = object_box(&plan, "input_data");
    let teacher = object_box(&plan, "teacher");
    let student = object_box(&plan, "student");
    let rightmost_box_x = plan
        .objects
        .iter()
        .filter_map(|object| match object {
            DrawObject::Box { bbox, .. } => Some(bbox[2]),
            _ => None,
        })
        .fold(0.0, f64::max);
    assert!(
        (teacher[0] - student[0]).abs() < 0.001 && teacher[3] < student[1],
        "teacher and student should form the aligned training spine: {teacher:?} {student:?}"
    );
    assert!(
        rightmost_box_x - input[0] > 0.80,
        "main objects should use the paper-wide canvas instead of a narrow middle stack"
    );

    let input_edge = connector(&plan, "e_input_student");
    assert_eq!(
        input_edge.points,
        &[
            [input[2], center_y(student)],
            [student[0], center_y(student)]
        ]
    );

    let residual_edge = connector(&plan, "e_teacher_residual");
    let residual_label = residual_edge
        .label
        .as_ref()
        .expect("residual edge should keep the FigurePlan label");
    assert_eq!(residual_label.text, "Latent Residual");
    assert!(
        !label_intersects_any_segment(residual_label.bbox, residual_edge.points),
        "residual label should sit beside the edge, not on its stroke"
    );

    let loss_edge = connector(&plan, "e_task_loss");
    let loss_label = loss_edge
        .label
        .as_ref()
        .expect("task-loss feedback edge should carry its label");
    assert_eq!(loss_label.text, "Task Loss");
    assert!(
        !label_intersects_any_segment(loss_label.bbox, loss_edge.points),
        "task-loss label should sit beside the feedback edge, not on its stroke"
    );
    assert!(
        loss_edge
            .points
            .iter()
            .any(|point| point[1] > student[3] + 0.05),
        "task-loss feedback should visibly loop back to the student instead of becoming a forward terminal edge: {:?}",
        loss_edge.points
    );
}

#[test]
fn draw_plan_geometry_repair_preserves_figure_plan_latent_pair_and_inference_badge() {
    let figure_plan = teacher_student_latent_pair_semantic_plan();
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "teacher".to_string(),
            bbox: [0.10, 0.18, 0.30, 0.34],
            text: "Teacher LM".to_string(),
            role: "main".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student".to_string(),
            bbox: [0.10, 0.50, 0.30, 0.66],
            text: "Student".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "task_input_node".to_string(),
            bbox: [0.02, 0.41, 0.16, 0.59],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "task_output_node".to_string(),
            bbox: [0.78, 0.42, 0.88, 0.58],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "latent_teacher".to_string(),
            bbox: [0.46, 0.16, 0.56, 0.30],
            text: "h_T".to_string(),
            role: "module".to_string(),
            style: "muted_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "latent_student".to_string(),
            bbox: [0.46, 0.48, 0.56, 0.62],
            text: "h_S".to_string(),
            role: "module".to_string(),
            style: "muted_module".to_string(),
            z: 25,
        },
        DrawObject::Box {
            id: "inference_badge".to_string(),
            bbox: [0.62, 0.74, 0.88, 0.82],
            text: "Inference: Student only".to_string(),
            role: "context".to_string(),
            style: "neutral_module".to_string(),
            z: 26,
        },
        DrawObject::Connector {
            id: "e_input_student".to_string(),
            points: vec![[0.16, 0.50], [0.30, 0.58]],
            from: Some("task_input_node".to_string()),
            to: Some("student".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_input_teacher".to_string(),
            points: vec![[0.16, 0.50], [0.30, 0.26]],
            from: Some("task_input_node".to_string()),
            to: Some("teacher".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_teacher_latent".to_string(),
            points: vec![[0.30, 0.26], [0.46, 0.23]],
            from: Some("teacher".to_string()),
            to: Some("latent_teacher".to_string()),
            style: "normal_flow".to_string(),
            label: Some(DrawLabel {
                text: "latent".to_string(),
                bbox: [0.34, 0.18, 0.44, 0.22],
            }),
            z: 12,
        },
        DrawObject::Connector {
            id: "e_student_latent".to_string(),
            points: vec![[0.30, 0.58], [0.46, 0.55]],
            from: Some("student".to_string()),
            to: Some("latent_student".to_string()),
            style: "normal_flow".to_string(),
            label: Some(DrawLabel {
                text: "latent".to_string(),
                bbox: [0.34, 0.60, 0.44, 0.64],
            }),
            z: 13,
        },
        DrawObject::Connector {
            id: "e_residual".to_string(),
            points: vec![[0.51, 0.30], [0.51, 0.48]],
            from: Some("latent_teacher".to_string()),
            to: Some("latent_student".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "r = h_T - h_S".to_string(),
                bbox: [0.53, 0.35, 0.70, 0.40],
            }),
            z: 14,
        },
        DrawObject::Connector {
            id: "e_student_output".to_string(),
            points: vec![[0.30, 0.58], [0.78, 0.50]],
            from: Some("student".to_string()),
            to: Some("task_output_node".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 15,
        },
        DrawObject::Connector {
            id: "e_task_loss".to_string(),
            points: vec![[0.84, 0.58], [0.42, 0.58]],
            from: Some("task_output_node".to_string()),
            to: Some("student".to_string()),
            style: "main_flow".to_string(),
            label: Some(DrawLabel {
                text: "L_task".to_string(),
                bbox: [0.54, 0.50, 0.68, 0.56],
            }),
            z: 16,
        },
    ]);

    repair_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    let teacher = object_box(&plan, "teacher");
    let student = object_box(&plan, "student");
    let latent_teacher = object_box(&plan, "latent_teacher");
    let latent_student = object_box(&plan, "latent_student");
    assert!(
        latent_teacher[0] > teacher[2] && (center_y(latent_teacher) - center_y(teacher)).abs() < 0.001,
        "teacher latent node should be preserved and aligned with teacher: {latent_teacher:?} {teacher:?}"
    );
    assert!(
        latent_student[0] > student[2] && latent_student[1] < student[1],
        "student latent node should be preserved in a latent lane above the prediction flow: {latent_student:?} {student:?}"
    );
    assert!(box_text(&plan, "inference_badge").contains("Inference"));
    assert_eq!(
        connector_endpoints(&plan, "e_residual"),
        (Some("latent_teacher"), Some("latent_student"))
    );
    assert_eq!(
        input_teacher_style(&plan, "e_residual"),
        "dashed_supervision"
    );
    let residual = connector(&plan, "e_residual");
    let residual_label = residual
        .label
        .as_ref()
        .expect("latent residual pair edge should keep the FigurePlan label");
    assert!(residual_label.text.contains("h_T"));
    assert!(
        !label_intersects_any_segment(residual_label.bbox, residual.points),
        "latent residual label should not overlap its dashed edge"
    );

    let loss_feedback = connector(&plan, "e_task_loss");
    let loss_label = loss_feedback
        .label
        .as_ref()
        .expect("loss feedback label should remain editable");
    assert!(
        loss_label.bbox[1] > 0.69,
        "loss label should sit below the feedback loop and away from the student-output line: {:?}",
        loss_label.bbox
    );
}

#[test]
fn draw_plan_geometry_repair_canonicalizes_explicit_inference_lane_from_figure_plan() {
    let figure_plan = teacher_student_explicit_inference_semantic_plan();
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_input".to_string(),
            bbox: [0.05, 0.41, 0.18, 0.59],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "comp_teacher".to_string(),
            bbox: [0.28, 0.10, 0.52, 0.26],
            text: "Teacher\n(training only)".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "comp_latent".to_string(),
            bbox: [0.28, 0.64, 0.52, 0.76],
            text: "Latent residual r".to_string(),
            role: "loss".to_string(),
            style: "neutral_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "comp_student_train".to_string(),
            bbox: [0.28, 0.42, 0.52, 0.58],
            text: "Student\ntraining + inference".to_string(),
            role: "main".to_string(),
            style: "primary_module_regular".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "comp_task_loss".to_string(),
            bbox: [0.91, 0.42, 0.98, 0.58],
            text: "Task loss L_task".to_string(),
            role: "loss".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "comp_student_inf".to_string(),
            bbox: [0.62, 0.38, 0.74, 0.54],
            text: "Student\n(inference)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 26,
        },
        DrawObject::Box {
            id: "comp_answer".to_string(),
            bbox: [0.76, 0.42, 0.88, 0.58],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 27,
        },
        DrawObject::Connector {
            id: "edge_input_teacher".to_string(),
            points: vec![[0.18, 0.43], [0.18, 0.18], [0.28, 0.18]],
            from: Some("comp_input".to_string()),
            to: Some("comp_teacher".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "edge_teacher_latent".to_string(),
            points: vec![[0.40, 0.24], [0.40, 0.64]],
            from: Some("comp_teacher".to_string()),
            to: Some("comp_latent".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "edge_latent_student".to_string(),
            points: vec![[0.40, 0.64], [0.40, 0.54]],
            from: Some("comp_latent".to_string()),
            to: Some("comp_student_train".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "edge_input_student".to_string(),
            points: vec![[0.18, 0.50], [0.28, 0.50]],
            from: Some("comp_input".to_string()),
            to: Some("comp_student_train".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Connector {
            id: "edge_input_inf".to_string(),
            points: vec![[0.18, 0.46], [0.18, 0.62], [0.62, 0.62], [0.62, 0.54]],
            from: Some("comp_input".to_string()),
            to: Some("comp_student_inf".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 15,
        },
        DrawObject::Connector {
            id: "edge_inf_answer".to_string(),
            points: vec![[0.74, 0.46], [0.82, 0.46]],
            from: Some("comp_student_inf".to_string()),
            to: Some("comp_answer".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 16,
        },
        DrawObject::Connector {
            id: "e_student_output".to_string(),
            points: vec![[0.52, 0.46], [0.62, 0.46]],
            from: Some("comp_student_train".to_string()),
            to: Some("comp_student_inf".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 43,
        },
        DrawObject::Connector {
            id: "e_output_task_loss".to_string(),
            points: vec![[0.88, 0.50], [0.91, 0.50]],
            from: Some("comp_answer".to_string()),
            to: Some("comp_task_loss".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 44,
        },
        DrawObject::Box {
            id: "comp_inference_label".to_string(),
            bbox: [0.62, 0.72, 0.88, 0.80],
            text: "Inference".to_string(),
            role: "context".to_string(),
            style: "neutral_module".to_string(),
            z: 25,
        },
        DrawObject::Connector {
            id: "e_student_output_1".to_string(),
            points: vec![[0.52, 0.50], [0.76, 0.50]],
            from: Some("comp_student_train".to_string()),
            to: Some("comp_answer".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 45,
        },
    ]);

    repair_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    assert!(
        !plan.objects.iter().any(
            |object| matches!(object, DrawObject::Box { id, .. } if id == "comp_inference_label")
        ),
        "phase-only inference label should be folded into the inference student, not left as a floating box"
    );
    assert!(
        box_text(&plan, "comp_student_train").contains("training")
            && !box_text(&plan, "comp_student_train").contains("inference"),
        "training student should not conflate training and inference roles"
    );
    assert!(
        box_text(&plan, "comp_student_inf").contains("inference"),
        "explicit inference student should carry the inference phase label"
    );

    let inference = object_box(&plan, "comp_student_inf");
    let training_student = object_box(&plan, "comp_student_train");
    assert!(
        inference[0] > training_student[2] + 0.18
            && (center_y(inference) - center_y(training_student)).abs() < 0.08,
        "explicit inference student should live in the right continuation column: {inference:?} {training_student:?}"
    );
    let input = object_box(&plan, "comp_input");
    let inference_edge = connector(&plan, "edge_input_inf");
    assert!(
        inference_edge.points.len() <= 3
            && inference_edge.points.first().copied() == Some([input[2], center_y(input)])
            && inference_edge.points.last().copied() == Some([inference[0], center_y(inference)])
            && inference_edge.points.windows(2).all(|window| {
                (window[0][0] - window[1][0]).abs() < 0.0001
                    || (window[0][1] - window[1][1]).abs() < 0.0001
            }),
        "explicit input-to-inference flow should be a clean orthogonal route into the right continuation column: {:?}",
        inference_edge.points
    );
    assert!(
        !plan.objects.iter().any(|object| matches!(
            object,
            DrawObject::Connector { from, to, .. }
                if from.as_deref() == Some("comp_student_train")
                    && to.as_deref() == Some("comp_student_inf")
        )),
        "repair should remove model-invented training-to-inference edge absent from FigurePlan"
    );
    assert!(
        !plan.objects.iter().any(|object| matches!(
            object,
            DrawObject::Connector { from, to, .. }
                if from.as_deref() == Some("comp_student_train")
                    && to.as_deref() == Some("comp_answer")
        )),
        "repair should not add a duplicate training-to-answer edge when FigurePlan uses inference student as the answer source"
    );
    assert!(
        !plan.objects.iter().any(|object| matches!(
            object,
            DrawObject::Connector { from, to, .. }
                if from.as_deref() == Some("comp_answer")
                    && to.as_deref() == Some("comp_task_loss")
        )),
        "loss endpoint should follow FigurePlan student-to-loss edge, not a short output-to-loss shortcut"
    );
    assert!(
        plan.objects.iter().any(|object| matches!(
            object,
            DrawObject::Connector { from, to, .. }
                if from.as_deref() == Some("comp_student_train")
                    && to.as_deref() == Some("comp_task_loss")
        )),
        "student-to-loss edge from FigurePlan should be preserved"
    );
    let inference_answer = connector(&plan, "edge_inf_answer");
    let answer = object_box(&plan, "comp_answer");
    assert_eq!(
        inference_answer.points,
        &[
            [inference[2], center_y(inference)],
            [answer[0], center_y(inference)]
        ],
        "inference output should stay integrated with the right continuation column"
    );

    let latent = object_box(&plan, "comp_latent");
    let student = object_box(&plan, "comp_student_train");
    assert!(
        latent[0] > student[2],
        "residual box should be placed as a semantic module to the right of the training student"
    );
    let teacher_latent = connector(&plan, "edge_teacher_latent");
    assert!(
        teacher_latent.label.is_none(),
        "residual formula should be integrated into the residual box instead of duplicated as a floating label"
    );
    assert!(
        box_text(&plan, "comp_latent").contains("h_T - h_S"),
        "residual box should carry the compact formula text"
    );
}

#[test]
fn draw_plan_geometry_repair_keeps_inference_outputs_and_residual_loss_inside_main_canvas() {
    let figure_plan = teacher_student_margin_variant_semantic_plan();
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input_text".to_string(),
            bbox: [0.05, 0.41, 0.18, 0.59],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student_train".to_string(),
            bbox: [0.28, 0.42, 0.52, 0.58],
            text: "Student\n(training)".to_string(),
            role: "main".to_string(),
            style: "primary_module_regular".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "teacher".to_string(),
            bbox: [0.28, 0.10, 0.52, 0.26],
            text: "Teacher\n(training only)".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "latent_residual".to_string(),
            bbox: [0.58, 0.28, 0.72, 0.40],
            text: "Latent Residual\nh_T - h_S".to_string(),
            role: "module".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "task_label".to_string(),
            bbox: [0.91, 0.42, 0.98, 0.58],
            text: "Task label y".to_string(),
            role: "loss".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "residual_supervision".to_string(),
            bbox: [0.6334, 0.9484, 0.9082, 0.9916],
            text: "Residual loss".to_string(),
            role: "loss".to_string(),
            style: "neutral_module".to_string(),
            z: 25,
        },
        DrawObject::Box {
            id: "student_infer".to_string(),
            bbox: [0.62, 0.38, 0.74, 0.54],
            text: "Student\n(inference)".to_string(),
            role: "main".to_string(),
            style: "primary_module_regular".to_string(),
            z: 26,
        },
        DrawObject::Box {
            id: "answer_train_out".to_string(),
            bbox: [0.76, 0.42, 0.88, 0.58],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 27,
        },
        DrawObject::Box {
            id: "answer_infer_out".to_string(),
            bbox: [0.5917, 0.9484, 0.6999, 0.9916],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 28,
        },
        DrawObject::Connector {
            id: "e_teacher_residual".to_string(),
            points: vec![[0.52, 0.18], [0.58, 0.18], [0.58, 0.34]],
            from: Some("teacher".to_string()),
            to: Some("latent_residual".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_label_loss".to_string(),
            points: vec![[0.91, 0.50], [0.52, 0.50]],
            from: Some("task_label".to_string()),
            to: Some("student_train".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_residual_loss".to_string(),
            points: vec![[0.77, 0.97], [0.58, 0.97], [0.58, 0.67], [0.40, 0.67]],
            from: Some("residual_supervision".to_string()),
            to: Some("student_train".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "e_input_infer".to_string(),
            points: vec![[0.18, 0.46], [0.62, 0.46]],
            from: Some("input_text".to_string()),
            to: Some("student_infer".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Connector {
            id: "e_infer_answer".to_string(),
            points: vec![[0.40, 0.97], [0.64, 0.97]],
            from: Some("student_infer".to_string()),
            to: Some("answer_infer_out".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 14,
        },
    ]);

    repair_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    let student = object_box(&plan, "student_train");
    let inference = object_box(&plan, "student_infer");
    let answer_infer = object_box(&plan, "answer_infer_out");
    let residual_loss = object_box(&plan, "residual_supervision");
    assert!(
        inference[0] > student[2] + 0.18 && (center_y(inference) - center_y(student)).abs() < 0.08,
        "inference branch should move to the right continuation column: {inference:?} {student:?}"
    );
    assert!(
        answer_infer[1] < 0.90 && (center_y(answer_infer) - center_y(inference)).abs() < 0.02,
        "inference answer should be integrated with the right continuation column, not bottom margin: {answer_infer:?}"
    );
    assert!(
        residual_loss[1] < 0.80,
        "residual loss should stay inside the main diagram, not bottom margin: {residual_loss:?}"
    );
    assert!(
        plan.objects.iter().any(|object| matches!(
            object,
            DrawObject::Connector { from, to, .. }
                if from.as_deref() == Some("task_label")
                    && to.as_deref() == Some("student_train")
        )),
        "task label supervision edge declared by FigurePlan should be preserved"
    );
    let residual_loss_edge = connector(&plan, "e_residual_loss");
    assert!(
        residual_loss_edge
            .points
            .iter()
            .all(|point| point[1] < 0.90)
            && residual_loss_edge.points.windows(2).all(|window| {
                (window[0][0] - window[1][0]).abs() < 0.0001
                    || (window[0][1] - window[1][1]).abs() < 0.0001
            }),
        "residual loss route should be orthogonal and inside the main diagram: {:?}",
        residual_loss_edge.points
    );
    let teacher_residual = connector(&plan, "e_teacher_residual");
    assert!(
        teacher_residual.label.is_none(),
        "residual formula should be kept inside the residual box instead of duplicated on the edge"
    );
    assert!(
        box_text(&plan, "latent_residual").contains("h_T - h_S"),
        "residual box should carry the compact formula text"
    );
}

#[test]
fn draw_plan_geometry_repair_expands_teacher_latent_and_avoids_degenerate_task_loss_edge() {
    let figure_plan = teacher_student_quality_gate_semantic_plan();
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input".to_string(),
            bbox: [0.05, 0.41, 0.18, 0.59],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher".to_string(),
            bbox: [0.28, 0.10, 0.52, 0.26],
            text: "Teacher".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student_train".to_string(),
            bbox: [0.28, 0.42, 0.52, 0.58],
            text: "Student".to_string(),
            role: "main".to_string(),
            style: "primary_module_regular".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "teacher_latent".to_string(),
            bbox: [0.30, 0.28, 0.50, 0.36],
            text: "Latent h_T".to_string(),
            role: "context".to_string(),
            style: "neutral_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "residual".to_string(),
            bbox: [0.60, 0.34, 0.76, 0.46],
            text: "Residual\nh_T - h_S".to_string(),
            role: "loss".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.91, 0.42, 0.98, 0.58],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "neutral_module".to_string(),
            z: 25,
        },
        DrawObject::Box {
            id: "output_train".to_string(),
            bbox: [0.76, 0.42, 0.88, 0.58],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 26,
        },
        DrawObject::Connector {
            id: "e_teacher_latent".to_string(),
            points: vec![[0.40, 0.24], [0.40, 0.28]],
            from: Some("teacher".to_string()),
            to: Some("teacher_latent".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_latent_residual".to_string(),
            points: vec![[0.50, 0.32], [0.60, 0.40]],
            from: Some("teacher_latent".to_string()),
            to: Some("residual".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_student_output".to_string(),
            points: vec![[0.52, 0.50], [0.76, 0.50]],
            from: Some("student_train".to_string()),
            to: Some("output_train".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "e_task_loss".to_string(),
            points: vec![[0.88, 0.50], [0.91, 0.50]],
            from: Some("output_train".to_string()),
            to: Some("task_loss".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 13,
        },
    ]);

    repair_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    let teacher_latent = object_box(&plan, "teacher_latent");
    assert!(
        teacher_latent[3] - teacher_latent[1] >= 0.10,
        "teacher_latent should have enough height to pass local quality gate: {teacher_latent:?}"
    );
    let task_loss = object_box(&plan, "task_loss");
    let output = object_box(&plan, "output_train");
    assert!(
        task_loss[0] - output[2] >= 0.04,
        "output-to-task-loss gap should be long enough for a non-degenerate edge: {output:?} {task_loss:?}"
    );
    let task_loss_edge = connector(&plan, "e_task_loss");
    let edge_length = task_loss_edge
        .points
        .windows(2)
        .map(|window| (window[0][0] - window[1][0]).abs() + (window[0][1] - window[1][1]).abs())
        .sum::<f64>();
    assert!(
        edge_length >= 0.04,
        "task loss edge should pass degenerate-edge quality gate: {:?}",
        task_loss_edge.points
    );
}

#[test]
fn draw_plan_geometry_repair_routes_latest_residual_and_loss_without_crossings() {
    let figure_plan = teacher_student_latest_residual_feedback_semantic_plan();
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input_text".to_string(),
            bbox: [0.05, 0.41, 0.18, 0.59],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher_module".to_string(),
            bbox: [0.28, 0.10, 0.52, 0.26],
            text: "Teacher\n(Large LM)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student_module".to_string(),
            bbox: [0.28, 0.42, 0.52, 0.58],
            text: "Student\n(Compact)".to_string(),
            role: "main".to_string(),
            style: "primary_module_regular".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "latent_residual".to_string(),
            bbox: [0.58, 0.28, 0.72, 0.40],
            text: "Latent\nResidual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.93, 0.42, 0.99, 0.58],
            text: "Task\nLoss".to_string(),
            role: "loss".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "output_pred".to_string(),
            bbox: [0.76, 0.42, 0.88, 0.58],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 25,
        },
        DrawObject::Box {
            id: "inference_student".to_string(),
            bbox: [0.28, 0.74, 0.52, 0.90],
            text: "Student Only".to_string(),
            role: "output".to_string(),
            style: "primary_module_regular".to_string(),
            z: 26,
        },
        DrawObject::Connector {
            id: "e_teacher_latent".to_string(),
            points: vec![[0.52, 0.18], [0.58, 0.18], [0.58, 0.34]],
            from: Some("teacher_module".to_string()),
            to: Some("latent_residual".to_string()),
            style: "normal_flow".to_string(),
            label: Some(DrawLabel {
                text: "z_t".to_string(),
                bbox: [0.47, 0.10, 0.63, 0.16],
            }),
            z: 12,
        },
        DrawObject::Connector {
            id: "e_student_latent".to_string(),
            points: vec![[0.52, 0.50], [0.55, 0.50], [0.55, 0.34], [0.58, 0.34]],
            from: Some("student_module".to_string()),
            to: Some("latent_residual".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Connector {
            id: "e_latent_loss".to_string(),
            points: vec![[0.65, 0.40], [0.65, 0.42], [0.52, 0.42]],
            from: Some("latent_residual".to_string()),
            to: Some("student_module".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 14,
        },
        DrawObject::Connector {
            id: "e_task_loss".to_string(),
            points: vec![[0.96, 0.58], [0.96, 0.66], [0.40, 0.66], [0.40, 0.58]],
            from: Some("task_loss".to_string()),
            to: Some("student_module".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 15,
        },
        DrawObject::Connector {
            id: "e_student_output".to_string(),
            points: vec![[0.52, 0.50], [0.76, 0.50]],
            from: Some("student_module".to_string()),
            to: Some("output_pred".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 16,
        },
        DrawObject::Connector {
            id: "e_inference_input".to_string(),
            points: vec![[0.18, 0.50], [0.18, 0.82], [0.28, 0.82]],
            from: Some("input_text".to_string()),
            to: Some("inference_student".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 17,
        },
    ]);

    repair_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    assert!(
        !text_object_exists(&plan, "anno_training") && !text_object_exists(&plan, "anno_inference"),
        "FigurePlan phase annotations should be folded into module labels, not restored as floating text"
    );
    let teacher = object_box(&plan, "teacher_module");
    let student = object_box(&plan, "student_module");
    assert!(
        (teacher[0] - student[0]).abs() < 0.001 && (teacher[2] - student[2]).abs() < 0.001,
        "teacher and training student should be vertically aligned as the main learning spine: {teacher:?} {student:?}"
    );
    let inference = object_box(&plan, "inference_student");
    assert!(
        inference[0] > student[2] + 0.18
            && (center_y(inference) - center_y(student)).abs() < 0.08,
        "synthesized inference block should be a right-side continuation, not a bottom-center lane: {inference:?} {student:?}"
    );
    assert!(
        connector(&plan, "e_teacher_latent").label.is_none(),
        "residual formula should live inside the latent residual box, not as a floating edge label"
    );
    let task_loss = connector(&plan, "e_task_loss");
    assert!(
        task_loss.points.len() <= 3,
        "task loss feedback should be direct or 2-segment, not a long bottom loop: {:?}",
        task_loss.points
    );
    let student_latent = connector(&plan, "e_student_latent");
    let latent_loss = connector(&plan, "e_latent_loss");
    assert!(
        !connectors_cross(student_latent.points, latent_loss.points),
        "student latent edge and latent loss edge should not cross: {:?} {:?}",
        student_latent.points,
        latent_loss.points
    );
}

#[test]
fn draw_plan_geometry_repair_expands_tiny_separate_inference_input_regions() {
    let figure_plan = teacher_student_tiny_inference_regions_semantic_plan();
    let style = style_by_name(StyleName::WpsClean);
    let mut plan = draw_plan_from_figure_plan(&figure_plan, &style);

    repair_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    validate_draw_plan(&plan)
        .expect("repair should prevent zero-length connectors from overlapping inference regions");
    let inference_input = object_box(&plan, "input_inf");
    let inference_student = object_box(&plan, "student_inf");
    let training_student = object_box(&plan, "student");
    assert!(
        inference_input[1] >= 0.70
            && inference_input[3] <= 0.92
            && inference_student[0] > training_student[2] + 0.18
            && (center_y(inference_student) - center_y(training_student)).abs() < 0.08,
        "separate inference input should remain readable while inference student moves to the right continuation column: {inference_input:?} {inference_student:?}"
    );
    let inference_edge = connector(&plan, "e_input_inf");
    assert!(
        inference_edge.points.len() >= 2,
        "inference edge should keep a real route after orthogonalization: {:?}",
        inference_edge.points
    );
}

#[test]
fn draw_plan_geometry_repair_does_not_treat_task_loss_as_latent_pair() {
    let figure_plan = teacher_student_output_loss_supervision_semantic_plan();
    let style = style_by_name(StyleName::WpsClean);
    let mut plan = draw_plan_from_figure_plan(&figure_plan, &style);

    repair_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    validate_draw_plan(&plan).expect("task-loss supervision variant should remain valid");
    assert_eq!(
        box_text(&plan, "output_pred"),
        "ŷ",
        "output prediction must not be rewritten as h_T"
    );
    assert!(
        box_text(&plan, "loss_task").to_lowercase().contains("task"),
        "task loss node must remain a distinct Task Loss box"
    );
    let loss = object_box(&plan, "loss_task");
    let residual = object_box(&plan, "latent_resid");
    assert!(
        intersection_area(loss, residual) < 0.001,
        "task loss should not overlap latent residual: {loss:?} {residual:?}"
    );
    assert!(
        loss[0] <= 0.90 && loss[2] - loss[0] >= 0.10,
        "task loss should be readable and not compressed at the extreme right edge: {loss:?}"
    );
    let student_loss = connector(&plan, "e_student_taskloss");
    assert!(
        student_loss.points.len() == 2
            && (student_loss.points[0][1] - student_loss.points[1][1]).abs() < 0.0001,
        "student-to-task-loss route should be a single horizontal segment: {:?}",
        student_loss.points
    );
    assert!(
        !text_object_exists(&plan, "ann_inference"),
        "inference phase label should be folded into the inference module instead of restored as floating text"
    );
}

#[test]
fn draw_plan_geometry_repair_synthesizes_inference_block_from_annotation() {
    let figure_plan = teacher_student_no_inference_lane_annotation_plan();
    let style = style_by_name(StyleName::WpsClean);
    let mut plan = draw_plan_from_figure_plan(&figure_plan, &style);

    repair_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    validate_draw_plan(&plan).expect("no-inference-lane plan should remain valid");
    let teacher = object_box(&plan, "teacher_module");
    let student = object_box(&plan, "student_module");
    let inference = object_box(&plan, "inference_student");
    let loss = object_box(&plan, "task_loss");
    let input = object_box(&plan, "input_text");
    assert!(
        (teacher[0] - student[0]).abs() < 0.001 && (teacher[2] - student[2]).abs() < 0.001,
        "teacher and student should form a vertically aligned training spine: {teacher:?} {student:?}"
    );
    assert!(
        loss[2] - input[0] > 0.90,
        "teacher-student layout should use the paper-wide canvas: {input:?} {loss:?}"
    );
    assert!(
        !text_object_exists(&plan, "anno_inference"),
        "Inference annotation targeting the training student should become a real block, not a marginal note"
    );
    assert!(
        inference[0] > student[2] + 0.18
            && (center_y(inference) - center_y(student)).abs() < 0.08
            && box_text(&plan, "inference_student").contains("inference"),
        "inference-only student should be synthesized as a right-side continuation: {inference:?} {student:?}"
    );
    assert_eq!(
        connector_endpoints(&plan, "e_student_inference"),
        (Some("student_module"), Some("inference_student"))
    );
}

#[test]
fn draw_plan_preserves_missing_semantic_boxes_and_connectors_after_optimizer() {
    let previous = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.62, 0.72, 0.76, 0.84],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Connector {
            id: "e_student_to_loss".to_string(),
            points: vec![[0.52, 0.73], [0.62, 0.78]],
            from: Some("student".to_string()),
            to: Some("task_loss".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Text {
            id: "anno_inference".to_string(),
            bbox: [0.30, 0.88, 0.48, 0.94],
            text: "Inference only".to_string(),
            style: "annotation".to_string(),
            z: 40,
        },
    ]);
    let mut revised = minimal_draw_plan(vec![]);

    preserve_semantic_draw_objects(&previous, &mut revised);

    assert!(revised
        .objects
        .iter()
        .any(|object| matches!(object, DrawObject::Box { id, .. } if id == "task_loss")));
    assert!(revised.objects.iter().any(
        |object| matches!(object, DrawObject::Connector { id, .. } if id == "e_student_to_loss")
    ));
    assert!(
        !revised
            .objects
            .iter()
            .any(|object| matches!(object, DrawObject::Text { id, .. } if id == "anno_inference")),
        "marginal annotations may be intentionally removed"
    );
}

fn minimal_draw_plan(objects: Vec<DrawObject>) -> DrawPlan {
    DrawPlan {
        version: "0.2".to_string(),
        canvas: Canvas {
            aspect: CanvasAspect::PaperWide,
            target_width_mm: 85,
            safe_margin: 0.06,
        },
        style_tokens: BTreeMap::from([
            ("background".to_string(), "FFFFFF".to_string()),
            ("primary".to_string(), "2F6F9F".to_string()),
            ("accent".to_string(), "4B9A72".to_string()),
            ("text".to_string(), "1F2328".to_string()),
        ]),
        objects,
    }
}

fn teacher_student_semantic_plan() -> FigurePlan {
    serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher-student distillation with latent residual supervision.",
            "visual_focus": ["teacher_model", "student_model", "teacher_latent_residual"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "input", "bbox": [0.0, 0.33, 0.20, 0.66]},
                {"id": "teacher", "bbox": [0.22, 0.10, 0.42, 0.26]},
                {"id": "student", "bbox": [0.22, 0.42, 0.42, 0.58]},
                {"id": "output", "bbox": [0.68, 0.42, 0.78, 0.58]},
                {"id": "loss", "bbox": [0.84, 0.42, 0.95, 0.58]}
            ]
        },
        "components": [
            {"id": "task_input", "label": "x", "role": "input", "visual_weight": "normal", "region": "input", "allowed_asset_id": null},
            {"id": "teacher_model", "label": "Teacher", "role": "context", "visual_weight": "strong", "region": "teacher", "allowed_asset_id": null},
            {"id": "student_model", "label": "Student", "role": "main", "visual_weight": "strong", "region": "student", "allowed_asset_id": null},
            {"id": "task_output", "label": "ŷ", "role": "output", "visual_weight": "normal", "region": "output", "allowed_asset_id": null},
            {"id": "task_loss", "label": "Task Loss", "role": "loss", "visual_weight": "normal", "region": "loss", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "input_to_student", "from": "task_input", "to": "student_model", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "student_to_output", "from": "student_model", "to": "task_output", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "teacher_latent_residual", "from": "teacher_model", "to": "student_model", "label": "Latent Residual", "semantic": "supervision", "style": "dash", "importance": "main"},
            {"id": "output_to_loss", "from": "task_output", "to": "task_loss", "label": "", "semantic": "loss", "style": "solid", "importance": "normal"}
        ],
        "annotations": [
            {"id": "inference_note", "label": "Inference: student only", "target_id": "student_model", "bbox": [0.33, 0.86, 0.95, 1.0]}
        ],
        "assets": [],
        "design": {
            "style": "wps-clean",
            "max_colors": 3,
            "font_policy": "wps_friendly",
            "avoid_arrow_crossing": true,
            "prefer_native_shapes": true
        }
    }))
    .expect("semantic fixture should deserialize")
}

fn teacher_student_feedback_semantic_plan() -> FigurePlan {
    serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher-student distillation with latent residual supervision.",
            "visual_focus": ["teacher", "student", "latent_residual_edge", "task_loss_edge"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "teacher_region", "bbox": [0.041666666666666664, 0.08333333333333333, 0.3333333333333333, 0.8333333333333334]},
                {"id": "input_region", "bbox": [0.3333333333333333, 0.25, 0.7916666666666666, 0.75]},
                {"id": "student_region", "bbox": [0.5416666666666666, 0.08333333333333333, 1.0, 0.8333333333333334]},
                {"id": "output_region", "bbox": [0.8333333333333334, 0.25, 1.0, 0.75]},
                {"id": "legend_region", "bbox": [0.041666666666666664, 0.8333333333333334, 1.0, 1.0]}
            ]
        },
        "components": [
            {"id": "teacher", "label": "Teacher\n(Large LM)", "role": "context", "visual_weight": "strong", "region": "teacher_region", "allowed_asset_id": null},
            {"id": "input_data", "label": "Input x", "role": "input", "visual_weight": "normal", "region": "input_region", "allowed_asset_id": null},
            {"id": "student", "label": "Student\n(Compact)", "role": "main", "visual_weight": "strong", "region": "student_region", "allowed_asset_id": null},
            {"id": "output_pred", "label": "ŷ", "role": "output", "visual_weight": "normal", "region": "output_region", "allowed_asset_id": null},
            {"id": "task_loss", "label": "Task Loss", "role": "loss", "visual_weight": "normal", "region": "legend_region", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "e_input_student", "from": "input_data", "to": "student", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_teacher_residual", "from": "teacher", "to": "student", "label": "Latent Residual", "semantic": "supervision", "style": "dash", "importance": "main"},
            {"id": "e_student_output", "from": "student", "to": "output_pred", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_task_loss", "from": "output_pred", "to": "student", "label": "Task Loss", "semantic": "loss", "style": "solid", "importance": "normal"}
        ],
        "annotations": [
            {"id": "anno_inference", "label": "Inference: student only", "target_id": "student", "bbox": [0.5416666666666666, 0.8333333333333334, 1.0, 1.0]},
            {"id": "anno_training", "label": "Training", "target_id": "teacher", "bbox": [0.041666666666666664, 0.8333333333333334, 0.3333333333333333, 1.0]}
        ],
        "assets": [],
        "design": {
            "style": "wps-clean",
            "max_colors": 3,
            "font_policy": "wps_friendly",
            "avoid_arrow_crossing": true,
            "prefer_native_shapes": true
        }
    }))
    .expect("feedback semantic fixture should deserialize")
}

fn teacher_student_latent_pair_semantic_plan() -> FigurePlan {
    serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher-student distillation with latent residual supervision.",
            "visual_focus": ["teacher", "student", "latent_teacher", "latent_student", "e_residual"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "task_input", "bbox": [0.0, 0.3333333333333333, 0.20833333333333334, 0.6666666666666666]},
                {"id": "teacher_region", "bbox": [0.20833333333333334, 0.16666666666666666, 0.4583333333333333, 0.3333333333333333]},
                {"id": "student_region", "bbox": [0.20833333333333334, 0.5, 0.4583333333333333, 0.6666666666666666]},
                {"id": "latent_space", "bbox": [0.4583333333333333, 0.16666666666666666, 0.7083333333333334, 0.6666666666666666]},
                {"id": "task_output", "bbox": [0.75, 0.4166666666666667, 0.9166666666666666, 0.5833333333333334]},
                {"id": "inference_label", "bbox": [0.58, 0.72, 0.92, 0.84]}
            ]
        },
        "components": [
            {"id": "teacher", "label": "Teacher LM", "role": "main", "visual_weight": "normal", "region": "teacher_region", "allowed_asset_id": null},
            {"id": "student", "label": "Student", "role": "main", "visual_weight": "strong", "region": "student_region", "allowed_asset_id": null},
            {"id": "task_input_node", "label": "Input x", "role": "input", "visual_weight": "normal", "region": "task_input", "allowed_asset_id": null},
            {"id": "task_output_node", "label": "ŷ", "role": "output", "visual_weight": "normal", "region": "task_output", "allowed_asset_id": null},
            {"id": "latent_teacher", "label": "h_T", "role": "module", "visual_weight": "muted", "region": "latent_space", "allowed_asset_id": null},
            {"id": "latent_student", "label": "h_S", "role": "module", "visual_weight": "muted", "region": "latent_space", "allowed_asset_id": null},
            {"id": "inference_badge", "label": "Inference: Student only", "role": "context", "visual_weight": "normal", "region": "inference_label", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "e_input_student", "from": "task_input_node", "to": "student", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_input_teacher", "from": "task_input_node", "to": "teacher", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_teacher_latent", "from": "teacher", "to": "latent_teacher", "label": "latent", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_student_latent", "from": "student", "to": "latent_student", "label": "latent", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_residual", "from": "latent_teacher", "to": "latent_student", "label": "r = h_T - h_S", "semantic": "supervision", "style": "dash", "importance": "main"},
            {"id": "e_student_output", "from": "student", "to": "task_output_node", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_task_loss", "from": "task_output_node", "to": "student", "label": "L_task", "semantic": "loss", "style": "solid", "importance": "main"}
        ],
        "annotations": [],
        "assets": [],
        "design": {
            "style": "wps-clean",
            "max_colors": 3,
            "font_policy": "wps_friendly",
            "avoid_arrow_crossing": true,
            "prefer_native_shapes": true
        }
    }))
    .expect("latent pair semantic fixture should deserialize")
}

fn teacher_student_explicit_inference_semantic_plan() -> FigurePlan {
    serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher-student distillation with explicit inference lane.",
            "visual_focus": ["comp_teacher", "comp_latent", "comp_student_train", "comp_student_inf"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "input", "bbox": [0.0, 0.33, 0.20, 0.66]},
                {"id": "teacher", "bbox": [0.20, 0.10, 0.45, 0.28]},
                {"id": "latent_residual", "bbox": [0.48, 0.22, 0.70, 0.42]},
                {"id": "student_train", "bbox": [0.20, 0.42, 0.45, 0.60]},
                {"id": "task_loss", "bbox": [0.74, 0.58, 0.92, 0.76]},
                {"id": "inference", "bbox": [0.58, 0.36, 0.78, 0.56]},
                {"id": "answer_out", "bbox": [0.80, 0.40, 0.92, 0.56]}
            ]
        },
        "components": [
            {"id": "comp_input", "label": "Input x", "role": "input", "visual_weight": "normal", "region": "input", "allowed_asset_id": null},
            {"id": "comp_teacher", "label": "Teacher (frozen)", "role": "context", "visual_weight": "strong", "region": "teacher", "allowed_asset_id": null},
            {"id": "comp_latent", "label": "Latent residual r", "role": "loss", "visual_weight": "normal", "region": "latent_residual", "allowed_asset_id": null},
            {"id": "comp_student_train", "label": "Student (training)", "role": "main", "visual_weight": "strong", "region": "student_train", "allowed_asset_id": null},
            {"id": "comp_task_loss", "label": "Task loss L_task", "role": "loss", "visual_weight": "normal", "region": "task_loss", "allowed_asset_id": null},
            {"id": "comp_inference_label", "label": "Inference", "role": "main", "visual_weight": "strong", "region": "inference", "allowed_asset_id": null},
            {"id": "comp_student_inf", "label": "Student", "role": "main", "visual_weight": "strong", "region": "inference", "allowed_asset_id": null},
            {"id": "comp_answer", "label": "ŷ", "role": "output", "visual_weight": "normal", "region": "answer_out", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "edge_input_teacher", "from": "comp_input", "to": "comp_teacher", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "edge_teacher_latent", "from": "comp_teacher", "to": "comp_latent", "label": "h_t - h_s", "semantic": "supervision", "style": "solid", "importance": "normal"},
            {"id": "edge_latent_student", "from": "comp_latent", "to": "comp_student_train", "label": "", "semantic": "supervision", "style": "dash", "importance": "main"},
            {"id": "edge_input_student", "from": "comp_input", "to": "comp_student_train", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "edge_student_loss", "from": "comp_student_train", "to": "comp_task_loss", "label": "ŷ, y", "semantic": "loss", "style": "solid", "importance": "normal"},
            {"id": "edge_input_inf", "from": "comp_input", "to": "comp_student_inf", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "edge_inf_answer", "from": "comp_student_inf", "to": "comp_answer", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"}
        ],
        "annotations": [],
        "assets": [],
        "design": {
            "style": "wps-clean",
            "max_colors": 3,
            "font_policy": "wps_friendly",
            "avoid_arrow_crossing": true,
            "prefer_native_shapes": true
        }
    }))
    .expect("explicit inference semantic fixture should deserialize")
}

fn teacher_student_margin_variant_semantic_plan() -> FigurePlan {
    serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher-student distillation with training and inference phases.",
            "visual_focus": ["student_train", "student_infer", "latent_residual"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "input", "bbox": [0.0, 0.33, 0.20, 0.66]},
                {"id": "student", "bbox": [0.20, 0.42, 0.45, 0.60]},
                {"id": "teacher_latent", "bbox": [0.20, 0.10, 0.70, 0.42]},
                {"id": "task_loss", "bbox": [0.72, 0.42, 0.95, 0.60]},
                {"id": "residual_loss", "bbox": [0.62, 0.42, 0.90, 0.60]},
                {"id": "inference_student", "bbox": [0.20, 0.70, 0.50, 0.90]},
                {"id": "answer_train", "bbox": [0.74, 0.42, 0.90, 0.60]},
                {"id": "answer_infer", "bbox": [0.58, 0.70, 0.78, 0.90]}
            ]
        },
        "components": [
            {"id": "input_text", "label": "Input x", "role": "input", "visual_weight": "normal", "region": "input", "allowed_asset_id": null},
            {"id": "student_train", "label": "Student\n(Train)", "role": "main", "visual_weight": "strong", "region": "student", "allowed_asset_id": null},
            {"id": "teacher", "label": "Teacher\n(Large LM)", "role": "context", "visual_weight": "normal", "region": "teacher_latent", "allowed_asset_id": null},
            {"id": "latent_residual", "label": "Latent residual r", "role": "module", "visual_weight": "normal", "region": "teacher_latent", "allowed_asset_id": null},
            {"id": "task_label", "label": "Task label y", "role": "loss", "visual_weight": "normal", "region": "task_loss", "allowed_asset_id": null},
            {"id": "residual_supervision", "label": "Residual loss", "role": "loss", "visual_weight": "normal", "region": "residual_loss", "allowed_asset_id": null},
            {"id": "student_infer", "label": "Student\n(Inference)", "role": "main", "visual_weight": "strong", "region": "inference_student", "allowed_asset_id": null},
            {"id": "answer_train_out", "label": "ŷ", "role": "output", "visual_weight": "normal", "region": "answer_train", "allowed_asset_id": null},
            {"id": "answer_infer_out", "label": "ŷ", "role": "output", "visual_weight": "normal", "region": "answer_infer", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "e_input_student", "from": "input_text", "to": "student_train", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_student_answer", "from": "student_train", "to": "answer_train_out", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_teacher_residual", "from": "teacher", "to": "latent_residual", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_residual_student", "from": "latent_residual", "to": "student_train", "label": "", "semantic": "supervision", "style": "dash", "importance": "main"},
            {"id": "e_label_loss", "from": "task_label", "to": "student_train", "label": "", "semantic": "supervision", "style": "solid", "importance": "main"},
            {"id": "e_residual_loss", "from": "residual_supervision", "to": "student_train", "label": "", "semantic": "loss", "style": "dash", "importance": "normal"},
            {"id": "e_input_infer", "from": "input_text", "to": "student_infer", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_infer_answer", "from": "student_infer", "to": "answer_infer_out", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"}
        ],
        "annotations": [],
        "assets": [],
        "design": {
            "style": "wps-clean",
            "max_colors": 3,
            "font_policy": "wps_friendly",
            "avoid_arrow_crossing": true,
            "prefer_native_shapes": true
        }
    }))
    .expect("margin variant semantic fixture should deserialize")
}

fn teacher_student_quality_gate_semantic_plan() -> FigurePlan {
    serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher-student distillation with latent residual supervision.",
            "visual_focus": ["teacher_latent", "task_loss"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "input_region", "bbox": [0.0, 0.33, 0.20, 0.66]},
                {"id": "teacher_region", "bbox": [0.20, 0.10, 0.45, 0.28]},
                {"id": "student_train_region", "bbox": [0.20, 0.42, 0.45, 0.60]},
                {"id": "residual_region", "bbox": [0.50, 0.28, 0.78, 0.48]},
                {"id": "task_loss_region", "bbox": [0.88, 0.42, 1.0, 0.60]},
                {"id": "output_train_region", "bbox": [0.72, 0.42, 0.88, 0.60]}
            ]
        },
        "components": [
            {"id": "input", "label": "Input x", "role": "input", "visual_weight": "normal", "region": "input_region", "allowed_asset_id": null},
            {"id": "teacher", "label": "Teacher\n(large LM)", "role": "context", "visual_weight": "normal", "region": "teacher_region", "allowed_asset_id": null},
            {"id": "student_train", "label": "Student\n(compact)", "role": "main", "visual_weight": "strong", "region": "student_train_region", "allowed_asset_id": null},
            {"id": "teacher_latent", "label": "Latent h_T", "role": "context", "visual_weight": "normal", "region": "teacher_region", "allowed_asset_id": null},
            {"id": "residual", "label": "Residual\nh_T - h_S", "role": "loss", "visual_weight": "normal", "region": "residual_region", "allowed_asset_id": null},
            {"id": "task_loss", "label": "Task Loss", "role": "loss", "visual_weight": "normal", "region": "task_loss_region", "allowed_asset_id": null},
            {"id": "output_train", "label": "ŷ", "role": "output", "visual_weight": "normal", "region": "output_train_region", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "e_input_teacher", "from": "input", "to": "teacher", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_input_student", "from": "input", "to": "student_train", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_teacher_latent", "from": "teacher", "to": "teacher_latent", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_latent_residual", "from": "teacher_latent", "to": "residual", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_student_latent", "from": "student_train", "to": "residual", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_residual_student", "from": "residual", "to": "student_train", "label": "supervision", "semantic": "supervision", "style": "dash", "importance": "main"},
            {"id": "e_student_output", "from": "student_train", "to": "output_train", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_task_loss", "from": "output_train", "to": "task_loss", "label": "", "semantic": "loss", "style": "solid", "importance": "normal"}
        ],
        "annotations": [],
        "assets": [],
        "design": {
            "style": "wps-clean",
            "max_colors": 3,
            "font_policy": "wps_friendly",
            "avoid_arrow_crossing": true,
            "prefer_native_shapes": true
        }
    }))
    .expect("quality gate semantic fixture should deserialize")
}

fn teacher_student_latest_residual_feedback_semantic_plan() -> FigurePlan {
    serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher-student distillation with latent residual supervision.",
            "visual_focus": ["teacher_module", "student_module", "latent_residual"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "input_region", "bbox": [0.0, 0.33, 0.20, 0.66]},
                {"id": "teacher_region", "bbox": [0.20, 0.10, 0.45, 0.28]},
                {"id": "student_region", "bbox": [0.20, 0.42, 0.45, 0.60]},
                {"id": "latent_residual_region", "bbox": [0.50, 0.28, 0.78, 0.48]},
                {"id": "task_loss_region", "bbox": [0.88, 0.42, 1.0, 0.60]},
                {"id": "output_region", "bbox": [0.72, 0.42, 0.88, 0.60]},
                {"id": "inference_region", "bbox": [0.20, 0.70, 0.50, 0.90]}
            ]
        },
        "components": [
            {"id": "input_text", "label": "Input x", "role": "input", "visual_weight": "normal", "region": "input_region", "allowed_asset_id": null},
            {"id": "teacher_module", "label": "Teacher\n(Large LM)", "role": "main", "visual_weight": "strong", "region": "teacher_region", "allowed_asset_id": null},
            {"id": "student_module", "label": "Student\n(Compact)", "role": "main", "visual_weight": "strong", "region": "student_region", "allowed_asset_id": null},
            {"id": "latent_residual", "label": "Latent\nResidual", "role": "loss", "visual_weight": "normal", "region": "latent_residual_region", "allowed_asset_id": null},
            {"id": "task_loss", "label": "Task\nLoss", "role": "loss", "visual_weight": "normal", "region": "task_loss_region", "allowed_asset_id": null},
            {"id": "output_pred", "label": "ŷ", "role": "output", "visual_weight": "normal", "region": "output_region", "allowed_asset_id": null},
            {"id": "inference_student", "label": "Student Only", "role": "output", "visual_weight": "strong", "region": "inference_region", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "e_input_teacher", "from": "input_text", "to": "teacher_module", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_input_student", "from": "input_text", "to": "student_module", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_teacher_latent", "from": "teacher_module", "to": "latent_residual", "label": "z_t", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_student_latent", "from": "student_module", "to": "latent_residual", "label": "z_s", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_latent_loss", "from": "latent_residual", "to": "student_module", "label": "", "semantic": "supervision", "style": "dash", "importance": "main"},
            {"id": "e_task_loss", "from": "task_loss", "to": "student_module", "label": "", "semantic": "supervision", "style": "solid", "importance": "main"},
            {"id": "e_student_output", "from": "student_module", "to": "output_pred", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_inference_input", "from": "input_text", "to": "inference_student", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"}
        ],
        "annotations": [
            {"id": "anno_training", "label": "Training", "target_id": "teacher_module", "bbox": [0.22, 0.04, 0.54, 0.09]},
            {"id": "anno_inference", "label": "Inference", "target_id": "inference_student", "bbox": [0.22, 0.68, 0.54, 0.73]}
        ],
        "assets": [],
        "design": {
            "style": "wps-clean",
            "max_colors": 3,
            "font_policy": "wps_friendly",
            "avoid_arrow_crossing": true,
            "prefer_native_shapes": true
        }
    }))
    .expect("latest residual feedback semantic fixture should deserialize")
}

fn teacher_student_tiny_inference_regions_semantic_plan() -> FigurePlan {
    serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher-student distillation with explicit train and inference paths.",
            "visual_focus": ["teacher", "student", "student_inf", "residual"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "teacher_box", "bbox": [0.0, 0.0, 0.3125, 0.6666666666666666]},
                {"id": "student_train", "bbox": [0.4375, 0.16666666666666666, 0.75, 1.0]},
                {"id": "task_input", "bbox": [0.4375, 0.0, 0.75, 0.26666666666666666]},
                {"id": "latent_residual", "bbox": [0.1875, 0.5, 0.5, 0.9]},
                {"id": "task_loss", "bbox": [0.5625, 0.94, 0.8125, 1.0]},
                {"id": "answer_out", "bbox": [0.8125, 0.39999999999999997, 1.0, 0.9]},
                {"id": "inference_student", "bbox": [0.4375, 0.94, 0.75, 1.0]},
                {"id": "inference_input", "bbox": [0.4375, 0.94, 0.75, 1.0]},
                {"id": "inference_output", "bbox": [0.8125, 0.94, 1.0, 1.0]},
                {"id": "train_label", "bbox": [0.0, 0.94, 0.25, 1.0]},
                {"id": "inference_label", "bbox": [0.0, 0.94, 0.25, 1.0]},
                {"id": "divider", "bbox": [0.0, 0.94, 1.0, 1.0]}
            ]
        },
        "components": [
            {"id": "teacher", "label": "Teacher LM", "role": "context", "visual_weight": "normal", "region": "teacher_box", "allowed_asset_id": null},
            {"id": "student", "label": "Student", "role": "main", "visual_weight": "strong", "region": "student_train", "allowed_asset_id": null},
            {"id": "input", "label": "Task input x", "role": "input", "visual_weight": "normal", "region": "task_input", "allowed_asset_id": null},
            {"id": "residual", "label": "Latent residual", "role": "loss", "visual_weight": "strong", "region": "latent_residual", "allowed_asset_id": null},
            {"id": "loss", "label": "Task loss", "role": "loss", "visual_weight": "normal", "region": "task_loss", "allowed_asset_id": null},
            {"id": "answer", "label": "ŷ", "role": "output", "visual_weight": "normal", "region": "answer_out", "allowed_asset_id": null},
            {"id": "student_inf", "label": "Student", "role": "main", "visual_weight": "strong", "region": "inference_student", "allowed_asset_id": null},
            {"id": "input_inf", "label": "Task input x", "role": "input", "visual_weight": "normal", "region": "inference_input", "allowed_asset_id": null},
            {"id": "output_inf", "label": "ŷ", "role": "output", "visual_weight": "normal", "region": "inference_output", "allowed_asset_id": null},
            {"id": "train_tag", "label": "Training", "role": "context", "visual_weight": "muted", "region": "train_label", "allowed_asset_id": null},
            {"id": "inference_tag", "label": "Inference", "role": "context", "visual_weight": "muted", "region": "inference_label", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "e_input_student", "from": "input", "to": "student", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_input_teacher", "from": "input", "to": "teacher", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_teacher_residual", "from": "teacher", "to": "residual", "label": "", "semantic": "supervision", "style": "solid", "importance": "normal"},
            {"id": "e_residual_student", "from": "residual", "to": "student", "label": "", "semantic": "supervision", "style": "dash", "importance": "main"},
            {"id": "e_student_answer", "from": "student", "to": "answer", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_student_loss", "from": "student", "to": "loss", "label": "", "semantic": "loss", "style": "solid", "importance": "normal"},
            {"id": "e_input_inf", "from": "input_inf", "to": "student_inf", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_student_inf_out", "from": "student_inf", "to": "output_inf", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"}
        ],
        "annotations": [],
        "assets": [],
        "design": {
            "style": "wps-clean",
            "max_colors": 3,
            "font_policy": "wps_friendly",
            "avoid_arrow_crossing": true,
            "prefer_native_shapes": true
        }
    }))
    .expect("tiny inference regions semantic fixture should deserialize")
}

fn teacher_student_output_loss_supervision_semantic_plan() -> FigurePlan {
    serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher-student distillation with task and latent supervision.",
            "visual_focus": ["student_model", "output_pred", "loss_task", "latent_resid"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "input", "bbox": [0.0, 0.33, 0.20, 0.66]},
                {"id": "student", "bbox": [0.20, 0.42, 0.45, 0.60]},
                {"id": "task_output", "bbox": [0.72, 0.42, 0.88, 0.60]},
                {"id": "task_loss", "bbox": [0.88, 0.42, 1.0, 0.60]},
                {"id": "teacher", "bbox": [0.20, 0.10, 0.45, 0.28]},
                {"id": "latent_residual", "bbox": [0.50, 0.28, 0.78, 0.48]},
                {"id": "inference_student", "bbox": [0.20, 0.70, 0.50, 0.90]}
            ]
        },
        "components": [
            {"id": "input_text", "label": "Input x", "role": "input", "visual_weight": "normal", "region": "input", "allowed_asset_id": null},
            {"id": "student_model", "label": "Student", "role": "main", "visual_weight": "strong", "region": "student", "allowed_asset_id": null},
            {"id": "output_pred", "label": "ŷ", "role": "output", "visual_weight": "normal", "region": "task_output", "allowed_asset_id": null},
            {"id": "loss_task", "label": "Task Loss", "role": "loss", "visual_weight": "normal", "region": "task_loss", "allowed_asset_id": null},
            {"id": "teacher_model", "label": "Teacher", "role": "context", "visual_weight": "normal", "region": "teacher", "allowed_asset_id": null},
            {"id": "latent_resid", "label": "Latent Residual", "role": "loss", "visual_weight": "strong", "region": "latent_residual", "allowed_asset_id": null},
            {"id": "inference_student_model", "label": "Student", "role": "main", "visual_weight": "strong", "region": "inference_student", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "e_input_student", "from": "input_text", "to": "student_model", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_student_output", "from": "student_model", "to": "output_pred", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_output_taskloss", "from": "output_pred", "to": "loss_task", "label": "", "semantic": "supervision", "style": "solid", "importance": "normal"},
            {"id": "e_student_taskloss", "from": "student_model", "to": "loss_task", "label": "", "semantic": "supervision", "style": "solid", "importance": "normal"},
            {"id": "e_input_teacher", "from": "input_text", "to": "teacher_model", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_teacher_latent", "from": "teacher_model", "to": "latent_resid", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_latent_student", "from": "latent_resid", "to": "student_model", "label": "", "semantic": "supervision", "style": "dash", "importance": "main"},
            {"id": "e_input_inference", "from": "input_text", "to": "inference_student_model", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"}
        ],
        "annotations": [
            {"id": "ann_training", "label": "Training", "target_id": null, "bbox": [0.1, 0.06666666666666667, 0.7, 0.2333333333333333]},
            {"id": "ann_inference", "label": "Inference", "target_id": null, "bbox": [0.8, 0.06666666666666667, 1.0, 0.2333333333333333]},
            {"id": "ann_only_student", "label": "Only Student", "target_id": "inference_student_model", "bbox": [0.8, 0.94, 1.0, 1.0]}
        ],
        "assets": [],
        "design": {
            "style": "wps-clean",
            "max_colors": 3,
            "font_policy": "wps_friendly",
            "avoid_arrow_crossing": true,
            "prefer_native_shapes": true
        }
    }))
    .expect("output-loss supervision semantic fixture should deserialize")
}

fn teacher_student_no_inference_lane_annotation_plan() -> FigurePlan {
    serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher-student distillation with training and shared inference note.",
            "visual_focus": ["teacher_module", "student_module", "latent_residual", "task_loss"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "input_region", "bbox": [0.0, 0.33, 0.20, 0.66]},
                {"id": "teacher_region", "bbox": [0.20, 0.10, 0.45, 0.28]},
                {"id": "student_region", "bbox": [0.20, 0.42, 0.45, 0.60]},
                {"id": "latent_residual_region", "bbox": [0.50, 0.28, 0.78, 0.48]},
                {"id": "task_loss_region", "bbox": [0.88, 0.42, 1.0, 0.60]},
                {"id": "output_region", "bbox": [0.72, 0.42, 0.88, 0.60]}
            ]
        },
        "components": [
            {"id": "input_text", "label": "Input x", "role": "input", "visual_weight": "muted", "region": "input_region", "allowed_asset_id": null},
            {"id": "teacher_module", "label": "Teacher\n(Large LM)", "role": "main", "visual_weight": "strong", "region": "teacher_region", "allowed_asset_id": null},
            {"id": "student_module", "label": "Student\n(Compact)", "role": "main", "visual_weight": "strong", "region": "student_region", "allowed_asset_id": null},
            {"id": "latent_residual", "label": "Latent Residual", "role": "loss", "visual_weight": "normal", "region": "latent_residual_region", "allowed_asset_id": null},
            {"id": "task_loss", "label": "Task Loss", "role": "loss", "visual_weight": "normal", "region": "task_loss_region", "allowed_asset_id": null},
            {"id": "output_pred", "label": "ŷ", "role": "output", "visual_weight": "normal", "region": "output_region", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "e_input_to_teacher", "from": "input_text", "to": "teacher_module", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_input_to_student", "from": "input_text", "to": "student_module", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_teacher_to_latent", "from": "teacher_module", "to": "latent_residual", "label": "", "semantic": "supervision", "style": "dash", "importance": "main"},
            {"id": "e_latent_to_student", "from": "latent_residual", "to": "student_module", "label": "", "semantic": "supervision", "style": "dash", "importance": "main"},
            {"id": "e_student_to_output", "from": "student_module", "to": "output_pred", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_student_to_task_loss", "from": "student_module", "to": "task_loss", "label": "", "semantic": "loss", "style": "solid", "importance": "normal"}
        ],
        "annotations": [
            {"id": "anno_training", "label": "Training", "target_id": "teacher_module", "bbox": [0.2916666666666667, 0.03333333333333333, 0.75, 0.11666666666666665]},
            {"id": "anno_inference", "label": "Inference only", "target_id": "student_module", "bbox": [0.2916666666666667, 0.94, 0.75, 1.0]}
        ],
        "assets": [],
        "design": {
            "style": "wps-clean",
            "max_colors": 3,
            "font_policy": "wps_friendly",
            "avoid_arrow_crossing": true,
            "prefer_native_shapes": true
        }
    }))
    .expect("no inference lane annotation fixture should deserialize")
}

fn object_box(plan: &DrawPlan, id: &str) -> [f64; 4] {
    let object = plan
        .objects
        .iter()
        .find(|object| matches!(object, DrawObject::Box { id: object_id, .. } if object_id == id))
        .unwrap_or_else(|| panic!("box {id} exists"));
    match object {
        DrawObject::Box { bbox, .. } => *bbox,
        _ => unreachable!(),
    }
}

fn text_box(plan: &DrawPlan, id: &str) -> [f64; 4] {
    let object = plan
        .objects
        .iter()
        .find(|object| matches!(object, DrawObject::Text { id: object_id, .. } if object_id == id))
        .unwrap_or_else(|| panic!("text {id} exists"));
    match object {
        DrawObject::Text { bbox, .. } => *bbox,
        _ => unreachable!(),
    }
}

fn box_text<'a>(plan: &'a DrawPlan, id: &str) -> &'a str {
    let object = plan
        .objects
        .iter()
        .find(|object| matches!(object, DrawObject::Box { id: object_id, .. } if object_id == id))
        .unwrap_or_else(|| panic!("box {id} exists"));
    match object {
        DrawObject::Box { text, .. } => text,
        _ => unreachable!(),
    }
}

fn box_style<'a>(plan: &'a DrawPlan, id: &str) -> &'a str {
    let object = plan
        .objects
        .iter()
        .find(|object| matches!(object, DrawObject::Box { id: object_id, .. } if object_id == id))
        .unwrap_or_else(|| panic!("box {id} exists"));
    match object {
        DrawObject::Box { style, .. } => style,
        _ => unreachable!(),
    }
}

fn input_teacher_style<'a>(plan: &'a DrawPlan, id: &str) -> &'a str {
    let object = plan
        .objects
        .iter()
        .find(|object| matches!(object, DrawObject::Connector { id: object_id, .. } if object_id == id))
        .unwrap_or_else(|| panic!("connector {id} exists"));
    match object {
        DrawObject::Connector { style, .. } => style,
        _ => unreachable!(),
    }
}

fn center_x(bbox: [f64; 4]) -> f64 {
    (bbox[0] + bbox[2]) / 2.0
}

fn center_y(bbox: [f64; 4]) -> f64 {
    (bbox[1] + bbox[3]) / 2.0
}

fn box_touches_test_margin(bbox: [f64; 4]) -> bool {
    bbox[0] < 0.08 || bbox[1] < 0.08 || bbox[2] > 0.92 || bbox[3] > 0.92
}

struct ConnectorView<'a> {
    points: &'a [[f64; 2]],
    label: &'a Option<DrawLabel>,
}

fn connector<'a>(plan: &'a DrawPlan, id: &str) -> ConnectorView<'a> {
    let object = plan
        .objects
        .iter()
        .find(|object| matches!(object, DrawObject::Connector { id: object_id, .. } if object_id == id))
        .unwrap_or_else(|| panic!("connector {id} exists"));
    match object {
        DrawObject::Connector { points, label, .. } => ConnectorView { points, label },
        _ => unreachable!(),
    }
}

fn connector_endpoints<'a>(plan: &'a DrawPlan, id: &str) -> (Option<&'a str>, Option<&'a str>) {
    let object = plan
        .objects
        .iter()
        .find(|object| matches!(object, DrawObject::Connector { id: object_id, .. } if object_id == id))
        .unwrap_or_else(|| panic!("connector {id} exists"));
    match object {
        DrawObject::Connector { from, to, .. } => (from.as_deref(), to.as_deref()),
        _ => unreachable!(),
    }
}

fn component_overlap_gate_fails(left: [f64; 4], right: [f64; 4]) -> bool {
    let overlap = intersection_area(left, right);
    if overlap <= 0.0 {
        return false;
    }
    let left_area = box_area(left);
    let right_area = box_area(right);
    overlap > 0.003 && overlap / left_area.min(right_area).max(0.0001) > 0.15
}

fn anchor_towards(bbox: [f64; 4], target: [f64; 4]) -> [f64; 2] {
    let center = [(bbox[0] + bbox[2]) / 2.0, (bbox[1] + bbox[3]) / 2.0];
    let target_center = [(target[0] + target[2]) / 2.0, (target[1] + target[3]) / 2.0];
    let dx = target_center[0] - center[0];
    let dy = target_center[1] - center[1];
    if dx.abs() >= dy.abs() {
        let x = if dx >= 0.0 { bbox[2] } else { bbox[0] };
        [x, center[1]]
    } else {
        let y = if dy >= 0.0 { bbox[3] } else { bbox[1] };
        [center[0], y]
    }
}

fn text_object_exists(plan: &DrawPlan, id: &str) -> bool {
    plan.objects
        .iter()
        .any(|object| matches!(object, DrawObject::Text { id: object_id, .. } if object_id == id))
}

fn draw_plan_has_text(plan: &DrawPlan, id: &str, text_fragment: &str) -> bool {
    let text_fragment = text_fragment.to_lowercase();
    plan.objects.iter().any(|object| {
        matches!(
            object,
            DrawObject::Text { id: object_id, text, .. }
                if object_id == id && text.to_lowercase().contains(&text_fragment)
        )
    })
}

fn box_object_exists(plan: &DrawPlan, id: &str) -> bool {
    plan.objects
        .iter()
        .any(|object| matches!(object, DrawObject::Box { id: object_id, .. } if object_id == id))
}

fn connector_object_exists(plan: &DrawPlan, id: &str) -> bool {
    plan.objects.iter().any(
        |object| matches!(object, DrawObject::Connector { id: object_id, .. } if object_id == id),
    )
}

fn label_intersects_any_segment(label_bbox: [f64; 4], points: &[[f64; 2]]) -> bool {
    points.windows(2).any(|window| {
        let bbox = expand_box(
            [
                window[0][0].min(window[1][0]),
                window[0][1].min(window[1][1]),
                window[0][0].max(window[1][0]),
                window[0][1].max(window[1][1]),
            ],
            0.006,
        );
        intersection_area(label_bbox, bbox) > 0.001
    })
}

fn connectors_cross(left: &[[f64; 2]], right: &[[f64; 2]]) -> bool {
    left.windows(2).any(|left_window| {
        right.windows(2).any(|right_window| {
            segments_cross(
                (left_window[0], left_window[1]),
                (right_window[0], right_window[1]),
            )
        })
    })
}

fn connectors_share_reversed_segment(left: &[[f64; 2]], right: &[[f64; 2]]) -> bool {
    left.windows(2).any(|left_window| {
        right.windows(2).any(|right_window| {
            reversed_collinear_overlap(
                left_window[0],
                left_window[1],
                right_window[0],
                right_window[1],
            )
        })
    })
}

fn reversed_collinear_overlap(
    left_start: [f64; 2],
    left_end: [f64; 2],
    right_start: [f64; 2],
    right_end: [f64; 2],
) -> bool {
    if (left_start[1] - left_end[1]).abs() < 0.0001
        && (right_start[1] - right_end[1]).abs() < 0.0001
        && (left_start[1] - right_start[1]).abs() < 0.0001
    {
        let left_dir = (left_end[0] - left_start[0]).signum();
        let right_dir = (right_end[0] - right_start[0]).signum();
        let overlap_start = left_start[0]
            .min(left_end[0])
            .max(right_start[0].min(right_end[0]));
        let overlap_end = left_start[0]
            .max(left_end[0])
            .min(right_start[0].max(right_end[0]));
        return left_dir * right_dir < 0.0 && overlap_end - overlap_start > 0.01;
    }
    if (left_start[0] - left_end[0]).abs() < 0.0001
        && (right_start[0] - right_end[0]).abs() < 0.0001
        && (left_start[0] - right_start[0]).abs() < 0.0001
    {
        let left_dir = (left_end[1] - left_start[1]).signum();
        let right_dir = (right_end[1] - right_start[1]).signum();
        let overlap_start = left_start[1]
            .min(left_end[1])
            .max(right_start[1].min(right_end[1]));
        let overlap_end = left_start[1]
            .max(left_end[1])
            .min(right_start[1].max(right_end[1]));
        return left_dir * right_dir < 0.0 && overlap_end - overlap_start > 0.01;
    }
    false
}

fn polyline_length(points: &[[f64; 2]]) -> f64 {
    points
        .windows(2)
        .map(|window| segment_length((window[0], window[1])))
        .sum()
}

fn draw_object_id(object: &DrawObject) -> &str {
    match object {
        DrawObject::Box { id, .. }
        | DrawObject::Text { id, .. }
        | DrawObject::Connector { id, .. }
        | DrawObject::Image { id, .. }
        | DrawObject::Group { id, .. } => id,
    }
}

fn expand_box(bbox: [f64; 4], margin: f64) -> [f64; 4] {
    [
        (bbox[0] - margin).clamp(0.0, 1.0),
        (bbox[1] - margin).clamp(0.0, 1.0),
        (bbox[2] + margin).clamp(0.0, 1.0),
        (bbox[3] + margin).clamp(0.0, 1.0),
    ]
}

fn intersection_area(a: [f64; 4], b: [f64; 4]) -> f64 {
    let x1 = a[0].max(b[0]);
    let y1 = a[1].max(b[1]);
    let x2 = a[2].min(b[2]);
    let y2 = a[3].min(b[3]);
    ((x2 - x1).max(0.0)) * ((y2 - y1).max(0.0))
}

fn box_area(bbox: [f64; 4]) -> f64 {
    (bbox[2] - bbox[0]).max(0.0) * (bbox[3] - bbox[1]).max(0.0)
}

fn segments_cross(a: ([f64; 2], [f64; 2]), b: ([f64; 2], [f64; 2])) -> bool {
    if segment_length(a) < 0.04 || segment_length(b) < 0.04 {
        return false;
    }
    let p1 = (a.0[0], a.0[1]);
    let p2 = (a.1[0], a.1[1]);
    let q1 = (b.0[0], b.0[1]);
    let q2 = (b.1[0], b.1[1]);

    if points_close(p1, q1) || points_close(p1, q2) || points_close(p2, q1) || points_close(p2, q2)
    {
        return false;
    }

    let o1 = orientation(p1, p2, q1);
    let o2 = orientation(p1, p2, q2);
    let o3 = orientation(q1, q2, p1);
    let o4 = orientation(q1, q2, p2);
    o1 * o2 < 0.0 && o3 * o4 < 0.0
}

fn segment_length(segment: ([f64; 2], [f64; 2])) -> f64 {
    let dx = segment.1[0] - segment.0[0];
    let dy = segment.1[1] - segment.0[1];
    (dx * dx + dy * dy).sqrt()
}

fn orientation(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> f64 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

fn points_close(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 0.01 && (a.1 - b.1).abs() < 0.01
}

fn assert_point_close(actual: [f64; 2], expected: [f64; 2]) {
    assert!(
        (actual[0] - expected[0]).abs() < 0.0001 && (actual[1] - expected[1]).abs() < 0.0001,
        "point mismatch: actual={actual:?}, expected={expected:?}"
    );
}
