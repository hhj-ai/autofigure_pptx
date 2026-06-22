use methodfig::agent::{
    apply_patch_plan_to_figure, parse_patch_plan_text, patch_plan_has_unexecutable_layout_patch,
};
use methodfig::schema::{
    Annotation, CanvasAspect, Component, ComponentRole, IssueSeverity, LayoutRegion,
    LocalizedIssue, PatchExecutor, PatchOperation, PatchOperationType, PatchPlan, PatchStopReason,
    RejectedAsset, Review, ReviewScores, StyleName, VisualWeight,
};
use methodfig::tools::review::{
    apply_plan_geometry_gate, apply_render_quality_gate, build_quality_report, mock_patch_plan,
    mock_review, render_quality_issues, review_passes_threshold, sanitize_review_false_positives,
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
fn review_false_positive_filter_removes_missing_phase_only_annotations_only() {
    let mut clean = mock_review(1);
    clean.passed = false;
    clean.blocking_issues.push(
        "Missing training_label annotation from FigurePlan creates asymmetric annotation pattern"
            .to_string(),
    );
    clean.rejected_assets.push(RejectedAsset {
        asset_id: "training_label".to_string(),
        reason: "missing annotation from FigurePlan".to_string(),
    });

    sanitize_review_false_positives(&mut clean);

    assert!(clean.blocking_issues.is_empty());
    assert!(clean.rejected_assets.is_empty());
    assert!(clean.passed);

    let mut review = Review {
        passed: false,
        scores: ReviewScores {
            semantic_fidelity: 9,
            story_clarity: 9,
            visual_hierarchy: 9,
            paper_readability: 9,
            layout_cleanliness: 9,
            arrow_routing: 9,
            color_semantics: 9,
            aesthetic_quality: 9,
            wps_editability: 10,
        },
        blocking_issues: vec![
            "Missing training_label annotation from FigurePlan creates asymmetric annotation pattern"
                .to_string(),
            "training_label overlaps a connector and must move".to_string(),
        ],
        localized_issues: vec![
            LocalizedIssue {
                target_id: "training_label".to_string(),
                bbox: [0.36, 0.62, 0.50, 0.70],
                severity: IssueSeverity::Blocking,
                issue: "Missing training_label annotation from FigurePlan".to_string(),
                evidence: "The phase-only annotation is absent from DrawPlan".to_string(),
                suggested_direction: "Restore the floating Training label".to_string(),
            },
            LocalizedIssue {
                target_id: "training_loss".to_string(),
                bbox: [0.70, 0.42, 0.82, 0.52],
                severity: IssueSeverity::Blocking,
                issue: "training_loss label overlaps a connector".to_string(),
                evidence: "The visible loss label intersects the edge route".to_string(),
                suggested_direction: "Move the loss label away from the connector".to_string(),
            },
        ],
        accepted_assets: vec![],
        rejected_assets: vec![RejectedAsset {
            asset_id: "training_label".to_string(),
            reason: "missing annotation from FigurePlan".to_string(),
        }],
    };

    sanitize_review_false_positives(&mut review);

    assert!(!review
        .blocking_issues
        .iter()
        .any(|issue| issue.contains("Missing training_label")));
    assert!(review
        .blocking_issues
        .iter()
        .any(|issue| issue.contains("overlaps")));
    assert!(review
        .localized_issues
        .iter()
        .all(|issue| issue.target_id != "training_label"));
    assert!(review
        .localized_issues
        .iter()
        .any(|issue| issue.target_id == "training_loss"));
    assert!(review.rejected_assets.is_empty());
    assert!(!review.passed);
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
fn quality_report_binds_render_issues_to_target_ids() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {"id": "teacher", "kind": "component", "bbox": [0.10, 0.20, 0.40, 0.50]},
                {"id": "student", "kind": "component", "bbox": [0.20, 0.25, 0.46, 0.55]},
                {
                    "id": "e_teacher_student",
                    "kind": "edge",
                    "bbox": [0.35, 0.30, 0.82, 0.80],
                    "from": "teacher",
                    "to": "student",
                    "points": [[0.35,0.30],[0.36,0.80],[0.50,0.80],[0.50,0.32],[0.82,0.32]]
                }
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    assert!(!report.passed);
    let overlap = report
        .issues
        .iter()
        .find(|issue| issue.issue_type == "component_overlap")
        .expect("overlap issue should be structured");
    assert_eq!(overlap.severity, "blocking");
    assert!(overlap.target_ids.contains(&"teacher".to_string()));
    assert!(overlap.target_ids.contains(&"student".to_string()));

    let detour = report
        .issues
        .iter()
        .find(|issue| issue.issue_type == "route_detour")
        .expect("detour issue should be structured");
    assert!(detour.target_ids.contains(&"e_teacher_student".to_string()));
    assert!(detour.target_ids.contains(&"teacher".to_string()));
    assert!(detour.target_ids.contains(&"student".to_string()));
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
fn quality_report_flags_component_crowding_without_overlap() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {"id": "left", "kind": "component", "bbox": [0.10, 0.30, 0.30, 0.55]},
                {"id": "right", "kind": "component", "bbox": [0.322, 0.31, 0.52, 0.56]}
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let crowding = report
        .issues
        .iter()
        .find(|issue| issue.issue_type == "component_crowding")
        .expect("nearby boxes should be treated as crowded even without overlap");
    assert_eq!(crowding.severity, "major");
    assert!(crowding.target_ids.contains(&"left".to_string()));
    assert!(crowding.target_ids.contains(&"right".to_string()));
    assert!(crowding.evidence.contains("too close"));
}

#[test]
fn quality_report_flags_smoke_dogleg_far_label_and_narrow_gutter() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {"id": "task_input", "kind": "component", "bbox": [0.05265625, 0.84465625, 0.15265625, 0.96784375], "text": "Task Input x", "font_size_pt": 13.1, "margin_in": 0.022},
                {"id": "student_module", "kind": "component", "bbox": [0.0745, 0.56625, 0.3005, 0.74625], "text": "Student\\n(small LM, inference only)", "font_size_pt": 13.1, "margin_in": 0.032},
                {"id": "teacher_module", "kind": "component", "bbox": [0.7203333333333333, 0.56625, 0.9463333333333332, 0.74625], "text": "Teacher\\n(large LM)", "font_size_pt": 13.1, "margin_in": 0.032},
                {"id": "task_loss", "kind": "component", "bbox": [0.185, 0.84465625, 0.285, 0.96784375], "text": "Task Loss L_task", "font_size_pt": 13.1, "margin_in": 0.022},
                {"id": "e_input_teacher", "kind": "edge", "bbox": [0.15265625, 0.65625, 0.7203333333333333, 0.90625], "from": "task_input", "to": "teacher_module", "points": [[0.15265625,0.90625],[0.15265625,0.78125],[0.7203333333333333,0.78125],[0.7203333333333333,0.65625]]},
                {"id": "e_student_taskloss", "kind": "edge", "bbox": [0.15, 0.74625, 0.235, 0.84465625], "from": "student_module", "to": "task_loss", "points": [[0.235,0.74625],[0.15,0.74625],[0.15,0.84465625],[0.185,0.84465625]]},
                {"id": "e_student_taskloss_label", "kind": "label", "bbox": [0.3125, 0.63125, 0.4725, 0.68125], "text": "ŷ vs y", "font_size_pt": 10.9, "margin_in": 0.006}
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let route_targets = report
        .issues
        .iter()
        .filter(|issue| issue.issue_type == "route_detour")
        .flat_map(|issue| issue.target_ids.iter().cloned())
        .collect::<Vec<_>>();
    assert!(
        route_targets.contains(&"e_input_teacher".to_string()),
        "4-point input-to-teacher dogleg should be reported: {:#?}",
        report.issues
    );
    assert!(
        route_targets.contains(&"e_student_taskloss".to_string()),
        "short student-to-loss connector should not make a 4-point loop: {:#?}",
        report.issues
    );
    assert!(
        report.issues.iter().any(|issue| {
            issue.issue_type == "label_far_from_edge"
                && issue
                    .target_ids
                    .contains(&"e_student_taskloss_label".to_string())
                && issue.target_ids.contains(&"e_student_taskloss".to_string())
        }),
        "floating connector label should be bound to its edge: {:#?}",
        report.issues
    );
    assert!(
        report.issues.iter().any(|issue| {
            issue.issue_type == "component_crowding"
                && issue.target_ids.contains(&"task_input".to_string())
                && issue.target_ids.contains(&"task_loss".to_string())
        }),
        "2.7mm input/loss gutter should be treated as too tight for paper-width figures: {:#?}",
        report.issues
    );
}

#[test]
fn quality_report_flags_smoke_labels_detached_from_actual_polyline() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {
                    "id": "edge_head_loss",
                    "kind": "edge",
                    "bbox": [0.858, 0.73, 0.858, 0.78],
                    "from": "comp_student_head",
                    "to": "comp_task_loss",
                    "points": [[0.858, 0.73], [0.858, 0.78]]
                },
                {
                    "id": "edge_head_loss_label",
                    "kind": "label",
                    "bbox": [0.68, 0.73, 0.84, 0.78],
                    "text": "ŷ",
                    "font_size_pt": 10.9,
                    "margin_in": 0.006
                },
                {
                    "id": "edge_tenc_resid",
                    "kind": "edge",
                    "bbox": [0.5, 0.29375, 0.65, 0.29375],
                    "from": "comp_teacher_encoder",
                    "to": "comp_residual",
                    "points": [[0.5, 0.29375], [0.65, 0.29375]]
                },
                {
                    "id": "edge_tenc_resid_label",
                    "kind": "label",
                    "bbox": [0.309, 0.06675, 0.469, 0.11675],
                    "text": "h_t",
                    "font_size_pt": 10.9,
                    "margin_in": 0.006
                }
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    for (label_id, edge_id) in [
        ("edge_head_loss_label", "edge_head_loss"),
        ("edge_tenc_resid_label", "edge_tenc_resid"),
    ] {
        assert!(
            report.issues.iter().any(|issue| {
                issue.issue_type == "label_far_from_edge"
                    && issue.target_ids.contains(&label_id.to_string())
                    && issue.target_ids.contains(&edge_id.to_string())
            }),
            "{label_id} should be reported as detached from {edge_id}: {:#?}",
            report.issues
        );
    }
}

#[test]
fn quality_report_flags_explicit_fanin_edge_label_far_from_edge_from_latest_smoke() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {
                    "id": "teacher_to_alignment",
                    "kind": "edge",
                    "from": "teacher",
                    "to": "alignment_loss",
                    "points": [[0.1875, 0.21875], [0.1875, 0.4375], [0.43783333333333335, 0.4375]],
                    "bbox": [0.1875, 0.21875, 0.43783333333333335, 0.4375]
                },
                {
                    "id": "student_to_alignment",
                    "kind": "edge",
                    "from": "student",
                    "to": "alignment_loss",
                    "points": [[0.18749999999999997, 0.553], [0.18749999999999997, 0.5025], [0.5098333333333334, 0.5025]],
                    "bbox": [0.18749999999999997, 0.5025, 0.5098333333333334, 0.553]
                },
                {
                    "id": "student_to_alignment_label",
                    "kind": "label",
                    "bbox": [0.11749999999999997, 0.8220000000000001, 0.25749999999999995, 0.8720000000000001],
                    "text": "latent",
                    "font_size_pt": 10.9,
                    "margin_in": 0.006
                }
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let far = report
        .issues
        .iter()
        .find(|issue| issue.issue_type == "label_far_from_edge")
        .expect("explicit label for fan-in edge should still be reported when detached");
    assert_eq!(far.severity, "major");
    assert!(far
        .target_ids
        .contains(&"student_to_alignment_label".to_string()));
    assert!(far.target_ids.contains(&"student_to_alignment".to_string()));
}

#[test]
fn quality_report_flags_excessive_internal_whitespace_from_text_metadata() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {
                    "id": "oversized_short_label",
                    "kind": "component",
                    "bbox": [0.10, 0.20, 0.65, 0.55],
                    "text": "LM",
                    "font_size_pt": 16.0,
                    "margin_in": 0.02
                }
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let whitespace = report
        .issues
        .iter()
        .find(|issue| issue.issue_type == "excessive_internal_whitespace")
        .expect("large short-label box should be flagged");
    assert_eq!(whitespace.severity, "major");
    assert_eq!(
        whitespace.target_ids,
        vec!["oversized_short_label".to_string()]
    );
    assert!(whitespace.evidence.contains("internal whitespace"));
}

#[test]
fn quality_report_flags_training_only_teacher_box_whitespace_from_latest_smoke() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {
                    "id": "teacher_model",
                    "kind": "component",
                    "bbox": [0.02, 0.21125, 0.37098, 0.35125],
                    "text": "Teacher LM\n(large, training-only)",
                    "font_size_pt": 13.1,
                    "margin_in": 0.025
                }
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let whitespace = report
        .issues
        .iter()
        .find(|issue| issue.issue_type == "excessive_internal_whitespace")
        .expect("wide training-only teacher box should be reported as excessive whitespace");
    assert_eq!(whitespace.severity, "major");
    assert_eq!(whitespace.target_ids, vec!["teacher_model".to_string()]);
}

#[test]
fn quality_report_flags_single_word_wrap_risk_from_text_metadata() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {
                    "id": "comp_output",
                    "kind": "component",
                    "bbox": [0.856, 0.69, 0.977, 0.977],
                    "text": "Prediction",
                    "font_size_pt": 13.1,
                    "margin_in": 0.035
                }
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let wrap = report
        .issues
        .iter()
        .find(|issue| issue.issue_type == "text_wrap_risk")
        .expect("single long word should not be squeezed into a narrow box");
    assert_eq!(wrap.severity, "major");
    assert_eq!(wrap.target_ids, vec!["comp_output".to_string()]);
    assert!(wrap.evidence.contains("Prediction"));
}

#[test]
fn quality_report_flags_input_phrase_too_narrow_from_latest_smoke() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {
                    "id": "task_input",
                    "kind": "component",
                    "bbox": [0.02, 0.400125, 0.12, 0.500125],
                    "text": "Task Input x",
                    "font_size_pt": 13.1,
                    "margin_in": 0.018
                }
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let wrap = report
        .issues
        .iter()
        .find(|issue| issue.issue_type == "text_wrap_risk")
        .expect("short input phrase still needs enough horizontal width");
    assert_eq!(wrap.severity, "major");
    assert_eq!(wrap.target_ids, vec!["task_input".to_string()]);
    assert!(wrap.evidence.contains("Task Input x"));
}

#[test]
fn quality_report_flags_small_but_visible_component_collision() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {
                    "id": "answer",
                    "kind": "component",
                    "bbox": [0.3595833333333333, 0.08875, 0.5570833333333334, 0.22375],
                    "text": "Answer distribution",
                    "font_size_pt": 13.1,
                    "margin_in": 0.032
                },
                {
                    "id": "latent_residual",
                    "kind": "component",
                    "bbox": [0.52625, 0.15125, 0.8070833333333334, 0.28625],
                    "text": "Latent residual",
                    "font_size_pt": 13.1,
                    "margin_in": 0.032
                }
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let overlap = report
        .issues
        .iter()
        .find(|issue| issue.issue_type == "component_overlap")
        .expect("millimeter-scale visible box collision must be reported");
    assert_eq!(overlap.severity, "blocking");
    assert!(overlap.target_ids.contains(&"answer".to_string()));
    assert!(overlap.target_ids.contains(&"latent_residual".to_string()));
}

#[test]
fn quality_report_flags_thin_smoke_component_collision() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {
                    "id": "teacher_residual_out",
                    "kind": "component",
                    "bbox": [0.757, 0.5262499999999999, 0.929, 0.6362499999999999],
                    "text": "Latent\nresidual r",
                    "font_size_pt": 13.1,
                    "margin_in": 0.019
                },
                {
                    "id": "task_loss_box",
                    "kind": "component",
                    "bbox": [0.7879999999999999, 0.618, 0.94, 0.728],
                    "text": "Task loss",
                    "font_size_pt": 13.1,
                    "margin_in": 0.019
                }
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let overlap = report
        .issues
        .iter()
        .find(|issue| issue.issue_type == "component_overlap")
        .expect("thin but visible smoke collision must be reported");
    assert_eq!(overlap.severity, "blocking");
    assert!(overlap
        .target_ids
        .contains(&"teacher_residual_out".to_string()));
    assert!(overlap.target_ids.contains(&"task_loss_box".to_string()));
}

#[test]
fn quality_report_flags_edge_crossing_through_unrelated_component() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {"id": "task_input", "kind": "component", "bbox": [0.19, 0.4295833333, 0.31, 0.6329166667]},
                {"id": "student", "kind": "component", "bbox": [0.3613333333, 0.403, 0.5553333333, 0.847]},
                {"id": "teacher", "kind": "component", "bbox": [0.6946666667, 0.278, 0.8886666667, 0.597]},
                {
                    "id": "e_input_teacher",
                    "kind": "edge",
                    "from": "task_input",
                    "to": "teacher",
                    "points": [[0.31, 0.4375], [0.6946666667, 0.4375]],
                    "bbox": [0.31, 0.4375, 0.6946666667, 0.4375]
                }
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let crossing = report
        .issues
        .iter()
        .find(|issue| issue.issue_type == "edge_crosses_component")
        .expect("connector crossing a non-endpoint component must be reported");
    assert_eq!(crossing.severity, "blocking");
    assert!(crossing.target_ids.contains(&"e_input_teacher".to_string()));
    assert!(crossing.target_ids.contains(&"student".to_string()));
    assert!(!crossing.target_ids.contains(&"task_input".to_string()));
    assert!(!crossing.target_ids.contains(&"teacher".to_string()));
}

#[test]
fn quality_report_flags_thin_connector_running_through_annotation() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {
                    "id": "anno_residual",
                    "kind": "annotation",
                    "bbox": [0.5417, 0.3125, 0.75, 0.4125],
                    "text": "residual cue",
                    "font_size_pt": 10.5,
                    "margin_in": 0.02
                },
                {
                    "id": "e_residual_student",
                    "kind": "edge",
                    "points": [[0.5408, 0.28625], [0.5408, 0.403]],
                    "bbox": [0.5408, 0.28625, 0.5408, 0.403]
                }
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let label_overlap = report
        .issues
        .iter()
        .find(|issue| issue.issue_type == "label_overlaps_edge")
        .expect("thin connector through annotation bbox must be reported");
    assert_eq!(label_overlap.severity, "blocking");
    assert!(label_overlap
        .target_ids
        .contains(&"anno_residual".to_string()));
    assert!(label_overlap
        .target_ids
        .contains(&"e_residual_student".to_string()));
}

#[test]
fn quality_report_flags_connector_label_crowding_own_edge_from_latest_smoke() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {
                    "id": "e_teacher_supervision",
                    "kind": "edge",
                    "from": "teacher_model",
                    "to": "latent_residuals",
                    "points": [[0.19549, 0.35125], [0.54098, 0.35125], [0.54098, 0.593]],
                    "bbox": [0.19549, 0.35125, 0.54098, 0.593]
                },
                {
                    "id": "e_teacher_supervision_label",
                    "kind": "label",
                    "bbox": [0.288235, 0.36925, 0.44823499999999994, 0.41925000000000007],
                    "text": "latent residuals",
                    "font_size_pt": 10.9,
                    "margin_in": 0.006
                }
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let label_overlap = report
        .issues
        .iter()
        .find(|issue| issue.issue_type == "label_overlaps_edge")
        .expect("connector label crowding its own edge should be reported");
    assert_eq!(label_overlap.severity, "blocking");
    assert!(label_overlap
        .target_ids
        .contains(&"e_teacher_supervision_label".to_string()));
    assert!(label_overlap
        .target_ids
        .contains(&"e_teacher_supervision".to_string()));
}

#[test]
fn quality_report_flags_three_point_input_student_detour_from_latest_smoke() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {
                    "id": "task_input",
                    "kind": "component",
                    "bbox": [0.02, 0.510625, 0.12000000000000001, 0.645625],
                    "text": "Task Input"
                },
                {
                    "id": "student_model",
                    "kind": "component",
                    "bbox": [0.278, 0.778, 0.6595, 0.972],
                    "text": "Student\n(compact)"
                },
                {
                    "id": "e_input_student",
                    "kind": "edge",
                    "from": "task_input",
                    "to": "student_model",
                    "points": [[0.12000000000000001, 0.578125], [0.12000000000000001, 0.875], [0.278, 0.875]],
                    "bbox": [0.12000000000000001, 0.578125, 0.278, 0.875]
                }
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let detour = report
        .issues
        .iter()
        .find(|issue| issue.issue_type == "route_detour")
        .expect("three-point input-to-student elbow detour should be reported");
    assert_eq!(detour.severity, "major");
    assert!(detour.target_ids.contains(&"e_input_student".to_string()));
    assert!(detour.target_ids.contains(&"task_input".to_string()));
    assert!(detour.target_ids.contains(&"student_model".to_string()));
}

#[test]
fn quality_report_flags_supervision_branch_asymmetry_from_latest_smoke() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {
                    "id": "teacher_model",
                    "kind": "component",
                    "bbox": [0.02, 0.21125, 0.37098, 0.35125],
                    "text": "Teacher LM\\n(large, training-only)"
                },
                {
                    "id": "student_model",
                    "kind": "component",
                    "bbox": [0.278, 0.778, 0.6595, 0.972],
                    "text": "Student\\n(compact)"
                },
                {
                    "id": "latent_residuals",
                    "kind": "component",
                    "bbox": [0.45098, 0.593, 0.63098, 0.7229999999999999],
                    "text": "Latent Residual\nSupervision"
                },
                {
                    "id": "e_teacher_supervision",
                    "kind": "edge",
                    "from": "teacher_model",
                    "to": "latent_residuals",
                    "points": [[0.19549, 0.35125], [0.54098, 0.35125], [0.54098, 0.593]],
                    "bbox": [0.19549, 0.35125, 0.54098, 0.593]
                },
                {
                    "id": "e_supervision_student",
                    "kind": "edge",
                    "from": "latent_residuals",
                    "to": "student_model",
                    "points": [[0.46875, 0.7229999999999999], [0.46875, 0.778]],
                    "bbox": [0.46875, 0.7229999999999999, 0.46875, 0.778]
                }
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let asymmetry = report
        .issues
        .iter()
        .find(|issue| issue.issue_type == "supervision_branch_asymmetry")
        .expect("supervision node crowded toward one branch should be reported");
    assert_eq!(asymmetry.severity, "major");
    for id in [
        "latent_residuals",
        "teacher_model",
        "student_model",
        "e_teacher_supervision",
        "e_supervision_student",
    ] {
        assert!(asymmetry.target_ids.contains(&id.to_string()));
    }
}

#[test]
fn quality_report_flags_annotation_covering_component_text() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {
                    "id": "student_pred",
                    "kind": "component",
                    "bbox": [0.48, 0.52, 0.68, 0.72],
                    "text": "Prediction\nHead",
                    "font_size_pt": 13.1,
                    "margin_in": 0.035
                },
                {
                    "id": "ann_residual_eq",
                    "kind": "annotation",
                    "bbox": [0.375, 0.3875, 0.8333333333333334, 0.8375],
                    "text": "L_res = ||h_S - h_T||",
                    "font_size_pt": 10.9,
                    "margin_in": 0.022
                }
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let overlap = report
        .issues
        .iter()
        .find(|issue| issue.issue_type == "label_overlaps_component")
        .expect("annotation covering component text should be reported");
    assert_eq!(overlap.severity, "blocking");
    assert!(overlap.target_ids.contains(&"ann_residual_eq".to_string()));
    assert!(overlap.target_ids.contains(&"student_pred".to_string()));
}

#[test]
fn quality_report_flags_inference_annotation_in_teacher_student_corridor_from_smoke() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {
                    "id": "comp_teacher",
                    "kind": "component",
                    "bbox": [0.115, 0.5105, 0.355, 0.6455],
                    "text": "Teacher LLM",
                    "font_size_pt": 18.0,
                    "margin_in": 0.02
                },
                {
                    "id": "comp_student",
                    "kind": "component",
                    "bbox": [0.755, 0.5105, 0.995, 0.6455],
                    "text": "Student Model",
                    "font_size_pt": 18.0,
                    "margin_in": 0.02
                },
                {
                    "id": "anno_inference",
                    "kind": "annotation",
                    "bbox": [0.577, 0.4644375, 0.737, 0.5244375],
                    "text": "Inference: student only",
                    "font_size_pt": 10.5,
                    "margin_in": 0.02
                },
                {
                    "id": "edge_teacher_to_student",
                    "kind": "edge",
                    "bbox": [0.355, 0.578, 0.755, 0.578],
                    "points": [[0.355, 0.578], [0.755, 0.578]]
                }
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let corridor = report
        .issues
        .iter()
        .find(|issue| issue.issue_type == "annotation_in_main_corridor")
        .expect("inference annotation in the teacher/student corridor should be reported");
    assert_eq!(corridor.severity, "major");
    assert!(corridor.target_ids.contains(&"anno_inference".to_string()));
    assert!(corridor.target_ids.contains(&"comp_student".to_string()));
}

#[test]
fn quality_report_flags_inference_annotation_between_vertical_teacher_student_branches() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {
                    "id": "teacher_model",
                    "kind": "component",
                    "bbox": [0.36975, 0.12, 0.56775, 0.255],
                    "text": "Teacher",
                    "font_size_pt": 13.1,
                    "margin_in": 0.024
                },
                {
                    "id": "student_model",
                    "kind": "component",
                    "bbox": [0.36975, 0.745, 0.56775, 0.88],
                    "text": "Student",
                    "font_size_pt": 13.1,
                    "margin_in": 0.024
                },
                {
                    "id": "ann_inference",
                    "kind": "annotation",
                    "bbox": [0.29075, 0.5395, 0.45075, 0.5995],
                    "text": "Inference: student only",
                    "font_size_pt": 10.9,
                    "margin_in": 0.008
                },
                {
                    "id": "e_teacher_student",
                    "kind": "edge",
                    "bbox": [0.56775, 0.1875, 0.56775, 0.8125],
                    "points": [[0.56775, 0.1875], [0.56775, 0.8125]],
                    "from": "teacher_model",
                    "to": "student_model"
                }
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let corridor = report
        .issues
        .iter()
        .find(|issue| issue.issue_type == "annotation_in_main_corridor")
        .expect("vertical teacher/student branch gap annotation should be reported");
    assert_eq!(corridor.severity, "major");
    assert!(corridor.target_ids.contains(&"ann_inference".to_string()));
    assert!(corridor.target_ids.contains(&"teacher_model".to_string()));
    assert!(corridor.target_ids.contains(&"student_model".to_string()));
}

#[test]
fn quality_report_allows_inference_annotation_at_right_student_periphery_from_latest_smoke() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {
                    "id": "teacher_encoder",
                    "kind": "component",
                    "bbox": [0.3383333333333333, 0.02, 0.5583333333333333, 0.14],
                    "text": "Teacher Encoder",
                    "font_size_pt": 13.1,
                    "margin_in": 0.021
                },
                {
                    "id": "student_encoder",
                    "kind": "component",
                    "bbox": [0.278, 0.6249999999999999, 0.52, 0.7449999999999999],
                    "text": "Student Encoder",
                    "font_size_pt": 13.1,
                    "margin_in": 0.021
                },
                {
                    "id": "student_head",
                    "kind": "component",
                    "bbox": [0.6, 0.8199999999999998, 0.78, 0.94],
                    "text": "Student Head",
                    "font_size_pt": 13.1,
                    "margin_in": 0.021
                },
                {
                    "id": "answer",
                    "kind": "component",
                    "bbox": [0.845, 0.84, 1.0, 0.96],
                    "text": "Answer ŷ",
                    "font_size_pt": 13.1,
                    "margin_in": 0.021
                },
                {
                    "id": "ann_inference",
                    "kind": "annotation",
                    "bbox": [0.7494999999999999, 0.63, 0.94, 0.71],
                    "text": "Inference: student only",
                    "font_size_pt": 10.9,
                    "margin_in": 0.01
                }
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.issue_type == "annotation_in_main_corridor"),
        "right-side student/output periphery inference annotation should not be treated as the teacher-student corridor: {:?}",
        report.issues
    );
}

#[test]
fn quality_report_flags_connected_residual_signal_box_between_vertical_teacher_student_branches() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {
                    "id": "teacher_model",
                    "kind": "component",
                    "bbox": [0.36975, 0.12, 0.56775, 0.255],
                    "text": "Teacher",
                    "font_size_pt": 13.1,
                    "margin_in": 0.024
                },
                {
                    "id": "student_model",
                    "kind": "component",
                    "bbox": [0.36975, 0.745, 0.56775, 0.88],
                    "text": "Student",
                    "font_size_pt": 13.1,
                    "margin_in": 0.024
                },
                {
                    "id": "latent_residual_signal",
                    "kind": "component",
                    "bbox": [0.5585, 0.5075, 0.6915, 0.6175],
                    "text": "Latent Residual",
                    "font_size_pt": 13.1,
                    "margin_in": 0.019
                },
                {
                    "id": "e_teacher_to_residual",
                    "kind": "edge",
                    "bbox": [0.46875, 0.255, 0.625, 0.5075],
                    "points": [[0.46875, 0.255], [0.46875, 0.5075], [0.625, 0.5075]],
                    "from": "teacher_model",
                    "to": "latent_residual_signal"
                },
                {
                    "id": "e_student_to_residual",
                    "kind": "edge",
                    "bbox": [0.46875, 0.6175, 0.625, 0.745],
                    "points": [[0.46875, 0.745], [0.46875, 0.6175], [0.625, 0.6175]],
                    "from": "student_model",
                    "to": "latent_residual_signal"
                }
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let residual = report
        .issues
        .iter()
        .find(|issue| issue.issue_type == "residual_signal_in_branch_corridor")
        .expect(
            "connected residual signal box in the teacher/student branch gap should be reported",
        );
    assert_eq!(residual.severity, "major");
    assert!(residual
        .target_ids
        .contains(&"latent_residual_signal".to_string()));
    assert!(residual.target_ids.contains(&"teacher_model".to_string()));
    assert!(residual.target_ids.contains(&"student_model".to_string()));
}

#[test]
fn quality_report_flags_standalone_inference_lane_component_from_smoke() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {
                    "id": "comp_teacher_input",
                    "kind": "component",
                    "bbox": [0.05925, 0.153, 0.222, 0.38866666666666666],
                    "text": "Task Input",
                    "font_size_pt": 13.1,
                    "margin_in": 0.025
                },
                {
                    "id": "comp_student_input",
                    "kind": "component",
                    "bbox": [0.05925, 0.7033333333333333, 0.222, 0.8383333333333333],
                    "text": "Task Input",
                    "font_size_pt": 13.1,
                    "margin_in": 0.025
                },
                {
                    "id": "comp_student_encoder",
                    "kind": "component",
                    "bbox": [0.34875, 0.7025, 0.58875, 0.8375],
                    "text": "Student Encoder",
                    "font_size_pt": 13.1,
                    "margin_in": 0.025
                },
                {
                    "id": "comp_inference_note",
                    "kind": "component",
                    "bbox": [0.05562500000000001, 0.5633333333333331, 0.225625, 0.6633333333333332],
                    "text": "Inference Only",
                    "font_size_pt": 13.1,
                    "margin_in": 0.025
                },
                {
                    "id": "edge_student_flow",
                    "kind": "edge",
                    "from": "comp_student_input",
                    "to": "comp_student_encoder",
                    "points": [[0.222, 0.77], [0.34875, 0.77]],
                    "bbox": [0.222, 0.77, 0.34875, 0.77]
                }
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let standalone = report
        .issues
        .iter()
        .find(|issue| issue.issue_type == "standalone_inference_lane")
        .expect("unconnected Inference Only component should be reported");
    assert_eq!(standalone.severity, "major");
    assert!(standalone
        .target_ids
        .contains(&"comp_inference_note".to_string()));
}

#[test]
fn quality_report_allows_compact_student_only_inference_note_badge() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {
                    "id": "student",
                    "kind": "component",
                    "bbox": [0.36133333333333334, 0.528, 0.8886666666666666, 0.972],
                    "text": "Student\n(compact)",
                    "font_size_pt": 13.1,
                    "margin_in": 0.035
                },
                {
                    "id": "inference_note",
                    "kind": "component",
                    "bbox": [0.11508333333333332, 0.69375, 0.32133333333333336, 0.80625],
                    "text": "Inference: student only",
                    "font_size_pt": 13.1,
                    "margin_in": 0.02
                }
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.issue_type == "standalone_inference_lane"),
        "a compact note/badge that explicitly says student-only inference should not be treated as a routed standalone inference lane"
    );
}

#[test]
fn quality_report_flags_task_loss_in_teacher_student_branch_corridor_from_latest_smoke() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {"id": "comp_student", "kind": "component", "bbox": [0.291625, 0.5575, 0.489625, 0.6925], "text": "Student", "font_size_pt": 13.1, "margin_in": 0.025},
                {"id": "comp_task_loss", "kind": "component", "bbox": [0.41875, 0.3925, 0.51875, 0.5025], "text": "Task Loss", "font_size_pt": 13.1, "margin_in": 0.025},
                {"id": "comp_teacher", "kind": "component", "bbox": [0.666625, 0.22416666666666663, 0.864625, 0.35916666666666663], "text": "Teacher", "font_size_pt": 13.1, "margin_in": 0.025},
                {"id": "edge_student_to_task_loss", "kind": "edge", "from": "comp_student", "to": "comp_task_loss", "points": [[0.46875, 0.5575], [0.46875, 0.5025]], "bbox": [0.46875, 0.5025, 0.46875, 0.5575]}
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let corridor = report
        .issues
        .iter()
        .find(|issue| issue.issue_type == "task_loss_in_branch_corridor")
        .expect("task loss occupying the teacher/student branch corridor should be reported");
    assert_eq!(corridor.severity, "major");
    assert!(corridor.target_ids.contains(&"comp_task_loss".to_string()));
    assert!(corridor.target_ids.contains(&"comp_student".to_string()));
    assert!(corridor.target_ids.contains(&"comp_teacher".to_string()));
}

#[test]
fn quality_report_flags_unanchored_compact_inference_note_from_latest_smoke() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {"id": "comp_student", "kind": "component", "bbox": [0.291625, 0.5575, 0.489625, 0.6925], "text": "Student", "font_size_pt": 13.1, "margin_in": 0.025},
                {"id": "comp_output", "kind": "component", "bbox": [0.23625, 0.8875, 0.545, 0.9875], "text": "Prediction ŷ", "font_size_pt": 13.1, "margin_in": 0.025},
                {"id": "comp_latent_residual", "kind": "component", "bbox": [0.665625, 0.57, 0.865625, 0.68], "text": "Latent Residual", "font_size_pt": 13.1, "margin_in": 0.025},
                {"id": "comp_inference_note", "kind": "component", "bbox": [0.77625, 0.8091666666666666, 0.95625, 0.9408333333333334], "text": "Inference: student only", "font_size_pt": 13.1, "margin_in": 0.02}
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let unanchored = report
        .issues
        .iter()
        .find(|issue| issue.issue_type == "inference_note_unanchored")
        .expect("compact inference note far from student/output should be reported");
    assert_eq!(unanchored.severity, "major");
    assert!(unanchored
        .target_ids
        .contains(&"comp_inference_note".to_string()));
}

#[test]
fn quality_report_flags_long_three_point_input_teacher_branch_detour_from_latest_smoke() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {"id": "comp_input", "kind": "component", "bbox": [0.021875, 0.5575, 0.134375, 0.6925], "text": "Input x", "font_size_pt": 13.1, "margin_in": 0.025},
                {"id": "comp_teacher", "kind": "component", "bbox": [0.666625, 0.22416666666666663, 0.864625, 0.35916666666666663], "text": "Teacher", "font_size_pt": 13.1, "margin_in": 0.025},
                {
                    "id": "edge_input_to_teacher",
                    "kind": "edge",
                    "from": "comp_input",
                    "to": "comp_teacher",
                    "points": [[0.134375, 0.625], [0.134375, 0.29166666666666663], [0.666625, 0.29166666666666663]],
                    "bbox": [0.134375, 0.29166666666666663, 0.666625, 0.625]
                }
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let detour = report
        .issues
        .iter()
        .find(|issue| {
            issue.issue_type == "route_detour"
                && issue
                    .target_ids
                    .contains(&"edge_input_to_teacher".to_string())
        })
        .expect("long three-point input-to-teacher branch route should be reported");
    assert_eq!(detour.severity, "major");
    assert!(detour.target_ids.contains(&"comp_input".to_string()));
    assert!(detour.target_ids.contains(&"comp_teacher".to_string()));
}

#[test]
fn quality_report_flags_long_three_point_input_student_detour_from_latest_smoke() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {"id": "comp_input", "kind": "component", "bbox": [0.02, 0.55, 0.123, 0.685], "text": "Task Input", "font_size_pt": 13.1, "margin_in": 0.024},
                {"id": "comp_student", "kind": "component", "bbox": [0.457, 0.22, 0.7, 0.42], "text": "Student\n(compact)", "font_size_pt": 13.1, "margin_in": 0.035},
                {
                    "id": "edge_input_to_student",
                    "kind": "edge",
                    "from": "comp_input",
                    "to": "comp_student",
                    "points": [[0.123, 0.6175], [0.123, 0.32], [0.457, 0.32]],
                    "bbox": [0.123, 0.32, 0.457, 0.6175]
                }
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let detour = report
        .issues
        .iter()
        .find(|issue| {
            issue.issue_type == "route_detour"
                && issue
                    .target_ids
                    .contains(&"edge_input_to_student".to_string())
        })
        .expect("long three-point input-to-student route should be reported");
    assert_eq!(detour.severity, "major");
    assert!(detour.target_ids.contains(&"comp_input".to_string()));
    assert!(detour.target_ids.contains(&"comp_student".to_string()));
}

#[test]
fn quality_report_flags_outer_margin_input_student_detour_from_latest_smoke() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {"id": "task_input", "kind": "component", "bbox": [0.065, 0.881, 0.185, 0.981], "text": "Task Input", "font_size_pt": 13.1, "margin_in": 0.018},
                {"id": "student", "kind": "component", "bbox": [0.55, 0.25, 0.85, 0.45], "text": "Student\n(trainable)", "font_size_pt": 13.1, "margin_in": 0.035},
                {
                    "id": "e_input_to_student",
                    "kind": "edge",
                    "from": "task_input",
                    "to": "student",
                    "points": [[0.185, 0.931], [0.68, 0.931], [0.68, 0.45], [0.7, 0.45]],
                    "bbox": [0.185, 0.45, 0.7, 0.931]
                }
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let detour = report
        .issues
        .iter()
        .find(|issue| {
            issue.issue_type == "route_detour"
                && issue.target_ids.contains(&"e_input_to_student".to_string())
        })
        .expect("outer-margin input-to-student L route should be reported");
    assert_eq!(detour.severity, "major");
    assert!(detour.target_ids.contains(&"task_input".to_string()));
    assert!(detour.target_ids.contains(&"student".to_string()));
}

#[test]
fn quality_report_flags_outer_margin_student_output_detour_from_latest_smoke() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {"id": "student", "kind": "component", "bbox": [0.55, 0.25, 0.85, 0.45], "text": "Student\n(trainable)", "font_size_pt": 13.1, "margin_in": 0.035},
                {"id": "task_output", "kind": "component", "bbox": [0.86, 0.881, 0.985, 0.981], "text": "Task Output", "font_size_pt": 13.1, "margin_in": 0.018},
                {
                    "id": "e_student_to_output",
                    "kind": "edge",
                    "from": "student",
                    "to": "task_output",
                    "points": [[0.85, 0.35], [0.85, 0.886], [0.925, 0.886]],
                    "bbox": [0.85, 0.35, 0.925, 0.886]
                }
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let detour = report
        .issues
        .iter()
        .find(|issue| {
            issue.issue_type == "route_detour"
                && issue
                    .target_ids
                    .contains(&"e_student_to_output".to_string())
        })
        .expect("outer-margin student-to-output U route should be reported");
    assert_eq!(detour.severity, "major");
    assert!(detour.target_ids.contains(&"student".to_string()));
    assert!(detour.target_ids.contains(&"task_output".to_string()));
}

#[test]
fn quality_report_flags_crowded_student_branch_and_shallow_u_route_from_current_smoke() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {"id": "student_encoder", "kind": "component", "bbox": [0.2404, 0.5996, 0.4204, 0.7171], "text": "Student Encoder", "font_size_pt": 13.1, "margin_in": 0.018},
                {"id": "student_latent", "kind": "component", "bbox": [0.4112, 0.5996, 0.5112, 0.7279], "text": "z_s", "font_size_pt": 13.1, "margin_in": 0.018},
                {"id": "student_head", "kind": "component", "bbox": [0.2804, 0.7721, 0.3804, 0.9004], "text": "Task Head", "font_size_pt": 13.1, "margin_in": 0.018},
                {"id": "task_pred", "kind": "component", "bbox": [0.8458, 0.7600, 0.9458, 0.9125], "text": "ŷ", "font_size_pt": 13.1, "margin_in": 0.027},
                {
                    "id": "e_student_latent",
                    "kind": "edge",
                    "from": "student_encoder",
                    "to": "student_latent",
                    "points": [[0.4204, 0.6584], [0.4612, 0.6638]],
                    "bbox": [0.4204, 0.6584, 0.4612, 0.6638]
                },
                {
                    "id": "e_student_head",
                    "kind": "edge",
                    "from": "student_latent",
                    "to": "student_head",
                    "points": [[0.4612, 0.7279], [0.4612, 0.7721], [0.3304, 0.7721]],
                    "bbox": [0.3304, 0.7279, 0.4612, 0.7721]
                },
                {
                    "id": "e_student_pred",
                    "kind": "edge",
                    "from": "student_head",
                    "to": "task_pred",
                    "points": [[0.3804, 0.8362], [0.3804, 0.8613], [0.8458, 0.8613], [0.8458, 0.8362]],
                    "bbox": [0.3804, 0.8362, 0.8458, 0.8613]
                }
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.issue_type == "component_overlap"
                && issue.target_ids.contains(&"student_encoder".to_string())
                && issue.target_ids.contains(&"student_latent".to_string())),
        "thin but visible overlap between sequential student modules should not be scored as clean: {:#?}",
        report.issues
    );
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.issue_type == "route_detour"
                && issue.target_ids.contains(&"e_student_pred".to_string())),
        "shallow U route from task head to prediction should be reported: {:#?}",
        report.issues
    );
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.issue_type == "oversized_tiny_output"
                && issue.target_ids.contains(&"task_pred".to_string())),
        "single-character prediction box should be tightened instead of wasting the right lane: {:#?}",
        report.issues
    );
    assert!(!report.passed);
}

#[test]
fn quality_report_flags_far_student_task_loss_route_from_latest_smoke() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {"id": "teacher_enc", "kind": "component", "bbox": [0.30725, 0.19666666666666663, 0.50525, 0.37666666666666665], "text": "Teacher\nEncoder", "font_size_pt": 13.1, "margin_in": 0.032},
                {"id": "student_enc", "kind": "component", "bbox": [0.30725, 0.49833333333333335, 0.50525, 0.8583333333333333], "text": "Student\nEncoder", "font_size_pt": 13.1, "margin_in": 0.035},
                {"id": "latent_residual", "kind": "component", "bbox": [0.5702499999999999, 0.35500000000000004, 0.7032499999999999, 0.455], "text": "Latent Residual", "font_size_pt": 13.1, "margin_in": 0.018},
                {"id": "task_loss", "kind": "component", "bbox": [0.865, 0.345, 0.9791666666666666, 0.465], "text": "Task Loss", "font_size_pt": 13.1, "margin_in": 0.021},
                {"id": "output_pred", "kind": "component", "bbox": [0.7991666666666666, 0.58, 0.9791666666666666, 0.68], "text": "Prediction ŷ", "font_size_pt": 13.1, "margin_in": 0.018},
                {
                    "id": "e_student_task",
                    "kind": "edge",
                    "from": "student_enc",
                    "to": "task_loss",
                    "points": [[0.40625, 0.49833333333333335], [0.40625, 0.465], [0.9220833333333334, 0.465]],
                    "bbox": [0.40625, 0.465, 0.9220833333333334, 0.49833333333333335]
                }
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let far_loss = report
        .issues
        .iter()
        .find(|issue| issue.issue_type == "task_loss_far_from_student_source")
        .expect("student-to-task-loss long horizontal route should be reported");
    assert_eq!(far_loss.severity, "major");
    assert!(far_loss.target_ids.contains(&"e_student_task".to_string()));
    assert!(far_loss.target_ids.contains(&"student_enc".to_string()));
    assert!(far_loss.target_ids.contains(&"task_loss".to_string()));
    assert!(!report.passed);
}

#[test]
fn quality_report_flags_task_loss_reverse_flow_from_right_edge_smoke() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {"id": "student_head", "kind": "component", "bbox": [0.6, 0.8199999999999998, 0.78, 0.94], "text": "Student Head", "font_size_pt": 13.1, "margin_in": 0.021},
                {"id": "task_loss", "kind": "component", "bbox": [0.415, 0.83, 0.555, 0.93], "text": "Task Loss", "font_size_pt": 13.1, "margin_in": 0.021},
                {"id": "e_task_loss", "kind": "edge", "from": "student_head", "to": "task_loss", "bbox": [0.555, 0.88, 0.6, 0.88], "points": [[0.6, 0.88], [0.555, 0.88]]}
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let reverse = report
        .issues
        .iter()
        .find(|issue| issue.issue_type == "task_loss_reverse_flow")
        .expect("student-to-task-loss edge flowing leftward should be reported");
    assert_eq!(reverse.severity, "major");
    assert!(reverse.target_ids.contains(&"e_task_loss".to_string()));
    assert!(reverse.target_ids.contains(&"student_head".to_string()));
    assert!(reverse.target_ids.contains(&"task_loss".to_string()));
    assert!(!report.passed);
}

#[test]
fn quality_report_flags_floating_annotation_and_tall_output_from_current_smoke() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {"id": "teacher_model", "kind": "component", "bbox": [0.2378, 0.16, 0.4498, 0.34], "text": "Teacher\n(frozen)", "font_size_pt": 13.1, "margin_in": 0.032},
                {"id": "student_model", "kind": "component", "bbox": [0.2238, 0.66, 0.4638, 0.84], "text": "Student\n(trainable)", "font_size_pt": 13.1, "margin_in": 0.032},
                {"id": "task_output", "kind": "component", "bbox": [0.8351, 0.4018, 0.9461, 0.6755], "text": "Task\nOutput ŷ", "font_size_pt": 13.1, "margin_in": 0.035},
                {"id": "student_to_output", "kind": "edge", "from": "student_model", "to": "task_output", "points": [[0.4638, 0.6678], [0.8351, 0.6678]], "bbox": [0.4638, 0.6678, 0.8351, 0.6678]},
                {"id": "teacher_frozen_note", "kind": "annotation", "bbox": [0.28125, 0.025, 0.65625, 0.125], "text": "inference excluded", "font_size_pt": 10.9, "margin_in": 0.013}
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    assert!(
        report.issues.iter().any(
            |issue| issue.issue_type == "annotation_excessive_whitespace"
                && issue
                    .target_ids
                    .contains(&"teacher_frozen_note".to_string())
        ),
        "large floating annotation should be a local issue: {:#?}",
        report.issues
    );
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.issue_type == "output_excessive_whitespace"
                && issue.target_ids.contains(&"task_output".to_string())),
        "tall output box with short text should be a local issue: {:#?}",
        report.issues
    );
    assert!(!report.passed);
}

#[test]
fn quality_report_flags_annotation_too_close_to_flow_edge_from_latest_smoke() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {
                    "id": "edge_input_to_student",
                    "kind": "edge",
                    "from": "comp_input",
                    "to": "comp_student",
                    "points": [[0.123, 0.6175], [0.123, 0.32], [0.457, 0.32]],
                    "bbox": [0.123, 0.32, 0.457, 0.6175]
                },
                {
                    "id": "anno_residual_dashed",
                    "kind": "annotation",
                    "bbox": [0.21, 0.338, 0.37, 0.408],
                    "text": "Auxiliary training signal",
                    "font_size_pt": 10.9,
                    "margin_in": 0.009
                }
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let near_edge = report
        .issues
        .iter()
        .find(|issue| issue.issue_type == "annotation_too_close_to_edge")
        .expect("annotation crowding a flow edge should be reported");
    assert_eq!(near_edge.severity, "major");
    assert!(near_edge
        .target_ids
        .contains(&"anno_residual_dashed".to_string()));
    assert!(near_edge
        .target_ids
        .contains(&"edge_input_to_student".to_string()));
}

#[test]
fn quality_report_flags_oversized_compact_inference_note_from_latest_smoke() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {
                    "id": "comp_inference_note",
                    "kind": "component",
                    "bbox": [0.4885, 0.47, 0.6685, 0.57],
                    "text": "Inference: student only",
                    "font_size_pt": 13.1,
                    "margin_in": 0.018
                }
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let whitespace = report
        .issues
        .iter()
        .find(|issue| issue.issue_type == "inference_note_excessive_whitespace")
        .expect("oversized compact inference note should be reported");
    assert_eq!(whitespace.severity, "major");
    assert!(whitespace
        .target_ids
        .contains(&"comp_inference_note".to_string()));
}

#[test]
fn quality_report_allows_wide_compact_horizontal_flow() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {"id": "teacher", "kind": "component", "bbox": [0.10, 0.39, 0.34, 0.59], "text": "Teacher LM", "font_size_pt": 13.1, "margin_in": 0.035},
                {"id": "student", "kind": "component", "bbox": [0.38, 0.39, 0.62, 0.59], "text": "Student LM", "font_size_pt": 13.1, "margin_in": 0.035},
                {"id": "output", "kind": "component", "bbox": [0.66, 0.39, 0.90, 0.59], "text": "Prediction", "font_size_pt": 13.1, "margin_in": 0.035}
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.issue_type == "under_utilized"),
        "a wide one-row method flow should not be rejected only because it is vertically compact"
    );
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.issue_type == "vertical_under_utilization"),
        "a vertically centered one-row flow should not be rejected for top-heavy whitespace"
    );
}

#[test]
fn quality_report_flags_top_heavy_vertical_underutilization_from_guard_smoke() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {"id": "task_input", "kind": "component", "bbox": [0.405, 0.175, 0.505, 0.325], "text": "Task Input", "font_size_pt": 13.1, "margin_in": 0.026},
                {"id": "teacher_model", "kind": "component", "bbox": [0.10, 0.15, 0.35, 0.35], "text": "Teacher\\n(Large LM)", "font_size_pt": 13.1, "margin_in": 0.035},
                {"id": "student_model", "kind": "component", "bbox": [0.65, 0.15, 0.88, 0.35], "text": "Student\\n(Compact)", "font_size_pt": 13.1, "margin_in": 0.035},
                {"id": "latent_residual", "kind": "component", "bbox": [0.40, 0.43, 0.60, 0.53], "text": "Latent Residual", "font_size_pt": 13.1, "margin_in": 0.018},
                {"id": "task_loss", "kind": "component", "bbox": [0.445, 0.04, 0.605, 0.14], "text": "Task Loss", "font_size_pt": 13.1, "margin_in": 0.018},
                {"id": "student_output", "kind": "component", "bbox": [0.765, 0.40, 0.893, 0.50], "text": "Answer", "font_size_pt": 13.1, "margin_in": 0.018},
                {"id": "inference_note", "kind": "component", "bbox": [0.749, 0.555, 0.909, 0.625], "text": "Inference: student only", "font_size_pt": 13.1, "margin_in": 0.018}
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let issue = report
        .issues
        .iter()
        .find(|issue| issue.issue_type == "vertical_under_utilization")
        .expect("top-heavy figure with large bottom whitespace should be flagged");
    assert_eq!(issue.severity, "major");
    assert!(issue.target_ids.contains(&"task_input".to_string()));
    assert!(issue.target_ids.contains(&"student_model".to_string()));
    assert!(issue.evidence.contains("bottom whitespace"));
}

#[test]
fn quality_report_flags_teacher_student_topology_inversion_from_latest_smoke() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {"id": "task_input", "kind": "component", "bbox": [0.02, 0.3983333333, 0.12, 0.5183333333], "text": "Task Input", "font_size_pt": 13.1, "margin_in": 0.018},
                {"id": "student_enc", "kind": "component", "bbox": [0.153, 0.2363333333, 0.4095, 0.3886666667], "text": "Student\nEncoder", "font_size_pt": 13.1, "margin_in": 0.018},
                {"id": "student_head", "kind": "component", "bbox": [0.18225, 0.4636666667, 0.38025, 0.616], "text": "Student\nHead", "font_size_pt": 13.1, "margin_in": 0.018},
                {"id": "teacher_enc", "kind": "component", "bbox": [0.5905, 0.528, 0.847, 0.6803333333], "text": "Teacher\nEncoder", "font_size_pt": 13.1, "margin_in": 0.018},
                {"id": "teacher_head", "kind": "component", "bbox": [0.61975, 0.2363333333, 0.81775, 0.3886666667], "text": "Teacher\nHead", "font_size_pt": 13.1, "margin_in": 0.018},
                {"id": "task_loss", "kind": "component", "bbox": [0.20125, 0.681, 0.36125, 0.781], "text": "Task Loss", "font_size_pt": 13.1, "margin_in": 0.018},
                {"id": "latent_residual_loss", "kind": "component", "bbox": [0.385, 0.0541666667, 0.615, 0.1541666667], "text": "Latent Residual", "font_size_pt": 13.1, "margin_in": 0.018},
                {"id": "inference_note", "kind": "component", "bbox": [0.43025, 0.6935, 0.59025, 0.7635], "text": "Inference:\nstudent only", "font_size_pt": 13.1, "margin_in": 0.018},
                {"id": "a_student_label", "kind": "annotation", "bbox": [0.15125, 0.793, 0.41125, 0.8596666667], "text": "Student (operational)", "font_size_pt": 10.0, "margin_in": 0.01},
                {"id": "a_teacher_label", "kind": "annotation", "bbox": [0.38025, 0.788, 0.64025, 0.8546666667], "text": "Teacher (training-only)", "font_size_pt": 10.0, "margin_in": 0.01},
                {"id": "e_input_to_student", "kind": "edge", "from": "task_input", "to": "student_enc", "bbox": [0.12, 0.3125, 0.153, 0.4583], "points": [[0.12, 0.4583], [0.153, 0.4583], [0.153, 0.3125]]},
                {"id": "e_student_enc_to_head", "kind": "edge", "from": "student_enc", "to": "student_head", "bbox": [0.28125, 0.3887, 0.28125, 0.4637], "points": [[0.28125, 0.3887], [0.28125, 0.4637]]},
                {"id": "e_input_to_teacher", "kind": "edge", "from": "task_input", "to": "teacher_enc", "bbox": [0.12, 0.4583, 0.5905, 0.641], "points": [[0.12, 0.4583], [0.12, 0.641], [0.5905, 0.641]]},
                {"id": "e_teacher_enc_to_head", "kind": "edge", "from": "teacher_enc", "to": "teacher_head", "bbox": [0.71875, 0.3887, 0.71875, 0.528], "points": [[0.71875, 0.528], [0.71875, 0.3887]]},
                {"id": "e_student_to_task_loss", "kind": "edge", "from": "student_head", "to": "task_loss", "bbox": [0.28125, 0.616, 0.28125, 0.681], "points": [[0.28125, 0.616], [0.28125, 0.681]]},
                {"id": "e_teacher_to_latent_loss", "kind": "edge", "from": "teacher_head", "to": "latent_residual_loss", "bbox": [0.5, 0.1542, 0.71875, 0.2363], "points": [[0.71875, 0.2363], [0.5, 0.2363], [0.5, 0.1542]]},
                {"id": "e_student_to_latent_loss", "kind": "edge", "from": "student_head", "to": "latent_residual_loss", "bbox": [0.28125, 0.1542, 0.5, 0.4637], "points": [[0.28125, 0.4637], [0.5, 0.4637], [0.5, 0.1542]]}
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let inversion = report
        .issues
        .iter()
        .find(|issue| issue.issue_type == "teacher_student_branch_inversion")
        .expect("teacher below student should be reported as a topology inversion");
    assert_eq!(inversion.severity, "blocking");
    assert!(inversion.target_ids.contains(&"teacher_enc".to_string()));
    assert!(inversion.target_ids.contains(&"student_enc".to_string()));

    let reverse_flow = report
        .issues
        .iter()
        .find(|issue| issue.issue_type == "teacher_internal_flow_reversed")
        .expect("teacher encoder-to-head upward edge should be reported");
    assert_eq!(reverse_flow.severity, "blocking");
    assert!(reverse_flow
        .target_ids
        .contains(&"e_teacher_enc_to_head".to_string()));
    assert!(reverse_flow.target_ids.contains(&"teacher_enc".to_string()));
    assert!(reverse_flow
        .target_ids
        .contains(&"teacher_head".to_string()));
}

#[test]
fn quality_report_allows_teacher_above_student_encoder_topology() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {"id": "task_input", "kind": "component", "bbox": [0.06, 0.40, 0.18, 0.52], "text": "Task Input", "font_size_pt": 13.1, "margin_in": 0.018},
                {"id": "teacher_enc", "kind": "component", "bbox": [0.26, 0.18, 0.46, 0.32], "text": "Teacher Encoder", "font_size_pt": 13.1, "margin_in": 0.018},
                {"id": "teacher_head", "kind": "component", "bbox": [0.54, 0.18, 0.72, 0.32], "text": "Teacher Head", "font_size_pt": 13.1, "margin_in": 0.018},
                {"id": "student_enc", "kind": "component", "bbox": [0.26, 0.58, 0.46, 0.72], "text": "Student Encoder", "font_size_pt": 13.1, "margin_in": 0.018},
                {"id": "student_head", "kind": "component", "bbox": [0.54, 0.58, 0.72, 0.72], "text": "Student Head", "font_size_pt": 13.1, "margin_in": 0.018},
                {"id": "e_teacher_enc_to_head", "kind": "edge", "from": "teacher_enc", "to": "teacher_head", "bbox": [0.46, 0.25, 0.54, 0.25], "points": [[0.46, 0.25], [0.54, 0.25]]},
                {"id": "e_student_enc_to_head", "kind": "edge", "from": "student_enc", "to": "student_head", "bbox": [0.46, 0.65, 0.54, 0.65], "points": [[0.46, 0.65], [0.54, 0.65]]}
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.issue_type == "teacher_student_branch_inversion"),
        "teacher branch above student branch is the intended topology"
    );
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.issue_type == "teacher_internal_flow_reversed"),
        "left-to-right teacher encoder-to-head flow should not be rejected"
    );
}

#[test]
fn quality_report_flags_task_loss_label_on_prediction_edge_from_latest_smoke() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {"id": "comp_student", "kind": "component", "bbox": [0.3661666667, 0.69125, 0.5921666667, 0.87125], "text": "Student\n(Compact)", "font_size_pt": 13.1, "margin_in": 0.032},
                {"id": "comp_student_out", "kind": "component", "bbox": [0.85, 0.73125, 0.95, 0.83125], "text": "ŷ", "font_size_pt": 13.1, "margin_in": 0.018},
                {"id": "e_student_to_out", "kind": "edge", "bbox": [0.5921666667, 0.78125, 0.85, 0.78125], "from": "comp_student", "to": "comp_student_out", "points": [[0.5921666667, 0.78125], [0.85, 0.78125]]},
                {"id": "e_student_to_out_label", "kind": "label", "bbox": [0.6410833333, 0.67625, 0.8010833333, 0.72625], "text": "Task loss", "font_size_pt": 10.9, "margin_in": 0.006}
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let issue = report
        .issues
        .iter()
        .find(|issue| issue.issue_type == "loss_label_on_prediction_edge")
        .expect("task-loss label on a student-to-output prediction edge should be reported");
    assert_eq!(issue.severity, "major");
    assert!(issue
        .target_ids
        .contains(&"e_student_to_out_label".to_string()));
    assert!(issue.target_ids.contains(&"e_student_to_out".to_string()));
}

#[test]
fn quality_report_flags_bottom_margin_inference_annotation_from_latest_smoke() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {"id": "comp_student", "kind": "component", "bbox": [0.3661666667, 0.69125, 0.5921666667, 0.87125], "text": "Student\n(Compact)", "font_size_pt": 13.1, "margin_in": 0.032},
                {"id": "comp_student_out", "kind": "component", "bbox": [0.85, 0.73125, 0.95, 0.83125], "text": "ŷ", "font_size_pt": 13.1, "margin_in": 0.018},
                {"id": "ann_inference", "kind": "annotation", "bbox": [0.3839166667, 0.915, 0.5744166667, 0.98], "text": "Inference: student only", "font_size_pt": 10.9, "margin_in": 0.008}
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let issue = report
        .issues
        .iter()
        .find(|issue| issue.issue_type == "inference_annotation_in_bottom_margin")
        .expect("inference annotation pushed into the bottom margin should be reported");
    assert_eq!(issue.severity, "major");
    assert!(issue.target_ids.contains(&"ann_inference".to_string()));
    assert!(issue.target_ids.contains(&"comp_student".to_string()));
}

#[test]
fn quality_report_flags_input_to_student_rectangular_detour_from_guard_smoke() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {"id": "input_text", "kind": "component", "bbox": [0.035, 0.47875, 0.16, 0.57875], "text": "Task Input x", "font_size_pt": 13.1, "margin_in": 0.018},
                {"id": "student_model", "kind": "component", "bbox": [0.63, 0.46375, 0.87, 0.59875], "text": "Student (compact)", "font_size_pt": 13.1, "margin_in": 0.018},
                {"id": "residual_supervision", "kind": "component", "bbox": [0.4, 0.15, 0.6, 0.25], "text": "Latent Residual", "font_size_pt": 13.1, "margin_in": 0.018},
                {"id": "e_input_student", "kind": "edge", "from": "input_text", "to": "student_model", "bbox": [0.16, 0.52875, 0.63, 0.61375], "points": [[0.16, 0.52875], [0.16, 0.61375], [0.63, 0.61375], [0.63, 0.53125]]},
                {"id": "e_residual_student", "kind": "edge", "from": "residual_supervision", "to": "student_model", "bbox": [0.5, 0.25, 0.63, 0.49375], "points": [[0.5, 0.25], [0.63, 0.49375]]}
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let issue = report
        .issues
        .iter()
        .find(|issue| {
            issue.issue_type == "route_detour"
                && issue.target_ids.contains(&"e_input_student".to_string())
        })
        .expect("input-to-student rectangular branch detour should be reported");
    assert_eq!(issue.severity, "major");
}

#[test]
fn quality_report_flags_residual_student_wandering_route_from_guard_smoke() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {"id": "residual_supervision", "kind": "component", "bbox": [0.4, 0.15, 0.6, 0.25], "text": "Latent Residual", "font_size_pt": 13.1, "margin_in": 0.018},
                {"id": "student_model", "kind": "component", "bbox": [0.63, 0.46375, 0.87, 0.59875], "text": "Student (compact)", "font_size_pt": 13.1, "margin_in": 0.018},
                {"id": "input_text", "kind": "component", "bbox": [0.035, 0.47875, 0.16, 0.57875], "text": "Task Input x", "font_size_pt": 13.1, "margin_in": 0.018},
                {"id": "e_input_student", "kind": "edge", "from": "input_text", "to": "student_model", "bbox": [0.16, 0.52875, 0.63, 0.52875], "points": [[0.16, 0.52875], [0.63, 0.52875]]},
                {"id": "e_residual_student", "kind": "edge", "from": "residual_supervision", "to": "student_model", "bbox": [0.5, 0.25, 0.63, 0.49375], "points": [[0.5, 0.25], [0.5, 0.315], [0.63, 0.315], [0.63, 0.49375]]}
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let issue = report
        .issues
        .iter()
        .find(|issue| {
            issue.issue_type == "route_detour"
                && issue.target_ids.contains(&"e_residual_student".to_string())
        })
        .expect("residual-to-student wandering supervision route should be reported");
    assert_eq!(issue.severity, "major");
}

#[test]
fn quality_report_flags_prediction_box_mixing_task_loss_from_guard_smoke() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": [
                {"id": "student_predict_out", "kind": "component", "bbox": [0.66, 0.29, 0.84, 0.39], "text": "ŷ + Task Loss", "font_size_pt": 13.1, "margin_in": 0.018}
            ]
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    let issue = report
        .issues
        .iter()
        .find(|issue| issue.issue_type == "prediction_loss_semantic_mix")
        .expect("prediction output mixed with task-loss semantics should be reported");
    assert_eq!(issue.severity, "major");
    assert!(issue
        .target_ids
        .contains(&"student_predict_out".to_string()));
}

#[test]
fn quality_score_keeps_granular_signal_for_many_major_issues() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout_map = temp.path().join("layout_map.json");
    let objects = (0..7)
        .map(|index| {
            let y1 = 0.05 + index as f64 * 0.125;
            serde_json::json!({
                "id": format!("box_{index}"),
                "kind": "component",
                "bbox": [0.05, y1, 0.95, y1 + 0.10],
                "text": "x",
                "font_size_pt": 4,
                "margin_in": 0.02
            })
        })
        .collect::<Vec<_>>();
    std::fs::write(
        &layout_map,
        serde_json::json!({
            "canvas": {"width": 7.1, "height": 3.2, "aspect": "paper-wide", "target_width_mm": 85},
            "objects": objects
        })
        .to_string(),
    )
    .expect("layout map");

    let report = build_quality_report(&layout_map).expect("quality report should build");

    assert!(!report.passed);
    assert!(
        report.issues.len() >= 7,
        "test layout should produce many severe quality issues"
    );
    assert!(
        report.score > 0,
        "quality score must preserve ordering signal even before the figure is acceptable"
    );
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
