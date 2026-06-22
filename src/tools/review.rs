use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::schema::{
    FigurePlan, IssueSeverity, LocalizedIssue, PatchExecutor, PatchOperation, PatchOperationType,
    PatchPlan, PatchStopReason, RejectedAsset, Review, ReviewScores,
};

pub fn review_passes_threshold(review: &Review) -> bool {
    review.blocking_issues.is_empty()
        && review.scores.color_semantics >= 8
        && review.scores.semantic_fidelity >= 8
        && review.scores.story_clarity >= 8
        && review.scores.visual_hierarchy >= 8
        && review.scores.paper_readability >= 8
        && review.scores.layout_cleanliness >= 8
        && review.scores.arrow_routing >= 8
        && review.scores.aesthetic_quality >= 8
        && review.scores.wps_editability >= 9
}

pub fn sanitize_review_false_positives(review: &mut Review) {
    review
        .blocking_issues
        .retain(|issue| !is_missing_phase_only_annotation_false_positive(issue));
    review
        .localized_issues
        .retain(|issue| !localized_issue_is_missing_phase_only_annotation_false_positive(issue));
    review
        .rejected_assets
        .retain(|asset| !rejected_asset_is_missing_phase_only_annotation_false_positive(asset));
    review.passed = review_passes_threshold(review);
}

pub fn render_quality_issues(layout_map_path: &Path) -> Result<Vec<String>> {
    Ok(build_quality_report(layout_map_path)?
        .issues
        .into_iter()
        .filter(|issue| issue.severity == "blocking" || issue.severity == "major")
        .map(|issue| issue.evidence)
        .collect())
}

pub fn build_quality_report(layout_map_path: &Path) -> Result<QualityReport> {
    let layout_map: LayoutMap = serde_json::from_slice(&fs::read(layout_map_path)?)?;
    Ok(build_quality_report_from_map(&layout_map))
}

pub fn apply_render_quality_gate(review: &mut Review, layout_map_path: &Path) -> Result<()> {
    for issue in render_quality_issues(layout_map_path)? {
        if !review.blocking_issues.contains(&issue) {
            review.blocking_issues.push(issue);
        }
    }
    review.passed = review_passes_threshold(review);
    Ok(())
}

pub fn apply_plan_geometry_gate(plan: &FigurePlan, review: &mut Review) {
    for issue in plan_geometry_issues(plan) {
        if !review.blocking_issues.contains(&issue) {
            review.blocking_issues.push(issue);
        }
    }
    review.passed = review_passes_threshold(review);
}

fn localized_issue_is_missing_phase_only_annotation_false_positive(issue: &LocalizedIssue) -> bool {
    let combined = format!(
        "{} {} {} {}",
        issue.target_id, issue.issue, issue.evidence, issue.suggested_direction
    );
    is_missing_phase_only_annotation_false_positive(&combined)
        || (is_phase_only_annotation_id(&issue.target_id)
            && text_mentions_missing_annotation(&combined))
}

fn rejected_asset_is_missing_phase_only_annotation_false_positive(asset: &RejectedAsset) -> bool {
    let combined = format!("{} {}", asset.asset_id, asset.reason);
    is_missing_phase_only_annotation_false_positive(&combined)
        || (is_phase_only_annotation_id(&asset.asset_id)
            && text_mentions_missing_annotation(&asset.reason))
}

fn is_missing_phase_only_annotation_false_positive(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    if !text_mentions_missing_annotation(&lower) {
        return false;
    }
    let explicit_phase_id = [
        "training_label",
        "train_label",
        "inference_label",
        "infer_label",
        "testing_label",
        "test_label",
        "anno_training",
        "anno_train",
        "anno_inference",
        "anno_infer",
        "anno_testing",
        "anno_test",
    ]
    .iter()
    .any(|needle| lower.contains(needle));
    let phase_only_phrase = lower.contains("phase-only") || lower.contains("phase only");
    let figure_plan_phase_annotation = lower.contains("figureplan")
        && text_mentions_annotation(&lower)
        && text_mentions_phase_word(&lower)
        && !lower.contains("loss");
    explicit_phase_id || phase_only_phrase || figure_plan_phase_annotation
}

fn text_mentions_missing_annotation(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    text_mentions_missing(&lower) && text_mentions_annotation(&lower)
}

fn text_mentions_missing(text: &str) -> bool {
    [
        "missing",
        "absent",
        "not present",
        "omitted",
        "removed",
        "lack",
        "lacks",
        "without",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

fn text_mentions_annotation(text: &str) -> bool {
    [
        "annotation",
        "label",
        "figureplan",
        "floating text",
        "phase",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

fn text_mentions_phase_word(text: &str) -> bool {
    ["training", "train", "inference", "infer", "testing", "test"]
        .iter()
        .any(|needle| text.contains(needle))
}

fn is_phase_only_annotation_id(id: &str) -> bool {
    let tokens = id
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    let has_phase_token = tokens.iter().any(|token| is_phase_token(token));
    let has_only_generic_or_phase_tokens = tokens
        .iter()
        .all(|token| is_phase_token(token) || is_generic_annotation_token(token));
    has_phase_token && has_only_generic_or_phase_tokens
}

fn is_phase_token(token: &str) -> bool {
    matches!(
        token,
        "training" | "train" | "inference" | "infer" | "testing" | "test"
    )
}

fn is_generic_annotation_token(token: &str) -> bool {
    matches!(
        token,
        "annotation" | "anno" | "ann" | "label" | "phase" | "note" | "tag"
    )
}

fn build_quality_report_from_map(layout_map: &LayoutMap) -> QualityReport {
    let issues = quality_issues_from_map(layout_map);
    let penalty = issues
        .iter()
        .map(|issue| match issue.severity.as_str() {
            // score 是给 best-so-far 和 regression budget 用的诊断分；通过与否仍由 blocking/major 决定。
            "blocking" => 12,
            "major" => 6,
            "minor" => 2,
            _ => 1,
        })
        .sum::<u32>();
    QualityReport {
        version: "0.1".to_string(),
        passed: !issues
            .iter()
            .any(|issue| issue.severity == "blocking" || issue.severity == "major"),
        score: 100_u32.saturating_sub(penalty),
        issues,
    }
}

fn quality_issues_from_map(layout_map: &LayoutMap) -> Vec<QualityIssue> {
    let mut issues = Vec::new();
    let components = layout_map
        .objects
        .iter()
        .filter(|object| object.kind == "component")
        .collect::<Vec<_>>();
    let edges = layout_map
        .objects
        .iter()
        .filter(|object| object.kind == "edge")
        .collect::<Vec<_>>();
    let labels = layout_map
        .objects
        .iter()
        .filter(|object| object.kind == "label" || object.kind == "annotation")
        .collect::<Vec<_>>();

    push_teacher_student_topology_issues(&mut issues, &components, &edges);
    push_teacher_student_supervision_symmetry_issues(&mut issues, &components, &edges);

    for component in &components {
        let (width, height) = box_size(component.bbox);
        if width < 0.035 || height < 0.08 {
            push_issue(
                &mut issues,
                "component_collapsed",
                "blocking",
                vec![component.id.clone()],
                format!(
                    "render quality failed: component {} is too small or collapsed",
                    component.id
                ),
                "Increase the component bbox so text remains readable at target paper width.",
            );
        }
        if let Some((fill_ratio, component_area)) =
            internal_whitespace_ratio(component, &layout_map.canvas)
        {
            let excessive_whitespace = (component_area > 0.08 && fill_ratio < 0.12)
                || (component_area > 0.055 && fill_ratio < 0.08);
            let teacher_context_whitespace = teacher_context_component_has_excessive_whitespace(
                component,
                fill_ratio,
                component_area,
            );
            if excessive_whitespace || teacher_context_whitespace {
                push_issue(
                    &mut issues,
                    "excessive_internal_whitespace",
                    "major",
                    vec![component.id.clone()],
                    format!(
                        "render quality failed: component {} has excessive internal whitespace (estimated text fill {:.3})",
                        component.id, fill_ratio
                    ),
                    "Increase paper-width font size, tighten the component bbox, or add necessary editable internal structure instead of leaving a large empty container.",
                );
            }
        }
        if let Some(risk) = text_wrap_risk(component, &layout_map.canvas) {
            push_issue(
                &mut issues,
                "text_wrap_risk",
                "major",
                vec![component.id.clone()],
                format!(
                    "render quality failed: component {} is too narrow for text '{}' ({:.1} mm needed, {:.1} mm available at target width)",
                    component.id, risk.token, risk.required_width_mm, risk.available_width_mm
                ),
                "Widen the component bbox, shorten the label, or lower the paper-width font size so the longest token fits without awkward wrapping.",
            );
        }
        if oversized_tiny_output_component(component) {
            push_issue(
                &mut issues,
                "oversized_tiny_output",
                "major",
                vec![component.id.clone()],
                format!(
                    "render quality failed: output component {} is oversized for a tiny label",
                    component.id
                ),
                "Shrink the tiny output/prediction box or place it beside its source so the output cue does not waste the right lane.",
            );
        }
        if output_component_has_excessive_whitespace(component) {
            push_issue(
                &mut issues,
                "output_excessive_whitespace",
                "major",
                vec![component.id.clone()],
                format!(
                    "render quality failed: output component {} is too tall for its short label",
                    component.id
                ),
                "Reduce the output box height and align it with the incoming flow so the output cue does not become a tall empty container.",
            );
        }
        if output_component_mixes_prediction_and_loss(component) {
            push_issue(
                &mut issues,
                "prediction_loss_semantic_mix",
                "major",
                vec![component.id.clone()],
                format!(
                    "render quality failed: prediction/output component {} mixes task-loss semantics into the forward output node",
                    component.id
                ),
                "Keep prediction/output nodes semantically clean; move task-loss text into a distinct compact objective node or supervision label.",
            );
        }
        if standalone_inference_component(component, &edges) {
            push_issue(
                &mut issues,
                "standalone_inference_lane",
                "major",
                vec![component.id.clone()],
                format!(
                    "render quality failed: inference component {} is a standalone lane without connector endpoints",
                    component.id
                ),
                "Remove the standalone inference lane or fold the inference-only cue into a nearby editable annotation attached to the student path; do not duplicate the input branch.",
            );
        }
        if let Some(anchor_ids) = unanchored_compact_inference_note(component, &components, &edges)
        {
            let mut target_ids = vec![component.id.clone()];
            target_ids.extend(anchor_ids);
            push_issue(
                &mut issues,
                "inference_note_unanchored",
                "major",
                target_ids,
                format!(
                    "render quality failed: compact inference note {} is far from the student/output anchor",
                    component.id
                ),
                "Keep the student-only inference cue compact, but place it next to the student or output path instead of floating in unrelated whitespace.",
            );
        }
        if compact_inference_note_has_excessive_whitespace(component, &edges) {
            push_issue(
                &mut issues,
                "inference_note_excessive_whitespace",
                "major",
                vec![component.id.clone()],
                format!(
                    "render quality failed: compact inference note {} uses an oversized box for a small cue",
                    component.id
                ),
                "Fold the student-only inference cue into compact editable text or shrink the note box to a small badge without wasting canvas space.",
            );
        }
        if let Some((teacher, student)) =
            task_loss_in_teacher_student_branch_corridor(component, &components, &edges)
        {
            push_issue(
                &mut issues,
                "task_loss_in_branch_corridor",
                "major",
                vec![component.id.clone(), teacher.id.clone(), student.id.clone()],
                format!(
                    "render quality failed: task loss {} sits in the teacher/student branch corridor",
                    component.id
                ),
                "Move the task-loss cue to the student/output periphery or align it as a side objective; do not place it in the vertical gap between teacher and student branches.",
            );
        }
        if let Some((teacher, student)) =
            residual_signal_in_teacher_student_branch_corridor(component, &components, &edges)
        {
            push_issue(
                &mut issues,
                "residual_signal_in_branch_corridor",
                "major",
                vec![component.id.clone(), teacher.id.clone(), student.id.clone()],
                format!(
                    "render quality failed: residual signal {} occupies the teacher/student branch corridor as a standalone box",
                    component.id
                ),
                "Fold the residual signal into a compact dashed supervision connector label or move it outside the teacher/student branch gap; avoid a standalone residual box in the main corridor.",
            );
        }
    }

    for (index, left) in components.iter().enumerate() {
        for right in components.iter().skip(index + 1) {
            let overlap = intersection_area(left.bbox, right.bbox);
            if overlap <= 0.0 {
                if let Some((axis, gap_mm)) =
                    component_crowding_gap(left.bbox, right.bbox, &layout_map.canvas)
                {
                    push_issue(
                        &mut issues,
                        "component_crowding",
                        "major",
                        vec![left.id.clone(), right.id.clone()],
                        format!(
                            "render quality failed: components {} and {} are too close on the {} axis ({:.1} mm gap at target width)",
                            left.id, right.id, axis, gap_mm
                        ),
                        "Increase the gap between the two component bboxes or reroute nearby connectors; do not rely on non-overlap alone.",
                    );
                }
                continue;
            }
            let left_area = area(left.bbox);
            let right_area = area(right.bbox);
            let ratio = overlap / left_area.min(right_area).max(0.0001);
            let (overlap_width, overlap_height) = intersection_dimensions(left.bbox, right.bbox);
            let overlap_width_mm = overlap_width * target_width_mm(&layout_map.canvas);
            let overlap_height_mm = overlap_height * target_height_mm(&layout_map.canvas);
            if component_visible_collision(
                overlap,
                overlap_width,
                overlap_height,
                ratio,
                &layout_map.canvas,
            ) || connected_component_thin_visible_collision(
                left,
                right,
                overlap_width,
                overlap_height,
                &edges,
                &layout_map.canvas,
            ) {
                push_issue(
                    &mut issues,
                    "component_overlap",
                    "blocking",
                    vec![left.id.clone(), right.id.clone()],
                    format!(
                        "render quality failed: component overlap between {} and {} ({:.1} x {:.1} mm at target width)",
                        left.id, right.id, overlap_width_mm, overlap_height_mm
                    ),
                    "Move or resize one of the overlapping component bboxes; do not restart the whole layout.",
                );
            }
        }
    }

    if let Some(union) = union_bbox(components.iter().map(|object| object.bbox)) {
        let utilization = area(union);
        let x_span = union[2] - union[0];
        let y_span = union[3] - union[1];
        if utilization < 0.28 && x_span < 0.65 && y_span < 0.35 {
            push_issue(
                &mut issues,
                "under_utilized",
                "major",
                components.iter().map(|object| object.id.clone()).collect(),
                format!(
                    "render quality failed: canvas is under-utilized by main content (occupied bbox area {:.3})",
                    utilization
                ),
                "Expand the main component group into available whitespace while preserving reading order.",
            );
        }
        let top_gap = union[1].max(0.0);
        let bottom_gap = (1.0 - union[3]).max(0.0);
        let top_heavy = bottom_gap > 0.24 && bottom_gap > top_gap + 0.18;
        let bottom_heavy = top_gap > 0.24 && top_gap > bottom_gap + 0.18;
        if components.len() >= 4 && x_span >= 0.65 && y_span < 0.70 && (top_heavy || bottom_heavy) {
            let (side, gap) = if top_heavy {
                ("bottom", bottom_gap)
            } else {
                ("top", top_gap)
            };
            push_issue(
                &mut issues,
                "vertical_under_utilization",
                "major",
                components.iter().map(|object| object.id.clone()).collect(),
                format!(
                    "render quality failed: main content is vertically top/bottom imbalanced with {:.1} mm {} whitespace at target width",
                    gap * target_height_mm(&layout_map.canvas),
                    side
                ),
                "Recenter or expand the main component group vertically so the figure uses the paper-height canvas instead of leaving a large blank band.",
            );
        }
    }

    let component_union = union_bbox(components.iter().map(|object| object.bbox));
    for label in &labels {
        let label_area = area(label.bbox);
        if label_area < 0.0005 {
            continue;
        }
        if let Some((teacher, student)) =
            inference_annotation_in_teacher_student_corridor(label, &components)
        {
            push_issue(
                &mut issues,
                "annotation_in_main_corridor",
                "major",
                vec![label.id.clone(), teacher.id.clone(), student.id.clone()],
                format!(
                    "render quality failed: inference annotation {} sits inside the main teacher-student corridor",
                    label.id
                ),
                "Move the inference annotation to the student periphery, outside connector corridors and away from the teacher-student transfer path.",
            );
        }
        if let Some(anchor_ids) = inference_annotation_in_bottom_margin(label, &components) {
            let mut target_ids = vec![label.id.clone()];
            target_ids.extend(anchor_ids);
            push_issue(
                &mut issues,
                "inference_annotation_in_bottom_margin",
                "major",
                target_ids,
                format!(
                    "render quality failed: inference annotation {} is pushed into the bottom canvas margin",
                    label.id
                ),
                "Move the inference cue next to the student/output path or fold it into the student component label; do not leave it as a detached bottom-margin caption.",
            );
        }
        if annotation_has_excessive_whitespace(label) {
            push_issue(
                &mut issues,
                "annotation_excessive_whitespace",
                "major",
                vec![label.id.clone()],
                format!(
                    "render quality failed: annotation {} is oversized for a short note",
                    label.id
                ),
                "Shrink the annotation bbox and anchor it next to its semantic target instead of leaving a large floating caption.",
            );
        }
        let target_edge = resolve_label_target_edge(label, &edges);
        if target_edge.is_none() {
            if let Some(component_union) = component_union {
                let expanded = expand_box(component_union, 0.08);
                if !boxes_overlap(expanded, label.bbox) {
                    push_issue(
                        &mut issues,
                        "label_outside_main_area",
                        "major",
                        vec![label.id.clone()],
                        format!(
                            "render quality failed: label {} sits outside the main figure area",
                            label.id
                        ),
                        "Move the label close to its target component or edge inside the main content bbox.",
                    );
                }
            }
        }
        for component in &components {
            let overlap = intersection_area(label.bbox, component.bbox);
            if overlap <= 0.0 {
                continue;
            }
            let (overlap_width, overlap_height) =
                intersection_dimensions(label.bbox, component.bbox);
            let overlap_width_mm = overlap_width * target_width_mm(&layout_map.canvas);
            let overlap_height_mm = overlap_height * target_height_mm(&layout_map.canvas);
            let component_ratio = overlap / area(component.bbox).max(0.0001);
            let label_ratio = overlap / label_area.max(0.0001);
            if overlap_width_mm >= 1.0
                && overlap_height_mm >= 1.0
                && (component_ratio > 0.08 || label_ratio > 0.08)
            {
                push_issue(
                    &mut issues,
                    "label_overlaps_component",
                    "blocking",
                    vec![label.id.clone(), component.id.clone()],
                    format!(
                        "render quality failed: label {} overlaps component {} ({:.1} x {:.1} mm at target width)",
                        label.id, component.id, overlap_width_mm, overlap_height_mm
                    ),
                    "Move the label or annotation outside the component bbox; do not leave editable text covering module text.",
                );
                break;
            }
        }
        for edge in &edges {
            if let Some(target_edge) = target_edge {
                if edge.id != target_edge.id {
                    continue;
                }
            }
            if label_overlaps_edge(label.bbox, edge) {
                push_issue(
                    &mut issues,
                    "label_overlaps_edge",
                    "blocking",
                    vec![label.id.clone(), edge.id.clone()],
                    format!(
                        "render quality failed: label {} overlaps edge {}",
                        label.id, edge.id
                    ),
                    "Move the label bbox off the connector stroke; keep the connector route unchanged unless necessary.",
                );
                break;
            }
            if label.kind == "label" && label_crowds_own_edge(label.bbox, edge) {
                push_issue(
                    &mut issues,
                    "label_overlaps_edge",
                    "blocking",
                    vec![label.id.clone(), edge.id.clone()],
                    format!(
                        "render quality failed: connector label {} is too close to edge {}",
                        label.id, edge.id
                    ),
                    "Move the connector label off the stroke with a clear paper-width gap; keep it near the same edge, not floating in unrelated whitespace.",
                );
                break;
            }
            if target_edge.is_some() {
                if let Some((student, output)) =
                    loss_label_on_prediction_edge(label, edge, &components)
                {
                    push_issue(
                        &mut issues,
                        "loss_label_on_prediction_edge",
                        "major",
                        vec![
                            label.id.clone(),
                            edge.id.clone(),
                            student.id.clone(),
                            output.id.clone(),
                        ],
                        format!(
                            "render quality failed: loss label {} is attached to prediction edge {} instead of a loss objective",
                            label.id, edge.id
                        ),
                        "Move the task-loss cue to a separate objective component or supervision edge; keep the student-to-output prediction edge semantically clean.",
                    );
                    break;
                }
            }
            if label.kind == "annotation" && annotation_too_close_to_edge(label.bbox, edge) {
                push_issue(
                    &mut issues,
                    "annotation_too_close_to_edge",
                    "major",
                    vec![label.id.clone(), edge.id.clone()],
                    format!(
                        "render quality failed: annotation {} sits too close to edge {}",
                        label.id, edge.id
                    ),
                    "Remove the generic annotation or move it away from the connector with clear whitespace; use edge labels only when the text is necessary.",
                );
                break;
            }
            if target_edge.is_some()
                && (!edge_target_has_multiple_incoming(edge, &edges)
                    || label_explicitly_targets_edge(label, edge))
                && label_far_from_edge(label, edge)
            {
                push_issue(
                    &mut issues,
                    "label_far_from_edge",
                    "major",
                    vec![label.id.clone(), edge.id.clone()],
                    format!(
                        "render quality failed: label {} is too far from its target edge {}",
                        label.id, edge.id
                    ),
                    "Move the label bbox next to the connector segment it describes; do not leave edge labels floating in unrelated whitespace.",
                );
                break;
            }
        }
    }

    for edge in &edges {
        if edge_length(edge) < 0.04 {
            push_issue(
                &mut issues,
                "degenerate_edge",
                "blocking",
                edge_target_ids(edge),
                format!(
                    "render quality failed: degenerate edge {} is too short",
                    edge.id
                ),
                "Reconnect the edge between distinct source/target anchors or remove the redundant edge.",
            );
        }
        if route_detour_ratio(edge) > 2.2
            || has_excessive_four_point_dogleg(edge, &edges)
            || has_long_three_point_input_context_detour(edge, &components)
            || has_rectangular_input_student_branch_detour(edge, &components)
            || has_three_point_input_student_elbow_detour(edge, &components)
            || has_residual_student_wandering_route(edge, &components)
            || has_main_output_outer_margin_detour(edge, &components)
            || has_shallow_u_main_output_route(edge, &components)
            || has_outer_margin_route(edge)
        {
            push_issue(
                &mut issues,
                "route_detour",
                "major",
                edge_target_ids(edge),
                format!(
                    "render quality failed: edge {} takes an excessive detour relative to its endpoints",
                    edge.id
                ),
                "Simplify the connector to a direct or two-segment orthogonal route without dogleg wandering.",
            );
        }
        if let Some((source, loss)) = far_student_task_loss_route(edge, &components) {
            push_issue(
                &mut issues,
                "task_loss_far_from_student_source",
                "major",
                vec![edge.id.clone(), source.id.clone(), loss.id.clone()],
                format!(
                    "render quality failed: task-loss edge {} sends the student objective to a far right lane",
                    edge.id
                ),
                "Move the task-loss cue near the student/output path or shorten the student-to-loss connector; do not stretch a long horizontal loss line across the main figure.",
            );
        }
        if let Some((source, loss)) = task_loss_reverse_flow(edge, &components) {
            push_issue(
                &mut issues,
                "task_loss_reverse_flow",
                "major",
                vec![edge.id.clone(), source.id.clone(), loss.id.clone()],
                format!(
                    "render quality failed: task-loss edge {} flows leftward/backward from student source {} to {}",
                    edge.id, source.id, loss.id
                ),
                "Move the task-loss cue to the right/upper-right of the student head or reroute the connector so the task objective follows the left-to-right reading direction.",
            );
        }
        if edge_length(edge) >= 0.04 {
            for component in &components {
                if edge.from.as_deref() == Some(component.id.as_str())
                    || edge.to.as_deref() == Some(component.id.as_str())
                {
                    continue;
                }
                if edge_crosses_component(edge, component) {
                    push_issue(
                        &mut issues,
                        "edge_crosses_component",
                        "blocking",
                        vec![edge.id.clone(), component.id.clone()],
                        format!(
                            "render quality failed: edge {} crosses through non-endpoint component {}",
                            edge.id, component.id
                        ),
                        "Reroute the connector around the crossed component while preserving the same source and target ids.",
                    );
                    break;
                }
            }
        }
    }

    for (index, left) in edges.iter().enumerate() {
        for right in edges.iter().skip(index + 1) {
            if edge_length(left) < 0.04 || edge_length(right) < 0.04 {
                continue;
            }
            if edge_segments(left).iter().any(|left_segment| {
                edge_segments(right)
                    .iter()
                    .any(|right_segment| segments_cross(*left_segment, *right_segment))
            }) {
                push_issue(
                    &mut issues,
                    "edge_crossing",
                    "blocking",
                    vec![left.id.clone(), right.id.clone()],
                    format!(
                        "render quality failed: edge crossing between {} and {}",
                        left.id, right.id
                    ),
                    "Reroute one connector around the crossing while keeping stable edge ids.",
                );
            }
        }
    }

    issues
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QualityReport {
    pub version: String,
    pub passed: bool,
    pub score: u32,
    pub issues: Vec<QualityIssue>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QualityIssue {
    pub issue_id: String,
    pub issue_type: String,
    pub severity: String,
    pub target_ids: Vec<String>,
    pub evidence: String,
    pub suggested_action: String,
}

fn push_issue(
    issues: &mut Vec<QualityIssue>,
    issue_type: &str,
    severity: &str,
    target_ids: Vec<String>,
    evidence: String,
    suggested_action: &str,
) {
    let issue_id = format!("quality_{:03}", issues.len() + 1);
    issues.push(QualityIssue {
        issue_id,
        issue_type: issue_type.to_string(),
        severity: severity.to_string(),
        target_ids,
        evidence,
        suggested_action: suggested_action.to_string(),
    });
}

fn plan_geometry_issues(plan: &FigurePlan) -> Vec<String> {
    let mut issues = Vec::new();
    let region_by_id: HashMap<_, _> = plan
        .layout
        .regions
        .iter()
        .map(|region| (region.id.as_str(), region.bbox))
        .collect();
    let component_box_by_id: HashMap<_, _> = plan
        .components
        .iter()
        .filter_map(|component| {
            region_by_id
                .get(component.region.as_str())
                .copied()
                .map(|bbox| (component.id.as_str(), bbox))
        })
        .collect();
    let component_boxes: Vec<_> = plan
        .components
        .iter()
        .filter_map(|component| component_box_by_id.get(component.id.as_str()).copied())
        .collect();
    if let Some(union) = union_bbox(component_boxes.iter().copied()) {
        if area(union) < 0.28 {
            issues.push(format!(
                "render quality failed: plan under-utilizes canvas area (occupied bbox area {:.3})",
                area(union)
            ));
        }
    }

    let edge_pairs: Vec<_> = plan
        .edges
        .iter()
        .filter_map(|edge| {
            let from = component_box_by_id.get(edge.from.as_str())?;
            let to = component_box_by_id.get(edge.to.as_str())?;
            Some((edge.id.as_str(), *from, *to))
        })
        .collect();

    let mut degree: HashMap<&str, usize> = HashMap::new();
    for edge in &plan.edges {
        *degree.entry(edge.from.as_str()).or_default() += 1;
        *degree.entry(edge.to.as_str()).or_default() += 1;
    }
    let collapsed_layout = component_boxes
        .first()
        .map(|first| {
            component_boxes
                .iter()
                .all(|bbox| boxes_close(*bbox, *first))
        })
        .unwrap_or(false);
    let simple_chain = plan.components.len() >= 3
        && plan.edges.len() + 1 == plan.components.len()
        && degree.values().all(|count| *count <= 2)
        && plan
            .components
            .iter()
            .all(|component| region_by_id.contains_key(component.region.as_str()))
        && !collapsed_layout;

    if simple_chain {
        if let Some(component_union) = union_bbox(component_boxes.iter().copied()) {
            let x_span = component_union[2] - component_union[0];
            let y_span = component_union[3] - component_union[1];
            let horizontal_chain = x_span >= y_span * 1.5;
            let vertical_chain = y_span >= x_span * 1.5;
            if !horizontal_chain && !vertical_chain {
                issues.push(
                    "render quality failed: simple chain should read horizontally or vertically, not diagonally"
                        .to_string(),
                );
            } else if horizontal_chain && y_span > 0.18 {
                issues.push(
                    "render quality failed: simple horizontal chain wastes vertical space and should align on one row"
                        .to_string(),
                );
            } else if vertical_chain && x_span > 0.18 {
                issues.push(
                    "render quality failed: simple vertical chain wastes horizontal space and should align on one column"
                        .to_string(),
                );
            }

            for (edge_id, from, to) in &edge_pairs {
                let dx = (to[0] + to[2]) / 2.0 - (from[0] + from[2]) / 2.0;
                let dy = (to[1] + to[3]) / 2.0 - (from[1] + from[3]) / 2.0;
                if dx.abs() > 0.04 && dy.abs() > 0.04 {
                    issues.push(format!(
                        "render quality failed: simple chain edge {} is diagonal instead of orthogonal",
                        edge_id
                    ));
                }
            }
        }
    }

    issues
}

pub fn mock_review(round_index: u32) -> Review {
    if round_index == 0 {
        Review {
            passed: false,
            scores: ReviewScores {
                semantic_fidelity: 8,
                story_clarity: 7,
                visual_hierarchy: 6,
                paper_readability: 7,
                layout_cleanliness: 7,
                arrow_routing: 8,
                color_semantics: 8,
                aesthetic_quality: 7,
                wps_editability: 9,
            },
            blocking_issues: vec![
                "Main contribution is not visually dominant enough at target width.".to_string(),
            ],
            localized_issues: vec![LocalizedIssue {
                target_id: "student".to_string(),
                bbox: [0.32, 0.28, 0.58, 0.62],
                severity: IssueSeverity::Major,
                issue: "Main module is too similar to context modules.".to_string(),
                evidence: "The central block does not stand out enough in the preview.".to_string(),
                suggested_direction: "Increase width and use primary fill for the main path."
                    .to_string(),
            }],
            accepted_assets: vec![],
            rejected_assets: vec![],
        }
    } else {
        let scores = ReviewScores {
            semantic_fidelity: 9,
            story_clarity: 9,
            visual_hierarchy: 9,
            paper_readability: 8,
            layout_cleanliness: 9,
            arrow_routing: 9,
            color_semantics: 8,
            aesthetic_quality: 8,
            wps_editability: 10,
        };
        Review {
            passed: review_passes_threshold(&Review {
                passed: false,
                scores: scores.clone(),
                blocking_issues: vec![],
                localized_issues: vec![],
                accepted_assets: vec![],
                rejected_assets: vec![],
            }),
            scores,
            blocking_issues: vec![],
            localized_issues: vec![],
            accepted_assets: vec!["student_icon".to_string(), "vision_icon".to_string()],
            rejected_assets: vec![],
        }
    }
}

pub fn mock_patch_plan() -> PatchPlan {
    PatchPlan {
        operations: vec![PatchOperation {
            id: "op_001".to_string(),
            target_id: "student".to_string(),
            executor: PatchExecutor::Reasoner,
            operation_type: PatchOperationType::LayoutPatch,
            action: "Increase visual emphasis for the main contribution module.".to_string(),
            expected_effect: "Main method path becomes dominant at paper width.".to_string(),
        }],
        stop_reason: PatchStopReason::Continue,
    }
}

#[derive(Debug, Deserialize)]
struct LayoutMap {
    #[serde(default)]
    canvas: LayoutCanvas,
    objects: Vec<LayoutObject>,
}

#[derive(Debug, Deserialize)]
struct LayoutCanvas {
    #[serde(default = "default_canvas_width_in")]
    width: f64,
    #[serde(default = "default_canvas_height_in")]
    height: f64,
    #[serde(default = "default_target_width_mm")]
    target_width_mm: f64,
}

impl Default for LayoutCanvas {
    fn default() -> Self {
        Self {
            width: default_canvas_width_in(),
            height: default_canvas_height_in(),
            target_width_mm: default_target_width_mm(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct LayoutObject {
    id: String,
    kind: String,
    bbox: [f64; 4],
    points: Option<Vec<[f64; 2]>>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    font_size_pt: Option<f64>,
    #[serde(default)]
    margin_in: Option<f64>,
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    to: Option<String>,
}

fn default_canvas_width_in() -> f64 {
    7.1
}

fn default_canvas_height_in() -> f64 {
    3.2
}

fn default_target_width_mm() -> f64 {
    85.0
}

fn box_size(bbox: [f64; 4]) -> (f64, f64) {
    ((bbox[2] - bbox[0]).max(0.0), (bbox[3] - bbox[1]).max(0.0))
}

fn area(bbox: [f64; 4]) -> f64 {
    let (width, height) = box_size(bbox);
    width * height
}

fn component_crowding_gap(
    left: [f64; 4],
    right: [f64; 4],
    canvas: &LayoutCanvas,
) -> Option<(&'static str, f64)> {
    let horizontal_gap =
        positive_gap(left[2], right[0]).or_else(|| positive_gap(right[2], left[0]));
    if let Some(gap) = horizontal_gap {
        let y_overlap = range_overlap_ratio((left[1], left[3]), (right[1], right[3]));
        let gap_mm = gap * target_width_mm(canvas);
        if y_overlap > 0.25 && gap_mm < 3.2 {
            return Some(("horizontal", gap_mm));
        }
    }

    let vertical_gap = positive_gap(left[3], right[1]).or_else(|| positive_gap(right[3], left[1]));
    if let Some(gap) = vertical_gap {
        let x_overlap = range_overlap_ratio((left[0], left[2]), (right[0], right[2]));
        let gap_mm = gap * target_height_mm(canvas);
        if x_overlap > 0.25 && gap_mm < 1.8 {
            return Some(("vertical", gap_mm));
        }
    }

    None
}

fn internal_whitespace_ratio(
    component: &LayoutObject,
    canvas: &LayoutCanvas,
) -> Option<(f64, f64)> {
    let text = component.text.as_deref()?.trim();
    if text.is_empty() {
        return None;
    }
    let font_size_pt = component.font_size_pt?;
    if !font_size_pt.is_finite() || font_size_pt <= 0.0 {
        return None;
    }

    let bbox = component.bbox;
    let component_area = area(bbox);
    let margin_in = component.margin_in.unwrap_or(0.0).max(0.0);
    let margin_x = margin_in / canvas_width_in(canvas);
    let margin_y = margin_in / canvas_height_in(canvas);
    let content_width = (bbox[2] - bbox[0] - margin_x * 2.0).max(0.001);
    let content_height = (bbox[3] - bbox[1] - margin_y * 2.0).max(0.001);
    let content_area = content_width * content_height;
    if content_area < 0.001 {
        return None;
    }

    let paper_scale = target_width_mm(canvas) / (canvas_width_in(canvas) * 25.4);
    let paper_font_size_pt = font_size_pt * paper_scale.clamp(0.35, 1.0);
    let line_weights = text.lines().map(weighted_char_count).collect::<Vec<_>>();
    let line_count = line_weights.len().max(1) as f64;
    let max_line_weight = line_weights.into_iter().fold(0.0, f64::max).max(1.0);
    let text_width_mm = max_line_weight * paper_font_size_pt * 0.352_778 * 0.54;
    let text_height_mm = line_count * paper_font_size_pt * 0.352_778 * 1.2;
    let text_width = (text_width_mm / target_width_mm(canvas)).min(content_width);
    let text_height = (text_height_mm / target_height_mm(canvas)).min(content_height);
    let fill_ratio = (text_width * text_height) / content_area;

    Some((fill_ratio, component_area))
}

fn teacher_context_component_has_excessive_whitespace(
    component: &LayoutObject,
    fill_ratio: f64,
    component_area: f64,
) -> bool {
    if component.kind != "component" {
        return false;
    }
    let identity = object_identity(component);
    let teacher_context = identity.contains("teacher")
        || identity.contains("training-only")
        || identity.contains("training only")
        || identity.contains("frozen")
        || identity.contains("large");
    if !teacher_context {
        return false;
    }
    let text = component.text.as_deref().unwrap_or("").trim();
    if text.is_empty() || text.lines().filter(|line| !line.trim().is_empty()).count() > 3 {
        return false;
    }
    let max_line_weight = text.lines().map(weighted_char_count).fold(0.0, f64::max);
    let (width, height) = box_size(component.bbox);
    component_area > 0.040
        && width > 0.30
        && height > 0.11
        && fill_ratio < 0.60
        && max_line_weight <= 13.0
}

struct TextWrapRisk {
    token: String,
    required_width_mm: f64,
    available_width_mm: f64,
}

fn text_wrap_risk(component: &LayoutObject, canvas: &LayoutCanvas) -> Option<TextWrapRisk> {
    let text = component.text.as_deref()?.trim();
    if text.is_empty() || text.contains('\n') {
        return None;
    }
    let font_size_pt = component.font_size_pt?;
    if !font_size_pt.is_finite() || font_size_pt <= 0.0 {
        return None;
    }
    let margin_in = component.margin_in.unwrap_or(0.0).max(0.0);
    let content_width =
        (component.bbox[2] - component.bbox[0] - margin_in / canvas_width_in(canvas) * 2.0)
            .max(0.0);
    let available_width_mm = content_width * target_width_mm(canvas);
    let paper_scale = target_width_mm(canvas) / (canvas_width_in(canvas) * 25.4);
    let paper_font_size_pt = font_size_pt * paper_scale.clamp(0.35, 1.0);
    if let Some(risk) =
        input_phrase_width_risk(component, text, paper_font_size_pt, available_width_mm)
    {
        return Some(risk);
    }
    let token = text
        .split_whitespace()
        .filter(|token| token.chars().count() >= 7)
        .max_by(|left, right| {
            weighted_char_count(left)
                .partial_cmp(&weighted_char_count(right))
                .unwrap_or(std::cmp::Ordering::Equal)
        })?;
    let required_width_mm = weighted_char_count(token) * paper_font_size_pt * 0.352_778;
    if required_width_mm > available_width_mm * 0.92 && available_width_mm < 16.0 {
        return Some(TextWrapRisk {
            token: token.to_string(),
            required_width_mm,
            available_width_mm,
        });
    }
    None
}

fn input_phrase_width_risk(
    component: &LayoutObject,
    text: &str,
    paper_font_size_pt: f64,
    available_width_mm: f64,
) -> Option<TextWrapRisk> {
    let identity = object_identity(component);
    if !(identity.contains("input")
        || identity.contains("source")
        || identity.contains("data")
        || identity.contains("x"))
    {
        return None;
    }
    if text.split_whitespace().count() < 2 || text.chars().count() > 24 {
        return None;
    }
    let (width, height) = box_size(component.bbox);
    if width >= 0.14 || height < 0.075 {
        return None;
    }
    let required_width_mm = weighted_char_count(text) * paper_font_size_pt * 0.352_778;
    if required_width_mm > available_width_mm * 1.05 && available_width_mm < 12.5 {
        Some(TextWrapRisk {
            token: text.to_string(),
            required_width_mm,
            available_width_mm,
        })
    } else {
        None
    }
}

fn oversized_tiny_output_component(component: &LayoutObject) -> bool {
    if component.kind != "component" {
        return false;
    }
    let identity = object_identity(component);
    let is_output = identity.contains("output")
        || identity.contains("prediction")
        || identity.contains("pred")
        || identity.contains("answer")
        || identity.contains('ŷ');
    if !is_output {
        return false;
    }
    let text = component.text.as_deref().unwrap_or("").trim();
    if text.is_empty() || weighted_char_count(text) > 2.5 {
        return false;
    }
    area(component.bbox) > 0.012 || box_size(component.bbox).1 > 0.13
}

fn output_component_has_excessive_whitespace(component: &LayoutObject) -> bool {
    if component.kind != "component" || !is_output_layout_object(component) {
        return false;
    }
    let text = component.text.as_deref().unwrap_or("").trim();
    if text.is_empty() {
        return false;
    }
    let visible_chars = text.chars().filter(|ch| !ch.is_whitespace()).count();
    let line_count = text.lines().filter(|line| !line.trim().is_empty()).count();
    let (_, height) = box_size(component.bbox);
    height > 0.18 && area(component.bbox) > 0.018 && visible_chars <= 24 && line_count <= 3
}

fn output_component_mixes_prediction_and_loss(component: &LayoutObject) -> bool {
    if component.kind != "component" || !is_output_layout_object(component) {
        return false;
    }
    let identity = object_identity(component);
    let prediction_cue = identity.contains("prediction")
        || identity.contains("predict")
        || identity.contains("pred")
        || identity.contains("output")
        || identity.contains('ŷ');
    let loss_cue = identity.contains("task loss")
        || identity.contains("task_loss")
        || identity.contains("l_task")
        || (identity.contains("task") && identity.contains("loss"));
    prediction_cue && loss_cue
}

fn is_output_layout_object(component: &LayoutObject) -> bool {
    let identity = object_identity(component);
    identity.contains("output")
        || identity.contains("prediction")
        || identity.contains("pred")
        || identity.contains("answer")
        || identity.contains('ŷ')
}

fn positive_gap(first_end: f64, second_start: f64) -> Option<f64> {
    let gap = second_start - first_end;
    (gap > 0.0).then_some(gap)
}

fn range_overlap_ratio(a: (f64, f64), b: (f64, f64)) -> f64 {
    let overlap = (a.1.min(b.1) - a.0.max(b.0)).max(0.0);
    let shortest = (a.1 - a.0).min(b.1 - b.0).max(0.0001);
    overlap / shortest
}

fn weighted_char_count(text: &str) -> f64 {
    text.chars()
        .filter(|character| !character.is_whitespace())
        .map(|character| if character.is_ascii() { 0.55 } else { 1.0 })
        .sum()
}

fn canvas_width_in(canvas: &LayoutCanvas) -> f64 {
    if canvas.width.is_finite() && canvas.width > 0.0 {
        canvas.width
    } else {
        default_canvas_width_in()
    }
}

fn canvas_height_in(canvas: &LayoutCanvas) -> f64 {
    if canvas.height.is_finite() && canvas.height > 0.0 {
        canvas.height
    } else {
        default_canvas_height_in()
    }
}

fn target_width_mm(canvas: &LayoutCanvas) -> f64 {
    if canvas.target_width_mm.is_finite() && canvas.target_width_mm > 0.0 {
        canvas.target_width_mm
    } else {
        default_target_width_mm()
    }
}

fn target_height_mm(canvas: &LayoutCanvas) -> f64 {
    target_width_mm(canvas) * canvas_height_in(canvas) / canvas_width_in(canvas)
}

fn intersection_area(a: [f64; 4], b: [f64; 4]) -> f64 {
    let (width, height) = intersection_dimensions(a, b);
    width * height
}

fn intersection_dimensions(a: [f64; 4], b: [f64; 4]) -> (f64, f64) {
    let x1 = a[0].max(b[0]);
    let y1 = a[1].max(b[1]);
    let x2 = a[2].min(b[2]);
    let y2 = a[3].min(b[3]);
    ((x2 - x1).max(0.0), (y2 - y1).max(0.0))
}

fn component_visible_collision(
    overlap: f64,
    overlap_width: f64,
    overlap_height: f64,
    ratio: f64,
    canvas: &LayoutCanvas,
) -> bool {
    let overlap_width_mm = overlap_width * target_width_mm(canvas);
    let overlap_height_mm = overlap_height * target_height_mm(canvas);
    let area_collision = overlap > 0.003 && ratio > 0.15;
    let broad_collision = overlap_width_mm >= 0.8 && overlap_height_mm >= 0.8 && ratio > 0.01;
    // 真实 smoke 暴露过横向重叠很长、纵向只压入约 0.7mm 的情况；纸面上这仍会盖住边框和文字。
    let thin_but_visible_collision = overlap_width_mm >= 6.0
        && overlap_height_mm >= 0.55
        && overlap_height >= 0.012
        && ratio > 0.12;
    area_collision || broad_collision || thin_but_visible_collision
}

fn connected_component_thin_visible_collision(
    left: &LayoutObject,
    right: &LayoutObject,
    overlap_width: f64,
    overlap_height: f64,
    edges: &[&LayoutObject],
    canvas: &LayoutCanvas,
) -> bool {
    if !edges.iter().any(|edge| {
        (edge.from.as_deref() == Some(left.id.as_str())
            && edge.to.as_deref() == Some(right.id.as_str()))
            || (edge.from.as_deref() == Some(right.id.as_str())
                && edge.to.as_deref() == Some(left.id.as_str()))
    }) {
        return false;
    }
    let overlap_width_mm = overlap_width * target_width_mm(canvas);
    let overlap_height_mm = overlap_height * target_height_mm(canvas);
    let same_row_overlap =
        overlap_width_mm >= 0.45 && overlap_height_mm >= 3.0 && overlap_height >= 0.06;
    let same_column_overlap =
        overlap_height_mm >= 0.45 && overlap_width_mm >= 3.0 && overlap_width >= 0.04;
    same_row_overlap || same_column_overlap
}

fn edge_length(edge: &LayoutObject) -> f64 {
    edge_segments(edge)
        .iter()
        .map(|segment| segment_length(*segment))
        .sum()
}

fn edge_target_ids(edge: &LayoutObject) -> Vec<String> {
    let mut ids = vec![edge.id.clone()];
    if let Some(from) = &edge.from {
        ids.push(from.clone());
    }
    if let Some(to) = &edge.to {
        ids.push(to.clone());
    }
    ids
}

fn route_detour_ratio(edge: &LayoutObject) -> f64 {
    let Some(points) = edge.points.as_ref() else {
        return 0.0;
    };
    if points.len() < 5 {
        return 0.0;
    }
    let Some(first) = points.first() else {
        return 0.0;
    };
    let Some(last) = points.last() else {
        return 0.0;
    };
    let direct = segment_length((*first, *last));
    if direct < 0.05 {
        return 0.0;
    }
    edge_length(edge) / direct
}

fn resolve_label_target_edge<'a>(
    label: &LayoutObject,
    edges: &'a [&LayoutObject],
) -> Option<&'a LayoutObject> {
    if label.kind != "label" {
        return None;
    }
    let explicit_id = label.id.strip_suffix("_label");
    if let Some(explicit_id) = explicit_id {
        if let Some(edge) = edges.iter().find(|edge| edge.id == explicit_id) {
            return Some(*edge);
        }
    }
    let center = bbox_center(label.bbox);
    let max_binding_distance = label_distance_limit(label) + 0.08;
    edges
        .iter()
        .filter_map(|edge| {
            let min_distance = edge_segments(edge)
                .iter()
                .map(|segment| point_to_segment_distance(center, *segment))
                .fold(f64::INFINITY, f64::min);
            (min_distance <= max_binding_distance).then_some((*edge, min_distance))
        })
        .min_by(|left, right| left.1.total_cmp(&right.1))
        .map(|(edge, _)| edge)
}

fn has_outer_margin_route(edge: &LayoutObject) -> bool {
    let Some(points) = edge.points.as_ref() else {
        return false;
    };
    if points.len() < 5 {
        return false;
    }
    let direct = segment_length((points[0], *points.last().unwrap_or(&points[0])));
    if direct < 0.20 {
        return false;
    }
    let route = edge_length(edge);
    if route <= direct * 1.45 {
        return false;
    }

    let route_box = points_to_box(points);
    let touches_outer_margin =
        route_box[0] < 0.06 || route_box[1] < 0.06 || route_box[2] > 0.94 || route_box[3] > 0.94;
    if !touches_outer_margin {
        return false;
    }

    let mut axis_changes = 0usize;
    let mut last_axis = None;
    let mut has_outer_leg = false;
    let mut has_span: f64 = 0.0;
    for window in points.windows(2) {
        let segment = (window[0], window[1]);
        let Some(axis) = segment_axis(segment) else {
            continue;
        };
        if let Some(last_axis) = last_axis {
            if last_axis != axis {
                axis_changes += 1;
            }
        }
        last_axis = Some(axis);
        let segment_length = segment_length(segment);
        has_span = has_span.max(segment_length);
        if segment_length > 0.22 && segment_touches_canvas_outer_margin(segment) {
            has_outer_leg = true;
        }
    }
    has_outer_leg && axis_changes >= 2 && has_span > 0.18
}

fn segment_touches_canvas_outer_margin(segment: ([f64; 2], [f64; 2])) -> bool {
    let (left, right) = (
        segment.0[0].min(segment.1[0]),
        segment.0[0].max(segment.1[0]),
    );
    let (top, bottom) = (
        segment.0[1].min(segment.1[1]),
        segment.0[1].max(segment.1[1]),
    );
    left < 0.06 || right > 0.94 || top < 0.06 || bottom > 0.94
}

fn has_long_three_point_input_context_detour(
    edge: &LayoutObject,
    components: &[&LayoutObject],
) -> bool {
    let Some(points) = edge.points.as_ref() else {
        return false;
    };
    if points.len() != 3 {
        return false;
    }
    let Some(from_id) = edge.from.as_deref() else {
        return false;
    };
    let Some(to_id) = edge.to.as_deref() else {
        return false;
    };
    let Some(from_component) = components
        .iter()
        .copied()
        .find(|component| component.id == from_id)
    else {
        return false;
    };
    let Some(to_component) = components
        .iter()
        .copied()
        .find(|component| component.id == to_id)
    else {
        return false;
    };
    let target_is_student_or_main = object_identity_contains(to_component, "student")
        || object_identity_contains(to_component, "main");
    let target_is_context =
        is_teacher_layout_object(to_component) || object_identity_contains(to_component, "context");
    if !object_identity_contains(from_component, "input")
        || !(target_is_context || target_is_student_or_main)
    {
        return false;
    }
    let first_leg = segment_length((points[0], points[1]));
    let second_leg = segment_length((points[1], points[2]));
    let direct = segment_length((points[0], points[2]));
    let min_first_leg = if target_is_student_or_main {
        0.16
    } else {
        0.30
    };
    if direct < 0.24 || first_leg < min_first_leg || second_leg < 0.24 {
        return false;
    }
    edge_length(edge) / direct > 1.32
}

fn has_main_output_outer_margin_detour(edge: &LayoutObject, components: &[&LayoutObject]) -> bool {
    let Some(points) = edge.points.as_ref() else {
        return false;
    };
    if points.len() != 3 {
        return false;
    }
    let (Some(from_id), Some(to_id)) = (edge.from.as_deref(), edge.to.as_deref()) else {
        return false;
    };
    let Some(from_component) = components
        .iter()
        .copied()
        .find(|component| component.id == from_id)
    else {
        return false;
    };
    let Some(to_component) = components
        .iter()
        .copied()
        .find(|component| component.id == to_id)
    else {
        return false;
    };
    if !object_identity_contains(from_component, "student")
        || !(object_identity_contains(to_component, "output")
            || object_identity_contains(to_component, "prediction")
            || object_identity_contains(to_component, "pred"))
    {
        return false;
    }
    if horizontal_separation(from_component.bbox, to_component.bbox) > 0.10
        || vertical_separation(from_component.bbox, to_component.bbox) < 0.22
    {
        return false;
    }
    let vertical_leg = points.windows(2).any(|window| {
        (window[0][0] - window[1][0]).abs() < 0.004 && (window[0][1] - window[1][1]).abs() > 0.28
    });
    let route_box = points_to_box(points);
    vertical_leg && (route_box[1] < 0.12 || route_box[3] > 0.82)
}

fn far_student_task_loss_route<'a>(
    edge: &LayoutObject,
    components: &[&'a LayoutObject],
) -> Option<(&'a LayoutObject, &'a LayoutObject)> {
    let points = edge.points.as_ref()?;
    let (Some(from_id), Some(to_id)) = (edge.from.as_deref(), edge.to.as_deref()) else {
        return None;
    };
    let source = components
        .iter()
        .copied()
        .find(|component| component.id == from_id)?;
    let loss = components
        .iter()
        .copied()
        .find(|component| component.id == to_id)?;
    if !student_or_main_source_layout_object(source) || !is_task_loss_layout_object(loss) {
        return None;
    }
    if horizontal_separation(source.bbox, loss.bbox) < 0.28 {
        return None;
    }
    let route_box = points_to_box(points);
    if route_box[2] - route_box[0] < 0.36 {
        return None;
    }
    if !has_long_horizontal_segment(points, 0.30) {
        return None;
    }
    Some((source, loss))
}

fn task_loss_reverse_flow<'a>(
    edge: &LayoutObject,
    components: &[&'a LayoutObject],
) -> Option<(&'a LayoutObject, &'a LayoutObject)> {
    let points = edge.points.as_ref()?;
    let (Some(from_id), Some(to_id)) = (edge.from.as_deref(), edge.to.as_deref()) else {
        return None;
    };
    let source = components
        .iter()
        .copied()
        .find(|component| component.id == from_id)?;
    let loss = components
        .iter()
        .copied()
        .find(|component| component.id == to_id)?;
    if !student_or_main_source_layout_object(source) || !is_task_loss_layout_object(loss) {
        return None;
    }
    if loss.bbox[2] >= source.bbox[0] - 0.03 {
        return None;
    }
    let (Some(first), Some(last)) = (points.first(), points.last()) else {
        return None;
    };
    if first[0] <= last[0] + 0.03 {
        return None;
    }
    Some((source, loss))
}

fn student_or_main_source_layout_object(component: &LayoutObject) -> bool {
    object_identity_contains(component, "student") || object_identity_contains(component, "main")
}

fn has_long_horizontal_segment(points: &[[f64; 2]], min_len: f64) -> bool {
    points.windows(2).any(|window| {
        (window[0][1] - window[1][1]).abs() < 0.004
            && (window[0][0] - window[1][0]).abs() >= min_len
    })
}

fn has_shallow_u_main_output_route(edge: &LayoutObject, components: &[&LayoutObject]) -> bool {
    let Some(points) = edge.points.as_ref() else {
        return false;
    };
    if points.len() != 4 {
        return false;
    }
    let (Some(from_id), Some(to_id)) = (edge.from.as_deref(), edge.to.as_deref()) else {
        return false;
    };
    let Some(from_component) = components
        .iter()
        .copied()
        .find(|component| component.id == from_id)
    else {
        return false;
    };
    let Some(to_component) = components
        .iter()
        .copied()
        .find(|component| component.id == to_id)
    else {
        return false;
    };
    let from_identity = object_identity(from_component);
    let to_identity = object_identity(to_component);
    let from_is_main_head = from_identity.contains("student")
        || from_identity.contains("head")
        || from_identity.contains("main");
    let to_is_output = to_identity.contains("output")
        || to_identity.contains("prediction")
        || to_identity.contains("pred")
        || to_identity.contains("answer")
        || to_identity.contains('ŷ');
    if !from_is_main_head || !to_is_output {
        return false;
    }
    if vertical_separation(from_component.bbox, to_component.bbox) > 0.03
        || horizontal_separation(from_component.bbox, to_component.bbox) > 0.55
    {
        return false;
    }
    let axes = points
        .windows(2)
        .filter_map(|window| segment_axis((window[0], window[1])))
        .collect::<Vec<_>>();
    if axes.as_slice() != ["vertical", "horizontal", "vertical"] {
        return false;
    }
    points_to_box(points)[2] - points_to_box(points)[0] > 0.20
}

fn has_rectangular_input_student_branch_detour(
    edge: &LayoutObject,
    components: &[&LayoutObject],
) -> bool {
    let Some(points) = edge.points.as_ref() else {
        return false;
    };
    if points.len() != 4 {
        return false;
    }
    let (Some(from_id), Some(to_id)) = (edge.from.as_deref(), edge.to.as_deref()) else {
        return false;
    };
    let Some(from_component) = components
        .iter()
        .copied()
        .find(|component| component.id == from_id)
    else {
        return false;
    };
    let Some(to_component) = components
        .iter()
        .copied()
        .find(|component| component.id == to_id)
    else {
        return false;
    };
    if !object_identity_contains(from_component, "input")
        || !object_identity_contains(to_component, "student")
    {
        return false;
    }
    let axes = points
        .windows(2)
        .filter_map(|window| segment_axis((window[0], window[1])))
        .collect::<Vec<_>>();
    if axes.as_slice() != ["vertical", "horizontal", "vertical"] {
        return false;
    }
    let route_box = points_to_box(points);
    let route_width = route_box[2] - route_box[0];
    let route_height = route_box[3] - route_box[1];
    let endpoint_vertical_span = (points[0][1] - points[3][1]).abs();
    route_width > 0.25 && route_height > endpoint_vertical_span + 0.055
}

fn has_three_point_input_student_elbow_detour(
    edge: &LayoutObject,
    components: &[&LayoutObject],
) -> bool {
    let Some(points) = edge.points.as_ref() else {
        return false;
    };
    if points.len() != 3 {
        return false;
    }
    let (Some(from_id), Some(to_id)) = (edge.from.as_deref(), edge.to.as_deref()) else {
        return false;
    };
    let Some(from_component) = components
        .iter()
        .copied()
        .find(|component| component.id == from_id)
    else {
        return false;
    };
    let Some(to_component) = components
        .iter()
        .copied()
        .find(|component| component.id == to_id)
    else {
        return false;
    };
    if !object_identity_contains(from_component, "input")
        || !object_identity_contains(to_component, "student")
    {
        return false;
    }
    let axes = points
        .windows(2)
        .filter_map(|window| segment_axis((window[0], window[1])))
        .collect::<Vec<_>>();
    if axes.as_slice() != ["vertical", "horizontal"]
        && axes.as_slice() != ["horizontal", "vertical"]
    {
        return false;
    }
    let vertical_leg = points
        .windows(2)
        .map(|window| (window[0][1] - window[1][1]).abs())
        .fold(0.0, f64::max);
    let horizontal_leg = points
        .windows(2)
        .map(|window| (window[0][0] - window[1][0]).abs())
        .fold(0.0, f64::max);
    vertical_leg > 0.20 && horizontal_leg > 0.10
}

fn has_residual_student_wandering_route(edge: &LayoutObject, components: &[&LayoutObject]) -> bool {
    let Some(points) = edge.points.as_ref() else {
        return false;
    };
    if points.len() != 4 {
        return false;
    }
    let (Some(from_id), Some(to_id)) = (edge.from.as_deref(), edge.to.as_deref()) else {
        return false;
    };
    let Some(from_component) = components
        .iter()
        .copied()
        .find(|component| component.id == from_id)
    else {
        return false;
    };
    let Some(to_component) = components
        .iter()
        .copied()
        .find(|component| component.id == to_id)
    else {
        return false;
    };
    let from_identity = object_identity(from_component);
    if !(from_identity.contains("residual") || from_identity.contains("latent"))
        || !object_identity_contains(to_component, "student")
    {
        return false;
    }
    let axes = points
        .windows(2)
        .filter_map(|window| segment_axis((window[0], window[1])))
        .collect::<Vec<_>>();
    if axes.as_slice() != ["vertical", "horizontal", "vertical"] {
        return false;
    }
    let direct = segment_length((points[0], points[3]));
    if direct < 0.10 {
        return false;
    }
    let route_box = points_to_box(points);
    edge_length(edge) / direct > 1.20
        && route_box[3] - route_box[1] > 0.18
        && route_box[2] - route_box[0] > 0.10
}

fn has_excessive_four_point_dogleg(edge: &LayoutObject, edges: &[&LayoutObject]) -> bool {
    if edge_target_has_multiple_incoming(edge, edges) {
        return false;
    }
    let Some(points) = edge.points.as_ref() else {
        return false;
    };
    if points.len() != 4 {
        return false;
    }
    let direct = segment_length((points[0], points[3]));
    if direct < 0.10 {
        return false;
    }
    let route = edge_length(edge);
    if route / direct < 1.25 {
        return false;
    }
    let axes = points
        .windows(2)
        .filter_map(|window| segment_axis((window[0], window[1])))
        .collect::<Vec<_>>();
    axes.len() == 3 && axes[0] == axes[2] && axes[0] != axes[1]
}

fn edge_target_has_multiple_incoming(edge: &LayoutObject, edges: &[&LayoutObject]) -> bool {
    let Some(target_id) = edge.to.as_deref() else {
        return false;
    };
    edges
        .iter()
        .filter(|candidate| candidate.to.as_deref() == Some(target_id))
        .take(2)
        .count()
        > 1
}

fn label_explicitly_targets_edge(label: &LayoutObject, edge: &LayoutObject) -> bool {
    label.id.strip_suffix("_label") == Some(edge.id.as_str())
}

fn segment_axis(segment: ([f64; 2], [f64; 2])) -> Option<&'static str> {
    let dx = (segment.1[0] - segment.0[0]).abs();
    let dy = (segment.1[1] - segment.0[1]).abs();
    if dx < 0.004 && dy > 0.015 {
        Some("vertical")
    } else if dy < 0.004 && dx > 0.015 {
        Some("horizontal")
    } else {
        None
    }
}

fn edge_segments(edge: &LayoutObject) -> Vec<([f64; 2], [f64; 2])> {
    if let Some(points) = &edge.points {
        if points.len() >= 2 {
            return points
                .windows(2)
                .map(|window| (window[0], window[1]))
                .collect();
        }
    }
    vec![([edge.bbox[0], edge.bbox[1]], [edge.bbox[2], edge.bbox[3]])]
}

fn label_overlaps_edge(label_bbox: [f64; 4], edge: &LayoutObject) -> bool {
    let label_area = area(label_bbox).max(0.0001);
    edge_segments(edge).iter().any(|(start, end)| {
        let segment_bbox = expand_box(segment_bbox(*start, *end), 0.008);
        let overlap = intersection_area(label_bbox, segment_bbox);
        let area_overlap = overlap > 0.0007 && overlap / label_area > 0.12;
        let stroke_overlap =
            segment_intersects_box_interior((*start, *end), expand_box(label_bbox, 0.005), 0.006);
        area_overlap || stroke_overlap
    })
}

fn label_crowds_own_edge(label_bbox: [f64; 4], edge: &LayoutObject) -> bool {
    edge_segments(edge)
        .iter()
        .any(|segment| label_crowds_segment(label_bbox, *segment))
}

fn label_crowds_segment(label_bbox: [f64; 4], segment: ([f64; 2], [f64; 2])) -> bool {
    let ([x1, y1], [x2, y2]) = segment;
    let dx = (x2 - x1).abs();
    let dy = (y2 - y1).abs();
    let min_clearance = 0.022;
    let center_distance = point_to_segment_distance(bbox_center(label_bbox), segment);
    if center_distance > 0.065 {
        return false;
    }
    if dy < 0.004 && dx > 0.030 {
        let seg_left = x1.min(x2);
        let seg_right = x1.max(x2);
        if axis_overlap_ratio(label_bbox[0], label_bbox[2], seg_left, seg_right) < 0.20 {
            return false;
        }
        let clearance = if label_bbox[1] >= y1 {
            label_bbox[1] - y1
        } else if label_bbox[3] <= y1 {
            y1 - label_bbox[3]
        } else {
            0.0
        };
        return clearance < min_clearance;
    }
    if dx < 0.004 && dy > 0.030 {
        let seg_top = y1.min(y2);
        let seg_bottom = y1.max(y2);
        if axis_overlap_ratio(label_bbox[1], label_bbox[3], seg_top, seg_bottom) < 0.20 {
            return false;
        }
        let clearance = if label_bbox[0] >= x1 {
            label_bbox[0] - x1
        } else if label_bbox[2] <= x1 {
            x1 - label_bbox[2]
        } else {
            0.0
        };
        return clearance < min_clearance;
    }
    false
}

fn annotation_too_close_to_edge(label_bbox: [f64; 4], edge: &LayoutObject) -> bool {
    edge_segments(edge).iter().any(|(start, end)| {
        let segment_box = expand_box(segment_bbox(*start, *end), 0.026);
        if intersection_area(label_bbox, segment_box) <= 0.0001 {
            return false;
        }
        let horizontal_segment = (start[1] - end[1]).abs() < 0.0001;
        let vertical_segment = (start[0] - end[0]).abs() < 0.0001;
        (horizontal_segment
            && axis_overlap_ratio(label_bbox[0], label_bbox[2], start[0], end[0]) > 0.20)
            || (vertical_segment
                && axis_overlap_ratio(label_bbox[1], label_bbox[3], start[1], end[1]) > 0.20)
    })
}

fn label_far_from_edge(label: &LayoutObject, edge: &LayoutObject) -> bool {
    let center = bbox_center(label.bbox);
    let min_distance = edge_segments(edge)
        .iter()
        .map(|segment| point_to_segment_distance(center, *segment))
        .fold(f64::INFINITY, f64::min);
    min_distance.is_finite() && min_distance > label_distance_limit(label)
}

fn annotation_has_excessive_whitespace(label: &LayoutObject) -> bool {
    if label.kind != "annotation" {
        return false;
    }
    let text = label.text.as_deref().unwrap_or("").trim();
    if text.is_empty() {
        return false;
    }
    let visible_chars = text.chars().filter(|ch| !ch.is_whitespace()).count();
    let (width, height) = box_size(label.bbox);
    visible_chars <= 28 && (area(label.bbox) > 0.024 || width > 0.28 || height > 0.09)
}

fn inference_annotation_in_teacher_student_corridor<'a>(
    label: &LayoutObject,
    components: &[&'a LayoutObject],
) -> Option<(&'a LayoutObject, &'a LayoutObject)> {
    if label.kind != "annotation" || !is_inference_annotation_layout_object(label) {
        return None;
    }
    let teachers = components
        .iter()
        .copied()
        .filter(|component| object_identity_contains(component, "teacher"))
        .collect::<Vec<_>>();
    let students = components
        .iter()
        .copied()
        .filter(|component| object_identity_contains(component, "student"))
        .collect::<Vec<_>>();
    for teacher in &teachers {
        for student in &students {
            if annotation_sits_between_teacher_and_student(label.bbox, teacher.bbox, student.bbox) {
                return Some((*teacher, *student));
            }
        }
    }
    None
}

fn inference_annotation_in_bottom_margin(
    label: &LayoutObject,
    components: &[&LayoutObject],
) -> Option<Vec<String>> {
    if label.kind != "annotation" || !is_student_only_inference_cue(label) {
        return None;
    }
    if label.bbox[3] < 0.96 || center_y(label.bbox) < 0.88 {
        return None;
    }
    Some(
        components
            .iter()
            .copied()
            .filter(|component| is_student_or_output_anchor(component))
            .take(2)
            .map(|component| component.id.clone())
            .collect(),
    )
}

fn is_student_only_inference_cue(object: &LayoutObject) -> bool {
    let identity = object_identity(object);
    identity.contains("inference")
        || identity.contains("infer")
        || identity.contains("student only")
        || identity.contains("student-only")
}

fn is_inference_annotation_layout_object(object: &LayoutObject) -> bool {
    let identity = object_identity(object);
    identity.contains("inference")
        || identity.contains("infer")
        || identity.contains("student only")
        || identity.contains("student-only")
        || identity.contains("supervision")
        || identity.contains("residual")
}

fn loss_label_on_prediction_edge<'a>(
    label: &LayoutObject,
    edge: &LayoutObject,
    components: &[&'a LayoutObject],
) -> Option<(&'a LayoutObject, &'a LayoutObject)> {
    if label.kind != "label" || !object_identity_contains(label, "loss") {
        return None;
    }
    let endpoints = [edge.from.as_deref(), edge.to.as_deref()]
        .into_iter()
        .flatten()
        .filter_map(|id| {
            components
                .iter()
                .copied()
                .find(|component| component.id == id)
        })
        .collect::<Vec<_>>();
    if endpoints.len() < 2
        || endpoints
            .iter()
            .any(|component| is_task_loss_layout_object(component))
    {
        return None;
    }
    let output = endpoints
        .iter()
        .copied()
        .find(|component| is_output_layout_object(component))?;
    let student = endpoints.iter().copied().find(|component| {
        component.id != output.id && object_identity_contains(component, "student")
    })?;
    Some((student, output))
}

fn standalone_inference_component(component: &LayoutObject, edges: &[&LayoutObject]) -> bool {
    if component.kind != "component" || !is_inference_annotation_layout_object(component) {
        return false;
    }
    if compact_student_only_inference_badge(component) {
        return false;
    }
    !edges.iter().any(|edge| {
        edge.from.as_deref() == Some(component.id.as_str())
            || edge.to.as_deref() == Some(component.id.as_str())
    })
}

fn unanchored_compact_inference_note(
    component: &LayoutObject,
    components: &[&LayoutObject],
    edges: &[&LayoutObject],
) -> Option<Vec<String>> {
    if component.kind != "component" || !compact_student_only_inference_badge(component) {
        return None;
    }
    if edges.iter().any(|edge| {
        edge.from.as_deref() == Some(component.id.as_str())
            || edge.to.as_deref() == Some(component.id.as_str())
    }) {
        return None;
    }
    let anchors = components
        .iter()
        .copied()
        .filter(|candidate| candidate.id != component.id)
        .filter(|candidate| is_student_or_output_anchor(candidate))
        .collect::<Vec<_>>();
    if anchors.is_empty()
        || anchors
            .iter()
            .any(|anchor| compact_note_is_visually_anchored(component.bbox, anchor.bbox))
    {
        return None;
    }
    Some(
        anchors
            .into_iter()
            .take(2)
            .map(|anchor| anchor.id.clone())
            .collect(),
    )
}

fn compact_inference_note_has_excessive_whitespace(
    component: &LayoutObject,
    edges: &[&LayoutObject],
) -> bool {
    if component.kind != "component" || !compact_student_only_inference_badge(component) {
        return false;
    }
    if edges.iter().any(|edge| {
        edge.from.as_deref() == Some(component.id.as_str())
            || edge.to.as_deref() == Some(component.id.as_str())
    }) {
        return false;
    }
    area(component.bbox) > 0.016 || (component.bbox[3] - component.bbox[1]).max(0.0) > 0.095
}

fn compact_student_only_inference_badge(component: &LayoutObject) -> bool {
    let identity = object_identity(component);
    (identity.contains("note") || identity.contains("badge"))
        && (identity.contains("student only") || identity.contains("student-only"))
}

fn is_student_or_output_anchor(component: &LayoutObject) -> bool {
    let identity = object_identity(component);
    identity.contains("student")
        || identity.contains("output")
        || identity.contains("prediction")
        || identity.contains("pred")
}

fn compact_note_is_visually_anchored(note: [f64; 4], anchor: [f64; 4]) -> bool {
    (horizontal_separation(note, anchor) <= 0.10 && vertical_separation(note, anchor) <= 0.12)
        || (axis_overlap_ratio(note[1], note[3], anchor[1], anchor[3]) > 0.20
            && horizontal_separation(note, anchor) <= 0.12)
}

fn task_loss_in_teacher_student_branch_corridor<'a>(
    component: &LayoutObject,
    components: &[&'a LayoutObject],
    edges: &[&LayoutObject],
) -> Option<(&'a LayoutObject, &'a LayoutObject)> {
    if component.kind != "component" || !is_task_loss_layout_object(component) {
        return None;
    }
    let connected_to_student = edges.iter().any(|edge| {
        edge_touches_object(edge, component.id.as_str())
            && edge_opposite_endpoint(edge, component.id.as_str())
                .and_then(|id| {
                    components
                        .iter()
                        .copied()
                        .find(|candidate| candidate.id == id)
                })
                .is_some_and(|candidate| object_identity_contains(candidate, "student"))
    });
    if !connected_to_student {
        return None;
    }
    let teachers = components
        .iter()
        .copied()
        .filter(|candidate| is_teacher_layout_object(candidate))
        .collect::<Vec<_>>();
    let students = components
        .iter()
        .copied()
        .filter(|candidate| object_identity_contains(candidate, "student"))
        .collect::<Vec<_>>();
    for teacher in &teachers {
        for student in &students {
            if component_sits_between_branch_rows(component.bbox, teacher.bbox, student.bbox) {
                return Some((*teacher, *student));
            }
        }
    }
    None
}

fn residual_signal_in_teacher_student_branch_corridor<'a>(
    component: &LayoutObject,
    components: &[&'a LayoutObject],
    edges: &[&LayoutObject],
) -> Option<(&'a LayoutObject, &'a LayoutObject)> {
    if component.kind != "component" || !is_residual_signal_layout_object(component) {
        return None;
    }
    let teachers = components
        .iter()
        .copied()
        .filter(|candidate| is_teacher_layout_object(candidate))
        .collect::<Vec<_>>();
    let students = components
        .iter()
        .copied()
        .filter(|candidate| object_identity_contains(candidate, "student"))
        .collect::<Vec<_>>();
    for teacher in &teachers {
        for student in &students {
            if !simple_teacher_student_branch_pair(teacher, student) {
                continue;
            }
            if !component_connected_to_branch_pair(component, teacher, student, edges) {
                continue;
            }
            if bbox_sits_between_branch_rows_with_side_gap(
                component.bbox,
                teacher.bbox,
                student.bbox,
                0.09,
            ) {
                return Some((*teacher, *student));
            }
        }
    }
    None
}

fn push_teacher_student_topology_issues(
    issues: &mut Vec<QualityIssue>,
    components: &[&LayoutObject],
    edges: &[&LayoutObject],
) {
    let Some(teacher_encoder) = primary_teacher_student_encoder_component(components, "teacher")
    else {
        return;
    };
    let Some(student_encoder) = primary_teacher_student_encoder_component(components, "student")
    else {
        return;
    };

    if center_y(teacher_encoder.bbox) > center_y(student_encoder.bbox) + 0.10 {
        push_issue(
            issues,
            "teacher_student_branch_inversion",
            "blocking",
            vec![teacher_encoder.id.clone(), student_encoder.id.clone()],
            format!(
                "render quality failed: teacher encoder {} is below student encoder {}, reversing the expected teacher-over-student branch order",
                teacher_encoder.id, student_encoder.id
            ),
            "Move the teacher branch above the student branch or keep both branches on a clean left-to-right row; do not place the teacher encoder below the student encoder in a vertical branch layout.",
        );
    }

    for edge in edges {
        if edge.from.as_deref() != Some(teacher_encoder.id.as_str()) {
            continue;
        }
        let Some(target_id) = edge.to.as_deref() else {
            continue;
        };
        let Some(target) = components
            .iter()
            .copied()
            .find(|component| component.id == target_id)
        else {
            continue;
        };
        if !teacher_head_like_component(target) {
            continue;
        }
        if center_y(target.bbox) + 0.06 < center_y(teacher_encoder.bbox) {
            push_issue(
                issues,
                "teacher_internal_flow_reversed",
                "blocking",
                vec![edge.id.clone(), teacher_encoder.id.clone(), target.id.clone()],
                format!(
                    "render quality failed: teacher flow edge {} runs upward from encoder {} to head {}, reversing local reading order",
                    edge.id, teacher_encoder.id, target.id
                ),
                "Keep the teacher encoder-to-head flow horizontal or left-to-right/downstream; do not put the teacher head above its encoder.",
            );
        }
    }
}

fn push_teacher_student_supervision_symmetry_issues(
    issues: &mut Vec<QualityIssue>,
    components: &[&LayoutObject],
    edges: &[&LayoutObject],
) {
    for supervision in components
        .iter()
        .copied()
        .filter(|component| is_residual_signal_layout_object(component))
    {
        let teachers = components
            .iter()
            .copied()
            .filter(|component| is_teacher_layout_object(component))
            .collect::<Vec<_>>();
        let students = components
            .iter()
            .copied()
            .filter(|component| object_identity_contains(component, "student"))
            .collect::<Vec<_>>();
        for teacher in &teachers {
            for student in &students {
                if center_y(teacher.bbox) + 0.25 >= center_y(student.bbox) {
                    continue;
                }
                let teacher_edges = connected_edges_between(edges, supervision, teacher);
                let student_edges = connected_edges_between(edges, supervision, student);
                if teacher_edges.is_empty() || student_edges.is_empty() {
                    continue;
                }
                if !bbox_sits_between_branch_rows_with_side_gap(
                    supervision.bbox,
                    teacher.bbox,
                    student.bbox,
                    0.16,
                ) {
                    continue;
                }
                let branch_mid_y = (center_y(teacher.bbox) + center_y(student.bbox)) / 2.0;
                let y_delta = (center_y(supervision.bbox) - branch_mid_y).abs();
                let upper_gap = (supervision.bbox[1] - teacher.bbox[3]).max(0.0);
                let lower_gap = (student.bbox[1] - supervision.bbox[3]).max(0.0);
                if y_delta <= 0.06 && (upper_gap - lower_gap).abs() <= 0.14 {
                    continue;
                }
                let mut target_ids = vec![
                    supervision.id.clone(),
                    teacher.id.clone(),
                    student.id.clone(),
                ];
                target_ids.extend(teacher_edges.iter().map(|edge| edge.id.clone()));
                target_ids.extend(student_edges.iter().map(|edge| edge.id.clone()));
                push_issue(
                    issues,
                    "supervision_branch_asymmetry",
                    "major",
                    target_ids,
                    format!(
                        "render quality failed: supervision node {} is not centered between teacher {} and student {} branches",
                        supervision.id, teacher.id, student.id
                    ),
                    "Move the supervision node toward the midpoint between the teacher and student branches and shorten both supervision connectors; do not crowd it against one branch.",
                );
                return;
            }
        }
    }
}

fn connected_edges_between<'a>(
    edges: &'a [&'a LayoutObject],
    left: &LayoutObject,
    right: &LayoutObject,
) -> Vec<&'a LayoutObject> {
    edges
        .iter()
        .copied()
        .filter(|edge| {
            edge_touches_object(edge, left.id.as_str())
                && edge_touches_object(edge, right.id.as_str())
        })
        .collect()
}

fn primary_teacher_student_encoder_component<'a>(
    components: &[&'a LayoutObject],
    role: &str,
) -> Option<&'a LayoutObject> {
    components
        .iter()
        .copied()
        .filter(|component| role_encoder_like_component(component, role))
        .min_by(|left, right| {
            let left_identity = object_identity(left);
            let right_identity = object_identity(right);
            let left_exact = left_identity.contains("encoder");
            let right_exact = right_identity.contains("encoder");
            right_exact
                .cmp(&left_exact)
                .then_with(|| center_y(left.bbox).total_cmp(&center_y(right.bbox)))
        })
}

fn role_encoder_like_component(component: &LayoutObject, role: &str) -> bool {
    if component.kind != "component" || !object_identity_contains(component, role) {
        return false;
    }
    let identity = object_identity(component);
    let encoder_like = identity.contains("encoder")
        || identity.contains("encode")
        || identity.contains("_enc")
        || identity.contains("-enc")
        || identity.contains(" enc");
    encoder_like
        && !identity.contains("head")
        && !identity.contains("loss")
        && !identity.contains("input")
        && !identity.contains("output")
        && !identity.contains("prediction")
}

fn teacher_head_like_component(component: &LayoutObject) -> bool {
    if component.kind != "component" || !object_identity_contains(component, "teacher") {
        return false;
    }
    let identity = object_identity(component);
    identity.contains("head") && !identity.contains("loss")
}

fn simple_teacher_student_branch_pair(teacher: &LayoutObject, student: &LayoutObject) -> bool {
    simple_branch_label_object(teacher, "teacher") && simple_branch_label_object(student, "student")
}

fn simple_branch_label_object(object: &LayoutObject, keyword: &str) -> bool {
    let identity = object_identity(object);
    if !identity.contains(keyword) {
        return false;
    }
    ![
        "encoder",
        "decoder",
        "projection",
        "proj",
        "head",
        "tower",
        "latent",
        "output",
        "prediction",
        "compact",
        "loss",
    ]
    .iter()
    .any(|token| identity.contains(token))
}

fn component_connected_to_branch_pair(
    component: &LayoutObject,
    teacher: &LayoutObject,
    student: &LayoutObject,
    edges: &[&LayoutObject],
) -> bool {
    let connected_to_teacher = edges.iter().any(|edge| {
        edge_touches_object(edge, component.id.as_str())
            && edge_touches_object(edge, teacher.id.as_str())
    });
    let connected_to_student = edges.iter().any(|edge| {
        edge_touches_object(edge, component.id.as_str())
            && edge_touches_object(edge, student.id.as_str())
    });
    connected_to_teacher && connected_to_student
}

fn is_residual_signal_layout_object(component: &LayoutObject) -> bool {
    let identity = object_identity(component);
    (identity.contains("residual") || identity.contains("latent"))
        && !identity.contains("teacher")
        && !identity.contains("student")
        && !identity.contains("input")
        && !identity.contains("output")
        && !identity.contains("task")
}

fn is_task_loss_layout_object(component: &LayoutObject) -> bool {
    let identity = object_identity(component);
    identity.contains("task") && identity.contains("loss")
}

fn is_teacher_layout_object(component: &LayoutObject) -> bool {
    let identity = object_identity(component);
    identity.contains("teacher") || identity.contains("frozen")
}

fn edge_touches_object(edge: &LayoutObject, id: &str) -> bool {
    edge.from.as_deref() == Some(id) || edge.to.as_deref() == Some(id)
}

fn edge_opposite_endpoint<'a>(edge: &'a LayoutObject, id: &str) -> Option<&'a str> {
    if edge.from.as_deref() == Some(id) {
        edge.to.as_deref()
    } else if edge.to.as_deref() == Some(id) {
        edge.from.as_deref()
    } else {
        None
    }
}

fn component_sits_between_branch_rows(
    component: [f64; 4],
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
    let component_center_y = center_y(component);
    if component_center_y <= upper[3] || component_center_y >= lower[1] {
        return false;
    }
    let branch_span = [
        upper[0].min(lower[0]),
        upper[3],
        upper[2].max(lower[2]),
        lower[1],
    ];
    axis_overlap_ratio(component[0], component[2], branch_span[0], branch_span[2]) > 0.10
}

fn object_identity_contains(object: &LayoutObject, needle: &str) -> bool {
    object_identity(object).contains(needle)
}

fn object_identity(object: &LayoutObject) -> String {
    format!(
        "{} {}",
        object.id.to_lowercase(),
        object.text.as_deref().unwrap_or("").to_lowercase()
    )
}

fn annotation_sits_between_teacher_and_student(
    annotation: [f64; 4],
    teacher: [f64; 4],
    student: [f64; 4],
) -> bool {
    if annotation_sits_between_branch_rows(annotation, teacher, student) {
        return true;
    }
    let (left, right) = if bbox_center(teacher)[0] <= bbox_center(student)[0] {
        (teacher, student)
    } else {
        (student, teacher)
    };
    let corridor_width = right[0] - left[2];
    if corridor_width < 0.12 {
        return false;
    }
    let horizontally_between = annotation[0] >= left[2] - 0.02 && annotation[2] <= right[0] + 0.04;
    if !horizontally_between {
        return false;
    }
    let row = [
        left[0].min(right[0]),
        left[1].min(right[1]),
        left[2].max(right[2]),
        left[3].max(right[3]),
    ];
    vertical_separation(annotation, row) < 0.05
        || axis_overlap_ratio(annotation[1], annotation[3], row[1], row[3]) > 0.10
}

fn annotation_sits_between_branch_rows(
    annotation: [f64; 4],
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
    let annotation_center = bbox_center(annotation);
    if annotation_center[1] <= upper[3] || annotation_center[1] >= lower[1] {
        return false;
    }
    let branch_span = [
        upper[0].min(lower[0]),
        upper[3],
        upper[2].max(lower[2]),
        lower[1],
    ];
    let near_branch_centerline = annotation_center[0] >= branch_span[0] - 0.03
        && annotation_center[0] <= branch_span[2] + 0.03;
    near_branch_centerline
        && (axis_overlap_ratio(annotation[0], annotation[2], branch_span[0], branch_span[2]) > 0.10
            || horizontal_separation(annotation, branch_span) <= 0.03)
}

fn bbox_sits_between_branch_rows_with_side_gap(
    bbox: [f64; 4],
    teacher: [f64; 4],
    student: [f64; 4],
    side_gap: f64,
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
    let bbox_center_y = center_y(bbox);
    if bbox_center_y <= upper[3] || bbox_center_y >= lower[1] {
        return false;
    }
    let branch_span = [
        upper[0].min(lower[0]),
        upper[3],
        upper[2].max(lower[2]),
        lower[1],
    ];
    axis_overlap_ratio(bbox[0], bbox[2], branch_span[0], branch_span[2]) > 0.10
        || horizontal_separation(bbox, branch_span) <= side_gap
}

fn label_distance_limit(label: &LayoutObject) -> f64 {
    let visible_chars = label
        .text
        .as_deref()
        .unwrap_or("")
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .count();
    if visible_chars <= 8 {
        0.08
    } else {
        0.16
    }
}

fn bbox_center(bbox: [f64; 4]) -> [f64; 2] {
    [(bbox[0] + bbox[2]) / 2.0, (bbox[1] + bbox[3]) / 2.0]
}

fn center_y(bbox: [f64; 4]) -> f64 {
    (bbox[1] + bbox[3]) / 2.0
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

fn horizontal_separation(left: [f64; 4], right: [f64; 4]) -> f64 {
    if left[2] < right[0] {
        right[0] - left[2]
    } else if right[2] < left[0] {
        left[0] - right[2]
    } else {
        0.0
    }
}

fn axis_overlap_ratio(left_start: f64, left_end: f64, right_start: f64, right_end: f64) -> f64 {
    let overlap = (left_end.min(right_end) - left_start.max(right_start)).max(0.0);
    let left_len = (left_end - left_start).abs();
    let right_len = (right_end - right_start).abs();
    if left_len <= f64::EPSILON || right_len <= f64::EPSILON {
        0.0
    } else {
        overlap / left_len.min(right_len)
    }
}

fn point_to_segment_distance(point: [f64; 2], segment: ([f64; 2], [f64; 2])) -> f64 {
    let vx = segment.1[0] - segment.0[0];
    let vy = segment.1[1] - segment.0[1];
    let wx = point[0] - segment.0[0];
    let wy = point[1] - segment.0[1];
    let length_sq = vx * vx + vy * vy;
    if length_sq <= f64::EPSILON {
        return ((point[0] - segment.0[0]).powi(2) + (point[1] - segment.0[1]).powi(2)).sqrt();
    }
    let t = ((wx * vx + wy * vy) / length_sq).clamp(0.0, 1.0);
    let projection = [segment.0[0] + t * vx, segment.0[1] + t * vy];
    ((point[0] - projection[0]).powi(2) + (point[1] - projection[1]).powi(2)).sqrt()
}

fn edge_crosses_component(edge: &LayoutObject, component: &LayoutObject) -> bool {
    let Some(interior) = shrink_box(component.bbox, 0.004) else {
        return false;
    };
    edge_segments(edge)
        .iter()
        .any(|segment| segment_intersects_box_interior(*segment, interior, 0.008))
}

fn segment_intersects_box_interior(
    segment: ([f64; 2], [f64; 2]),
    bbox: [f64; 4],
    min_inside_length: f64,
) -> bool {
    let length = segment_length(segment);
    if length < min_inside_length {
        return false;
    }
    let Some((start_t, end_t)) = clip_segment_to_box(segment, bbox) else {
        return false;
    };
    (end_t - start_t).max(0.0) * length > min_inside_length
}

fn clip_segment_to_box(segment: ([f64; 2], [f64; 2]), bbox: [f64; 4]) -> Option<(f64, f64)> {
    let x0 = segment.0[0];
    let y0 = segment.0[1];
    let dx = segment.1[0] - x0;
    let dy = segment.1[1] - y0;
    let mut start_t = 0.0;
    let mut end_t = 1.0;
    for (p, q) in [
        (-dx, x0 - bbox[0]),
        (dx, bbox[2] - x0),
        (-dy, y0 - bbox[1]),
        (dy, bbox[3] - y0),
    ] {
        if !clip_line_parameter(p, q, &mut start_t, &mut end_t) {
            return None;
        }
    }
    (end_t > start_t).then_some((start_t, end_t))
}

fn clip_line_parameter(p: f64, q: f64, start_t: &mut f64, end_t: &mut f64) -> bool {
    if p.abs() < f64::EPSILON {
        return q >= 0.0;
    }
    let r = q / p;
    if p < 0.0 {
        if r > *end_t {
            return false;
        }
        if r > *start_t {
            *start_t = r;
        }
    } else {
        if r < *start_t {
            return false;
        }
        if r < *end_t {
            *end_t = r;
        }
    }
    true
}

fn segment_bbox(start: [f64; 2], end: [f64; 2]) -> [f64; 4] {
    [
        start[0].min(end[0]),
        start[1].min(end[1]),
        start[0].max(end[0]),
        start[1].max(end[1]),
    ]
}

fn points_to_box(points: &[[f64; 2]]) -> [f64; 4] {
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

fn segment_length(segment: ([f64; 2], [f64; 2])) -> f64 {
    let dx = segment.1[0] - segment.0[0];
    let dy = segment.1[1] - segment.0[1];
    (dx * dx + dy * dy).sqrt()
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

fn orientation(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> f64 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

fn points_close(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 0.01 && (a.1 - b.1).abs() < 0.01
}

fn union_bbox<I>(boxes: I) -> Option<[f64; 4]>
where
    I: IntoIterator<Item = [f64; 4]>,
{
    let mut iter = boxes.into_iter();
    let first = iter.next()?;
    Some(iter.fold(first, |acc, bbox| {
        [
            acc[0].min(bbox[0]),
            acc[1].min(bbox[1]),
            acc[2].max(bbox[2]),
            acc[3].max(bbox[3]),
        ]
    }))
}

fn expand_box(bbox: [f64; 4], margin: f64) -> [f64; 4] {
    [
        (bbox[0] - margin).clamp(0.0, 1.0),
        (bbox[1] - margin).clamp(0.0, 1.0),
        (bbox[2] + margin).clamp(0.0, 1.0),
        (bbox[3] + margin).clamp(0.0, 1.0),
    ]
}

fn shrink_box(bbox: [f64; 4], margin: f64) -> Option<[f64; 4]> {
    let shrunk = [
        (bbox[0] + margin).clamp(0.0, 1.0),
        (bbox[1] + margin).clamp(0.0, 1.0),
        (bbox[2] - margin).clamp(0.0, 1.0),
        (bbox[3] - margin).clamp(0.0, 1.0),
    ];
    (shrunk[0] < shrunk[2] && shrunk[1] < shrunk[3]).then_some(shrunk)
}

fn boxes_overlap(a: [f64; 4], b: [f64; 4]) -> bool {
    intersection_area(a, b) > 0.0
}

fn boxes_close(a: [f64; 4], b: [f64; 4]) -> bool {
    (a[0] - b[0]).abs() < 0.01
        && (a[1] - b[1]).abs() < 0.01
        && (a[2] - b[2]).abs() < 0.01
        && (a[3] - b[3]).abs() < 0.01
}
