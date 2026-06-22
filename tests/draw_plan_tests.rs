use std::collections::BTreeMap;

use methodfig::schema::{
    validate_draw_plan, Canvas, CanvasAspect, DrawLabel, DrawObject, DrawPlan, FigurePlan,
    StyleName,
};
use methodfig::style::style_by_name;
use methodfig::tools::draw_plan::{
    draw_plan_from_figure_plan, normalize_draw_plan_bounds, polish_model_draw_plan_geometry,
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
fn draw_plan_from_multimodal_fusion_stacks_inputs_without_connector_through_encoder() {
    let figure_plan = FigurePlan::mock_from_method(
        "Two encoders fuse image and text features before classification.",
        StyleName::WpsClean,
        CanvasAspect::PaperWide,
        85,
    );
    let style = style_by_name(StyleName::WpsClean);

    let draw_plan = draw_plan_from_figure_plan(&figure_plan, &style);

    let vision = object_box(&draw_plan, "vision_encoder");
    let text = object_box(&draw_plan, "text_encoder");
    let fusion = object_box(&draw_plan, "fusion");
    let head = object_box(&draw_plan, "head");
    let vision_edge = connector(&draw_plan, "vision_to_fusion");

    assert!(
        center_y(vision) < center_y(text),
        "multimodal inputs should be stacked as two inputs into fusion: vision={vision:?}, text={text:?}"
    );
    assert!(
        center_x(vision) < center_x(fusion) && center_x(text) < center_x(fusion),
        "both encoder inputs should sit left of fusion: vision={vision:?}, text={text:?}, fusion={fusion:?}"
    );
    assert!(
        center_x(fusion) < center_x(head),
        "task head should continue to the right of fusion: fusion={fusion:?}, head={head:?}"
    );
    assert!(
        !label_intersects_any_segment(text, vision_edge.points),
        "vision_to_fusion must not pass through text_encoder: text={text:?}, edge={:?}",
        vision_edge.points
    );
}

#[test]
fn teacher_student_round_trip_keeps_residual_connector_valid_from_real_smoke() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "goal": {
            "task": "teacher-student-distillation-with-latent-residuals",
            "audience": "ml_researchers",
            "output_context": "paper_figure"
        },
        "canvas": {
            "aspect": "paper-wide",
            "target_width_mm": 85,
            "safe_margin": 0.06
        },
        "story": {
            "main_message": "Compact student learns from task data and latent residual supervision from a large teacher; only the student runs at inference.",
            "visual_focus": [
                "student",
                "teacher",
                "latent_residual",
                "task_input",
                "task_loss",
                "inference_note"
            ],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "task_input", "bbox": [0.08333333333333333, 0.75, 0.2916666666666667, 1.0]},
                {"id": "student", "bbox": [0.2916666666666667, 0.5833333333333334, 0.75, 1.0]},
                {"id": "teacher", "bbox": [0.625, 0.16666666666666666, 1.0, 0.8333333333333334]},
                {"id": "latent_residual", "bbox": [0.4583333333333333, 0.16666666666666666, 1.0, 0.5833333333333334]},
                {"id": "task_loss", "bbox": [0.2916666666666667, 0.16666666666666666, 0.75, 0.5833333333333334]},
                {"id": "final_answer", "bbox": [0.2916666666666667, 0.94, 0.75, 1.0]},
                {"id": "inference_note", "bbox": [0.625, 0.9166666666666666, 1.0, 1.0]}
            ]
        },
        "components": [
            {"id": "c_input", "label": "Task Input", "role": "input", "visual_weight": "normal", "region": "task_input"},
            {"id": "c_student", "label": "Student\n(compact LM)", "role": "main", "visual_weight": "strong", "region": "student"},
            {"id": "c_teacher", "label": "Teacher\n(large LM)", "role": "module", "visual_weight": "normal", "region": "teacher"},
            {"id": "c_residual", "label": "Latent Residual\nSupervision", "role": "loss", "visual_weight": "strong", "region": "latent_residual"},
            {"id": "c_task_loss", "label": "Task Loss", "role": "loss", "visual_weight": "normal", "region": "task_loss"},
            {"id": "c_answer", "label": "Final Answer", "role": "output", "visual_weight": "normal", "region": "final_answer"},
            {"id": "c_inference", "label": "Inference: student only", "role": "context", "visual_weight": "muted", "region": "inference_note"}
        ],
        "edges": [
            {"id": "e_input_student", "from": "c_input", "to": "c_student", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_input_teacher", "from": "c_input", "to": "c_teacher", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_teacher_residual", "from": "c_teacher", "to": "c_residual", "label": "latent residuals", "semantic": "supervision", "style": "dash", "importance": "main"},
            {"id": "e_residual_student", "from": "c_residual", "to": "c_student", "label": "", "semantic": "supervision", "style": "dash", "importance": "main"},
            {"id": "e_student_loss", "from": "c_student", "to": "c_task_loss", "label": "", "semantic": "loss", "style": "solid", "importance": "normal"},
            {"id": "e_student_answer", "from": "c_student", "to": "c_answer", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_inference_hint", "from": "c_student", "to": "c_inference", "label": "", "semantic": "reference", "style": "dash", "importance": "aux"}
        ],
        "annotations": [
            {"id": "a_residual_type", "label": "dashed = residual signal", "target_id": "e_teacher_residual", "bbox": [0.5, 0.4666666666666666, 1.0, 1.0]}
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
    .expect("real smoke figure plan should deserialize");
    let style = style_by_name(StyleName::WpsClean);
    let mut draw_plan = draw_plan_from_figure_plan(&figure_plan, &style);

    repair_draw_plan_geometry_with_figure_plan(&mut draw_plan, &figure_plan);
    polish_model_draw_plan_geometry_with_figure_plan(&mut draw_plan, &figure_plan);

    validate_draw_plan(&draw_plan)
        .expect("teacher/residual connector should keep at least two points");
    assert!(
        connector(&draw_plan, "e_teacher_residual").points.len() >= 2,
        "residual supervision connector must remain drawable"
    );
}

#[test]
fn normalize_draw_plan_bounds_repairs_single_point_connector_from_endpoint_boxes() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "c_teacher".to_string(),
            bbox: [0.24, 0.22, 0.44, 0.38],
            text: "Teacher".to_string(),
            role: "main".to_string(),
            style: "primary_module_regular".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "c_residual".to_string(),
            bbox: [0.50, 0.22, 0.62, 0.38],
            text: "Latent Residual".to_string(),
            role: "module".to_string(),
            style: "accent_module".to_string(),
            z: 21,
        },
        DrawObject::Connector {
            id: "e_teacher_residual".to_string(),
            points: vec![[0.44, 0.30]],
            from: Some("c_teacher".to_string()),
            to: Some("c_residual".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "latent residuals".to_string(),
                bbox: [0.46, 0.18, 0.60, 0.23],
            }),
            z: 10,
        },
    ]);

    normalize_draw_plan_bounds(&mut plan);

    validate_draw_plan(&plan).expect("single-point model connector should be repaired");
    assert_eq!(
        connector(&plan, "e_teacher_residual").points,
        vec![[0.44, 0.30], [0.50, 0.30]]
    );
}

#[test]
fn model_draw_plan_polish_repairs_comp_named_residual_task_feedback_smoke() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {
            "aspect": "paper-wide",
            "target_width_mm": 85,
            "safe_margin": 0.06
        },
        "story": {
            "main_message": "Compact student learns with task loss plus latent residual supervision from a frozen teacher; only student runs at inference.",
            "visual_focus": ["student_branch", "residual_supervision", "inference_note"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 16, "rows": 12},
            "regions": [
                {"id": "input_source", "bbox": [0.375, 0.8333333333333334, 0.625, 1.0]},
                {"id": "teacher_branch", "bbox": [0.125, 0.25, 0.375, 0.75]},
                {"id": "student_branch", "bbox": [0.625, 0.25, 0.875, 0.75]},
                {"id": "residual_objective", "bbox": [0.375, 0.08333333333333333, 0.625, 0.25]},
                {"id": "task_loss", "bbox": [0.625, 0.08333333333333333, 0.875, 0.25]},
                {"id": "inference_note", "bbox": [0.875, 0.5, 1.0, 0.6666666666666666]}
            ]
        },
        "components": [
            {"id": "comp_input", "label": "Input x", "role": "input", "visual_weight": "normal", "region": "input_source"},
            {"id": "comp_teacher", "label": "Teacher\n(frozen)", "role": "context", "visual_weight": "normal", "region": "teacher_branch"},
            {"id": "comp_student", "label": "Student\n(trainable)", "role": "main", "visual_weight": "strong", "region": "student_branch"},
            {"id": "comp_residual", "label": "Latent Residual\nL_res", "role": "loss", "visual_weight": "strong", "region": "residual_objective"},
            {"id": "comp_taskloss", "label": "Task Loss\nL_task", "role": "loss", "visual_weight": "normal", "region": "task_loss"},
            {"id": "comp_inference", "label": "Inference:\nstudent only", "role": "output", "visual_weight": "muted", "region": "inference_note"}
        ],
        "edges": [
            {"id": "edge_input_to_teacher", "from": "comp_input", "to": "comp_teacher", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "edge_input_to_student", "from": "comp_input", "to": "comp_student", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "edge_teacher_to_residual", "from": "comp_teacher", "to": "comp_residual", "label": "h_T", "semantic": "supervision", "style": "dash", "importance": "normal"},
            {"id": "edge_student_to_residual", "from": "comp_student", "to": "comp_residual", "label": "h_S", "semantic": "supervision", "style": "dash", "importance": "normal"},
            {"id": "edge_student_to_taskloss", "from": "comp_student", "to": "comp_taskloss", "label": "ŷ", "semantic": "loss", "style": "solid", "importance": "normal"},
            {"id": "edge_residual_to_student", "from": "comp_residual", "to": "comp_student", "label": "∇L_res", "semantic": "feedback", "style": "dash", "importance": "aux"},
            {"id": "edge_taskloss_to_student", "from": "comp_taskloss", "to": "comp_student", "label": "∇L_task", "semantic": "feedback", "style": "solid", "importance": "aux"}
        ],
        "annotations": [
            {"id": "annot_teacher_hidden", "label": "latent h_T", "target_id": "edge_teacher_to_residual", "bbox": [0.21875, 0.2916666666666667, 0.34375, 0.375]},
            {"id": "annot_student_hidden", "label": "latent h_S", "target_id": "edge_student_to_residual", "bbox": [0.71875, 0.2916666666666667, 0.84375, 0.375]}
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
    .expect("latest smoke figure plan should deserialize");
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_input".to_string(),
            bbox: [0.02, 0.44, 0.12, 0.56],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "comp_teacher".to_string(),
            bbox: [0.153, 0.16, 0.347, 0.34],
            text: "Teacher\n(frozen)".to_string(),
            role: "context".to_string(),
            style: "neutral_module_muted_dashed".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "comp_student".to_string(),
            bbox: [0.653, 0.41, 0.847, 0.59],
            text: "Student\n(trainable)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "comp_residual".to_string(),
            bbox: [0.412, 0.20, 0.582, 0.30],
            text: "Latent Residual\nL_res".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "comp_taskloss".to_string(),
            bbox: [0.668, 0.10666666666666666, 0.832, 0.22666666666666663],
            text: "Task Loss\nL_task".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 24,
        },
        DrawObject::Connector {
            id: "edge_input_to_teacher".to_string(),
            points: vec![[0.07, 0.44], [0.07, 0.39], [0.25, 0.39], [0.25, 0.34]],
            from: Some("comp_input".to_string()),
            to: Some("comp_teacher".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "edge_input_to_student".to_string(),
            points: vec![[0.12, 0.5], [0.653, 0.5]],
            from: Some("comp_input".to_string()),
            to: Some("comp_student".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "edge_teacher_to_residual".to_string(),
            points: vec![[0.347, 0.25], [0.412, 0.25]],
            from: Some("comp_teacher".to_string()),
            to: Some("comp_residual".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "h_T".to_string(),
                bbox: [0.3615, 0.176, 0.3975, 0.226],
            }),
            z: 12,
        },
        DrawObject::Connector {
            id: "edge_student_to_residual".to_string(),
            points: vec![[0.75, 0.41], [0.75, 0.25], [0.582, 0.25]],
            from: Some("comp_student".to_string()),
            to: Some("comp_residual".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "h_S".to_string(),
                bbox: [0.774, 0.305, 0.81, 0.355],
            }),
            z: 13,
        },
        DrawObject::Connector {
            id: "edge_student_to_taskloss".to_string(),
            points: vec![[0.75, 0.41], [0.75, 0.22666666666666663]],
            from: Some("comp_student".to_string()),
            to: Some("comp_taskloss".to_string()),
            style: "normal_flow".to_string(),
            label: Some(DrawLabel {
                text: "ŷ".to_string(),
                bbox: [0.774, 0.22666666666666663, 0.81, 0.27666666666666667],
            }),
            z: 14,
        },
        DrawObject::Connector {
            id: "edge_taskloss_to_student".to_string(),
            points: vec![[0.668, 0.22666666666666663], [0.668, 0.5], [0.847, 0.5]],
            from: Some("comp_taskloss".to_string()),
            to: Some("comp_student".to_string()),
            style: "aux_flow".to_string(),
            label: Some(DrawLabel {
                text: "∇L_task".to_string(),
                bbox: [0.545, 0.45, 0.644, 0.5],
            }),
            z: 15,
        },
        DrawObject::Connector {
            id: "edge_residual_to_student".to_string(),
            points: vec![[0.582, 0.25], [0.653, 0.25], [0.653, 0.41]],
            from: Some("comp_residual".to_string()),
            to: Some("comp_student".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "∇L_res".to_string(),
                bbox: [0.545, 0.305, 0.635, 0.355],
            }),
            z: 16,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    assert!(
        draw_plan_has_text(&plan, "comp_inference", "Inference"),
        "inference component should be restored as a compact annotation instead of being omitted"
    );
    let input_teacher = connector(&plan, "edge_input_to_teacher");
    assert!(
        input_teacher.points.len() <= 2,
        "input-to-teacher should be a direct short connector, got {:?}",
        input_teacher.points
    );
    let student = object_box(&plan, "comp_student");
    let task_feedback = connector(&plan, "edge_taskloss_to_student");
    assert!(
        task_feedback
            .points
            .iter()
            .any(|point| point[0] > student[2] + 0.02),
        "task feedback should route outside the student/residual corridor: {:?}",
        task_feedback.points
    );
    let student_residual = connector(&plan, "edge_student_to_residual");
    assert!(
        !connectors_cross(student_residual.points, task_feedback.points),
        "task feedback should not cross the student residual route: residual={:?}, task={:?}",
        student_residual.points,
        task_feedback.points
    );
    for edge_id in ["edge_taskloss_to_student", "edge_residual_to_student"] {
        let edge = connector(&plan, edge_id);
        if let Some(label) = &edge.label {
            assert!(
                !label_intersects_any_segment(label.bbox, edge.points),
                "{edge_id} label should not sit on its connector: label={:?}, points={:?}",
                label.bbox,
                edge.points
            );
        }
    }
}

#[test]
fn model_draw_plan_polish_moves_task_loss_out_of_student_head_output_column_from_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "student_output_head".to_string(),
            bbox: [0.843, 0.555, 0.98, 0.695],
            text: "Student\nHead".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "task_loss_box".to_string(),
            bbox: [0.836, 0.77, 1.0, 0.88],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "output_pred".to_string(),
            bbox: [0.7775, 0.90, 0.9475, 1.0],
            text: "Prediction".to_string(),
            role: "output".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Connector {
            id: "e_student_to_task_loss".to_string(),
            points: vec![[0.918, 0.695], [0.918, 0.77]],
            from: Some("student_output_head".to_string()),
            to: Some("task_loss_box".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_student_to_output".to_string(),
            points: vec![[0.8625, 0.695], [0.8625, 0.90]],
            from: Some("student_output_head".to_string()),
            to: Some("output_pred".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Text {
            id: "ann_inference".to_string(),
            bbox: [0.6255, 0.89, 0.7655, 0.94],
            text: "Inference: student only".to_string(),
            style: "annotation".to_string(),
            z: 23,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let task_loss = object_box(&plan, "task_loss_box");
    let output = object_box(&plan, "output_pred");
    let output_edge = connector(&plan, "e_student_to_output");
    assert!(
        !label_intersects_any_segment(task_loss, output_edge.points),
        "student output route should not pass through task loss: task={task_loss:?}, edge={:?}",
        output_edge.points
    );
    assert!(
        horizontal_separation(task_loss, output) >= 0.015,
        "task loss and output should be visibly separated instead of stacked in one column: task={task_loss:?}, output={output:?}"
    );
    let note = text_box(&plan, "ann_inference");
    assert!(
        horizontal_separation(note, output) >= 0.02 || vertical_separation(note, output) >= 0.03,
        "inference note should not crowd the output corridor: note={note:?}, output={output:?}"
    );
}

#[test]
fn model_draw_plan_polish_moves_residual_equation_annotation_out_of_main_corridor_from_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "teacher_enc_mod".to_string(),
            bbox: [0.08, 0.18, 0.25, 0.32],
            text: "Teacher\nEncoder".to_string(),
            role: "module".to_string(),
            style: "muted_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student_pred_mod".to_string(),
            bbox: [0.38, 0.695, 0.57, 0.835],
            text: "Student\nPredictor".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "residual_mod".to_string(),
            bbox: [0.55, 0.405, 0.714, 0.535],
            text: "Latent\nResidual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 22,
        },
        DrawObject::Text {
            id: "ann_residual_eq".to_string(),
            bbox: [0.372, 0.382, 0.532, 0.452],
            text: "||z_T - z_S||^2".to_string(),
            style: "annotation".to_string(),
            z: 23,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let residual = object_box(&plan, "residual_mod");
    let annotation = text_box(&plan, "ann_residual_eq");
    assert!(
        annotation[0] >= residual[2] + 0.015,
        "residual equation annotation should sit outside the teacher-student corridor, next to residual box: annotation={annotation:?}, residual={residual:?}"
    );
}

#[test]
fn model_draw_plan_polish_restores_missing_inference_component_box_from_latest_smoke() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Student uses residual supervision during training and runs alone during inference.",
            "visual_focus": ["teacher_branch", "student_branch", "inference_note"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "input_source", "bbox": [0.0, 0.36, 0.18, 0.64]},
                {"id": "teacher_branch", "bbox": [0.18, 0.18, 0.42, 0.42]},
                {"id": "student_branch", "bbox": [0.60, 0.42, 0.86, 0.66]},
                {"id": "residual_node", "bbox": [0.40, 0.06, 0.60, 0.25]},
                {"id": "inference_note", "bbox": [0.62, 0.68, 0.86, 0.78]}
            ]
        },
        "components": [
            {"id": "input_text", "label": "Input x", "role": "input", "visual_weight": "normal", "region": "input_source"},
            {"id": "teacher_model", "label": "Teacher T", "role": "module", "visual_weight": "normal", "region": "teacher_branch"},
            {"id": "student_model", "label": "Student S", "role": "main", "visual_weight": "strong", "region": "student_branch"},
            {"id": "residual_supervision", "label": "Latent Residual L_res", "role": "loss", "visual_weight": "strong", "region": "residual_node"},
            {"id": "inference_only", "label": "Inference: S only", "role": "context", "visual_weight": "muted", "region": "inference_note"}
        ],
        "edges": [
            {"id": "e_input_teacher", "from": "input_text", "to": "teacher_model", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_input_student", "from": "input_text", "to": "student_model", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_teacher_residual", "from": "teacher_model", "to": "residual_supervision", "label": "z_T", "semantic": "supervision", "style": "dash", "importance": "main"},
            {"id": "e_residual_student", "from": "residual_supervision", "to": "student_model", "label": "z_T - z_S", "semantic": "supervision", "style": "dash", "importance": "main"},
            {"id": "e_student_task", "from": "student_model", "to": "student_model", "label": "Task loss L_task", "semantic": "loss", "style": "solid", "importance": "aux"}
        ],
        "annotations": [
            {"id": "anno_inference", "label": "Compact; outside main flow", "target_id": "inference_only", "bbox": [0.875, 0.225, 1.0, 0.3]}
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
    .expect("latest smoke figure plan should deserialize");
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "student_model".to_string(),
            bbox: [0.6231666666666668, 0.46375, 0.8351666666666667, 0.59375],
            text: "Student S".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 20,
        },
        DrawObject::Text {
            id: "ann_inference".to_string(),
            bbox: [0.6091666666666667, 0.68375, 0.8491666666666667, 0.74375],
            text: "Inference: S only".to_string(),
            style: "annotation".to_string(),
            z: 21,
        },
        DrawObject::Text {
            id: "anno_inference".to_string(),
            bbox: [0.6391666666666668, 0.68375, 0.8191666666666668, 0.74875],
            text: "Compact; outside main flow".to_string(),
            style: "annotation".to_string(),
            z: 22,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    assert!(
        box_object_exists(&plan, "inference_only"),
        "FigurePlan inference component should be restored as an editable box"
    );
    assert!(
        !text_object_exists(&plan, "ann_inference") && !text_object_exists(&plan, "anno_inference"),
        "duplicate floating inference texts should be removed once the component box exists"
    );
    let note = object_box(&plan, "inference_only");
    let student = object_box(&plan, "student_model");
    assert!(
        note[1] >= student[3] + 0.025,
        "inference note box should sit below the student, outside the main flow: note={note:?}, student={student:?}"
    );
}

#[test]
fn model_draw_plan_polish_repairs_projection_latent_branch_from_latest_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input_text".to_string(),
            bbox: [0.04, 0.14, 0.14, 0.24],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher_encode".to_string(),
            bbox: [0.20, 0.14, 0.44, 0.24],
            text: "Teacher Encoder".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "teacher_latent".to_string(),
            bbox: [0.035, 0.28, 0.165, 0.38],
            text: "z_T".to_string(),
            role: "main".to_string(),
            style: "neutral_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "teacher_proj".to_string(),
            bbox: [0.05, 0.42, 0.15, 0.52],
            text: "Proj".to_string(),
            role: "module".to_string(),
            style: "muted_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "student_encode".to_string(),
            bbox: [0.53, 0.14, 0.71, 0.24],
            text: "Student Encoder".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "student_latent".to_string(),
            bbox: [0.555, 0.30, 0.685, 0.40],
            text: "z_S".to_string(),
            role: "main".to_string(),
            style: "neutral_module".to_string(),
            z: 25,
        },
        DrawObject::Box {
            id: "student_head".to_string(),
            bbox: [0.54, 0.46, 0.70, 0.56],
            text: "Prediction Head".to_string(),
            role: "output".to_string(),
            style: "primary_module".to_string(),
            z: 26,
        },
        DrawObject::Box {
            id: "output_text".to_string(),
            bbox: [0.745, 0.46, 0.845, 0.56],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 27,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.85, 0.315, 0.98, 0.415],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 28,
        },
        DrawObject::Box {
            id: "inference_note".to_string(),
            bbox: [0.76025, 0.1575, 0.92025, 0.2225],
            text: "Inference: student only".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 29,
        },
        DrawObject::Connector {
            id: "e_input_to_student".to_string(),
            points: vec![[0.14, 0.19], [0.14, 0.255], [0.53, 0.255], [0.53, 0.19]],
            from: Some("input_text".to_string()),
            to: Some("student_encode".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Connector {
            id: "e_latent_residual".to_string(),
            points: vec![[0.15, 0.47], [0.15, 0.41], [0.555, 0.41], [0.555, 0.35]],
            from: Some("teacher_proj".to_string()),
            to: Some("student_latent".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "Latent Residual".to_string(),
                bbox: [0.2725, 0.342, 0.4325, 0.392],
            }),
            z: 30,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let input = object_box(&plan, "input_text");
    let teacher = object_box(&plan, "teacher_encode");
    let teacher_latent = object_box(&plan, "teacher_latent");
    let teacher_proj = object_box(&plan, "teacher_proj");
    let student_head = object_box(&plan, "student_head");
    let task_loss = object_box(&plan, "task_loss");
    assert!(
        box_object_exists(&plan, "inference_note"),
        "inference note should remain a readable component box in this projected latent topology"
    );
    let inference = object_box(&plan, "inference_note");
    assert!(
        teacher_latent[0] > input[2] + 0.04 && teacher_proj[0] > input[2] + 0.04,
        "teacher latent/proj should not be squeezed below the input: latent={teacher_latent:?}, proj={teacher_proj:?}, input={input:?}"
    );
    assert!(
        center_x(teacher_latent) > teacher[2] && center_x(teacher_proj) > teacher_latent[2],
        "teacher latent/proj should continue the teacher branch to the right: teacher={teacher:?}, latent={teacher_latent:?}, proj={teacher_proj:?}"
    );
    assert!(
        center_y(task_loss) > student_head[3] + 0.05,
        "task loss should sit below the student head/output row, not in the branch corridor: task={task_loss:?}, head={student_head:?}"
    );
    assert!(
        box_height_for_test(inference) >= 0.075 && inference[1] > student_head[3] + 0.04,
        "inference note should be readable and outside the main flow: {inference:?}"
    );
    let input_student = connector(&plan, "e_input_to_student");
    assert!(
        input_student.points.len() <= 3,
        "input-to-student should be a compact branch route, not a four-point detour: {:?}",
        input_student.points
    );
    let residual = connector(&plan, "e_latent_residual");
    if let Some(label) = &residual.label {
        assert!(
            !label_intersects_any_segment(label.bbox, residual.points),
            "latent residual label should not sit on its dashed connector: label={:?}, points={:?}",
            label.bbox,
            residual.points
        );
    }
}

#[test]
fn model_draw_plan_polish_repairs_split_input_residual_hub_from_latest_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Text {
            id: "branch_label_teacher".to_string(),
            bbox: [0.25, 0.23, 0.39, 0.29],
            text: "Teacher".to_string(),
            style: "annotation".to_string(),
            z: 10,
        },
        DrawObject::Text {
            id: "branch_label_student".to_string(),
            bbox: [0.793, 0.022, 0.933, 0.082],
            text: "Student".to_string(),
            style: "annotation".to_string(),
            z: 11,
        },
        DrawObject::Box {
            id: "teacher_input".to_string(),
            bbox: [0.08, 0.62, 0.22, 0.72],
            text: "Task Input".to_string(),
            role: "input".to_string(),
            style: "muted_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher_encode".to_string(),
            bbox: [0.08, 0.36, 0.35, 0.48],
            text: "Teacher LM\n(frozen)".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "teacher_latent".to_string(),
            bbox: [0.12, 0.06, 0.31, 0.16],
            text: "Teacher\nLatent".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "residual_obj".to_string(),
            bbox: [0.40, 0.05, 0.60, 0.15],
            text: "Latent Residual\nSupervision".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "student_input".to_string(),
            bbox: [0.62, 0.62, 0.76, 0.72],
            text: "Task Input".to_string(),
            role: "input".to_string(),
            style: "muted_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "student_encode".to_string(),
            bbox: [0.62, 0.38, 0.79, 0.48],
            text: "Student\nEncoder".to_string(),
            role: "module".to_string(),
            style: "neutral_module".to_string(),
            z: 25,
        },
        DrawObject::Box {
            id: "student_latent".to_string(),
            bbox: [0.69, 0.15, 0.86, 0.25],
            text: "Student\nLatent".to_string(),
            role: "module".to_string(),
            style: "neutral_module".to_string(),
            z: 26,
        },
        DrawObject::Box {
            id: "student_decode".to_string(),
            bbox: [0.833, 0.38, 0.97, 0.48],
            text: "Student\nDecoder".to_string(),
            role: "module".to_string(),
            style: "neutral_module".to_string(),
            z: 27,
        },
        DrawObject::Box {
            id: "task_output".to_string(),
            bbox: [0.84, 0.55, 0.97, 0.65],
            text: "Answer".to_string(),
            role: "output".to_string(),
            style: "primary_module".to_string(),
            z: 28,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.87, 0.75, 0.97, 0.85],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 29,
        },
        DrawObject::Box {
            id: "inference_note".to_string(),
            bbox: [0.40, 0.635, 0.56, 0.705],
            text: "Inference: student only".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 30,
        },
        DrawObject::Connector {
            id: "e_teacher_in".to_string(),
            points: vec![[0.215, 0.62], [0.215, 0.48]],
            from: Some("teacher_input".to_string()),
            to: Some("teacher_encode".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 40,
        },
        DrawObject::Connector {
            id: "e_teacher_enc".to_string(),
            points: vec![[0.215, 0.36], [0.215, 0.16]],
            from: Some("teacher_encode".to_string()),
            to: Some("teacher_latent".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 41,
        },
        DrawObject::Connector {
            id: "e_student_in".to_string(),
            points: vec![[0.705, 0.62], [0.705, 0.48]],
            from: Some("student_input".to_string()),
            to: Some("student_encode".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 42,
        },
        DrawObject::Connector {
            id: "e_student_enc".to_string(),
            points: vec![[0.775, 0.38], [0.775, 0.25]],
            from: Some("student_encode".to_string()),
            to: Some("student_latent".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 43,
        },
        DrawObject::Connector {
            id: "e_student_dec".to_string(),
            points: vec![[0.8465, 0.25], [0.8465, 0.38]],
            from: Some("student_latent".to_string()),
            to: Some("student_decode".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 44,
        },
        DrawObject::Connector {
            id: "e_student_out".to_string(),
            points: vec![[0.905, 0.48], [0.905, 0.55]],
            from: Some("student_decode".to_string()),
            to: Some("task_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 45,
        },
        DrawObject::Connector {
            id: "e_task_loss".to_string(),
            points: vec![[0.92, 0.65], [0.92, 0.75]],
            from: Some("task_output".to_string()),
            to: Some("task_loss".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 46,
        },
        DrawObject::Connector {
            id: "e_residual_up".to_string(),
            points: vec![[0.775, 0.15], [0.775, 0.10], [0.60, 0.10]],
            from: Some("student_latent".to_string()),
            to: Some("residual_obj".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 47,
        },
        DrawObject::Connector {
            id: "e_residual_down".to_string(),
            points: vec![[0.40, 0.11], [0.31, 0.11]],
            from: Some("residual_obj".to_string()),
            to: Some("teacher_latent".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 48,
        },
        DrawObject::Connector {
            id: "e_residual".to_string(),
            points: vec![[0.31, 0.11], [0.31, 0.20], [0.69, 0.20]],
            from: Some("teacher_latent".to_string()),
            to: Some("student_latent".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "residual".to_string(),
                bbox: [0.4415, 0.224, 0.5585, 0.274],
            }),
            z: 49,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    assert!(
        !text_object_exists(&plan, "branch_label_teacher")
            && !text_object_exists(&plan, "branch_label_student"),
        "floating branch labels should be removed or integrated into nearby boxes"
    );
    assert!(
        box_object_exists(&plan, "teacher_input") && !box_object_exists(&plan, "student_input"),
        "duplicate Task Input boxes should be merged into one shared editable input"
    );
    let shared_input = object_box(&plan, "teacher_input");
    assert!(
        center_x(shared_input) > 0.38 && center_x(shared_input) < 0.58 && shared_input[1] > 0.66,
        "shared input should sit near the bottom center between the two branches: {shared_input:?}"
    );
    assert_eq!(
        connector_endpoints(&plan, "e_student_in").0,
        Some("teacher_input"),
        "student input connector should now originate from the shared input"
    );

    let teacher_latent = object_box(&plan, "teacher_latent");
    let student_latent = object_box(&plan, "student_latent");
    assert!(
        (center_y(teacher_latent) - center_y(student_latent)).abs() <= 0.02,
        "teacher/student latent nodes should align so residual supervision can be read horizontally"
    );
    for residual_id in ["e_residual_up", "e_residual_down"] {
        let residual = connector(&plan, residual_id);
        assert!(
            residual.points.len() == 2
                && (residual.points[0][1] - residual.points[1][1]).abs() < 0.006,
            "residual hub edges should be short horizontal rails: {residual_id}={:?}",
            residual.points
        );
    }
    if connector_object_exists(&plan, "e_residual") {
        let residual = connector(&plan, "e_residual");
        assert!(
            residual.points.len() == 2
                && (residual.points[0][1] - residual.points[1][1]).abs() < 0.006,
            "fallback direct residual edge should be horizontal if it remains: {:?}",
            residual.points
        );
        assert!(
            residual
                .label
                .as_ref()
                .is_none_or(|label| !label_intersects_any_segment(label.bbox, residual.points)),
            "residual label should not sit on the dashed connector"
        );
    }

    let inference = object_box(&plan, "inference_note");
    assert!(
        inference[0] >= 0.74 && inference[3] <= 0.16 && box_height_for_test(inference) >= 0.08,
        "inference note should be readable and outside the main branch corridor: {inference:?}"
    );
}

#[test]
fn model_draw_plan_polish_tightens_simple_branch_from_latest_review_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "task_input".to_string(),
            bbox: [0.11133333333333334, 0.31, 0.26366666666666666, 0.445],
            text: "Task Input".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student_branch".to_string(),
            bbox: [0.42866666666666664, 0.535, 0.6546666666666666, 0.715],
            text: "Student\n(compact)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "teacher_branch".to_string(),
            bbox: [0.39, 0.06, 0.61, 0.20],
            text: "Teacher\n(training only)".to_string(),
            role: "context".to_string(),
            style: "neutral_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "task_output".to_string(),
            bbox: [0.5416666666666667, 0.88, 0.6696666666666667, 0.98],
            text: "Output".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 23,
        },
        DrawObject::Connector {
            id: "input_to_student".to_string(),
            points: vec![
                [0.26366666666666666, 0.3775],
                [0.26366666666666666, 0.625],
                [0.42866666666666664, 0.625],
            ],
            from: Some("task_input".to_string()),
            to: Some("student_branch".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "input_to_teacher".to_string(),
            points: vec![
                [0.26366666666666666, 0.3775],
                [0.26366666666666666, 0.13],
                [0.39, 0.13],
            ],
            from: Some("task_input".to_string()),
            to: Some("teacher_branch".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "student_to_output".to_string(),
            points: vec![[0.6056666666666668, 0.715], [0.6056666666666668, 0.88]],
            from: Some("student_branch".to_string()),
            to: Some("task_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "latent_residual_edge".to_string(),
            points: vec![[0.5416666666666666, 0.20], [0.5416666666666666, 0.535]],
            from: Some("teacher_branch".to_string()),
            to: Some("student_branch".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "Latent Residual".to_string(),
                bbox: [0.5596666666666666, 0.3425, 0.7196666666666666, 0.3925],
            }),
            z: 13,
        },
        DrawObject::Text {
            id: "anno_student_inference".to_string(),
            bbox: [0.6896666666666667, 0.599, 0.8496666666666667, 0.651],
            text: "Used at inference".to_string(),
            style: "annotation".to_string(),
            z: 41,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let teacher = object_box(&plan, "teacher_branch");
    let student = object_box(&plan, "student_branch");
    let input = object_box(&plan, "task_input");
    let output = object_box(&plan, "task_output");
    assert!(
        box_height_for_test(teacher) <= 0.105
            && box_height_for_test(student) <= 0.125
            && box_height_for_test(input) <= 0.095
            && box_height_for_test(output) <= 0.085,
        "short labels should not keep tall boxes: teacher={teacher:?}, student={student:?}, input={input:?}, output={output:?}"
    );
    assert!(
        (box_area(teacher) - box_area(student)).abs() / box_area(student).max(0.0001) <= 0.25,
        "teacher/student branch boxes should have comparable visual weight: teacher={teacher:?}, student={student:?}"
    );
    assert!(
        connector(&plan, "input_to_teacher").points.len() <= 2
            && connector(&plan, "input_to_student").points.len() <= 2,
        "input branch connectors should not keep long vertical-then-horizontal detours"
    );
    let residual = connector(&plan, "latent_residual_edge");
    let label = residual
        .label
        .as_ref()
        .expect("latent residual label should remain");
    assert!(
        !label_intersects_any_segment(label.bbox, residual.points),
        "latent residual label should move clear of the dashed edge: label={:?}, edge={:?}",
        label.bbox,
        residual.points
    );
    let inference = text_box(&plan, "anno_student_inference");
    assert!(
        inference[0] <= student[2] + 0.025,
        "inference annotation should be visibly tethered to the student branch, not floating far right: note={inference:?}, student={student:?}"
    );
}

#[test]
fn model_draw_plan_polish_handles_shared_input_width_below_readability_floor() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "task_input".to_string(),
            bbox: [0.10, 0.82, 0.19999999999999998, 0.92],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher_branch".to_string(),
            bbox: [0.42, 0.18, 0.60, 0.32],
            text: "Teacher".to_string(),
            role: "context".to_string(),
            style: "neutral_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student_branch".to_string(),
            bbox: [0.42, 0.52, 0.62, 0.68],
            text: "Student".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Connector {
            id: "input_to_teacher".to_string(),
            points: vec![[0.20, 0.87], [0.42, 0.25]],
            from: Some("task_input".to_string()),
            to: Some("teacher_branch".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "input_to_student".to_string(),
            points: vec![[0.20, 0.87], [0.42, 0.60]],
            from: Some("task_input".to_string()),
            to: Some("student_branch".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 11,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    validate_draw_plan(&plan).expect("shared input polish should not panic or corrupt geometry");
    assert!(
        box_width_for_test(object_box(&plan, "task_input")) >= 0.099,
        "shared input should remain readable even when the model emits a 0.09999999999999998 width"
    );
}

#[test]
fn model_draw_plan_polish_widens_shared_task_input_phrase_from_semantic_gate_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "task_input".to_string(),
            bbox: [0.40, 0.38, 0.50, 0.48],
            text: "Task Input".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher_model".to_string(),
            bbox: [0.38, 0.15, 0.62, 0.30],
            text: "Teacher LM".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student_model".to_string(),
            bbox: [0.66, 0.37, 0.90, 0.49],
            text: "Student Model".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Connector {
            id: "e_input_teacher".to_string(),
            points: vec![[0.45, 0.38], [0.45, 0.30]],
            from: Some("task_input".to_string()),
            to: Some("teacher_model".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_input_student".to_string(),
            points: vec![[0.50, 0.43], [0.66, 0.43]],
            from: Some("task_input".to_string()),
            to: Some("student_model".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 11,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let input = object_box(&plan, "task_input");
    assert!(
        box_width_for_test(input) >= 0.145,
        "Task Input should widen enough to avoid paper-width phrase wrapping: {input:?}"
    );
}

#[test]
fn model_draw_plan_polish_widens_left_edge_task_input_phrase_from_real_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "task_input".to_string(),
            bbox: [0.0125, 0.4325, 0.1125, 0.5675],
            text: "Task Input".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher_encoder".to_string(),
            bbox: [0.255, 0.13, 0.525, 0.2867],
            text: "Teacher Encoder".to_string(),
            role: "module".to_string(),
            style: "neutral_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student_encoder".to_string(),
            bbox: [0.255, 0.7133, 0.525, 0.87],
            text: "Student Encoder".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Connector {
            id: "edge_input_to_teacher".to_string(),
            points: vec![[0.1125, 0.5], [0.1125, 0.2083], [0.255, 0.2083]],
            from: Some("task_input".to_string()),
            to: Some("teacher_encoder".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "edge_input_to_student".to_string(),
            points: vec![[0.1125, 0.5], [0.1125, 0.7917], [0.255, 0.7917]],
            from: Some("task_input".to_string()),
            to: Some("student_encoder".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 11,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let input = object_box(&plan, "task_input");
    assert!(
        box_width_for_test(input) >= 0.145,
        "left-edge Task Input should widen instead of preserving the model's narrow region box: {input:?}"
    );
}

#[test]
fn model_draw_plan_polish_moves_bottom_margin_inference_text_near_student() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher-student distillation with latent residual supervision.",
            "visual_focus": ["student_model", "teacher_model", "latent_residual"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "input_region", "bbox": [0.00, 0.38, 0.20, 0.68]},
                {"id": "teacher_region", "bbox": [0.30, 0.25, 0.62, 0.48]},
                {"id": "student_region", "bbox": [0.30, 0.58, 0.62, 0.86]},
                {"id": "objective_region", "bbox": [0.62, 0.40, 0.86, 0.64]},
                {"id": "output_region", "bbox": [0.68, 0.64, 0.90, 0.84]},
                {"id": "inference_note_region", "bbox": [0.35, 0.86, 0.60, 0.98]}
            ]
        },
        "components": [
            {"id": "task_input", "label": "Task\nInput", "role": "input", "visual_weight": "normal", "region": "input_region"},
            {"id": "teacher_model", "label": "Teacher LM\n(large)", "role": "module", "visual_weight": "normal", "region": "teacher_region"},
            {"id": "student_model", "label": "Student\n(compact)", "role": "main", "visual_weight": "strong", "region": "student_region"},
            {"id": "latent_residual", "label": "Latent\nResidual", "role": "loss", "visual_weight": "normal", "region": "objective_region"},
            {"id": "final_output", "label": "Prediction", "role": "output", "visual_weight": "normal", "region": "output_region"},
            {"id": "inference_note", "label": "Inference: student only", "role": "context", "visual_weight": "muted", "region": "inference_note_region"}
        ],
        "edges": [
            {"id": "e_input_to_teacher", "from": "task_input", "to": "teacher_model", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_input_to_student", "from": "task_input", "to": "student_model", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_teacher_to_residual", "from": "teacher_model", "to": "latent_residual", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_residual_to_student", "from": "latent_residual", "to": "student_model", "label": "", "semantic": "supervision", "style": "dash", "importance": "main"},
            {"id": "e_student_to_output", "from": "student_model", "to": "final_output", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"}
        ],
        "annotations": [
            {"id": "ann_task_loss", "label": "+ task loss", "target_id": "student_model", "bbox": [0.375, 0.8, 0.875, 1.0]}
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
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "task_input".to_string(),
            bbox: [0.065, 0.4725, 0.185, 0.6525],
            text: "Task\nInput".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher_model".to_string(),
            bbox: [
                0.3661666666666667,
                0.3433333333333333,
                0.5921666666666667,
                0.5233333333333333,
            ],
            text: "Teacher LM\n(large)".to_string(),
            role: "module".to_string(),
            style: "neutral_module_muted_dashed".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student_model".to_string(),
            bbox: [
                0.3661666666666667,
                0.6416666666666667,
                0.5921666666666667,
                0.8216666666666667,
            ],
            text: "Student\n(compact)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "latent_residual".to_string(),
            bbox: [
                0.6756666666666666,
                0.4416666666666667,
                0.8086666666666666,
                0.5816666666666668,
            ],
            text: "Latent\nResidual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "final_output".to_string(),
            bbox: [0.7025, 0.7016666666666667, 0.8575, 0.8016666666666667],
            text: "Prediction".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Text {
            id: "inference_note".to_string(),
            bbox: [
                0.3839166666666667,
                0.9116666666666666,
                0.5744166666666668,
                0.9766666666666666,
            ],
            text: "Inference: student only".to_string(),
            style: "annotation".to_string(),
            z: 25,
        },
        DrawObject::Connector {
            id: "e_input_to_teacher".to_string(),
            points: vec![
                [0.185, 0.4979166666666667],
                [0.3661666666666667, 0.4979166666666667],
            ],
            from: Some("task_input".to_string()),
            to: Some("teacher_model".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_input_to_student".to_string(),
            points: vec![
                [0.185, 0.6470833333333333],
                [0.3661666666666667, 0.6470833333333333],
            ],
            from: Some("task_input".to_string()),
            to: Some("student_model".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_teacher_to_residual".to_string(),
            points: vec![
                [0.5921666666666667, 0.5116666666666667],
                [0.6756666666666666, 0.5116666666666667],
            ],
            from: Some("teacher_model".to_string()),
            to: Some("latent_residual".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "e_residual_to_student".to_string(),
            points: vec![
                [0.6756666666666666, 0.5816666666666668],
                [0.6756666666666666, 0.7316666666666667],
                [0.5921666666666667, 0.7316666666666667],
            ],
            from: Some("latent_residual".to_string()),
            to: Some("student_model".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Connector {
            id: "e_student_to_output".to_string(),
            points: vec![
                [0.5921666666666667, 0.7516666666666667],
                [0.7025, 0.7516666666666667],
            ],
            from: Some("student_model".to_string()),
            to: Some("final_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 14,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    let note = inference_note_bbox(&plan).expect("inference cue should remain visible");
    let student = object_box(&plan, "student_model");
    assert!(
        note[3] <= 0.90,
        "inference note should not remain in the bottom margin: note={note:?}"
    );
    assert!(
        vertical_separation(note, student) <= 0.08 || horizontal_separation(note, student) <= 0.08,
        "inference note should remain anchored near the student/output path: note={note:?}, student={student:?}"
    );
}

#[test]
fn model_draw_plan_polish_moves_right_edge_inference_text_near_student() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher-student distillation where latent residuals from a large teacher supervise a compact student, with only the student used at inference.",
            "visual_focus": ["student", "teacher", "latent_residual"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 8},
            "regions": [
                {"id": "input_region", "bbox": [0.041666666666666664, 0.4375, 0.20833333333333334, 1.0]},
                {"id": "student_region", "bbox": [0.25, 0.3125, 0.7083333333333334, 1.0]},
                {"id": "teacher_region", "bbox": [0.25, 0.0375, 0.7083333333333334, 0.2875]},
                {"id": "latent_residual_region", "bbox": [0.5, 0.1875, 1.0, 0.625]},
                {"id": "task_loss_region", "bbox": [0.5, 0.5, 1.0, 1.0]},
                {"id": "output_region", "bbox": [0.75, 0.4375, 1.0, 1.0]},
                {"id": "inference_note_region", "bbox": [0.75, 0.0375, 1.0, 0.2]}
            ]
        },
        "components": [
            {"id": "input", "label": "Input x", "role": "input", "visual_weight": "normal", "region": "input_region"},
            {"id": "student", "label": "Student\n(compact LM)", "role": "main", "visual_weight": "strong", "region": "student_region"},
            {"id": "teacher", "label": "Teacher\n(large LM)", "role": "context", "visual_weight": "muted", "region": "teacher_region"},
            {"id": "latent_residual", "label": "Latent Residual", "role": "loss", "visual_weight": "normal", "region": "latent_residual_region"},
            {"id": "task_loss", "label": "Task Loss", "role": "loss", "visual_weight": "normal", "region": "task_loss_region"},
            {"id": "output", "label": "Prediction ŷ", "role": "output", "visual_weight": "normal", "region": "output_region"},
            {"id": "inference_note", "label": "Inference: student only", "role": "context", "visual_weight": "muted", "region": "inference_note_region"}
        ],
        "edges": [
            {"id": "e_input_student", "from": "input", "to": "student", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_input_teacher", "from": "input", "to": "teacher", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_teacher_latent", "from": "teacher", "to": "latent_residual", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_student_latent", "from": "student", "to": "latent_residual", "label": "", "semantic": "supervision", "style": "dash", "importance": "main"},
            {"id": "e_student_task", "from": "student", "to": "task_loss", "label": "", "semantic": "loss", "style": "solid", "importance": "main"},
            {"id": "e_student_output", "from": "student", "to": "output", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"}
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
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input".to_string(),
            bbox: [0.065, 0.341875, 0.185, 0.476875],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student".to_string(),
            bbox: [0.3591666666666667, 0.56625, 0.5991666666666666, 0.74625],
            text: "Student\n(compact LM)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "teacher".to_string(),
            bbox: [
                0.3591666666666667,
                0.0655,
                0.5991666666666666,
                0.25949999999999995,
            ],
            text: "Teacher\n(large LM)".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "latent_residual".to_string(),
            bbox: [0.67, 0.265, 0.81, 0.375],
            text: "Latent Residual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.67, 0.43, 0.79, 0.54],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "output".to_string(),
            bbox: [0.845, 0.60625, 1.0, 0.72625],
            text: "Prediction ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 25,
        },
        DrawObject::Text {
            id: "inference_note".to_string(),
            bbox: [0.7895000000000001, 0.875, 0.98, 0.94],
            text: "Inference: student only".to_string(),
            style: "annotation".to_string(),
            z: 26,
        },
        DrawObject::Connector {
            id: "e_input_teacher".to_string(),
            points: vec![
                [0.185, 0.409375],
                [0.185, 0.16249999999999998],
                [0.3591666666666667, 0.16249999999999998],
            ],
            from: Some("input".to_string()),
            to: Some("teacher".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_input_student".to_string(),
            points: vec![
                [0.185, 0.409375],
                [0.185, 0.65625],
                [0.3591666666666667, 0.65625],
            ],
            from: Some("input".to_string()),
            to: Some("student".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_teacher_latent".to_string(),
            points: vec![
                [0.5991666666666666, 0.16249999999999998],
                [0.5991666666666666, 0.32],
                [0.67, 0.32],
            ],
            from: Some("teacher".to_string()),
            to: Some("latent_residual".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "e_student_latent".to_string(),
            points: vec![
                [0.47916666666666663, 0.56625],
                [0.47916666666666663, 0.32],
                [0.67, 0.32],
            ],
            from: Some("student".to_string()),
            to: Some("latent_residual".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Connector {
            id: "e_student_task".to_string(),
            points: vec![
                [0.5991666666666666, 0.65625],
                [0.5991666666666666, 0.485],
                [0.67, 0.485],
            ],
            from: Some("student".to_string()),
            to: Some("task_loss".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 14,
        },
        DrawObject::Connector {
            id: "e_student_output".to_string(),
            points: vec![[0.5991666666666666, 0.66625], [0.845, 0.66625]],
            from: Some("student".to_string()),
            to: Some("output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 15,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    let note = inference_note_bbox(&plan).expect("inference cue should remain visible");
    let student = object_box(&plan, "student");
    assert!(
        note[2] <= 0.96,
        "inference note should not remain flush with the right edge: note={note:?}"
    );
    assert!(
        vertical_separation(note, student) <= 0.10 || horizontal_separation(note, student) <= 0.08,
        "right-edge inference note should be pulled back near the student path: note={note:?}, student={student:?}"
    );
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
fn model_draw_plan_polish_with_figure_plan_removes_split_residual_boxes() {
    let figure_plan = teacher_student_no_inference_lane_annotation_plan();
    let style = style_by_name(StyleName::WpsClean);
    let mut plan = draw_plan_from_figure_plan(&figure_plan, &style);
    plan.objects.retain(|object| {
        !matches!(
            object,
            DrawObject::Connector { id, .. }
                if id == "e_teacher_to_latent" || id == "e_latent_to_student"
        )
    });
    plan.objects.push(DrawObject::Box {
        id: "teacher_residual_obj".to_string(),
        bbox: [0.05, 0.05, 0.38, 0.244],
        text: "Latent Residual L_res^T".to_string(),
        role: "loss".to_string(),
        style: "accent_module".to_string(),
        z: 40,
    });
    plan.objects.push(DrawObject::Box {
        id: "student_residual_obj".to_string(),
        bbox: [0.61, 0.05, 0.95, 0.244],
        text: "Latent Residual L_res^S".to_string(),
        role: "loss".to_string(),
        style: "accent_module".to_string(),
        z: 41,
    });
    plan.objects.push(DrawObject::Connector {
        id: "e_teacher_to_latent".to_string(),
        points: vec![[0.32, 0.20], [0.20, 0.15]],
        from: Some("teacher_module".to_string()),
        to: Some("teacher_residual_obj".to_string()),
        style: "dashed_supervision".to_string(),
        label: None,
        z: 10,
    });
    plan.objects.push(DrawObject::Connector {
        id: "e_latent_to_student".to_string(),
        points: vec![[0.38, 0.244], [0.38, 0.50], [0.24, 0.50]],
        from: Some("teacher_residual_obj".to_string()),
        to: Some("student_module".to_string()),
        style: "dashed_supervision".to_string(),
        label: None,
        z: 11,
    });

    polish_model_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    assert!(
        !box_object_exists(&plan, "teacher_residual_obj")
            && !box_object_exists(&plan, "student_residual_obj"),
        "split residual boxes are not in FigurePlan and should be removed"
    );
    assert_eq!(
        connector_endpoints(&plan, "e_teacher_to_latent"),
        (Some("teacher_module"), Some("latent_residual"))
    );
    assert_eq!(
        connector_endpoints(&plan, "e_latent_to_student"),
        (Some("latent_residual"), Some("student_module"))
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
fn model_draw_plan_polish_resyncs_solid_teacher_residual_after_topology_repairs_from_smoke() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher residuals supervise a compact student.",
            "visual_focus": ["teacher_model", "latent_residuals", "student_model"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "teacher_region", "bbox": [0.18, 0.14, 0.38, 0.30]},
                {"id": "latent_region", "bbox": [0.14, 0.52, 0.34, 0.64]},
                {"id": "student_region", "bbox": [0.60, 0.36, 0.82, 0.50]}
            ]
        },
        "components": [
            {"id": "teacher_model", "label": "Teacher LM", "role": "context", "visual_weight": "muted", "region": "teacher_region", "allowed_asset_id": null},
            {"id": "latent_residuals", "label": "Latent Residuals", "role": "loss", "visual_weight": "normal", "region": "latent_region", "allowed_asset_id": null},
            {"id": "student_model", "label": "Student Model", "role": "main", "visual_weight": "strong", "region": "student_region", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "e_teacher_residuals", "from": "teacher_model", "to": "latent_residuals", "label": "", "semantic": "supervision", "style": "solid", "importance": "normal"},
            {"id": "e_residuals_student", "from": "latent_residuals", "to": "student_model", "label": "supervision", "semantic": "supervision", "style": "dash", "importance": "normal"}
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
            id: "teacher_model".to_string(),
            bbox: [0.38, 0.15, 0.62, 0.30],
            text: "Teacher LM".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "latent_residuals".to_string(),
            bbox: [0.12, 0.52, 0.32, 0.63],
            text: "Latent Residuals".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student_model".to_string(),
            bbox: [0.66, 0.37, 0.90, 0.49],
            text: "Student Model".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Connector {
            id: "e_teacher_residuals".to_string(),
            points: vec![[0.50, 0.30], [0.22, 0.30], [0.22, 0.52]],
            from: Some("teacher_model".to_string()),
            to: Some("latent_residuals".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_residuals_student".to_string(),
            points: vec![[0.32, 0.58], [0.66, 0.58], [0.66, 0.43]],
            from: Some("latent_residuals".to_string()),
            to: Some("student_model".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 11,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut draw_plan, &figure_plan);

    assert_eq!(
        input_teacher_style(&draw_plan, "e_teacher_residuals"),
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
fn model_draw_plan_polish_restores_figure_plan_edge_endpoints_from_connector_target_smoke() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher residual supervision flows through an explicit residual node.",
            "visual_focus": ["teacher_encoder", "latent_residual_label", "student_encoder"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "teacher_region", "bbox": [0.20, 0.31, 0.40, 0.52]},
                {"id": "student_region", "bbox": [0.62, 0.48, 0.82, 0.69]},
                {"id": "residual_region", "bbox": [0.30, 0.58, 0.43, 0.68]}
            ]
        },
        "components": [
            {"id": "teacher_encoder", "label": "Teacher\nEncoder", "role": "context", "visual_weight": "normal", "region": "teacher_region", "allowed_asset_id": null},
            {"id": "student_encoder", "label": "Student\nEncoder", "role": "main", "visual_weight": "strong", "region": "student_region", "allowed_asset_id": null},
            {"id": "latent_residual_label", "label": "Latent residual", "role": "loss", "visual_weight": "normal", "region": "residual_region", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "e_teacher_to_residual", "from": "teacher_encoder", "to": "latent_residual_label", "label": "", "semantic": "supervision", "style": "solid", "importance": "normal"},
            {"id": "e_residual_to_student", "from": "latent_residual_label", "to": "student_encoder", "label": "", "semantic": "supervision", "style": "dash", "importance": "normal"}
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
    .expect("smoke fixture should deserialize");
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "teacher_encoder".to_string(),
            bbox: [0.20, 0.316, 0.40, 0.516],
            text: "Teacher\nEncoder".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student_encoder".to_string(),
            bbox: [0.62, 0.486, 0.818, 0.686],
            text: "Student\nEncoder".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "latent_residual_label".to_string(),
            bbox: [0.4335, 0.121, 0.5665, 0.221],
            text: "Latent residual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 22,
        },
        DrawObject::Connector {
            id: "e_teacher_to_residual".to_string(),
            points: vec![[0.32825, 0.386], [0.32825, 0.416]],
            from: Some("teacher_encoder".to_string()),
            to: Some("e_residual_to_student".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_residual_to_student".to_string(),
            points: vec![
                [0.5, 0.221],
                [0.5, 0.286],
                [0.61975, 0.286],
                [0.61975, 0.486],
            ],
            from: Some("latent_residual_label".to_string()),
            to: Some("student_encoder".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 11,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut draw_plan, &figure_plan);

    assert_eq!(
        connector_endpoints(&draw_plan, "e_teacher_to_residual"),
        (Some("teacher_encoder"), Some("latent_residual_label")),
        "FigurePlan edge ids should restore semantic endpoints when model points to a connector id"
    );
    let edge = connector(&draw_plan, "e_teacher_to_residual");
    assert!(
        edge.points.len() >= 2
            && !edge.points.windows(2).all(|window| {
                (window[0][0] - window[1][0]).abs() < 0.0001
                    && (window[0][1] - window[1][1]).abs() < 0.035
            }),
        "restored connector should not remain a degenerate stub: {:?}",
        edge.points
    );
}

#[test]
fn model_draw_plan_polish_reroutes_residual_node_off_input_student_crossing_from_followup_smoke() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher residual supervision supports the student while both receive the same input.",
            "visual_focus": ["teacher_encoder", "student_encoder", "latent_residual_label"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "input_region", "bbox": [0.02, 0.48, 0.16, 0.62]},
                {"id": "teacher_region", "bbox": [0.20, 0.31, 0.40, 0.52]},
                {"id": "student_region", "bbox": [0.62, 0.48, 0.82, 0.69]},
                {"id": "residual_region", "bbox": [0.30, 0.58, 0.43, 0.68]},
                {"id": "output_region", "bbox": [0.71, 0.34, 0.88, 0.45]},
                {"id": "loss_region", "bbox": [0.72, 0.24, 0.88, 0.35]}
            ]
        },
        "components": [
            {"id": "input_text", "label": "Input text", "role": "input", "visual_weight": "normal", "region": "input_region", "allowed_asset_id": null},
            {"id": "teacher_encoder", "label": "Teacher\nEncoder", "role": "context", "visual_weight": "normal", "region": "teacher_region", "allowed_asset_id": null},
            {"id": "student_encoder", "label": "Student\nEncoder", "role": "main", "visual_weight": "strong", "region": "student_region", "allowed_asset_id": null},
            {"id": "latent_residual_label", "label": "Latent residual", "role": "loss", "visual_weight": "normal", "region": "residual_region", "allowed_asset_id": null},
            {"id": "task_output", "label": "Task output", "role": "output", "visual_weight": "strong", "region": "output_region", "allowed_asset_id": null},
            {"id": "task_loss_label", "label": "Task loss", "role": "loss", "visual_weight": "normal", "region": "loss_region", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "e_input_to_teacher", "from": "input_text", "to": "teacher_encoder", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_input_to_student", "from": "input_text", "to": "student_encoder", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_teacher_to_residual", "from": "teacher_encoder", "to": "latent_residual_label", "label": "", "semantic": "supervision", "style": "solid", "importance": "normal"},
            {"id": "e_residual_to_student", "from": "latent_residual_label", "to": "student_encoder", "label": "", "semantic": "supervision", "style": "dash", "importance": "normal"},
            {"id": "e_student_to_output", "from": "student_encoder", "to": "task_output", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_output_to_loss", "from": "task_output", "to": "task_loss_label", "label": "", "semantic": "loss", "style": "solid", "importance": "normal"}
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
    .expect("follow-up smoke fixture should deserialize");
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input_text".to_string(),
            bbox: [0.02, 0.506, 0.145, 0.626],
            text: "Input text".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher_encoder".to_string(),
            bbox: [0.20, 0.316, 0.40, 0.516],
            text: "Teacher\nEncoder".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student_encoder".to_string(),
            bbox: [0.62, 0.486, 0.818, 0.686],
            text: "Student\nEncoder".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "latent_residual_label".to_string(),
            bbox: [0.30, 0.581, 0.433, 0.681],
            text: "Latent residual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "task_output".to_string(),
            bbox: [0.714, 0.346, 0.884, 0.446],
            text: "Task output".to_string(),
            role: "output".to_string(),
            style: "primary_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "task_loss_label".to_string(),
            bbox: [0.719, 0.246, 0.879, 0.346],
            text: "Task loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 25,
        },
        DrawObject::Connector {
            id: "e_input_to_student".to_string(),
            points: vec![[0.145, 0.566], [0.62, 0.566], [0.62, 0.586]],
            from: Some("input_text".to_string()),
            to: Some("student_encoder".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_teacher_to_residual".to_string(),
            points: vec![[0.3665, 0.516], [0.3665, 0.581]],
            from: Some("teacher_encoder".to_string()),
            to: Some("latent_residual_label".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_residual_to_student".to_string(),
            points: vec![[0.433, 0.586], [0.62, 0.586]],
            from: Some("latent_residual_label".to_string()),
            to: Some("student_encoder".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "e_student_to_output".to_string(),
            points: vec![[0.719, 0.586], [0.799, 0.586], [0.799, 0.446]],
            from: Some("student_encoder".to_string()),
            to: Some("task_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Connector {
            id: "e_output_to_loss".to_string(),
            points: vec![[0.799, 0.346], [0.799, 0.296]],
            from: Some("task_output".to_string()),
            to: Some("task_loss_label".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 14,
        },
    ]);

    assert!(
        connectors_cross(
            connector(&draw_plan, "e_input_to_student").points,
            connector(&draw_plan, "e_teacher_to_residual").points,
        ),
        "fixture should start with the observed crossing"
    );

    polish_model_draw_plan_geometry_with_figure_plan(&mut draw_plan, &figure_plan);

    assert!(
        !connectors_cross(
            connector(&draw_plan, "e_input_to_student").points,
            connector(&draw_plan, "e_teacher_to_residual").points,
        ),
        "input-to-student route should not cross teacher-to-residual after polish: input={:?}, residual={:?}",
        connector(&draw_plan, "e_input_to_student").points,
        connector(&draw_plan, "e_teacher_to_residual").points
    );
    let residual = object_box(&draw_plan, "latent_residual_label");
    let input_student = connector(&draw_plan, "e_input_to_student");
    assert!(
        !label_near_any_segment(residual, input_student.points, 0.006),
        "residual node should not sit on the input-to-student main data route: residual={residual:?}, input={:?}",
        input_student.points
    );
}

#[test]
fn model_draw_plan_polish_reopens_right_edge_student_output_loss_lane_from_followup_smoke() {
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "teacher_input".to_string(),
            bbox: [0.065, 0.42, 0.185, 0.52],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "muted_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher_encoder".to_string(),
            bbox: [0.26, 0.38, 0.42, 0.56],
            text: "Teacher\n(Large LM)".to_string(),
            role: "module".to_string(),
            style: "neutral_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "teacher_latent".to_string(),
            bbox: [0.46, 0.40, 0.56, 0.54],
            text: "Latent h_T".to_string(),
            role: "data".to_string(),
            style: "neutral_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "latent_residual".to_string(),
            bbox: [0.46, 0.145, 0.66, 0.275],
            text: "Latent Residual\nSupervision".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "student_input".to_string(),
            bbox: [0.72, 0.42, 0.84, 0.52],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "muted_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "student_encoder".to_string(),
            bbox: [0.88, 0.38, 1.0, 0.56],
            text: "Student\n(Compact)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 25,
        },
        DrawObject::Box {
            id: "student_output".to_string(),
            bbox: [0.8745, 0.20, 0.9855, 0.30],
            text: "Answer ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 26,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.8445, 0.715, 0.9545, 0.815],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 27,
        },
        DrawObject::Box {
            id: "inference_note".to_string(),
            bbox: [0.7495, 0.07, 0.94, 0.15],
            text: "Inference: student only".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 28,
        },
        DrawObject::Connector {
            id: "e_teacher_in".to_string(),
            points: vec![[0.185, 0.47], [0.26, 0.47]],
            from: Some("teacher_input".to_string()),
            to: Some("teacher_encoder".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_teacher_latent".to_string(),
            points: vec![[0.42, 0.47], [0.46, 0.47]],
            from: Some("teacher_encoder".to_string()),
            to: Some("teacher_latent".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_teacher_to_residual".to_string(),
            points: vec![[0.56, 0.40], [0.56, 0.275]],
            from: Some("teacher_latent".to_string()),
            to: Some("latent_residual".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "e_residual_to_student".to_string(),
            points: vec![[0.66, 0.21], [0.88, 0.21], [0.88, 0.38]],
            from: Some("latent_residual".to_string()),
            to: Some("student_encoder".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Connector {
            id: "e_student_in".to_string(),
            points: vec![[0.84, 0.47], [0.88, 0.47]],
            from: Some("student_input".to_string()),
            to: Some("student_encoder".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 14,
        },
        DrawObject::Connector {
            id: "e_student_out".to_string(),
            points: vec![[0.93, 0.38], [0.93, 0.30]],
            from: Some("student_encoder".to_string()),
            to: Some("student_output".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 15,
        },
        DrawObject::Connector {
            id: "e_task_loss".to_string(),
            points: vec![
                [0.8745, 0.25],
                [0.8745, 0.585],
                [0.9545, 0.585],
                [0.8995, 0.715],
            ],
            from: Some("student_output".to_string()),
            to: Some("task_loss".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 16,
        },
        DrawObject::Text {
            id: "anno_training_only".to_string(),
            bbox: [0.6965, 0.603, 0.8565, 0.673],
            text: "training only".to_string(),
            style: "annotation".to_string(),
            z: 29,
        },
    ]);

    assert!(
        label_intersects_any_segment(
            object_box(&draw_plan, "student_output"),
            connector(&draw_plan, "e_residual_to_student").points
        ),
        "fixture should start with residual supervision crossing the output box"
    );
    assert!(
        connectors_cross(
            connector(&draw_plan, "e_student_in").points,
            connector(&draw_plan, "e_task_loss").points
        ),
        "fixture should start with task-loss route crossing the student input route"
    );

    polish_model_draw_plan_geometry(&mut draw_plan);

    let student = object_box(&draw_plan, "student_encoder");
    let output = object_box(&draw_plan, "student_output");
    assert!(
        student[2] < 0.90 && output[0] > student[2],
        "student should move left enough to create a right-side output/loss lane: student={student:?}, output={output:?}"
    );
    assert!(
        !label_intersects_any_segment(
            output,
            connector(&draw_plan, "e_residual_to_student").points
        ),
        "residual supervision should not cross the output box: output={output:?}, residual={:?}",
        connector(&draw_plan, "e_residual_to_student").points
    );
    assert!(
        !connectors_cross(
            connector(&draw_plan, "e_student_in").points,
            connector(&draw_plan, "e_task_loss").points
        ),
        "task-loss route should not cross the student input route: input={:?}, loss={:?}",
        connector(&draw_plan, "e_student_in").points,
        connector(&draw_plan, "e_task_loss").points
    );
}

#[test]
fn model_draw_plan_polish_residual_crossing_skips_high_student_without_clamp_panic() {
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input_text".to_string(),
            bbox: [0.02, 0.18, 0.145, 0.28],
            text: "Input text".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher_encoder".to_string(),
            bbox: [0.20, 0.12, 0.40, 0.30],
            text: "Teacher\nEncoder".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student_encoder".to_string(),
            bbox: [0.62, 0.118, 0.82, 0.318],
            text: "Student\nEncoder".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "latent_residual_label".to_string(),
            bbox: [0.30, 0.22, 0.433, 0.32],
            text: "Latent residual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Connector {
            id: "e_input_to_student".to_string(),
            points: vec![[0.145, 0.23], [0.62, 0.23]],
            from: Some("input_text".to_string()),
            to: Some("student_encoder".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_teacher_to_residual".to_string(),
            points: vec![[0.3665, 0.30], [0.3665, 0.22]],
            from: Some("teacher_encoder".to_string()),
            to: Some("latent_residual_label".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_residual_to_student".to_string(),
            points: vec![[0.433, 0.23], [0.62, 0.23]],
            from: Some("latent_residual_label".to_string()),
            to: Some("student_encoder".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 12,
        },
    ]);

    polish_model_draw_plan_geometry(&mut draw_plan);

    assert!(
        object_box(&draw_plan, "student_encoder")[1] >= 0.118,
        "high student fixture should remain valid instead of panicking"
    );
}

#[test]
fn model_draw_plan_polish_expands_student_only_inference_note_from_latest_smoke() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Student runs alone at inference.",
            "visual_focus": ["student_encoder", "student_head", "task_output", "inference_note"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "student_region", "bbox": [0.14, 0.54, 0.56, 0.67]},
                {"id": "head_region", "bbox": [0.59, 0.70, 0.79, 0.86]},
                {"id": "output_region", "bbox": [0.84, 0.45, 0.98, 0.56]},
                {"id": "note_region", "bbox": [0.61, 0.57, 0.77, 0.64]}
            ]
        },
        "components": [
            {"id": "student_encoder", "label": "Student\nEncoder", "role": "main", "visual_weight": "strong", "region": "student_region", "allowed_asset_id": null},
            {"id": "student_head", "label": "Student\nHead", "role": "main", "visual_weight": "strong", "region": "head_region", "allowed_asset_id": null},
            {"id": "task_output", "label": "Output ŷ", "role": "output", "visual_weight": "strong", "region": "output_region", "allowed_asset_id": null},
            {"id": "inference_note", "label": "Inference: student only", "role": "context", "visual_weight": "muted", "region": "note_region", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "e_student_to_head", "from": "student_encoder", "to": "student_head", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_head_to_output", "from": "student_head", "to": "task_output", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"}
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
            id: "student_encoder".to_string(),
            bbox: [0.14233333333333345, 0.548, 0.562, 0.668],
            text: "Student\nEncoder".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student_head".to_string(),
            bbox: [0.591, 0.7, 0.789, 0.859],
            text: "Student\nHead".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "task_output".to_string(),
            bbox: [0.843, 0.4563333333333333, 0.98, 0.5563333333333333],
            text: "Output ŷ".to_string(),
            role: "output".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "inference_note".to_string(),
            bbox: [0.61225, 0.5755, 0.77225, 0.6405],
            text: "Inference: student only".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 23,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut draw_plan, &figure_plan);

    let note = object_box(&draw_plan, "inference_note");
    assert!(
        box_height_for_test(note) >= 0.080 && box_width_for_test(note) >= 0.16,
        "student-only inference note should stay readable at paper width: {note:?}"
    );
}

#[test]
fn model_draw_plan_polish_clears_y_branch_inference_lane_loss_label_and_input_detours() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher provides latent residual supervision while the student predicts the answer.",
            "visual_focus": ["student_branch", "teacher_branch", "latent_residual"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 8},
            "regions": [
                {"id": "task_input_region", "bbox": [0.02, 0.48, 0.12, 0.58]},
                {"id": "teacher_branch_region", "bbox": [0.1648, 0.1913, 0.3768, 0.3713]},
                {"id": "student_branch_region", "bbox": [0.6113, 0.4413, 0.8470, 0.6213]},
                {"id": "latent_residual_region", "bbox": [0.4418, 0.2313, 0.5748, 0.3313]},
                {"id": "task_output_region", "bbox": [0.7292, 0.8250, 0.8292, 0.9250]},
                {"id": "inference_note_region", "bbox": [0.4013, 0.4804, 0.5713, 0.5821]}
            ]
        },
        "components": [
            {"id": "task_input", "label": "Task Input\nx", "role": "input", "visual_weight": "normal", "region": "task_input_region", "allowed_asset_id": null},
            {"id": "teacher_branch", "label": "Teacher\n(frozen)", "role": "module", "visual_weight": "normal", "region": "teacher_branch_region", "allowed_asset_id": null},
            {"id": "student_branch", "label": "Student\n(trainable)", "role": "main", "visual_weight": "strong", "region": "student_branch_region", "allowed_asset_id": null},
            {"id": "latent_residual", "label": "Latent Residual\nL_res", "role": "loss", "visual_weight": "strong", "region": "latent_residual_region", "allowed_asset_id": null},
            {"id": "task_output", "label": "ŷ", "role": "output", "visual_weight": "normal", "region": "task_output_region", "allowed_asset_id": null},
            {"id": "inference_note", "label": "Inference\nonly", "role": "context", "visual_weight": "muted", "region": "inference_note_region", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "e_input_teacher", "from": "task_input", "to": "teacher_branch", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_input_student", "from": "task_input", "to": "student_branch", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_teacher_residual", "from": "teacher_branch", "to": "latent_residual", "label": "h_t", "semantic": "supervision", "style": "dash", "importance": "main"},
            {"id": "e_student_residual", "from": "student_branch", "to": "latent_residual", "label": "h_s", "semantic": "supervision", "style": "dash", "importance": "main"},
            {"id": "e_student_output", "from": "student_branch", "to": "task_output", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_task_loss", "from": "task_output", "to": "student_branch", "label": "L_task", "semantic": "loss", "style": "solid", "importance": "normal"}
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
    .expect("latest y-branch smoke fixture should deserialize");
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "task_input".to_string(),
            bbox: [0.02, 0.48125, 0.12, 0.58125],
            text: "Task Input\nx".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher_branch".to_string(),
            bbox: [0.1648333333333333, 0.19125, 0.37683333333333335, 0.37125],
            text: "Teacher\n(frozen)".to_string(),
            role: "module".to_string(),
            style: "muted_module_dashed".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student_branch".to_string(),
            bbox: [0.6113333333333335, 0.44125, 0.847, 0.62125],
            text: "Student\n(trainable)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "latent_residual".to_string(),
            bbox: [0.44183333333333336, 0.23125, 0.5748333333333333, 0.33125],
            text: "Latent Residual\nL_res".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "task_output".to_string(),
            bbox: [0.7291666666666667, 0.825, 0.8291666666666667, 0.925],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "inference_note".to_string(),
            bbox: [
                0.40133333333333354,
                0.4804166666666667,
                0.5713333333333335,
                0.5820833333333333,
            ],
            text: "Inference\nonly".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 25,
        },
        DrawObject::Connector {
            id: "e_input_teacher".to_string(),
            points: vec![
                [0.12, 0.53125],
                [0.08, 0.53125],
                [0.08, 0.37125],
                [0.1648333333333333, 0.37125],
            ],
            from: Some("task_input".to_string()),
            to: Some("teacher_branch".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_input_student".to_string(),
            points: vec![
                [0.12, 0.53125],
                [0.12, 0.4554166666666667],
                [0.6113333333333335, 0.4554166666666667],
                [0.6113333333333335, 0.53125],
            ],
            from: Some("task_input".to_string()),
            to: Some("student_branch".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_teacher_residual".to_string(),
            points: vec![
                [0.37683333333333335, 0.28125],
                [0.44183333333333336, 0.28125],
            ],
            from: Some("teacher_branch".to_string()),
            to: Some("latent_residual".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "h_t".to_string(),
                bbox: [0.3913333333333333, 0.20725, 0.4273333333333333, 0.25725],
            }),
            z: 12,
        },
        DrawObject::Connector {
            id: "e_student_residual".to_string(),
            points: vec![
                [0.7291666666666667, 0.44125],
                [0.4983333333333333, 0.44125],
                [0.5083333333333333, 0.33125],
            ],
            from: Some("student_branch".to_string()),
            to: Some("latent_residual".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "h_s".to_string(),
                bbox: [0.59575, 0.36725, 0.63175, 0.41725],
            }),
            z: 13,
        },
        DrawObject::Connector {
            id: "e_student_output".to_string(),
            points: vec![[0.7791666666666668, 0.62125], [0.7791666666666668, 0.825]],
            from: Some("student_branch".to_string()),
            to: Some("task_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 14,
        },
        DrawObject::Connector {
            id: "e_task_loss".to_string(),
            points: vec![[0.7291666666666667, 0.825], [0.7291666666666667, 0.62125]],
            from: Some("task_output".to_string()),
            to: Some("student_branch".to_string()),
            style: "main_flow".to_string(),
            label: Some(DrawLabel {
                text: "L_task".to_string(),
                bbox: [0.6151666666666668, 0.775, 0.7051666666666667, 0.825],
            }),
            z: 15,
        },
    ]);

    let note = object_box(&draw_plan, "inference_note");
    let input = object_box(&draw_plan, "task_input");
    let student = object_box(&draw_plan, "student_branch");
    assert!(
        note[0] > input[2]
            && note[2] < student[0]
            && axis_overlap_ratio_for_test(note[1], note[3], input[1], student[3]) > 0.35,
        "fixture should start with the inference note inside the input-student corridor"
    );

    polish_model_draw_plan_geometry_with_figure_plan(&mut draw_plan, &figure_plan);

    assert!(
        !box_object_exists(&draw_plan, "inference_note"),
        "standalone inference lane should no longer remain as a component"
    );
    assert!(
        draw_plan_has_text(&draw_plan, "inference_note", "student only"),
        "inference cue should remain as compact editable annotation"
    );
    assert_eq!(
        connector(&draw_plan, "e_input_student").points.len(),
        2,
        "input-to-student should be a direct local route"
    );
    assert_eq!(
        connector(&draw_plan, "e_input_teacher").points.len(),
        2,
        "input-to-teacher should not keep the outer dogleg"
    );
    assert!(
        connector(&draw_plan, "e_task_loss").label.is_none(),
        "task loss cue should not be a connector label on an output-student edge"
    );
    assert!(
        draw_plan_has_text(&draw_plan, "e_task_loss_cue", "L_task"),
        "task-loss cue should be preserved as an editable annotation"
    );
    assert!(
        !label_near_any_segment(
            text_box(&draw_plan, "inference_note"),
            connector(&draw_plan, "e_input_student").points,
            0.006
        ),
        "inference annotation should move off the input-student connector"
    );
}

#[test]
fn model_draw_plan_polish_resnaps_task_loss_label_to_short_loss_edge_from_latest_smoke() {
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "task_head".to_string(),
            bbox: [0.546, 0.786229166666667, 0.694, 0.8862291666666671],
            text: "Task Head".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.749, 0.786229166666667, 0.897, 0.8862291666666671],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 21,
        },
        DrawObject::Connector {
            id: "e_task_loss".to_string(),
            points: vec![[0.694, 0.836229166666667], [0.749, 0.836229166666667]],
            from: Some("task_head".to_string()),
            to: Some("task_loss".to_string()),
            style: "main_flow".to_string(),
            label: Some(DrawLabel {
                text: "L_task".to_string(),
                bbox: [0.394, 0.811229166666667, 0.534, 0.861229166666667],
            }),
            z: 10,
        },
    ]);

    polish_model_draw_plan_geometry(&mut draw_plan);

    let label = connector(&draw_plan, "e_task_loss")
        .label
        .as_ref()
        .expect("task loss label should remain on the connector");
    let task_loss_route_y = connector(&draw_plan, "e_task_loss").points[0][1];
    assert!(
        (center_y(label.bbox) - task_loss_route_y).abs() <= 0.08,
        "L_task label should be snapped near the short task-loss connector: {:?}",
        label.bbox
    );
    assert!(
        label.bbox[0] >= object_box(&draw_plan, "task_head")[2] - 0.03,
        "L_task should no longer float in unrelated left whitespace: {:?}",
        label.bbox
    );
}

#[test]
fn model_draw_plan_polish_anchors_bottom_frozen_annotation_near_teacher_from_latest_smoke() {
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "teacher".to_string(),
            bbox: [0.3375, 0.16, 0.5375, 0.34],
            text: "Teacher\n(large LM)".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student".to_string(),
            bbox: [0.34, 0.44, 0.54, 0.62],
            text: "Student\n(small LM)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Text {
            id: "a_freeze".to_string(),
            bbox: [0.4375, 0.94, 0.5625, 1.0],
            text: "frozen".to_string(),
            style: "annotation".to_string(),
            z: 30,
        },
    ]);

    polish_model_draw_plan_geometry(&mut draw_plan);

    let teacher = object_box(&draw_plan, "teacher");
    let freeze = text_box(&draw_plan, "a_freeze");
    assert!(
        freeze[3] < 0.45
            && (horizontal_separation(freeze, teacher) <= 0.08
                || vertical_separation(freeze, teacher) <= 0.08),
        "frozen annotation should be anchored near teacher, not left at the bottom margin: {freeze:?}"
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
fn model_draw_plan_polish_compacts_right_edge_output_without_overlap_or_degenerate_edge() {
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "student_head".to_string(),
            bbox: [0.76, 0.54, 0.92, 0.70],
            text: "Task\nHead".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "output".to_string(),
            bbox: [0.90, 0.58, 1.00, 0.68],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 21,
        },
        DrawObject::Connector {
            id: "e_output".to_string(),
            points: vec![[0.92, 0.62], [0.96, 0.62]],
            from: Some("student_head".to_string()),
            to: Some("output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
    ]);

    polish_model_draw_plan_geometry(&mut draw_plan);

    let head = object_box(&draw_plan, "student_head");
    let output = object_box(&draw_plan, "output");
    let edge = connector(&draw_plan, "e_output");
    let edge_length: f64 = edge
        .points
        .windows(2)
        .map(|window| segment_length((window[0], window[1])))
        .sum();

    assert_eq!(
        intersection_area(head, output),
        0.0,
        "right-edge output should not keep overlapping the task head: head={head:?}, output={output:?}"
    );
    assert!(
        output[0] >= head[2] + 0.035,
        "right-edge output should leave enough connector span: head={head:?}, output={output:?}"
    );
    assert!(
        edge_length >= 0.04,
        "output connector should not remain degenerate: edge={:?}, length={edge_length}",
        edge.points
    );
}

#[test]
fn model_draw_plan_polish_separates_vertical_output_from_head() {
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "student_head".to_string(),
            bbox: [0.845, 0.6535833333333333, 0.988, 0.79975],
            text: "Student Head".to_string(),
            role: "module".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "final_output".to_string(),
            bbox: [0.8464999999999999, 0.8099999999999999, 0.9465, 0.91],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 21,
        },
        DrawObject::Connector {
            id: "e_head_to_output".to_string(),
            points: vec![[0.9165, 0.79975], [0.9165, 0.84]],
            from: Some("student_head".to_string()),
            to: Some("final_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
    ]);

    polish_model_draw_plan_geometry(&mut draw_plan);

    let head = object_box(&draw_plan, "student_head");
    let output = object_box(&draw_plan, "final_output");
    assert!(
        output[1] >= head[3] + 0.03,
        "vertical output should leave a visible gutter below the head: head={head:?}, output={output:?}"
    );
}

#[test]
fn model_draw_plan_polish_repairs_stacked_teacher_residual_smoke_layout() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher projection provides residual supervision while the student predicts with task loss.",
            "visual_focus": ["student_enc", "student_head", "teacher_enc", "teacher_proj", "latent_residual"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "input_region", "bbox": [0.0075, 0.35, 0.1075, 0.45]},
                {"id": "student_enc_region", "bbox": [0.125, 0.18, 0.35, 0.33]},
                {"id": "student_head_region", "bbox": [0.4863, 0.55, 0.65, 0.75]},
                {"id": "task_loss_region", "bbox": [0.66, 0.1325, 0.80, 0.2425]},
                {"id": "teacher_enc_region", "bbox": [0.52, 0.78, 0.75, 0.91]},
                {"id": "teacher_proj_region", "bbox": [0.52, 0.90, 0.75, 1.00]},
                {"id": "latent_region", "bbox": [0.7057, 0.2675, 0.885, 0.3675]},
                {"id": "output_region", "bbox": [0.90, 0.625, 1.00, 0.725]},
                {"id": "inference_region", "bbox": [0.39, 0.205, 0.52, 0.305]}
            ]
        },
        "components": [
            {"id": "input", "label": "x", "role": "input", "visual_weight": "normal", "region": "input_region", "allowed_asset_id": null},
            {"id": "student_enc", "label": "Student Encoder", "role": "main", "visual_weight": "strong", "region": "student_enc_region", "allowed_asset_id": null},
            {"id": "student_head", "label": "Task Head", "role": "main", "visual_weight": "strong", "region": "student_head_region", "allowed_asset_id": null},
            {"id": "task_loss", "label": "L_task", "role": "loss", "visual_weight": "normal", "region": "task_loss_region", "allowed_asset_id": null},
            {"id": "teacher_enc", "label": "Teacher Encoder", "role": "context", "visual_weight": "muted", "region": "teacher_enc_region", "allowed_asset_id": null},
            {"id": "teacher_proj", "label": "Projection", "role": "context", "visual_weight": "muted", "region": "teacher_proj_region", "allowed_asset_id": null},
            {"id": "latent_residual", "label": "L_resid", "role": "loss", "visual_weight": "normal", "region": "latent_region", "allowed_asset_id": null},
            {"id": "output", "label": "ŷ", "role": "output", "visual_weight": "normal", "region": "output_region", "allowed_asset_id": null},
            {"id": "inference_note", "label": "Inference: student only", "role": "context", "visual_weight": "muted", "region": "inference_region", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "e_student_head", "from": "student_enc", "to": "student_head", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_head_out", "from": "student_head", "to": "output", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_head_taskloss", "from": "student_head", "to": "task_loss", "label": "", "semantic": "loss", "style": "solid", "importance": "normal"},
            {"id": "e_teacher_proj", "from": "teacher_enc", "to": "teacher_proj", "label": "", "semantic": "supervision", "style": "dash", "importance": "normal"},
            {"id": "e_proj_residual", "from": "teacher_proj", "to": "latent_residual", "label": "h_t", "semantic": "supervision", "style": "dash", "importance": "main"},
            {"id": "e_student_residual", "from": "student_enc", "to": "latent_residual", "label": "h_s", "semantic": "supervision", "style": "dash", "importance": "main"}
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
            id: "input".to_string(),
            bbox: [0.0075, 0.35, 0.1075, 0.45],
            text: "x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student_enc".to_string(),
            bbox: [0.125, 0.18, 0.35, 0.33],
            text: "Student\nEncoder".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student_head".to_string(),
            bbox: [0.4863, 0.55, 0.65, 0.75],
            text: "Task\nHead".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.66, 0.1325, 0.80, 0.2425],
            text: "L_task".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "teacher_enc".to_string(),
            bbox: [0.52, 0.78, 0.75, 0.91],
            text: "Teacher\nEncoder".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "teacher_proj".to_string(),
            bbox: [0.52, 0.90, 0.75, 1.00],
            text: "Projection".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 25,
        },
        DrawObject::Box {
            id: "latent_residual".to_string(),
            bbox: [0.7057, 0.2675, 0.885, 0.3675],
            text: "L_resid".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 26,
        },
        DrawObject::Box {
            id: "output".to_string(),
            bbox: [0.90, 0.625, 1.00, 0.725],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 27,
        },
        DrawObject::Box {
            id: "inference_note".to_string(),
            bbox: [0.39, 0.205, 0.52, 0.305],
            text: "Inference: student only".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 28,
        },
        DrawObject::Connector {
            id: "e_student_head".to_string(),
            points: vec![[0.35, 0.675], [0.4863, 0.675]],
            from: Some("student_enc".to_string()),
            to: Some("student_head".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_head_out".to_string(),
            points: vec![[0.65, 0.675], [0.90, 0.675]],
            from: Some("student_head".to_string()),
            to: Some("output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_head_taskloss".to_string(),
            points: vec![[0.56815, 0.55], [0.56815, 0.2425], [0.73, 0.2425]],
            from: Some("student_head".to_string()),
            to: Some("task_loss".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "e_teacher_proj".to_string(),
            points: vec![[0.635, 0.91], [0.635, 0.925]],
            from: Some("teacher_enc".to_string()),
            to: Some("teacher_proj".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Connector {
            id: "e_proj_residual".to_string(),
            points: vec![[0.72785, 0.90], [0.72785, 0.3675]],
            from: Some("teacher_proj".to_string()),
            to: Some("latent_residual".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "h_t".to_string(),
                bbox: [0.78, 0.563, 0.94, 0.613],
            }),
            z: 14,
        },
        DrawObject::Connector {
            id: "e_student_residual".to_string(),
            points: vec![[0.35, 0.3175], [0.7057, 0.3175]],
            from: Some("student_enc".to_string()),
            to: Some("latent_residual".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "h_s".to_string(),
                bbox: [0.1575, 0.118, 0.3175, 0.168],
            }),
            z: 15,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut draw_plan, &figure_plan);

    let note = object_box(&draw_plan, "inference_note");
    assert!(
        note[2] - note[0] >= 0.16,
        "inference note should be wide enough for its longest token: note={note:?}"
    );
    let task_loss = object_box(&draw_plan, "task_loss");
    let residual = object_box(&draw_plan, "latent_residual");
    assert!(
        residual[1] - task_loss[3] >= 0.045,
        "loss stack should leave a visible gutter: task_loss={task_loss:?}, residual={residual:?}"
    );

    let teacher = object_box(&draw_plan, "teacher_enc");
    let projection = object_box(&draw_plan, "teacher_proj");
    let teacher_proj = connector(&draw_plan, "e_teacher_proj");
    let teacher_proj_len: f64 = teacher_proj
        .points
        .windows(2)
        .map(|window| segment_length((window[0], window[1])))
        .sum();
    assert!(
        projection[1] - teacher[3] >= 0.025,
        "stacked teacher boxes should not touch or overlap: teacher={teacher:?}, projection={projection:?}"
    );
    assert!(
        teacher_proj_len >= 0.04,
        "teacher stack connector should not remain degenerate: edge={:?}, length={teacher_proj_len}",
        teacher_proj.points
    );

    let proj_residual = connector(&draw_plan, "e_proj_residual");
    assert!(
        !label_intersects_any_segment(teacher, proj_residual.points),
        "teacher-to-residual edge should route outside teacher encoder: teacher={teacher:?}, edge={:?}",
        proj_residual.points
    );
    let head_out = connector(&draw_plan, "e_head_out");
    assert!(
        !connectors_cross(&head_out.points, &proj_residual.points),
        "teacher-to-residual edge should not cross main output edge: output={:?}, residual={:?}",
        head_out.points,
        proj_residual.points
    );
    let head_task = connector(&draw_plan, "e_head_taskloss");
    let student_residual = connector(&draw_plan, "e_student_residual");
    assert!(
        !connectors_cross(&head_task.points, &student_residual.points),
        "task-loss feedback should not cross student residual edge: task={:?}, residual={:?}, teacher_residual={:?}",
        head_task.points, student_residual.points, proj_residual.points
    );
}

#[test]
fn model_draw_plan_polish_moves_inference_note_away_from_loss_crowding() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Inference note should not crowd residual supervision.",
            "visual_focus": ["latent_residual_supervision", "inference_only"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "latent_region", "bbox": [0.403, 0.195, 0.597, 0.305]},
                {"id": "inference_region", "bbox": [0.415, 0.06, 0.585, 0.16]}
            ]
        },
        "components": [
            {"id": "latent_residual_supervision", "label": "Latent Residual", "role": "loss", "visual_weight": "normal", "region": "latent_region", "allowed_asset_id": null},
            {"id": "inference_only", "label": "Inference only", "role": "context", "visual_weight": "muted", "region": "inference_region", "allowed_asset_id": null}
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
            id: "latent_residual_supervision".to_string(),
            bbox: [0.403, 0.195, 0.597, 0.305],
            text: "Latent Residual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "inference_only".to_string(),
            bbox: [0.415, 0.06, 0.585, 0.1600000000000001],
            text: "Inference\nonly".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 21,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut draw_plan, &figure_plan);

    let residual = object_box(&draw_plan, "latent_residual_supervision");
    let note = object_box(&draw_plan, "inference_only");
    let x_overlap = (note[2].min(residual[2]) - note[0].max(residual[0])).max(0.0);
    let y_gap = if center_y(note) <= center_y(residual) {
        residual[1] - note[3]
    } else {
        note[1] - residual[3]
    };
    assert!(
        x_overlap <= 0.01 || y_gap >= 0.055,
        "inference note should not crowd the residual loss box: note={note:?}, residual={residual:?}, x_overlap={x_overlap}, y_gap={y_gap}"
    );
}

#[test]
fn model_draw_plan_polish_moves_residual_hub_out_of_branch_gap_crowding() {
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "c_task_input".to_string(),
            bbox: [0.08875, 0.5725, 0.22375, 0.8225],
            text: "Task Input".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "c_teacher_encoder".to_string(),
            bbox: [0.3405, 0.32, 0.847, 0.47],
            text: "Teacher Encoder".to_string(),
            role: "module".to_string(),
            style: "muted_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "c_student_encoder".to_string(),
            bbox: [0.3335, 0.58, 0.84, 0.73],
            text: "Student Encoder".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "c_latent_residual".to_string(),
            bbox: [0.69, 0.475, 0.89, 0.575],
            text: "Latent Residual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "c_task_output".to_string(),
            bbox: [0.88, 0.60, 0.98, 0.71],
            text: "Task Output".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Connector {
            id: "e_teacher_to_residual".to_string(),
            points: vec![[0.59375, 0.47], [0.59375, 0.475], [0.79, 0.475]],
            from: Some("c_teacher_encoder".to_string()),
            to: Some("c_latent_residual".to_string()),
            style: "normal_flow".to_string(),
            label: Some(DrawLabel {
                text: "h_t".to_string(),
                bbox: [0.51375, 0.258, 0.67375, 0.308],
            }),
            z: 10,
        },
        DrawObject::Connector {
            id: "e_student_to_residual".to_string(),
            points: vec![[0.58675, 0.58], [0.58675, 0.575], [0.79, 0.575]],
            from: Some("c_student_encoder".to_string()),
            to: Some("c_latent_residual".to_string()),
            style: "normal_flow".to_string(),
            label: Some(DrawLabel {
                text: "h_s".to_string(),
                bbox: [0.518, 0.5, 0.678, 0.55],
            }),
            z: 11,
        },
    ]);

    polish_model_draw_plan_geometry(&mut draw_plan);

    let residual = object_box(&draw_plan, "c_latent_residual");
    for branch_id in ["c_teacher_encoder", "c_student_encoder"] {
        let branch = object_box(&draw_plan, branch_id);
        let x_overlap = (residual[2].min(branch[2]) - residual[0].max(branch[0])).max(0.0);
        let y_gap = if center_y(residual) <= center_y(branch) {
            branch[1] - residual[3]
        } else {
            residual[1] - branch[3]
        };
        assert!(
            x_overlap <= 0.01 || y_gap >= 0.055,
            "residual hub should not crowd branch {branch_id}: residual={residual:?}, branch={branch:?}, x_overlap={x_overlap}, y_gap={y_gap}"
        );
    }
}

#[test]
fn model_draw_plan_polish_simplifies_residual_and_output_routes_from_review_feedback() {
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "teacher_transform".to_string(),
            bbox: [0.11133333333333333, 0.379, 0.347, 0.597],
            text: "Teacher\n(Large LM)".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student_transform".to_string(),
            bbox: [0.653, 0.3405, 0.8886666666666666, 0.597],
            text: "Student\n(Compact)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "teacher_latent".to_string(),
            bbox: [0.09733333333333331, 0.0835, 0.347, 0.1915],
            text: "Latent h_T".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "student_output".to_string(),
            bbox: [0.646, 0.0835, 0.8956666666666666, 0.1915],
            text: "Output ŷ".to_string(),
            role: "output".to_string(),
            style: "primary_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.7208333333333332, 0.2415, 0.8208333333333333, 0.3495],
            text: "L_task".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 24,
        },
        DrawObject::Connector {
            id: "student_to_output".to_string(),
            points: vec![
                [0.653, 0.46875],
                [0.8956666666666666, 0.46875],
                [0.8956666666666666, 0.1375],
            ],
            from: Some("student_transform".to_string()),
            to: Some("student_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "output_to_loss".to_string(),
            points: vec![[0.7708333333333333, 0.1915], [0.7708333333333333, 0.2415]],
            from: Some("student_output".to_string()),
            to: Some("task_loss".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "latent_residual".to_string(),
            points: vec![
                [0.347, 0.1375],
                [0.347, 0.2165],
                [0.653, 0.2165],
                [0.653, 0.46875],
            ],
            from: Some("teacher_latent".to_string()),
            to: Some("student_transform".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "L_residual".to_string(),
                bbox: [0.42, 0.1485, 0.58, 0.1985],
            }),
            z: 12,
        },
    ]);

    polish_model_draw_plan_geometry(&mut draw_plan);

    let task_loss = object_box(&draw_plan, "task_loss");
    let student = object_box(&draw_plan, "student_transform");
    let output = object_box(&draw_plan, "student_output");
    assert!(
        task_loss[3] <= output[3] + 0.03 || task_loss[1] >= student[3] + 0.03,
        "task loss should not sit in the narrow output/student corridor: task_loss={task_loss:?}, output={output:?}, student={student:?}"
    );

    let student_output = connector(&draw_plan, "student_to_output");
    assert_eq!(
        student_output.points.len(),
        2,
        "student_to_output should be a direct vertical connector: {:?}",
        student_output.points
    );
    assert!(
        (student_output.points[0][0] - student_output.points[1][0]).abs() < 0.0001,
        "student_to_output should be vertical: {:?}",
        student_output.points
    );

    let output_loss = connector(&draw_plan, "output_to_loss");
    assert_eq!(
        output_loss.points.len(),
        2,
        "output_to_loss should be a direct side connector after moving task_loss: {:?}",
        output_loss.points
    );
    assert!(
        (output_loss.points[0][1] - output_loss.points[1][1]).abs() < 0.0001,
        "output_to_loss should be horizontal: {:?}",
        output_loss.points
    );

    let residual = connector(&draw_plan, "latent_residual");
    assert_eq!(
        residual.points.len(),
        2,
        "latent residual should be a single horizontal rail: {:?}",
        residual.points
    );
    assert!(
        (residual.points[0][1] - residual.points[1][1]).abs() < 0.0001,
        "latent residual rail should be horizontal: {:?}",
        residual.points
    );
    let label = residual
        .label
        .as_ref()
        .expect("latent residual label should remain editable");
    let rail_y = residual.points[0][1];
    assert!(
        label.bbox[3] <= rail_y - 0.01 || label.bbox[1] >= rail_y + 0.01,
        "latent residual label should be off the dashed rail: label={:?}, rail_y={rail_y}",
        label.bbox
    );
}

#[test]
fn model_draw_plan_polish_reroutes_task_label_edge_around_student_latent() {
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input".to_string(),
            bbox: [0.06, 0.42, 0.16, 0.58],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student_latent".to_string(),
            bbox: [0.54, 0.70, 0.70, 0.86],
            text: "h_S".to_string(),
            role: "data".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.76, 0.75, 0.92, 0.87],
            text: "L_task".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 22,
        },
        DrawObject::Connector {
            id: "e_task_label".to_string(),
            points: vec![[0.16, 0.50], [0.16, 0.81], [0.76, 0.81]],
            from: Some("input".to_string()),
            to: Some("task_loss".to_string()),
            style: "normal_flow".to_string(),
            label: Some(DrawLabel {
                text: "y".to_string(),
                bbox: [0.08, 0.84, 0.14, 0.89],
            }),
            z: 10,
        },
    ]);

    polish_model_draw_plan_geometry(&mut draw_plan);

    let student_latent = object_box(&draw_plan, "student_latent");
    let edge = connector(&draw_plan, "e_task_label");
    assert!(
        !label_intersects_any_segment(student_latent, edge.points),
        "task label edge should route around student latent instead of through it: latent={student_latent:?}, edge={:?}",
        edge.points
    );
}

#[test]
fn model_draw_plan_polish_compacts_oversized_short_loss_box() {
    let mut draw_plan = minimal_draw_plan(vec![DrawObject::Box {
        id: "comp_task_loss".to_string(),
        bbox: [0.06966666666666667, 0.0905, 0.38866666666666666, 0.2845],
        text: "Task Loss".to_string(),
        role: "loss".to_string(),
        style: "accent_module".to_string(),
        z: 20,
    }]);

    polish_model_draw_plan_geometry(&mut draw_plan);

    let task_loss = object_box(&draw_plan, "comp_task_loss");
    assert!(
        task_loss[2] - task_loss[0] <= 0.20,
        "short loss label should not remain in a wide empty container: {task_loss:?}"
    );
    assert!(
        task_loss[3] - task_loss[1] <= 0.14,
        "short loss label should not remain in a tall empty container: {task_loss:?}"
    );
}

#[test]
fn model_draw_plan_polish_moves_figure_plan_annotation_off_component() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Branch labels must not cover loss boxes.",
            "visual_focus": ["comp_teacher", "comp_task_loss"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "teacher_region", "bbox": [0.12, 0.55, 0.34, 0.73]},
                {"id": "loss_region", "bbox": [0.07, 0.09, 0.39, 0.28]}
            ]
        },
        "components": [
            {"id": "comp_teacher", "label": "Teacher", "role": "context", "visual_weight": "normal", "region": "teacher_region", "allowed_asset_id": null},
            {"id": "comp_task_loss", "label": "Task Loss", "role": "loss", "visual_weight": "normal", "region": "loss_region", "allowed_asset_id": null}
        ],
        "edges": [],
        "annotations": [
            {"id": "anno_teacher_label", "label": "Large LM", "target_id": "comp_teacher", "bbox": [0.125, 0.25, 0.4166666666666667, 0.55]}
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
            id: "comp_teacher".to_string(),
            bbox: [0.12, 0.55, 0.34, 0.73],
            text: "Teacher\n(frozen)".to_string(),
            role: "context".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "comp_task_loss".to_string(),
            bbox: [0.06966666666666667, 0.0905, 0.38866666666666666, 0.2845],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 21,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut draw_plan, &figure_plan);

    let annotation = text_box(&draw_plan, "anno_teacher_label");
    let task_loss = object_box(&draw_plan, "comp_task_loss");
    assert!(
        intersection_area(annotation, task_loss) == 0.0,
        "upserted figure-plan annotation should move off component text: annotation={annotation:?}, task_loss={task_loss:?}"
    );
}

#[test]
fn model_draw_plan_polish_moves_model_authored_annotations_off_components() {
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "student_model".to_string(),
            bbox: [0.07, 0.60, 0.305, 0.905],
            text: "Student\n(Compact LM)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 20,
        },
        DrawObject::Text {
            id: "ann_teacher_label".to_string(),
            bbox: [0.11499999999999998, 0.7725, 0.27499999999999997, 0.8425],
            text: "Frozen / no gradients".to_string(),
            style: "annotation".to_string(),
            z: 21,
        },
        DrawObject::Text {
            id: "ann_dashed_meaning".to_string(),
            bbox: [0.11499999999999998, 0.7725, 0.27499999999999997, 0.8425],
            text: "Supervision signal (not gradient path)".to_string(),
            style: "annotation".to_string(),
            z: 22,
        },
    ]);

    polish_model_draw_plan_geometry(&mut draw_plan);

    let student = object_box(&draw_plan, "student_model");
    for id in ["ann_teacher_label", "ann_dashed_meaning"] {
        let annotation = text_box(&draw_plan, id);
        assert!(
            intersection_area(annotation, student) == 0.0,
            "model-authored annotation {id} should move outside student component: annotation={annotation:?}, student={student:?}"
        );
    }
}

#[test]
fn model_draw_plan_polish_moves_connector_label_off_endpoint_component() {
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "teacher_model".to_string(),
            bbox: [0.07, 0.20, 0.305, 0.38],
            text: "Teacher\n(Large LM)".to_string(),
            role: "module".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "latent_residual".to_string(),
            bbox: [0.35, 0.25, 0.55, 0.44],
            text: "Latent Residual\nSupervision".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 21,
        },
        DrawObject::Connector {
            id: "e_teacher_to_residual".to_string(),
            points: vec![[0.305, 0.345], [0.35, 0.345]],
            from: Some("teacher_model".to_string()),
            to: Some("latent_residual".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "latent".to_string(),
                bbox: [0.127, 0.31999999999999995, 0.28700000000000003, 0.37],
            }),
            z: 10,
        },
    ]);

    polish_model_draw_plan_geometry(&mut draw_plan);

    let teacher = object_box(&draw_plan, "teacher_model");
    let edge = connector(&draw_plan, "e_teacher_to_residual");
    let label = edge.label.as_ref().expect("connector label should remain");
    assert!(
        intersection_area(label.bbox, teacher) == 0.0,
        "connector label should move outside endpoint component: label={:?}, teacher={teacher:?}",
        label.bbox
    );
}

#[test]
fn model_draw_plan_polish_moves_figure_plan_annotation_off_connector() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Training-only annotation must not cover the teacher-student supervision edge.",
            "visual_focus": ["comp_teacher", "comp_student"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "teacher_region", "bbox": [0.28, 0.12, 0.64, 0.32]},
                {"id": "student_region", "bbox": [0.28, 0.54, 0.64, 0.74]}
            ]
        },
        "components": [
            {"id": "comp_teacher", "label": "Teacher", "role": "context", "visual_weight": "normal", "region": "teacher_region", "allowed_asset_id": null},
            {"id": "comp_student", "label": "Student", "role": "main", "visual_weight": "strong", "region": "student_region", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "e_teacher_student_dashed", "from": "comp_teacher", "to": "comp_student", "label": "", "semantic": "supervision", "style": "dash", "importance": "main"}
        ],
        "annotations": [
            {"id": "anno_train_only", "label": "training only", "target_id": "comp_student", "bbox": [0.3883333333333333, 0.332, 0.5283333333333333, 0.41200000000000003]}
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
            id: "comp_teacher".to_string(),
            bbox: [0.28, 0.12, 0.64, 0.32],
            text: "Teacher\n(frozen)".to_string(),
            role: "context".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "comp_student".to_string(),
            bbox: [0.28, 0.54, 0.64, 0.74],
            text: "Student\n(trainable)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Connector {
            id: "e_teacher_student_dashed".to_string(),
            points: vec![[0.4583333333333333, 0.32], [0.4583333333333333, 0.54]],
            from: Some("comp_teacher".to_string()),
            to: Some("comp_student".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 10,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut draw_plan, &figure_plan);

    let annotation = text_box(&draw_plan, "anno_train_only");
    let supervision_edge = connector(&draw_plan, "e_teacher_student_dashed");
    assert!(
        !label_touches_any_segment(annotation, supervision_edge.points),
        "upserted annotation should move off the supervision edge: annotation={annotation:?}, edge={:?}",
        supervision_edge.points
    );
}

#[test]
fn model_draw_plan_polish_reroutes_connector_around_crossing_edges() {
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_teacher".to_string(),
            bbox: [0.12, 0.55, 0.34, 0.73],
            text: "Teacher\n(frozen)".to_string(),
            role: "context".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "comp_student".to_string(),
            bbox: [0.66, 0.55, 0.88, 0.73],
            text: "Student\n(trainable)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "comp_latent_residual".to_string(),
            bbox: [0.42, 0.05, 0.58, 0.18],
            text: "Latent Residual\n+ Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "comp_task_loss".to_string(),
            bbox: [0.06966666666666667, 0.0905, 0.38866666666666666, 0.2845],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Connector {
            id: "edge_teacher_to_residual".to_string(),
            points: vec![[0.34, 0.64], [0.34, 0.3775], [0.42, 0.3775], [0.42, 0.115]],
            from: Some("comp_teacher".to_string()),
            to: Some("comp_latent_residual".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "edge_residual_to_student".to_string(),
            points: vec![[0.58, 0.115], [0.66, 0.115], [0.66, 0.55]],
            from: Some("comp_latent_residual".to_string()),
            to: Some("comp_student".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "edge_student_to_task_loss".to_string(),
            points: vec![
                [0.8125, 0.3405],
                [0.8125, 0.2845],
                [0.22916666666666663, 0.2845],
            ],
            from: Some("comp_student".to_string()),
            to: Some("comp_task_loss".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 12,
        },
    ]);

    polish_model_draw_plan_geometry(&mut draw_plan);

    let task_edge = connector(&draw_plan, "edge_student_to_task_loss");
    let teacher_residual = connector(&draw_plan, "edge_teacher_to_residual");
    let residual_student = connector(&draw_plan, "edge_residual_to_student");
    assert!(
        !connectors_cross(task_edge.points, teacher_residual.points),
        "task-loss connector should not cross teacher residual edge: task={:?}, residual={:?}",
        task_edge.points,
        teacher_residual.points
    );
    assert!(
        !connectors_cross(task_edge.points, residual_student.points),
        "task-loss connector should not cross residual-to-student edge: task={:?}, residual={:?}",
        task_edge.points,
        residual_student.points
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
    let residual_box = object_box(&draw_plan, "latent_residual");
    assert_point_close(
        residual_student.points[0],
        [residual_box[0], residual_box[3]],
    );
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

    let residual = object_box(&draw_plan, "comp_latent_residual");
    if box_object_exists(&draw_plan, "comp_inference_note") {
        let note = object_box(&draw_plan, "comp_inference_note");
        assert_eq!(
            intersection_area(note, expand_box(residual, 0.04)),
            0.0,
            "inference note should leave whitespace around the residual objective: note={note:?}, residual={residual:?}"
        );
    } else {
        assert!(
            draw_plan.objects.iter().any(|object| matches!(
                object,
                DrawObject::Text { text, .. } if text.to_lowercase().contains("inference")
            )),
            "folding an inference note out of the residual gap must preserve an editable inference annotation"
        );
    }
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
fn model_draw_plan_polish_repairs_outer_margin_input_and_output_routes_from_latest_smoke() {
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "task_input".to_string(),
            bbox: [0.065, 0.881, 0.185, 0.981],
            text: "Task Input".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher".to_string(),
            bbox: [0.08, 0.25, 0.42, 0.45],
            text: "Teacher\n(frozen)".to_string(),
            role: "module".to_string(),
            style: "muted_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student".to_string(),
            bbox: [0.55, 0.25, 0.85, 0.45],
            text: "Student\n(trainable)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "task_output".to_string(),
            bbox: [0.86, 0.881, 0.985, 0.981],
            text: "Task Output".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "latent_residual_alignment".to_string(),
            bbox: [0.475, 0.5, 0.645, 0.63],
            text: "Latent Residual\nAlignment".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 24,
        },
        DrawObject::Connector {
            id: "e_input_to_teacher".to_string(),
            points: vec![[0.125, 0.881], [0.125, 0.45]],
            from: Some("task_input".to_string()),
            to: Some("teacher".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_input_to_student".to_string(),
            points: vec![[0.185, 0.931], [0.68, 0.931], [0.68, 0.45], [0.7, 0.45]],
            from: Some("task_input".to_string()),
            to: Some("student".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_residual_to_student".to_string(),
            points: vec![[0.645, 0.5], [0.645, 0.35], [0.55, 0.35]],
            from: Some("latent_residual_alignment".to_string()),
            to: Some("student".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 14,
        },
        DrawObject::Connector {
            id: "e_student_to_output".to_string(),
            points: vec![[0.85, 0.35], [0.85, 0.886], [0.925, 0.886]],
            from: Some("student".to_string()),
            to: Some("task_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 15,
        },
    ]);

    polish_model_draw_plan_geometry(&mut draw_plan);

    let input = object_box(&draw_plan, "task_input");
    let student = object_box(&draw_plan, "student");
    let output = object_box(&draw_plan, "task_output");
    assert!(
        input[1] < 0.72,
        "shared input should move off the bottom margin instead of driving a long outer L route: input={input:?}, student={student:?}"
    );
    assert!(
        (center_y(output) - center_y(student)).abs() < 0.08,
        "output should align with student for a direct rightward flow instead of staying on the bottom margin: output={output:?}, student={student:?}"
    );

    let input_edge = connector(&draw_plan, "e_input_to_student");
    assert!(
        points_to_box_for_test(input_edge.points)[3] < 0.78,
        "input-to-student route should not keep a bottom-margin L detour: {:?}",
        input_edge.points
    );
    let output_edge = connector(&draw_plan, "e_student_to_output");
    assert!(
        points_to_box_for_test(output_edge.points)[3]
            - points_to_box_for_test(output_edge.points)[1]
            < 0.18,
        "student-to-output route should not keep a right-edge U detour: {:?}",
        output_edge.points
    );
}

#[test]
fn model_draw_plan_polish_repairs_teacher_student_reverse_layout_from_latest_smoke() {
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input".to_string(),
            bbox: [0.065, 0.44, 0.185, 0.6016666666666668],
            text: "Task Input".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student".to_string(),
            bbox: [0.06966666666666665, 0.5975, 0.26366666666666666, 0.7775],
            text: "Student\n(Compact)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "teacher".to_string(),
            bbox: [0.65, 0.45, 0.9, 0.6],
            text: "Teacher\n(Large)".to_string(),
            role: "module".to_string(),
            style: "neutral_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.418, 0.7825, 0.582, 0.8925000000000001],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "residual".to_string(),
            bbox: [0.68, 0.31, 0.87, 0.41],
            text: "Latent Residual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "joint_loss".to_string(),
            bbox: [0.45, 0.45, 0.62, 0.55],
            text: "Joint Objective".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 25,
        },
        DrawObject::Connector {
            id: "e_input_student".to_string(),
            points: vec![
                [0.125, 0.6016666666666668],
                [0.125, 0.7775],
                [0.16666666666666666, 0.7775],
            ],
            from: Some("input".to_string()),
            to: Some("student".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_input_teacher".to_string(),
            points: vec![
                [0.185, 0.5208333333333334],
                [0.0425, 0.5208333333333334],
                [0.0425, 0.9],
                [0.65, 0.9],
                [0.65, 0.6],
            ],
            from: Some("input".to_string()),
            to: Some("teacher".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_student_task".to_string(),
            points: vec![
                [0.26366666666666666, 0.6875],
                [0.26366666666666666, 0.8375],
                [0.418, 0.8375],
            ],
            from: Some("student".to_string()),
            to: Some("task_loss".to_string()),
            style: "normal_flow".to_string(),
            label: Some(DrawLabel {
                text: "prediction".to_string(),
                bbox: [0.08566666666666665, 0.8125, 0.24566666666666664, 0.8625],
            }),
            z: 12,
        },
        DrawObject::Connector {
            id: "e_teacher_residual".to_string(),
            points: vec![[0.775, 0.45], [0.775, 0.41]],
            from: Some("teacher".to_string()),
            to: Some("residual".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "latent".to_string(),
                bbox: [0.7050000000000001, 0.612, 0.8450000000000001, 0.662],
            }),
            z: 13,
        },
        DrawObject::Connector {
            id: "e_task_joint".to_string(),
            points: vec![[0.5, 0.7825], [0.625, 0.7825], [0.625, 0.55], [0.535, 0.55]],
            from: Some("task_loss".to_string()),
            to: Some("joint_loss".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 14,
        },
        DrawObject::Connector {
            id: "e_residual_joint".to_string(),
            points: vec![[0.775, 0.36], [0.535, 0.36], [0.535, 0.45]],
            from: Some("residual".to_string()),
            to: Some("joint_loss".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 15,
        },
        DrawObject::Connector {
            id: "e_joint_student".to_string(),
            points: vec![
                [0.45, 0.5],
                [0.26366666666666666, 0.5],
                [0.26366666666666666, 0.5975],
            ],
            from: Some("joint_loss".to_string()),
            to: Some("student".to_string()),
            style: "main_flow".to_string(),
            label: Some(DrawLabel {
                text: "optimize".to_string(),
                bbox: [
                    0.26366666666666666,
                    0.42599999999999993,
                    0.38066666666666665,
                    0.476,
                ],
            }),
            z: 16,
        },
        DrawObject::Box {
            id: "inference".to_string(),
            bbox: [0.39, 0.06, 0.61, 0.2],
            text: "Inference\n(Student Only)".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 26,
        },
        DrawObject::Connector {
            id: "e_student_inference".to_string(),
            points: vec![[0.26366666666666666, 0.6875], [0.5, 0.2]],
            from: Some("student".to_string()),
            to: Some("inference".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 17,
        },
    ]);

    polish_model_draw_plan_geometry(&mut draw_plan);

    let input_teacher = connector(&draw_plan, "e_input_teacher");
    assert!(
        points_to_box_for_test(input_teacher.points)[3] <= 0.62,
        "input-to-teacher should not keep the bottom-margin dogleg from the smoke final: {:?}",
        input_teacher.points
    );
    let student = object_box(&draw_plan, "student");
    let inference = object_box(&draw_plan, "inference");
    assert!(
        vertical_separation(student, inference) <= 0.02,
        "student-only inference should sit beside the student, not at the top margin: student={student:?}, inference={inference:?}"
    );
    let student_inference = connector(&draw_plan, "e_student_inference");
    assert!(
        student_inference.points.windows(2).all(|window| {
            (window[0][0] - window[1][0]).abs() < 0.001
                || (window[0][1] - window[1][1]).abs() < 0.001
        }),
        "student-to-inference connector should be orthogonal, not a diagonal crossing: {:?}",
        student_inference.points
    );
    let joint_student = connector(&draw_plan, "e_joint_student");
    assert!(
        !connectors_cross(joint_student.points, student_inference.points),
        "objective feedback should not cross the student inference path: {:?} vs {:?}",
        joint_student.points,
        student_inference.points
    );
    let teacher = object_box(&draw_plan, "teacher");
    let residual = object_box(&draw_plan, "residual");
    let joint_loss = object_box(&draw_plan, "joint_loss");
    assert!(
        vertical_separation(teacher, residual) >= 0.055,
        "teacher and residual need a visible gutter: teacher={teacher:?}, residual={residual:?}"
    );
    assert!(
        horizontal_separation(teacher, joint_loss) >= 0.055,
        "teacher and joint objective need a visible horizontal gutter: teacher={teacher:?}, joint={joint_loss:?}"
    );
    let teacher_residual = connector(&draw_plan, "e_teacher_residual");
    let label = teacher_residual
        .label
        .as_ref()
        .expect("latent label should stay editable");
    assert!(
        label_near_any_segment(label.bbox, teacher_residual.points, 0.04),
        "latent label should snap near its connector instead of floating below teacher: {:?} vs {:?}",
        label.bbox,
        teacher_residual.points
    );
}

#[test]
fn model_draw_plan_polish_repairs_annotation_and_residual_stack_from_latest_smoke() {
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "task_input".to_string(),
            bbox: [0.06791666666666667, 0.05, 0.22375, 0.185],
            text: "Task input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student_encoder".to_string(),
            bbox: [0.07622916666666668, 0.22, 0.29622916666666665, 0.355],
            text: "Student encoder".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "student_latent".to_string(),
            bbox: [0.05583333333333332, 0.38, 0.2358333333333333, 0.515],
            text: "Student latent h_S".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "task_head".to_string(),
            bbox: [0.2758333333333333, 0.38, 0.4158333333333333, 0.515],
            text: "Task head".to_string(),
            role: "module".to_string(),
            style: "neutral_module".to_string(),
            z: 26,
        },
        DrawObject::Box {
            id: "student_output".to_string(),
            bbox: [0.4758333333333333, 0.38, 0.6158333333333333, 0.515],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 28,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.6758333333333333, 0.38, 0.8458333333333333, 0.49],
            text: "Task loss L_task".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 27,
        },
        DrawObject::Box {
            id: "teacher_encoder".to_string(),
            bbox: [
                0.5508333333333333,
                0.09206249999999996,
                0.7308333333333333,
                0.22706249999999997,
            ],
            text: "Teacher encoder".to_string(),
            role: "main".to_string(),
            style: "muted_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "teacher_latent".to_string(),
            bbox: [0.5508333333333333, 0.2670625, 0.7308333333333334, 0.4020625],
            text: "Teacher latent h_T".to_string(),
            role: "module".to_string(),
            style: "muted_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "residual_supervision".to_string(),
            bbox: [0.31791666666666674, 0.85625, 0.72375, 0.95625],
            text: "Latent residual\nL_res = ||h_S - h_T||^2".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 25,
        },
        DrawObject::Connector {
            id: "e_task_loss".to_string(),
            points: vec![
                [0.6158333333333333, 0.43500000000000005],
                [0.6508333333333334, 0.43500000000000005],
                [0.6508333333333334, 0.49],
                [0.6758333333333333, 0.49],
            ],
            from: Some("student_output".to_string()),
            to: Some("task_loss".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 18,
        },
        DrawObject::Connector {
            id: "e_residual".to_string(),
            points: vec![
                [0.6408333333333334, 0.4020625],
                [0.6408333333333334, 0.85625],
            ],
            from: Some("teacher_latent".to_string()),
            to: Some("residual_supervision".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "supervise".to_string(),
                bbox: [
                    0.6588333333333334,
                    0.60415625,
                    0.8188333333333333,
                    0.65415625,
                ],
            }),
            z: 14,
        },
        DrawObject::Connector {
            id: "e_residual_student".to_string(),
            points: vec![
                [0.2358333333333333, 0.515],
                [0.2358333333333333, 0.7],
                [0.45, 0.7],
                [0.45, 0.86875],
                [0.5208333333333334, 0.86875],
            ],
            from: Some("student_latent".to_string()),
            to: Some("residual_supervision".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 15,
        },
        DrawObject::Connector {
            id: "e_head_output".to_string(),
            points: vec![[0.4158333333333333, 0.4475], [0.4758333333333333, 0.4475]],
            from: Some("task_head".to_string()),
            to: Some("student_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 17,
        },
        DrawObject::Text {
            id: "anno_teacher_frozen".to_string(),
            bbox: [0.29833333333333334, 0.425, 0.45833333333333337, 0.495],
            text: "Frozen / train only".to_string(),
            style: "annotation".to_string(),
            z: 31,
        },
        DrawObject::Text {
            id: "ann_inference".to_string(),
            bbox: [
                0.29833333333333334,
                0.43500000000000005,
                0.45833333333333337,
                0.495,
            ],
            text: "Inference: student only".to_string(),
            style: "annotation".to_string(),
            z: 32,
        },
        DrawObject::Text {
            id: "anno_student_primary".to_string(),
            bbox: [0.057833333333333306, 0.718, 0.21783333333333332, 0.788],
            text: "Deployed model".to_string(),
            style: "annotation".to_string(),
            z: 33,
        },
    ]);

    polish_model_draw_plan_geometry(&mut draw_plan);

    assert!(
        !text_object_exists(&draw_plan, "ann_inference"),
        "overlapping student-only inference annotation should be removed or folded into existing branch text"
    );
    if text_object_exists(&draw_plan, "anno_teacher_frozen") {
        let teacher_note = text_box(&draw_plan, "anno_teacher_frozen");
        let task_head = object_box(&draw_plan, "task_head");
        assert!(
            intersection_area(teacher_note, task_head) <= 0.0001,
            "teacher note should not sit on task head: {teacher_note:?} vs {task_head:?}"
        );
    }
    let task_input = object_box(&draw_plan, "task_input");
    let student_encoder = object_box(&draw_plan, "student_encoder");
    assert!(
        vertical_separation(task_input, student_encoder) >= 0.045,
        "task input and student encoder need a readable vertical gutter: {task_input:?} vs {student_encoder:?}"
    );
    let teacher_encoder = object_box(&draw_plan, "teacher_encoder");
    let teacher_latent = object_box(&draw_plan, "teacher_latent");
    assert!(
        vertical_separation(teacher_encoder, teacher_latent) >= 0.045,
        "teacher encoder and latent box need a readable vertical gutter: {teacher_encoder:?} vs {teacher_latent:?}"
    );
    let student_latent = object_box(&draw_plan, "student_latent");
    assert!(
        vertical_separation(student_encoder, student_latent) >= 0.045,
        "student encoder and latent box need a readable vertical gutter: {student_encoder:?} vs {student_latent:?}"
    );
    let residual = object_box(&draw_plan, "residual_supervision");
    assert!(
        residual[2] - residual[0] <= 0.32,
        "residual supervision should not remain a wide bottom slab: {residual:?}"
    );
    let residual_edge = connector(&draw_plan, "e_residual_student");
    assert!(
        residual_edge.points.len() <= 3,
        "residual-to-student supervision route should be a compact elbow, not a 5-point detour: {:?}",
        residual_edge.points
    );
    let task_edge = connector(&draw_plan, "e_task_loss");
    assert!(
        task_edge.points.len() <= 3,
        "student output to task loss should be direct, not a 4-point hook: {:?}",
        task_edge.points
    );
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
    let student = object_box(&draw_plan, "comp_student");
    assert_eq!(edge.points.len(), 3, "{:?}", edge.points);
    assert_point_close(edge.points[1], [student[2], 0.43]);
    assert_point_close(edge.points[2], [student[2], student[1]]);
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
fn model_draw_plan_polish_folds_standalone_inference_component_from_smoke() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher supervises a compact student with student-only inference.",
            "visual_focus": ["comp_teacher", "comp_student", "comp_inference"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 8},
            "regions": [
                {"id": "teacher_region", "bbox": [0.262, 0.30, 0.488, 0.48]},
                {"id": "student_region", "bbox": [0.262, 0.66, 0.488, 0.84]},
                {"id": "inference_note", "bbox": [0.565, 0.10666666666666666, 0.935, 0.22666666666666663]}
            ]
        },
        "components": [
            {"id": "comp_teacher", "label": "Teacher", "role": "context", "visual_weight": "muted", "region": "teacher_region", "allowed_asset_id": null},
            {"id": "comp_student", "label": "Student\\n(compact)", "role": "main", "visual_weight": "strong", "region": "student_region", "allowed_asset_id": null},
            {"id": "comp_inference", "label": "Inference: student only", "role": "context", "visual_weight": "muted", "region": "inference_note", "allowed_asset_id": null}
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
    .expect("smoke-derived figure plan should deserialize");
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_teacher".to_string(),
            bbox: [0.262, 0.30, 0.488, 0.48],
            text: "Teacher".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "comp_student".to_string(),
            bbox: [0.262, 0.66, 0.488, 0.84],
            text: "Student\n(compact)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "comp_inference".to_string(),
            bbox: [0.565, 0.10666666666666666, 0.935, 0.22666666666666663],
            text: "Inference: student only".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 26,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    assert!(
        !box_object_exists(&plan, "comp_inference"),
        "standalone protected inference components should not remain as unanchored lanes"
    );
    assert!(
        draw_plan_has_text(&plan, "ann_inference", "student only"),
        "folding the component must preserve inference semantics as compact editable text"
    );
}

#[test]
fn model_draw_plan_polish_separates_thin_visible_loss_residual_overlap_from_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "teacher_residual_out".to_string(),
            bbox: [0.757, 0.5262499999999999, 0.929, 0.6362499999999999],
            text: "Latent\nresidual r".to_string(),
            role: "module".to_string(),
            style: "accent_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "task_loss_box".to_string(),
            bbox: [0.7879999999999999, 0.618, 0.94, 0.728],
            text: "Task loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 25,
        },
        DrawObject::Connector {
            id: "e_residual_supervise".to_string(),
            points: vec![[0.757, 0.6362499999999999], [0.722, 0.81]],
            from: Some("teacher_residual_out".to_string()),
            to: Some("student_model".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 28,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let residual = object_box(&plan, "teacher_residual_out");
    let loss = object_box(&plan, "task_loss_box");
    assert!(
        intersection_area(residual, loss) <= f64::EPSILON,
        "thin visible smoke overlap should be fully separated: residual={residual:?}, loss={loss:?}"
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
fn repair_draw_plan_geometry_resnaps_multimodal_fanin_labels_from_mock_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "vision_encoder".to_string(),
            bbox: [0.088, 0.29156, 0.27752, 0.44644],
            text: "Vision Enc.".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "text_encoder".to_string(),
            bbox: [0.088, 0.53356, 0.27752, 0.68844],
            text: "Text Enc.".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "fusion".to_string(),
            bbox: [0.42172, 0.41256, 0.61124, 0.56744],
            text: "Fusion".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Connector {
            id: "vision_to_fusion".to_string(),
            points: vec![
                [0.18276, 0.369],
                [0.34962, 0.369],
                [0.34962, 0.49],
                [0.51648, 0.49],
            ],
            from: Some("vision_encoder".to_string()),
            to: Some("fusion".to_string()),
            style: "main_flow".to_string(),
            label: Some(DrawLabel {
                text: "visual tokens".to_string(),
                bbox: [0.26962, 0.689, 0.42962, 0.739],
            }),
            z: 10,
        },
        DrawObject::Connector {
            id: "text_to_fusion".to_string(),
            points: vec![
                [0.18276, 0.611],
                [0.34962, 0.611],
                [0.34962, 0.49],
                [0.51648, 0.49],
            ],
            from: Some("text_encoder".to_string()),
            to: Some("fusion".to_string()),
            style: "main_flow".to_string(),
            label: Some(DrawLabel {
                text: "text tokens".to_string(),
                bbox: [0.26962, 0.770, 0.42962, 0.820],
            }),
            z: 10,
        },
    ]);

    repair_draw_plan_geometry(&mut plan);

    for id in ["vision_to_fusion", "text_to_fusion"] {
        let edge = connector(&plan, id);
        let label = edge
            .label
            .as_ref()
            .expect("fan-in connector label should remain");
        assert!(
            label_near_any_segment(label.bbox, edge.points, 0.08),
            "{id} label should be near its explicit connector after repair: label={:?}, points={:?}",
            label.bbox,
            edge.points
        );
        assert!(
            !label_touches_any_segment(label.bbox, edge.points),
            "{id} label should not sit on top of its connector stroke: label={:?}, points={:?}",
            label.bbox,
            edge.points
        );
    }
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
fn model_draw_plan_polish_moves_shared_input_to_clear_middle_slot_from_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_teacher_encoder".to_string(),
            bbox: [0.06966666666666665, 0.3075, 0.36966666666666664, 0.4425],
            text: "Teacher Encoder".to_string(),
            role: "module".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "comp_teacher_head".to_string(),
            bbox: [
                0.09966666666666665,
                0.6616666666666666,
                0.3396666666666667,
                0.7966666666666666,
            ],
            text: "Latent Head".to_string(),
            role: "module".to_string(),
            style: "neutral_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "comp_input".to_string(),
            bbox: [
                0.3821666666666667,
                0.27366666666666667,
                0.48216666666666674,
                0.42366666666666664,
            ],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "comp_student_encoder".to_string(),
            bbox: [
                0.6588333333333333,
                0.2548333333333333,
                0.8988333333333333,
                0.3898333333333333,
            ],
            text: "Student Encoder".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "comp_student_head".to_string(),
            bbox: [
                0.7708333333333333,
                0.43749999999999994,
                0.9628333333333332,
                0.5375,
            ],
            text: "Prediction Head".to_string(),
            role: "output".to_string(),
            style: "primary_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "comp_output".to_string(),
            bbox: [
                0.5299999999999999,
                0.6791666666666666,
                0.6299999999999999,
                0.7791666666666667,
            ],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "primary_module".to_string(),
            z: 25,
        },
        DrawObject::Box {
            id: "comp_latent_residual".to_string(),
            bbox: [0.3685, 0.44999999999999996, 0.5015000000000001, 0.55],
            text: "Latent Residual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 26,
        },
        DrawObject::Box {
            id: "comp_task_loss".to_string(),
            bbox: [0.776, 0.85, 0.94, 0.96],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 27,
        },
        DrawObject::Connector {
            id: "edge_input_to_teacher".to_string(),
            points: vec![
                [0.3821666666666667, 0.3486666666666667],
                [0.36966666666666664, 0.3486666666666667],
                [0.36966666666666664, 0.375],
            ],
            from: Some("comp_input".to_string()),
            to: Some("comp_teacher_encoder".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "edge_input_to_student".to_string(),
            points: vec![
                [0.48216666666666674, 0.3223333333333333],
                [0.6588333333333333, 0.3223333333333333],
            ],
            from: Some("comp_input".to_string()),
            to: Some("comp_student_encoder".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "edge_student_encode".to_string(),
            points: vec![
                [0.8668333333333332, 0.3898333333333333],
                [0.8668333333333332, 0.43749999999999994],
            ],
            from: Some("comp_student_encoder".to_string()),
            to: Some("comp_student_head".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "edge_student_predict".to_string(),
            points: vec![
                [0.7708333333333333, 0.48749999999999993],
                [0.7708333333333333, 0.7291666666666666],
                [0.6299999999999999, 0.7291666666666666],
            ],
            from: Some("comp_student_head".to_string()),
            to: Some("comp_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Connector {
            id: "edge_teacher_to_residual".to_string(),
            points: vec![
                [0.21966666666666668, 0.6616666666666666],
                [0.29466666666666663, 0.6616666666666666],
                [0.29466666666666663, 0.5],
                [0.4023333333333333, 0.5],
                [0.4023333333333333, 0.55],
                [0.43500000000000005, 0.55],
            ],
            from: Some("comp_teacher_head".to_string()),
            to: Some("comp_latent_residual".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 14,
        },
        DrawObject::Connector {
            id: "edge_student_to_residual".to_string(),
            points: vec![
                [0.6588333333333333, 0.3223333333333333],
                [0.6588333333333333, 0.5],
                [0.5015000000000001, 0.5],
            ],
            from: Some("comp_student_encoder".to_string()),
            to: Some("comp_latent_residual".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 15,
        },
        DrawObject::Connector {
            id: "edge_task_loss".to_string(),
            points: vec![[0.858, 0.5375], [0.858, 0.85]],
            from: Some("comp_student_head".to_string()),
            to: Some("comp_task_loss".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 16,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let input = object_box(&plan, "comp_input");
    let teacher = object_box(&plan, "comp_teacher_encoder");
    let residual = object_box(&plan, "comp_latent_residual");
    assert!(
        horizontal_separation(teacher, input) >= 0.045,
        "shared input should keep a real horizontal gutter from the left teacher branch: teacher={teacher:?}, input={input:?}"
    );
    assert!(
        vertical_separation(input, residual) >= 0.055
            || horizontal_separation(input, residual) >= 0.03,
        "shared input should not crowd the residual hub below it: input={input:?}, residual={residual:?}"
    );
    let input_teacher = connector(&plan, "edge_input_to_teacher");
    let longest = input_teacher
        .points
        .windows(2)
        .map(|window| segment_length((window[0], window[1])))
        .fold(0.0, f64::max);
    assert!(
        longest >= 0.045,
        "input-to-teacher edge should no longer be degenerate: {:?}",
        input_teacher.points
    );
}

#[test]
fn model_draw_plan_polish_repairs_branch_input_output_gutters_from_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "teacher_input".to_string(),
            bbox: [0.07876666666666667, 0.275, 0.1862333333333333, 0.395],
            text: "Task Input".to_string(),
            role: "input".to_string(),
            style: "muted_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher_enc".to_string(),
            bbox: [0.23043333333333332, 0.20376666666666665, 0.3379, 0.4779],
            text: "Teacher\nEncoder".to_string(),
            role: "module".to_string(),
            style: "neutral_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "teacher_latent".to_string(),
            bbox: [0.032999999999999974, 0.15, 0.232, 0.25],
            text: "Latent\nRepresentation".to_string(),
            role: "output".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "student_input".to_string(),
            bbox: [0.6621, 0.275, 0.7695666666666666, 0.395],
            text: "Task Input".to_string(),
            role: "input".to_string(),
            style: "muted_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "student_enc".to_string(),
            bbox: [0.857, 0.20376666666666665, 0.994, 0.4779],
            text: "Student\nEncoder".to_string(),
            role: "module".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "student_pred".to_string(),
            bbox: [0.6308333333333334, 0.15, 0.8008333333333333, 0.25],
            text: "Answer\nPrediction".to_string(),
            role: "output".to_string(),
            style: "primary_module".to_string(),
            z: 25,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [
                0.6658333333333333,
                0.55,
                0.7658333333333334,
                0.6500000000000001,
            ],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 26,
        },
        DrawObject::Box {
            id: "residual_supervision".to_string(),
            bbox: [
                0.4289166666666666,
                0.29999999999999993,
                0.5619166666666666,
                0.4,
            ],
            text: "Latent Residual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 27,
        },
        DrawObject::Connector {
            id: "e_t_in_enc".to_string(),
            points: vec![
                [0.1862333333333333, 0.3408333333333333],
                [0.23043333333333332, 0.3408333333333333],
            ],
            from: Some("teacher_input".to_string()),
            to: Some("teacher_enc".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_t_enc_lat".to_string(),
            points: vec![
                [0.2841666666666666, 0.20376666666666665],
                [0.2841666666666666, 0.15],
                [0.232, 0.15],
            ],
            from: Some("teacher_enc".to_string()),
            to: Some("teacher_latent".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_s_in_enc".to_string(),
            points: vec![
                [0.7695666666666666, 0.3408333333333333],
                [0.857, 0.3408333333333333],
            ],
            from: Some("student_input".to_string()),
            to: Some("student_enc".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "e_s_enc_pred".to_string(),
            points: vec![
                [0.857, 0.22688333333333333],
                [0.8008333333333333, 0.22688333333333333],
            ],
            from: Some("student_enc".to_string()),
            to: Some("student_pred".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Connector {
            id: "e_task_loss".to_string(),
            points: vec![
                [0.7158333333333333, 0.25],
                [0.6371, 0.25],
                [0.6371, 0.55],
                [0.7158333333333333, 0.55],
            ],
            from: Some("student_pred".to_string()),
            to: Some("task_loss".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 14,
        },
        DrawObject::Connector {
            id: "e_residual".to_string(),
            points: vec![[0.1325, 0.25], [0.1325, 0.35], [0.4289166666666666, 0.35]],
            from: Some("teacher_latent".to_string()),
            to: Some("residual_supervision".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 15,
        },
        DrawObject::Connector {
            id: "e_residual_to_student".to_string(),
            points: vec![
                [0.5619166666666666, 0.3408333333333333],
                [0.5958333333333333, 0.3408333333333333],
                [0.5958333333333333, 0.1],
                [0.857, 0.1],
                [0.857, 0.20376666666666665],
            ],
            from: Some("residual_supervision".to_string()),
            to: Some("student_enc".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 16,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let teacher_input = object_box(&plan, "teacher_input");
    let teacher_latent = object_box(&plan, "teacher_latent");
    let student_input = object_box(&plan, "student_input");
    let student_pred = object_box(&plan, "student_pred");
    assert!(
        vertical_separation(teacher_latent, teacher_input) >= 0.055,
        "teacher input should leave a visible gutter below teacher latent: input={teacher_input:?}, latent={teacher_latent:?}"
    );
    assert!(
        vertical_separation(student_pred, student_input) >= 0.055,
        "student input should leave a visible gutter below prediction: input={student_input:?}, pred={student_pred:?}"
    );
    assert!(
        !label_near_any_segment(teacher_input, connector(&plan, "e_residual").points, 0.004),
        "residual connector should not cross teacher input: input={teacher_input:?}, points={:?}",
        connector(&plan, "e_residual").points
    );
    let task_edge = connector(&plan, "e_task_loss");
    assert!(
        task_edge.points.len() <= 2,
        "vertically aligned output-to-task-loss edge should be direct, not a rectangular detour: {:?}",
        task_edge.points
    );
}

#[test]
fn model_draw_plan_polish_repairs_y_branch_label_note_and_input_detour_from_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input_data".to_string(),
            bbox: [
                0.10133333333333333,
                0.5209999999999999,
                0.20133333333333334,
                0.6289999999999999,
            ],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student".to_string(),
            bbox: [0.23633333333333334, 0.4655, 0.472, 0.6845],
            text: "Student\n(compact)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "teacher".to_string(),
            bbox: [0.6113333333333334, 0.4655, 0.847, 0.6845],
            text: "Teacher\n(frozen)".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "residual_alignment".to_string(),
            bbox: [
                0.4208333333333333,
                0.15999999999999998,
                0.6208333333333332,
                0.29,
            ],
            text: "Latent Residual\nAlignment".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "task_output".to_string(),
            bbox: [0.517, 0.5249999999999999, 0.617, 0.6249999999999999],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "inference_note".to_string(),
            bbox: [
                0.2741666666666666,
                0.7535,
                0.4341666666666666,
                0.8234999999999999,
            ],
            text: "Inference:\nstudent only".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 25,
        },
        DrawObject::Connector {
            id: "e_input_to_student".to_string(),
            points: vec![
                [0.15133333333333332, 0.5209999999999999],
                [0.35416666666666663, 0.5209999999999999],
                [0.35416666666666663, 0.6845],
            ],
            from: Some("input_data".to_string()),
            to: Some("student".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_input_to_teacher".to_string(),
            points: vec![
                [0.20133333333333334, 0.575],
                [0.20133333333333334, 0.9],
                [0.6113333333333334, 0.9],
                [0.6113333333333334, 0.6845],
            ],
            from: Some("input_data".to_string()),
            to: Some("teacher".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_student_to_output".to_string(),
            points: vec![[0.472, 0.575], [0.517, 0.575]],
            from: Some("student".to_string()),
            to: Some("task_output".to_string()),
            style: "main_flow".to_string(),
            label: Some(DrawLabel {
                text: "Task loss".to_string(),
                bbox: [0.4145, 0.4699999999999999, 0.5745, 0.5199999999999999],
            }),
            z: 12,
        },
        DrawObject::Connector {
            id: "e_teacher_to_residual".to_string(),
            points: vec![
                [0.7291666666666667, 0.4655],
                [0.7291666666666667, 0.29],
                [0.5208333333333333, 0.29],
            ],
            from: Some("teacher".to_string()),
            to: Some("residual_alignment".to_string()),
            style: "normal_flow".to_string(),
            label: Some(DrawLabel {
                text: "z_t".to_string(),
                bbox: [0.607, 0.314, 0.643, 0.3640000000000001],
            }),
            z: 13,
        },
        DrawObject::Connector {
            id: "e_student_to_residual".to_string(),
            points: vec![[0.44641666666666663, 0.4655], [0.44641666666666663, 0.29]],
            from: Some("student".to_string()),
            to: Some("residual_alignment".to_string()),
            style: "normal_flow".to_string(),
            label: Some(DrawLabel {
                text: "z_s".to_string(),
                bbox: [
                    0.47041666666666665,
                    0.29,
                    0.5064166666666666,
                    0.3400000000000001,
                ],
            }),
            z: 14,
        },
        DrawObject::Connector {
            id: "e_residual_to_student".to_string(),
            points: vec![
                [0.4208333333333333, 0.29],
                [0.4208333333333333, 0.575],
                [0.472, 0.575],
            ],
            from: Some("residual_alignment".to_string()),
            to: Some("student".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 15,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    assert!(
        !box_object_exists(&plan, "inference_note") && inference_note_bbox(&plan).is_some(),
        "bottom peripheral inference note should remain as editable text, not a collapsed component"
    );

    let input = object_box(&plan, "input_data");
    let student = object_box(&plan, "student");
    assert!(
        horizontal_separation(input, student) >= 0.055,
        "shared input should leave a paper-width horizontal gutter from student: input={input:?}, student={student:?}"
    );

    let output = object_box(&plan, "task_output");
    let student_output = connector(&plan, "e_student_to_output");
    let label = student_output
        .label
        .as_ref()
        .expect("student output edge should retain a task-loss label");
    assert_eq!(
        intersection_area(label.bbox, student),
        0.0,
        "task-loss label should not cover the student box: label={:?}, student={student:?}",
        label.bbox
    );
    assert_eq!(
        intersection_area(label.bbox, output),
        0.0,
        "task-loss label should not cover the compact output box: label={:?}, output={output:?}",
        label.bbox
    );
    assert!(
        label.bbox[2] - label.bbox[0] <= 0.125
            && label_near_any_segment(label.bbox, student_output.points, 0.10),
        "task-loss label should be compact and anchored near the short student-output edge: label={:?}, edge={:?}",
        label.bbox,
        student_output.points
    );

    let input_teacher = connector(&plan, "e_input_to_teacher");
    let route_box = points_to_box_for_test(input_teacher.points);
    assert!(
        route_box[3] <= 0.76 && input_teacher.points.len() <= 4,
        "input-to-teacher edge should stay in the local branch corridor, not detour around the bottom canvas: {:?}",
        input_teacher.points
    );
    for id in ["student", "task_output"] {
        assert!(
            !label_intersects_any_segment(object_box(&plan, id), input_teacher.points),
            "input-to-teacher edge should not cross {id}: {:?}",
            input_teacher.points
        );
    }
}

#[test]
fn model_draw_plan_polish_separates_residual_supervision_from_task_output_from_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input".to_string(),
            bbox: [0.035, 0.5612500000000001, 0.155, 0.7412500000000002],
            text: "Task Input\n(sequence)".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher".to_string(),
            bbox: [0.25, 0.405, 0.57, 0.585],
            text: "Teacher\n(latent residual source)".to_string(),
            role: "module".to_string(),
            style: "accent_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student".to_string(),
            bbox: [0.278, 0.64, 0.628, 0.79],
            text: "Student\n(task + residual training)".to_string(),
            role: "main".to_string(),
            style: "strong_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "latent_residual_supervision".to_string(),
            bbox: [0.72, 0.53, 0.92, 0.66],
            text: "Latent Residual\nSupervision".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "task_output".to_string(),
            bbox: [0.8197, 0.693, 0.972, 0.793],
            text: "Task\nOutput".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Connector {
            id: "e_input_to_teacher".to_string(),
            points: vec![[0.155, 0.5731250000000001], [0.25, 0.5731250000000001]],
            from: Some("input".to_string()),
            to: Some("teacher".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_input_to_student".to_string(),
            points: vec![[0.155, 0.7150000000000001], [0.278, 0.7150000000000001]],
            from: Some("input".to_string()),
            to: Some("student".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_teacher_to_supervision".to_string(),
            points: vec![[0.57, 0.5575], [0.72, 0.5575]],
            from: Some("teacher".to_string()),
            to: Some("latent_residual_supervision".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "e_supervision_to_student".to_string(),
            points: vec![[0.72, 0.65], [0.628, 0.65]],
            from: Some("latent_residual_supervision".to_string()),
            to: Some("student".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Connector {
            id: "e_student_to_output".to_string(),
            points: vec![[0.628, 0.743], [0.8197, 0.743]],
            from: Some("student".to_string()),
            to: Some("task_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 14,
        },
        DrawObject::Text {
            id: "inference_note".to_string(),
            bbox: [0.37300000000000005, 0.8450000000000001, 0.533, 0.915],
            text: "At inference: student only".to_string(),
            style: "annotation".to_string(),
            z: 25,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let residual = object_box(&plan, "latent_residual_supervision");
    let output = object_box(&plan, "task_output");
    assert!(
        vertical_separation(residual, output) >= 0.055,
        "right-side residual supervision and task output need a paper-width gutter: residual={residual:?}, output={output:?}"
    );
    let edge = connector(&plan, "e_student_to_output");
    assert!(
        edge.points.len() <= 3 && !label_intersects_any_segment(residual, edge.points),
        "student-output route should remain compact and avoid the residual box after spacing repair: {:?}",
        edge.points
    );
}

#[test]
fn model_draw_plan_polish_balances_simple_y_branch_from_vision_review_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_input".to_string(),
            bbox: [
                0.062083333333333345,
                0.30750000000000005,
                0.16708333333333333,
                0.44250000000000006,
            ],
            text: "Task Input".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "comp_student".to_string(),
            bbox: [
                0.2758333333333334,
                0.5141666666666668,
                0.5158333333333334,
                0.6941666666666667,
            ],
            text: "Student\n(Compact LM)".to_string(),
            role: "main".to_string(),
            style: "primary_module_bold".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "comp_teacher".to_string(),
            bbox: [
                0.23633333333333337,
                0.06966666666666667,
                0.5553333333333333,
                0.222,
            ],
            text: "Teacher\n(Large LM)".to_string(),
            role: "context".to_string(),
            style: "muted_module_dashed".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "comp_latent_residual".to_string(),
            bbox: [0.67875, 0.23666666666666664, 0.81875, 0.3466666666666666],
            text: "Latent Residual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "comp_task_loss".to_string(),
            bbox: [0.67875, 0.6741666666666666, 0.81875, 0.7841666666666667],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "comp_output".to_string(),
            bbox: [
                0.8399999999999999,
                0.5541666666666667,
                1.0,
                0.6541666666666668,
            ],
            text: "Prediction".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 25,
        },
        DrawObject::Connector {
            id: "edge_input_to_student".to_string(),
            points: vec![
                [0.16708333333333333, 0.37500000000000006],
                [0.16708333333333333, 0.6041666666666667],
                [0.2758333333333334, 0.6041666666666667],
            ],
            from: Some("comp_input".to_string()),
            to: Some("comp_student".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "edge_input_to_teacher".to_string(),
            points: vec![
                [0.16708333333333333, 0.37500000000000006],
                [0.16708333333333333, 0.14583333333333334],
                [0.23633333333333337, 0.14583333333333334],
            ],
            from: Some("comp_input".to_string()),
            to: Some("comp_teacher".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "edge_teacher_to_residual".to_string(),
            points: vec![
                [0.5553333333333333, 0.14583333333333334],
                [0.61875, 0.14583333333333334],
                [0.61875, 0.29166666666666663],
            ],
            from: Some("comp_teacher".to_string()),
            to: Some("comp_latent_residual".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "edge_student_to_residual".to_string(),
            points: vec![
                [0.5158333333333334, 0.6041666666666667],
                [0.5158333333333334, 0.29166666666666663],
                [0.67875, 0.29166666666666663],
            ],
            from: Some("comp_student".to_string()),
            to: Some("comp_latent_residual".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Connector {
            id: "edge_student_to_task_loss".to_string(),
            points: vec![
                [0.5158333333333334, 0.6841666666666666],
                [0.67875, 0.6841666666666666],
            ],
            from: Some("comp_student".to_string()),
            to: Some("comp_task_loss".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 14,
        },
        DrawObject::Connector {
            id: "edge_student_to_output".to_string(),
            points: vec![
                [0.5158333333333334, 0.6041666666666667],
                [0.8399999999999999, 0.6041666666666667],
            ],
            from: Some("comp_student".to_string()),
            to: Some("comp_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 15,
        },
        DrawObject::Text {
            id: "ann_inference".to_string(),
            bbox: [
                0.25583333333333336,
                0.7341666666666667,
                0.5358333333333334,
                0.7941666666666667,
            ],
            text: "Inference: student only".to_string(),
            style: "annotation".to_string(),
            z: 26,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let teacher = object_box(&plan, "comp_teacher");
    let student = object_box(&plan, "comp_student");
    assert!(
        box_width_for_test(teacher) <= box_width_for_test(student) * 1.15,
        "simple Y-branch teacher/student boxes should be visually balanced: teacher={teacher:?}, student={student:?}"
    );
    assert!(
        box_style(&plan, "comp_teacher").contains("muted")
            && box_style(&plan, "comp_teacher").contains("dash"),
        "teacher branch should keep muted/dashed styling"
    );

    let residual = object_box(&plan, "comp_latent_residual");
    assert!(
        box_width_for_test(residual) >= 0.17 && vertical_separation(residual, student) >= 0.05,
        "latent residual should be readable and sit clearly between teacher and student rows: residual={residual:?}, student={student:?}"
    );

    let residual_edge = connector(&plan, "edge_student_to_residual");
    let route_box = points_to_box_for_test(residual_edge.points);
    assert!(
        route_box[3] - route_box[1] <= 0.25,
        "student-to-residual supervision route should be short, not a tall detour: {:?}",
        residual_edge.points
    );

    let note = text_box(&plan, "ann_inference");
    assert!(
        note[1] >= student[3] + 0.08,
        "inference note should sit in a compact marginal caption area below the student row: note={note:?}, student={student:?}"
    );
}

#[test]
fn model_draw_plan_polish_repairs_same_row_teacher_branch_from_latest_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_input".to_string(),
            bbox: [
                0.05416666666666667,
                0.6408333333333334,
                0.15416666666666667,
                0.7758333333333334,
            ],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "comp_student".to_string(),
            bbox: [
                0.2758333333333334,
                0.5558333333333334,
                0.5158333333333334,
                0.7358333333333333,
            ],
            text: "Student\n(Compact LM)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "comp_teacher".to_string(),
            bbox: [
                0.8008333333333334,
                0.5558333333333334,
                0.9628333333333333,
                0.7358333333333333,
            ],
            text: "Teacher\n(Large LM)".to_string(),
            role: "context".to_string(),
            style: "neutral_module_muted_dashed".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "comp_latent_residual".to_string(),
            bbox: [
                0.6058333333333333,
                0.8108333333333333,
                0.8058333333333333,
                0.9408333333333333,
            ],
            text: "Latent Residual\nSupervision".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "comp_task_loss".to_string(),
            bbox: [
                0.18083333333333337,
                0.8658333333333332,
                0.34583333333333344,
                0.9658333333333333,
            ],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "comp_output".to_string(),
            bbox: [
                0.3708333333333334,
                0.7996666666666666,
                0.4708333333333334,
                0.8996666666666667,
            ],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 25,
        },
        DrawObject::Connector {
            id: "edge_input_to_student".to_string(),
            points: vec![
                [0.15416666666666667, 0.6458333333333334],
                [0.2758333333333334, 0.6458333333333334],
            ],
            from: Some("comp_input".to_string()),
            to: Some("comp_student".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "edge_input_to_teacher".to_string(),
            points: vec![
                [0.15416666666666667, 0.7083333333333334],
                [0.15416666666666667, 0.5308333333333334],
                [0.8008333333333334, 0.5308333333333334],
                [0.8008333333333334, 0.5558333333333334],
            ],
            from: Some("comp_input".to_string()),
            to: Some("comp_teacher".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "edge_teacher_to_residual".to_string(),
            points: vec![
                [0.8818333333333334, 0.7358333333333333],
                [0.8818333333333334, 0.8108333333333333],
                [0.7058333333333333, 0.8108333333333333],
            ],
            from: Some("comp_teacher".to_string()),
            to: Some("comp_latent_residual".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "edge_residual_to_student".to_string(),
            points: vec![
                [0.6058333333333333, 0.8758333333333332],
                [0.5158333333333334, 0.8758333333333332],
                [0.5158333333333334, 0.7358333333333333],
            ],
            from: Some("comp_latent_residual".to_string()),
            to: Some("comp_student".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Connector {
            id: "edge_student_to_output".to_string(),
            points: vec![
                [0.4208333333333334, 0.7358333333333333],
                [0.4208333333333334, 0.7996666666666666],
            ],
            from: Some("comp_student".to_string()),
            to: Some("comp_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 14,
        },
        DrawObject::Connector {
            id: "edge_student_to_task_loss".to_string(),
            points: vec![
                [0.3108333333333334, 0.7358333333333333],
                [0.3108333333333334, 0.8658333333333332],
            ],
            from: Some("comp_student".to_string()),
            to: Some("comp_task_loss".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 15,
        },
        DrawObject::Text {
            id: "ann_inference".to_string(),
            bbox: [
                0.5558333333333334,
                0.6058333333333333,
                0.7158333333333333,
                0.6858333333333333,
            ],
            text: "Inference: student only".to_string(),
            style: "annotation".to_string(),
            z: 26,
        },
        DrawObject::Text {
            id: "anno_teacher_label".to_string(),
            bbox: [0.625, 0.20833333333333334, 1.0, 0.5],
            text: "Frozen".to_string(),
            style: "annotation".to_string(),
            z: 27,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let teacher = object_box(&plan, "comp_teacher");
    let student = object_box(&plan, "comp_student");
    assert!(
        teacher[3] <= student[1] - 0.045,
        "same-row teacher branch should move above the student instead of compressing the teacher at the right edge: teacher={teacher:?}, student={student:?}"
    );

    let task_loss = object_box(&plan, "comp_task_loss");
    let output = object_box(&plan, "comp_output");
    assert!(
        horizontal_separation(task_loss, output) >= 0.05
            || vertical_separation(task_loss, output) >= 0.04,
        "task loss and output should keep a visible sibling gutter: task_loss={task_loss:?}, output={output:?}"
    );

    let inference = text_box(&plan, "ann_inference");
    assert!(
        inference[0] >= student[2] + 0.03 || inference[1] >= student[3] + 0.06,
        "inference caption should leave the teacher-student main corridor: inference={inference:?}, student={student:?}, teacher={teacher:?}"
    );

    if let Some(teacher_label) = optional_text_box(&plan, "anno_teacher_label") {
        assert!(
            box_area(teacher_label) <= 0.018,
            "short frozen annotation should be compact, not a large floating caption: {teacher_label:?}"
        );
    }

    let input_teacher = connector(&plan, "edge_input_to_teacher");
    assert!(
        !has_long_horizontal_segment_for_test(input_teacher.points, 0.42),
        "input-to-teacher connector should not use a long top rail detour: {:?}",
        input_teacher.points
    );
}

#[test]
fn model_draw_plan_polish_repairs_direct_teacher_student_residual_label_from_vision_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "task_input".to_string(),
            bbox: [0.01999999999999999, 0.45999999999999996, 0.12, 0.59],
            text: "Task Input".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student_model".to_string(),
            bbox: [0.401, 0.566, 0.641, 0.746],
            text: "Student\n(Deployed)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "teacher_model".to_string(),
            bbox: [0.06, 0.645, 0.28, 0.785],
            text: "Teacher\n(Training only)".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "final_output".to_string(),
            bbox: [0.736, 0.606, 0.9, 0.7060000000000001],
            text: "Final Answer".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.689, 0.28, 0.853, 0.39],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 24,
        },
        DrawObject::Text {
            id: "inference_note".to_string(),
            bbox: [0.441, 0.7710000000000001, 0.601, 0.8360000000000002],
            text: "Inference: student only".to_string(),
            style: "annotation".to_string(),
            z: 25,
        },
        DrawObject::Text {
            id: "residual_label".to_string(),
            bbox: [0.487, 0.302, 0.647, 0.372],
            text: "Latent Residual".to_string(),
            style: "annotation".to_string(),
            z: 26,
        },
        DrawObject::Connector {
            id: "input_to_student".to_string(),
            points: vec![[0.12, 0.578], [0.401, 0.578]],
            from: Some("task_input".to_string()),
            to: Some("student_model".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "input_to_teacher".to_string(),
            points: vec![[0.06999999999999999, 0.59], [0.06999999999999999, 0.645]],
            from: Some("task_input".to_string()),
            to: Some("teacher_model".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "student_to_output".to_string(),
            points: vec![[0.641, 0.656], [0.736, 0.656]],
            from: Some("student_model".to_string()),
            to: Some("final_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "teacher_to_residual".to_string(),
            points: vec![[0.28, 0.6559999999999999], [0.401, 0.6559999999999999]],
            from: Some("teacher_model".to_string()),
            to: Some("student_model".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "Latent Residual".to_string(),
                bbox: [
                    0.44100000000000006,
                    0.5039999999999999,
                    0.601,
                    0.5539999999999999,
                ],
            }),
            z: 13,
        },
        DrawObject::Connector {
            id: "student_to_loss".to_string(),
            points: vec![[0.665, 0.566], [0.665, 0.39]],
            from: Some("student_model".to_string()),
            to: Some("task_loss".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 14,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    assert!(
        !text_object_exists(&plan, "residual_label"),
        "standalone residual annotation should be removed when the same text already labels the supervision connector"
    );
    let teacher = object_box(&plan, "teacher_model");
    let student = object_box(&plan, "student_model");
    assert!(
        teacher[3] <= student[1] - 0.045,
        "direct teacher-student supervision branch should place teacher above the student for Y-branch readability: teacher={teacher:?}, student={student:?}"
    );
    let inference = text_box(&plan, "inference_note");
    assert!(
        vertical_separation(inference, teacher) >= 0.04
            || horizontal_separation(inference, teacher) >= 0.06,
        "inference note should not crowd the teacher vertical space: inference={inference:?}, teacher={teacher:?}"
    );
}

#[test]
fn model_draw_plan_polish_compacts_training_only_annotation_from_latest_smoke() {
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
            "grid": {"columns": 12, "rows": 8},
            "regions": [
                {"id": "input_region", "bbox": [0.08333333333333333, 0.375, 0.2916666666666667, 1.0]},
                {"id": "student_region", "bbox": [0.2916666666666667, 0.3125, 0.75, 1.0]},
                {"id": "teacher_region", "bbox": [0.5833333333333334, 0.3125, 1.0, 1.0]},
                {"id": "task_loss_region", "bbox": [0.375, 0.8125, 0.8333333333333334, 1.0]},
                {"id": "latent_residual_region", "bbox": [0.5, 0.0625, 1.0, 0.2875]},
                {"id": "output_region", "bbox": [0.375, 0.0625, 0.8333333333333334, 0.2875]},
                {"id": "inference_note_region", "bbox": [0.7916666666666666, 0.4375, 1.0, 1.0]}
            ]
        },
        "components": [
            {"id": "input", "label": "Input x", "role": "input", "visual_weight": "normal", "region": "input_region", "allowed_asset_id": null},
            {"id": "student", "label": "Student\n(compact)", "role": "main", "visual_weight": "strong", "region": "student_region", "allowed_asset_id": null},
            {"id": "teacher", "label": "Teacher\n(large)", "role": "context", "visual_weight": "normal", "region": "teacher_region", "allowed_asset_id": null},
            {"id": "task_loss", "label": "Task loss", "role": "loss", "visual_weight": "normal", "region": "task_loss_region", "allowed_asset_id": null},
            {"id": "latent_residual", "label": "Latent residual", "role": "loss", "visual_weight": "strong", "region": "latent_residual_region", "allowed_asset_id": null},
            {"id": "output", "label": "Output ŷ", "role": "output", "visual_weight": "normal", "region": "output_region", "allowed_asset_id": null},
            {"id": "inference_note", "label": "Inference: student only", "role": "context", "visual_weight": "normal", "region": "inference_note_region", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "e_input_student", "from": "input", "to": "student", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_input_teacher", "from": "input", "to": "teacher", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_student_output", "from": "student", "to": "output", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_student_taskloss", "from": "student", "to": "task_loss", "label": "", "semantic": "loss", "style": "solid", "importance": "normal"},
            {"id": "e_teacher_residual", "from": "teacher", "to": "latent_residual", "label": "h_T - h_S", "semantic": "supervision", "style": "dash", "importance": "main"},
            {"id": "e_residual_student", "from": "latent_residual", "to": "student", "label": "", "semantic": "supervision", "style": "dash", "importance": "main"}
        ],
        "annotations": [
            {"id": "ann_training_only", "label": "training only", "target_id": "teacher", "bbox": [0.625, 0.7125, 1.0, 1.0]}
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
    .expect("latest smoke FigurePlan fixture should deserialize");

    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input".to_string(),
            bbox: [0.02, 0.62, 0.12, 0.73],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student".to_string(),
            bbox: [0.22, 0.52, 0.47, 0.77],
            text: "Student\n(compact)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "teacher".to_string(),
            bbox: [0.22, 0.25, 0.47, 0.45],
            text: "Teacher\n(large)".to_string(),
            role: "context".to_string(),
            style: "neutral_module_muted_dashed".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.1106666666666668, 0.85125, 0.2746666666666667, 0.96125],
            text: "Task loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "latent_residual".to_string(),
            bbox: [0.145, 0.085, 0.345, 0.185],
            text: "Latent residual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "output".to_string(),
            bbox: [0.645, 0.17, 0.781, 0.27],
            text: "Output ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 25,
        },
        DrawObject::Text {
            id: "inference_note".to_string(),
            bbox: [0.42, 0.865, 0.58, 0.935],
            text: "Inference: student only".to_string(),
            style: "annotation".to_string(),
            z: 26,
        },
        DrawObject::Text {
            id: "ann_training_only".to_string(),
            bbox: [0.625, 0.7125, 1.0, 1.0],
            text: "training only".to_string(),
            style: "annotation".to_string(),
            z: 27,
        },
        DrawObject::Connector {
            id: "e_input_student".to_string(),
            points: vec![[0.12, 0.645], [0.22, 0.645]],
            from: Some("input".to_string()),
            to: Some("student".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_input_teacher".to_string(),
            points: vec![[0.12, 0.675], [0.12, 0.35], [0.22, 0.35]],
            from: Some("input".to_string()),
            to: Some("teacher".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_student_output".to_string(),
            points: vec![
                [0.345, 0.52],
                [0.345, 0.47],
                [0.679, 0.47],
                [0.679, 0.27],
                [0.713, 0.27],
            ],
            from: Some("student".to_string()),
            to: Some("output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "e_student_taskloss".to_string(),
            points: vec![[0.24733333333333335, 0.77], [0.24733333333333335, 0.85125]],
            from: Some("student".to_string()),
            to: Some("task_loss".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Connector {
            id: "e_teacher_residual".to_string(),
            points: vec![[0.245, 0.25], [0.245, 0.185]],
            from: Some("teacher".to_string()),
            to: Some("latent_residual".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "h_T - h_S".to_string(),
                bbox: [0.269, 0.1925, 0.359, 0.2425],
            }),
            z: 14,
        },
        DrawObject::Connector {
            id: "e_residual_student".to_string(),
            points: vec![[0.345, 0.135], [0.96, 0.135], [0.96, 0.645], [0.22, 0.645]],
            from: Some("latent_residual".to_string()),
            to: Some("student".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 15,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    let training = text_box(&plan, "ann_training_only");
    let teacher = object_box(&plan, "teacher");
    assert!(
        box_area(training) <= 0.018 && training[2] <= 0.94 && training[3] <= 0.94,
        "training-only annotation should be compact and inside the safe canvas: {training:?}"
    );
    assert!(
        ((center_x(training) - center_x(teacher)).powi(2)
            + (center_y(training) - center_y(teacher)).powi(2))
        .sqrt()
            <= 0.30,
        "training-only annotation should anchor near the teacher/context branch: training={training:?}, teacher={teacher:?}"
    );
}

#[test]
fn model_draw_plan_polish_repairs_two_stage_branch_crowding_from_latest_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_task_input".to_string(),
            bbox: [0.02, 0.37, 0.12, 0.505],
            text: "Task Input\nx".to_string(),
            role: "input".to_string(),
            style: "primary_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "comp_student_enc".to_string(),
            bbox: [0.11133333333333333, 0.153, 0.347, 0.4095],
            text: "Student\nEncoder".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "comp_student_head".to_string(),
            bbox: [0.22916666666666666, 0.4445, 0.39716666666666667, 0.5745],
            text: "Student\nHead".to_string(),
            role: "output".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "comp_teacher_enc".to_string(),
            bbox: [0.653, 0.4655, 0.8886666666666666, 0.722],
            text: "Teacher\nEncoder".to_string(),
            role: "module".to_string(),
            style: "neutral_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "comp_teacher_head".to_string(),
            bbox: [0.653, 0.153, 0.8886666666666666, 0.347],
            text: "Teacher\nHead".to_string(),
            role: "module".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "comp_task_loss".to_string(),
            bbox: [0.23899999999999996, 0.05, 0.3873333333333333, 0.15],
            text: "Task Loss\nL_task".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 25,
        },
        DrawObject::Box {
            id: "comp_residual_align".to_string(),
            bbox: [0.414, 0.0975, 0.586, 0.2275],
            text: "Residual\nAlignment".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 26,
        },
        DrawObject::Text {
            id: "ann_inference".to_string(),
            bbox: [0.21791666666666665, 0.6245, 0.40841666666666665, 0.7045],
            text: "Inference: student only".to_string(),
            style: "annotation".to_string(),
            z: 27,
        },
        DrawObject::Connector {
            id: "edge_input_to_student".to_string(),
            points: vec![
                [0.12, 0.4375],
                [0.11133333333333333, 0.4375],
                [0.11133333333333333, 0.28125],
            ],
            from: Some("comp_task_input".to_string()),
            to: Some("comp_student_enc".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "edge_input_to_teacher".to_string(),
            points: vec![
                [0.12, 0.4375],
                [0.155, 0.4375],
                [0.155, 0.722],
                [0.653, 0.722],
            ],
            from: Some("comp_task_input".to_string()),
            to: Some("comp_teacher_enc".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "edge_student_enc_to_head".to_string(),
            points: vec![
                [0.22916666666666666, 0.4095],
                [0.22916666666666666, 0.4445],
                [0.31316666666666665, 0.4445],
            ],
            from: Some("comp_student_enc".to_string()),
            to: Some("comp_student_head".to_string()),
            style: "main_flow".to_string(),
            label: Some(DrawLabel {
                text: "h_s".to_string(),
                bbox: [0.17116666666666666, 0.4195, 0.21116666666666667, 0.4695],
            }),
            z: 12,
        },
        DrawObject::Connector {
            id: "edge_teacher_enc_to_head".to_string(),
            points: vec![[0.7708333333333333, 0.4655], [0.7708333333333333, 0.347]],
            from: Some("comp_teacher_enc".to_string()),
            to: Some("comp_teacher_head".to_string()),
            style: "normal_flow".to_string(),
            label: Some(DrawLabel {
                text: "h_t".to_string(),
                bbox: [0.7948333333333333, 0.38125, 0.8308333333333333, 0.43125],
            }),
            z: 13,
        },
        DrawObject::Connector {
            id: "edge_student_to_taskloss".to_string(),
            points: vec![
                [0.22916666666666666, 0.5095],
                [0.3873333333333333, 0.5095],
                [0.3873333333333333, 0.1],
            ],
            from: Some("comp_student_head".to_string()),
            to: Some("comp_task_loss".to_string()),
            style: "main_flow".to_string(),
            label: Some(DrawLabel {
                text: "ŷ".to_string(),
                bbox: [0.41133333333333333, 0.27975, 0.4473333333333333, 0.32975],
            }),
            z: 14,
        },
        DrawObject::Connector {
            id: "edge_teacher_to_residual".to_string(),
            points: vec![[0.653, 0.1625], [0.586, 0.1625]],
            from: Some("comp_teacher_head".to_string()),
            to: Some("comp_residual_align".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "r = h_t - h_s".to_string(),
                bbox: [0.43, 0.89, 0.57, 0.94],
            }),
            z: 15,
        },
        DrawObject::Connector {
            id: "edge_residual_to_student".to_string(),
            points: vec![
                [0.414, 0.1625],
                [0.39716666666666667, 0.1625],
                [0.39716666666666667, 0.4445],
            ],
            from: Some("comp_residual_align".to_string()),
            to: Some("comp_student_head".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "L_res".to_string(),
                bbox: [0.4211666666666667, 0.3945, 0.5021666666666667, 0.4445],
            }),
            z: 16,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let student_enc = object_box(&plan, "comp_student_enc");
    let student_head = object_box(&plan, "comp_student_head");
    assert!(
        vertical_separation(student_enc, student_head) >= 0.055,
        "student encoder/head need a visible vertical gutter: encoder={student_enc:?}, head={student_head:?}"
    );

    let task_loss = object_box(&plan, "comp_task_loss");
    let residual = object_box(&plan, "comp_residual_align");
    assert!(
        vertical_separation(student_enc, task_loss) >= 0.055
            || horizontal_separation(student_enc, task_loss) >= 0.050,
        "task loss should not crowd the student encoder: encoder={student_enc:?}, task_loss={task_loss:?}"
    );
    assert!(
        vertical_separation(task_loss, residual) >= 0.045
            || horizontal_separation(task_loss, residual) >= 0.055,
        "task loss and residual alignment need visible separation: task_loss={task_loss:?}, residual={residual:?}"
    );

    let input_teacher = connector(&plan, "edge_input_to_teacher");
    let teacher_enc = object_box(&plan, "comp_teacher_enc");
    assert!(
        !input_teacher.points.windows(2).any(|segment| {
            (segment[0][1] - segment[1][1]).abs() < 0.006
                && (segment[0][0] - segment[1][0]).abs() > 0.42
                && segment[0][1] >= teacher_enc[3] - 0.02
        }),
        "input-to-teacher connector should not keep the long lower rail detour: {:?}",
        input_teacher.points
    );

    let inference = text_box(&plan, "ann_inference");
    assert!(
        !label_near_any_segment(inference, input_teacher.points, 0.025),
        "inference caption should move away from the input-to-teacher route: inference={inference:?}, points={:?}",
        input_teacher.points
    );

    for edge_id in ["edge_teacher_to_residual", "edge_student_to_taskloss"] {
        let edge = connector(&plan, edge_id);
        if let Some(label) = &edge.label {
            assert!(
                label_near_any_segment(label.bbox, edge.points, 0.030),
                "{edge_id} label should either be removed or snapped back near its connector: label={:?}, points={:?}",
                label.bbox,
                edge.points
            );
        }
    }
}

#[test]
fn model_draw_plan_polish_compacts_prediction_and_routes_residual_from_latest_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input".to_string(),
            bbox: [
                0.023333333333333334,
                0.47416666666666674,
                0.1433333333333333,
                0.6091666666666667,
            ],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher".to_string(),
            bbox: [
                0.2828333333333334,
                0.20166666666666663,
                0.5088333333333334,
                0.3816666666666666,
            ],
            text: "Teacher\n(large LM)".to_string(),
            role: "module".to_string(),
            style: "muted_module_dashed".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student".to_string(),
            bbox: [
                0.2828333333333334,
                0.7016666666666668,
                0.5088333333333334,
                0.8816666666666667,
            ],
            text: "Student\n(compact)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "latent_residual_obj".to_string(),
            bbox: [0.6291666666666667, 0.45, 0.8291666666666666, 0.58],
            text: "Latent Residual\nSupervision".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "task_loss_obj".to_string(),
            bbox: [0.5703333333333334, 0.89, 0.7343333333333333, 1.0],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "prediction".to_string(),
            bbox: [0.6106666666666667, 0.65, 0.7106666666666668, 0.79],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 26,
        },
        DrawObject::Text {
            id: "ann_inference".to_string(),
            bbox: [0.2758333333333334, 0.92, 0.5158333333333334, 0.98],
            text: "Inference: student only".to_string(),
            style: "annotation".to_string(),
            z: 27,
        },
        DrawObject::Connector {
            id: "e_input_teacher".to_string(),
            points: vec![
                [0.1433333333333333, 0.5416666666666667],
                [0.1433333333333333, 0.29166666666666663],
                [0.2828333333333334, 0.29166666666666663],
            ],
            from: Some("input".to_string()),
            to: Some("teacher".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_input_student".to_string(),
            points: vec![
                [0.1433333333333333, 0.5416666666666667],
                [0.1433333333333333, 0.7916666666666667],
                [0.2828333333333334, 0.7916666666666667],
            ],
            from: Some("input".to_string()),
            to: Some("student".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_teacher_residual".to_string(),
            points: vec![
                [0.5088333333333334, 0.29166666666666663],
                [0.6291666666666667, 0.29166666666666663],
                [0.6291666666666667, 0.515],
            ],
            from: Some("teacher".to_string()),
            to: Some("latent_residual_obj".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "z_t".to_string(),
                bbox: [
                    0.6531666666666667,
                    0.3783333333333333,
                    0.6891666666666667,
                    0.42833333333333334,
                ],
            }),
            z: 12,
        },
        DrawObject::Connector {
            id: "e_student_residual".to_string(),
            points: vec![
                [0.39583333333333337, 0.7016666666666668],
                [0.39583333333333337, 0.515],
                [0.6291666666666667, 0.515],
            ],
            from: Some("student".to_string()),
            to: Some("latent_residual_obj".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "z_s".to_string(),
                bbox: [0.4945, 0.441, 0.5305, 0.491],
            }),
            z: 13,
        },
        DrawObject::Connector {
            id: "e_residual_student".to_string(),
            points: vec![
                [0.6291666666666667, 0.58],
                [0.6291666666666667, 0.7916666666666667],
                [0.5088333333333334, 0.7916666666666667],
            ],
            from: Some("latent_residual_obj".to_string()),
            to: Some("student".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "residual loss".to_string(),
                bbox: [
                    0.6471666666666667,
                    0.8096666666666668,
                    0.8071666666666666,
                    0.8596666666666668,
                ],
            }),
            z: 14,
        },
        DrawObject::Connector {
            id: "e_student_task".to_string(),
            points: vec![
                [0.5088333333333334, 0.7916666666666667],
                [0.4883333333333334, 0.87],
            ],
            from: Some("student".to_string()),
            to: Some("task_loss_obj".to_string()),
            style: "normal_flow".to_string(),
            label: Some(DrawLabel {
                text: "ŷ".to_string(),
                bbox: [
                    0.5165833333333334,
                    0.8058333333333333,
                    0.5565833333333334,
                    0.8558333333333333,
                ],
            }),
            z: 15,
        },
        DrawObject::Connector {
            id: "e_student_output".to_string(),
            points: vec![[0.5088333333333334, 0.72], [0.6106666666666667, 0.72]],
            from: Some("student".to_string()),
            to: Some("prediction".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 16,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let prediction = object_box(&plan, "prediction");
    assert!(
        prediction[2] - prediction[0] <= 0.105 && prediction[3] - prediction[1] <= 0.105,
        "single-symbol prediction should be compact: {prediction:?}"
    );
    let residual_student = connector(&plan, "e_residual_student");
    assert!(
        !label_near_any_segment(prediction, residual_student.points, 0.004),
        "residual-to-student connector should route around prediction: prediction={prediction:?}, points={:?}",
        residual_student.points
    );
    let task_loss = object_box(&plan, "task_loss_obj");
    assert!(
        task_loss[3] <= 0.94,
        "task loss should stay inside the bottom safe area: {task_loss:?}"
    );
    let inference = text_box(&plan, "ann_inference");
    assert!(
        inference[3] <= 0.94,
        "inference caption should not sit on the bottom edge: {inference:?}"
    );
}

#[test]
fn model_draw_plan_polish_separates_right_side_task_loss_output_and_inference_from_latest_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input_text".to_string(),
            bbox: [0.05416666666666667, 0.33875, 0.15416666666666667, 0.47375],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student_model".to_string(),
            bbox: [
                0.2828333333333334,
                0.5558333333333334,
                0.5088333333333334,
                0.7358333333333333,
            ],
            text: "Student\n(compact)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "teacher_model".to_string(),
            bbox: [
                0.2737933333333334,
                0.06966666666666667,
                0.5178733333333334,
                0.26366666666666666,
            ],
            text: "Teacher\n(large, frozen)".to_string(),
            role: "context".to_string(),
            style: "muted_module_dashed".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "latent_residual_obj".to_string(),
            bbox: [
                0.5988333333333333,
                0.37083333333333335,
                0.7988333333333333,
                0.5008333333333334,
            ],
            text: "Latent Residual\nL_res".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "task_loss_obj".to_string(),
            bbox: [0.63675, 0.7058333333333333, 0.80075, 0.8358333333333332],
            text: "Task Loss\nL_task".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "output_pred".to_string(),
            bbox: [0.812, 0.6583333333333333, 0.98, 0.7583333333333334],
            text: "Prediction ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 25,
        },
        DrawObject::Connector {
            id: "e_input_student".to_string(),
            points: vec![
                [0.15416666666666667, 0.40625],
                [0.15416666666666667, 0.6458333333333334],
                [0.2828333333333334, 0.6458333333333334],
            ],
            from: Some("input_text".to_string()),
            to: Some("student_model".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_input_teacher".to_string(),
            points: vec![
                [0.15416666666666667, 0.40625],
                [0.15416666666666667, 0.16666666666666666],
                [0.2737933333333334, 0.16666666666666666],
            ],
            from: Some("input_text".to_string()),
            to: Some("teacher_model".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_student_output".to_string(),
            points: vec![
                [0.5088333333333334, 0.7083333333333334],
                [0.812, 0.7083333333333334],
            ],
            from: Some("student_model".to_string()),
            to: Some("output_pred".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "e_teacher_residual".to_string(),
            points: vec![
                [0.5178733333333334, 0.16666666666666666],
                [0.5988333333333333, 0.16666666666666666],
                [0.5988333333333333, 0.43583333333333335],
            ],
            from: Some("teacher_model".to_string()),
            to: Some("latent_residual_obj".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "z_t".to_string(),
                bbox: [
                    0.6228333333333333,
                    0.27624999999999994,
                    0.6588333333333334,
                    0.3262500000000001,
                ],
            }),
            z: 13,
        },
        DrawObject::Connector {
            id: "e_student_residual".to_string(),
            points: vec![
                [0.5088333333333334, 0.6458333333333334],
                [0.5088333333333334, 0.43583333333333335],
                [0.5988333333333333, 0.43583333333333335],
            ],
            from: Some("student_model".to_string()),
            to: Some("latent_residual_obj".to_string()),
            style: "normal_flow".to_string(),
            label: Some(DrawLabel {
                text: "z_s".to_string(),
                bbox: [
                    0.44883333333333336,
                    0.43583333333333335,
                    0.48483333333333334,
                    0.4858333333333334,
                ],
            }),
            z: 14,
        },
        DrawObject::Connector {
            id: "e_residual_supervise".to_string(),
            points: vec![
                [0.5988333333333333, 0.5008333333333334],
                [0.5988333333333333, 0.6458333333333334],
                [0.5088333333333334, 0.6458333333333334],
            ],
            from: Some("latent_residual_obj".to_string()),
            to: Some("student_model".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 15,
        },
        DrawObject::Connector {
            id: "e_task_supervise".to_string(),
            points: vec![
                [0.63675, 0.7208333333333333],
                [0.5088333333333334, 0.7208333333333333],
            ],
            from: Some("task_loss_obj".to_string()),
            to: Some("student_model".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 16,
        },
        DrawObject::Text {
            id: "ann_inference".to_string(),
            bbox: [
                0.2758333333333334,
                0.8258333333333333,
                0.5158333333333334,
                0.8858333333333334,
            ],
            text: "Inference: student only".to_string(),
            style: "annotation".to_string(),
            z: 26,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let student = object_box(&plan, "student_model");
    let latent = object_box(&plan, "latent_residual_obj");
    let task_loss = object_box(&plan, "task_loss_obj");
    let output = object_box(&plan, "output_pred");
    assert!(
        horizontal_separation(task_loss, output) >= 0.050
            || vertical_separation(task_loss, output) >= 0.035,
        "right-side task loss and prediction output should not stay crowded: task_loss={task_loss:?}, output={output:?}"
    );
    assert!(
        (task_loss[0] - latent[0]).abs() <= 0.035
            || task_loss[0] <= output[0] - 0.050 - box_width_for_test(task_loss),
        "task loss should align with the supervision column or leave a visible gutter from output: task_loss={task_loss:?}, latent={latent:?}, output={output:?}"
    );

    let inference = text_box(&plan, "ann_inference");
    assert!(
        inference[0] >= student[2] + 0.08 || inference[1] >= 0.88,
        "inference annotation should move out of the student-underflow corridor: inference={inference:?}, student={student:?}"
    );
}

#[test]
fn model_draw_plan_polish_repairs_left_teacher_crossing_objective_topology_from_latest_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_input".to_string(),
            bbox: [0.023333333333333334, 0.118, 0.1433333333333333, 0.253],
            text: "Task Input".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "comp_teacher".to_string(),
            bbox: [0.02, 0.45, 0.18, 0.65],
            text: "Teacher\nModel".to_string(),
            role: "context".to_string(),
            style: "neutral_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "comp_student".to_string(),
            bbox: [0.4218333333333334, 0.4, 0.6198333333333333, 0.58],
            text: "Student\nModel".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "comp_task_output".to_string(),
            bbox: [0.44299999999999995, 0.839, 0.598, 0.974],
            text: "Answer\nPrediction".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "comp_latent_residual".to_string(),
            bbox: [0.7125, 0.10125, 0.9125, 0.21125],
            text: "Latent Residual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "comp_combined_objective".to_string(),
            bbox: [0.72, 0.66375, 0.92, 0.77375],
            text: "Task Loss + Latent Residual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 25,
        },
        DrawObject::Connector {
            id: "edge_input_to_student".to_string(),
            points: vec![
                [0.1433333333333333, 0.1855],
                [0.4218333333333334, 0.1855],
                [0.4218333333333334, 0.49],
            ],
            from: Some("comp_input".to_string()),
            to: Some("comp_student".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "edge_input_to_teacher".to_string(),
            points: vec![[0.09999999999999999, 0.253], [0.09999999999999999, 0.45]],
            from: Some("comp_input".to_string()),
            to: Some("comp_teacher".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "edge_student_to_output".to_string(),
            points: vec![
                [0.5205, 0.58],
                [0.633, 0.58],
                [0.633, 0.839],
                [0.598, 0.839],
            ],
            from: Some("comp_student".to_string()),
            to: Some("comp_task_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "edge_teacher_to_latent".to_string(),
            points: vec![[0.18, 0.55], [0.18, 0.15625], [0.7125, 0.15625]],
            from: Some("comp_teacher".to_string()),
            to: Some("comp_latent_residual".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Connector {
            id: "edge_latent_to_objective".to_string(),
            points: vec![[0.8200000000000001, 0.21125], [0.8200000000000001, 0.66375]],
            from: Some("comp_latent_residual".to_string()),
            to: Some("comp_combined_objective".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 14,
        },
        DrawObject::Connector {
            id: "edge_student_to_objective".to_string(),
            points: vec![[0.6198333333333333, 0.49], [0.72, 0.49], [0.72, 0.71875]],
            from: Some("comp_student".to_string()),
            to: Some("comp_combined_objective".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 15,
        },
        DrawObject::Connector {
            id: "edge_teacher_to_objective".to_string(),
            points: vec![[0.18, 0.625], [0.18, 0.71875], [0.72, 0.71875]],
            from: Some("comp_teacher".to_string()),
            to: Some("comp_combined_objective".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 16,
        },
        DrawObject::Text {
            id: "ann_inference".to_string(),
            bbox: [0.42558333333333337, 0.605, 0.6160833333333333, 0.675],
            text: "Inference: student only".to_string(),
            style: "annotation".to_string(),
            z: 26,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let teacher = object_box(&plan, "comp_teacher");
    let student = object_box(&plan, "comp_student");
    let teacher_student_x_overlap =
        (teacher[2].min(student[2]) - teacher[0].max(student[0])).max(0.0);
    assert!(
        teacher[3] <= student[1] - 0.035 && teacher_student_x_overlap >= 0.10,
        "teacher should be restored as an upper peer branch, not left-bottom side block: teacher={teacher:?}, student={student:?}"
    );

    let input_student = connector(&plan, "edge_input_to_student");
    let teacher_latent = connector(&plan, "edge_teacher_to_latent");
    assert!(
        input_student.points.len() <= 3
            && !has_long_horizontal_segment_for_test(input_student.points, 0.30),
        "input-to-student should be a compact local route: {:?}",
        input_student.points
    );
    assert!(
        !connectors_cross(input_student.points, teacher_latent.points),
        "input-to-student and teacher-to-latent should not cross: input_student={:?}, teacher_latent={:?}",
        input_student.points,
        teacher_latent.points
    );

    let student_output = connector(&plan, "edge_student_to_output");
    let teacher_objective = connector(&plan, "edge_teacher_to_objective");
    assert!(
        student_output.points.len() <= 2,
        "student-to-output should be a direct vertical route: {:?}",
        student_output.points
    );
    assert!(
        !connectors_cross(student_output.points, teacher_objective.points),
        "student-to-output and teacher-to-objective should not cross: student_output={:?}, teacher_objective={:?}",
        student_output.points,
        teacher_objective.points
    );

    let inference = text_box(&plan, "ann_inference");
    assert!(
        !label_near_any_segment(inference, student_output.points, 0.030),
        "inference annotation should move off the student-output edge: inference={inference:?}, edge={:?}",
        student_output.points
    );
}

#[test]
fn model_draw_plan_polish_simplifies_teacher_alignment_stair_step_from_latest_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input".to_string(),
            bbox: [0.023333333333333334, 0.46375, 0.1433333333333333, 0.59875],
            text: "Task Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher_branch".to_string(),
            bbox: [0.23633333333333337, 0.0905, 0.5553333333333333, 0.472],
            text: "Teacher LM\n(latent residual source)".to_string(),
            role: "context".to_string(),
            style: "accent_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student_branch".to_string(),
            bbox: [0.23633333333333337, 0.5905, 0.5553333333333333, 0.972],
            text: "Student Model\n(task predictor)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "alignment_node".to_string(),
            bbox: [0.7125, 0.49000000000000005, 0.9125, 0.6199999999999999],
            text: "Latent Residual\nAlignment".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "output".to_string(),
            bbox: [0.8899999999999999, 0.7809999999999999, 0.99, 0.881],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "task_label".to_string(),
            bbox: [
                0.7149999999999997,
                0.6749999999999999,
                0.8349999999999999,
                0.775,
            ],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 25,
        },
        DrawObject::Connector {
            id: "e_teacher_align".to_string(),
            points: vec![
                [0.5553333333333333, 0.28125],
                [0.5553333333333333, 0.58125],
                [0.6339166666666667, 0.58125],
                [0.6339166666666667, 0.5549999999999999],
                [0.7125, 0.5549999999999999],
            ],
            from: Some("teacher_branch".to_string()),
            to: Some("alignment_node".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "h_t".to_string(),
                bbox: [
                    0.5793333333333334,
                    0.40624999999999994,
                    0.6153333333333334,
                    0.4562500000000001,
                ],
            }),
            z: 12,
        },
        DrawObject::Connector {
            id: "e_student_align".to_string(),
            points: vec![[0.5553333333333333, 0.60525], [0.7125, 0.60525]],
            from: Some("student_branch".to_string()),
            to: Some("alignment_node".to_string()),
            style: "main_flow".to_string(),
            label: Some(DrawLabel {
                text: "h_s".to_string(),
                bbox: [0.6159166666666667, 0.62925, 0.6519166666666667, 0.67925],
            }),
            z: 13,
        },
        DrawObject::Connector {
            id: "e_student_output".to_string(),
            points: vec![[0.5553333333333333, 0.831], [0.8899999999999999, 0.831]],
            from: Some("student_branch".to_string()),
            to: Some("output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 14,
        },
        DrawObject::Connector {
            id: "e_taskloss".to_string(),
            points: vec![[0.7149999999999997, 0.725], [0.5553333333333333, 0.725]],
            from: Some("task_label".to_string()),
            to: Some("student_branch".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 15,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let edge = connector(&plan, "e_teacher_align");
    assert!(
        edge.points.len() <= 4,
        "teacher-to-alignment connector should collapse the redundant stair-step: {:?}",
        edge.points
    );
    assert!(
        edge.label
            .as_ref()
            .is_some_and(|label| label_near_any_segment(label.bbox, edge.points, 0.030)),
        "teacher alignment label should be attached to the simplified edge: label={:?}, points={:?}",
        edge.label,
        edge.points
    );
}

#[test]
fn model_draw_plan_polish_repairs_projector_encoder_overlap_from_latest_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input".to_string(),
            bbox: [0.03266666666666662, 0.7, 0.13266666666666663, 0.8],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student_enc".to_string(),
            bbox: [
                0.19266666666666662,
                0.7150000000000001,
                0.3906666666666666,
                0.895,
            ],
            text: "Student\nEncoder".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student_proj".to_string(),
            bbox: [0.11133333333333333, 0.278, 0.472, 0.6595],
            text: "Student\nProjector".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "student_out".to_string(),
            bbox: [0.2916666666666667, 0.10625, 0.3916666666666667, 0.20625],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "primary_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "teacher_enc".to_string(),
            bbox: [0.6113333333333334, 0.528, 0.972, 0.972],
            text: "Teacher\nEncoder".to_string(),
            role: "module".to_string(),
            style: "muted_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "teacher_proj".to_string(),
            bbox: [0.6193333333333334, 0.278, 0.98, 0.6595],
            text: "Teacher\nProjector".to_string(),
            role: "module".to_string(),
            style: "muted_module".to_string(),
            z: 25,
        },
        DrawObject::Box {
            id: "teacher_out".to_string(),
            bbox: [0.7916666666666667, 0.10625, 0.8916666666666667, 0.20625],
            text: "z_t".to_string(),
            role: "output".to_string(),
            style: "muted_module".to_string(),
            z: 26,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.4366666666666667, 0.10625, 0.5766666666666667, 0.20625],
            text: "L_task".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 27,
        },
        DrawObject::Text {
            id: "inference_note".to_string(),
            bbox: [
                0.4409166666666667,
                0.7725000000000002,
                0.6009166666666667,
                0.8375000000000002,
            ],
            text: "Inference: student only".to_string(),
            style: "annotation".to_string(),
            z: 29,
        },
        DrawObject::Connector {
            id: "e_input_student".to_string(),
            points: vec![[0.13266666666666663, 0.75], [0.19266666666666662, 0.75]],
            from: Some("input".to_string()),
            to: Some("student_enc".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_student_up1".to_string(),
            points: vec![
                [0.29166666666666663, 0.7150000000000001],
                [0.29166666666666663, 0.6595],
            ],
            from: Some("student_enc".to_string()),
            to: Some("student_proj".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_student_up2".to_string(),
            points: vec![[0.3416666666666667, 0.278], [0.3416666666666667, 0.20625]],
            from: Some("student_proj".to_string()),
            to: Some("student_out".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "e_input_teacher".to_string(),
            points: vec![
                [0.13266666666666663, 0.75],
                [0.08, 0.75],
                [0.08, 0.972],
                [0.6113333333333334, 0.972],
            ],
            from: Some("input".to_string()),
            to: Some("teacher_enc".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Connector {
            id: "e_teacher_up1".to_string(),
            points: vec![[0.7916666666666667, 0.75], [0.7996666666666667, 0.6595]],
            from: Some("teacher_enc".to_string()),
            to: Some("teacher_proj".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 14,
        },
        DrawObject::Connector {
            id: "e_teacher_up2".to_string(),
            points: vec![[0.8416666666666668, 0.278], [0.8416666666666668, 0.20625]],
            from: Some("teacher_proj".to_string()),
            to: Some("teacher_out".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 15,
        },
        DrawObject::Connector {
            id: "e_task_loss".to_string(),
            points: vec![[0.3916666666666667, 0.15625], [0.4366666666666667, 0.15625]],
            from: Some("student_out".to_string()),
            to: Some("task_loss".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 16,
        },
        DrawObject::Connector {
            id: "e_residual".to_string(),
            points: vec![
                [0.7916666666666667, 0.23124999999999998],
                [0.472, 0.23124999999999998],
            ],
            from: Some("teacher_out".to_string()),
            to: Some("student_proj".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "z_t - z_s".to_string(),
                bbox: [
                    0.5868333333333333,
                    0.15724999999999995,
                    0.6768333333333333,
                    0.20725,
                ],
            }),
            z: 17,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let student_proj = object_box(&plan, "student_proj");
    let teacher_proj = object_box(&plan, "teacher_proj");
    let teacher_enc = object_box(&plan, "teacher_enc");
    assert!(
        box_width_for_test(student_proj) <= 0.24
            && box_width_for_test(teacher_proj) <= 0.24
            && box_height_for_test(student_proj) <= 0.18
            && box_height_for_test(teacher_proj) <= 0.18,
        "projector boxes should be compact, not giant empty containers: student_proj={student_proj:?}, teacher_proj={teacher_proj:?}"
    );
    assert!(
        !component_overlap_gate_fails(teacher_enc, teacher_proj)
            && vertical_separation(teacher_enc, teacher_proj) >= 0.045,
        "teacher encoder/projector stack should have a visible gutter: teacher_enc={teacher_enc:?}, teacher_proj={teacher_proj:?}"
    );

    let inference = text_box(&plan, "inference_note");
    let student_enc = object_box(&plan, "student_enc");
    assert!(
        inference[2] <= student_enc[0] - 0.020 || inference[1] >= student_enc[3] + 0.030,
        "inference note should move to student periphery instead of teacher/student corridor: note={inference:?}, student_enc={student_enc:?}"
    );

    let input_teacher = connector(&plan, "e_input_teacher");
    assert!(
        input_teacher.points.len() <= 3
            && !input_teacher.points.iter().any(|point| point[0] < 0.10)
            && !has_long_horizontal_segment_for_test(input_teacher.points, 0.35),
        "input-to-teacher should be a local route, not a bottom-left detour: {:?}",
        input_teacher.points
    );
}

#[test]
fn model_draw_plan_polish_repairs_two_stage_head_output_and_feedback_routes_from_latest_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input".to_string(),
            bbox: [0.06791666666666667, 0.84375, 0.22375, 0.97875],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student_enc".to_string(),
            bbox: [
                0.06966666666666668,
                0.28500000000000003,
                0.222,
                0.46499999999999997,
            ],
            text: "Student\nEncoder".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student_head".to_string(),
            bbox: [
                0.06966666666666668,
                0.5399999999999999,
                0.222,
                0.7199999999999999,
            ],
            text: "Student\nHead".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "student_out".to_string(),
            bbox: [0.14583333333333334, 0.0625, 0.24583333333333335, 0.1625],
            text: "ŷ_s".to_string(),
            role: "output".to_string(),
            style: "primary_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "teacher_enc".to_string(),
            bbox: [0.778, 0.62875, 0.972, 0.80875],
            text: "Teacher\nEncoder".to_string(),
            role: "module".to_string(),
            style: "muted_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "teacher_head".to_string(),
            bbox: [0.778, 0.28500000000000003, 0.972, 0.46499999999999997],
            text: "Teacher\nHead".to_string(),
            role: "module".to_string(),
            style: "muted_module".to_string(),
            z: 25,
        },
        DrawObject::Box {
            id: "teacher_out".to_string(),
            bbox: [0.771, 0.058499999999999996, 0.979, 0.1665],
            text: "ŷ_t".to_string(),
            role: "output".to_string(),
            style: "muted_module".to_string(),
            z: 26,
        },
        DrawObject::Box {
            id: "latent_residual".to_string(),
            bbox: [0.5459999999999999, 0.1915, 0.716, 0.26],
            text: "Latent Residual\nL_res = ||z_t − z_s||²".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 27,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [
                0.30083333333333334,
                0.047500000000000056,
                0.4608333333333333,
                0.17749999999999994,
            ],
            text: "Task Loss\nL_task = CE(y, ŷ_s)".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 28,
        },
        DrawObject::Box {
            id: "inference_note".to_string(),
            bbox: [0.27225, 0.3425, 0.43225, 0.40750000000000003],
            text: "Inference: student only".to_string(),
            role: "context".to_string(),
            style: "neutral_module".to_string(),
            z: 29,
        },
        DrawObject::Connector {
            id: "e_input_student".to_string(),
            points: vec![
                [0.14583333333333334, 0.84375],
                [0.04466666666666668, 0.84375],
                [0.04466666666666668, 0.46499999999999997],
                [0.14583333333333334, 0.46499999999999997],
            ],
            from: Some("input".to_string()),
            to: Some("student_enc".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_student_enc_head".to_string(),
            points: vec![
                [0.14583333333333334, 0.46499999999999997],
                [0.14583333333333334, 0.5399999999999999],
            ],
            from: Some("student_enc".to_string()),
            to: Some("student_head".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_student_head_out".to_string(),
            points: vec![
                [0.14583333333333334, 0.5399999999999999],
                [0.247, 0.5399999999999999],
                [0.247, 0.1625],
                [0.19583333333333336, 0.1625],
            ],
            from: Some("student_head".to_string()),
            to: Some("student_out".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "e_input_teacher".to_string(),
            points: vec![[0.22375, 0.91125], [0.22375, 0.87875], [0.778, 0.87875]],
            from: Some("input".to_string()),
            to: Some("teacher_enc".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Connector {
            id: "e_teacher_enc_head".to_string(),
            points: vec![[0.875, 0.62875], [0.875, 0.46499999999999997]],
            from: Some("teacher_enc".to_string()),
            to: Some("teacher_head".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 14,
        },
        DrawObject::Connector {
            id: "e_teacher_head_out".to_string(),
            points: vec![[0.875, 0.28500000000000003], [0.875, 0.1665]],
            from: Some("teacher_head".to_string()),
            to: Some("teacher_out".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 15,
        },
        DrawObject::Connector {
            id: "e_student_to_task".to_string(),
            points: vec![
                [0.24583333333333335, 0.11249999999999999],
                [0.30083333333333334, 0.11249999999999999],
            ],
            from: Some("student_out".to_string()),
            to: Some("task_loss".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 16,
        },
        DrawObject::Connector {
            id: "e_teacher_to_residual".to_string(),
            points: vec![[0.771, 0.1125], [0.716, 0.1125], [0.716, 0.22575]],
            from: Some("teacher_out".to_string()),
            to: Some("latent_residual".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "z_t".to_string(),
                bbox: [0.716, 0.03849999999999995, 0.752, 0.0885],
            }),
            z: 17,
        },
        DrawObject::Connector {
            id: "e_student_to_residual".to_string(),
            points: vec![
                [0.222, 0.6299999999999999],
                [0.5459999999999999, 0.6299999999999999],
                [0.5459999999999999, 0.22575],
            ],
            from: Some("student_head".to_string()),
            to: Some("latent_residual".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "z_s".to_string(),
                bbox: [0.57, 0.402875, 0.606, 0.4528750000000001],
            }),
            z: 18,
        },
        DrawObject::Connector {
            id: "e_task_to_student".to_string(),
            points: vec![
                [0.30083333333333334, 0.11249999999999999],
                [0.30083333333333334, 0.027499999999999997],
                [0.04, 0.027499999999999997],
                [0.04, 0.375],
                [0.222, 0.375],
            ],
            from: Some("task_loss".to_string()),
            to: Some("student_enc".to_string()),
            style: "main_flow".to_string(),
            label: Some(DrawLabel {
                text: "∇L_task".to_string(),
                bbox: [0.064, 0.17624999999999996, 0.16299999999999998, 0.22625],
            }),
            z: 19,
        },
        DrawObject::Connector {
            id: "e_residual_to_student".to_string(),
            points: vec![
                [0.5459999999999999, 0.22575],
                [0.222, 0.22575],
                [0.222, 0.375],
            ],
            from: Some("latent_residual".to_string()),
            to: Some("student_enc".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 20,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let latent = object_box(&plan, "latent_residual");
    assert!(
        box_width_for_test(latent) >= 0.22 && box_height_for_test(latent) >= 0.085,
        "latent residual should be wide/tall enough for the equation at paper width: {latent:?}"
    );

    let note = inference_note_bbox(&plan).expect("inference note should remain visible");
    let student_head = object_box(&plan, "student_head");
    assert!(
        note[0] >= student_head[2] + 0.025 && note[1] >= student_head[3] + 0.025,
        "inference note should move below/right of the student stack, not stay in the student-residual corridor: note={note:?}, student_head={student_head:?}"
    );

    let student_out = connector(&plan, "e_student_head_out");
    assert!(
        student_out.points.len() == 2
            && (student_out.points[0][0] - student_out.points[1][0]).abs() < 0.01,
        "student head to output should be a direct vertical route: {:?}",
        student_out.points
    );

    let task_student = connector(&plan, "e_task_to_student");
    let task_student_len: f64 = task_student
        .points
        .windows(2)
        .map(|window| segment_length((window[0], window[1])))
        .sum();
    assert!(
        task_student.points.len() <= 4
            && !task_student.points.iter().any(|point| point[0] < 0.06)
            && task_student_len < 0.50,
        "task feedback should be local, not a left/top loop: points={:?}, len={task_student_len}",
        task_student.points
    );

    let residual_student = connector(&plan, "e_residual_to_student");
    let student_enc = object_box(&plan, "student_enc");
    assert!(
        !label_intersects_any_segment(student_enc, residual_student.points),
        "residual-to-student should not run through the student encoder body: student={student_enc:?}, edge={:?}",
        residual_student.points
    );
    assert!(
        !connectors_cross(student_out.points, residual_student.points),
        "student output route and residual-to-student route should not cross: output={:?}, residual={:?}",
        student_out.points, residual_student.points
    );
}

#[test]
fn model_draw_plan_polish_removes_duplicate_inference_and_moves_task_loss_from_branch_corridor() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input_text".to_string(),
            bbox: [0.06966666666666668, 0.52625, 0.222, 0.66125],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher_model".to_string(),
            bbox: [
                0.3938333333333334,
                0.28500000000000003,
                0.6198333333333333,
                0.465,
            ],
            text: "Teacher\n(large LM)".to_string(),
            role: "main".to_string(),
            style: "neutral_module_muted_dashed".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student_model".to_string(),
            bbox: [0.4078333333333334, 0.7225, 0.6338333333333334, 0.9025],
            text: "Student\n(compact)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "latent_residual_supervision".to_string(),
            bbox: [
                0.7238333333333333,
                0.5399999999999999,
                0.9038333333333332,
                0.6699999999999998,
            ],
            text: "Latent\nResidual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.4650000000000001, 0.5674999999999999, 0.577, 0.6675],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "output_pred".to_string(),
            bbox: [
                0.8599999999999999,
                0.76,
                0.9599999999999999,
                0.8600000000000001,
            ],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 25,
        },
        DrawObject::Connector {
            id: "e_input_student".to_string(),
            points: vec![
                [0.222, 0.59375],
                [0.222, 0.8125],
                [0.4078333333333334, 0.8125],
            ],
            from: Some("input_text".to_string()),
            to: Some("student_model".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_input_teacher".to_string(),
            points: vec![
                [0.222, 0.59375],
                [0.3938333333333334, 0.59375],
                [0.3938333333333334, 0.375],
            ],
            from: Some("input_text".to_string()),
            to: Some("teacher_model".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_teacher_residual".to_string(),
            points: vec![
                [0.6198333333333333, 0.375],
                [0.7238333333333333, 0.375],
                [0.7238333333333333, 0.5399999999999999],
            ],
            from: Some("teacher_model".to_string()),
            to: Some("latent_residual_supervision".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "h_T − h_S".to_string(),
                bbox: [
                    0.7478333333333333,
                    0.43249999999999994,
                    0.8378333333333333,
                    0.4825,
                ],
            }),
            z: 12,
        },
        DrawObject::Connector {
            id: "e_residual_student".to_string(),
            points: vec![
                [0.7238333333333333, 0.6049999999999999],
                [0.6338333333333334, 0.6049999999999999],
                [0.6338333333333334, 0.7225],
            ],
            from: Some("latent_residual_supervision".to_string()),
            to: Some("student_model".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Connector {
            id: "e_student_task".to_string(),
            points: vec![[0.521, 0.7225], [0.521, 0.6675]],
            from: Some("student_model".to_string()),
            to: Some("task_loss".to_string()),
            style: "normal_flow".to_string(),
            label: Some(DrawLabel {
                text: "L_task".to_string(),
                bbox: [0.40700000000000003, 0.67, 0.497, 0.7200000000000001],
            }),
            z: 14,
        },
        DrawObject::Connector {
            id: "e_student_output".to_string(),
            points: vec![[0.6338333333333334, 0.81], [0.8599999999999999, 0.81]],
            from: Some("student_model".to_string()),
            to: Some("output_pred".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 15,
        },
        DrawObject::Text {
            id: "inference_note".to_string(),
            bbox: [0.4255833333333333, 0.915, 0.6160833333333333, 0.98],
            text: "Inference: student only".to_string(),
            style: "annotation".to_string(),
            z: 30,
        },
        DrawObject::Text {
            id: "ann_inference".to_string(),
            bbox: [0.4308333333333334, 0.915, 0.6108333333333333, 0.98],
            text: "Inference →".to_string(),
            style: "annotation".to_string(),
            z: 31,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let inference_count = plan
        .objects
        .iter()
        .filter(|object| match object {
            DrawObject::Text { text, .. } | DrawObject::Box { text, .. } => {
                text.to_lowercase().contains("inference")
            }
            _ => false,
        })
        .count();
    assert!(
        inference_count <= 1,
        "overlapping duplicate inference annotations should collapse to at most one visible note, count={inference_count}"
    );

    let task_loss = object_box(&plan, "task_loss");
    let teacher = object_box(&plan, "teacher_model");
    let student = object_box(&plan, "student_model");
    assert!(
        !task_loss_sits_between_rows_for_test(task_loss, teacher, student)
            && task_loss[0] >= student[2] + 0.030,
        "task loss should move to the student side, outside the teacher/student corridor: task={task_loss:?}, teacher={teacher:?}, student={student:?}"
    );

    let task_edge = connector(&plan, "e_student_task");
    assert!(
        task_edge
            .points
            .windows(2)
            .all(|window| window[1][1] + 0.001 >= window[0][1]),
        "student-to-task route should not go upward against reading direction: {:?}",
        task_edge.points
    );
}

#[test]
fn model_draw_plan_polish_handles_narrow_two_stage_student_head_without_clamp_panic() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "student_encoder".to_string(),
            bbox: [0.08, 0.20, 0.22, 0.34],
            text: "Student Encoder".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student_head".to_string(),
            bbox: [0.085, 0.42, 0.115, 0.52],
            text: "Student Head".to_string(),
            role: "output".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "teacher_encoder".to_string(),
            bbox: [0.60, 0.20, 0.76, 0.34],
            text: "Teacher Encoder".to_string(),
            role: "module".to_string(),
            style: "muted_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "teacher_head".to_string(),
            bbox: [0.60, 0.42, 0.76, 0.52],
            text: "Teacher Head".to_string(),
            role: "module".to_string(),
            style: "muted_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.08, 0.61, 0.22, 0.71],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 24,
        },
        DrawObject::Connector {
            id: "student_to_loss".to_string(),
            points: vec![[0.10, 0.52], [0.15, 0.61]],
            from: Some("student_head".to_string()),
            to: Some("task_loss".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let route = connector(&plan, "student_to_loss");
    assert!(
        route
            .points
            .iter()
            .all(|point| point[0].is_finite() && point[1].is_finite()),
        "narrow student head should not create invalid reroute points: {:?}",
        route.points
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
fn model_draw_plan_polish_moves_task_loss_label_off_head_and_edge_from_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input_text".to_string(),
            bbox: [0.065, 0.4608333333333333, 0.185, 0.6016666666666667],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher_enc".to_string(),
            bbox: [0.24543333333333336, 0.2006, 0.4629, 0.3409],
            text: "Teacher\nEncoder".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "teacher_proj".to_string(),
            bbox: [0.24543333333333336, 0.3839, 0.4629, 0.5242],
            text: "Teacher\nProjection".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "student_enc".to_string(),
            bbox: [0.20025, 0.6621000000000001, 0.42025, 0.7904],
            text: "Student\nEncoder".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "student_proj".to_string(),
            bbox: [0.47025, 0.6621000000000001, 0.69025, 0.7904],
            text: "Student\nProjection".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "student_head".to_string(),
            bbox: [0.20025, 0.8204, 0.42025, 0.9487],
            text: "Task Head".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 25,
        },
        DrawObject::Box {
            id: "residual_align".to_string(),
            bbox: [0.5696666666666665, 0.435, 0.722, 0.565],
            text: "Latent Residual\nL2 Align".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 26,
        },
        DrawObject::Box {
            id: "task_ce".to_string(),
            bbox: [
                0.03924999999999998,
                0.8799999999999999,
                0.20325000000000001,
                0.98,
            ],
            text: "Task Loss\nCE(y, ŷ)".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 27,
        },
        DrawObject::Box {
            id: "output_pred".to_string(),
            bbox: [0.47025000000000006, 0.80455, 0.69025, 0.9045500000000001],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 28,
        },
        DrawObject::Connector {
            id: "e_input_teacher".to_string(),
            points: vec![
                [0.185, 0.53125],
                [0.185, 0.27075],
                [0.24543333333333336, 0.27075],
            ],
            from: Some("input_text".to_string()),
            to: Some("teacher_enc".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_input_student".to_string(),
            points: vec![
                [0.125, 0.6016666666666667],
                [0.125, 0.6318833333333334],
                [0.31025, 0.6318833333333334],
                [0.31025, 0.6621000000000001],
            ],
            from: Some("input_text".to_string()),
            to: Some("student_enc".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_teacher_enc_proj".to_string(),
            points: vec![[0.3541666666666667, 0.3409], [0.3541666666666667, 0.3839]],
            from: Some("teacher_enc".to_string()),
            to: Some("teacher_proj".to_string()),
            style: "main_flow".to_string(),
            label: Some(DrawLabel {
                text: "z_t".to_string(),
                bbox: [
                    0.2841666666666667,
                    0.5362,
                    0.4241666666666667,
                    0.5862000000000002,
                ],
            }),
            z: 12,
        },
        DrawObject::Connector {
            id: "e_student_enc_proj".to_string(),
            points: vec![[0.42025, 0.7262500000000001], [0.47025, 0.7262500000000001]],
            from: Some("student_enc".to_string()),
            to: Some("student_proj".to_string()),
            style: "main_flow".to_string(),
            label: Some(DrawLabel {
                text: "z_s".to_string(),
                bbox: [0.70225, 0.70125, 0.84225, 0.7512500000000001],
            }),
            z: 13,
        },
        DrawObject::Connector {
            id: "e_student_proj_head".to_string(),
            points: vec![[0.47025, 0.7904], [0.42025, 0.7904], [0.42025, 0.8204]],
            from: Some("student_proj".to_string()),
            to: Some("student_head".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 14,
        },
        DrawObject::Connector {
            id: "e_student_head_out".to_string(),
            points: vec![[0.42025, 0.85455], [0.47025000000000006, 0.85455]],
            from: Some("student_head".to_string()),
            to: Some("output_pred".to_string()),
            style: "normal_flow".to_string(),
            label: Some(DrawLabel {
                text: "ŷ".to_string(),
                bbox: [0.43, 0.06, 0.5700000000000001, 0.11000000000000004],
            }),
            z: 15,
        },
        DrawObject::Connector {
            id: "e_residual_up".to_string(),
            points: vec![
                [0.4629, 0.45405],
                [0.5696666666666665, 0.45405],
                [0.5696666666666665, 0.5],
            ],
            from: Some("teacher_proj".to_string()),
            to: Some("residual_align".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "z_t".to_string(),
                bbox: [0.4629, 0.38604999999999995, 0.5259, 0.43605],
            }),
            z: 16,
        },
        DrawObject::Connector {
            id: "e_residual_down".to_string(),
            points: vec![
                [0.6458333333333333, 0.6621000000000001],
                [0.6458333333333333, 0.565],
            ],
            from: Some("student_proj".to_string()),
            to: Some("residual_align".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "z_s".to_string(),
                bbox: [
                    0.6638333333333333,
                    0.58855,
                    0.7268333333333332,
                    0.6385500000000001,
                ],
            }),
            z: 17,
        },
        DrawObject::Connector {
            id: "e_task_loss".to_string(),
            points: vec![[0.31025, 0.9487], [0.20325000000000001, 0.9299999999999999]],
            from: Some("student_head".to_string()),
            to: Some("task_ce".to_string()),
            style: "normal_flow".to_string(),
            label: Some(DrawLabel {
                text: "ŷ, y".to_string(),
                bbox: [
                    0.24025000000000002,
                    0.8899999999999999,
                    0.38025000000000003,
                    0.94,
                ],
            }),
            z: 18,
        },
        DrawObject::Text {
            id: "ann_inference".to_string(),
            bbox: [
                0.32825000000000004,
                0.5538833333333333,
                0.4882500000000001,
                0.6138833333333333,
            ],
            text: "Inference: student only".to_string(),
            style: "annotation".to_string(),
            z: 29,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let edge = connector(&plan, "e_task_loss");
    assert!(
        edge.label.is_none(),
        "redundant task-loss connector labels should be removed when the loss box already contains CE(y, y_hat): {:?}",
        edge.label
    );
    assert!(
        box_text(&plan, "task_ce").contains("CE"),
        "removing the connector label must preserve the editable loss formula in the task-loss box"
    );
}

#[test]
fn model_draw_plan_polish_widens_narrow_latent_residual_loss_from_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "latent_residual".to_string(),
            bbox: [
                0.5506666666666667,
                0.4866666666666667,
                0.6506666666666668,
                0.5966666666666667,
            ],
            text: "Latent Residual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [
                0.6483333333333332,
                0.5791666666666666,
                0.7683333333333333,
                0.6891666666666667,
            ],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 24,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let residual = object_box(&plan, "latent_residual");
    assert!(
        residual[2] - residual[0] >= 0.13,
        "Latent Residual loss box should be wide enough for the longest token at paper width: {residual:?}"
    );
    let overlap = intersection_area(residual, object_box(&plan, "task_loss"));
    assert!(
        overlap < 0.0001,
        "widening the residual box should not create a visible overlap with Task Loss: overlap={overlap}, residual={residual:?}"
    );
}

#[test]
fn model_draw_plan_polish_compacts_oversized_student_box_from_loss_width_smoke() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher latent residual supervision trains a compact student.",
            "visual_focus": ["student", "task_output", "inference_note"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 8},
            "regions": [
                {"id": "student_region", "bbox": [0.3333333333333333, 0.5, 0.9166666666666666, 1.0]},
                {"id": "task_output_region", "bbox": [0.6666666666666666, 0.625, 1.0, 1.0]},
                {"id": "inference_note_region", "bbox": [0.75, 0.84375, 1.0, 1.0]}
            ]
        },
        "components": [
            {"id": "student", "label": "Student\\n(compact)", "role": "main", "visual_weight": "strong", "region": "student_region", "allowed_asset_id": null},
            {"id": "task_output", "label": "Task output ŷ", "role": "output", "visual_weight": "normal", "region": "task_output_region", "allowed_asset_id": null},
            {"id": "inference_note", "label": "Inference: student only", "role": "context", "visual_weight": "muted", "region": "inference_note_region", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "student_to_output", "from": "student", "to": "task_output", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"}
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
    .expect("smoke-derived figure plan should deserialize");
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "student".to_string(),
            bbox: [0.36133333333333334, 0.528, 0.8886666666666666, 0.972],
            text: "Student\n(compact)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "task_output".to_string(),
            bbox: [0.88, 0.6799999999999999, 0.98, 0.8200000000000001],
            text: "Task output ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "inference_note".to_string(),
            bbox: [0.11508333333333332, 0.69375, 0.32133333333333336, 0.80625],
            text: "Inference: student only".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 25,
        },
        DrawObject::Connector {
            id: "student_to_output".to_string(),
            points: vec![[0.8886666666666666, 0.75], [0.88, 0.75]],
            from: Some("student".to_string()),
            to: Some("task_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 12,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    let student = object_box(&plan, "student");
    assert!(
        student[2] - student[0] <= 0.28 && student[3] - student[1] <= 0.24,
        "short Student box should be compacted instead of keeping huge internal whitespace: {student:?}"
    );
    assert_eq!(
        intersection_area(student, object_box(&plan, "task_output")),
        0.0,
        "compacting the student box should separate it from the output box"
    );
}

#[test]
fn model_draw_plan_polish_compacts_full_loss_width_smoke_student_box() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher latent residual supervision trains a compact student.",
            "visual_focus": ["input", "student", "teacher", "latent_residual", "task_output", "inference_note"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 8},
            "regions": [
                {"id": "input_region", "bbox": [0.08333333333333333, 0.375, 0.29166666666666663, 0.6875]},
                {"id": "student_region", "bbox": [0.3333333333333333, 0.5, 0.9166666666666666, 1.0]},
                {"id": "teacher_region", "bbox": [0.5, 0.1875, 0.75, 0.4375]},
                {"id": "latent_residual_region", "bbox": [0.75, 0.0, 0.9583333333333334, 0.21875]},
                {"id": "task_output_region", "bbox": [0.6666666666666666, 0.625, 1.0, 1.0]},
                {"id": "inference_note_region", "bbox": [0.75, 0.84375, 1.0, 1.0]}
            ]
        },
        "components": [
            {"id": "input", "label": "Task input x", "role": "input", "visual_weight": "normal", "region": "input_region", "allowed_asset_id": null},
            {"id": "student", "label": "Student\\n(compact)", "role": "main", "visual_weight": "strong", "region": "student_region", "allowed_asset_id": null},
            {"id": "teacher", "label": "Teacher\\n(large LM)", "role": "module", "visual_weight": "normal", "region": "teacher_region", "allowed_asset_id": null},
            {"id": "latent_residual", "label": "Latent residual\\nsupervision", "role": "loss", "visual_weight": "normal", "region": "latent_residual_region", "allowed_asset_id": null},
            {"id": "task_output", "label": "Task output ŷ", "role": "output", "visual_weight": "normal", "region": "task_output_region", "allowed_asset_id": null},
            {"id": "inference_note", "label": "Inference: student only", "role": "context", "visual_weight": "muted", "region": "inference_note_region", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "e_input_student", "from": "input", "to": "student", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_input_teacher", "from": "input", "to": "teacher", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_teacher_residual", "from": "teacher", "to": "latent_residual", "label": "h_T", "semantic": "supervision", "style": "dash", "importance": "normal"},
            {"id": "e_student_residual", "from": "student", "to": "latent_residual", "label": "h_S", "semantic": "supervision", "style": "dash", "importance": "normal"},
            {"id": "e_student_output", "from": "student", "to": "task_output", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"}
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
    .expect("smoke-derived figure plan should deserialize");
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input".to_string(),
            bbox: [0.11133333333333333, 0.43425, 0.26366666666666666, 0.62825],
            text: "Task input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student".to_string(),
            bbox: [0.36133333333333334, 0.528, 0.8886666666666666, 0.972],
            text: "Student\n(compact)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "teacher".to_string(),
            bbox: [0.512, 0.2225, 0.738, 0.40249999999999997],
            text: "Teacher\n(large LM)".to_string(),
            role: "module".to_string(),
            style: "neutral_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "latent_residual".to_string(),
            bbox: [
                0.7333333333333333,
                0.03750000000000006,
                0.9333333333333332,
                0.1675,
            ],
            text: "Latent residual\nsupervision".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "task_output".to_string(),
            bbox: [0.88, 0.6799999999999999, 0.98, 0.8200000000000001],
            text: "Task output ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "inference_note".to_string(),
            bbox: [0.11508333333333332, 0.69375, 0.32133333333333336, 0.80625],
            text: "Inference: student only".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 25,
        },
        DrawObject::Connector {
            id: "e_input_student".to_string(),
            points: vec![
                [0.26366666666666666, 0.53125],
                [0.36133333333333334, 0.53125],
            ],
            from: Some("input".to_string()),
            to: Some("student".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_input_teacher".to_string(),
            points: vec![
                [0.26366666666666666, 0.53125],
                [0.26366666666666666, 0.3125],
                [0.512, 0.3125],
            ],
            from: Some("input".to_string()),
            to: Some("teacher".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_teacher_residual".to_string(),
            points: vec![
                [0.625, 0.40249999999999997],
                [0.625, 0.1675],
                [0.8333333333333333, 0.1675],
            ],
            from: Some("teacher".to_string()),
            to: Some("latent_residual".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "h_T".to_string(),
                bbox: [0.544, 0.1675, 0.607, 0.21750000000000008],
            }),
            z: 12,
        },
        DrawObject::Connector {
            id: "e_student_residual".to_string(),
            points: vec![[0.8333333333333333, 0.528], [0.8333333333333333, 0.1675]],
            from: Some("student".to_string()),
            to: Some("latent_residual".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "h_S".to_string(),
                bbox: [0.7523333333333333, 0.32275, 0.8153333333333332, 0.37275],
            }),
            z: 13,
        },
        DrawObject::Connector {
            id: "e_student_output".to_string(),
            points: vec![[0.625, 0.75], [0.88, 0.75]],
            from: Some("student".to_string()),
            to: Some("task_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 14,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    let student = object_box(&plan, "student");
    assert!(
        student[2] - student[0] <= 0.28 && student[3] - student[1] <= 0.24,
        "full smoke student box should be compacted despite adjacent output/residual edges: {student:?}"
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
fn model_draw_plan_polish_recenters_top_blank_teacher_student_group_from_latest_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "task_input".to_string(),
            bbox: [
                0.05416666666666667,
                0.42,
                0.15416666666666667,
                0.5299999999999999,
            ],
            text: "Task Input".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 10,
        },
        DrawObject::Box {
            id: "student_model".to_string(),
            bbox: [0.31766666666666665, 0.3975, 0.5156666666666666, 0.5375],
            text: "Student\nModel".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher_model".to_string(),
            bbox: [0.6113333333333334, 0.3975, 0.8093333333333333, 0.5375],
            text: "Teacher\nModel".to_string(),
            role: "context".to_string(),
            style: "primary_module_teacher_muted_dashed".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "task_output".to_string(),
            bbox: [
                0.41666666666666674,
                0.77,
                0.5766666666666667,
                0.8700000000000001,
            ],
            text: "Task Output".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "latent_residual".to_string(),
            bbox: [
                0.6056666666666666,
                0.6124999999999999,
                0.8066666666666666,
                0.7224999999999999,
            ],
            text: "Latent Residual\nSupervision".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 20,
        },
        DrawObject::Text {
            id: "student_label".to_string(),
            bbox: [
                0.17216666666666666,
                0.493,
                0.3121666666666666,
                0.5529999999999999,
            ],
            text: "Student".to_string(),
            style: "annotation".to_string(),
            z: 30,
        },
        DrawObject::Text {
            id: "teacher_label".to_string(),
            bbox: [0.6473333333333333, 0.3, 0.7873333333333333, 0.36],
            text: "Teacher".to_string(),
            style: "annotation".to_string(),
            z: 30,
        },
        DrawObject::Text {
            id: "inference_note".to_string(),
            bbox: [0.20141666666666674, 0.785, 0.3614166666666667, 0.855],
            text: "Inference: student only".to_string(),
            style: "annotation".to_string(),
            z: 30,
        },
        DrawObject::Connector {
            id: "e_input_student".to_string(),
            points: vec![[0.15416666666666667, 0.4675], [0.31766666666666665, 0.4675]],
            from: Some("task_input".to_string()),
            to: Some("student_model".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 5,
        },
        DrawObject::Connector {
            id: "e_student_output".to_string(),
            points: vec![[0.4966666666666667, 0.5375], [0.4966666666666667, 0.77]],
            from: Some("student_model".to_string()),
            to: Some("task_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 5,
        },
        DrawObject::Connector {
            id: "e_input_teacher".to_string(),
            points: vec![
                [0.15416666666666667, 0.475],
                [0.15416666666666667, 0.3725],
                [0.6113333333333334, 0.3725],
                [0.6113333333333334, 0.3975],
            ],
            from: Some("task_input".to_string()),
            to: Some("teacher_model".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 5,
        },
        DrawObject::Connector {
            id: "e_teacher_residual".to_string(),
            points: vec![
                [0.7061666666666666, 0.5375],
                [0.7061666666666666, 0.6124999999999999],
            ],
            from: Some("teacher_model".to_string()),
            to: Some("latent_residual".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 5,
        },
        DrawObject::Connector {
            id: "e_residual_student".to_string(),
            points: vec![
                [0.6056666666666666, 0.6675],
                [0.5156666666666666, 0.6675],
                [0.5156666666666666, 0.5375],
            ],
            from: Some("latent_residual".to_string()),
            to: Some("student_model".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 5,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let union = [
        object_box(&plan, "task_input"),
        object_box(&plan, "student_model"),
        object_box(&plan, "teacher_model"),
        object_box(&plan, "task_output"),
        object_box(&plan, "latent_residual"),
    ]
    .into_iter()
    .reduce(union_box_for_test)
    .expect("component union exists");
    let top_blank = union[1];
    let bottom_blank = 1.0 - union[3];
    assert!(
        (top_blank - bottom_blank).abs() <= 0.12,
        "main group should be vertically rebalanced instead of staying bottom-heavy: union={union:?}"
    );
    assert!(
        union[1] < 0.34 && union[3] < 0.82,
        "group should move up from the latest smoke geometry rather than farther down: union={union:?}"
    );
    let output_edge = connector(&plan, "e_student_output");
    let student = object_box(&plan, "student_model");
    let output = object_box(&plan, "task_output");
    assert!(
        output_edge.points[0][0] >= student[0] && output_edge.points[0][0] <= student[2],
        "student-output edge should still start inside the moved student box: student={student:?}, edge={:?}",
        output_edge.points
    );
    assert!(
        (output_edge.points[0][1] - student[3]).abs() < 0.0001,
        "student-output edge start y should move with student bottom: student={student:?}, edge={:?}",
        output_edge.points
    );
    assert_point_close(output_edge.points[1], [center_x(output), output[1]]);
}

#[test]
fn model_draw_plan_polish_keeps_short_direct_connectors_without_dogleg() {
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
    assert_eq!(
        edge.points,
        &[[0.36, 0.14], [0.36, 0.18]],
        "short direct connector should not be expanded into a dogleg: {:?}",
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

#[test]
fn model_draw_plan_polish_unmerges_task_loss_and_reroutes_residual_crossing_from_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_teacher_encode".to_string(),
            bbox: [0.3196666666666667, 0.18, 0.58, 0.38],
            text: "Teacher\nEncoder".to_string(),
            role: "module".to_string(),
            style: "neutral_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "comp_teacher_latent".to_string(),
            bbox: [0.4600000000000001, 0.51, 0.58, 0.6100000000000001],
            text: "Latent\nResidual".to_string(),
            role: "output".to_string(),
            style: "accent_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "comp_student_encode".to_string(),
            bbox: [0.3196666666666667, 0.68, 0.58, 0.88],
            text: "Student\nEncoder".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "comp_student_predict".to_string(),
            bbox: [
                0.6446666666666667,
                0.39499999999999996,
                0.8166666666666667,
                0.5249999999999999,
            ],
            text: "Student\nPredict\n+ Task Loss".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "comp_task_output".to_string(),
            bbox: [0.87, 0.144, 1.0, 0.284],
            text: "Prediction".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 25,
        },
        DrawObject::Box {
            id: "comp_task_loss".to_string(),
            bbox: [0.7096666666666668, 0.8875, 0.8736666666666667, 0.9875],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 26,
        },
        DrawObject::Box {
            id: "comp_latent_residual_loss".to_string(),
            bbox: [0.78, 0.58, 0.95, 0.68],
            text: "Latent Residual\nSupervision".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 27,
        },
        DrawObject::Connector {
            id: "edge_student_encode_to_predict".to_string(),
            points: vec![
                [0.58, 0.78],
                [0.62, 0.78],
                [0.62, 0.55],
                [0.6753333333333333, 0.55],
                [0.6753333333333333, 0.5249999999999999],
                [0.7306666666666667, 0.5249999999999999],
            ],
            from: Some("comp_student_encode".to_string()),
            to: Some("comp_student_predict".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Connector {
            id: "edge_latent_to_residual_loss".to_string(),
            points: vec![[0.58, 0.595], [0.78, 0.595]],
            from: Some("comp_teacher_latent".to_string()),
            to: Some("comp_latent_residual_loss".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 16,
        },
        DrawObject::Connector {
            id: "edge_predict_to_task_loss".to_string(),
            points: vec![
                [0.7306666666666667, 0.5249999999999999],
                [0.96, 0.5249999999999999],
                [0.96, 0.8875],
                [0.8736666666666667, 0.8875],
            ],
            from: Some("comp_student_predict".to_string()),
            to: Some("comp_task_loss".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 15,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    assert!(
        !box_text(&plan, "comp_student_predict")
            .to_lowercase()
            .contains("task loss"),
        "student prediction module should not duplicate an existing task loss box"
    );
    let output = object_box(&plan, "comp_task_output");
    assert!(
        output[2] - output[0] >= 0.15 && output[2] <= 1.0,
        "Prediction output should be wide enough for its longest token: {output:?}"
    );
    let student_predict = connector(&plan, "edge_student_encode_to_predict");
    let residual = connector(&plan, "edge_latent_to_residual_loss");
    assert!(
        !connectors_cross(&student_predict.points, &residual.points),
        "residual supervision route should not cross the student main route: main={:?}, residual={:?}",
        student_predict.points,
        residual.points
    );
}

#[test]
fn model_draw_plan_polish_compacts_oversized_short_main_boxes_from_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_teacher".to_string(),
            bbox: [
                0.23633333333333334,
                0.11133333333333333,
                0.5553333333333333,
                0.472,
            ],
            text: "Teacher\n(Large LM)".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "comp_student".to_string(),
            bbox: [
                0.23633333333333334,
                0.6113333333333334,
                0.5553333333333333,
                0.972,
            ],
            text: "Student\n(Compact)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "comp_latent_residual".to_string(),
            bbox: [
                0.6471666666666667,
                0.45633333333333326,
                0.8111666666666666,
                0.5863333333333334,
            ],
            text: "Latent\nResidual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "comp_task_loss".to_string(),
            bbox: [
                0.4959999999999999,
                0.4563333333333335,
                0.628,
                0.5863333333333334,
            ],
            text: "Task\nLoss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "comp_output".to_string(),
            bbox: [0.653, 0.6113333333333334, 0.972, 0.972],
            text: "Output".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 25,
        },
        DrawObject::Connector {
            id: "edge_student_to_output".to_string(),
            points: vec![
                [0.5553333333333333, 0.7916666666666667],
                [0.653, 0.7916666666666667],
            ],
            from: Some("comp_student".to_string()),
            to: Some("comp_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 14,
        },
        DrawObject::Connector {
            id: "edge_student_to_task_loss".to_string(),
            points: vec![
                [0.39583333333333337, 0.7916666666666667],
                [0.39583333333333337, 0.5863333333333334],
                [0.5619999999999999, 0.5863333333333334],
            ],
            from: Some("comp_student".to_string()),
            to: Some("comp_task_loss".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 15,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let teacher = object_box(&plan, "comp_teacher");
    let student = object_box(&plan, "comp_student");
    let output = object_box(&plan, "comp_output");
    let task_loss = object_box(&plan, "comp_task_loss");
    let residual = object_box(&plan, "comp_latent_residual");
    assert!(
        box_area(teacher) < 0.055 && box_area(student) < 0.055 && box_area(output) < 0.040,
        "short labels should not sit in huge empty boxes: teacher={teacher:?}, student={student:?}, output={output:?}"
    );
    assert!(
        vertical_separation(student, task_loss) >= 0.045,
        "compacting student should leave a visible gutter above task loss: student={student:?}, task_loss={task_loss:?}"
    );
    assert!(
        vertical_separation(output, residual) >= 0.045,
        "compacting output should leave a visible gutter from residual: output={output:?}, residual={residual:?}"
    );
}

#[test]
fn model_draw_plan_polish_compacts_tall_short_input_main_and_output_boxes_from_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_input".to_string(),
            bbox: [
                0.023333333333333334,
                0.44,
                0.1433333333333333,
                0.9766666666666667,
            ],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "comp_teacher".to_string(),
            bbox: [0.278, 0.11133333333333333, 0.472, 0.472],
            text: "Teacher\n(frozen)".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "comp_student".to_string(),
            bbox: [0.278, 0.6113333333333334, 0.472, 0.972],
            text: "Student".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "comp_task_output".to_string(),
            bbox: [0.815, 0.6066666666666667, 0.935, 0.9766666666666667],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 23,
        },
        DrawObject::Connector {
            id: "edge_input_teacher".to_string(),
            points: vec![
                [0.1433333333333333, 0.7083333333333334],
                [0.278, 0.2916666666666667],
            ],
            from: Some("comp_input".to_string()),
            to: Some("comp_teacher".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "edge_input_student".to_string(),
            points: vec![
                [0.1433333333333333, 0.7083333333333334],
                [0.278, 0.7916666666666667],
            ],
            from: Some("comp_input".to_string()),
            to: Some("comp_student".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "edge_student_output".to_string(),
            points: vec![[0.472, 0.7916666666666667], [0.815, 0.7916666666666667]],
            from: Some("comp_student".to_string()),
            to: Some("comp_task_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 12,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let input = object_box(&plan, "comp_input");
    let student = object_box(&plan, "comp_student");
    let output = object_box(&plan, "comp_task_output");
    assert!(
        input[3] - input[1] <= 0.18 && box_area(input) < 0.025,
        "short input text should not remain in a tall empty box: {input:?}"
    );
    assert!(
        student[3] - student[1] <= 0.18 && box_area(student) < 0.035,
        "short student text should not remain in a tall empty box: {student:?}"
    );
    assert!(
        output[3] - output[1] <= 0.18 && box_area(output) < 0.025,
        "single-token output should not remain in a tall empty box: {output:?}"
    );

    let edge = connector(&plan, "edge_student_output");
    assert!(
        (edge.points[0][0] - student[2]).abs() < 0.001
            && (edge.points[1][0] - output[0]).abs() < 0.001,
        "connectors should realign to compacted boxes: {:?}",
        edge.points
    );
}

#[test]
fn model_draw_plan_polish_moves_task_loss_out_of_student_output_lane_from_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "student_block".to_string(),
            bbox: [
                0.30366666666666664,
                0.7016666666666668,
                0.5296666666666666,
                0.8816666666666667,
            ],
            text: "Student\n(compact)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.6, 0.7366666666666667, 0.76, 0.8466666666666668],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "output".to_string(),
            bbox: [0.8875, 0.74, 0.9875, 0.8400000000000001],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 25,
        },
        DrawObject::Connector {
            id: "e_student_taskloss".to_string(),
            points: vec![
                [0.5296666666666666, 0.7916666666666667],
                [0.6, 0.7916666666666667],
            ],
            from: Some("student_block".to_string()),
            to: Some("task_loss".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 15,
        },
        DrawObject::Connector {
            id: "e_student_output".to_string(),
            points: vec![
                [0.5296666666666666, 0.7916666666666667],
                [0.5296666666666666, 0.8716666666666668],
                [0.8875, 0.8716666666666668],
                [0.8875, 0.79],
            ],
            from: Some("student_block".to_string()),
            to: Some("output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 17,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let student = object_box(&plan, "student_block");
    let output = object_box(&plan, "output");
    let task_loss = object_box(&plan, "task_loss");
    let student_output_lane = [
        student[0].min(output[0]),
        student[1].min(output[1]),
        student[2].max(output[2]),
        student[3].max(output[3]),
    ];
    assert!(
        vertical_separation(task_loss, student_output_lane) >= 0.03,
        "task loss should move out of the student-output main lane: task_loss={task_loss:?}, student={student:?}, output={output:?}"
    );
    let edge = connector(&plan, "e_student_output");
    assert!(
        edge.points.len() <= 2,
        "student-output connector should return to a direct route once task loss leaves the lane: {:?}",
        edge.points
    );
}

#[test]
fn model_draw_plan_polish_removes_redundant_phase_loss_and_inference_notes_from_smoke() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Student distillation uses a latent residual during training and keeps only the compact student at inference.",
            "visual_focus": ["teacher_encoder", "student_encoder", "latent_residual", "task_output"],
            "reading_order": "top_to_bottom"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 8},
            "regions": [
                {"id": "teacher_region", "bbox": [0.3383333333333333, 0.25375, 0.5783333333333334, 0.43375]},
                {"id": "student_region", "bbox": [0.3453333333333333, 0.69125, 0.5713333333333334, 0.87125]},
                {"id": "residual_region", "bbox": [0.5946666666666667, 0.7125, 0.7946666666666666, 0.8125]},
                {"id": "output_region", "bbox": [0.8196666666666667, 0.73, 0.995, 0.86]},
                {"id": "inference_note_region", "bbox": [0.3683333333333334, 0.50125, 0.5483333333333333, 0.64125]}
            ]
        },
        "components": [
            {"id": "teacher_encoder", "label": "Teacher LM\n(large, frozen)", "role": "module", "visual_weight": "muted", "region": "teacher_region", "allowed_asset_id": null},
            {"id": "student_encoder", "label": "Student\n(compact)", "role": "main", "visual_weight": "strong", "region": "student_region", "allowed_asset_id": null},
            {"id": "latent_residual", "label": "Latent Residual", "role": "loss", "visual_weight": "normal", "region": "residual_region", "allowed_asset_id": null},
            {"id": "task_output", "label": "Prediction ŷ", "role": "output", "visual_weight": "normal", "region": "output_region", "allowed_asset_id": null},
            {"id": "inference_only_note", "label": "Inference: student only", "role": "context", "visual_weight": "muted", "region": "inference_note_region", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "edge_teacher_to_residual", "from": "teacher_encoder", "to": "latent_residual", "label": "z_t", "semantic": "supervision", "style": "dash", "importance": "normal"},
            {"id": "edge_student_to_residual", "from": "student_encoder", "to": "latent_residual", "label": "z_s", "semantic": "supervision", "style": "dash", "importance": "normal"},
            {"id": "edge_student_to_output", "from": "student_encoder", "to": "task_output", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"}
        ],
        "annotations": [
            {"id": "anno_task_loss", "label": "Task Loss", "target_id": "edge_student_to_output", "bbox": [0.625, 0.8125, 1.0, 1.0]},
            {"id": "anno_training_only", "label": "training only", "target_id": "teacher_encoder", "bbox": [0.2916666666666667, 0.0375, 0.75, 0.1625]}
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
    .expect("smoke-derived figure plan should deserialize");
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "teacher_encoder".to_string(),
            bbox: [0.3383333333333333, 0.25375, 0.5783333333333334, 0.43375],
            text: "Teacher LM\n(large, frozen, training only)".to_string(),
            role: "module".to_string(),
            style: "muted_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student_encoder".to_string(),
            bbox: [0.3453333333333333, 0.69125, 0.5713333333333334, 0.87125],
            text: "Student\n(compact, inference-only)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "latent_residual".to_string(),
            bbox: [0.5946666666666667, 0.7125, 0.7946666666666666, 0.8125],
            text: "Latent Residual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "task_output".to_string(),
            bbox: [0.8196666666666667, 0.73, 0.995, 0.86],
            text: "Prediction ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Connector {
            id: "edge_teacher_to_residual".to_string(),
            points: vec![
                [0.5783333333333334, 0.34375],
                [0.6708333333333333, 0.34375],
                [0.6946666666666667, 0.7125],
            ],
            from: Some("teacher_encoder".to_string()),
            to: Some("latent_residual".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "z_t".to_string(),
                bbox: [0.70075, 0.503125, 0.86075, 0.553125],
            }),
            z: 12,
        },
        DrawObject::Connector {
            id: "edge_student_to_residual".to_string(),
            points: vec![
                [0.5713333333333334, 0.78125],
                [0.5713333333333334, 0.7625],
                [0.5946666666666667, 0.7625],
            ],
            from: Some("student_encoder".to_string()),
            to: Some("latent_residual".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "z_s".to_string(),
                bbox: [0.3783333333333334, 0.88325, 0.5383333333333333, 0.93325],
            }),
            z: 13,
        },
        DrawObject::Connector {
            id: "edge_student_to_output".to_string(),
            points: vec![[0.5713333333333334, 0.78125], [0.8196666666666667, 0.795]],
            from: Some("student_encoder".to_string()),
            to: Some("task_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 14,
        },
        DrawObject::Box {
            id: "inference_only_note".to_string(),
            bbox: [0.3683333333333334, 0.50125, 0.5483333333333333, 0.64125],
            text: "Inference: student only".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 25,
        },
        DrawObject::Text {
            id: "anno_task_loss".to_string(),
            bbox: [0.37, 0.06, 0.63, 0.14],
            text: "Task Loss".to_string(),
            style: "annotation".to_string(),
            z: 26,
        },
        DrawObject::Text {
            id: "anno_training_only".to_string(),
            bbox: [0.2916666666666667, 0.0375, 0.75, 0.1625],
            text: "training only".to_string(),
            style: "annotation".to_string(),
            z: 27,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    assert!(
        !box_object_exists(&plan, "inference_only_note"),
        "student box already says inference-only; a separate protected note box should not occupy the main corridor"
    );
    assert!(
        box_text(&plan, "student_encoder")
            .to_lowercase()
            .contains("inference-only"),
        "removing the note must not erase the only inference semantics"
    );
    assert!(
        !text_object_exists(&plan, "anno_training_only"),
        "training-only is already encoded in the teacher label and should not be reinserted as a large top annotation"
    );
    assert!(
        !text_object_exists(&plan, "anno_task_loss"),
        "a detached Task Loss text annotation should not float without a loss box or connector label anchor"
    );
    let task_loss_edge = connector(&plan, "edge_student_to_output");
    let task_loss_label = task_loss_edge
        .label
        .as_ref()
        .expect("generic Task Loss annotation should be folded into the target connector label");
    assert_eq!(task_loss_label.text, "Task Loss");
    assert!(
        !label_intersects_any_segment(task_loss_label.bbox, task_loss_edge.points),
        "folded Task Loss label should sit beside the edge rather than on the stroke"
    );
}

#[test]
fn model_draw_plan_polish_folds_detached_protected_inference_and_residual_notes_from_smoke() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher supervises a student with latent residuals and student-only inference.",
            "visual_focus": ["comp_teacher", "comp_student", "comp_residual", "comp_task_output"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 8},
            "regions": [
                {"id": "teacher_branch", "bbox": [0.34, 0.35, 0.62, 0.50]},
                {"id": "student_branch", "bbox": [0.34, 0.58, 0.62, 0.73]},
                {"id": "objective_center", "bbox": [0.66, 0.32, 0.88, 0.45]},
                {"id": "output", "bbox": [0.80, 0.58, 0.98, 0.72]},
                {"id": "inference_note", "bbox": [0.34, 0.13, 0.52, 0.27]}
            ]
        },
        "components": [
            {"id": "comp_teacher", "label": "Teacher Model", "role": "context", "visual_weight": "muted", "region": "teacher_branch", "allowed_asset_id": null},
            {"id": "comp_student", "label": "Student Model", "role": "main", "visual_weight": "strong", "region": "student_branch", "allowed_asset_id": null},
            {"id": "comp_residual", "label": "Latent Residual", "role": "loss", "visual_weight": "strong", "region": "objective_center", "allowed_asset_id": null},
            {"id": "comp_task_output", "label": "Prediction", "role": "output", "visual_weight": "normal", "region": "output", "allowed_asset_id": null},
            {"id": "comp_inference", "label": "Inference: student only", "role": "context", "visual_weight": "muted", "region": "inference_note", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "edge_residual_to_student", "from": "comp_residual", "to": "comp_student", "label": "supervise", "semantic": "supervision", "style": "dash", "importance": "normal"},
            {"id": "edge_student_to_output", "from": "comp_student", "to": "comp_task_output", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"}
        ],
        "annotations": [
            {"id": "anno_residual_label", "label": "residual signal", "target_id": "edge_residual_to_student", "bbox": [0.4583333333333333, 0.475, 1.0, 1.0]}
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
    .expect("smoke-derived figure plan should deserialize");
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_student".to_string(),
            bbox: [0.3591666666666667, 0.58, 0.5991666666666666, 0.715],
            text: "Student Model".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "comp_residual".to_string(),
            bbox: [0.6708333333333333, 0.3271875, 0.8708333333333332, 0.4371875],
            text: "Latent Residual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "comp_task_output".to_string(),
            bbox: [0.8103333333333333, 0.58, 0.9733333333333333, 0.715],
            text: "Prediction".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "comp_inference".to_string(),
            bbox: [0.34099999999999997, 0.13, 0.5210000000000001, 0.27],
            text: "Inference: student only".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 26,
        },
        DrawObject::Connector {
            id: "edge_residual_to_student".to_string(),
            points: vec![
                [0.6708333333333333, 0.4371875],
                [0.6341666666666667, 0.4371875],
                [0.6341666666666667, 0.58],
                [0.5991666666666666, 0.58],
            ],
            from: Some("comp_residual".to_string()),
            to: Some("comp_student".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "supervise".to_string(),
                bbox: [0.42000000000000004, 0.06, 0.58, 0.11],
            }),
            z: 12,
        },
        DrawObject::Connector {
            id: "edge_student_to_output".to_string(),
            points: vec![[0.5991666666666666, 0.6475], [0.8103333333333333, 0.6475]],
            from: Some("comp_student".to_string()),
            to: Some("comp_task_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Text {
            id: "anno_residual_label".to_string(),
            bbox: [0.6799999999999999, 0.727, 0.94, 0.8069999999999999],
            text: "residual signal".to_string(),
            style: "annotation".to_string(),
            z: 28,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    assert!(
        !box_object_exists(&plan, "comp_inference"),
        "detached protected inference note should fold into a compact text annotation"
    );
    assert!(
        plan.objects.iter().any(|object| matches!(
            object,
            DrawObject::Text { text, .. } if text.to_lowercase().contains("inference")
        )),
        "folding the protected note must preserve inference semantics as editable text"
    );
    assert!(
        !text_object_exists(&plan, "anno_residual_label"),
        "residual signal annotation duplicates the Latent Residual node and should not be reinserted"
    );
    let edge = connector(&plan, "edge_residual_to_student");
    assert!(
        edge.label.is_none(),
        "generic supervise label is redundant beside a Latent Residual node"
    );
    assert!(
        edge.points.len() <= 3,
        "residual-to-student connector should not keep a 4-point detour: {:?}",
        edge.points
    );
}

#[test]
fn model_draw_plan_polish_folds_inference_component_near_student_from_figure_plan() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Student-only inference note is an auxiliary cue that should be folded into annotation.",
            "visual_focus": ["comp_student", "comp_inference"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 8},
            "regions": [
                {"id": "student_region", "bbox": [0.35, 0.58, 0.65, 0.77]},
                {"id": "inference_region", "bbox": [0.18, 0.62, 0.31, 0.77]},
            ]
        },
        "components": [
            {
                "id": "comp_student",
                "label": "Student Module",
                "role": "main",
                "visual_weight": "strong",
                "region": "student_region",
                "allowed_asset_id": null
            },
            {
                "id": "comp_inference",
                "label": "Inference: student only",
                "role": "context",
                "visual_weight": "muted",
                "region": "inference_region",
                "allowed_asset_id": null
            }
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
    })).expect("figure-plan fixture should deserialize");

    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_student".to_string(),
            bbox: [0.35, 0.58, 0.65, 0.77],
            text: "Student Module".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "comp_inference".to_string(),
            bbox: [0.18, 0.62, 0.31, 0.77],
            text: "Inference: student only".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 21,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    assert!(
        !box_object_exists(&plan, "comp_inference"),
        "detached standalone inference component should fold into text to avoid large corridor-wide boxes"
    );
    assert!(
        plan.objects.iter().any(|object| matches!(
            object,
            DrawObject::Text { text, .. } if text.to_lowercase().contains("inference")
        )),
        "folding should preserve inference semantics as editable text"
    );
}

#[test]
fn model_draw_plan_polish_folds_protected_inference_note_crowding_task_loss_from_smoke() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher-student distillation keeps student-only inference while using task loss.",
            "visual_focus": ["comp_student_encoder", "comp_student_head", "comp_task_loss"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 8},
            "regions": [
                {"id": "student_encoder", "bbox": [0.278, 0.58, 0.5, 0.73]},
                {"id": "student_head", "bbox": [0.85, 0.63, 0.99, 0.73]},
                {"id": "task_loss", "bbox": [0.776, 0.78, 0.94, 0.91]},
                {"id": "inference_note", "bbox": [0.403, 0.8, 0.597, 0.94]}
            ]
        },
        "components": [
            {"id": "comp_student_encoder", "label": "Student\nEncoder", "role": "main", "visual_weight": "strong", "region": "student_encoder", "allowed_asset_id": null},
            {"id": "comp_student_head", "label": "Student Head", "role": "output", "visual_weight": "normal", "region": "student_head", "allowed_asset_id": null},
            {"id": "comp_task_loss", "label": "Task Loss\nL_task", "role": "loss", "visual_weight": "normal", "region": "task_loss", "allowed_asset_id": null},
            {"id": "comp_inference_note", "label": "Inference: student only", "role": "context", "visual_weight": "muted", "region": "inference_note", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "edge_student_output", "from": "comp_student_encoder", "to": "comp_student_head", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "edge_head_loss", "from": "comp_student_head", "to": "comp_task_loss", "label": "ŷ", "semantic": "loss", "style": "solid", "importance": "normal"}
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
    .expect("smoke-derived figure plan should deserialize");
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_student_encoder".to_string(),
            bbox: [0.278, 0.58, 0.5, 0.73],
            text: "Student\nEncoder".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "comp_student_head".to_string(),
            bbox: [0.85, 0.63, 0.99, 0.73],
            text: "Student Head".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "comp_task_loss".to_string(),
            bbox: [0.776, 0.78, 0.94, 0.91],
            text: "Task Loss\nL_task".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "comp_inference_note".to_string(),
            bbox: [0.403, 0.8, 0.597, 0.94],
            text: "Inference: student only".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 25,
        },
        DrawObject::Connector {
            id: "edge_student_output".to_string(),
            points: vec![[0.5, 0.655], [0.85, 0.68]],
            from: Some("comp_student_encoder".to_string()),
            to: Some("comp_student_head".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "edge_head_loss".to_string(),
            points: vec![[0.858, 0.73], [0.858, 0.78]],
            from: Some("comp_student_head".to_string()),
            to: Some("comp_task_loss".to_string()),
            style: "loss_flow".to_string(),
            label: Some(DrawLabel {
                text: "ŷ".to_string(),
                bbox: [0.68, 0.73, 0.84, 0.78],
            }),
            z: 11,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    assert!(
        !box_object_exists(&plan, "comp_inference_note"),
        "unconnected muted inference note should fold instead of crowding the task-loss/output corridor"
    );
    assert!(
        text_object_exists(&plan, "ann_inference"),
        "folded inference note must remain editable text rather than disappearing"
    );
    let annotation = text_box(&plan, "ann_inference");
    let student = object_box(&plan, "comp_student_encoder");
    assert!(
        annotation[1] >= student[3] + 0.025,
        "folded inference annotation should move below the student branch instead of sitting in the main flow corridor: annotation={annotation:?}, student={student:?}"
    );
    assert!(
        box_object_exists(&plan, "comp_task_loss"),
        "task loss node must remain visible after folding the note"
    );
}

#[test]
fn model_draw_plan_polish_folds_unconnected_protected_inference_note_from_final_smoke() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher-student inference keeps only the student path.",
            "visual_focus": ["comp_student_enc", "comp_student_pred", "comp_inference_note"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 8},
            "regions": [
                {"id": "student_input", "bbox": [0.58, 0.85, 0.795, 0.95]},
                {"id": "student_enc", "bbox": [0.5675, 0.65125, 0.8075, 0.78625]},
                {"id": "student_pred", "bbox": [0.5975, 0.34, 0.7775, 0.46]},
                {"id": "inference_note", "bbox": [0.3575, 0.84, 0.5275, 0.94]}
            ]
        },
        "components": [
            {"id": "comp_student_input", "label": "Task Input x", "role": "input", "visual_weight": "normal", "region": "student_input", "allowed_asset_id": null},
            {"id": "comp_student_enc", "label": "Student Encoder", "role": "module", "visual_weight": "strong", "region": "student_enc", "allowed_asset_id": null},
            {"id": "comp_student_pred", "label": "Student Prediction ŷ", "role": "output", "visual_weight": "strong", "region": "student_pred", "allowed_asset_id": null},
            {"id": "comp_inference_note", "label": "Inference: student only", "role": "context", "visual_weight": "muted", "region": "inference_note", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "edge_input_to_student", "from": "comp_student_input", "to": "comp_student_enc", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "edge_student_to_pred", "from": "comp_student_enc", "to": "comp_student_pred", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"}
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
    .expect("final-smoke inference note figure plan should deserialize");
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_student_input".to_string(),
            bbox: [0.58, 0.85, 0.795, 0.95],
            text: "Task Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "comp_student_enc".to_string(),
            bbox: [0.5675, 0.65125, 0.8075, 0.78625],
            text: "Student Encoder".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "comp_student_pred".to_string(),
            bbox: [0.5975, 0.34, 0.7775, 0.46],
            text: "Student Prediction ŷ".to_string(),
            role: "output".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "comp_inference_note".to_string(),
            bbox: [0.3575, 0.84, 0.5275, 0.94],
            text: "Inference only".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 23,
        },
        DrawObject::Connector {
            id: "edge_input_to_student".to_string(),
            points: vec![[0.6875, 0.85], [0.6875, 0.78625]],
            from: Some("comp_student_input".to_string()),
            to: Some("comp_student_enc".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "edge_student_to_pred".to_string(),
            points: vec![[0.6875, 0.65125], [0.6875, 0.46]],
            from: Some("comp_student_enc".to_string()),
            to: Some("comp_student_pred".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 11,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    assert!(
        !box_object_exists(&plan, "comp_inference_note"),
        "unconnected protected inference note should fold instead of staying as a standalone lane"
    );
    assert!(
        draw_plan_has_text(&plan, "ann_inference", "student only"),
        "folded inference note should remain as editable annotation text"
    );
    let annotation = text_box(&plan, "ann_inference");
    assert!(
        intersection_area(annotation, object_box(&plan, "comp_student_input")) <= 0.0001
            && intersection_area(annotation, object_box(&plan, "comp_student_enc")) <= 0.0001,
        "folded inference annotation should not sit on top of student boxes: annotation={annotation:?}"
    );
}

#[test]
fn model_draw_plan_polish_repairs_same_row_teacher_student_shared_input_collapse_from_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "task_input".to_string(),
            bbox: [0.0, 0.45625, 0.1, 0.60625],
            text: "Task Input".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher_encoder".to_string(),
            bbox: [0.165, 0.44125, 0.395, 0.62125],
            text: "Teacher\n(Large LM)".to_string(),
            role: "module".to_string(),
            style: "muted_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student_encoder".to_string(),
            bbox: [0.637, 0.44125, 0.863, 0.62125],
            text: "Student\n(Compact)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "latent_residual_obj".to_string(),
            bbox: [0.43, 0.47625, 0.57, 0.58625],
            text: "Latent Residual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "student_output".to_string(),
            bbox: [0.75, 0.10625, 0.878, 0.20625],
            text: "Answer".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Text {
            id: "ann_inference".to_string(),
            bbox: [0.655, 0.66425, 0.815, 0.72425],
            text: "Inference: student only".to_string(),
            style: "annotation".to_string(),
            z: 40,
        },
        DrawObject::Connector {
            id: "e_input_to_teacher".to_string(),
            points: vec![[0.1, 0.53125], [0.165, 0.53125]],
            from: Some("task_input".to_string()),
            to: Some("teacher_encoder".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_input_to_student".to_string(),
            points: vec![
                [0.1, 0.53125],
                [0.1, 0.64625],
                [0.637, 0.64625],
                [0.637, 0.53125],
            ],
            from: Some("task_input".to_string()),
            to: Some("student_encoder".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_student_to_residual".to_string(),
            points: vec![[0.637, 0.53125], [0.57, 0.53125]],
            from: Some("student_encoder".to_string()),
            to: Some("latent_residual_obj".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "e_student_to_output".to_string(),
            points: vec![[0.814, 0.44125], [0.814, 0.20625]],
            from: Some("student_encoder".to_string()),
            to: Some("student_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Connector {
            id: "e_teacher_to_residual".to_string(),
            points: vec![[0.395, 0.53125], [0.43, 0.53125]],
            from: Some("teacher_encoder".to_string()),
            to: Some("latent_residual_obj".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 14,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let teacher = object_box(&plan, "teacher_encoder");
    let student = object_box(&plan, "student_encoder");
    let residual = object_box(&plan, "latent_residual_obj");
    assert!(
        teacher[3] + 0.05 <= student[1],
        "same-row teacher/student collapse should be split into readable branches: teacher={teacher:?}, student={student:?}"
    );
    assert!(
        horizontal_separation(teacher, residual) >= 0.045,
        "teacher and residual objective need a visible horizontal gutter: teacher={teacher:?}, residual={residual:?}"
    );
    let input_student = connector(&plan, "e_input_to_student");
    assert!(
        input_student.points.len() <= 2
            && !label_intersects_any_segment(teacher, input_student.points)
            && !label_intersects_any_segment(residual, input_student.points),
        "input-to-student route should be direct after moving the teacher branch: {:?}",
        input_student.points
    );
    let teacher_residual = connector(&plan, "e_teacher_to_residual");
    let teacher_residual_len = teacher_residual
        .points
        .windows(2)
        .map(|window| segment_length((window[0], window[1])))
        .fold(0.0_f64, f64::max);
    assert!(
        teacher_residual_len >= 0.045,
        "teacher-to-residual edge should not remain degenerate: {:?}",
        teacher_residual.points
    );
}

#[test]
fn model_draw_plan_polish_folds_unconnected_residual_box_into_supervision_label_from_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_teacher_latent".to_string(),
            bbox: [0.09733333333333331, 0.5835, 0.347, 0.6915],
            text: "Teacher\nLatent z_t".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "comp_student_enc".to_string(),
            bbox: [0.653, 0.278, 0.8886666666666666, 0.472],
            text: "Student\nEncoder".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "comp_residual".to_string(),
            bbox: [0.4175, 0.45, 0.5825, 0.55],
            text: "Latent Residual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 26,
        },
        DrawObject::Connector {
            id: "edge_residual_supervision".to_string(),
            points: vec![
                [0.347, 0.6375],
                [0.347, 0.435],
                [0.653, 0.435],
                [0.653, 0.375],
            ],
            from: Some("comp_teacher_latent".to_string()),
            to: Some("comp_student_enc".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 15,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    assert!(
        !box_object_exists(&plan, "comp_residual"),
        "unconnected residual box should fold into the dashed residual edge label"
    );
    let edge = connector(&plan, "edge_residual_supervision");
    let label = edge
        .label
        .as_ref()
        .expect("residual supervision edge should carry the folded residual label");
    assert_eq!(label.text, "Latent Residual");
    assert!(
        edge.points.len() <= 3,
        "residual supervision edge should not keep a four-point staircase after removing the box: {:?}",
        edge.points
    );
}

#[test]
fn model_draw_plan_polish_folds_right_edge_residual_bridge_without_clamp_panic() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "teacher_box".to_string(),
            bbox: [0.82, 0.25, 0.953, 0.35],
            text: "Teacher".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student_box".to_string(),
            bbox: [0.82, 0.65, 0.953, 0.75],
            text: "Student".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "residual_signal".to_string(),
            bbox: [0.835, 0.45, 0.95, 0.55],
            text: "Residual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 22,
        },
        DrawObject::Connector {
            id: "edge_teacher_residual".to_string(),
            points: vec![[0.8865, 0.35], [0.8925, 0.45]],
            from: Some("teacher_box".to_string()),
            to: Some("residual_signal".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "edge_student_residual".to_string(),
            points: vec![[0.8865, 0.65], [0.8925, 0.55]],
            from: Some("student_box".to_string()),
            to: Some("residual_signal".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 11,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    assert!(
        !box_object_exists(&plan, "residual_signal"),
        "foldable residual signal should become an editable connector label"
    );
    let edge = connector(&plan, "edge_teacher_residual");
    assert!(
        edge.points.iter().all(|point| point[0].is_finite()
            && point[1].is_finite()
            && point[0] >= 0.0
            && point[0] <= 1.0
            && point[1] >= 0.0
            && point[1] <= 1.0),
        "right-edge residual bridge should route inside the canvas: {:?}",
        edge.points
    );
}

#[test]
fn model_draw_plan_polish_balances_multistage_teacher_student_branches_from_latest_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input_data".to_string(),
            bbox: [0.02, 0.44, 0.12, 0.56],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher_enc".to_string(),
            bbox: [0.196, 0.3025, 0.394, 0.4825],
            text: "Teacher\nEncoder".to_string(),
            role: "module".to_string(),
            style: "muted_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student_enc".to_string(),
            bbox: [0.14, 0.528, 0.36, 0.847],
            text: "Student\nEncoder".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "teacher_proj".to_string(),
            bbox: [0.435, 0.233, 0.675, 0.552],
            text: "Teacher\nProjection".to_string(),
            role: "module".to_string(),
            style: "muted_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "student_proj".to_string(),
            bbox: [0.60, 0.5975, 0.84, 0.7775],
            text: "Student\nProjection".to_string(),
            role: "module".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "teacher_out".to_string(),
            bbox: [0.878, 0.3275, 1.0, 0.4575],
            text: "Teacher\nLatent z_T".to_string(),
            role: "output".to_string(),
            style: "muted_module".to_string(),
            z: 25,
        },
        DrawObject::Box {
            id: "student_out".to_string(),
            bbox: [0.88, 0.6175, 0.98, 0.7575],
            text: "Student\nOutput ŷ".to_string(),
            role: "output".to_string(),
            style: "primary_module".to_string(),
            z: 26,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.88, 0.77, 1.0, 0.87],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 27,
        },
        DrawObject::Box {
            id: "latent_residual".to_string(),
            bbox: [0.72, 0.1625, 0.88, 0.2725],
            text: "Latent Residual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 28,
        },
        DrawObject::Box {
            id: "inference_note".to_string(),
            bbox: [0.42, 0.8575, 0.58, 0.9425],
            text: "Inference: student only".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 29,
        },
        DrawObject::Connector {
            id: "e_input_teacher".to_string(),
            points: vec![[0.12, 0.46125], [0.196, 0.46125]],
            from: Some("input_data".to_string()),
            to: Some("teacher_enc".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_input_student".to_string(),
            points: vec![[0.12, 0.5], [0.14, 0.5], [0.14, 0.6875]],
            from: Some("input_data".to_string()),
            to: Some("student_enc".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_teacher_enc_proj".to_string(),
            points: vec![[0.394, 0.3925], [0.435, 0.3925]],
            from: Some("teacher_enc".to_string()),
            to: Some("teacher_proj".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "e_student_enc_proj".to_string(),
            points: vec![[0.36, 0.6875], [0.60, 0.6875]],
            from: Some("student_enc".to_string()),
            to: Some("student_proj".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Connector {
            id: "e_teacher_proj_out".to_string(),
            points: vec![[0.675, 0.3925], [0.878, 0.3925]],
            from: Some("teacher_proj".to_string()),
            to: Some("teacher_out".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 14,
        },
        DrawObject::Connector {
            id: "e_student_proj_out".to_string(),
            points: vec![[0.84, 0.6875], [0.88, 0.6875]],
            from: Some("student_proj".to_string()),
            to: Some("student_out".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 15,
        },
        DrawObject::Connector {
            id: "e_student_task_loss".to_string(),
            points: vec![[0.93, 0.7575], [0.94, 0.78]],
            from: Some("student_out".to_string()),
            to: Some("task_loss".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 16,
        },
        DrawObject::Connector {
            id: "e_teacher_to_residual".to_string(),
            points: vec![[0.939, 0.4575], [0.939, 0.2725], [0.8, 0.2725]],
            from: Some("teacher_out".to_string()),
            to: Some("latent_residual".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 17,
        },
        DrawObject::Connector {
            id: "e_residual_to_student".to_string(),
            points: vec![[0.72, 0.2725], [0.72, 0.5975]],
            from: Some("latent_residual".to_string()),
            to: Some("student_proj".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 18,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let teacher_proj = object_box(&plan, "teacher_proj");
    let student_proj = object_box(&plan, "student_proj");
    assert!(
        teacher_proj[3] - teacher_proj[1] <= student_proj[3] - student_proj[1] + 0.04,
        "teacher/student projection stages should keep comparable visual weight: teacher={teacher_proj:?}, student={student_proj:?}"
    );
    assert!(
        vertical_separation(teacher_proj, student_proj) >= 0.055,
        "paired projection stages need a visible teacher/student gutter: teacher={teacher_proj:?}, student={student_proj:?}"
    );

    let teacher_out = object_box(&plan, "teacher_out");
    let student_out = object_box(&plan, "student_out");
    let residual = object_box(&plan, "latent_residual");
    assert!(
        center_y(residual) > center_y(teacher_out) && center_y(residual) < center_y(student_out),
        "latent residual should sit between branch endpoints, not above the teacher path: residual={residual:?}, teacher_out={teacher_out:?}, student_out={student_out:?}"
    );

    let task_loss = object_box(&plan, "task_loss");
    assert!(
        vertical_separation(student_out, task_loss) >= 0.055
            || horizontal_separation(student_out, task_loss) >= 0.035,
        "task loss should not touch the student output: output={student_out:?}, task_loss={task_loss:?}"
    );
    let task_edge = connector(&plan, "e_student_task_loss");
    let task_edge_len = task_edge
        .points
        .windows(2)
        .map(|window| segment_length((window[0], window[1])))
        .fold(0.0_f64, f64::max);
    assert!(
        task_edge_len >= 0.045,
        "student-output-to-task-loss connector should not be degenerate: {:?}",
        task_edge.points
    );
    assert!(
        !connectors_cross(
            connector(&plan, "e_teacher_proj_out").points,
            connector(&plan, "e_residual_to_student").points
        ),
        "residual route should not cross the teacher projection flow"
    );
}

#[test]
fn model_draw_plan_polish_repairs_projectionless_multistage_stack_from_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input_text".to_string(),
            bbox: [0.08875, 0.424375, 0.22375, 0.559375],
            text: "Task\nInput".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher_encoder".to_string(),
            bbox: [0.2871, 0.1621, 0.68165, 0.3529],
            text: "Teacher\nEncoder".to_string(),
            role: "module".to_string(),
            style: "muted_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "teacher_latent".to_string(),
            bbox: [0.2871, 0.3971, 0.68165, 0.5879],
            text: "Teacher\nLatent z_T".to_string(),
            role: "output".to_string(),
            style: "muted_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "student_encoder".to_string(),
            bbox: [0.2871, 0.6621, 0.462275, 0.7904],
            text: "Student\nEncoder".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "student_latent".to_string(),
            bbox: [0.506475, 0.6621, 0.68165, 0.7904],
            text: "Student\nLatent z_S".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "student_head".to_string(),
            bbox: [0.2871, 0.8346, 0.462275, 0.9629],
            text: "Task\nHead".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 25,
        },
        DrawObject::Box {
            id: "residual_node".to_string(),
            bbox: [0.71665, 0.490425, 0.88065, 0.620425],
            text: "Latent\nResidual\n||z_T - z_S||^2".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 26,
        },
        DrawObject::Box {
            id: "task_loss_node".to_string(),
            bbox: [0.0721, 0.84125, 0.2321, 0.97125],
            text: "Task Loss\nL_task".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 27,
        },
        DrawObject::Box {
            id: "output_pred".to_string(),
            bbox: [0.507275, 0.84875, 0.607275, 0.94875],
            text: "y_hat".to_string(),
            role: "output".to_string(),
            style: "primary_module".to_string(),
            z: 28,
        },
        DrawObject::Connector {
            id: "e_input_teacher".to_string(),
            points: vec![[0.22375, 0.491875], [0.22375, 0.2575], [0.2871, 0.2575]],
            from: Some("input_text".to_string()),
            to: Some("teacher_encoder".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_input_student".to_string(),
            points: vec![[0.22375, 0.491875], [0.22375, 0.72625], [0.2871, 0.72625]],
            from: Some("input_text".to_string()),
            to: Some("student_encoder".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_teacher_enc".to_string(),
            points: vec![[0.484375, 0.3529], [0.484375, 0.3971]],
            from: Some("teacher_encoder".to_string()),
            to: Some("teacher_latent".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "e_student_enc".to_string(),
            points: vec![[0.462275, 0.72625], [0.506475, 0.72625]],
            from: Some("student_encoder".to_string()),
            to: Some("student_latent".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Connector {
            id: "e_student_head".to_string(),
            points: vec![
                [0.506475, 0.72625],
                [0.506475, 0.8125],
                [0.462275, 0.8125],
                [0.462275, 0.89875],
            ],
            from: Some("student_latent".to_string()),
            to: Some("student_head".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 14,
        },
        DrawObject::Connector {
            id: "e_teacher_residual".to_string(),
            points: vec![
                [0.484375, 0.4925],
                [0.484375, 0.555425],
                [0.71665, 0.555425],
            ],
            from: Some("teacher_latent".to_string()),
            to: Some("residual_node".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 15,
        },
        DrawObject::Connector {
            id: "e_student_residual".to_string(),
            points: vec![
                [0.5940625, 0.6621],
                [0.5940625, 0.555425],
                [0.71665, 0.555425],
            ],
            from: Some("student_latent".to_string()),
            to: Some("residual_node".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 16,
        },
        DrawObject::Connector {
            id: "e_student_output".to_string(),
            points: vec![[0.462275, 0.89875], [0.507275, 0.89875]],
            from: Some("student_head".to_string()),
            to: Some("output_pred".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 17,
        },
        DrawObject::Connector {
            id: "e_task_loss".to_string(),
            points: vec![[0.2871, 0.90625], [0.2321, 0.90625]],
            from: Some("student_head".to_string()),
            to: Some("task_loss_node".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 18,
        },
        DrawObject::Connector {
            id: "e_residual_feedback".to_string(),
            points: vec![[0.71665, 0.620425], [0.71665, 0.72625], [0.68165, 0.72625]],
            from: Some("residual_node".to_string()),
            to: Some("student_latent".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 19,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let teacher_encoder = object_box(&plan, "teacher_encoder");
    let teacher_latent = object_box(&plan, "teacher_latent");
    assert!(
        vertical_separation(teacher_encoder, teacher_latent) >= 0.055,
        "teacher stages need visible vertical gutter: encoder={teacher_encoder:?}, latent={teacher_latent:?}"
    );

    let student_encoder = object_box(&plan, "student_encoder");
    let student_head = object_box(&plan, "student_head");
    assert!(
        vertical_separation(student_encoder, student_head) >= 0.055,
        "student encoder and task head need visible vertical gutter: encoder={student_encoder:?}, head={student_head:?}"
    );

    let residual = object_box(&plan, "residual_node");
    assert!(
        horizontal_separation(teacher_latent, residual) >= 0.045
            || vertical_separation(teacher_latent, residual) >= 0.055,
        "latent residual hub should have a real gutter from teacher latent: latent={teacher_latent:?}, residual={residual:?}"
    );
    assert!(
        !label_near_any_segment(teacher_latent, connector(&plan, "e_student_residual").points, 0.004),
        "student residual connector should route outside teacher latent: teacher={teacher_latent:?}, points={:?}",
        connector(&plan, "e_student_residual").points
    );

    let task_loss = object_box(&plan, "task_loss_node");
    assert!(
        task_loss[0] >= student_head[2] + 0.035,
        "task loss should move to the right of the student head instead of using a reverse leftward edge: head={student_head:?}, task_loss={task_loss:?}"
    );
    let task_edge = connector(&plan, "e_task_loss");
    assert!(
        task_edge
            .points
            .last()
            .is_some_and(|point| point[0] > task_edge.points[0][0]),
        "task-loss edge should progress rightward after repair: {:?}",
        task_edge.points
    );
    assert!(
        !label_near_any_segment(object_box(&plan, "output_pred"), task_edge.points, 0.004),
        "task-loss route should go around the prediction output instead of crossing it: output={:?}, points={:?}",
        object_box(&plan, "output_pred"),
        task_edge.points
    );
}

#[test]
fn model_draw_plan_polish_splits_embedded_inference_note_from_output_box_from_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "task_input".to_string(),
            bbox: [0.1667, 0.471875, 0.2847, 0.621875],
            text: "Task Input".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher_model".to_string(),
            bbox: [0.47, 0.13, 0.68, 0.33],
            text: "Teacher\n(frozen)".to_string(),
            role: "module".to_string(),
            style: "muted_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student_model".to_string(),
            bbox: [0.35, 0.65, 0.6, 0.85],
            text: "Student\n(trainable)".to_string(),
            role: "main".to_string(),
            style: "primary_module_strong".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "latent_residual".to_string(),
            bbox: [0.745, 0.1, 0.945, 0.22],
            text: "Latent Residual\nSupervision".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "final_output".to_string(),
            bbox: [0.7625, 0.735, 0.9425, 0.865],
            text: "Final\nPrediction\n(inference: student only)".to_string(),
            role: "output".to_string(),
            style: "primary_module".to_string(),
            z: 24,
        },
        DrawObject::Connector {
            id: "e_student_output".to_string(),
            points: vec![[0.6, 0.8], [0.7625, 0.8]],
            from: Some("student_model".to_string()),
            to: Some("final_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let output_text = plan
        .objects
        .iter()
        .find_map(|object| match object {
            DrawObject::Box { id, text, .. } if id == "final_output" => Some(text.clone()),
            _ => None,
        })
        .expect("final output box should exist");
    assert!(
        !output_text.to_lowercase().contains("inference")
            && output_text.contains("Final")
            && output_text.contains("Prediction"),
        "output box should keep prediction text but not merge inference note: {output_text:?}"
    );
    assert!(
        draw_plan_has_text(&plan, "ann_inference", "student only"),
        "embedded inference note should be preserved as a separate editable annotation"
    );
    let output = object_box(&plan, "final_output");
    let inference = text_box(&plan, "ann_inference");
    assert!(
        intersection_area(output, inference) == 0.0,
        "split inference annotation should not overlap the output box: output={output:?}, inference={inference:?}"
    );
}

#[test]
fn model_draw_plan_polish_compacts_protected_inference_note_between_branches_from_smoke() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher-student distillation compares teacher and student latent states while student-only inference is an auxiliary cue.",
            "visual_focus": ["teacher_enc", "student_enc", "latent_residual", "inference_note"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 24, "rows": 12},
            "regions": [
                {"id": "input_data_region", "bbox": [0.041666666666666664, 0.4166666666666667, 0.16666666666666666, 1.0]},
                {"id": "teacher_enc_region", "bbox": [0.20833333333333334, 0.08333333333333333, 0.5833333333333334, 0.5]},
                {"id": "student_enc_region", "bbox": [0.20833333333333334, 0.5833333333333334, 0.5833333333333334, 1.0]},
                {"id": "teacher_proj_region", "bbox": [0.4583333333333333, 0.08333333333333333, 1.0, 0.5]},
                {"id": "student_proj_region", "bbox": [0.4583333333333333, 0.5833333333333334, 1.0, 1.0]},
                {"id": "latent_residual_region", "bbox": [0.6666666666666666, 0.3333333333333333, 1.0, 1.0]},
                {"id": "task_loss_region", "bbox": [0.6666666666666666, 0.6666666666666666, 1.0, 1.0]},
                {"id": "inference_note_region", "bbox": [0.875, 0.5833333333333334, 1.0, 1.0]}
            ]
        },
        "components": [
            {"id": "input_data", "label": "Input x", "role": "input", "visual_weight": "normal", "region": "input_data_region", "allowed_asset_id": null},
            {"id": "teacher_enc", "label": "Teacher\nEncoder", "role": "module", "visual_weight": "strong", "region": "teacher_enc_region", "allowed_asset_id": null},
            {"id": "student_enc", "label": "Student\nEncoder", "role": "module", "visual_weight": "strong", "region": "student_enc_region", "allowed_asset_id": null},
            {"id": "teacher_proj", "label": "Teacher\nLatent z_T", "role": "module", "visual_weight": "normal", "region": "teacher_proj_region", "allowed_asset_id": null},
            {"id": "student_proj", "label": "Student\nLatent z_S", "role": "module", "visual_weight": "normal", "region": "student_proj_region", "allowed_asset_id": null},
            {"id": "latent_residual", "label": "Latent Residual\n||z_T - z_S||^2", "role": "loss", "visual_weight": "strong", "region": "latent_residual_region", "allowed_asset_id": null},
            {"id": "task_loss", "label": "Task Loss\nL_task", "role": "loss", "visual_weight": "normal", "region": "task_loss_region", "allowed_asset_id": null},
            {"id": "inference_note", "label": "Inference:\nstudent only", "role": "context", "visual_weight": "muted", "region": "inference_note_region", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "e_input_teacher", "from": "input_data", "to": "teacher_enc", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_input_student", "from": "input_data", "to": "student_enc", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_teacher_enc_proj", "from": "teacher_enc", "to": "teacher_proj", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_student_enc_proj", "from": "student_enc", "to": "student_proj", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_teacher_residual", "from": "teacher_proj", "to": "latent_residual", "label": "", "semantic": "supervision", "style": "dash", "importance": "main"},
            {"id": "e_student_residual", "from": "student_proj", "to": "latent_residual", "label": "", "semantic": "supervision", "style": "dash", "importance": "main"},
            {"id": "e_student_task", "from": "student_proj", "to": "task_loss", "label": "", "semantic": "loss", "style": "solid", "importance": "normal"}
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
    .expect("latest inference note smoke figure plan should deserialize");
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input_data".to_string(),
            bbox: [
                0.05416666666666667,
                0.47416666666666674,
                0.15416666666666667,
                0.6091666666666667,
            ],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher_enc".to_string(),
            bbox: [
                0.2968333333333334,
                0.20166666666666663,
                0.49483333333333335,
                0.3816666666666666,
            ],
            text: "Teacher\nEncoder".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student_enc".to_string(),
            bbox: [
                0.2968333333333334,
                0.7016666666666668,
                0.49483333333333335,
                0.8816666666666667,
            ],
            text: "Student\nEncoder".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "teacher_proj".to_string(),
            bbox: [
                0.6231666666666666,
                0.20166666666666663,
                0.8351666666666666,
                0.3816666666666666,
            ],
            text: "Teacher\nLatent z_T".to_string(),
            role: "module".to_string(),
            style: "neutral_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "student_proj".to_string(),
            bbox: [
                0.6231666666666666,
                0.7016666666666668,
                0.8351666666666666,
                0.8816666666666667,
            ],
            text: "Student\nLatent z_S".to_string(),
            role: "module".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "latent_residual".to_string(),
            bbox: [
                0.6291666666666667,
                0.4766666666666666,
                0.8291666666666666,
                0.6066666666666667,
            ],
            text: "Latent Residual\n||z_T - z_S||^2".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 25,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [
                0.8753333333333332,
                0.7316666666666667,
                0.9753333333333333,
                0.8516666666666668,
            ],
            text: "Task Loss\nL_task".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 26,
        },
        DrawObject::Box {
            id: "inference_note".to_string(),
            bbox: [
                0.3058333333333334,
                0.5616666666666668,
                0.48583333333333334,
                0.6616666666666668,
            ],
            text: "Inference: student only".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 27,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    let note = object_box(&plan, "inference_note");
    let student = object_box(&plan, "student_enc");
    assert!(
        box_area(note) <= 0.016 && note[3] - note[1] <= 0.095,
        "protected inference note should be compact enough for the quality gate: note={note:?}"
    );
    assert!(
        vertical_separation(note, student) >= 0.055,
        "protected inference note should keep a visible gutter from student encoder: note={note:?}, student={student:?}"
    );
}

#[test]
fn model_draw_plan_polish_moves_upserted_inference_annotation_below_student_corridor_from_smoke() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher-student distillation with latent residual supervision: a compact student learns from task labels and latent residuals from a large teacher, enabling efficient inference-only deployment.",
            "visual_focus": ["student", "teacher", "latent_residual"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 24, "rows": 12},
            "regions": [
                {"id": "input", "bbox": [0.041666666666666664, 0.375, 0.20833333333333334, 1.0]},
                {"id": "student", "bbox": [0.22916666666666666, 0.25, 0.6666666666666666, 1.0]},
                {"id": "teacher", "bbox": [0.22916666666666666, 0.041666666666666664, 0.6666666666666666, 0.2916666666666667]},
                {"id": "latent_residual", "bbox": [0.5, 0.125, 1.0, 0.5]},
                {"id": "task_loss", "bbox": [0.5, 0.4583333333333333, 1.0, 1.0]},
                {"id": "output", "bbox": [0.7291666666666666, 0.375, 1.0, 1.0]},
                {"id": "inference_note", "bbox": [0.7291666666666666, 0.7083333333333334, 1.0, 1.0]}
            ]
        },
        "components": [
            {"id": "comp_input", "label": "Task Input\nx", "role": "input", "visual_weight": "normal", "region": "input", "allowed_asset_id": null},
            {"id": "comp_student", "label": "Student\n(Compact)", "role": "main", "visual_weight": "strong", "region": "student", "allowed_asset_id": null},
            {"id": "comp_teacher", "label": "Teacher\n(Large LM)", "role": "context", "visual_weight": "normal", "region": "teacher", "allowed_asset_id": null},
            {"id": "comp_latent_residual", "label": "Latent Residual\nSupervision", "role": "loss", "visual_weight": "strong", "region": "latent_residual", "allowed_asset_id": null},
            {"id": "comp_task_loss", "label": "Task Loss\nL_task", "role": "loss", "visual_weight": "normal", "region": "task_loss", "allowed_asset_id": null},
            {"id": "comp_output", "label": "Prediction\nŷ", "role": "output", "visual_weight": "normal", "region": "output", "allowed_asset_id": null},
            {"id": "comp_inference_note", "label": "Inference: student only", "role": "context", "visual_weight": "muted", "region": "inference_note", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "edge_input_student", "from": "comp_input", "to": "comp_student", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "edge_input_teacher", "from": "comp_input", "to": "comp_teacher", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "edge_teacher_latent", "from": "comp_teacher", "to": "comp_latent_residual", "label": "", "semantic": "supervision", "style": "solid", "importance": "normal"},
            {"id": "edge_latent_student", "from": "comp_latent_residual", "to": "comp_student", "label": "", "semantic": "supervision", "style": "dash", "importance": "main"},
            {"id": "edge_student_taskloss", "from": "comp_student", "to": "comp_task_loss", "label": "", "semantic": "loss", "style": "solid", "importance": "normal"},
            {"id": "edge_taskloss_student", "from": "comp_task_loss", "to": "comp_student", "label": "", "semantic": "feedback", "style": "solid", "importance": "normal"},
            {"id": "edge_student_output", "from": "comp_student", "to": "comp_output", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"}
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
    .expect("post compact inference smoke figure plan should deserialize");
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_input".to_string(),
            bbox: [0.065, 0.30583333333333335, 0.185, 0.4858333333333333],
            text: "Task Input\nx".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "comp_student".to_string(),
            bbox: [0.33491666666666664, 0.535, 0.5609166666666666, 0.715],
            text: "Student\n(Compact)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "comp_teacher".to_string(),
            bbox: [
                0.33491666666666664,
                0.06966666666666667,
                0.5610833333333333,
                0.26366666666666666,
            ],
            text: "Teacher\n(Large LM)".to_string(),
            role: "context".to_string(),
            style: "neutral_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "comp_latent_residual".to_string(),
            bbox: [0.5670833333333333, 0.2475, 0.7330833333333333, 0.3775],
            text: "Latent Residual\nSupervision".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "comp_task_loss".to_string(),
            bbox: [0.5681666666666667, 0.75, 0.7321666666666666, 0.88],
            text: "Task Loss\nL_task".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "comp_output".to_string(),
            bbox: [0.7571666666666667, 0.535, 0.91, 0.715],
            text: "Prediction\nŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 25,
        },
        DrawObject::Connector {
            id: "edge_input_student".to_string(),
            points: vec![
                [0.185, 0.3958333333333333],
                [0.185, 0.625],
                [0.33491666666666664, 0.625],
            ],
            from: Some("comp_input".to_string()),
            to: Some("comp_student".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "edge_input_teacher".to_string(),
            points: vec![
                [0.185, 0.3958333333333333],
                [0.185, 0.16666666666666666],
                [0.33491666666666664, 0.16666666666666666],
            ],
            from: Some("comp_input".to_string()),
            to: Some("comp_teacher".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "edge_teacher_latent".to_string(),
            points: vec![
                [0.448, 0.16666666666666666],
                [0.448, 0.3125],
                [0.5670833333333333, 0.3125],
            ],
            from: Some("comp_teacher".to_string()),
            to: Some("comp_latent_residual".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "edge_latent_student".to_string(),
            points: vec![
                [0.5670833333333333, 0.3125],
                [0.5609166666666666, 0.3125],
                [0.5609166666666666, 0.535],
            ],
            from: Some("comp_latent_residual".to_string()),
            to: Some("comp_student".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Connector {
            id: "edge_student_taskloss".to_string(),
            points: vec![
                [0.4979166666666666, 0.625],
                [0.4979166666666666, 0.75],
                [0.5330416666666666, 0.75],
                [0.5330416666666666, 0.815],
                [0.5681666666666667, 0.815],
            ],
            from: Some("comp_student".to_string()),
            to: Some("comp_task_loss".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 14,
        },
        DrawObject::Connector {
            id: "edge_taskloss_student".to_string(),
            points: vec![
                [0.5681666666666667, 0.7291666666666666],
                [0.5609166666666666, 0.625],
            ],
            from: Some("comp_task_loss".to_string()),
            to: Some("comp_student".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 15,
        },
        DrawObject::Connector {
            id: "edge_student_output".to_string(),
            points: vec![[0.5609166666666666, 0.625], [0.7571666666666667, 0.625]],
            from: Some("comp_student".to_string()),
            to: Some("comp_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 16,
        },
        DrawObject::Text {
            id: "ann_inference".to_string(),
            bbox: [
                0.35266666666666663,
                0.405,
                0.5431666666666666,
                0.48500000000000004,
            ],
            text: "Inference: student only".to_string(),
            style: "annotation".to_string(),
            z: 26,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    let note = text_box(&plan, "ann_inference");
    let student = object_box(&plan, "comp_student");
    let latent_student = connector(&plan, "edge_latent_student");
    assert!(
        note[1] >= student[3] + 0.015 || note[0] >= student[2] + 0.02,
        "inference annotation should leave the teacher-student corridor and sit at the student/output periphery: note={note:?}, student={student:?}"
    );
    assert!(
        !label_near_any_segment(note, latent_student.points, 0.018),
        "inference annotation should keep clear whitespace from the latent-to-student connector: note={note:?}, edge={:?}",
        latent_student.points
    );
}

#[test]
fn model_draw_plan_polish_moves_unanchored_inference_note_component_to_student_periphery_from_smoke(
) {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher encoder produces latent residual supervision while a student encoder/head handles task prediction.",
            "visual_focus": ["student_enc", "student_head", "residual_obj", "inference_note"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 24, "rows": 12},
            "regions": [
                {"id": "input", "bbox": [0.041666666666666664, 0.375, 0.20833333333333334, 0.625]},
                {"id": "teacher", "bbox": [0.25, 0.125, 0.625, 0.5]},
                {"id": "student", "bbox": [0.25, 0.5, 0.75, 1.0]},
                {"id": "residual", "bbox": [0.5, 0.5, 0.875, 0.875]},
                {"id": "task_loss", "bbox": [0.75, 0.625, 1.0, 1.0]},
                {"id": "inference", "bbox": [0.375, 0.0, 0.625, 0.25]}
            ]
        },
        "components": [
            {"id": "input", "label": "Input x", "role": "input", "visual_weight": "normal", "region": "input", "allowed_asset_id": null},
            {"id": "teacher_enc", "label": "Teacher\nEncoder", "role": "module", "visual_weight": "strong", "region": "teacher", "allowed_asset_id": null},
            {"id": "teacher_latent", "label": "Teacher\nLatent z_T", "role": "output", "visual_weight": "normal", "region": "teacher", "allowed_asset_id": null},
            {"id": "student_enc", "label": "Student\nEncoder", "role": "module", "visual_weight": "normal", "region": "student", "allowed_asset_id": null},
            {"id": "student_head", "label": "Student\nHead", "role": "module", "visual_weight": "normal", "region": "student", "allowed_asset_id": null},
            {"id": "task_loss", "label": "Task Loss", "role": "loss", "visual_weight": "normal", "region": "task_loss", "allowed_asset_id": null},
            {"id": "residual_obj", "label": "Latent Residual\nSupervision", "role": "loss", "visual_weight": "strong", "region": "residual", "allowed_asset_id": null},
            {"id": "inference_note", "label": "Inference:\nstudent only", "role": "context", "visual_weight": "muted", "region": "inference", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "e_input_teacher", "from": "input", "to": "teacher_enc", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_input_student", "from": "input", "to": "student_enc", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_teacher_latent", "from": "teacher_enc", "to": "teacher_latent", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_student_forward", "from": "student_enc", "to": "student_head", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_teacher_residual", "from": "teacher_latent", "to": "residual_obj", "label": "z_T", "semantic": "supervision", "style": "dash", "importance": "main"},
            {"id": "e_student_residual", "from": "student_head", "to": "residual_obj", "label": "z_S", "semantic": "supervision", "style": "dash", "importance": "main"},
            {"id": "e_task_loss", "from": "student_head", "to": "task_loss", "label": "ŷ", "semantic": "loss", "style": "solid", "importance": "normal"}
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
    .expect("unanchored inference note smoke figure plan should deserialize");
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input".to_string(),
            bbox: [0.065, 0.45, 0.185, 0.55],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher_enc".to_string(),
            bbox: [0.305, 0.18, 0.555, 0.38],
            text: "Teacher\nEncoder".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "teacher_latent".to_string(),
            bbox: [
                0.33000000000000007,
                0.48000000000000004,
                0.53,
                0.5800000000000001,
            ],
            text: "Teacher\nLatent z_T".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "student_enc".to_string(),
            bbox: [0.32000000000000006, 0.6199999999999999, 0.54, 0.72],
            text: "Student\nEncoder".to_string(),
            role: "module".to_string(),
            style: "neutral_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "student_head".to_string(),
            bbox: [0.52, 0.76, 0.72, 0.8600000000000001],
            text: "Student\nHead".to_string(),
            role: "module".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.7650000000000001, 0.76, 0.915, 0.8600000000000001],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 25,
        },
        DrawObject::Box {
            id: "residual_obj".to_string(),
            bbox: [0.595, 0.6049999999999999, 0.7950000000000002, 0.705],
            text: "Latent Residual\nSupervision".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 26,
        },
        DrawObject::Box {
            id: "inference_note".to_string(),
            bbox: [0.42000000000000004, 0.06, 0.5800000000000001, 0.14],
            text: "Inference:\nstudent only".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 27,
        },
        DrawObject::Connector {
            id: "e_input_teacher".to_string(),
            points: vec![[0.185, 0.5], [0.185, 0.28], [0.305, 0.28]],
            from: Some("input".to_string()),
            to: Some("teacher_enc".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_input_student".to_string(),
            points: vec![
                [0.185, 0.5],
                [0.185, 0.6699999999999999],
                [0.32000000000000006, 0.6699999999999999],
            ],
            from: Some("input".to_string()),
            to: Some("student_enc".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_teacher_latent".to_string(),
            points: vec![
                [0.43000000000000005, 0.38],
                [0.43000000000000005, 0.48000000000000004],
            ],
            from: Some("teacher_enc".to_string()),
            to: Some("teacher_latent".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "e_student_forward".to_string(),
            points: vec![[0.53, 0.72], [0.53, 0.76]],
            from: Some("student_enc".to_string()),
            to: Some("student_head".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Connector {
            id: "e_teacher_residual".to_string(),
            points: vec![[0.53, 0.53], [0.595, 0.53], [0.595, 0.6549999999999999]],
            from: Some("teacher_latent".to_string()),
            to: Some("residual_obj".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "z_T".to_string(),
                bbox: [0.532, 0.45599999999999996, 0.595, 0.506],
            }),
            z: 14,
        },
        DrawObject::Connector {
            id: "e_student_residual".to_string(),
            points: vec![[0.6950000000000001, 0.76], [0.6950000000000001, 0.705]],
            from: Some("student_head".to_string()),
            to: Some("residual_obj".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "z_S".to_string(),
                bbox: [0.7190000000000001, 0.7074999999999999, 0.782, 0.7575],
            }),
            z: 15,
        },
        DrawObject::Connector {
            id: "e_task_loss".to_string(),
            points: vec![[0.72, 0.81], [0.7650000000000001, 0.81]],
            from: Some("student_head".to_string()),
            to: Some("task_loss".to_string()),
            style: "normal_flow".to_string(),
            label: Some(DrawLabel {
                text: "ŷ".to_string(),
                bbox: [
                    0.55,
                    0.8720000000000001,
                    0.6900000000000001,
                    0.9220000000000002,
                ],
            }),
            z: 16,
        },
        DrawObject::Text {
            id: "ann_residual".to_string(),
            bbox: [0.248, 0.77, 0.508, 0.85],
            text: "L_res = ||z_T - z_S||^2".to_string(),
            style: "annotation".to_string(),
            z: 28,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    let note = inference_note_bbox(&plan).expect("inference note should remain editable");
    let student_enc = object_box(&plan, "student_enc");
    let student_head = object_box(&plan, "student_head");
    let anchored_to_student = [student_enc, student_head].iter().any(|student| {
        horizontal_separation(note, *student) <= 0.12 && vertical_separation(note, *student) <= 0.16
    });
    assert!(
        anchored_to_student,
        "inference note should move from teacher-top whitespace to the student branch periphery: note={note:?}, student_enc={student_enc:?}, student_head={student_head:?}"
    );
    assert!(
        text_object_exists(&plan, "inference_note"),
        "peripheral compact inference note should render as an editable annotation, not a collapsed component box"
    );
    let task_edge = connector(&plan, "e_task_loss");
    let task_label = task_edge
        .label
        .as_ref()
        .expect("task loss connector should keep its label");
    let edge_left = task_edge
        .points
        .iter()
        .map(|point| point[0])
        .fold(f64::INFINITY, f64::min);
    let edge_right = task_edge
        .points
        .iter()
        .map(|point| point[0])
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(
        center_x(task_label.bbox) >= edge_left - 0.04
            && center_x(task_label.bbox) <= edge_right + 0.04,
        "short task-loss label should be snapped next to its connector, not left floating under student head: label={:?}, edge={:?}",
        task_label.bbox,
        task_edge.points
    );
}

#[test]
fn model_draw_plan_polish_folds_connected_residual_signal_box_between_vertical_branches_from_smoke()
{
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "teacher_model".to_string(),
            bbox: [0.36975, 0.12, 0.56775, 0.255],
            text: "Teacher".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student_model".to_string(),
            bbox: [0.36975, 0.745, 0.56775, 0.88],
            text: "Student".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "latent_residual_signal".to_string(),
            bbox: [0.5585, 0.5075, 0.6915, 0.6175],
            text: "Latent Residual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Connector {
            id: "e_teacher_to_residual".to_string(),
            points: vec![[0.46875, 0.255], [0.46875, 0.5075], [0.625, 0.5075]],
            from: Some("teacher_model".to_string()),
            to: Some("latent_residual_signal".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "z_t".to_string(),
                bbox: [0.49275, 0.35625, 0.55575, 0.40625],
            }),
            z: 12,
        },
        DrawObject::Connector {
            id: "e_student_to_residual".to_string(),
            points: vec![[0.46875, 0.745], [0.46875, 0.6175], [0.625, 0.6175]],
            from: Some("student_model".to_string()),
            to: Some("latent_residual_signal".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "z_s".to_string(),
                bbox: [0.38175, 0.6175, 0.44475, 0.6675],
            }),
            z: 13,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    assert!(
        !box_object_exists(&plan, "latent_residual_signal"),
        "branch-gap residual signal should be folded into a connector label instead of occupying a standalone box"
    );
    assert!(
        !plan.objects.iter().any(|object| matches!(
            object,
            DrawObject::Connector { from, to, .. }
                if from.as_deref() == Some("latent_residual_signal")
                    || to.as_deref() == Some("latent_residual_signal")
        )),
        "no connector should keep an endpoint to the removed residual signal box"
    );
    assert!(plan.objects.iter().any(|object| matches!(
        object,
        DrawObject::Connector { id, from, to, .. }
            if id == "e_teacher_to_residual"
                && from.as_deref() == Some("teacher_model")
                && to.as_deref() == Some("student_model")
    )));
    let residual_edge = connector(&plan, "e_teacher_to_residual");
    let label = residual_edge
        .label
        .as_ref()
        .expect("folded residual signal should become an editable connector label");
    assert!(
        label.text.to_lowercase().contains("latent residual"),
        "folded label should preserve the residual signal text: {:?}",
        label.text
    );
    assert!(
        !label_intersects_any_segment(label.bbox, residual_edge.points),
        "folded residual label should avoid the connector stroke: label={:?}, points={:?}",
        label.bbox,
        residual_edge.points
    );
}

#[test]
fn model_draw_plan_polish_removes_redundant_inference_parenthetical_from_main_label() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "student_model".to_string(),
            bbox: [0.275, 0.54, 0.505, 0.68],
            text: "Student\n(frozen at inference)\n(inference-only)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "teacher_model".to_string(),
            bbox: [0.54, 0.54, 0.68, 0.68],
            text: "Teacher\n(training-only)".to_string(),
            role: "context".to_string(),
            style: "neutral_module".to_string(),
            z: 23,
        },
        DrawObject::Connector {
            id: "e_teacher_student".to_string(),
            points: vec![[0.54, 0.61], [0.505, 0.61]],
            from: Some("teacher_model".to_string()),
            to: Some("student_model".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 11,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let text = box_text(&plan, "student_model").to_lowercase();
    assert!(
        text.contains("frozen at inference"),
        "the richer inference cue should be preserved: {text}"
    );
    assert!(
        !text.contains("inference-only") && !text.contains("inference only)"),
        "redundant inference-only parenthetical should be removed from the main module label: {text}"
    );
}

#[test]
fn model_draw_plan_polish_removes_inference_only_parenthetical_after_compact_phrase() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "teacher_model".to_string(),
            bbox: [0.54, 0.54, 0.68, 0.68],
            text: "Teacher\n(training-only)".to_string(),
            role: "context".to_string(),
            style: "neutral_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "student_model".to_string(),
            bbox: [0.275, 0.54, 0.505, 0.68],
            text: "Student\n(compact)\n(inference-only)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Connector {
            id: "e_teacher_student".to_string(),
            points: vec![[0.54, 0.61], [0.505, 0.61]],
            from: Some("teacher_model".to_string()),
            to: Some("student_model".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 11,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let text = box_text(&plan, "student_model").to_lowercase();
    assert!(
        !text.contains("inference-only") && !text.contains("inference only)"),
        "inference-only parenthetical should be removed even without extra richer context: {text}"
    );
}

#[test]
fn model_draw_plan_polish_removes_generic_path_signal_annotations() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "teacher_model".to_string(),
            bbox: [0.18, 0.33, 0.42, 0.52],
            text: "Teacher".to_string(),
            role: "context".to_string(),
            style: "neutral_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "student_model".to_string(),
            bbox: [0.18, 0.64, 0.42, 0.82],
            text: "Student".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Text {
            id: "ann_supervision".to_string(),
            bbox: [0.13, 0.07, 0.34, 0.14],
            text: "Supervision".to_string(),
            style: "annotation".to_string(),
            z: 30,
        },
        DrawObject::Text {
            id: "ann_supervision_signal".to_string(),
            bbox: [0.13, 0.16, 0.34, 0.23],
            text: "residual supervision".to_string(),
            style: "annotation".to_string(),
            z: 31,
        },
        DrawObject::Text {
            id: "anno_inference_only".to_string(),
            bbox: [0.13, 0.24, 0.34, 0.31],
            text: "inference only".to_string(),
            style: "annotation".to_string(),
            z: 32,
        },
        DrawObject::Connector {
            id: "e_student_output".to_string(),
            points: vec![[0.42, 0.73], [0.62, 0.73], [0.62, 0.72]],
            from: Some("student_model".to_string()),
            to: Some("student_output".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Box {
            id: "student_output".to_string(),
            bbox: [0.62, 0.67, 0.87, 0.79],
            text: "Prediction".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    assert!(
        !text_object_exists(&plan, "ann_supervision"),
        "generic \"Supervision\" annotation should be removed when redundant with structure"
    );
    assert!(
        !text_object_exists(&plan, "ann_supervision_signal"),
        "residual supervision annotation should be removed when redundant with structure"
    );
}

#[test]
fn model_draw_plan_polish_simplifies_stacked_main_to_output_connector() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "student_model".to_string(),
            bbox: [0.22, 0.45, 0.44, 0.73],
            text: "Student".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "student_output".to_string(),
            bbox: [0.65, 0.45, 0.90, 0.73],
            text: "Prediction".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Connector {
            id: "e_student_output".to_string(),
            points: vec![[0.44, 0.59], [0.44, 0.90], [0.65, 0.90], [0.65, 0.59]],
            from: Some("student_model".to_string()),
            to: Some("student_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 11,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let edge = connector(&plan, "e_student_output");
    assert!(
        edge.points.len() <= 3,
        "main→output route should be compacted when a 4-point detour is unnecessary: {:?}",
        edge.points
    );
}

#[test]
fn model_draw_plan_polish_tightens_short_vertical_edge_label_near_route_from_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "teacher_model".to_string(),
            bbox: [0.54, 0.54, 0.68, 0.68],
            text: "Teacher\n(training-only)".to_string(),
            role: "context".to_string(),
            style: "neutral_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "latent_residual_supervision".to_string(),
            bbox: [0.485, 0.1, 0.665, 0.22],
            text: "Latent Residual\nSupervision".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 24,
        },
        DrawObject::Connector {
            id: "e_teacher_residual".to_string(),
            points: vec![[0.575, 0.54], [0.575, 0.22]],
            from: Some("teacher_model".to_string()),
            to: Some("latent_residual_supervision".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "latent z".to_string(),
                bbox: [0.593, 0.355, 0.753, 0.405],
            }),
            z: 12,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let edge = connector(&plan, "e_teacher_residual");
    let label = edge
        .label
        .as_ref()
        .expect("teacher residual edge should retain its label");
    let min_vertical_delta = edge
        .points
        .windows(2)
        .filter(|window| (window[0][0] - window[1][0]).abs() < 0.001)
        .map(|window| (center_x(label.bbox) - window[0][0]).abs())
        .fold(f64::INFINITY, f64::min);
    assert!(
        label.bbox[2] - label.bbox[0] <= 0.12,
        "short connector labels should not keep a wide 0.16 bbox that visually floats from the edge: {:?}",
        label.bbox
    );
    assert!(
        min_vertical_delta <= 0.08,
        "short vertical edge label should be visually attached to its actual route: label={:?}, points={:?}",
        label.bbox,
        edge.points
    );
}

#[test]
fn model_draw_plan_polish_moves_connected_residual_hub_out_of_tight_branch_gutter_from_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "teacher_encoder".to_string(),
            bbox: [0.2968333333333334, 0.25, 0.49483333333333335, 0.43],
            text: "Teacher\nEncoder".to_string(),
            role: "context".to_string(),
            style: "neutral_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "student_encoder".to_string(),
            bbox: [0.2968333333333334, 0.55, 0.49483333333333335, 0.73],
            text: "Student\nEncoder".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "latent_residual".to_string(),
            bbox: [0.35, 0.45, 0.55, 0.55],
            text: "Latent Residual\nSupervision".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 24,
        },
        DrawObject::Connector {
            id: "e_teacher_residual".to_string(),
            points: vec![
                [0.49483333333333335, 0.34],
                [0.49483333333333335, 0.5],
                [0.35, 0.5],
            ],
            from: Some("teacher_encoder".to_string()),
            to: Some("latent_residual".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "e_residual_student".to_string(),
            points: vec![[0.45, 0.55], [0.39583333333333337, 0.55]],
            from: Some("latent_residual".to_string()),
            to: Some("student_encoder".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 13,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let residual = object_box(&plan, "latent_residual");
    let teacher = object_box(&plan, "teacher_encoder");
    let student = object_box(&plan, "student_encoder");
    assert!(
        vertical_separation(residual, teacher) >= 0.055
            || horizontal_separation(residual, teacher) >= 0.03,
        "connected residual hub should not stay squeezed against teacher branch: residual={residual:?}, teacher={teacher:?}"
    );
    assert!(
        vertical_separation(residual, student) >= 0.055
            || horizontal_separation(residual, student) >= 0.03,
        "connected residual hub should not stay squeezed against student branch: residual={residual:?}, student={student:?}"
    );
}

#[test]
fn model_draw_plan_polish_adds_gutter_between_student_and_connected_task_loss_from_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "student_encoder".to_string(),
            bbox: [0.2968333333333334, 0.55, 0.49483333333333335, 0.73],
            text: "Student\nEncoder".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.29700000000000004, 0.77, 0.495, 0.8700000000000001],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 24,
        },
        DrawObject::Connector {
            id: "e_student_task".to_string(),
            points: vec![[0.396, 0.73], [0.396, 0.77]],
            from: Some("student_encoder".to_string()),
            to: Some("task_loss".to_string()),
            style: "loss_flow".to_string(),
            label: None,
            z: 13,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let student = object_box(&plan, "student_encoder");
    let task_loss = object_box(&plan, "task_loss");
    assert!(
        vertical_separation(student, task_loss) >= 0.055
            || horizontal_separation(student, task_loss) >= 0.03,
        "task loss should get enough gutter from its connected student module: student={student:?}, task_loss={task_loss:?}"
    );
}

#[test]
fn model_draw_plan_polish_moves_inference_annotation_out_of_teacher_student_corridor_from_smoke() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher LLM supervises a student model while inference uses the student only.",
            "visual_focus": ["comp_teacher", "comp_student", "anno_inference"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "teacher", "bbox": [0.115, 0.5105, 0.355, 0.6455]},
                {"id": "student", "bbox": [0.755, 0.5105, 0.995, 0.6455]},
                {"id": "input", "bbox": [0.1113, 0.835, 0.2637, 0.935]}
            ]
        },
        "components": [
            {"id": "comp_teacher", "label": "Teacher LLM", "role": "context", "visual_weight": "normal", "region": "teacher", "allowed_asset_id": null},
            {"id": "comp_student", "label": "Student Model", "role": "main", "visual_weight": "strong", "region": "student", "allowed_asset_id": null},
            {"id": "comp_src_input", "label": "Task Input", "role": "input", "visual_weight": "normal", "region": "input", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "edge_input_to_student", "from": "comp_src_input", "to": "comp_student", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "edge_teacher_to_student", "from": "comp_teacher", "to": "comp_student", "label": "residual", "semantic": "supervision", "style": "dash", "importance": "main"}
        ],
        "annotations": [
            {"id": "anno_inference", "label": "Inference: student only", "target_id": "comp_student", "bbox": [0.577, 0.4644375, 0.737, 0.5244375000000001]}
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
    .expect("smoke-derived annotation corridor plan should deserialize");
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_src_input".to_string(),
            bbox: [0.11133333333333334, 0.835, 0.26366666666666666, 0.935],
            text: "Task Input".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "comp_teacher".to_string(),
            bbox: [
                0.11499999999999999,
                0.5105000000000001,
                0.355,
                0.6455000000000001,
            ],
            text: "Teacher LLM".to_string(),
            role: "context".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "comp_student".to_string(),
            bbox: [0.755, 0.5105000000000001, 0.995, 0.6455000000000001],
            text: "Student Model".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Text {
            id: "anno_inference".to_string(),
            bbox: [0.577, 0.4644375, 0.737, 0.5244375000000001],
            text: "Inference: student only".to_string(),
            style: "annotation".to_string(),
            z: 30,
        },
        DrawObject::Connector {
            id: "edge_input_to_student".to_string(),
            points: vec![[0.26366666666666666, 0.885], [0.755, 0.885], [0.755, 0.578]],
            from: Some("comp_src_input".to_string()),
            to: Some("comp_student".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "edge_teacher_to_student".to_string(),
            points: vec![[0.355, 0.578], [0.755, 0.578]],
            from: Some("comp_teacher".to_string()),
            to: Some("comp_student".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 11,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    let annotation = text_box(&plan, "anno_inference");
    let student = object_box(&plan, "comp_student");
    let input_edge = connector(&plan, "edge_input_to_student");
    let teacher_edge = connector(&plan, "edge_teacher_to_student");
    assert!(
        annotation[1] >= student[3] + 0.025 || annotation[0] >= student[0],
        "inference annotation should move to the student periphery instead of sitting in the teacher-student corridor: annotation={annotation:?}, student={student:?}"
    );
    assert!(
        !label_intersects_any_segment(annotation, input_edge.points)
            && !label_intersects_any_segment(annotation, teacher_edge.points),
        "reanchored inference annotation should avoid nearby connector strokes: annotation={annotation:?}"
    );
}

#[test]
fn model_draw_plan_polish_moves_task_loss_out_of_teacher_student_branch_corridor_from_latest_smoke()
{
    let figure_plan = latest_compact_smoke_teacher_student_plan();
    let mut plan = latest_compact_smoke_draw_plan();

    polish_model_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    let teacher = object_box(&plan, "comp_teacher");
    let student = object_box(&plan, "comp_student");
    let task_loss = object_box(&plan, "comp_task_loss");
    let branch_gap = task_loss[1] < student[1] && task_loss[3] > teacher[3];
    assert!(
        !branch_gap || horizontal_separation(task_loss, student) >= 0.03,
        "task loss should not remain in the teacher/student branch corridor above the student: teacher={teacher:?}, student={student:?}, task_loss={task_loss:?}"
    );
    assert!(
        center_y(task_loss) >= student[1] || horizontal_separation(task_loss, student) >= 0.03,
        "student-to-task-loss relation should become a side/peripheral cue, not an upward branch-separation connector: student={student:?}, task_loss={task_loss:?}"
    );
}

#[test]
fn model_draw_plan_polish_pulls_far_student_task_loss_near_output_path_from_latest_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input_data".to_string(),
            bbox: [0.021875, 0.37, 0.134375, 0.505],
            text: "Task Input".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher_enc".to_string(),
            bbox: [0.30725, 0.19666666666666663, 0.50525, 0.37666666666666665],
            text: "Teacher\nEncoder".to_string(),
            role: "context".to_string(),
            style: "neutral_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student_enc".to_string(),
            bbox: [0.30725, 0.49833333333333335, 0.50525, 0.8583333333333333],
            text: "Student\nEncoder".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "latent_residual".to_string(),
            bbox: [
                0.5702499999999999,
                0.35500000000000004,
                0.7032499999999999,
                0.455,
            ],
            text: "Latent Residual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.865, 0.345, 0.9791666666666666, 0.465],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "output_pred".to_string(),
            bbox: [0.7991666666666666, 0.58, 0.9791666666666666, 0.68],
            text: "Prediction ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 25,
        },
        DrawObject::Connector {
            id: "e_input_student".to_string(),
            points: vec![
                [0.134375, 0.4375],
                [0.134375, 0.6783333333333333],
                [0.30725, 0.6783333333333333],
            ],
            from: Some("input_data".to_string()),
            to: Some("student_enc".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_input_teacher".to_string(),
            points: vec![
                [0.134375, 0.4375],
                [0.134375, 0.2866666666666666],
                [0.30725, 0.2866666666666666],
            ],
            from: Some("input_data".to_string()),
            to: Some("teacher_enc".to_string()),
            style: "solid_connector".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_teacher_residual".to_string(),
            points: vec![[0.50525, 0.2866666666666666], [0.5702499999999999, 0.405]],
            from: Some("teacher_enc".to_string()),
            to: Some("latent_residual".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "e_residual_student".to_string(),
            points: vec![[0.5702499999999999, 0.405], [0.50525, 0.6783333333333333]],
            from: Some("latent_residual".to_string()),
            to: Some("student_enc".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Connector {
            id: "e_student_output".to_string(),
            points: vec![[0.50525, 0.6783333333333333], [0.7991666666666666, 0.63]],
            from: Some("student_enc".to_string()),
            to: Some("output_pred".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 14,
        },
        DrawObject::Connector {
            id: "e_student_task".to_string(),
            points: vec![
                [0.40625, 0.49833333333333335],
                [0.40625, 0.465],
                [0.9220833333333333, 0.465],
            ],
            from: Some("student_enc".to_string()),
            to: Some("task_loss".to_string()),
            style: "loss_flow".to_string(),
            label: None,
            z: 15,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let student = object_box(&plan, "student_enc");
    let task_loss = object_box(&plan, "task_loss");
    let residual = object_box(&plan, "latent_residual");
    let output = object_box(&plan, "output_pred");
    let task_edge = connector(&plan, "e_student_task");
    let task_edge_box = points_to_box_for_test(task_edge.points);
    assert!(
        horizontal_separation(student, task_loss) < 0.28,
        "far task loss should be pulled near the student/output path instead of staying on the right margin: student={student:?}, task_loss={task_loss:?}"
    );
    assert!(
        task_edge_box[2] - task_edge_box[0] < 0.36
            && !has_long_horizontal_segment_for_test(task_edge.points, 0.30),
        "student-to-task-loss connector should no longer contain a long right-margin horizontal segment: task_loss={task_loss:?}, points={:?}",
        task_edge.points
    );
    assert!(
        !component_overlap_gate_fails(task_loss, residual)
            && !component_overlap_gate_fails(task_loss, output),
        "pulled task loss must not fix the long connector by colliding with residual/output boxes: task_loss={task_loss:?}, residual={residual:?}, output={output:?}"
    );
}

#[test]
fn model_draw_plan_polish_reanchors_far_inference_note_from_latest_smoke() {
    let figure_plan = latest_compact_smoke_teacher_student_plan();
    let mut plan = latest_compact_smoke_draw_plan();

    polish_model_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    let student = object_box(&plan, "comp_student");
    let output = object_box(&plan, "comp_output");
    if box_object_exists(&plan, "comp_inference_note") {
        let note = object_box(&plan, "comp_inference_note");
        assert!(
            compact_inference_note_is_anchored(note, student, output),
            "compact inference note should be visually anchored near the student or output, not floating far right: note={note:?}, student={student:?}, output={output:?}"
        );
    } else {
        let note_id = if text_object_exists(&plan, "ann_inference") {
            "ann_inference"
        } else {
            "anno_inference"
        };
        let note = text_box(&plan, note_id);
        assert!(
            compact_inference_note_is_anchored(note, student, output),
            "folded inference annotation should stay near the student or output: note={note:?}, student={student:?}, output={output:?}"
        );
    }
}

#[test]
fn model_draw_plan_polish_recompacts_upserted_inference_annotation_from_latest_smoke() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "A teacher model supplies latent residual supervision while a compact student predicts the task output.",
            "visual_focus": ["student_model", "teacher_model", "latent_residual"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "input_region", "bbox": [0.07, 0.35, 0.22, 0.49]},
                {"id": "student_region", "bbox": [0.40, 0.56, 0.64, 0.75]},
                {"id": "teacher_region", "bbox": [0.32, 0.09, 0.72, 0.29]},
                {"id": "output_region", "bbox": [0.81, 0.64, 0.98, 0.74]},
                {"id": "latent_region", "bbox": [0.42, 0.37, 0.62, 0.51]},
                {"id": "task_loss_region", "bbox": [0.19, 0.59, 0.36, 0.72]}
            ]
        },
        "components": [
            {"id": "input_text", "label": "Input x", "role": "input", "visual_weight": "normal", "region": "input_region", "allowed_asset_id": null},
            {"id": "student_model", "label": "Student\n(Compact LM)", "role": "main", "visual_weight": "strong", "region": "student_region", "allowed_asset_id": null},
            {"id": "teacher_model", "label": "Teacher\n(Large LM)", "role": "context", "visual_weight": "muted", "region": "teacher_region", "allowed_asset_id": null},
            {"id": "output_pred", "label": "Prediction ŷ", "role": "output", "visual_weight": "normal", "region": "output_region", "allowed_asset_id": null},
            {"id": "latent_residual", "label": "Latent Residual\nL_res", "role": "loss", "visual_weight": "strong", "region": "latent_region", "allowed_asset_id": null},
            {"id": "task_loss", "label": "Task Loss\nL_task", "role": "loss", "visual_weight": "normal", "region": "task_loss_region", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "e_input_student", "from": "input_text", "to": "student_model", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_input_teacher", "from": "input_text", "to": "teacher_model", "label": "", "semantic": "reference", "style": "solid", "importance": "normal"},
            {"id": "e_student_output", "from": "student_model", "to": "output_pred", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_teacher_residual", "from": "teacher_model", "to": "latent_residual", "label": "", "semantic": "supervision", "style": "dash", "importance": "main"},
            {"id": "e_residual_student", "from": "latent_residual", "to": "student_model", "label": "", "semantic": "supervision", "style": "dash", "importance": "main"},
            {"id": "e_student_taskloss", "from": "student_model", "to": "task_loss", "label": "", "semantic": "loss", "style": "solid", "importance": "normal"}
        ],
        "annotations": [
            {"id": "anno_inference", "label": "Inference: student only", "target_id": null, "bbox": [0.7083333333333334, 0.75, 1.0, 1.0]}
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
    .expect("latest inference annotation smoke fixture should deserialize");
    let style = style_by_name(StyleName::WpsClean);
    let mut plan = draw_plan_from_figure_plan(&figure_plan, &style);

    polish_model_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    let note = text_box(&plan, "anno_inference");
    let student = object_box(&plan, "student_model");
    let output = object_box(&plan, "output_pred");
    assert!(
        box_area(note) < 0.025,
        "upserted inference annotation should be re-compacted instead of keeping the large FigurePlan bbox: note={note:?}"
    );
    assert!(
        compact_inference_note_is_anchored(note, student, output),
        "re-compacted inference annotation should remain anchored near student/output: note={note:?}, student={student:?}, output={output:?}"
    );
}

#[test]
fn model_draw_plan_polish_moves_targeted_inference_annotation_out_of_residual_corridor_from_latest_smoke(
) {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher residuals supervise a compact student predictor.",
            "visual_focus": ["teacher_encoder", "student_predictor", "latent_residual"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "input_slot", "bbox": [0.0625, 0.375, 0.28125, 1.0]},
                {"id": "teacher_branch", "bbox": [0.3125, 0.125, 0.78125, 0.5625]},
                {"id": "student_branch", "bbox": [0.3125, 0.5625, 0.78125, 1.0]},
                {"id": "supervision_top", "bbox": [0.59375, 0.28125, 1.0, 0.8125]},
                {"id": "task_output", "bbox": [0.84375, 0.5625, 1.0, 1.0]},
                {"id": "inference_note", "bbox": [0.3125, 0.9375, 1.0, 1.0]}
            ]
        },
        "components": [
            {"id": "task_input", "label": "Task\nInput", "role": "input", "visual_weight": "strong", "region": "input_slot", "allowed_asset_id": null},
            {"id": "teacher_encoder", "label": "Teacher\n(Large LM)", "role": "module", "visual_weight": "normal", "region": "teacher_branch", "allowed_asset_id": null},
            {"id": "student_predictor", "label": "Student\n(Compact)", "role": "main", "visual_weight": "strong", "region": "student_branch", "allowed_asset_id": null},
            {"id": "latent_residual", "label": "Latent\nResidual", "role": "loss", "visual_weight": "strong", "region": "supervision_top", "allowed_asset_id": null},
            {"id": "final_answer", "label": "Answer", "role": "output", "visual_weight": "strong", "region": "task_output", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "e_input_to_teacher", "from": "task_input", "to": "teacher_encoder", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_input_to_student", "from": "task_input", "to": "student_predictor", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_teacher_to_residual", "from": "teacher_encoder", "to": "latent_residual", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_residual_to_student", "from": "latent_residual", "to": "student_predictor", "label": "residual", "semantic": "supervision", "style": "dash", "importance": "aux"},
            {"id": "e_student_to_answer", "from": "student_predictor", "to": "final_answer", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"}
        ],
        "annotations": [
            {"id": "anno_inference", "label": "Inference: student only", "target_id": "student_predictor", "bbox": [0.34375, 0.94, 1.0, 1.0]},
            {"id": "anno_teacher_role", "label": "latent residual generator", "target_id": "teacher_encoder", "bbox": [0.325, 0.0375, 0.78125, 0.1375]}
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
    .expect("targeted inference annotation smoke fixture should deserialize");
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "task_input".to_string(),
            bbox: [0.0905, 0.4725, 0.25325, 0.6525],
            text: "Task\nInput".to_string(),
            role: "input".to_string(),
            style: "primary_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher_encoder".to_string(),
            bbox: [0.433875, 0.25375, 0.659875, 0.43375],
            text: "Teacher\n(Large LM)".to_string(),
            role: "module".to_string(),
            style: "neutral_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student_predictor".to_string(),
            bbox: [0.433875, 0.69125, 0.659875, 0.87125],
            text: "Student\n(Compact)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "latent_residual".to_string(),
            bbox: [0.714875, 0.481875, 0.878875, 0.611875],
            text: "Latent\nResidual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "final_answer".to_string(),
            bbox: [0.848125, 0.73125, 0.978125, 0.83125],
            text: "Answer".to_string(),
            role: "output".to_string(),
            style: "primary_module".to_string(),
            z: 24,
        },
        DrawObject::Text {
            id: "anno_inference".to_string(),
            bbox: [
                0.6073750000000001,
                0.5318749999999999,
                0.767375,
                0.5918749999999999,
            ],
            text: "Inference: student only".to_string(),
            style: "annotation".to_string(),
            z: 30,
        },
        DrawObject::Connector {
            id: "e_teacher_to_residual".to_string(),
            points: vec![
                [0.659875, 0.34375],
                [0.659875, 0.546875],
                [0.714875, 0.546875],
            ],
            from: Some("teacher_encoder".to_string()),
            to: Some("latent_residual".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_residual_to_student".to_string(),
            points: vec![
                [0.714875, 0.611875],
                [0.659875, 0.611875],
                [0.659875, 0.69125],
            ],
            from: Some("latent_residual".to_string()),
            to: Some("student_predictor".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "residual".to_string(),
                bbox: [0.628875, 0.635875, 0.745875, 0.685875],
            }),
            z: 11,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    let note = text_box(&plan, "anno_inference");
    let residual = object_box(&plan, "latent_residual");
    let student = object_box(&plan, "student_predictor");
    let teacher_residual = connector(&plan, "e_teacher_to_residual");
    assert_eq!(
        intersection_area(note, residual),
        0.0,
        "targeted inference annotation should move away from residual when target-adjacent space is blocked: note={note:?}, residual={residual:?}"
    );
    assert!(
        !label_intersects_any_segment(note, teacher_residual.points),
        "targeted inference annotation should not cover the teacher-to-residual edge: note={note:?}, edge={:?}",
        teacher_residual.points
    );
    assert!(
        horizontal_separation(note, student) <= 0.25 || vertical_separation(note, student) <= 0.16,
        "moved inference annotation should remain visually tied to the student: note={note:?}, student={student:?}"
    );
}

#[test]
fn model_draw_plan_polish_repairs_bottom_input_and_top_losses_from_latest_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input_text".to_string(),
            bbox: [0.053, 0.8196666666666667, 0.222, 0.972],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher_model".to_string(),
            bbox: [0.1495, 0.13625, 0.3755, 0.31625],
            text: "Teacher\n(Large LM)".to_string(),
            role: "context".to_string(),
            style: "neutral_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "teacher_latent".to_string(),
            bbox: [0.0621, 0.4754166666666667, 0.4629, 0.6554166666666666],
            text: "Teacher latent z_t".to_string(),
            role: "context".to_string(),
            style: "neutral_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "student_model".to_string(),
            bbox: [0.6495, 0.13625, 0.8755, 0.31625],
            text: "Student\n(Compact LM)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "student_output".to_string(),
            bbox: [0.7625, 0.4479166666666667, 0.8985, 0.5479166666666667],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.45675, 0.02, 0.62075, 0.15],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 25,
        },
        DrawObject::Box {
            id: "residual_loss".to_string(),
            bbox: [0.6616, 0.02, 0.8634, 0.15],
            text: "Residual Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 26,
        },
        DrawObject::Box {
            id: "inference_note".to_string(),
            bbox: [0.73525, 0.5979166666666668, 0.92575, 0.6779166666666667],
            text: "Inference: student only".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 27,
        },
        DrawObject::Connector {
            id: "e_input_teacher".to_string(),
            points: vec![
                [0.1375, 0.8196666666666667],
                [0.0371, 0.8196666666666667],
                [0.0371, 0.31625],
                [0.2625, 0.31625],
            ],
            from: Some("input_text".to_string()),
            to: Some("teacher_model".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_input_student".to_string(),
            points: vec![
                [0.222, 0.8958333333333333],
                [0.222, 0.6804166666666667],
                [0.6495, 0.6804166666666667],
                [0.6495, 0.22625],
            ],
            from: Some("input_text".to_string()),
            to: Some("student_model".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_teacher_latent".to_string(),
            points: vec![[0.2625, 0.31625], [0.2625, 0.4754166666666667]],
            from: Some("teacher_model".to_string()),
            to: Some("teacher_latent".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "e_student_out".to_string(),
            points: vec![[0.8305, 0.31625], [0.8305, 0.4479166666666667]],
            from: Some("student_model".to_string()),
            to: Some("student_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Connector {
            id: "e_latent_residual".to_string(),
            points: vec![
                [0.4629, 0.5654166666666667],
                [0.6145, 0.5654166666666667],
                [0.6145, 0.31625],
                [0.6495, 0.31625],
            ],
            from: Some("teacher_latent".to_string()),
            to: Some("student_model".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "residual".to_string(),
                bbox: [0.4629, 0.5894166666666667, 0.5799, 0.6394166666666667],
            }),
            z: 14,
        },
        DrawObject::Connector {
            id: "e_task_supervision".to_string(),
            points: vec![
                [0.7625, 0.4979166666666667],
                [0.62075, 0.4979166666666667],
                [0.62075, 0.085],
            ],
            from: Some("student_output".to_string()),
            to: Some("task_loss".to_string()),
            style: "normal_flow".to_string(),
            label: Some(DrawLabel {
                text: "ŷ vs y".to_string(),
                bbox: [0.52175, 0.2664583333333333, 0.60275, 0.31645833333333334],
            }),
            z: 15,
        },
        DrawObject::Connector {
            id: "e_residual_supervision".to_string(),
            points: vec![[0.4629, 0.5654166666666667], [0.4629, 0.085], [0.78, 0.085]],
            from: Some("teacher_latent".to_string()),
            to: Some("residual_loss".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "z_t vs z_s".to_string(),
                bbox: [0.3399, 0.085, 0.4389, 0.135],
            }),
            z: 16,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let input = object_box(&plan, "input_text");
    let teacher = object_box(&plan, "teacher_model");
    let student = object_box(&plan, "student_model");
    let task_loss = object_box(&plan, "task_loss");
    let residual_loss = object_box(&plan, "residual_loss");
    assert!(
        center_y(input) < 0.40 && input[2] <= teacher[0] - 0.01,
        "shared input should move from the bottom margin to the left of the teacher/student branches: input={input:?}, teacher={teacher:?}, student={student:?}"
    );
    for edge_id in ["e_input_teacher", "e_input_student"] {
        let edge = connector(&plan, edge_id);
        let edge_box = points_to_box_for_test(edge.points);
        assert!(
            edge_box[3] < 0.46,
            "{edge_id} should not keep the bottom U-shaped detour: {:?}",
            edge.points
        );
        assert!(
            !label_intersects_any_segment(teacher, edge.points),
            "{edge_id} should not route through the teacher box: {:?}",
            edge.points
        );
    }
    assert!(
        task_loss[1] > 0.18,
        "task loss should be pulled out of the top corridor and back near the output: {task_loss:?}"
    );
    assert!(
        residual_loss[1] > 0.18,
        "residual loss should be pulled out of the top corridor and back near its source: {residual_loss:?}"
    );
    let task_edge = connector(&plan, "e_task_supervision");
    let residual_edge = connector(&plan, "e_residual_supervision");
    let latent_edge = connector(&plan, "e_latent_residual");
    assert!(
        !connectors_cross(task_edge.points, residual_edge.points),
        "task and residual supervision edges should not cross after top-edge objective repair: task={:?}, residual={:?}",
        task_edge.points,
        residual_edge.points
    );
    assert!(
        !connectors_cross(task_edge.points, latent_edge.points),
        "task supervision should not cross the latent residual path: task={:?}, latent={:?}",
        task_edge.points,
        latent_edge.points
    );
}

#[test]
fn model_draw_plan_polish_repairs_outer_shared_input_to_student_after_crossing_reroute_from_smoke()
{
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_input_task".to_string(),
            bbox: [0.065, 0.3815, 0.185, 0.5165],
            text: "Task Input".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "comp_teacher_enc".to_string(),
            bbox: [0.2968333333333334, 0.268, 0.49483333333333335, 0.448],
            text: "Teacher\nEncoder".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "comp_teacher_proj".to_string(),
            bbox: [0.2968333333333334, 0.568, 0.49483333333333335, 0.76],
            text: "Latent\nProjection".to_string(),
            role: "module".to_string(),
            style: "neutral_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "comp_latent_residual".to_string(),
            bbox: [0.378, 0.825, 0.522, 0.925],
            text: "Latent Residual\nObjective".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "comp_student_enc".to_string(),
            bbox: [0.66, 0.48, 0.86, 0.6],
            text: "Student\nEncoder".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "comp_student_head".to_string(),
            bbox: [0.66, 0.703, 0.86, 0.823],
            text: "Student\nHead".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 25,
        },
        DrawObject::Box {
            id: "comp_task_loss".to_string(),
            bbox: [0.86, 0.54, 1.0, 0.64],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 26,
        },
        DrawObject::Box {
            id: "comp_output_answer".to_string(),
            bbox: [0.68, 0.31, 0.84, 0.41],
            text: "Answer".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 27,
        },
        DrawObject::Connector {
            id: "edge_input_to_teacher".to_string(),
            points: vec![[0.185, 0.41475], [0.2968333333333334, 0.41475]],
            from: Some("comp_input_task".to_string()),
            to: Some("comp_teacher_enc".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "edge_teacher_to_proj".to_string(),
            points: vec![[0.39583333333333337, 0.448], [0.39583333333333337, 0.568]],
            from: Some("comp_teacher_enc".to_string()),
            to: Some("comp_teacher_proj".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "edge_teacher_to_latent".to_string(),
            points: vec![[0.45, 0.76], [0.45, 0.825]],
            from: Some("comp_teacher_proj".to_string()),
            to: Some("comp_latent_residual".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "edge_latent_to_student".to_string(),
            points: vec![[0.522, 0.825], [0.66, 0.825], [0.66, 0.6]],
            from: Some("comp_latent_residual".to_string()),
            to: Some("comp_student_enc".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Connector {
            id: "edge_input_to_student".to_string(),
            points: vec![
                [0.185, 0.49825],
                [0.08, 0.49825],
                [0.08, 0.03],
                [0.86, 0.03],
                [0.86, 0.48],
            ],
            from: Some("comp_input_task".to_string()),
            to: Some("comp_student_enc".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 14,
        },
        DrawObject::Connector {
            id: "edge_student_enc_to_head".to_string(),
            points: vec![[0.76, 0.6], [0.76, 0.703]],
            from: Some("comp_student_enc".to_string()),
            to: Some("comp_student_head".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 15,
        },
        DrawObject::Connector {
            id: "edge_student_to_task_loss".to_string(),
            points: vec![[0.86, 0.58], [0.88, 0.58]],
            from: Some("comp_student_head".to_string()),
            to: Some("comp_task_loss".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 16,
        },
        DrawObject::Connector {
            id: "edge_student_to_output".to_string(),
            points: vec![[0.76, 0.703], [0.635, 0.703], [0.635, 0.41], [0.76, 0.41]],
            from: Some("comp_student_head".to_string()),
            to: Some("comp_output_answer".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 17,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let edge = connector(&plan, "edge_input_to_student");
    let route_box = points_to_box_for_test(edge.points);
    assert!(
        route_box[1] > 0.24 && route_box[3] < 0.68 && edge.points.len() <= 4,
        "input-to-student should be repaired back into the local teacher/student corridor, not the outer canvas: {:?}",
        edge.points
    );
    for id in [
        "comp_teacher_enc",
        "comp_teacher_proj",
        "comp_latent_residual",
    ] {
        assert!(
            !label_intersects_any_segment(object_box(&plan, id), edge.points),
            "repaired input-to-student edge should not pass through {id}: {:?}",
            edge.points
        );
    }
}

#[test]
fn model_draw_plan_polish_stacks_student_encoder_above_head_from_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_student_enc".to_string(),
            bbox: [0.66, 0.7026666666666667, 0.86, 0.8226666666666667],
            text: "Student\nEncoder".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "comp_student_head".to_string(),
            bbox: [0.66, 0.48, 0.86, 0.6],
            text: "Student\nHead".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Connector {
            id: "edge_student_enc_to_head".to_string(),
            points: vec![[0.76, 0.7026666666666667], [0.76, 0.6]],
            from: Some("comp_student_enc".to_string()),
            to: Some("comp_student_head".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let encoder = object_box(&plan, "comp_student_enc");
    let head = object_box(&plan, "comp_student_head");
    let edge = connector(&plan, "edge_student_enc_to_head");
    assert!(
        center_y(encoder) < center_y(head) && vertical_separation(encoder, head) >= 0.05,
        "student encoder should feed downward into student head: encoder={encoder:?}, head={head:?}"
    );
    assert!(
        edge.points.len() <= 2,
        "student encoder-to-head connector should be direct after stacking: {:?}",
        edge.points
    );
}

#[test]
fn model_draw_plan_polish_moves_output_task_loss_below_output_from_corridor_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "teacher_encoder".to_string(),
            bbox: [0.38016666666666665, 0.15125, 0.5781666666666667, 0.28625],
            text: "Teacher".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student_output".to_string(),
            bbox: [0.86, 0.73125, 1.0, 0.83125],
            text: "Output ŷ".to_string(),
            role: "output".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.90, 0.3825, 1.0, 0.4925],
            text: "Task loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 22,
        },
        DrawObject::Connector {
            id: "e_student_taskloss".to_string(),
            points: vec![[0.95, 0.73125], [0.95, 0.4925]],
            from: Some("student_output".to_string()),
            to: Some("task_loss".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let output = object_box(&plan, "student_output");
    let task_loss = object_box(&plan, "task_loss");
    let edge = connector(&plan, "e_student_taskloss");
    assert!(
        center_y(task_loss) > output[1] - 0.04,
        "output task loss should leave the upper branch corridor and sit near the output periphery: output={output:?}, task_loss={task_loss:?}"
    );
    assert!(
        edge.points.len() <= 3,
        "output-to-task-loss connector should be direct after moving the loss: {:?}",
        edge.points
    );
}

#[test]
fn model_draw_plan_polish_moves_inference_text_out_of_teacher_student_corridor_from_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "teacher_encoder".to_string(),
            bbox: [0.38016666666666665, 0.15125, 0.5781666666666667, 0.28625],
            text: "Teacher".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student_encoder".to_string(),
            bbox: [0.38016666666666665, 0.71375, 0.5781666666666667, 0.84875],
            text: "Student".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "alignment_objective".to_string(),
            bbox: [0.6848333333333333, 0.62, 0.8568333333333332, 0.75],
            text: "Residual\nalignment".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 22,
        },
        DrawObject::Text {
            id: "anno_inference".to_string(),
            bbox: [0.3839166666666667, 0.58375, 0.5744166666666667, 0.66375],
            text: "Inference: student only".to_string(),
            style: "annotation".to_string(),
            z: 30,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let Some(note) = optional_text_box(&plan, "anno_inference") else {
        return;
    };
    let student = object_box(&plan, "student_encoder");
    let teacher = object_box(&plan, "teacher_encoder");
    assert!(
        note[1] > student[3] + 0.015
            || note[0] > student[2] + 0.02
            || note[3] < teacher[1] - 0.015,
        "inference text should move outside the teacher-student corridor: note={note:?}, teacher={teacher:?}, student={student:?}"
    );
}

#[test]
fn model_draw_plan_polish_increases_student_chain_gutters_and_output_gap_from_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input_text".to_string(),
            bbox: [0.02, 0.17455, 0.12000000000000001, 0.28255],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher_encoder".to_string(),
            bbox: [0.1204, 0.2871, 0.3004, 0.3871],
            text: "Teacher\n(frozen)".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "teacher_latent".to_string(),
            bbox: [0.1303, 0.4906, 0.2413, 0.5906],
            text: "Latent h_T".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "student_encoder".to_string(),
            bbox: [0.53, 0.2871, 0.71, 0.4021],
            text: "Student\n(trainable)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "student_latent".to_string(),
            bbox: [0.556, 0.4471, 0.684, 0.5521],
            text: "Latent h_S".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "student_head".to_string(),
            bbox: [0.556, 0.5971, 0.684, 0.7121],
            text: "Task head".to_string(),
            role: "module".to_string(),
            style: "neutral_module".to_string(),
            z: 25,
        },
        DrawObject::Box {
            id: "student_output".to_string(),
            bbox: [0.719, 0.6046, 0.819, 0.7046],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 26,
        },
        DrawObject::Connector {
            id: "e_student_to_latent".to_string(),
            points: vec![[0.62, 0.4021], [0.62, 0.4471]],
            from: Some("student_encoder".to_string()),
            to: Some("student_latent".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_latent_to_head".to_string(),
            points: vec![[0.62, 0.5521], [0.62, 0.5971]],
            from: Some("student_latent".to_string()),
            to: Some("student_head".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_head_to_output".to_string(),
            points: vec![[0.684, 0.6546], [0.719, 0.6546]],
            from: Some("student_head".to_string()),
            to: Some("student_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 12,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let encoder = object_box(&plan, "student_encoder");
    let latent = object_box(&plan, "student_latent");
    let head = object_box(&plan, "student_head");
    let output = object_box(&plan, "student_output");
    assert!(
        vertical_separation(encoder, latent) >= 0.055
            && vertical_separation(latent, head) >= 0.055,
        "student chain should keep visible gutters after polish: encoder={encoder:?}, latent={latent:?}, head={head:?}"
    );
    assert!(
        horizontal_separation(head, output) >= 0.045,
        "student output should not sit flush against the head: head={head:?}, output={output:?}"
    );
    let edge = connector(&plan, "e_head_to_output");
    let max_segment = edge
        .points
        .windows(2)
        .map(|window| segment_length((window[0], window[1])))
        .fold(0.0_f64, f64::max);
    assert!(
        max_segment >= 0.045,
        "head-to-output connector should not be a degenerate short edge: {:?}",
        edge.points
    );
}

#[test]
fn model_draw_plan_polish_removes_redundant_frozen_annotation_near_task_edge_from_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input_text".to_string(),
            bbox: [0.02, 0.17455, 0.12000000000000001, 0.28255],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher_encoder".to_string(),
            bbox: [0.1204, 0.2871, 0.3004, 0.3871],
            text: "Teacher\n(frozen)".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "task_loss_node".to_string(),
            bbox: [0.676, 0.0585, 0.8423, 0.1665],
            text: "Task loss\nL(ŷ, y)".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 22,
        },
        DrawObject::Connector {
            id: "e_ground_truth_to_task_loss".to_string(),
            points: vec![[0.12, 0.22855], [0.12, 0.1665], [0.676, 0.1665]],
            from: Some("input_text".to_string()),
            to: Some("task_loss_node".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Text {
            id: "anno_frozen".to_string(),
            bbox: [0.318, 0.084, 0.478, 0.1465],
            text: "frozen".to_string(),
            style: "annotation".to_string(),
            z: 30,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    assert!(
        !text_object_exists(&plan, "anno_frozen"),
        "redundant frozen annotation should be removed because the teacher box already carries the frozen semantics"
    );
}

#[test]
fn model_draw_plan_polish_repairs_latest_branch_corridor_smoke_layout() {
    let figure_plan = latest_branch_corridor_smoke_plan();
    let mut plan = latest_branch_corridor_smoke_draw_plan();

    polish_model_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    let input = object_box(&plan, "comp_input");
    let student = object_box(&plan, "comp_student");
    let input_edge = connector(&plan, "edge_input_to_student");
    assert!(
        (center_y(input) - center_y(student)).abs() < 0.04,
        "input should be vertically aligned to the student path so the data-flow edge can be direct: input={input:?}, student={student:?}"
    );
    assert!(
        input_edge.points.len() <= 2,
        "input-to-student edge should be a direct route after input alignment: {:?}",
        input_edge.points
    );
    assert!(
        !text_object_exists(&plan, "anno_residual_dashed")
            && !text_object_exists(&plan, "anno_student_dominant"),
        "generic auxiliary/main-path annotations should be removed instead of crowding the main connector"
    );
    assert!(
        !box_object_exists(&plan, "comp_inference_note")
            || box_area(object_box(&plan, "comp_inference_note")) < 0.014,
        "student-only inference cue should be folded to text or kept as a compact badge"
    );
}

#[test]
fn model_draw_plan_polish_balances_shared_input_between_teacher_and_student_from_latest_smoke() {
    let figure_plan = shared_input_teacher_student_smoke_plan();
    let mut plan = shared_input_teacher_student_smoke_draw_plan();

    polish_model_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    let input = object_box(&plan, "comp_input");
    let teacher = object_box(&plan, "comp_teacher");
    let student = object_box(&plan, "comp_student");
    let desired_y = (center_y(teacher) + center_y(student)) / 2.0;
    assert!(
        (center_y(input) - desired_y).abs() < 0.08,
        "shared input should sit between teacher and student branch targets, not on only one branch: input={input:?}, teacher={teacher:?}, student={student:?}"
    );
    let input_teacher = connector(&plan, "edge_input_teacher");
    let longest_segment = input_teacher
        .points
        .windows(2)
        .map(|window| segment_length((window[0], window[1])))
        .fold(0.0, f64::max);
    assert!(
        longest_segment < 0.35,
        "input-to-teacher route should not keep the long vertical detour from the smoke: {:?}",
        input_teacher.points
    );
    let training_only = text_box(&plan, "anno_training_only");
    let teacher_residual = connector(&plan, "edge_teacher_residual");
    assert!(
        !label_near_any_segment(training_only, teacher_residual.points, 0.026),
        "training-only phase text should be preserved but moved away from the residual connector: annotation={training_only:?}, edge={:?}",
        teacher_residual.points
    );
}

#[test]
fn model_draw_plan_polish_deduplicates_inference_annotations_from_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "teacher_model".to_string(),
            bbox: [0.415, 0.2225, 0.627, 0.4025],
            text: "Teacher\n(frozen)".to_string(),
            role: "module".to_string(),
            style: "neutral_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student_model".to_string(),
            bbox: [0.32, 0.81, 0.722, 0.94],
            text: "Student\n(trainable)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 23,
        },
        DrawObject::Text {
            id: "ann_inference".to_string(),
            bbox: [0.381, 0.71, 0.661, 0.77],
            text: "Inference: student only".to_string(),
            style: "annotation".to_string(),
            z: 29,
        },
        DrawObject::Text {
            id: "anno_inference".to_string(),
            bbox: [
                0.7316666666666666,
                0.43424999999999986,
                0.94,
                0.5142499999999999,
            ],
            text: "No teacher at inference".to_string(),
            style: "annotation".to_string(),
            z: 30,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let inference_texts = plan
        .objects
        .iter()
        .filter(|object| {
            matches!(
                object,
                DrawObject::Text { id, text, .. }
                    if id.to_lowercase().contains("inference")
                        || text.to_lowercase().contains("inference")
                        || text.to_lowercase().contains("student only")
            )
        })
        .count();
    assert_eq!(
        inference_texts, 1,
        "duplicate inference annotations should collapse to one editable note"
    );
    assert!(
        text_object_exists(&plan, "ann_inference"),
        "the direct student-only annotation should be kept over the weaker no-teacher duplicate"
    );
}

#[test]
fn model_draw_plan_polish_removes_top_edge_inference_annotation_from_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "teacher_enc".to_string(),
            bbox: [0.426, 0.285, 0.624, 0.465],
            text: "Teacher\nEncoder".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student_enc".to_string(),
            bbox: [0.726, 0.265, 0.924, 0.445],
            text: "Student\nEncoder".to_string(),
            role: "main".to_string(),
            style: "neutral_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "student_head".to_string(),
            bbox: [0.6419999999999999, 0.5, 0.84, 0.67],
            text: "Student\nHead".to_string(),
            role: "module".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "teacher_latent".to_string(),
            bbox: [0.419, 0.585, 0.631, 0.765],
            text: "Teacher\nLatent z_T".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 25,
        },
        DrawObject::Text {
            id: "ann_inference".to_string(),
            bbox: [0.37, 0.06, 0.63, 0.11999999999999994],
            text: "Inference: student only".to_string(),
            style: "annotation".to_string(),
            z: 28,
        },
        DrawObject::Text {
            id: "anno_teacher_capacity".to_string(),
            bbox: [0.3, 0.049999999999999996, 0.75, 0.19999999999999998],
            text: "Large model (frozen)".to_string(),
            style: "annotation".to_string(),
            z: 29,
        },
        DrawObject::Text {
            id: "anno_student_compact".to_string(),
            bbox: [0.65, 0.049999999999999996, 1.0, 0.19999999999999998],
            text: "Compact model (trainable)".to_string(),
            style: "annotation".to_string(),
            z: 30,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    assert!(
        !text_object_exists(&plan, "ann_inference"),
        "top-edge inference annotation should be removed instead of remaining as a floating phase label"
    );
}

#[test]
fn model_draw_plan_polish_separates_horizontally_crowded_connected_modules_from_smoke() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "teacher_model".to_string(),
            bbox: [0.2246, 0.0996, 0.337275, 0.3379],
            text: "Teacher\n(LM)".to_string(),
            role: "context".to_string(),
            style: "neutral_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "teacher_latent".to_string(),
            bbox: [0.3383124999999999, 0.09960000000000001, 0.5373125, 0.3379],
            text: "Latent\nRepresentation".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "student_model".to_string(),
            bbox: [0.2246, 0.4746, 0.337275, 0.7129],
            text: "Student".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "student_output".to_string(),
            bbox: [0.3603124999999999, 0.4746, 0.5153125, 0.7129],
            text: "Answer\nPrediction".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 25,
        },
        DrawObject::Connector {
            id: "e_teacher_latent".to_string(),
            points: vec![[0.2809375, 0.21875], [0.3383124999999999, 0.21875]],
            from: Some("teacher_model".to_string()),
            to: Some("teacher_latent".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_student_output".to_string(),
            points: vec![[0.2809375, 0.59375], [0.3603124999999999, 0.59375]],
            from: Some("student_model".to_string()),
            to: Some("student_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 11,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let teacher = object_box(&plan, "teacher_model");
    let latent = object_box(&plan, "teacher_latent");
    let student = object_box(&plan, "student_model");
    let output = object_box(&plan, "student_output");
    assert!(
        horizontal_separation(teacher, latent) >= 0.03,
        "teacher and latent modules should have a readable horizontal gutter: teacher={teacher:?}, latent={latent:?}"
    );
    assert!(
        horizontal_separation(student, output) >= 0.03,
        "student and output modules should have a readable horizontal gutter: student={student:?}, output={output:?}"
    );
}

#[test]
fn model_draw_plan_polish_widens_short_main_module_label_from_smoke() {
    let mut plan = minimal_draw_plan(vec![DrawObject::Box {
        id: "student_model".to_string(),
        bbox: [0.2246, 0.4746, 0.337275, 0.7129],
        text: "Student".to_string(),
        role: "main".to_string(),
        style: "primary_module".to_string(),
        z: 24,
    }]);

    polish_model_draw_plan_geometry(&mut plan);

    let student = object_box(&plan, "student_model");
    assert!(
        student[2] - student[0] >= 0.13,
        "short main module labels need a minimum width to avoid paper-width wrap risk: {student:?}"
    );
}

#[test]
fn model_draw_plan_polish_removes_auxiliary_inference_note_connectors_from_smoke() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Student-only inference is a compact context note, not a routed branch.",
            "visual_focus": ["comp_student", "comp_inference_note"],
            "reading_order": "top_to_bottom"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 12, "rows": 6},
            "regions": [
                {"id": "student_region", "bbox": [0.278, 0.6113333333333334, 0.472, 0.972]},
                {"id": "inference_note", "bbox": [0.785, 0.10666666666666669, 0.965, 0.31]}
            ]
        },
        "components": [
            {"id": "comp_student", "label": "Student", "role": "main", "visual_weight": "strong", "region": "student_region", "allowed_asset_id": null},
            {"id": "comp_inference_note", "label": "Inference: student only", "role": "context", "visual_weight": "muted", "region": "inference_note", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "edge_inference_hint", "from": "comp_student", "to": "comp_inference_note", "label": "", "semantic": "reference", "style": "dash", "importance": "aux"}
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
    .expect("smoke inference note figure plan should deserialize");
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_student".to_string(),
            bbox: [0.278, 0.6113333333333334, 0.472, 0.972],
            text: "Student".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "comp_inference_note".to_string(),
            bbox: [0.785, 0.10666666666666669, 0.965, 0.31],
            text: "Inference: student only".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 21,
        },
        DrawObject::Connector {
            id: "edge_inference_hint".to_string(),
            points: vec![
                [0.375, 0.6113333333333334],
                [0.79, 0.6113333333333334],
                [0.79, 0.31],
                [0.875, 0.31],
            ],
            from: Some("comp_student".to_string()),
            to: Some("comp_inference_note".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 10,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut plan, &figure_plan);

    assert!(
        box_object_exists(&plan, "comp_inference_note"),
        "the note text should remain editable as a context object"
    );
    assert!(
        !connector_object_exists(&plan, "edge_inference_hint"),
        "auxiliary inference-note connectors create route_detour noise and should be removed"
    );
}

#[test]
fn model_draw_plan_polish_repairs_current_smoke_student_branch_and_preserves_frozen_note() {
    let figure_plan: FigurePlan = serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher-student distillation with latent residual supervision.",
            "visual_focus": ["student_encoder", "teacher_encoder", "residual_obj"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 16, "rows": 12},
            "regions": [
                {"id": "input_slot", "bbox": [0.05, 0.35, 0.20, 0.58]},
                {"id": "teacher_branch", "bbox": [0.28, 0.09, 0.52, 0.42]},
                {"id": "student_branch", "bbox": [0.24, 0.58, 0.52, 0.92]},
                {"id": "latent_residual", "bbox": [0.60, 0.36, 0.78, 0.52]},
                {"id": "task_output", "bbox": [0.84, 0.74, 0.96, 0.92]},
                {"id": "task_loss", "bbox": [0.60, 0.70, 0.74, 0.86]},
                {"id": "inference_note", "bbox": [0.74, 0.56, 0.92, 0.66]}
            ]
        },
        "components": [
            {"id": "input_text", "label": "Input x", "role": "input", "visual_weight": "normal", "region": "input_slot", "allowed_asset_id": null},
            {"id": "teacher_encoder", "label": "Teacher Encoder", "role": "module", "visual_weight": "normal", "region": "teacher_branch", "allowed_asset_id": null},
            {"id": "teacher_latent", "label": "z_t", "role": "output", "visual_weight": "normal", "region": "teacher_branch", "allowed_asset_id": null},
            {"id": "student_encoder", "label": "Student Encoder", "role": "main", "visual_weight": "strong", "region": "student_branch", "allowed_asset_id": null},
            {"id": "student_latent", "label": "z_s", "role": "output", "visual_weight": "normal", "region": "student_branch", "allowed_asset_id": null},
            {"id": "student_head", "label": "Task Head", "role": "module", "visual_weight": "normal", "region": "student_branch", "allowed_asset_id": null},
            {"id": "residual_obj", "label": "L_res = ||z_t - z_s||", "role": "loss", "visual_weight": "strong", "region": "latent_residual", "allowed_asset_id": null},
            {"id": "task_pred", "label": "ŷ", "role": "output", "visual_weight": "normal", "region": "task_output", "allowed_asset_id": null},
            {"id": "task_obj", "label": "L_task", "role": "loss", "visual_weight": "normal", "region": "task_loss", "allowed_asset_id": null},
            {"id": "inference_tag", "label": "Inference: student only", "role": "context", "visual_weight": "muted", "region": "inference_note", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "e_input_teacher", "from": "input_text", "to": "teacher_encoder", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_teacher_latent", "from": "teacher_encoder", "to": "teacher_latent", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_input_student", "from": "input_text", "to": "student_encoder", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_student_latent", "from": "student_encoder", "to": "student_latent", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_student_head", "from": "student_latent", "to": "student_head", "label": "", "semantic": "data_flow", "style": "solid", "importance": "normal"},
            {"id": "e_student_pred", "from": "student_head", "to": "task_pred", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "e_teacher_residual", "from": "teacher_latent", "to": "residual_obj", "label": "supervision", "semantic": "supervision", "style": "dash", "importance": "main"},
            {"id": "e_student_residual", "from": "student_latent", "to": "residual_obj", "label": "", "semantic": "supervision", "style": "dash", "importance": "main"},
            {"id": "e_task_supervision", "from": "task_pred", "to": "task_obj", "label": "supervision", "semantic": "supervision", "style": "solid", "importance": "normal"}
        ],
        "annotations": [
            {"id": "ann_teacher_freeze", "label": "Frozen", "target_id": "teacher_encoder", "bbox": [0.3333, 0.0375, 0.4167, 0.1]},
            {"id": "ann_residual_dashed", "label": "Latent residual alignment", "target_id": "residual_obj", "bbox": [0.60, 0.5875, 0.7667, 0.65]}
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
            id: "input_text".to_string(),
            bbox: [0.065, 0.3983, 0.185, 0.5392],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher_encoder".to_string(),
            bbox: [0.2871, 0.0996, 0.5046, 0.2171],
            text: "Teacher Encoder".to_string(),
            role: "module".to_string(),
            style: "neutral_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "teacher_latent".to_string(),
            bbox: [0.2871, 0.2721, 0.5046, 0.4004],
            text: "z_t".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "student_encoder".to_string(),
            bbox: [0.2404, 0.5996, 0.4204, 0.7171],
            text: "Student Encoder".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "student_latent".to_string(),
            bbox: [0.4112, 0.5996, 0.5112, 0.7279],
            text: "z_s".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "student_head".to_string(),
            bbox: [0.2804, 0.7721, 0.3804, 0.9004],
            text: "Task Head".to_string(),
            role: "module".to_string(),
            style: "neutral_module".to_string(),
            z: 25,
        },
        DrawObject::Box {
            id: "residual_obj".to_string(),
            bbox: [0.6113, 0.3825, 0.7637, 0.4925],
            text: "L_res = ||z_t - z_s||".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 26,
        },
        DrawObject::Box {
            id: "task_pred".to_string(),
            bbox: [0.8458, 0.7600, 0.9458, 0.9125],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 27,
        },
        DrawObject::Box {
            id: "task_obj".to_string(),
            bbox: [0.6067, 0.7263, 0.7267, 0.8363],
            text: "L_task".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 28,
        },
        DrawObject::Connector {
            id: "e_input_teacher".to_string(),
            points: vec![[0.185, 0.4688], [0.185, 0.1584], [0.2871, 0.1584]],
            from: Some("input_text".to_string()),
            to: Some("teacher_encoder".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "e_teacher_latent".to_string(),
            points: vec![[0.3958, 0.2171], [0.3958, 0.2721]],
            from: Some("teacher_encoder".to_string()),
            to: Some("teacher_latent".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "e_input_student".to_string(),
            points: vec![[0.185, 0.4688], [0.185, 0.6584], [0.2404, 0.6584]],
            from: Some("input_text".to_string()),
            to: Some("student_encoder".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "e_student_latent".to_string(),
            points: vec![[0.4204, 0.6584], [0.4612, 0.6638]],
            from: Some("student_encoder".to_string()),
            to: Some("student_latent".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Connector {
            id: "e_student_head".to_string(),
            points: vec![[0.4612, 0.7279], [0.4612, 0.7721], [0.3304, 0.7721]],
            from: Some("student_latent".to_string()),
            to: Some("student_head".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 14,
        },
        DrawObject::Connector {
            id: "e_student_pred".to_string(),
            points: vec![
                [0.3804, 0.8362],
                [0.3804, 0.8613],
                [0.8458, 0.8613],
                [0.8458, 0.8362],
            ],
            from: Some("student_head".to_string()),
            to: Some("task_pred".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 15,
        },
        DrawObject::Connector {
            id: "e_teacher_residual".to_string(),
            points: vec![[0.5046, 0.3914], [0.6113, 0.3914]],
            from: Some("teacher_latent".to_string()),
            to: Some("residual_obj".to_string()),
            style: "dashed_supervision".to_string(),
            label: Some(DrawLabel {
                text: "supervision".to_string(),
                bbox: [0.5166, 0.3112, 0.6766, 0.3612],
            }),
            z: 16,
        },
        DrawObject::Connector {
            id: "e_student_residual".to_string(),
            points: vec![[0.4612, 0.6638], [0.4612, 0.4375], [0.6113, 0.4375]],
            from: Some("student_latent".to_string()),
            to: Some("residual_obj".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 17,
        },
        DrawObject::Connector {
            id: "e_task_supervision".to_string(),
            points: vec![[0.8458, 0.7813], [0.7267, 0.7813]],
            from: Some("task_pred".to_string()),
            to: Some("task_obj".to_string()),
            style: "normal_flow".to_string(),
            label: Some(DrawLabel {
                text: "supervision".to_string(),
                bbox: [0.5867, 0.6642, 0.7467, 0.7142],
            }),
            z: 18,
        },
        DrawObject::Text {
            id: "ann_residual_dashed".to_string(),
            bbox: [0.60, 0.5875, 0.7667, 0.65],
            text: "Latent residual alignment".to_string(),
            style: "annotation".to_string(),
            z: 40,
        },
    ]);

    polish_model_draw_plan_geometry_with_figure_plan(&mut draw_plan, &figure_plan);

    let student_encoder = object_box(&draw_plan, "student_encoder");
    let student_latent = object_box(&draw_plan, "student_latent");
    let student_head = object_box(&draw_plan, "student_head");
    let task_pred = object_box(&draw_plan, "task_pred");
    assert!(
        center_y(student_encoder) < center_y(student_latent)
            && center_y(student_latent) < center_y(student_head),
        "student branch should be a clean top-down chain: encoder={student_encoder:?}, latent={student_latent:?}, head={student_head:?}"
    );
    assert!(
        vertical_separation(student_encoder, student_latent) >= 0.04
            && vertical_separation(student_latent, student_head) >= 0.04,
        "student branch boxes need visible gutters: encoder={student_encoder:?}, latent={student_latent:?}, head={student_head:?}"
    );
    assert!(
        (center_y(student_head) - center_y(task_pred)).abs() < 0.04
            && center_x(task_pred) > center_x(student_head),
        "prediction should sit beside the task head, not at the end of a bottom U route: head={student_head:?}, pred={task_pred:?}"
    );
    assert!(
        box_area(task_pred) <= 0.0105,
        "single-character prediction output should be compact: {task_pred:?}"
    );
    let student_pred = connector(&draw_plan, "e_student_pred");
    assert!(
        student_pred.points.len() <= 2,
        "task head to prediction should be a direct adjacent connector: {:?}",
        student_pred.points
    );
    assert!(
        draw_plan_has_text(&draw_plan, "ann_teacher_freeze", "frozen"),
        "semantic Frozen annotation from FigurePlan should survive polish"
    );
}

#[test]
fn model_draw_plan_polish_compacts_floating_annotation_and_tall_output_from_current_smoke() {
    let mut draw_plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "teacher_model".to_string(),
            bbox: [0.23775, 0.16, 0.44975, 0.34],
            text: "Teacher\n(frozen)".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "student_model".to_string(),
            bbox: [0.22375, 0.66, 0.46375, 0.84],
            text: "Student\n(trainable)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "task_output".to_string(),
            bbox: [0.835125, 0.401796875, 0.946125, 0.675546875],
            text: "Task\nOutput ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [0.839296875, 0.771328125, 0.941953125, 0.901328125],
            text: "Task\nLoss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Connector {
            id: "student_to_output".to_string(),
            points: vec![[0.46375, 0.6677734375], [0.835125, 0.6677734375]],
            from: Some("student_model".to_string()),
            to: Some("task_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "output_to_loss".to_string(),
            points: vec![[0.890625, 0.675546875], [0.890625, 0.771328125]],
            from: Some("task_output".to_string()),
            to: Some("task_loss".to_string()),
            style: "main_flow".to_string(),
            label: Some(DrawLabel {
                text: "y".to_string(),
                bbox: [0.806625, 0.6984375, 0.866625, 0.7484375],
            }),
            z: 11,
        },
        DrawObject::Text {
            id: "teacher_frozen_note".to_string(),
            bbox: [0.28125, 0.025, 0.65625, 0.125],
            text: "inference excluded".to_string(),
            style: "annotation".to_string(),
            z: 40,
        },
    ]);

    polish_model_draw_plan_geometry(&mut draw_plan);

    let note = text_box(&draw_plan, "teacher_frozen_note");
    let teacher = object_box(&draw_plan, "teacher_model");
    assert!(
        box_area(note) < 0.018 && note[2] - note[0] <= 0.22,
        "floating inference note should be compact: {note:?}"
    );
    assert!(
        horizontal_separation(note, teacher) <= 0.04 || vertical_separation(note, teacher) <= 0.04,
        "floating inference note should be visibly anchored to the teacher box: note={note:?}, teacher={teacher:?}"
    );

    let output = object_box(&draw_plan, "task_output");
    assert!(
        output[3] - output[1] <= 0.16,
        "short task output label should not keep a tall empty box: {output:?}"
    );
    assert!(
        (center_y(output) - 0.6677734375).abs() < 0.04,
        "task output should stay aligned with the incoming student output route: {output:?}"
    );
}

#[test]
fn model_draw_plan_polish_compacts_current_smoke_prediction_and_student_inference_note() {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "input_text".to_string(),
            bbox: [
                0.05416666666666667,
                0.5158333333333334,
                0.15416666666666667,
                0.6508333333333334,
            ],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "teacher_module".to_string(),
            bbox: [0.3245, 0.2849999999999999, 0.5505, 0.465],
            text: "Teacher\n(large LM)".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student_module".to_string(),
            bbox: [0.3245, 0.7016666666666668, 0.5505, 0.8816666666666667],
            text: "Student\n(compact)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [
                0.5473333333333334,
                0.6566666666666666,
                0.7473333333333334,
                0.7566666666666666,
            ],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "latent_residual".to_string(),
            bbox: [
                0.7708333333333333,
                0.4633333333333333,
                0.9708333333333333,
                0.5633333333333334,
            ],
            text: "Latent Residual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "output_pred".to_string(),
            bbox: [0.7363333333333334, 0.6113333333333334, 0.972, 0.972],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 25,
        },
        DrawObject::Box {
            id: "inference_note".to_string(),
            bbox: [0.4913333333333334, 0.86, 0.7113333333333334, 0.98],
            text: "Inference: student only".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 26,
        },
        DrawObject::Connector {
            id: "e_student_output".to_string(),
            points: vec![
                [0.5505, 0.7916666666666667],
                [0.7363333333333334, 0.7916666666666667],
            ],
            from: Some("student_module".to_string()),
            to: Some("output_pred".to_string()),
            style: "normal_flow".to_string(),
            label: None,
            z: 12,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let output = object_box(&plan, "output_pred");
    assert!(
        output[2] - output[0] <= 0.13 && output[3] - output[1] <= 0.13,
        "single-symbol prediction output should be a compact badge, not a large empty panel: {output:?}"
    );
    assert!(
        (center_y(output) - 0.7916666666666667).abs() < 0.04,
        "compact prediction output should remain aligned with the student output connector: {output:?}"
    );

    let inference = inference_note_bbox(&plan)
        .expect("inference note should remain as a compact semantic annotation or badge");
    let student = object_box(&plan, "student_module");
    assert!(
        inference[3] - inference[1] <= 0.09,
        "student-only inference note should be a compact badge: {inference:?}"
    );
    assert!(
        intersection_area(inference, student) == 0.0
            && (vertical_separation(inference, student) >= 0.018
                || horizontal_separation(inference, student) >= 0.018),
        "student-only inference note must not crowd or overlap the student box: note={inference:?}, student={student:?}"
    );
}

#[test]
fn model_draw_plan_polish_moves_student_inference_annotation_and_shortens_loss_route_from_current_smoke(
) {
    let mut plan = minimal_draw_plan(vec![
        DrawObject::Box {
            id: "teacher_module".to_string(),
            bbox: [0.3245, 0.2849999999999999, 0.5505, 0.465],
            text: "Teacher\n(large LM)".to_string(),
            role: "module".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "student_module".to_string(),
            bbox: [0.3245, 0.7016666666666668, 0.5505, 0.8816666666666667],
            text: "Student\n(compact)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "task_loss".to_string(),
            bbox: [
                0.6199999999999999,
                0.5666666666666667,
                0.76,
                0.6666666666666667,
            ],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "latent_residual".to_string(),
            bbox: [0.6708333333333333, 0.35, 0.8708333333333332, 0.45],
            text: "Latent Residual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "output_pred".to_string(),
            bbox: [
                0.7231666666666667,
                0.74,
                0.8231666666666668,
                0.8400000000000001,
            ],
            text: "ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 25,
        },
        DrawObject::Text {
            id: "teacher_training_label".to_string(),
            bbox: [0.35, 0.235, 0.525, 0.275],
            text: "Training only".to_string(),
            style: "muted_annotation".to_string(),
            z: 27,
        },
        DrawObject::Text {
            id: "ann_inference".to_string(),
            bbox: [
                0.4741666666666667,
                0.31999999999999995,
                0.6341666666666667,
                0.38,
            ],
            text: "Inference: student only".to_string(),
            style: "annotation".to_string(),
            z: 28,
        },
        DrawObject::Connector {
            id: "e_student_loss".to_string(),
            points: vec![
                [0.5505, 0.7916666666666667],
                [0.5505, 0.8750000000000001],
                [0.96, 0.8750000000000001],
                [0.96, 0.6166666666666667],
                [0.6199999999999999, 0.6166666666666667],
            ],
            from: Some("student_module".to_string()),
            to: Some("task_loss".to_string()),
            style: "normal_flow".to_string(),
            label: Some(DrawLabel {
                text: "ŷ, y".to_string(),
                bbox: [
                    0.71925,
                    0.8990000000000001,
                    0.7912499999999999,
                    0.9490000000000002,
                ],
            }),
            z: 14,
        },
    ]);

    polish_model_draw_plan_geometry(&mut plan);

    let teacher = object_box(&plan, "teacher_module");
    let student = object_box(&plan, "student_module");
    if let Some(inference) = optional_text_box(&plan, "ann_inference") {
        assert_eq!(
            intersection_area(inference, teacher),
            0.0,
            "student inference annotation must be moved off the teacher box: ann={inference:?}, teacher={teacher:?}"
        );
        assert!(
            vertical_separation(inference, student) <= 0.06
                || horizontal_separation(inference, student) <= 0.06,
            "student inference annotation should remain visually anchored near the student: ann={inference:?}, student={student:?}"
        );
    }

    let loss_edge = connector(&plan, "e_student_loss");
    let edge_box = points_to_box_for_test(loss_edge.points);
    assert!(
        loss_edge.points.len() <= 4 && edge_box[2] <= 0.78 && edge_box[2] - edge_box[0] <= 0.22,
        "student-to-loss connector should be a local short elbow, not a right-margin detour: {:?}",
        loss_edge.points
    );
    assert!(
        loss_edge.label.is_none(),
        "generic ŷ,y label should be removed from the cramped student-to-loss connector"
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

fn latest_compact_smoke_teacher_student_plan() -> FigurePlan {
    serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher-student distillation with latent residual supervision: a compact student learns from task labels and latent residuals from a large teacher, enabling efficient inference.",
            "visual_focus": ["student", "teacher", "latent_residual"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 16, "rows": 12},
            "regions": [
                {"id": "input", "bbox": [0.0, 0.3333333333333333, 0.15625, 0.9166666666666666]},
                {"id": "student", "bbox": [0.21875, 0.25, 0.5625, 1.0]},
                {"id": "task_loss", "bbox": [0.40625, 0.375, 0.53125, 0.625]},
                {"id": "teacher", "bbox": [0.59375, 0.08333333333333333, 0.9375, 0.5]},
                {"id": "latent_residual", "bbox": [0.59375, 0.5416666666666666, 0.9375, 0.7083333333333334]},
                {"id": "output", "bbox": [0.21875, 0.875, 0.5625, 1.0]},
                {"id": "inference_note", "bbox": [0.84375, 0.7916666666666666, 0.96875, 0.9583333333333334]}
            ]
        },
        "components": [
            {"id": "comp_input", "label": "Input x", "role": "input", "visual_weight": "normal", "region": "input", "allowed_asset_id": null},
            {"id": "comp_student", "label": "Student", "role": "main", "visual_weight": "strong", "region": "student", "allowed_asset_id": null},
            {"id": "comp_task_loss", "label": "Task Loss", "role": "loss", "visual_weight": "normal", "region": "task_loss", "allowed_asset_id": null},
            {"id": "comp_teacher", "label": "Teacher", "role": "context", "visual_weight": "normal", "region": "teacher", "allowed_asset_id": null},
            {"id": "comp_latent_residual", "label": "Latent Residual", "role": "loss", "visual_weight": "strong", "region": "latent_residual", "allowed_asset_id": null},
            {"id": "comp_output", "label": "Prediction ŷ", "role": "output", "visual_weight": "normal", "region": "output", "allowed_asset_id": null},
            {"id": "comp_inference_note", "label": "Inference: student only", "role": "context", "visual_weight": "muted", "region": "inference_note", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "edge_input_to_student", "from": "comp_input", "to": "comp_student", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "edge_student_to_task_loss", "from": "comp_student", "to": "comp_task_loss", "label": "", "semantic": "loss", "style": "solid", "importance": "normal"},
            {"id": "edge_input_to_teacher", "from": "comp_input", "to": "comp_teacher", "label": "", "semantic": "reference", "style": "solid", "importance": "normal"},
            {"id": "edge_teacher_to_latent", "from": "comp_teacher", "to": "comp_latent_residual", "label": "", "semantic": "reference", "style": "solid", "importance": "main"},
            {"id": "edge_latent_to_student", "from": "comp_latent_residual", "to": "comp_student", "label": "", "semantic": "supervision", "style": "dash", "importance": "main"},
            {"id": "edge_student_to_output", "from": "comp_student", "to": "comp_output", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"}
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
    .expect("latest compact smoke fixture should deserialize")
}

fn latest_compact_smoke_draw_plan() -> DrawPlan {
    minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_input".to_string(),
            bbox: [0.021875000000000006, 0.5575, 0.134375, 0.6925],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "comp_student".to_string(),
            bbox: [0.291625, 0.5575, 0.489625, 0.6925],
            text: "Student".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "comp_task_loss".to_string(),
            bbox: [0.41875, 0.3925, 0.51875, 0.5025],
            text: "Task Loss".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "comp_teacher".to_string(),
            bbox: [0.666625, 0.22416666666666663, 0.864625, 0.35916666666666663],
            text: "Teacher".to_string(),
            role: "context".to_string(),
            style: "neutral_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "comp_latent_residual".to_string(),
            bbox: [0.665625, 0.57, 0.865625, 0.68],
            text: "Latent Residual".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "comp_output".to_string(),
            bbox: [0.23625, 0.8875, 0.545, 0.9875],
            text: "Prediction ŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 25,
        },
        DrawObject::Box {
            id: "comp_inference_note".to_string(),
            bbox: [
                0.7762500000000001,
                0.8091666666666666,
                0.95625,
                0.9408333333333334,
            ],
            text: "Inference: student only".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 26,
        },
        DrawObject::Connector {
            id: "edge_input_to_student".to_string(),
            points: vec![[0.134375, 0.625], [0.291625, 0.625]],
            from: Some("comp_input".to_string()),
            to: Some("comp_student".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "edge_student_to_task_loss".to_string(),
            points: vec![[0.46875, 0.5575], [0.46875, 0.5025]],
            from: Some("comp_student".to_string()),
            to: Some("comp_task_loss".to_string()),
            style: "solid_connector".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "edge_input_to_teacher".to_string(),
            points: vec![
                [0.134375, 0.625],
                [0.134375, 0.29166666666666663],
                [0.666625, 0.29166666666666663],
            ],
            from: Some("comp_input".to_string()),
            to: Some("comp_teacher".to_string()),
            style: "solid_connector".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "edge_teacher_to_latent".to_string(),
            points: vec![[0.765625, 0.35916666666666663], [0.765625, 0.57]],
            from: Some("comp_teacher".to_string()),
            to: Some("comp_latent_residual".to_string()),
            style: "solid_connector".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Connector {
            id: "edge_latent_to_student".to_string(),
            points: vec![[0.665625, 0.625], [0.489625, 0.625]],
            from: Some("comp_latent_residual".to_string()),
            to: Some("comp_student".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 14,
        },
        DrawObject::Connector {
            id: "edge_student_to_output".to_string(),
            points: vec![[0.390625, 0.6925], [0.390625, 0.8875]],
            from: Some("comp_student".to_string()),
            to: Some("comp_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 15,
        },
    ])
}

fn compact_inference_note_is_anchored(note: [f64; 4], student: [f64; 4], output: [f64; 4]) -> bool {
    let near_student =
        horizontal_separation(note, student) <= 0.10 && vertical_separation(note, student) <= 0.12;
    let near_output =
        horizontal_separation(note, output) <= 0.10 && vertical_separation(note, output) <= 0.10;
    near_student || near_output
}

fn latest_branch_corridor_smoke_plan() -> FigurePlan {
    serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Teacher provides latent residual supervision for a compact student; inference uses only the student path.",
            "visual_focus": ["comp_student", "comp_teacher", "comp_supervision"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 16, "rows": 12},
            "regions": [
                {"id": "input", "bbox": [0.0, 0.45, 0.15, 0.72]},
                {"id": "teacher", "bbox": [0.08, 0.05, 0.34, 0.28]},
                {"id": "student", "bbox": [0.43, 0.20, 0.72, 0.44]},
                {"id": "supervision", "bbox": [0.48, 0.04, 0.68, 0.19]},
                {"id": "output", "bbox": [0.86, 0.27, 0.98, 0.43]},
                {"id": "inference_note", "bbox": [0.48, 0.46, 0.68, 0.58]}
            ]
        },
        "components": [
            {"id": "comp_input", "label": "Task Input", "role": "input", "visual_weight": "normal", "region": "input", "allowed_asset_id": null},
            {"id": "comp_teacher", "label": "Teacher\n(latent)", "role": "context", "visual_weight": "normal", "region": "teacher", "allowed_asset_id": null},
            {"id": "comp_student", "label": "Student\n(compact)", "role": "main", "visual_weight": "strong", "region": "student", "allowed_asset_id": null},
            {"id": "comp_supervision", "label": "Latent Residual\nSupervision", "role": "loss", "visual_weight": "strong", "region": "supervision", "allowed_asset_id": null},
            {"id": "comp_output", "label": "Answer", "role": "output", "visual_weight": "normal", "region": "output", "allowed_asset_id": null},
            {"id": "comp_inference_note", "label": "Inference: student only", "role": "context", "visual_weight": "muted", "region": "inference_note", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "edge_input_to_student", "from": "comp_input", "to": "comp_student", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "edge_teacher_to_supervision", "from": "comp_teacher", "to": "comp_supervision", "label": "", "semantic": "supervision", "style": "solid", "importance": "main"},
            {"id": "edge_supervision_to_student", "from": "comp_supervision", "to": "comp_student", "label": "", "semantic": "supervision", "style": "dash", "importance": "main"},
            {"id": "edge_student_to_output", "from": "comp_student", "to": "comp_output", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"}
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
    .expect("latest branch corridor smoke fixture should deserialize")
}

fn latest_branch_corridor_smoke_draw_plan() -> DrawPlan {
    minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_input".to_string(),
            bbox: [0.02, 0.55, 0.123, 0.685],
            text: "Task Input".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "comp_teacher".to_string(),
            bbox: [0.108, 0.08, 0.32, 0.26],
            text: "Teacher\n(latent)".to_string(),
            role: "context".to_string(),
            style: "neutral_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "comp_student".to_string(),
            bbox: [0.457, 0.22, 0.7, 0.42],
            text: "Student\n(compact)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "comp_supervision".to_string(),
            bbox: [0.492, 0.05, 0.658, 0.18],
            text: "Latent Residual\nSupervision".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "comp_output".to_string(),
            bbox: [0.8745, 0.285, 0.9745, 0.415],
            text: "Answer".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Box {
            id: "comp_inference_note".to_string(),
            bbox: [0.4885, 0.47, 0.6685, 0.57],
            text: "Inference: student only".to_string(),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: 25,
        },
        DrawObject::Text {
            id: "anno_student_dominant".to_string(),
            bbox: [0.4485, 0.582, 0.7085, 0.662],
            text: "Main inference path".to_string(),
            style: "annotation".to_string(),
            z: 30,
        },
        DrawObject::Text {
            id: "anno_residual_dashed".to_string(),
            bbox: [0.21, 0.338, 0.37, 0.408],
            text: "Auxiliary training signal".to_string(),
            style: "annotation".to_string(),
            z: 31,
        },
        DrawObject::Connector {
            id: "edge_input_to_student".to_string(),
            points: vec![[0.123, 0.6175], [0.123, 0.32], [0.457, 0.32]],
            from: Some("comp_input".to_string()),
            to: Some("comp_student".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "edge_teacher_to_supervision".to_string(),
            points: vec![[0.32, 0.115], [0.492, 0.115]],
            from: Some("comp_teacher".to_string()),
            to: Some("comp_supervision".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "edge_supervision_to_student".to_string(),
            points: vec![[0.5785, 0.18], [0.5785, 0.22]],
            from: Some("comp_supervision".to_string()),
            to: Some("comp_student".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "edge_student_to_output".to_string(),
            points: vec![[0.7, 0.35], [0.8745, 0.35]],
            from: Some("comp_student".to_string()),
            to: Some("comp_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 13,
        },
    ])
}

fn shared_input_teacher_student_smoke_plan() -> FigurePlan {
    serde_json::from_value(json!({
        "version": "0.1",
        "canvas": {"aspect": "paper-wide", "target_width_mm": 85, "safe_margin": 0.06},
        "story": {
            "main_message": "Both teacher and student consume the same task input; the teacher produces latent residual supervision and the student predicts the answer.",
            "visual_focus": ["comp_input", "comp_teacher", "comp_student"],
            "reading_order": "left_to_right"
        },
        "layout": {
            "template": "teacher_student",
            "grid": {"columns": 16, "rows": 12},
            "regions": [
                {"id": "input", "bbox": [0.02, 0.56, 0.20, 0.80]},
                {"id": "teacher", "bbox": [0.44, 0.03, 0.68, 0.23]},
                {"id": "student", "bbox": [0.44, 0.54, 0.70, 0.76]},
                {"id": "latent_residual", "bbox": [0.52, 0.33, 0.73, 0.48]},
                {"id": "output", "bbox": [0.84, 0.54, 0.96, 0.77]}
            ]
        },
        "components": [
            {"id": "comp_input", "label": "Task Input\nx", "role": "input", "visual_weight": "normal", "region": "input", "allowed_asset_id": null},
            {"id": "comp_teacher", "label": "Teacher\n(Large LM)", "role": "context", "visual_weight": "normal", "region": "teacher", "allowed_asset_id": null},
            {"id": "comp_student", "label": "Student\n(Compact LM)", "role": "main", "visual_weight": "strong", "region": "student", "allowed_asset_id": null},
            {"id": "comp_latent_residual", "label": "Latent Residual\nSupervision", "role": "loss", "visual_weight": "strong", "region": "latent_residual", "allowed_asset_id": null},
            {"id": "comp_output", "label": "Answer\nŷ", "role": "output", "visual_weight": "normal", "region": "output", "allowed_asset_id": null}
        ],
        "edges": [
            {"id": "edge_input_student", "from": "comp_input", "to": "comp_student", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"},
            {"id": "edge_input_teacher", "from": "comp_input", "to": "comp_teacher", "label": "", "semantic": "reference", "style": "solid", "importance": "normal"},
            {"id": "edge_teacher_residual", "from": "comp_teacher", "to": "comp_latent_residual", "label": "", "semantic": "supervision", "style": "solid", "importance": "main"},
            {"id": "edge_residual_student", "from": "comp_latent_residual", "to": "comp_student", "label": "", "semantic": "supervision", "style": "dash", "importance": "main"},
            {"id": "edge_student_output", "from": "comp_student", "to": "comp_output", "label": "", "semantic": "data_flow", "style": "solid", "importance": "main"}
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
    .expect("shared input smoke fixture should deserialize")
}

fn shared_input_teacher_student_smoke_draw_plan() -> DrawPlan {
    minimal_draw_plan(vec![
        DrawObject::Box {
            id: "comp_input".to_string(),
            bbox: [0.028, 0.5975, 0.1803, 0.7775],
            text: "Task Input\nx".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
            z: 20,
        },
        DrawObject::Box {
            id: "comp_student".to_string(),
            bbox: [0.4425, 0.56625, 0.6825, 0.74625],
            text: "Student\n(Compact LM)".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 21,
        },
        DrawObject::Box {
            id: "comp_teacher".to_string(),
            bbox: [0.4495, 0.035, 0.6755, 0.215],
            text: "Teacher\n(Large LM)".to_string(),
            role: "context".to_string(),
            style: "neutral_module".to_string(),
            z: 22,
        },
        DrawObject::Box {
            id: "comp_latent_residual".to_string(),
            bbox: [0.525, 0.34, 0.725, 0.47],
            text: "Latent Residual\nSupervision".to_string(),
            role: "loss".to_string(),
            style: "accent_module".to_string(),
            z: 23,
        },
        DrawObject::Box {
            id: "comp_output".to_string(),
            bbox: [0.8403, 0.54875, 0.9513, 0.76375],
            text: "Answer\nŷ".to_string(),
            role: "output".to_string(),
            style: "neutral_module".to_string(),
            z: 24,
        },
        DrawObject::Text {
            id: "anno_training_only".to_string(),
            bbox: [0.643, 0.24625, 0.803, 0.30875],
            text: "training only".to_string(),
            style: "annotation".to_string(),
            z: 30,
        },
        DrawObject::Connector {
            id: "edge_input_student".to_string(),
            points: vec![[0.1803, 0.65625], [0.4425, 0.65625]],
            from: Some("comp_input".to_string()),
            to: Some("comp_student".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 10,
        },
        DrawObject::Connector {
            id: "edge_input_teacher".to_string(),
            points: vec![[0.1803, 0.6875], [0.1803, 0.125], [0.4495, 0.125]],
            from: Some("comp_input".to_string()),
            to: Some("comp_teacher".to_string()),
            style: "solid_connector".to_string(),
            label: None,
            z: 11,
        },
        DrawObject::Connector {
            id: "edge_teacher_residual".to_string(),
            points: vec![[0.625, 0.215], [0.625, 0.34]],
            from: Some("comp_teacher".to_string()),
            to: Some("comp_latent_residual".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 12,
        },
        DrawObject::Connector {
            id: "edge_residual_student".to_string(),
            points: vec![[0.5625, 0.47], [0.5625, 0.56625]],
            from: Some("comp_latent_residual".to_string()),
            to: Some("comp_student".to_string()),
            style: "dashed_supervision".to_string(),
            label: None,
            z: 13,
        },
        DrawObject::Connector {
            id: "edge_student_output".to_string(),
            points: vec![[0.6825, 0.65625], [0.8403, 0.65625]],
            from: Some("comp_student".to_string()),
            to: Some("comp_output".to_string()),
            style: "main_flow".to_string(),
            label: None,
            z: 14,
        },
    ])
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

fn optional_text_box(plan: &DrawPlan, id: &str) -> Option<[f64; 4]> {
    plan.objects.iter().find_map(|object| match object {
        DrawObject::Text {
            id: object_id,
            bbox,
            ..
        } if object_id == id => Some(*bbox),
        _ => None,
    })
}

fn inference_note_bbox(plan: &DrawPlan) -> Option<[f64; 4]> {
    plan.objects.iter().find_map(|object| match object {
        DrawObject::Box { bbox, text, .. } | DrawObject::Text { bbox, text, .. }
            if text.to_lowercase().contains("inference")
                && text.to_lowercase().contains("student") =>
        {
            Some(*bbox)
        }
        _ => None,
    })
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

fn points_to_box_for_test(points: &[[f64; 2]]) -> [f64; 4] {
    let first = points.first().copied().unwrap_or([0.5, 0.5]);
    points
        .iter()
        .fold([first[0], first[1], first[0], first[1]], |acc, point| {
            [
                acc[0].min(point[0]),
                acc[1].min(point[1]),
                acc[2].max(point[0]),
                acc[3].max(point[1]),
            ]
        })
}

fn has_long_horizontal_segment_for_test(points: &[[f64; 2]], min_len: f64) -> bool {
    points.windows(2).any(|window| {
        (window[0][1] - window[1][1]).abs() < 0.006 && (window[0][0] - window[1][0]).abs() > min_len
    })
}

fn box_width_for_test(bbox: [f64; 4]) -> f64 {
    bbox[2] - bbox[0]
}

fn box_height_for_test(bbox: [f64; 4]) -> f64 {
    bbox[3] - bbox[1]
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

fn label_touches_any_segment(label_bbox: [f64; 4], points: &[[f64; 2]]) -> bool {
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
        intersection_area(label_bbox, bbox) > 0.0001
    })
}

fn label_near_any_segment(label_bbox: [f64; 4], points: &[[f64; 2]], margin: f64) -> bool {
    points.windows(2).any(|window| {
        let bbox = expand_box(
            [
                window[0][0].min(window[1][0]),
                window[0][1].min(window[1][1]),
                window[0][0].max(window[1][0]),
                window[0][1].max(window[1][1]),
            ],
            margin,
        );
        intersection_area(label_bbox, bbox) > 0.0001
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

fn union_box_for_test(left: [f64; 4], right: [f64; 4]) -> [f64; 4] {
    [
        left[0].min(right[0]),
        left[1].min(right[1]),
        left[2].max(right[2]),
        left[3].max(right[3]),
    ]
}

fn vertical_separation(left: [f64; 4], right: [f64; 4]) -> f64 {
    if left[3] < right[1] {
        right[1] - left[3]
    } else if right[3] < left[1] {
        left[1] - right[3]
    } else {
        0.0
    }
}

fn axis_overlap_ratio_for_test(
    left_start: f64,
    left_end: f64,
    right_start: f64,
    right_end: f64,
) -> f64 {
    let overlap = (left_end.min(right_end) - left_start.max(right_start)).max(0.0);
    let shorter = (left_end - left_start)
        .abs()
        .min((right_end - right_start).abs())
        .max(0.0001);
    overlap / shorter
}

fn task_loss_sits_between_rows_for_test(
    task_loss: [f64; 4],
    teacher: [f64; 4],
    student: [f64; 4],
) -> bool {
    let upper = if center_y(teacher) <= center_y(student) {
        teacher
    } else {
        student
    };
    let lower = if center_y(teacher) <= center_y(student) {
        student
    } else {
        teacher
    };
    let task_center_y = center_y(task_loss);
    if task_center_y <= upper[3] || task_center_y >= lower[1] {
        return false;
    }
    let branch_span = [
        upper[0].min(lower[0]),
        upper[3],
        upper[2].max(lower[2]),
        lower[1],
    ];
    let x_overlap = (task_loss[2].min(branch_span[2]) - task_loss[0].max(branch_span[0])).max(0.0);
    x_overlap / box_width_for_test(task_loss).max(0.0001) > 0.10
}

fn horizontal_separation(left: [f64; 4], right: [f64; 4]) -> f64 {
    if left[2] < right[0] {
        right[0] - left[2]
    } else if right[2] < left[0] {
        left[0] - right[2]
    } else {
        0.0
    }
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
