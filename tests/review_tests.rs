use methodfig::agent::{
    apply_patch_plan_to_figure, parse_patch_plan_text, patch_plan_has_unexecutable_layout_patch,
};
use methodfig::schema::{
    Annotation, CanvasAspect, Component, ComponentRole, LayoutRegion, PatchExecutor,
    PatchOperation, PatchOperationType, PatchPlan, PatchStopReason, StyleName, VisualWeight,
};
use methodfig::tools::review::{
    apply_plan_geometry_gate, apply_render_quality_gate, mock_patch_plan, mock_review,
    render_quality_issues, review_passes_threshold,
};

#[test]
fn acceptance_thresholds_reject_first_mock_review_and_accept_second() {
    let first = mock_review(0);
    assert!(!review_passes_threshold(&first));
    assert!(!first.blocking_issues.is_empty());

    let second = mock_review(1);
    assert!(review_passes_threshold(&second));
    assert!(second.blocking_issues.is_empty());
}

#[test]
fn acceptance_thresholds_reject_low_color_or_aesthetic_scores() {
    let mut review = mock_review(1);
    review.scores.color_semantics = 4;
    assert!(!review_passes_threshold(&review));

    let mut review = mock_review(1);
    review.scores.aesthetic_quality = 7;
    assert!(!review_passes_threshold(&review));
}

#[test]
fn render_quality_gate_flags_overlaps_and_degenerate_edges() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {"id": "a", "kind": "component", "bbox": [0.10, 0.20, 0.40, 0.50]},
                {"id": "b", "kind": "component", "bbox": [0.20, 0.25, 0.46, 0.55]},
                {"id": "bad_edge", "kind": "edge", "bbox": [0.30, 0.30, 0.31, 0.31]}
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let issues = render_quality_issues(&layout_map).expect("quality gate should read layout map");

    assert!(issues.iter().any(|issue| issue.contains("overlap")));
    assert!(issues.iter().any(|issue| issue.contains("degenerate edge")));
}

#[test]
fn render_quality_gate_flags_label_overlap_and_low_utilization() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {"id": "a", "kind": "component", "bbox": [0.10, 0.20, 0.22, 0.30]},
                {"id": "b", "kind": "component", "bbox": [0.30, 0.20, 0.42, 0.30]},
                {"id": "edge1", "kind": "edge", "bbox": [0.13, 0.18, 0.15, 0.32]},
                {"id": "label", "kind": "label", "bbox": [0.12, 0.20, 0.20, 0.30]}
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let issues = render_quality_issues(&layout_map).expect("quality gate should read layout map");

    assert!(issues.iter().any(|issue| issue.contains("under-utilized")));
    assert!(issues
        .iter()
        .any(|issue| issue.contains("label label overlaps edge edge1")));
}

#[test]
fn render_quality_gate_uses_polyline_points_for_edge_crossing() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {
                    "id": "vertical",
                    "kind": "edge",
                    "bbox": [0.68, 0.58, 0.68, 0.72],
                    "points": [[0.68, 0.58], [0.68, 0.72]]
                },
                {
                    "id": "elbow",
                    "kind": "edge",
                    "bbox": [0.52, 0.50, 0.82, 0.735],
                    "points": [[0.52, 0.735], [0.62, 0.735], [0.62, 0.535], [0.74, 0.535], [0.74, 0.50], [0.82, 0.50]]
                }
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let issues = render_quality_issues(&layout_map).expect("quality gate should read layout map");

    assert!(
        !issues.iter().any(|issue| issue.contains("edge crossing")),
        "polyline segments do not cross; the edge bbox diagonal must not be used as geometry: {issues:?}"
    );
}

#[test]
fn plan_geometry_gate_rejects_diagonal_simple_chain_without_treating_annotations_as_source_of_truth(
) {
    let mut plan = methodfig::schema::FigurePlan::mock_from_method(
        "Teacher guides student with latent residuals.",
        StyleName::WpsClean,
        CanvasAspect::PaperWide,
        85,
    );
    plan.layout.regions = vec![
        LayoutRegion {
            id: "teacher_region".to_string(),
            bbox: [0.05, 0.10, 0.22, 0.28],
        },
        LayoutRegion {
            id: "student_region".to_string(),
            bbox: [0.34, 0.34, 0.52, 0.52],
        },
        LayoutRegion {
            id: "output_region".to_string(),
            bbox: [0.65, 0.60, 0.83, 0.78],
        },
    ];
    plan.components[0].region = "teacher_region".to_string();
    plan.components[1].region = "student_region".to_string();
    plan.components[2].region = "output_region".to_string();
    plan.annotations.push(Annotation {
        id: "ann_corner".to_string(),
        label: "corner note".to_string(),
        target_id: None,
        bbox: Some([0.00, 0.00, 0.05, 0.05]),
    });
    let mut review = mock_review(1);

    apply_plan_geometry_gate(&plan, &mut review);

    assert!(review
        .blocking_issues
        .iter()
        .any(|issue| issue.contains("simple chain should read horizontally or vertically")));
    assert!(
        !review
            .blocking_issues
            .iter()
            .any(|issue| issue.contains("annotation ann_corner sits outside the main figure area")),
        "FigurePlan annotations are not rendered source-of-truth after DrawPlan repair"
    );
}

#[test]
fn render_quality_gate_blocks_acceptance_even_when_review_scores_are_high() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {"id": "a", "kind": "component", "bbox": [0.10, 0.20, 0.40, 0.50]},
                {"id": "b", "kind": "component", "bbox": [0.20, 0.25, 0.46, 0.55]}
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let mut review = mock_review(1);
    apply_render_quality_gate(&mut review, &layout_map).expect("quality gate should run");
    review.passed = review_passes_threshold(&review);

    assert!(!review.passed);
    assert!(review
        .blocking_issues
        .iter()
        .any(|issue| issue.contains("component overlap")));
}

#[test]
fn patch_routing_keeps_reasoner_layout_patch_for_main_module() {
    let patch = mock_patch_plan();
    assert_eq!(
        patch.operations[0].executor,
        methodfig::schema::PatchExecutor::Reasoner
    );
    assert_eq!(
        patch.operations[0].operation_type,
        methodfig::schema::PatchOperationType::LayoutPatch
    );

    let mut plan = methodfig::schema::FigurePlan::mock_from_method(
        "Teacher guides student with latent residuals.",
        StyleName::WpsClean,
        CanvasAspect::PaperWide,
        85,
    );
    apply_patch_plan_to_figure(&mut plan, &patch);
    assert!(plan
        .components
        .iter()
        .any(|component| component.visual_weight == VisualWeight::Strong));
    assert!(plan
        .story
        .visual_focus
        .contains(&"main contribution path emphasized".to_string()));
}

#[test]
fn layout_patch_updates_target_region_bbox() {
    let mut plan = methodfig::schema::FigurePlan::mock_from_method(
        "Teacher guides student with latent residuals.",
        StyleName::WpsClean,
        CanvasAspect::PaperWide,
        85,
    );
    let patch = PatchPlan {
        operations: vec![PatchOperation {
            id: "op_region".to_string(),
            target_id: "main_lane".to_string(),
            executor: PatchExecutor::Reasoner,
            operation_type: PatchOperationType::LayoutPatch,
            action:
                "Change main_lane bbox from [0.06, 0.22, 0.94, 0.76] to [0.12, 0.18, 0.88, 0.72]."
                    .to_string(),
            expected_effect: "The main lane leaves clearer margins.".to_string(),
        }],
        stop_reason: PatchStopReason::Continue,
    };

    apply_patch_plan_to_figure(&mut plan, &patch);

    let region = plan
        .layout
        .regions
        .iter()
        .find(|region| region.id == "main_lane")
        .expect("main lane exists");
    assert_eq!(region.bbox, [0.12, 0.18, 0.88, 0.72]);
}

#[test]
fn layout_patch_can_create_component_region_from_action() {
    let mut plan = methodfig::schema::FigurePlan::mock_from_method(
        "Teacher guides student with latent residuals.",
        StyleName::WpsClean,
        CanvasAspect::PaperWide,
        85,
    );
    plan.components.push(Component::new(
        "student_latent",
        "Latent h_S",
        ComponentRole::Module,
        VisualWeight::Normal,
        "main_lane",
    ));
    let patch = PatchPlan {
        operations: vec![PatchOperation {
            id: "op_student_latent_region".to_string(),
            target_id: "student_latent".to_string(),
            executor: PatchExecutor::Reasoner,
            operation_type: PatchOperationType::LayoutPatch,
            action: "Create dedicated student_latent_region with bbox [0.15, 0.55, 0.55, 0.75] and assign student_latent to this region.".to_string(),
            expected_effect: "Student latent state is visible below the student module.".to_string(),
        }],
        stop_reason: PatchStopReason::Continue,
    };

    apply_patch_plan_to_figure(&mut plan, &patch);

    let component = plan
        .components
        .iter()
        .find(|component| component.id == "student_latent")
        .expect("student latent exists");
    assert_eq!(component.region, "student_latent_region");
    let region = plan
        .layout
        .regions
        .iter()
        .find(|region| region.id == "student_latent_region")
        .expect("new region exists");
    assert_eq!(region.bbox, [0.15, 0.55, 0.55, 0.75]);
}

#[test]
fn layout_patch_expands_slightly_out_of_bounds_component_region() {
    let mut plan = methodfig::schema::FigurePlan::mock_from_method(
        "Teacher guides student with latent residuals.",
        StyleName::WpsClean,
        CanvasAspect::PaperWide,
        85,
    );
    plan.components.push(Component::new(
        "c_inference_only",
        "Inference: Student only",
        ComponentRole::Module,
        VisualWeight::Muted,
        "main_lane",
    ));
    let patch = PatchPlan {
        operations: vec![PatchOperation {
            id: "op_inference_caption".to_string(),
            target_id: "c_inference_only".to_string(),
            executor: PatchExecutor::Reasoner,
            operation_type: PatchOperationType::LayoutPatch,
            action: "Update its region from r_inference_label to a new adjacent region with bbox [0.2, 1.0, 0.6, 1.08].".to_string(),
            expected_effect: "Inference caption remains visible at the canvas bottom edge.".to_string(),
        }],
        stop_reason: PatchStopReason::Continue,
    };

    apply_patch_plan_to_figure(&mut plan, &patch);

    let region = plan
        .layout
        .regions
        .iter()
        .find(|region| region.id == "c_inference_only_region")
        .expect("inference region exists");
    assert_eq!(region.bbox, [0.2, 0.94, 0.6, 1.0]);
}

#[test]
fn layout_patch_updates_annotation_bbox() {
    let mut plan = methodfig::schema::FigurePlan::mock_from_method(
        "Teacher guides student with latent residuals.",
        StyleName::WpsClean,
        CanvasAspect::PaperWide,
        85,
    );
    plan.annotations.push(Annotation {
        id: "ann_training".to_string(),
        label: "Training".to_string(),
        target_id: Some("student".to_string()),
        bbox: Some([0.1, 0.1, 0.9, 0.9]),
    });
    let patch = PatchPlan {
        operations: vec![PatchOperation {
            id: "op_annotation".to_string(),
            target_id: "ann_training".to_string(),
            executor: PatchExecutor::Reasoner,
            operation_type: PatchOperationType::LayoutPatch,
            action: "Shrink ann_training bbox to [0.1, 0.17, 0.5, 0.32].".to_string(),
            expected_effect: "Training annotation avoids the data-flow edge.".to_string(),
        }],
        stop_reason: PatchStopReason::Continue,
    };

    apply_patch_plan_to_figure(&mut plan, &patch);

    assert_eq!(plan.annotations[0].bbox, Some([0.1, 0.17, 0.5, 0.32]));
}

#[test]
fn text_patch_updates_edge_label_from_single_quotes() {
    let mut plan = methodfig::schema::FigurePlan::mock_from_method(
        "Teacher guides student with latent residuals.",
        StyleName::WpsClean,
        CanvasAspect::PaperWide,
        85,
    );
    let patch = PatchPlan {
        operations: vec![PatchOperation {
            id: "op_edge_label".to_string(),
            target_id: "teacher_to_student".to_string(),
            executor: PatchExecutor::Reasoner,
            operation_type: PatchOperationType::TextPatch,
            action: "Change teacher_to_student label to 'latent supervision'.".to_string(),
            expected_effect: "The supervision edge label is clearer.".to_string(),
        }],
        stop_reason: PatchStopReason::Continue,
    };

    apply_patch_plan_to_figure(&mut plan, &patch);

    let edge = plan
        .edges
        .iter()
        .find(|edge| edge.id == "teacher_to_student")
        .expect("edge exists");
    assert_eq!(edge.label, "latent supervision");
}

#[test]
fn detects_unexecutable_layout_patch_without_bbox() {
    let missing_bbox = PatchPlan {
        operations: vec![PatchOperation {
            id: "op_missing_bbox".to_string(),
            target_id: "r_teacher".to_string(),
            executor: PatchExecutor::Reasoner,
            operation_type: PatchOperationType::LayoutPatch,
            action: "Shrink teacher region to avoid overlap.".to_string(),
            expected_effect: "Teacher no longer overlaps.".to_string(),
        }],
        stop_reason: PatchStopReason::Continue,
    };
    assert!(patch_plan_has_unexecutable_layout_patch(&missing_bbox));

    let executable = PatchPlan {
        operations: vec![PatchOperation {
            id: "op_with_bbox".to_string(),
            target_id: "r_teacher".to_string(),
            executor: PatchExecutor::Reasoner,
            operation_type: PatchOperationType::LayoutPatch,
            action: "Move r_teacher to [0.55, 0.08, 0.78, 0.32].".to_string(),
            expected_effect: "Teacher has a concrete region.".to_string(),
        }],
        stop_reason: PatchStopReason::Continue,
    };
    assert!(!patch_plan_has_unexecutable_layout_patch(&executable));
}

#[test]
fn patch_plan_parser_defaults_missing_stop_reason_to_continue() {
    let patch = parse_patch_plan_text(
        r#"{
          "operations": [
            {
              "id": "op1",
              "target_id": "r_teacher",
              "executor": "reasoner",
              "operation_type": "layout_patch",
              "action": "Move r_teacher to [0.2, 0.1, 0.4, 0.3].",
              "expected_effect": "Teacher is moved."
            }
          ]
        }"#,
    )
    .expect("missing stop_reason should default to continue");

    assert_eq!(patch.stop_reason, PatchStopReason::Continue);
    assert_eq!(patch.operations.len(), 1);
}
