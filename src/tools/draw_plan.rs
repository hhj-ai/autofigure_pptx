use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;

use crate::schema::{
    Component, DrawLabel, DrawObject, DrawPlan, EdgeImportance, EdgeSemantic, EdgeStyle,
    FigurePlan, Template,
};
use crate::style::StyleSpec;

pub fn draw_plan_from_figure_plan(plan: &FigurePlan, style: &StyleSpec) -> DrawPlan {
    let component_bbox = packed_component_boxes(plan);

    let mut objects = Vec::new();
    for (index, component) in plan.components.iter().enumerate() {
        let Some(bbox) = component_bbox.get(component.id.as_str()).copied() else {
            continue;
        };
        objects.push(DrawObject::Box {
            id: component.id.clone(),
            bbox,
            text: component.label.clone(),
            role: format!("{:?}", component.role).to_lowercase(),
            style: component_style(component),
            z: 20 + index as i32,
        });
        if let Some(asset_id) = &component.allowed_asset_id {
            objects.push(DrawObject::Image {
                id: format!("{asset_id}_slot"),
                bbox: inset_asset_box(bbox),
                asset_id: asset_id.clone(),
                z: 30 + index as i32,
            });
        }
    }

    for (index, edge) in plan.edges.iter().enumerate() {
        let (Some(from), Some(to)) = (
            component_bbox.get(edge.from.as_str()).copied(),
            component_bbox.get(edge.to.as_str()).copied(),
        ) else {
            continue;
        };
        let start = center(from);
        let end = center(to);
        let label = if edge.label.trim().is_empty() {
            None
        } else {
            Some(DrawLabel {
                text: edge.label.clone(),
                bbox: connector_label_bbox(start, end),
            })
        };
        objects.push(DrawObject::Connector {
            id: edge.id.clone(),
            points: vec![start, end],
            from: Some(edge.from.clone()),
            to: Some(edge.to.clone()),
            style: figure_plan_edge_style(edge.semantic, edge.style, edge.importance).to_string(),
            label,
            z: 10 + index as i32,
        });
    }

    for (index, annotation) in plan.annotations.iter().enumerate() {
        if let Some(bbox) = annotation.bbox {
            objects.push(DrawObject::Text {
                id: annotation.id.clone(),
                bbox,
                text: annotation.label.clone(),
                style: "annotation".to_string(),
                z: 40 + index as i32,
            });
        }
    }

    DrawPlan {
        version: "0.2".to_string(),
        canvas: plan.canvas.clone(),
        style_tokens: BTreeMap::from([
            ("background".to_string(), style.palette.background.clone()),
            ("primary".to_string(), style.palette.primary.clone()),
            ("accent".to_string(), style.palette.accent.clone()),
            ("neutral_fill".to_string(), style.palette.muted_fill.clone()),
            ("text".to_string(), style.palette.text.clone()),
        ]),
        objects,
    }
}

pub fn has_material_draw_plan_change(previous: &DrawPlan, current: &DrawPlan) -> bool {
    !draw_plan_material_changes(previous, current).is_empty()
}

pub fn draw_plan_material_changes(previous: &DrawPlan, current: &DrawPlan) -> Vec<String> {
    let previous_by_id = previous
        .objects
        .iter()
        .map(|object| (draw_object_id(object).to_string(), object))
        .collect::<BTreeMap<_, _>>();
    let current_by_id = current
        .objects
        .iter()
        .map(|object| (draw_object_id(object).to_string(), object))
        .collect::<BTreeMap<_, _>>();
    let mut changes = Vec::new();

    for id in previous_by_id.keys() {
        if !current_by_id.contains_key(id) {
            changes.push(format!("{id} removed"));
        }
    }
    for id in current_by_id.keys() {
        if !previous_by_id.contains_key(id) {
            changes.push(format!("{id} added"));
        }
    }
    for (id, previous_object) in previous_by_id {
        let Some(current_object) = current_by_id.get(&id) else {
            continue;
        };
        collect_object_material_changes(&id, previous_object, current_object, &mut changes);
    }

    changes
}

fn collect_object_material_changes(
    id: &str,
    previous: &DrawObject,
    current: &DrawObject,
    changes: &mut Vec<String>,
) {
    match (previous, current) {
        (
            DrawObject::Box {
                bbox: previous_bbox,
                text: previous_text,
                role: previous_role,
                style: previous_style,
                ..
            },
            DrawObject::Box {
                bbox: current_bbox,
                text: current_text,
                role: current_role,
                style: current_style,
                ..
            },
        ) => {
            push_bbox_change(id, "bbox", *previous_bbox, *current_bbox, changes);
            push_text_change(id, "text", previous_text, current_text, changes);
            push_text_change(id, "role", previous_role, current_role, changes);
            push_text_change(id, "style", previous_style, current_style, changes);
        }
        (
            DrawObject::Text {
                bbox: previous_bbox,
                text: previous_text,
                style: previous_style,
                ..
            },
            DrawObject::Text {
                bbox: current_bbox,
                text: current_text,
                style: current_style,
                ..
            },
        ) => {
            push_bbox_change(id, "bbox", *previous_bbox, *current_bbox, changes);
            push_text_change(id, "text", previous_text, current_text, changes);
            push_text_change(id, "style", previous_style, current_style, changes);
        }
        (
            DrawObject::Connector {
                points: previous_points,
                from: previous_from,
                to: previous_to,
                style: previous_style,
                label: previous_label,
                ..
            },
            DrawObject::Connector {
                points: current_points,
                from: current_from,
                to: current_to,
                style: current_style,
                label: current_label,
                ..
            },
        ) => {
            if !points_close(previous_points, current_points) {
                changes.push(format!("{id} points changed"));
            }
            if previous_from != current_from {
                changes.push(format!("{id} from changed"));
            }
            if previous_to != current_to {
                changes.push(format!("{id} to changed"));
            }
            push_text_change(id, "style", previous_style, current_style, changes);
            collect_label_changes(id, previous_label.as_ref(), current_label.as_ref(), changes);
        }
        (
            DrawObject::Image {
                bbox: previous_bbox,
                asset_id: previous_asset_id,
                ..
            },
            DrawObject::Image {
                bbox: current_bbox,
                asset_id: current_asset_id,
                ..
            },
        ) => {
            push_bbox_change(id, "bbox", *previous_bbox, *current_bbox, changes);
            push_text_change(id, "asset", previous_asset_id, current_asset_id, changes);
        }
        (
            DrawObject::Group {
                bbox: previous_bbox,
                label: previous_label,
                style: previous_style,
                ..
            },
            DrawObject::Group {
                bbox: current_bbox,
                label: current_label,
                style: current_style,
                ..
            },
        ) => {
            push_bbox_change(id, "bbox", *previous_bbox, *current_bbox, changes);
            if previous_label != current_label {
                changes.push(format!("{id} label changed"));
            }
            push_text_change(id, "style", previous_style, current_style, changes);
        }
        _ => changes.push(format!("{id} kind changed")),
    }
}

fn collect_label_changes(
    id: &str,
    previous: Option<&DrawLabel>,
    current: Option<&DrawLabel>,
    changes: &mut Vec<String>,
) {
    match (previous, current) {
        (None, None) => {}
        (Some(_), None) | (None, Some(_)) => changes.push(format!("{id} label changed")),
        (Some(previous), Some(current)) => {
            push_text_change(id, "label text", &previous.text, &current.text, changes);
            push_bbox_change(id, "label bbox", previous.bbox, current.bbox, changes);
        }
    }
}

fn push_bbox_change(
    id: &str,
    field: &str,
    previous: [f64; 4],
    current: [f64; 4],
    changes: &mut Vec<String>,
) {
    if !bbox_close(previous, current) {
        changes.push(format!("{id} {field} changed"));
    }
}

fn push_text_change(
    id: &str,
    field: &str,
    previous: &str,
    current: &str,
    changes: &mut Vec<String>,
) {
    if previous != current {
        changes.push(format!("{id} {field} changed"));
    }
}

fn bbox_close(previous: [f64; 4], current: [f64; 4]) -> bool {
    previous
        .iter()
        .zip(current)
        .all(|(left, right)| (*left - right).abs() < 0.001)
}

fn points_close(previous: &[[f64; 2]], current: &[[f64; 2]]) -> bool {
    previous.len() == current.len()
        && previous.iter().zip(current).all(|(left, right)| {
            (left[0] - right[0]).abs() < 0.001 && (left[1] - right[1]).abs() < 0.001
        })
}

pub fn repair_draw_plan_geometry(plan: &mut DrawPlan) {
    repair_draw_plan_geometry_inner(plan, None);
}

pub fn repair_draw_plan_geometry_with_figure_plan(plan: &mut DrawPlan, figure_plan: &FigurePlan) {
    repair_draw_plan_geometry_inner(plan, Some(figure_plan));
}

pub fn polish_model_draw_plan_geometry(plan: &mut DrawPlan) {
    polish_model_draw_plan_geometry_inner(plan, &HashSet::new());
}

pub fn normalize_draw_plan_bounds(plan: &mut DrawPlan) {
    let mut box_map = HashMap::new();
    for object in &mut plan.objects {
        match object {
            DrawObject::Box { id, bbox, .. } => {
                *bbox = shift_box_inside_canvas(*bbox);
                box_map.insert(id.clone(), *bbox);
            }
            DrawObject::Text { bbox, .. }
            | DrawObject::Image { bbox, .. }
            | DrawObject::Group { bbox, .. } => {
                *bbox = shift_box_inside_canvas(*bbox);
            }
            DrawObject::Connector { .. } => {}
        }
    }

    for object in &mut plan.objects {
        if let DrawObject::Connector {
            points,
            from,
            to,
            label,
            ..
        } = object
        {
            for point in points.iter_mut() {
                *point = clamp_point(*point);
            }
            if points.len() < 2 {
                if let Some(repaired) = repair_degenerate_connector_for_normalization(
                    points.as_slice(),
                    from.as_deref(),
                    to.as_deref(),
                    &box_map,
                ) {
                    *points = repaired;
                }
            }
            if let Some(label) = label {
                label.bbox = shift_box_inside_canvas(label.bbox);
            }
        }
    }
}

fn repair_degenerate_connector_for_normalization(
    points: &[[f64; 2]],
    from: Option<&str>,
    to: Option<&str>,
    box_map: &HashMap<String, [f64; 4]>,
) -> Option<Vec<[f64; 2]>> {
    if points.len() >= 2 {
        return None;
    }
    let from_box = from.and_then(|id| box_map.get(id)).copied();
    let to_box = to.and_then(|id| box_map.get(id)).copied();
    if let (Some(from_box), Some(to_box)) = (from_box, to_box) {
        if let Some(start) = points.first().copied() {
            let end = anchor_point_towards(to_box, start);
            let mut repaired = vec![clamp_point(start)];
            push_distinct(&mut repaired, clamp_point(end));
            if repaired.len() >= 2 {
                return Some(repaired);
            }
        }
        return Some(orthogonal_connector_points_between_boxes(from_box, to_box));
    }

    points.first().copied().map(|point| {
        let start = clamp_point(point);
        let end = [(start[0] + 0.04).min(1.0), start[1]];
        if (end[0] - start[0]).abs() < 0.0001 {
            vec![start, [(start[0] - 0.04).max(0.0), start[1]]]
        } else {
            vec![start, end]
        }
    })
}

fn polish_model_draw_plan_geometry_inner(
    plan: &mut DrawPlan,
    protected_note_ids: &HashSet<String>,
) {
    normalize_draw_plan_bounds(plan);
    plan.objects
        .retain(|object| !is_phase_only_text_annotation(object));
    remove_redundant_phase_loss_and_inference_notes(plan);
    remove_inference_annotations_overlapping_other_annotations(plan);
    remove_duplicate_connector_label_annotations(plan);
    remove_asymmetric_branch_annotations(plan);
    remove_template_reference_and_overlapping_path_annotations(plan);
    fold_standalone_inference_notes_into_student_annotations(plan, protected_note_ids);
    expand_tiny_model_boxes(plan);
    compact_oversized_loss_or_objective_boxes(plan);
    widen_loss_or_objective_boxes_for_readability(plan);
    compact_wide_residual_supervision_boxes(plan);
    compact_oversized_short_content_boxes(plan);
    widen_short_main_boxes_for_readability(plan);
    widen_short_input_boxes_for_readability(plan);
    compact_single_line_flow_boxes_in_vertical_stacks(plan);
    stack_crowded_student_branch_chains(plan);
    stack_student_encoder_head_pairs_top_down(plan);
    remove_embedded_task_loss_text_from_main_boxes(plan);
    remove_redundant_inference_only_parentheticals_from_main_boxes(plan);
    fold_unconnected_residual_boxes_into_supervision_labels(plan);
    fold_connected_residual_signal_boxes_between_branch_rows(plan);
    widen_output_boxes_for_long_tokens(plan);
    widen_short_context_note_boxes_for_text(plan);
    align_shared_input_boxes_with_branch_targets(plan);
    align_single_input_boxes_with_main_targets(plan);
    move_shared_inputs_off_outer_margins(plan);
    stabilize_teacher_student_shared_inputs(plan);
    repair_branch_input_output_gutters(plan);
    move_task_losses_to_clear_blocked_vertical_output_lanes(plan);
    move_student_only_inference_outputs_next_to_sources(plan);
    align_output_boxes_with_sources(plan);
    compact_tall_output_boxes_for_short_labels(plan);
    align_outer_margin_outputs_with_sources(plan);
    straighten_adjacent_main_output_connectors(plan);
    align_task_loss_boxes_with_outputs(plan);
    align_touching_task_loss_boxes_with_sources(plan);
    repair_same_row_teacher_student_shared_input_collapse(plan);
    balance_multistage_teacher_student_branches(plan);
    repair_two_stage_teacher_student_branch_layout(plan);
    balance_simple_teacher_student_y_branch_layout(plan);
    balance_direct_teacher_student_supervision_branches(plan);
    move_task_loss_boxes_out_of_output_main_corridors(plan);
    move_task_loss_boxes_out_of_main_output_horizontal_corridors(plan);
    move_task_loss_boxes_out_of_teacher_student_branch_corridors(plan);
    pull_far_student_task_loss_boxes_near_output_path(plan);
    pull_top_edge_task_losses_near_outputs(plan);
    move_output_task_losses_out_of_branch_corridors(plan);
    separate_task_loss_boxes_from_main_modules(plan);
    repair_right_edge_output_collisions(plan);
    separate_crowded_outputs_from_sources(plan);
    separate_outputs_from_residual_supervision_hubs(plan);
    separate_sibling_task_loss_and_output_boxes(plan);
    separate_horizontally_crowded_connected_boxes(plan);
    separate_note_like_connected_boxes_from_sources(plan);
    separate_crowded_loss_or_objective_boxes(plan);
    separate_teacher_context_objective_gutters(plan);
    pull_top_residual_losses_near_sources(plan);
    move_objective_hubs_out_of_branch_gap_crowding(plan);
    separate_stacked_context_boxes(plan);
    resolve_model_box_overlaps(plan);
    repair_degenerate_connector_points_from_boxes(plan);
    improve_connector_routes_against_boxes(plan);
    reroute_connectors_around_intermediate_boxes(plan);
    polish_labels_and_marginal_annotations(plan, true);
    move_annotations_off_components(plan);
    move_annotations_off_edges(plan);
    move_inference_annotations_out_of_teacher_student_corridors(plan);
    compact_floating_inference_excluded_annotations(plan);
    improve_connector_routes_against_boxes(plan);
    reroute_connectors_around_intermediate_boxes(plan);
    reroute_objective_feedback_away_from_reverse_shared_segments(plan);
    reroute_output_to_task_loss_around_intermediate_boxes(plan);
    remove_duplicate_connectors(plan);
    reroute_supervision_connectors_around_main_edges(plan);
    reroute_residual_objective_connectors_around_main_crossings(plan);
    reroute_connectors_around_crossing_edges(plan);
    reroute_task_loss_connectors_around_supervision_edges(plan);
    repair_outer_input_context_detours(plan);
    orthogonalize_student_only_inference_connectors(plan);
    reroute_connectors_around_crossing_edges(plan);
    repair_outer_input_context_detours(plan);
    straighten_residual_alignment_rails(plan);
    simplify_residual_objective_to_main_edges(plan);
    simplify_main_to_residual_supervision_edges(plan);
    reroute_branch_residual_connectors_around_inputs(plan);
    simplify_main_to_output_connectors(plan);
    move_task_losses_to_clear_blocked_vertical_output_lanes(plan);
    simplify_adjacent_output_loss_connectors(plan);
    simplify_student_to_task_loss_connectors(plan);
    remove_redundant_residual_supervision_labels(plan);
    remove_redundant_task_loss_connector_labels(plan);
    remove_duplicate_connectors(plan);
    fold_noisy_connected_inference_notes_into_student_annotations(plan, protected_note_ids);
    remove_redundant_phase_loss_and_inference_notes(plan);
    remove_duplicate_inference_note_boxes_when_annotation_exists(plan);
    remove_duplicate_inference_text_when_note_component_exists(plan);
    remove_auxiliary_inference_note_connectors(plan);
    move_student_inference_notes_near_student(plan);
    move_inference_note_boxes_out_of_flow_corridors(plan);
    move_unanchored_inference_note_boxes_to_student_periphery(plan);
    ensure_inference_note_boxes_readable(plan);
    convert_peripheral_inference_note_boxes_to_annotations(plan);
    move_context_notes_away_from_loss_boxes(plan);
    fold_detached_protected_inference_notes_into_student_annotations(plan, protected_note_ids);
    split_embedded_inference_notes_from_output_boxes(plan);
    snap_connector_labels_to_final_routes(plan);
    tighten_short_connector_labels_near_routes(plan);
    move_connector_labels_off_components(plan);
    snap_compact_task_loss_labels_near_short_output_edges(plan);
    compact_oversized_short_annotations(plan);
    anchor_training_only_annotations_near_teacher(plan);
    move_simple_y_branch_inference_caption_to_periphery(plan);
    repair_two_stage_teacher_student_branch_layout(plan);
    move_bottom_edge_objectives_and_inference_notes_inside_safe_area(plan);
    separate_right_side_task_loss_output_and_inference_corridor(plan);
    repair_left_teacher_central_student_objective_topology(plan);
    simplify_teacher_alignment_stair_step_connectors(plan);
    repair_multistage_projector_encoder_overlap_layout(plan);
    remove_overlapping_duplicate_inference_texts(plan);
    move_task_loss_boxes_out_of_teacher_student_branch_corridors(plan);
    repair_comp_named_residual_task_feedback_topology(plan);
    repair_student_head_output_task_loss_column(plan);
    repair_projection_latent_teacher_student_topology(plan);
    repair_split_duplicate_input_residual_hub_topology(plan);
    repair_simple_teacher_student_branch_compaction(plan);
    repair_right_edge_student_output_loss_lane(plan);
    repair_compact_teacher_student_y_branch_annotations(plan);
    move_residual_equation_annotations_out_of_main_corridors(plan);
    move_bottom_margin_inference_texts_near_student(plan);
    rebalance_vertically_underutilized_main_group(plan);
    anchor_training_only_annotations_near_teacher(plan);
    normalize_draw_plan_bounds(plan);
}

pub fn polish_model_draw_plan_geometry_with_figure_plan(
    plan: &mut DrawPlan,
    figure_plan: &FigurePlan,
) {
    let had_inference_text_annotation = existing_inference_text_annotation_index(plan).is_some()
        || figure_plan
            .annotations
            .iter()
            .any(|annotation| annotation_label_is_inference_specific(&annotation.label));
    normalize_figure_plan_component_objects(plan, figure_plan);
    add_missing_component_boxes_from_figure_plan(plan, figure_plan);
    sync_connector_endpoints_from_figure_plan(plan, figure_plan);
    sync_connector_styles_from_figure_plan(plan, figure_plan);
    remove_connectors_absent_from_figure_plan(plan, figure_plan);
    prune_residual_boxes_absent_from_figure_plan(plan, figure_plan);
    reposition_note_components_near_sources(plan, figure_plan);
    add_missing_connectors_from_figure_plan(plan, figure_plan);
    reposition_note_components_near_sources(plan, figure_plan);
    let protected_note_ids = figure_plan
        .components
        .iter()
        .filter(|component| !is_inference_note_component(component))
        .map(|component| component.id.clone())
        .collect::<HashSet<_>>();
    polish_model_draw_plan_geometry_inner(plan, &protected_note_ids);
    upsert_meaningful_annotations_from_figure_plan(plan, figure_plan);
    let target_bound_inference_annotation_ids = target_bound_inference_annotation_ids(figure_plan);
    remove_redundant_phase_loss_and_inference_notes(plan);
    move_student_inference_notes_near_student_except(plan, &target_bound_inference_annotation_ids);
    move_inference_note_boxes_out_of_flow_corridors(plan);
    move_annotations_off_components(plan);
    move_annotations_off_edges(plan);
    move_inference_annotations_out_of_teacher_student_corridors(plan);
    move_student_inference_notes_near_student_except(plan, &target_bound_inference_annotation_ids);
    move_inference_note_boxes_out_of_flow_corridors(plan);
    move_unanchored_inference_note_boxes_to_student_periphery(plan);
    move_inference_annotations_out_of_teacher_student_corridors(plan);
    if had_inference_text_annotation {
        fold_unconnected_component_inference_notes_into_annotation(plan);
    }
    remove_redundant_phase_loss_and_inference_notes(plan);
    remove_duplicate_inference_note_boxes_when_annotation_exists(plan);
    remove_duplicate_inference_text_when_note_component_exists(plan);
    compact_oversized_short_annotations(plan);
    anchor_training_only_annotations_near_teacher(plan);
    move_simple_y_branch_inference_caption_to_periphery(plan);
    separate_right_side_task_loss_output_and_inference_corridor(plan);
    repair_left_teacher_central_student_objective_topology(plan);
    simplify_teacher_alignment_stair_step_connectors(plan);
    repair_multistage_projector_encoder_overlap_layout(plan);
    remove_overlapping_duplicate_inference_texts(plan);
    move_task_loss_boxes_out_of_teacher_student_branch_corridors(plan);
    repair_comp_named_residual_task_feedback_topology(plan);
    restore_missing_inference_components_as_annotations(plan, figure_plan);
    restore_missing_inference_components_as_boxes(plan, figure_plan);
    remove_duplicate_inference_text_when_note_component_exists(plan);
    repair_student_head_output_task_loss_column(plan);
    repair_projection_latent_teacher_student_topology(plan);
    repair_split_duplicate_input_residual_hub_topology(plan);
    repair_simple_teacher_student_branch_compaction(plan);
    repair_teacher_student_residual_node_main_route_crossing(plan);
    repair_right_edge_student_output_loss_lane(plan);
    repair_compact_teacher_student_y_branch_annotations(plan);
    move_residual_equation_annotations_out_of_main_corridors(plan);
    move_bottom_margin_inference_texts_near_student(plan);
    rebalance_vertically_underutilized_main_group(plan);
    sync_connector_styles_only_from_figure_plan(plan, figure_plan);
    anchor_training_only_annotations_near_teacher(plan);
    normalize_draw_plan_bounds(plan);
}

fn normalize_figure_plan_component_objects(plan: &mut DrawPlan, figure_plan: &FigurePlan) {
    for component in &figure_plan.components {
        for object in &mut plan.objects {
            let replacement = match object {
                DrawObject::Text {
                    id, bbox, text, z, ..
                } if id == &component.id => Some(DrawObject::Box {
                    id: component.id.clone(),
                    bbox: *bbox,
                    text: if text.trim().is_empty() {
                        component.label.clone()
                    } else {
                        text.clone()
                    },
                    role: format!("{:?}", component.role).to_lowercase(),
                    style: component_style(component),
                    z: *z,
                }),
                _ => None,
            };
            if let Some(replacement) = replacement {
                *object = replacement;
                break;
            }
        }
    }
}

fn component_style(component: &Component) -> String {
    if is_objective_component(component) {
        return "accent_module".to_string();
    }
    match component.visual_weight {
        crate::schema::VisualWeight::Strong => "primary_module",
        crate::schema::VisualWeight::Muted => "muted_module",
        crate::schema::VisualWeight::Normal => "neutral_module",
    }
    .to_string()
}

fn is_objective_component(component: &Component) -> bool {
    if component.role == crate::schema::ComponentRole::Loss {
        return true;
    }
    let text = format!("{} {}", component.id, component.label).to_lowercase();
    text.contains("loss")
        || text.contains("residual")
        || text.contains("alignment")
        || text.contains("objective")
}

fn add_missing_component_boxes_from_figure_plan(plan: &mut DrawPlan, figure_plan: &FigurePlan) {
    let component_bbox = packed_component_boxes(figure_plan);
    let mut existing_ids = plan
        .objects
        .iter()
        .map(|object| draw_object_id(object).to_string())
        .collect::<HashSet<_>>();
    for component in &figure_plan.components {
        if existing_ids.contains(&component.id) {
            continue;
        }
        let Some(bbox) = component_bbox.get(component.id.as_str()).copied() else {
            continue;
        };
        plan.objects.push(DrawObject::Box {
            id: component.id.clone(),
            bbox,
            text: component.label.clone(),
            role: format!("{:?}", component.role).to_lowercase(),
            style: component_style(component),
            z: next_z(plan),
        });
        existing_ids.insert(component.id.clone());
    }
}

fn sync_connector_endpoints_from_figure_plan(plan: &mut DrawPlan, figure_plan: &FigurePlan) {
    let box_map = current_box_map(plan);
    for object in &mut plan.objects {
        let DrawObject::Connector {
            id,
            from,
            to,
            points,
            ..
        } = object
        else {
            continue;
        };
        let Some(edge) = figure_plan.edges.iter().find(|edge| edge.id == id.as_str()) else {
            continue;
        };
        if from.as_deref() == Some(edge.from.as_str()) && to.as_deref() == Some(edge.to.as_str()) {
            continue;
        }
        *from = Some(edge.from.clone());
        *to = Some(edge.to.clone());
        if let (Some(from_box), Some(to_box)) = (
            box_map.get(edge.from.as_str()).copied(),
            box_map.get(edge.to.as_str()).copied(),
        ) {
            *points = orthogonal_connector_points_between_boxes(from_box, to_box);
        }
    }
}

fn sync_connector_styles_from_figure_plan(plan: &mut DrawPlan, figure_plan: &FigurePlan) {
    for object in &mut plan.objects {
        let DrawObject::Connector {
            id,
            from,
            to,
            points,
            style,
            label,
            ..
        } = object
        else {
            continue;
        };
        let Some(edge) = figure_plan.edges.iter().find(|edge| {
            edge.id == id.as_str()
                || (from.as_deref() == Some(edge.from.as_str())
                    && to.as_deref() == Some(edge.to.as_str()))
        }) else {
            continue;
        };
        *style = figure_plan_edge_style(edge.semantic, edge.style, edge.importance).to_string();
        if edge.label.trim().is_empty() {
            *label = None;
        } else {
            let start = points.first().copied().unwrap_or([0.45, 0.45]);
            let end = points.last().copied().unwrap_or([0.55, 0.55]);
            *label = Some(DrawLabel {
                text: edge.label.clone(),
                bbox: connector_label_bbox(start, end),
            });
        }
    }
}

fn sync_connector_styles_only_from_figure_plan(plan: &mut DrawPlan, figure_plan: &FigurePlan) {
    for object in &mut plan.objects {
        let DrawObject::Connector {
            id,
            from,
            to,
            style,
            ..
        } = object
        else {
            continue;
        };
        let Some(edge) = figure_plan.edges.iter().find(|edge| {
            edge.id == id.as_str()
                || (from.as_deref() == Some(edge.from.as_str())
                    && to.as_deref() == Some(edge.to.as_str()))
        }) else {
            continue;
        };
        *style = figure_plan_edge_style(edge.semantic, edge.style, edge.importance).to_string();
    }
}

fn remove_connectors_absent_from_figure_plan(plan: &mut DrawPlan, figure_plan: &FigurePlan) {
    let component_ids = figure_plan
        .components
        .iter()
        .map(|component| component.id.as_str())
        .collect::<HashSet<_>>();
    plan.objects.retain(|object| {
        let DrawObject::Connector { from, to, .. } = object else {
            return true;
        };
        let (Some(from), Some(to)) = (from.as_deref(), to.as_deref()) else {
            return true;
        };
        let connects_declared_components =
            component_ids.contains(from) && component_ids.contains(to);
        !connects_declared_components || figure_plan_has_edge(Some(figure_plan), from, to)
    });
}

fn prune_residual_boxes_absent_from_figure_plan(plan: &mut DrawPlan, figure_plan: &FigurePlan) {
    if figure_plan.layout.template != Template::TeacherStudent {
        return;
    }
    let component_ids = figure_plan
        .components
        .iter()
        .map(|component| component.id.as_str())
        .collect::<HashSet<_>>();
    let has_declared_residual = figure_plan
        .components
        .iter()
        .any(|component| component_is_residual_like(component));
    if !has_declared_residual {
        return;
    }

    let extra_residual_ids = plan
        .objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Box { id, text, role, .. } = object else {
                return None;
            };
            (!component_ids.contains(id.as_str()) && is_extra_residual_box(id, text, role))
                .then(|| id.clone())
        })
        .collect::<HashSet<_>>();
    if extra_residual_ids.is_empty() {
        return;
    }

    plan.objects.retain(|object| match object {
        DrawObject::Box { id, .. } => !extra_residual_ids.contains(id),
        DrawObject::Connector { from, to, .. } => {
            !from
                .as_deref()
                .is_some_and(|id| extra_residual_ids.contains(id))
                && !to
                    .as_deref()
                    .is_some_and(|id| extra_residual_ids.contains(id))
        }
        _ => true,
    });
}

fn component_is_residual_like(component: &Component) -> bool {
    let text = format!("{} {} {:?}", component.id, component.label, component.role).to_lowercase();
    text.contains("residual") || text.contains("latent")
}

fn is_extra_residual_box(id: &str, text: &str, role: &str) -> bool {
    let text = format!("{id} {text} {role}").to_lowercase();
    (text.contains("residual") || text.contains("latent")) && !text.contains("inference")
}

fn reposition_note_components_near_sources(plan: &mut DrawPlan, figure_plan: &FigurePlan) {
    let box_map = current_box_map(plan);
    let note_component_ids = figure_plan
        .components
        .iter()
        .filter(|component| is_note_like_component(component))
        .map(|component| component.id.as_str())
        .collect::<HashSet<_>>();
    let mut moved_ids = HashSet::new();
    for component in &figure_plan.components {
        if !is_note_like_component(component) {
            continue;
        }
        let Some(bbox) = box_map.get(component.id.as_str()).copied() else {
            continue;
        };
        if !box_touches_outer_margin(bbox)
            && !note_component_conflicts_with_semantic_boxes(
                component.id.as_str(),
                bbox,
                &box_map,
                &note_component_ids,
            )
            && !note_component_conflicts_with_connector_segments(plan, component.id.as_str(), bbox)
        {
            continue;
        }
        let Some(source_box) = figure_plan
            .edges
            .iter()
            .find(|edge| edge.to == component.id)
            .and_then(|edge| box_map.get(edge.from.as_str()).copied())
            .or_else(|| orphan_note_component_source_box(component, figure_plan, &box_map))
        else {
            continue;
        };
        let next_bbox = clear_adjacent_note_box(
            component.id.as_str(),
            bbox,
            source_box,
            &box_map,
            &note_component_ids,
            plan,
        )
        .unwrap_or_else(|| adjacent_note_box(bbox, source_box));
        set_box_bbox(plan, &component.id, next_bbox);
        moved_ids.insert(component.id.clone());
    }
    if !moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    }
}

fn note_component_conflicts_with_semantic_boxes(
    note_id: &str,
    note_bbox: [f64; 4],
    box_map: &HashMap<String, [f64; 4]>,
    note_component_ids: &HashSet<&str>,
) -> bool {
    let expanded_note = expand_box(normalize_box(note_bbox), 0.04);
    box_map.iter().any(|(id, bbox)| {
        id != note_id
            && !note_component_ids.contains(id.as_str())
            && intersection_area(expanded_note, *bbox) > 0.0
    })
}

fn note_component_conflicts_with_connector_segments(
    plan: &DrawPlan,
    note_id: &str,
    note_bbox: [f64; 4],
) -> bool {
    connector_segment_boxes_excluding_endpoint(plan, note_id, 0.008)
        .iter()
        .any(|segment_box| intersection_area(note_bbox, *segment_box) > 0.0001)
}

fn clear_adjacent_note_box(
    note_id: &str,
    note_bbox: [f64; 4],
    source_bbox: [f64; 4],
    box_map: &HashMap<String, [f64; 4]>,
    note_component_ids: &HashSet<&str>,
    plan: &DrawPlan,
) -> Option<[f64; 4]> {
    let connector_segments = connector_segment_boxes_excluding_endpoint(plan, note_id, 0.008);
    adjacent_note_candidates(note_bbox, source_bbox)
        .into_iter()
        .find(|candidate| {
            note_candidate_is_clear(
                note_id,
                *candidate,
                box_map,
                note_component_ids,
                &connector_segments,
            )
        })
}

fn adjacent_note_candidates(note_bbox: [f64; 4], source_bbox: [f64; 4]) -> Vec<[f64; 4]> {
    let note_bbox = normalize_box(note_bbox);
    let source_bbox = normalize_box(source_bbox);
    let width = box_width(note_bbox).clamp(0.10, 0.22);
    let height = box_height(note_bbox).clamp(0.08, 0.14);
    let source_y = (center_y(source_bbox) - height / 2.0).clamp(0.06, 0.94 - height);
    let left_x = (source_bbox[0] - 0.04 - width).clamp(0.06, 0.94 - width);
    let right_x = (source_bbox[2] + 0.04).clamp(0.06, 0.94 - width);
    let left = [left_x, source_y, left_x + width, source_y + height];
    let right = [right_x, source_y, right_x + width, source_y + height];
    let source_x = (center_x(source_bbox) - width / 2.0).clamp(0.06, 0.94 - width);
    let below_y = (source_bbox[3] + 0.05).clamp(0.06, 0.94 - height);
    let above_y = (source_bbox[1] - 0.05 - height).clamp(0.06, 0.94 - height);
    let below = [source_x, below_y, source_x + width, below_y + height];
    let above = [source_x, above_y, source_x + width, above_y + height];
    let caption_x = (0.5 - width / 2.0).clamp(0.06, 0.94 - width);
    let top_caption = [caption_x, 0.06, caption_x + width, 0.06 + height];
    let bottom_caption = [caption_x, 0.94 - height, caption_x + width, 0.94];
    if center_x(source_bbox) <= 0.5 {
        vec![right, left, below, above, top_caption, bottom_caption]
    } else {
        vec![left, right, below, above, top_caption, bottom_caption]
    }
}

fn note_candidate_is_clear(
    note_id: &str,
    candidate: [f64; 4],
    box_map: &HashMap<String, [f64; 4]>,
    note_component_ids: &HashSet<&str>,
    connector_segments: &[[f64; 4]],
) -> bool {
    let expanded_candidate = expand_box(normalize_box(candidate), 0.035);
    box_map.iter().all(|(id, bbox)| {
        id == note_id
            || note_component_ids.contains(id.as_str())
            || intersection_area(expanded_candidate, *bbox) <= 0.0001
    }) && connector_segments
        .iter()
        .all(|segment_box| intersection_area(candidate, *segment_box) <= 0.0001)
}

fn move_inference_note_boxes_out_of_flow_corridors(plan: &mut DrawPlan) {
    let excluded_note_ids = HashSet::new();
    move_inference_note_boxes_out_of_flow_corridors_except(plan, &excluded_note_ids);
}

fn move_inference_note_boxes_out_of_flow_corridors_except(
    plan: &mut DrawPlan,
    excluded_note_ids: &HashSet<String>,
) {
    for pass in 0..2 {
        let box_route_map = current_box_route_info_map(plan);
        let box_map = current_box_map(plan);
        let mut note_boxes_and_text = inference_note_boxes_and_text(plan);
        note_boxes_and_text.retain(|id, _| !excluded_note_ids.contains(id.as_str()));
        if note_boxes_and_text.is_empty() {
            return;
        }
        let note_ids = note_boxes_and_text
            .iter()
            .map(|(id, _)| id.clone())
            .collect::<HashSet<_>>();
        if note_ids.is_empty() {
            return;
        }
        let note_id_refs = note_ids.iter().map(String::as_str).collect::<HashSet<_>>();
        let branch_pairs = teacher_student_branch_pairs(&box_route_map);
        let inference_anchors = inference_anchor_bboxes(&box_route_map);
        let mut moved_ids = HashSet::new();
        let mut made_progress = false;
        for note_id in &note_ids {
            let Some(note_bbox) = note_boxes_and_text.get(note_id.as_str()).copied() else {
                continue;
            };
            let in_corridor =
                inference_note_is_in_teacher_student_corridor(note_bbox, branch_pairs.as_slice());
            let conflicts =
                note_component_conflicts_with_connector_segments(plan, note_id, note_bbox);
            let anchored =
                compact_inference_note_is_anchored_to_student_or_output(note_bbox, &box_route_map);
            let mut needs_repair = conflicts || !anchored || in_corridor;
            if pass == 0 {
                needs_repair = conflicts || !anchored;
            }
            if !needs_repair {
                continue;
            }

            let Some(source_box) = nearest_inference_anchor_box(note_bbox, &box_route_map)
                .and_then(|id| box_map.get(id.as_str()).copied())
                .or_else(|| {
                    largest_student_box_id(plan, &note_ids)
                        .and_then(|id| box_map.get(id.as_str()).copied())
                })
            else {
                continue;
            };
            let strict = pass == 0;
            let next_bbox = clear_adjacent_inference_note_box(
                note_id,
                note_bbox,
                source_box,
                &inference_anchors,
                &box_map,
                &note_id_refs,
                plan,
                branch_pairs.as_slice(),
                strict,
            )
            .or_else(|| {
                if strict {
                    None
                } else {
                    clear_adjacent_inference_note_box(
                        note_id,
                        note_bbox,
                        source_box,
                        &inference_anchors,
                        &box_map,
                        &note_id_refs,
                        plan,
                        branch_pairs.as_slice(),
                        false,
                    )
                }
            })
            .or_else(|| {
                if strict {
                    None
                } else {
                    inference_note_outside_corridor_fallback_candidate(
                        note_id,
                        note_bbox,
                        source_box,
                        &box_map,
                        &note_id_refs,
                        plan,
                        branch_pairs.as_slice(),
                    )
                }
            })
            .unwrap_or_else(|| adjacent_note_box(note_bbox, source_box));
            if !boxes_nearly_equal(note_bbox, next_bbox) {
                set_object_bbox(plan, note_id, next_bbox);
                moved_ids.insert(note_id.clone());
                made_progress = true;
            }
        }
        if !moved_ids.is_empty() {
            realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
        }
        if !made_progress {
            return;
        }
    }
}

fn nearest_inference_anchor_box(
    note_bbox: [f64; 4],
    box_route_map: &HashMap<String, BoxRouteInfo>,
) -> Option<String> {
    let note_center = center(note_bbox);
    box_route_map
        .iter()
        .filter(|(id, info)| is_student_or_output_inference_anchor(id, info))
        .min_by(|left, right| {
            let left_distance = center_distance(note_center, center(left.1.bbox));
            let right_distance = center_distance(note_center, center(right.1.bbox));
            left_distance
                .partial_cmp(&right_distance)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(id, _)| id.clone())
}

fn inference_anchor_bboxes(box_route_map: &HashMap<String, BoxRouteInfo>) -> Vec<[f64; 4]> {
    box_route_map
        .iter()
        .filter_map(|(id, info)| {
            is_student_or_output_inference_anchor(id, info).then_some(info.bbox)
        })
        .collect()
}

fn center_distance(left: [f64; 2], right: [f64; 2]) -> f64 {
    let dx = left[0] - right[0];
    let dy = left[1] - right[1];
    (dx * dx + dy * dy).sqrt()
}

fn clear_adjacent_inference_note_box(
    note_id: &str,
    note_bbox: [f64; 4],
    source_bbox: [f64; 4],
    anchors: &[[f64; 4]],
    box_map: &HashMap<String, [f64; 4]>,
    note_component_ids: &HashSet<&str>,
    plan: &DrawPlan,
    branch_pairs: &[([f64; 4], [f64; 4])],
    disallow_teacher_student_corridor: bool,
) -> Option<[f64; 4]> {
    let connector_segments = connector_segment_boxes_excluding_endpoint(plan, note_id, 0.008);
    let mut best: Option<([f64; 4], f64)> = None;

    let anchor_sources = if anchors.is_empty() {
        vec![source_bbox]
    } else {
        anchors.to_vec()
    };

    for anchor in &anchor_sources {
        for candidate in adjacent_note_candidates(note_bbox, *anchor) {
            let candidate = normalize_box(candidate);
            if !note_candidate_is_clear(
                note_id,
                candidate,
                box_map,
                note_component_ids,
                &connector_segments,
            ) {
                continue;
            }
            if disallow_teacher_student_corridor
                && inference_note_is_in_teacher_student_corridor(candidate, branch_pairs)
            {
                continue;
            }

            let mut score = center_distance(center(candidate), center(note_bbox));
            if inference_note_is_in_teacher_student_corridor(candidate, branch_pairs) {
                score += if disallow_teacher_student_corridor {
                    99.0
                } else {
                    0.8
                };
            }

            if !candidate_is_near_inference_anchor(candidate, anchor_sources.as_slice()) {
                score += 0.20;
            }

            if let Some((_, best_score)) = best {
                if score >= best_score {
                    continue;
                }
            }
            best = Some((candidate, score));
        }
    }

    best.map(|(candidate, _)| candidate)
}

fn inference_note_outside_corridor_fallback_candidate(
    note_id: &str,
    note_bbox: [f64; 4],
    source_box: [f64; 4],
    box_map: &HashMap<String, [f64; 4]>,
    note_component_ids: &HashSet<&str>,
    plan: &DrawPlan,
    branch_pairs: &[([f64; 4], [f64; 4])],
) -> Option<[f64; 4]> {
    let width = box_width(note_bbox).clamp(0.10, 0.22);
    let height = box_height(note_bbox).clamp(0.08, 0.14);
    let source_center = center(source_box);
    if width <= 0.0 || height <= 0.0 {
        return None;
    }

    let mut candidates = Vec::new();
    let connector_segments = connector_segment_boxes_excluding_endpoint(plan, note_id, 0.008);
    for (teacher, student) in branch_pairs {
        let branch_union = union_box(*teacher, *student);
        let y = source_center[1] - height / 2.0;
        let side_y = branch_union[1] - height - 0.045;
        let bottom_y = branch_union[3] + 0.045;
        let far_left = branch_union[0] - 0.045 - width;
        let far_right = branch_union[2] + 0.045;
        let center_x = center(branch_union)[0] - width / 2.0;

        push_unique_box(
            &mut candidates,
            box_from_top_left_inside(far_left, y, width, height),
        );
        push_unique_box(
            &mut candidates,
            box_from_top_left_inside(far_right, y, width, height),
        );
        push_unique_box(
            &mut candidates,
            box_from_top_left_inside(center_x, side_y, width, height),
        );
        push_unique_box(
            &mut candidates,
            box_from_top_left_inside(center_x, bottom_y, width, height),
        );
        let anchor_x = source_center[0] - width - 0.04;
        let anchor_x2 = source_center[0] + 0.04;
        push_unique_box(
            &mut candidates,
            box_from_top_left_inside(anchor_x, y, width, height),
        );
        push_unique_box(
            &mut candidates,
            box_from_top_left_inside(anchor_x2, y, width, height),
        );
    }

    candidates
        .into_iter()
        .filter(|candidate| {
            !inference_note_is_in_teacher_student_corridor(*candidate, branch_pairs)
        })
        .filter(|candidate| {
            note_candidate_is_clear(
                note_id,
                *candidate,
                box_map,
                note_component_ids,
                &connector_segments,
            )
        })
        .min_by(|left, right| {
            box_center_distance(*left, note_bbox).total_cmp(&box_center_distance(*right, note_bbox))
        })
}

fn candidate_is_near_inference_anchor(candidate: [f64; 4], anchors: &[[f64; 4]]) -> bool {
    anchors.iter().any(|anchor| {
        horizontal_separation(candidate, *anchor) <= 0.25
            && vertical_separation(candidate, *anchor) <= 0.16
    })
}

fn connector_segment_boxes_excluding_endpoint(
    plan: &DrawPlan,
    excluded_id: &str,
    margin: f64,
) -> Vec<[f64; 4]> {
    plan.objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Connector {
                points, from, to, ..
            } = object
            else {
                return None;
            };
            if from.as_deref() == Some(excluded_id) || to.as_deref() == Some(excluded_id) {
                return None;
            }
            Some(points.windows(2).map(move |window| {
                expand_box(
                    [
                        window[0][0].min(window[1][0]),
                        window[0][1].min(window[1][1]),
                        window[0][0].max(window[1][0]),
                        window[0][1].max(window[1][1]),
                    ],
                    margin,
                )
            }))
        })
        .flatten()
        .collect()
}

fn orphan_note_component_source_box(
    note_component: &Component,
    figure_plan: &FigurePlan,
    box_map: &HashMap<String, [f64; 4]>,
) -> Option<[f64; 4]> {
    figure_plan
        .components
        .iter()
        .filter(|component| component.id != note_component.id)
        .filter_map(|component| {
            box_map
                .get(component.id.as_str())
                .copied()
                .map(|bbox| (component, bbox))
        })
        .min_by_key(|(component, _)| note_component_source_rank(component))
        .map(|(_, bbox)| bbox)
}

fn note_component_source_rank(component: &Component) -> i32 {
    let text = format!(
        "{} {} {:?} {:?}",
        component.id, component.label, component.role, component.visual_weight
    )
    .to_lowercase();
    if text.contains("student") || text.contains("main") {
        0
    } else if text.contains("model") || text.contains("module") {
        1
    } else {
        2
    }
}

fn is_note_like_component(component: &Component) -> bool {
    let text = format!(
        "{} {} {:?}",
        component.id.to_lowercase(),
        component.label.to_lowercase(),
        component.role
    )
    .to_lowercase();
    text.contains("note")
        || text.contains("inference")
        || text.contains("only")
        || text.contains("annotation")
}

fn box_touches_outer_margin(bbox: [f64; 4]) -> bool {
    let bbox = normalize_box(bbox);
    bbox[0] < 0.04 || bbox[1] < 0.04 || bbox[2] > 0.96 || bbox[3] > 0.96
}

fn adjacent_note_box(note_bbox: [f64; 4], source_bbox: [f64; 4]) -> [f64; 4] {
    let note_bbox = normalize_box(note_bbox);
    let source_bbox = normalize_box(source_bbox);
    let width = box_width(note_bbox).clamp(0.10, 0.22);
    let height = box_height(note_bbox).clamp(0.08, 0.14);
    let source_center = center(source_bbox);
    let x1 = if source_center[0] <= 0.5 {
        (source_bbox[2] + 0.04).min(0.90 - width)
    } else {
        (source_bbox[0] - 0.04 - width).max(0.10)
    };
    let y1 = (source_center[1] - height / 2.0).clamp(0.06, 0.90 - height);
    [x1, y1, x1 + width, y1 + height]
}

fn compact_inference_note_is_anchored_to_student_or_output(
    note_bbox: [f64; 4],
    box_map: &HashMap<String, BoxRouteInfo>,
) -> bool {
    box_map.iter().any(|(id, info)| {
        if !is_student_or_output_inference_anchor(id, info) {
            return false;
        }
        (horizontal_separation(note_bbox, info.bbox) <= 0.10
            && vertical_separation(note_bbox, info.bbox) <= 0.12)
            || (axis_overlap_ratio(note_bbox[1], note_bbox[3], info.bbox[1], info.bbox[3]) > 0.20
                && horizontal_separation(note_bbox, info.bbox) <= 0.12)
    })
}

fn route_box_text_matches_head_patterns(text: &str, role: &str, style: &str, id: &str) -> bool {
    let id = id.to_lowercase();
    let text = text.to_lowercase();
    let role = role.to_lowercase();
    let style = style.to_lowercase();
    (id.contains("head")
        || text.contains("head")
        || role.contains("head")
        || style.contains("head"))
        && !text.contains("loss")
}

fn is_student_or_output_inference_anchor(id: &str, info: &BoxRouteInfo) -> bool {
    let text = route_box_text(id, info);
    let is_student = text.contains("student") && !text.contains("inference");
    is_student
        || text.contains("output")
        || text.contains("prediction")
        || text.contains("pred")
        || text.contains("ŷ")
}

fn teacher_student_branch_pairs(
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Vec<([f64; 4], [f64; 4])> {
    let teachers = box_map
        .iter()
        .filter_map(|(_id, info)| is_teacher_branch_box(_id, info).then(|| info.bbox))
        .collect::<Vec<_>>();
    let students = box_map
        .iter()
        .filter_map(|(id, info)| is_student_like_branch_box(id, info).then(|| info.bbox))
        .collect::<Vec<_>>();
    if teachers.is_empty() || students.is_empty() {
        return Vec::new();
    }

    let mut pairs = Vec::new();
    for &teacher in &teachers {
        for &student in &students {
            if axis_overlap_ratio(teacher[0], teacher[2], student[0], student[2]) > 0.20 {
                pairs.push((teacher, student));
            }
        }
    }
    if pairs.is_empty() {
        for &student in &students {
            for &teacher in &teachers {
                pairs.push((teacher, student));
            }
        }
    }
    pairs
}

fn is_student_like_branch_box(id: &str, info: &BoxRouteInfo) -> bool {
    let text = route_box_text(id, info);
    text.contains("student")
        || id.to_lowercase().contains("student")
        || text.contains("compact")
        || text.contains("encoder")
        || text.contains("decoder")
}

fn inference_note_is_in_teacher_student_corridor(
    note: [f64; 4],
    branch_pairs: &[([f64; 4], [f64; 4])],
) -> bool {
    if branch_pairs.is_empty() {
        return false;
    }
    branch_pairs.iter().any(|(teacher, student)| {
        let upper = if center_y(*teacher) <= center_y(*student) {
            *teacher
        } else {
            *student
        };
        let lower = if center_y(*teacher) <= center_y(*student) {
            *student
        } else {
            *teacher
        };
        let note_center_y = center_y(note);
        if note_center_y <= upper[3] || note_center_y >= lower[1] {
            return false;
        }
        let branch_span = [
            upper[0].min(lower[0]),
            upper[3],
            upper[2].max(lower[2]),
            lower[1],
        ];
        (axis_overlap_ratio(note[0], note[2], branch_span[0], branch_span[2]) > 0.12)
            || (horizontal_separation(note, branch_span) <= 0.035)
    })
}

fn add_missing_connectors_from_figure_plan(plan: &mut DrawPlan, figure_plan: &FigurePlan) {
    let box_map = current_box_map(plan);
    for edge in &figure_plan.edges {
        if plan.objects.iter().any(|object| {
            matches!(
                object,
                DrawObject::Connector { id, from, to, .. }
                    if id == &edge.id
                        || (from.as_deref() == Some(edge.from.as_str())
                            && to.as_deref() == Some(edge.to.as_str()))
            )
        }) {
            continue;
        }
        let (Some(from_box), Some(to_box)) = (
            box_map.get(edge.from.as_str()).copied(),
            box_map.get(edge.to.as_str()).copied(),
        ) else {
            continue;
        };
        let from_center = center(from_box);
        let to_center = center(to_box);
        let start = anchor_point_towards(from_box, to_center);
        let end = anchor_point_towards(to_box, from_center);
        let label = (!edge.label.trim().is_empty()).then(|| DrawLabel {
            text: edge.label.clone(),
            bbox: connector_label_bbox(start, end),
        });
        plan.objects.push(DrawObject::Connector {
            id: unique_draw_object_id(plan, &edge.id),
            points: vec![start, end],
            from: Some(edge.from.clone()),
            to: Some(edge.to.clone()),
            style: figure_plan_edge_style(edge.semantic, edge.style, edge.importance).to_string(),
            label,
            z: next_z(plan),
        });
    }
}

fn repair_draw_plan_geometry_inner(plan: &mut DrawPlan, figure_plan: Option<&FigurePlan>) {
    if let Some(figure_plan) = figure_plan {
        prune_teacher_student_to_figure_plan(plan, figure_plan);
    }
    repair_teacher_student_lanes(plan, figure_plan);
    polish_labels_and_marginal_annotations(plan, false);
    compact_tall_output_boxes_for_short_labels(plan);
    compact_floating_inference_excluded_annotations(plan);
    simplify_student_to_task_loss_connectors(plan);
    // mock/repair 入口不会走完整 polish；这里必须在最终 route 后再吸附 label，
    // 否则避让逻辑会把 fan-in edge label 推到远离自身 connector 的空白区。
    snap_connector_labels_to_final_routes(plan);
    tighten_short_connector_labels_near_routes(plan);
    move_connector_labels_off_components(plan);
}

fn polish_labels_and_marginal_annotations(plan: &mut DrawPlan, aggressive_connector_polish: bool) {
    let component_union = union_bbox(plan.objects.iter().filter_map(|object| match object {
        DrawObject::Box { bbox, .. } => Some(*bbox),
        _ => None,
    }));
    plan.objects
        .retain(|object| !is_marginal_annotation(object, component_union));

    let mut label_boxes: Vec<[f64; 4]> = Vec::new();
    for object in &mut plan.objects {
        if let DrawObject::Connector { points, label, .. } = object {
            *points = if aggressive_connector_polish {
                polished_connector_points(points)
            } else {
                orthogonalized_points(points)
            };
            if let Some(label) = label {
                label.bbox = place_label_outside_edge(label.bbox, points);
                while label_boxes
                    .iter()
                    .any(|existing| boxes_overlap(*existing, label.bbox))
                {
                    label.bbox = shift_label(label.bbox, 0.04);
                }
                label_boxes.push(label.bbox);
            }
        }
    }
    avoid_connector_label_collisions(plan);
    remove_line_overlapping_annotations(plan);
}

fn fold_standalone_inference_notes_into_student_annotations(
    plan: &mut DrawPlan,
    protected_note_ids: &HashSet<String>,
) {
    let connected_ids = connector_endpoint_ids(plan);
    let note_ids = plan
        .objects
        .iter()
        .filter_map(|object| match object {
            DrawObject::Box {
                id,
                text,
                role,
                style,
                ..
            } => (!connected_ids.contains(id.as_str())
                && !protected_note_ids.contains(id)
                && is_standalone_inference_note_box(id, text, role, style))
            .then(|| id.clone()),
            _ => None,
        })
        .collect::<HashSet<_>>();
    if note_ids.is_empty() {
        return;
    }

    let student_id = largest_student_box_id(plan, &note_ids);
    let label = inference_annotation_label_from_notes(plan, &note_ids);
    let bbox = student_id
        .as_deref()
        .and_then(|id| current_box_map(plan).get(id).copied())
        .map(inference_annotation_bbox_near_student)
        .unwrap_or([0.62, 0.08, 0.90, 0.14]);
    plan.objects.retain(|object| match object {
        DrawObject::Box { id, .. } => !note_ids.contains(id),
        _ => true,
    });
    ensure_inference_annotation(plan, &label, bbox);
}

fn fold_detached_protected_inference_notes_into_student_annotations(
    plan: &mut DrawPlan,
    protected_note_ids: &HashSet<String>,
) {
    let connected_ids = connector_endpoint_ids(plan);
    let note_ids = plan
        .objects
        .iter()
        .filter_map(|object| match object {
            DrawObject::Box {
                id,
                bbox,
                text,
                role,
                style,
                ..
            } => (protected_note_ids.contains(id)
                && !connected_ids.contains(id.as_str())
                && is_standalone_inference_note_box(id, text, role, style)
                && protected_inference_note_has_foldable_role(role, style)
                && detached_protected_inference_note_should_fold(plan, id, *bbox))
            .then(|| id.clone()),
            _ => None,
        })
        .collect::<HashSet<_>>();
    if note_ids.is_empty() {
        return;
    }

    let student_id = largest_student_box_id(plan, &note_ids);
    let label = inference_annotation_label_from_notes(plan, &note_ids);
    let bbox = student_id
        .as_deref()
        .and_then(|id| current_box_map(plan).get(id).copied())
        .map(inference_annotation_bbox_near_student)
        .unwrap_or([0.62, 0.08, 0.90, 0.14]);
    plan.objects.retain(|object| match object {
        DrawObject::Box { id, .. } => !note_ids.contains(id),
        _ => true,
    });
    ensure_inference_annotation(plan, &label, bbox);
}

fn fold_unconnected_component_inference_notes_into_annotation(plan: &mut DrawPlan) {
    let connected_ids = connector_endpoint_ids(plan);
    let note_ids = plan
        .objects
        .iter()
        .filter_map(|object| match object {
            DrawObject::Box {
                id,
                text,
                role,
                style,
                ..
            } => (id.to_lowercase().contains("comp_inference")
                && !connected_ids.contains(id.as_str())
                && is_standalone_inference_note_box(id, text, role, style)
                && protected_inference_note_has_foldable_role(role, style))
            .then(|| id.clone()),
            _ => None,
        })
        .collect::<HashSet<_>>();
    if note_ids.is_empty() {
        return;
    }

    let student_id = largest_student_box_id(plan, &note_ids);
    let label = inference_annotation_label_from_notes(plan, &note_ids);
    let bbox = student_id
        .as_deref()
        .and_then(|id| current_box_map(plan).get(id).copied())
        .map(inference_annotation_bbox_near_student)
        .unwrap_or([0.62, 0.82, 0.90, 0.88]);
    plan.objects.retain(|object| match object {
        DrawObject::Box { id, .. } => !note_ids.contains(id),
        DrawObject::Connector { from, to, .. } => {
            !from.as_deref().is_some_and(|id| note_ids.contains(id))
                && !to.as_deref().is_some_and(|id| note_ids.contains(id))
        }
        _ => true,
    });
    ensure_inference_annotation(plan, &label, bbox);
}

fn protected_inference_note_has_foldable_role(role: &str, style: &str) -> bool {
    let role = role.to_lowercase();
    let style = style.to_lowercase();
    !role.contains("output") && (role.contains("context") || style.contains("muted"))
}

fn detached_protected_inference_note_should_fold(
    plan: &DrawPlan,
    note_id: &str,
    note_bbox: [f64; 4],
) -> bool {
    let note_id_lower = note_id.to_lowercase();
    if note_id_lower.contains("badge") || note_id_lower == "inference_note" {
        return false;
    }
    let crowds_loss_or_objective =
        protected_inference_note_crowds_loss_or_objective_area(plan, note_bbox);
    let lane_like = protected_inference_component_is_lane_like(note_id);
    let note_like = note_id_lower.contains("note");
    if !lane_like && !note_like {
        return crowds_loss_or_objective;
    }
    let excluded_ids = HashSet::new();
    let Some(student_id) = largest_student_box_id(plan, &excluded_ids) else {
        return false;
    };
    let Some(student_bbox) = current_box_map(plan).get(student_id.as_str()).copied() else {
        return false;
    };
    if lane_like {
        return true;
    }
    if note_bbox[3] < student_bbox[1] - 0.04 {
        return true;
    }
    if protected_inference_note_is_below_student_caption(note_bbox, student_bbox) {
        return true;
    }
    if protected_inference_note_is_student_adjacent_badge(note_bbox, student_bbox) {
        return false;
    }
    if note_id_lower.contains("note") {
        return true;
    }
    crowds_loss_or_objective
}

fn protected_inference_component_is_lane_like(note_id: &str) -> bool {
    let id = note_id.to_lowercase();
    id.contains("inference") && !id.contains("note") && !id.contains("badge")
}

fn protected_inference_note_is_student_adjacent_badge(
    note_bbox: [f64; 4],
    student_bbox: [f64; 4],
) -> bool {
    let (_, vertical_overlap) = intersection_dimensions(note_bbox, student_bbox);
    if vertical_overlap > 0.03 && horizontal_separation(note_bbox, student_bbox) <= 0.08 {
        return true;
    }
    false
}

fn protected_inference_note_is_below_student_caption(
    note_bbox: [f64; 4],
    student_bbox: [f64; 4],
) -> bool {
    note_bbox[1] >= student_bbox[3]
        && vertical_separation(note_bbox, student_bbox) <= 0.08
        && axis_overlap_ratio(note_bbox[0], note_bbox[2], student_bbox[0], student_bbox[2]) > 0.30
}

fn protected_inference_note_crowds_loss_or_objective_area(
    plan: &DrawPlan,
    note_bbox: [f64; 4],
) -> bool {
    let note_bbox = normalize_box(note_bbox);
    current_box_route_info_map(plan).iter().any(|(id, info)| {
        if !is_loss_or_objective_box(id, info) {
            return false;
        }
        let (_, vertical_overlap) = intersection_dimensions(note_bbox, info.bbox);
        vertical_overlap > 0.025 && horizontal_separation(note_bbox, info.bbox) <= 0.20
    })
}

fn fold_noisy_connected_inference_notes_into_student_annotations(
    plan: &mut DrawPlan,
    protected_note_ids: &HashSet<String>,
) {
    let mut note_ids = noisy_connected_inference_note_ids(plan);
    note_ids.retain(|id| !protected_note_ids.contains(id));
    if note_ids.is_empty() {
        return;
    }
    let student_id = student_id_for_connected_notes(plan, &note_ids)
        .or_else(|| largest_student_box_id(plan, &note_ids));
    let label = inference_annotation_label_from_notes(plan, &note_ids);
    let bbox = student_id
        .as_deref()
        .and_then(|id| current_box_map(plan).get(id).copied())
        .map(inference_annotation_bbox_near_student)
        .unwrap_or([0.62, 0.08, 0.90, 0.14]);
    plan.objects.retain(|object| match object {
        DrawObject::Box { id, .. } | DrawObject::Text { id, .. } => !note_ids.contains(id),
        DrawObject::Connector { from, to, .. } => {
            !from.as_deref().is_some_and(|id| note_ids.contains(id))
                && !to.as_deref().is_some_and(|id| note_ids.contains(id))
        }
        _ => true,
    });
    ensure_inference_annotation(plan, &label, bbox);
}

fn inference_annotation_label_from_notes(plan: &DrawPlan, note_ids: &HashSet<String>) -> String {
    let label = plan.objects.iter().find_map(|object| match object {
        DrawObject::Box { id, text, .. } | DrawObject::Text { id, text, .. }
            if note_ids.contains(id) && !text.trim().is_empty() =>
        {
            Some(text.trim().replace('\n', " "))
        }
        _ => None,
    });
    normalize_inference_annotation_label(label.as_deref())
}

fn normalize_inference_annotation_label(label: Option<&str>) -> String {
    let label = label.unwrap_or("Inference: student only").trim();
    let lower = label.to_lowercase();
    let normalized = lower.replace(['-', '_'], " ");
    if normalized.contains("student only")
        || normalized.contains("only student")
        || normalized.contains("inference only")
    {
        "Inference: student only".to_string()
    } else if lower.contains("inference") {
        label.to_string()
    } else {
        "Inference: student only".to_string()
    }
}

fn ensure_inference_annotation(plan: &mut DrawPlan, label: &str, bbox: [f64; 4]) {
    if let Some(index) = existing_inference_text_annotation_index(plan) {
        if let DrawObject::Text {
            bbox: object_bbox,
            text,
            style,
            ..
        } = &mut plan.objects[index]
        {
            *object_bbox = normalize_box(bbox);
            *text = label.to_string();
            *style = "annotation".to_string();
        }
        return;
    }
    let id = unique_draw_object_id(plan, "ann_inference");
    plan.objects.push(DrawObject::Text {
        id,
        bbox: normalize_box(bbox),
        text: label.to_string(),
        style: "annotation".to_string(),
        z: next_z(plan),
    });
}

fn existing_inference_text_annotation_index(plan: &DrawPlan) -> Option<usize> {
    plan.objects.iter().position(|object| {
        let DrawObject::Text {
            id, text, style, ..
        } = object
        else {
            return false;
        };
        let haystack = format!(
            "{} {} {}",
            id.to_lowercase(),
            text.to_lowercase(),
            style.to_lowercase()
        );
        haystack.contains("annotation") && haystack.contains("inference")
    })
}

fn remove_duplicate_inference_text_when_note_component_exists(plan: &mut DrawPlan) {
    let has_inference_note_box = plan.objects.iter().any(|object| {
        let DrawObject::Box { id, text, role, .. } = object else {
            return false;
        };
        let haystack = format!("{} {} {}", id, text, role).to_lowercase();
        haystack.contains("inference")
            && (haystack.contains("student only")
                || haystack.contains("only student")
                || haystack.contains("note"))
    });
    if !has_inference_note_box {
        return;
    }
    plan.objects.retain(|object| {
        let DrawObject::Text {
            id, text, style, ..
        } = object
        else {
            return true;
        };
        let haystack = format!("{} {} {}", id, text, style).to_lowercase();
        !(haystack.contains("inference")
            && (haystack.contains("student only") || haystack.contains("only student")))
    });
}

fn remove_overlapping_duplicate_inference_texts(plan: &mut DrawPlan) {
    let inference_texts = plan
        .objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Text { id, bbox, text, .. } = object else {
                return None;
            };
            text.to_lowercase()
                .contains("inference")
                .then(|| (id.clone(), *bbox, text.clone()))
        })
        .collect::<Vec<_>>();
    if inference_texts.len() < 2 {
        return;
    }

    let mut remove_ids = HashSet::new();
    for (index, (left_id, left_bbox, left_text)) in inference_texts.iter().enumerate() {
        for (right_id, right_bbox, right_text) in inference_texts.iter().skip(index + 1) {
            let overlap = intersection_area(*left_bbox, *right_bbox);
            if overlap <= 0.0 {
                continue;
            }
            let overlap_ratio =
                overlap / box_area(*left_bbox).min(box_area(*right_bbox)).max(0.0001);
            if overlap_ratio < 0.55 {
                continue;
            }
            remove_ids.insert(duplicate_inference_text_to_remove(
                left_id, left_text, right_id, right_text,
            ));
        }
    }
    if remove_ids.is_empty() {
        return;
    }
    plan.objects.retain(|object| match object {
        DrawObject::Text { id, .. } => !remove_ids.contains(id),
        _ => true,
    });
}

fn duplicate_inference_text_to_remove(
    left_id: &str,
    left_text: &str,
    right_id: &str,
    right_text: &str,
) -> String {
    let left_score = duplicate_inference_text_keep_score(left_id, left_text);
    let right_score = duplicate_inference_text_keep_score(right_id, right_text);
    if left_score >= right_score {
        right_id.to_string()
    } else {
        left_id.to_string()
    }
}

fn duplicate_inference_text_keep_score(id: &str, text: &str) -> i32 {
    let mut score = text.chars().count() as i32;
    let lower = format!("{} {}", id.to_lowercase(), text.to_lowercase());
    if lower.contains("student only") {
        score += 20;
    }
    if id.starts_with("ann_") || text.trim_end().ends_with('→') {
        score -= 10;
    }
    score
}

fn repair_comp_named_residual_task_feedback_topology(plan: &mut DrawPlan) {
    let box_map = current_box_map(plan);
    let Some(input_id) = find_box_id(plan, |id, text, role| {
        id.contains("input") || text.contains("input") || role.contains("input")
    }) else {
        return;
    };
    let Some(teacher_id) = find_box_id(plan, |id, text, _| {
        id.contains("teacher") || text.contains("teacher")
    }) else {
        return;
    };
    let Some(student_id) = find_box_id(plan, |id, text, _| {
        id.contains("student") || text.contains("student")
    }) else {
        return;
    };
    let Some(residual_id) = find_box_id(plan, |id, text, _| {
        id.contains("residual") || text.contains("residual")
    }) else {
        return;
    };
    let Some(task_loss_id) = find_box_id(plan, |id, text, role| {
        id.contains("taskloss")
            || id.contains("task_loss")
            || text.contains("task loss")
            || (role.contains("loss") && text.contains("task"))
    }) else {
        return;
    };
    let (Some(input), Some(teacher), Some(student), Some(residual), Some(task_loss)) = (
        box_map.get(&input_id).copied(),
        box_map.get(&teacher_id).copied(),
        box_map.get(&student_id).copied(),
        box_map.get(&residual_id).copied(),
        box_map.get(&task_loss_id).copied(),
    ) else {
        return;
    };
    let is_residual_task_feedback_layout = center_x(input) < center_x(teacher)
        && center_y(input) > center_y(teacher)
        && center_x(residual) < center_x(student)
        && center_y(residual) < center_y(student)
        && center_y(task_loss) < center_y(student);
    if !is_residual_task_feedback_layout {
        return;
    }

    for object in &mut plan.objects {
        let DrawObject::Connector {
            points,
            from,
            to,
            style,
            label,
            ..
        } = object
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        if from_id == input_id && to_id == teacher_id {
            *points = vec![[input[2], input[1]], [teacher[0], teacher[3]]];
            *style = "normal_flow".to_string();
            *label = None;
        } else if from_id == task_loss_id && to_id == student_id {
            let lane_x = (student[2] + 0.055).max(task_loss[2] + 0.040).min(0.96);
            let start = [task_loss[2], center_y(task_loss)];
            let end = [student[2], center_y(student)];
            *points = vec![start, [lane_x, start[1]], [lane_x, end[1]], end];
            *style = "dashed_supervision".to_string();
            if let Some(label) = label {
                label.bbox =
                    box_from_top_left_inside(lane_x + 0.012, task_loss[3] + 0.030, 0.105, 0.050);
            }
        } else if from_id == residual_id && to_id == student_id {
            let route_y = (residual[3] + 0.065)
                .min(student[1] - 0.035)
                .max(residual[3] + 0.035)
                .clamp(0.04, 0.96);
            let start = [center_x(residual), residual[3]];
            *points = vec![
                start,
                [start[0], route_y],
                [student[0], route_y],
                [student[0], student[1] + 0.030],
            ];
            *style = "dashed_supervision".to_string();
            if let Some(label) = label {
                label.bbox =
                    box_from_top_left_inside(start[0] + 0.018, route_y - 0.065, 0.105, 0.050);
            }
        }
    }
}

fn repair_student_head_output_task_loss_column(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut repair_target = None;
    for object in &plan.objects {
        let DrawObject::Connector { from, to, .. } = object else {
            continue;
        };
        let (Some(source_id), Some(output_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(source), Some(output)) = (box_map.get(source_id), box_map.get(output_id)) else {
            continue;
        };
        if !is_output_like(output_id, &output.text, &output.role)
            || output.bbox[1] <= source.bbox[3]
        {
            continue;
        }
        let loss_id = plan.objects.iter().find_map(|candidate| {
            let DrawObject::Connector {
                from: loss_from,
                to: loss_to,
                ..
            } = candidate
            else {
                return None;
            };
            if loss_from.as_deref() != Some(source_id) {
                return None;
            }
            let loss_id = loss_to.as_deref()?;
            let loss = box_map.get(loss_id)?;
            is_task_loss_route_box(loss_id, loss).then(|| loss_id.to_string())
        });
        let Some(loss_id) = loss_id else {
            continue;
        };
        repair_target = Some((source_id.to_string(), output_id.to_string(), loss_id));
        break;
    }
    let Some((source_id, output_id, loss_id)) = repair_target else {
        return;
    };
    let (Some(source), Some(output), Some(task_loss)) = (
        box_map.get(&source_id).map(|info| info.bbox),
        box_map.get(&output_id).map(|info| info.bbox),
        box_map.get(&loss_id).map(|info| info.bbox),
    ) else {
        return;
    };
    let output_lane_x = center_x(output).clamp(source[0] + 0.015, source[2] - 0.015);
    let vertical_min = source[3].min(output[1]);
    let vertical_max = source[3].max(output[1]);
    let lane_crosses_task_loss = output_lane_x >= task_loss[0] - 0.008
        && output_lane_x <= task_loss[2] + 0.008
        && vertical_max >= task_loss[1]
        && vertical_min <= task_loss[3];
    let task_loss_crowds_output = horizontal_separation(task_loss, output) < 0.015
        && vertical_separation(task_loss, output) < 0.040;
    if !lane_crosses_task_loss && !task_loss_crowds_output {
        return;
    }

    let width = box_width(task_loss).clamp(0.13, 0.17);
    let height = box_height(task_loss).clamp(0.085, 0.12);
    let candidate =
        box_from_top_left_inside(output[0] - width - 0.025, task_loss[1], width, height);
    set_box_bbox(plan, &loss_id, candidate);

    for object in &mut plan.objects {
        let DrawObject::Connector {
            points,
            from,
            to,
            style,
            label,
            ..
        } = object
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        if from_id == source_id && to_id == output_id {
            let x = center_x(output).clamp(source[0] + 0.015, source[2] - 0.015);
            *points = vec![[x, source[3]], [x, output[1]]];
            *style = "main_flow".to_string();
            *label = None;
        } else if from_id == source_id && to_id == loss_id {
            let x = center_x(source).clamp(source[0] + 0.015, source[2] - 0.015);
            let y = center_y(candidate);
            *points = vec![[x, source[3]], [x, y], [candidate[2], y]];
            *style = "normal_flow".to_string();
            *label = None;
        }
    }

    move_inference_annotations_away_from_output_column(plan, output, candidate);
}

fn move_inference_annotations_away_from_output_column(
    plan: &mut DrawPlan,
    output: [f64; 4],
    task_loss: [f64; 4],
) {
    let width = 0.16;
    let height = 0.055;
    let bbox = box_from_top_left_inside(task_loss[0], task_loss[3] + 0.025, width, height);
    for object in &mut plan.objects {
        let DrawObject::Text {
            bbox: object_bbox,
            text,
            ..
        } = object
        else {
            continue;
        };
        if !annotation_label_is_inference_specific(text) {
            continue;
        }
        if horizontal_separation(*object_bbox, output) < 0.025
            || vertical_separation(*object_bbox, output) < 0.030
        {
            *object_bbox = bbox;
        }
    }
}

fn move_residual_equation_annotations_out_of_main_corridors(plan: &mut DrawPlan) {
    let Some(residual_id) = find_box_id(plan, |id, text, _| {
        id.contains("residual") || text.contains("residual")
    }) else {
        return;
    };
    let Some(residual) = current_box_map(plan).get(&residual_id).copied() else {
        return;
    };
    for object in &mut plan.objects {
        let DrawObject::Text { bbox, text, .. } = object else {
            continue;
        };
        if !is_residual_equation_annotation_text(text) {
            continue;
        }
        let sits_left_of_residual = bbox[2] <= residual[0] + 0.025;
        let near_residual_row = vertical_separation(*bbox, residual) < 0.12;
        if !sits_left_of_residual || !near_residual_row {
            continue;
        }
        let width = box_width(*bbox).clamp(0.14, 0.19);
        let height = box_height(*bbox).clamp(0.050, 0.075);
        *bbox = box_from_top_left_inside(
            residual[2] + 0.025,
            center_y(residual) - height / 2.0,
            width,
            height,
        );
    }
}

fn is_residual_equation_annotation_text(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("||")
        || lower.contains("z_t")
        || lower.contains("z_s")
        || lower.contains("residual")
}

fn repair_projection_latent_teacher_student_topology(plan: &mut DrawPlan) {
    let box_map = current_box_map(plan);
    let required = [
        "input_text",
        "teacher_encode",
        "teacher_latent",
        "teacher_proj",
        "student_encode",
        "student_latent",
        "student_head",
        "output_text",
        "task_loss",
    ];
    if required.iter().any(|id| !box_map.contains_key(*id)) {
        return;
    }
    let input = box_map["input_text"];
    let teacher_latent = box_map["teacher_latent"];
    let teacher_proj = box_map["teacher_proj"];
    let needs_repair = teacher_latent[0] <= input[2] + 0.04
        || teacher_proj[0] <= input[2] + 0.04
        || connector_points_by_endpoint(plan, "input_text", "student_encode")
            .is_some_and(|points| points.len() >= 4 && same_y(points[0], *points.last().unwrap()));
    if !needs_repair {
        return;
    }

    let next_input = [0.05, 0.40, 0.15, 0.52];
    let next_teacher = [0.22, 0.18, 0.42, 0.30];
    let next_teacher_latent = [0.48, 0.18, 0.60, 0.30];
    let next_teacher_proj = [0.64, 0.18, 0.76, 0.30];
    let next_student = [0.22, 0.54, 0.42, 0.66];
    let next_student_latent = [0.48, 0.54, 0.60, 0.66];
    let next_student_head = [0.64, 0.54, 0.80, 0.66];
    let next_output = [0.85, 0.54, 0.95, 0.66];
    let next_task_loss = [0.84, 0.72, 0.97, 0.82];
    let next_inference = [0.48, 0.75, 0.74, 0.83];

    for (id, bbox) in [
        ("input_text", next_input),
        ("teacher_encode", next_teacher),
        ("teacher_latent", next_teacher_latent),
        ("teacher_proj", next_teacher_proj),
        ("student_encode", next_student),
        ("student_latent", next_student_latent),
        ("student_head", next_student_head),
        ("output_text", next_output),
        ("task_loss", next_task_loss),
    ] {
        set_box_bbox(plan, id, bbox);
    }
    upsert_or_replace_box_object(
        plan,
        "inference_note",
        next_inference,
        "Inference: student only",
        "context",
        "muted_module",
    );

    for object in &mut plan.objects {
        let DrawObject::Connector {
            points,
            from,
            to,
            style,
            label,
            ..
        } = object
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        match (from_id, to_id) {
            ("input_text", "teacher_encode") => {
                *points = vec![
                    [next_input[2], center_y(next_input)],
                    [next_teacher[0], center_y(next_input)],
                    [next_teacher[0], center_y(next_teacher)],
                ];
                *style = "normal_flow".to_string();
                *label = None;
            }
            ("input_text", "student_encode") => {
                *points = vec![
                    [next_input[2], center_y(next_input)],
                    [next_student[0], center_y(next_input)],
                    [next_student[0], center_y(next_student)],
                ];
                *style = "normal_flow".to_string();
                *label = None;
            }
            ("teacher_encode", "teacher_latent") => {
                *points = vec![
                    [next_teacher[2], center_y(next_teacher)],
                    [next_teacher_latent[0], center_y(next_teacher_latent)],
                ];
                *style = "normal_flow".to_string();
                *label = None;
            }
            ("teacher_latent", "teacher_proj") => {
                *points = vec![
                    [next_teacher_latent[2], center_y(next_teacher_latent)],
                    [next_teacher_proj[0], center_y(next_teacher_proj)],
                ];
                *style = "normal_flow".to_string();
                *label = None;
            }
            ("student_encode", "student_latent") => {
                *points = vec![
                    [next_student[2], center_y(next_student)],
                    [next_student_latent[0], center_y(next_student_latent)],
                ];
                *style = "normal_flow".to_string();
                *label = None;
            }
            ("student_latent", "student_head") => {
                *points = vec![
                    [next_student_latent[2], center_y(next_student_latent)],
                    [next_student_head[0], center_y(next_student_head)],
                ];
                *style = "normal_flow".to_string();
                *label = None;
            }
            ("student_head", "output_text") => {
                *points = vec![
                    [next_student_head[2], center_y(next_student_head)],
                    [next_output[0], center_y(next_output)],
                ];
                *style = "main_flow".to_string();
                *label = None;
            }
            ("student_head", "task_loss") => {
                let y = center_y(next_task_loss);
                *points = vec![
                    [center_x(next_student_head), next_student_head[3]],
                    [center_x(next_student_head), y],
                    [next_task_loss[0], y],
                ];
                *style = "normal_flow".to_string();
                *label = None;
            }
            ("teacher_proj", "student_latent") => {
                let lane_x = center_x(next_teacher_proj);
                *points = vec![
                    [lane_x, next_teacher_proj[3]],
                    [lane_x, center_y(next_student_latent)],
                    [next_student_latent[0], center_y(next_student_latent)],
                ];
                *style = "dashed_supervision".to_string();
                *label = Some(DrawLabel {
                    text: "Latent Residual".to_string(),
                    bbox: [0.43, 0.38, 0.59, 0.43],
                });
            }
            _ => {}
        }
    }
}

fn repair_split_duplicate_input_residual_hub_topology(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let required = [
        "teacher_input",
        "student_input",
        "teacher_encode",
        "teacher_latent",
        "residual_obj",
        "student_encode",
        "student_latent",
        "student_decode",
        "task_output",
        "task_loss",
    ];
    if required.iter().any(|id| !box_map.contains_key(*id)) {
        return;
    }
    let Some(inference_note) = inference_note_like_object_bbox(plan) else {
        return;
    };
    let teacher_input = &box_map["teacher_input"];
    let student_input = &box_map["student_input"];
    if !duplicate_task_input_pair(teacher_input, student_input) {
        return;
    }
    let teacher_latent = box_map["teacher_latent"].bbox;
    let student_latent = box_map["student_latent"].bbox;
    let has_floating_branch_labels = text_object_id_exists(plan, "branch_label_teacher")
        || text_object_id_exists(plan, "branch_label_student");
    let residual_is_wandering =
        connector_points_by_endpoint(plan, "teacher_latent", "student_latent").is_some_and(
            |points| {
                points.len() > 2
                    || !points
                        .first()
                        .zip(points.last())
                        .is_some_and(|(start, end)| same_y(*start, *end))
            },
        );
    let needs_repair = has_floating_branch_labels
        || residual_is_wandering
        || box_height(inference_note) < 0.08
        || inference_note[0] < 0.70
        || (center_y(teacher_latent) - center_y(student_latent)).abs() > 0.025;
    if !needs_repair {
        return;
    }

    let next_input = [0.42, 0.76, 0.54, 0.86];
    let next_teacher = [0.15, 0.45, 0.39, 0.57];
    let next_teacher_latent = [0.18, 0.20, 0.35, 0.30];
    let next_residual = [0.42, 0.20, 0.60, 0.30];
    let next_student = [0.58, 0.45, 0.76, 0.57];
    let next_student_latent = [0.67, 0.20, 0.84, 0.30];
    let next_student_decode = [0.80, 0.45, 0.94, 0.57];
    let next_output = [0.80, 0.64, 0.94, 0.74];
    let next_task_loss = [0.81, 0.82, 0.93, 0.92];
    let next_inference = [0.75, 0.04, 0.97, 0.13];

    for (id, bbox) in [
        ("teacher_input", next_input),
        ("teacher_encode", next_teacher),
        ("teacher_latent", next_teacher_latent),
        ("residual_obj", next_residual),
        ("student_encode", next_student),
        ("student_latent", next_student_latent),
        ("student_decode", next_student_decode),
        ("task_output", next_output),
        ("task_loss", next_task_loss),
    ] {
        set_box_bbox(plan, id, bbox);
    }

    plan.objects.retain(|object| match object {
        DrawObject::Text { id, .. }
            if id == "branch_label_teacher"
                || id == "branch_label_student"
                || id == "ann_inference" =>
        {
            false
        }
        DrawObject::Box { id, .. } if id == "student_input" => false,
        DrawObject::Connector { id, .. } if id == "e_residual" => false,
        _ => true,
    });
    upsert_or_replace_box_object(
        plan,
        "inference_note",
        next_inference,
        "Inference: student only",
        "context",
        "muted_module",
    );

    for object in &mut plan.objects {
        let DrawObject::Connector {
            id,
            points,
            from,
            to,
            style,
            label,
            ..
        } = object
        else {
            continue;
        };
        if from.as_deref() == Some("student_input") {
            *from = Some("teacher_input".to_string());
        }
        if to.as_deref() == Some("student_input") {
            *to = Some("teacher_input".to_string());
        }
        match id.as_str() {
            "e_teacher_in" => {
                *points = vec![
                    [center_x(next_input), next_input[1]],
                    [center_x(next_teacher), next_input[1]],
                    [center_x(next_teacher), next_teacher[3]],
                ];
                *style = "normal_flow".to_string();
                *label = None;
            }
            "e_teacher_enc" => {
                *points = vec![
                    [center_x(next_teacher), next_teacher[1]],
                    [center_x(next_teacher_latent), next_teacher_latent[3]],
                ];
                *style = "main_flow".to_string();
                *label = None;
            }
            "e_student_in" => {
                *points = vec![
                    [center_x(next_input), next_input[1]],
                    [center_x(next_student), next_input[1]],
                    [center_x(next_student), next_student[3]],
                ];
                *style = "normal_flow".to_string();
                *label = None;
            }
            "e_student_enc" => {
                *points = vec![
                    [center_x(next_student), next_student[1]],
                    [center_x(next_student_latent), next_student_latent[3]],
                ];
                *style = "main_flow".to_string();
                *label = None;
            }
            "e_student_dec" => {
                *points = vec![
                    [center_x(next_student_latent), next_student_latent[3]],
                    [center_x(next_student_decode), next_student_latent[3]],
                    [center_x(next_student_decode), next_student_decode[1]],
                ];
                *style = "main_flow".to_string();
                *label = None;
            }
            "e_student_out" => {
                *points = vec![
                    [center_x(next_student_decode), next_student_decode[3]],
                    [center_x(next_output), next_output[1]],
                ];
                *style = "main_flow".to_string();
                *label = None;
            }
            "e_task_loss" => {
                *points = vec![
                    [center_x(next_output), next_output[3]],
                    [center_x(next_task_loss), next_task_loss[1]],
                ];
                *style = "normal_flow".to_string();
                *label = None;
            }
            "e_residual_up" => {
                *points = vec![
                    [next_student_latent[0], center_y(next_student_latent)],
                    [next_residual[2], center_y(next_residual)],
                ];
                *style = "dashed_supervision".to_string();
                *label = None;
            }
            "e_residual_down" => {
                *points = vec![
                    [next_residual[0], center_y(next_residual)],
                    [next_teacher_latent[2], center_y(next_teacher_latent)],
                ];
                *style = "dashed_supervision".to_string();
                *label = None;
            }
            _ => {}
        }
    }
}

fn duplicate_task_input_pair(left: &BoxRouteInfo, right: &BoxRouteInfo) -> bool {
    let left_text = left.text.to_lowercase();
    let right_text = right.text.to_lowercase();
    left.role.to_lowercase().contains("input")
        && right.role.to_lowercase().contains("input")
        && left_text.contains("task")
        && left_text.contains("input")
        && right_text.contains("task")
        && right_text.contains("input")
}

fn text_object_id_exists(plan: &DrawPlan, target_id: &str) -> bool {
    plan.objects
        .iter()
        .any(|object| matches!(object, DrawObject::Text { id, .. } if id == target_id))
}

fn repair_simple_teacher_student_branch_compaction(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let required = [
        "task_input",
        "teacher_branch",
        "student_branch",
        "task_output",
    ];
    if required.iter().any(|id| !box_map.contains_key(*id)) {
        return;
    }
    if !box_map.contains_key("student_branch")
        || !connector_points_by_endpoint(plan, "teacher_branch", "student_branch")
            .or_else(|| connector_points_by_endpoint(plan, "student_branch", "teacher_branch"))
            .is_some()
    {
        return;
    }

    let task_input = &box_map["task_input"];
    let teacher = &box_map["teacher_branch"];
    let student = &box_map["student_branch"];
    let output = &box_map["task_output"];
    let searchable = format!(
        "{} {} {} {}",
        route_box_text("task_input", task_input),
        route_box_text("teacher_branch", teacher),
        route_box_text("student_branch", student),
        route_box_text("task_output", output)
    );
    if !(searchable.contains("task")
        && searchable.contains("input")
        && searchable.contains("teacher")
        && searchable.contains("student"))
    {
        return;
    }

    let input_teacher_detour = connector_points_by_endpoint(plan, "task_input", "teacher_branch")
        .is_some_and(|points| points.len() > 2);
    let input_student_detour = connector_points_by_endpoint(plan, "task_input", "student_branch")
        .is_some_and(|points| points.len() > 2);
    let needs_repair = box_height(teacher.bbox) > 0.115
        || box_height(student.bbox) > 0.135
        || box_height(task_input.bbox) > 0.105
        || box_height(output.bbox) > 0.090
        || input_teacher_detour
        || input_student_detour;
    if !needs_repair {
        return;
    }

    let next_input = [0.11133333333333334, 0.325, 0.26366666666666666, 0.415];
    let next_teacher = [0.43166666666666664, 0.06, 0.6516666666666666, 0.16];
    let next_student = [0.42866666666666664, 0.535, 0.6546666666666666, 0.655];
    let student_center_x = center_x(next_student);
    let next_output = [
        student_center_x - 0.064,
        0.80,
        student_center_x + 0.064,
        0.88,
    ];
    for (id, bbox) in [
        ("task_input", next_input),
        ("teacher_branch", next_teacher),
        ("student_branch", next_student),
        ("task_output", next_output),
    ] {
        set_box_bbox(plan, id, bbox);
    }

    for object in &mut plan.objects {
        match object {
            DrawObject::Connector {
                id,
                points,
                style,
                label,
                ..
            } if id == "input_to_teacher" => {
                *points = vec![
                    [next_input[2], center_y(next_input)],
                    [next_teacher[0], center_y(next_teacher)],
                ];
                *style = "normal_flow".to_string();
                *label = None;
            }
            DrawObject::Connector {
                id,
                points,
                style,
                label,
                ..
            } if id == "input_to_student" => {
                *points = vec![
                    [next_input[2], center_y(next_input)],
                    [next_student[0], center_y(next_student)],
                ];
                *style = "main_flow".to_string();
                *label = None;
            }
            DrawObject::Connector {
                id,
                points,
                style,
                label,
                ..
            } if id == "student_to_output" => {
                *points = vec![
                    [center_x(next_student), next_student[3]],
                    [center_x(next_output), next_output[1]],
                ];
                *style = "main_flow".to_string();
                *label = None;
            }
            DrawObject::Connector {
                id,
                points,
                style,
                label,
                ..
            } if id == "latent_residual_edge" => {
                let x = center_x(next_student);
                *points = vec![[x, next_teacher[3]], [x, next_student[1]]];
                *style = "dashed_supervision".to_string();
                if let Some(label) = label {
                    label.bbox = [x + 0.035, 0.315, x + 0.195, 0.365];
                }
            }
            DrawObject::Text { id, bbox, .. } if id == "anno_student_inference" => {
                *bbox = [
                    next_student[2] + 0.012,
                    0.572,
                    next_student[2] + 0.172,
                    0.624,
                ];
            }
            _ => {}
        }
    }
}

fn repair_teacher_student_residual_node_main_route_crossing(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let Some(input_id) = find_box_id(plan, |id, text, role| {
        id.contains("input") || text.contains("input") || role.contains("input")
    }) else {
        return;
    };
    let Some(teacher_id) = find_box_id(plan, |id, text, role| {
        (id.contains("teacher") || text.contains("teacher") || role.contains("context"))
            && !id.contains("student")
            && !text.contains("student")
    }) else {
        return;
    };
    let Some(student_id) = find_box_id(plan, |id, text, role| {
        id.contains("student") || text.contains("student") || role.contains("student")
    }) else {
        return;
    };
    let Some(residual_id) = find_box_id(plan, |id, text, role| {
        id.contains("residual")
            || text.contains("residual")
            || id.contains("latent")
            || text.contains("latent")
            || (role.contains("loss") && text.contains("residual"))
    }) else {
        return;
    };

    let (Some(input), Some(teacher), Some(student), Some(residual)) = (
        box_map.get(&input_id),
        box_map.get(&teacher_id),
        box_map.get(&student_id),
        box_map.get(&residual_id),
    ) else {
        return;
    };
    if !(center_x(input.bbox) < center_x(teacher.bbox)
        && center_x(teacher.bbox) < center_x(student.bbox)
        && center_x(residual.bbox) < center_x(student.bbox))
    {
        return;
    }

    let input_student_points = connector_points_by_endpoint(plan, &input_id, &student_id).cloned();
    let teacher_residual_points =
        connector_points_by_endpoint(plan, &teacher_id, &residual_id).cloned();
    let Some(input_student_points) = input_student_points else {
        return;
    };
    let crosses_teacher_residual = teacher_residual_points
        .as_ref()
        .is_some_and(|points| polylines_cross(&input_student_points, points));
    let residual_on_main_route =
        polyline_passes_near_box(&input_student_points, residual.bbox, 0.006);
    if !crosses_teacher_residual && !residual_on_main_route {
        return;
    }

    let gap_left = teacher.bbox[2] + 0.035;
    let gap_right = student.bbox[0] - 0.035;
    let gap_width = gap_right - gap_left;
    if gap_width < 0.12 {
        return;
    }
    let target_width = box_width(residual.bbox).clamp(0.12, gap_width.min(0.16));
    let target_height = box_height(residual.bbox).clamp(0.075, 0.11);
    let x1 =
        ((gap_left + gap_right - target_width) / 2.0).clamp(gap_left, gap_right - target_width);
    let route_y = center_y(teacher.bbox).clamp(teacher.bbox[1] + 0.025, teacher.bbox[3] - 0.025);
    let max_y1 = student.bbox[1] - target_height - 0.015;
    if max_y1 < 0.04 {
        return;
    }
    let y1 = (route_y - target_height / 2.0).clamp(0.04, max_y1);
    let next_residual = normalize_box([x1, y1, x1 + target_width, y1 + target_height]);
    if !route_box_candidate_is_clear(&residual_id, next_residual, &box_map) {
        return;
    }

    set_box_bbox(plan, &residual_id, next_residual);
    let moved_ids = HashSet::from([residual_id.clone()]);
    let next_box_map = current_box_map(plan);
    let (Some(input), Some(teacher), Some(student), Some(residual)) = (
        next_box_map.get(&input_id).copied(),
        next_box_map.get(&teacher_id).copied(),
        next_box_map.get(&student_id).copied(),
        next_box_map.get(&residual_id).copied(),
    ) else {
        return;
    };
    let input_student_y = center_y(input).clamp(student[1] + 0.025, student[3] - 0.025);
    let residual_y = center_y(residual);

    for object in &mut plan.objects {
        let DrawObject::Connector {
            points,
            from,
            to,
            style,
            label,
            ..
        } = object
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        if from_id == input_id && to_id == student_id {
            *points = remove_redundant_collinear_points(&[
                [input[2], input_student_y],
                [student[0], input_student_y],
            ]);
            *style = "main_flow".to_string();
            *label = None;
        } else if from_id == teacher_id && to_id == residual_id {
            *points = remove_redundant_collinear_points(&[
                [teacher[2], residual_y],
                [residual[0], residual_y],
            ]);
            *style = "normal_flow".to_string();
            *label = None;
        } else if from_id == residual_id && to_id == student_id {
            *points = remove_redundant_collinear_points(&[
                [residual[2], residual_y],
                [student[0], residual_y],
                [student[0], student[1]],
            ]);
            *style = "dashed_supervision".to_string();
            if let Some(label) = label {
                label.bbox = box_from_top_left_inside(
                    (residual[2] + 0.012).min(student[0] - 0.12),
                    (residual_y - 0.060).max(0.04),
                    0.105,
                    0.045,
                );
            }
        }
    }
    realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
}

fn polyline_passes_near_box(points: &[[f64; 2]], bbox: [f64; 4], margin: f64) -> bool {
    let bbox = expand_box(bbox, margin);
    points.windows(2).any(|window| {
        let segment_box = expand_box(points_to_box(window), margin);
        intersection_area(segment_box, bbox) > 0.0
    })
}

fn repair_right_edge_student_output_loss_lane(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let Some(student_id) = find_box_id(plan, |id, text, role| {
        (id.contains("student") || text.contains("student") || role.contains("student"))
            && !id.contains("input")
            && !id.contains("output")
            && !id.contains("loss")
            && !text.contains("input")
            && !text.contains("output")
            && !text.contains("loss")
    }) else {
        return;
    };
    let Some(student_input_id) = find_box_id(plan, |id, text, role| {
        (id.contains("student") || text.contains("student"))
            && (id.contains("input") || text.contains("input") || role.contains("input"))
    }) else {
        return;
    };
    let Some(output_id) = find_box_id(plan, |id, text, role| {
        ((id.contains("student") && id.contains("output"))
            || text.contains("answer")
            || (role.contains("output") && text.contains("answer")))
            && !text.contains("loss")
            && !text.contains("prediction")
            && !id.contains("pred")
    }) else {
        return;
    };
    let Some(task_loss_id) = find_box_id(plan, |id, text, role| {
        id.contains("task_loss")
            || text.contains("task loss")
            || (role.contains("loss") && text.contains("task"))
    }) else {
        return;
    };
    let Some(residual_id) = find_box_id(plan, |id, text, role| {
        id.contains("residual")
            || text.contains("residual")
            || (role.contains("loss") && text.contains("supervision"))
    }) else {
        return;
    };

    let (Some(student), Some(student_input), Some(output), Some(task_loss), Some(_residual)) = (
        box_map.get(&student_id),
        box_map.get(&student_input_id),
        box_map.get(&output_id),
        box_map.get(&task_loss_id),
        box_map.get(&residual_id),
    ) else {
        return;
    };
    let residual_student_points =
        connector_points_by_endpoint(plan, &residual_id, &student_id).cloned();
    let student_input_points =
        connector_points_by_endpoint(plan, &student_input_id, &student_id).cloned();
    let output_loss_points = connector_points_by_endpoint(plan, &output_id, &task_loss_id).cloned();

    let residual_crosses_output = residual_student_points
        .as_deref()
        .is_some_and(|points| polyline_passes_near_box(points, output.bbox, 0.004));
    let loss_crosses_student_input = student_input_points
        .as_deref()
        .zip(output_loss_points.as_deref())
        .is_some_and(|(input_points, loss_points)| polylines_cross(input_points, loss_points));
    let right_edge_trap = student.bbox[2] > 0.97
        && output.bbox[0] <= student.bbox[2]
        && task_loss.bbox[0] <= student.bbox[2];
    if !right_edge_trap && !residual_crosses_output && !loss_crosses_student_input {
        return;
    }

    let student_height = box_height(student.bbox).clamp(0.16, 0.20);
    let student_width = box_width(student.bbox).clamp(0.13, 0.15);
    let student_center_y = center_y(student.bbox).clamp(0.40, 0.58);
    let next_student = normalize_box([
        0.70,
        student_center_y - student_height / 2.0,
        0.70 + student_width,
        student_center_y + student_height / 2.0,
    ]);
    let input_width = box_width(student_input.bbox).clamp(0.12, 0.15);
    let input_height = box_height(student_input.bbox).clamp(0.09, 0.12);
    let input_gap = 0.02;
    let next_student_input = normalize_box([
        next_student[0] - input_gap - input_width,
        center_y(next_student) - input_height / 2.0,
        next_student[0] - input_gap,
        center_y(next_student) + input_height / 2.0,
    ]);
    let output_width = box_width(output.bbox).clamp(0.105, 0.125);
    let output_height = box_height(output.bbox).clamp(0.09, 0.11);
    let output_x1 = (next_student[2] + 0.035).min(0.98 - output_width);
    let next_output = normalize_box([
        output_x1,
        center_y(next_student) - output_height / 2.0,
        output_x1 + output_width,
        center_y(next_student) + output_height / 2.0,
    ]);
    let loss_width = box_width(task_loss.bbox).clamp(0.11, 0.13);
    let loss_height = box_height(task_loss.bbox).clamp(0.09, 0.11);
    let loss_x1 = (center_x(next_output) - loss_width / 2.0).clamp(0.02, 0.98 - loss_width);
    let loss_y1 = (next_student[3] + 0.105).clamp(0.60, 0.98 - loss_height);
    let next_task_loss = normalize_box([
        loss_x1,
        loss_y1,
        loss_x1 + loss_width,
        loss_y1 + loss_height,
    ]);

    let moving_ids = HashSet::from([
        student_id.clone(),
        student_input_id.clone(),
        output_id.clone(),
        task_loss_id.clone(),
    ]);
    for (id, candidate) in [
        (&student_id, next_student),
        (&student_input_id, next_student_input),
        (&output_id, next_output),
        (&task_loss_id, next_task_loss),
    ] {
        if !route_box_candidate_is_clear_except(id, candidate, &box_map, &moving_ids) {
            return;
        }
    }

    for (id, bbox) in [
        (&student_id, next_student),
        (&student_input_id, next_student_input),
        (&output_id, next_output),
        (&task_loss_id, next_task_loss),
    ] {
        set_box_bbox(plan, id, bbox);
    }
    let next_box_map = current_box_map(plan);
    let (Some(student), Some(student_input), Some(output), Some(task_loss), Some(residual)) = (
        next_box_map.get(&student_id).copied(),
        next_box_map.get(&student_input_id).copied(),
        next_box_map.get(&output_id).copied(),
        next_box_map.get(&task_loss_id).copied(),
        next_box_map.get(&residual_id).copied(),
    ) else {
        return;
    };
    let student_lane_y = center_y(student).clamp(student_input[1] + 0.02, student_input[3] - 0.02);
    let residual_y = center_y(residual);
    for object in &mut plan.objects {
        let DrawObject::Connector {
            points,
            from,
            to,
            style,
            label,
            ..
        } = object
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        if from_id == student_input_id && to_id == student_id {
            *points = vec![
                [student_input[2], student_lane_y],
                [student[0], student_lane_y],
            ];
            *style = "normal_flow".to_string();
            *label = None;
        } else if from_id == residual_id && to_id == student_id {
            *points = remove_redundant_collinear_points(&[
                [residual[2], residual_y],
                [student[0], residual_y],
                [student[0], student[1]],
            ]);
            *style = "dashed_supervision".to_string();
            *label = None;
        } else if from_id == student_id && to_id == output_id {
            *points = vec![
                [student[2], center_y(student)],
                [output[0], center_y(output)],
            ];
            *style = "normal_flow".to_string();
            *label = None;
        } else if from_id == output_id && to_id == task_loss_id {
            let x = center_x(output);
            *points = vec![[x, output[3]], [x, task_loss[1]]];
            *style = "normal_flow".to_string();
            *label = None;
        }
    }
}

fn repair_compact_teacher_student_y_branch_annotations(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let Some(input_id) = find_box_id(plan, |id, text, role| {
        id.contains("input") || text.contains("input") || role.contains("input")
    }) else {
        return;
    };
    let Some(teacher_id) = find_box_id(plan, |id, text, role| {
        (id.contains("teacher") || text.contains("teacher") || role.contains("context"))
            && !id.contains("student")
            && !text.contains("student")
    }) else {
        return;
    };
    let Some(student_id) = find_box_id(plan, |id, text, role| {
        (id.contains("student") || text.contains("student") || role.contains("student"))
            && !id.contains("input")
            && !id.contains("output")
            && !id.contains("loss")
            && !text.contains("input")
            && !text.contains("output")
            && !text.contains("loss")
            && !text.contains("inference")
    }) else {
        return;
    };
    let Some(residual_id) = find_box_id(plan, |id, text, role| {
        id.contains("residual")
            || text.contains("residual")
            || id.contains("latent")
            || text.contains("latent")
            || (role.contains("loss") && text.contains("res"))
    }) else {
        return;
    };
    let Some(output_id) = find_box_id(plan, |id, text, role| {
        id.contains("output")
            || text.contains("output")
            || text.contains('ŷ')
            || text.contains("answer")
            || role.contains("output")
    }) else {
        return;
    };

    let (Some(input), Some(teacher), Some(student), Some(residual), Some(output)) = (
        box_map.get(&input_id),
        box_map.get(&teacher_id),
        box_map.get(&student_id),
        box_map.get(&residual_id),
        box_map.get(&output_id),
    ) else {
        return;
    };
    if !(center_x(input.bbox) < center_x(teacher.bbox)
        && center_x(teacher.bbox) < center_x(student.bbox)
        && center_x(residual.bbox) > center_x(teacher.bbox)
        && center_x(residual.bbox) < center_x(student.bbox) + 0.02)
    {
        return;
    }

    let input_student_points = connector_points_by_endpoint(plan, &input_id, &student_id).cloned();
    let input_teacher_points = connector_points_by_endpoint(plan, &input_id, &teacher_id).cloned();
    let note = standalone_inference_note_blocking_main_route(
        plan,
        input_student_points.as_deref(),
        input.bbox,
        student.bbox,
    );
    let loss_label_edge = loss_label_between_output_and_student(plan, &output_id, &student_id);
    let input_student_detour = input_student_points
        .as_deref()
        .is_some_and(|points| points.len() > 2);
    let input_teacher_detour = input_teacher_points
        .as_deref()
        .is_some_and(|points| points.len() > 2);
    if note.is_none() && loss_label_edge.is_none() && !input_student_detour && !input_teacher_detour
    {
        return;
    }

    let input_bbox = input.bbox;
    let teacher_bbox = teacher.bbox;
    let student_bbox = student.bbox;
    let output_bbox = output.bbox;
    if let Some((note_id, note_text, note_bbox)) = note {
        let width = box_width(note_bbox).clamp(0.14, 0.18);
        let height = box_height(note_bbox).clamp(0.045, 0.065);
        let x1 = (output_bbox[2] + 0.018).min(0.98 - width);
        let y1 = (output_bbox[1] - height - 0.035).clamp(0.04, 0.98 - height);
        let next_bbox = box_from_top_left_inside(x1, y1, width, height);
        convert_box_to_annotation(
            plan,
            &note_id,
            next_bbox,
            &inference_annotation_label_from_notes_text(&note_text),
        );
    }

    if let Some((edge_id, label_text, edge_points)) = loss_label_edge {
        let width = 0.09;
        let height = 0.050;
        let route_x = edge_points
            .iter()
            .map(|point| point[0])
            .fold(center_x(output_bbox), f64::max);
        let x1 = (route_x - width - 0.045).clamp(0.04, 0.96 - width);
        let y1 =
            ((output_bbox[1] + student_bbox[3]) / 2.0 - height / 2.0).clamp(0.04, 0.98 - height);
        upsert_text_annotation(
            plan,
            &format!("{edge_id}_cue"),
            [x1, y1, x1 + width, y1 + height],
            &label_text,
            "annotation",
        );
        clear_connector_label(plan, &edge_id);
    }

    for object in &mut plan.objects {
        let DrawObject::Connector {
            points,
            from,
            to,
            style,
            label,
            ..
        } = object
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        if from_id == input_id && to_id == student_id {
            *points = remove_redundant_collinear_points(&[
                [input_bbox[2], center_y(input_bbox)],
                [student_bbox[0], center_y(input_bbox)],
            ]);
            *style = "main_flow".to_string();
            *label = None;
        } else if from_id == input_id && to_id == teacher_id {
            *points = remove_redundant_collinear_points(&[
                [input_bbox[2], center_y(input_bbox)],
                [teacher_bbox[0], center_y(teacher_bbox)],
            ]);
            *style = "normal_flow".to_string();
            *label = None;
        }
    }
}

fn standalone_inference_note_blocking_main_route(
    plan: &DrawPlan,
    main_route: Option<&[[f64; 2]]>,
    input_bbox: [f64; 4],
    student_bbox: [f64; 4],
) -> Option<(String, String, [f64; 4])> {
    let connected_ids = connector_endpoint_ids(plan);
    plan.objects.iter().find_map(|object| {
        let DrawObject::Box {
            id,
            bbox,
            text,
            role,
            style,
            ..
        } = object
        else {
            return None;
        };
        if connected_ids.contains(id.as_str())
            || !is_standalone_inference_note_box(id, text, role, style)
            || !(main_route.is_some_and(|points| polyline_passes_near_box(points, *bbox, 0.006))
                || inference_note_sits_in_input_student_corridor(*bbox, input_bbox, student_bbox))
        {
            return None;
        }
        Some((id.clone(), text.clone(), *bbox))
    })
}

fn inference_note_sits_in_input_student_corridor(
    note: [f64; 4],
    input: [f64; 4],
    student: [f64; 4],
) -> bool {
    let left = input[2].min(student[2]);
    let right = input[0].max(student[0]);
    if right <= left || note[0] < left - 0.01 || note[2] > right + 0.01 {
        return false;
    }
    let corridor_y1 = input[1].min(student[1]);
    let corridor_y2 = input[3].max(student[3]);
    axis_overlap_ratio(note[1], note[3], corridor_y1, corridor_y2) > 0.35
}

fn loss_label_between_output_and_student(
    plan: &DrawPlan,
    output_id: &str,
    student_id: &str,
) -> Option<(String, String, Vec<[f64; 2]>)> {
    plan.objects.iter().find_map(|object| {
        let DrawObject::Connector {
            id,
            points,
            from,
            to,
            label,
            ..
        } = object
        else {
            return None;
        };
        let label_text = label.as_ref()?.text.clone();
        if !annotation_label_mentions_task_loss(&label_text) {
            return None;
        }
        let endpoints_match = (from.as_deref() == Some(output_id)
            && to.as_deref() == Some(student_id))
            || (from.as_deref() == Some(student_id) && to.as_deref() == Some(output_id));
        endpoints_match.then(|| (id.clone(), label_text, points.clone()))
    })
}

fn annotation_label_mentions_task_loss(label: &str) -> bool {
    let lower = label.to_lowercase();
    lower.contains("loss") || lower.contains("l_task") || lower.contains("ltask")
}

fn inference_annotation_label_from_notes_text(text: &str) -> String {
    let lower = text.to_lowercase();
    if lower.contains("student") {
        text.replace('\n', ": ")
    } else {
        "Inference: student only".to_string()
    }
}

fn convert_box_to_annotation(plan: &mut DrawPlan, target_id: &str, bbox: [f64; 4], text: &str) {
    for object in &mut plan.objects {
        let DrawObject::Box { id, z, .. } = object else {
            continue;
        };
        if id != target_id {
            continue;
        }
        *object = DrawObject::Text {
            id: target_id.to_string(),
            bbox: normalize_box(bbox),
            text: text.to_string(),
            style: "annotation".to_string(),
            z: *z,
        };
        return;
    }
}

fn upsert_text_annotation(
    plan: &mut DrawPlan,
    target_id: &str,
    bbox: [f64; 4],
    text: &str,
    style: &str,
) {
    let bbox = normalize_box(bbox);
    for object in &mut plan.objects {
        if draw_object_id(object) != target_id {
            continue;
        }
        match object {
            DrawObject::Text {
                bbox: object_bbox,
                text: object_text,
                style: object_style,
                ..
            } => {
                *object_bbox = bbox;
                *object_text = text.to_string();
                *object_style = style.to_string();
            }
            DrawObject::Box { z, .. } => {
                let z = *z;
                *object = DrawObject::Text {
                    id: target_id.to_string(),
                    bbox,
                    text: text.to_string(),
                    style: style.to_string(),
                    z,
                };
            }
            _ => {}
        }
        return;
    }
    plan.objects.push(DrawObject::Text {
        id: target_id.to_string(),
        bbox,
        text: text.to_string(),
        style: style.to_string(),
        z: next_z(plan),
    });
}

fn clear_connector_label(plan: &mut DrawPlan, target_id: &str) {
    for object in &mut plan.objects {
        let DrawObject::Connector { id, label, .. } = object else {
            continue;
        };
        if id == target_id {
            *label = None;
            return;
        }
    }
}

fn anchor_training_only_annotations_near_teacher(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let teacher_candidate = box_map
        .iter()
        .filter(|(id, info)| route_box_text(id, info).contains("teacher"))
        .min_by(|left, right| box_area(left.1.bbox).total_cmp(&box_area(right.1.bbox)))
        .or_else(|| {
            box_map
                .iter()
                .filter(|(id, info)| is_teacher_or_context_route_box(id, info))
                .min_by(|left, right| box_area(left.1.bbox).total_cmp(&box_area(right.1.bbox)))
        });
    let Some((teacher_id, teacher)) = teacher_candidate else {
        return;
    };
    let teacher_center = center(teacher.bbox);
    let updates = plan
        .objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Text { id, bbox, text, .. } = object else {
                return None;
            };
            if !is_teacher_state_annotation_label(id, text) {
                return None;
            }
            let current = normalize_box(*bbox);
            let current_distance = center_distance(center(current), teacher_center);
            if current_distance <= 0.30 && current[2] <= 0.94 && current[3] <= 0.94 {
                return None;
            }
            let width = box_width(current).clamp(0.13, 0.17);
            let height = box_height(current).clamp(0.045, 0.060);
            let candidates = [
                box_from_top_left_inside(
                    teacher.bbox[2] + 0.020,
                    teacher.bbox[1] + 0.020,
                    width,
                    height,
                ),
                box_from_top_left_inside(
                    teacher.bbox[2] + 0.020,
                    center_y(teacher.bbox) - height / 2.0,
                    width,
                    height,
                ),
                box_from_top_left_inside(
                    center_x(teacher.bbox) - width / 2.0,
                    teacher.bbox[3] + 0.025,
                    width,
                    height,
                ),
                box_from_top_left_inside(
                    center_x(teacher.bbox) - width / 2.0,
                    teacher.bbox[1] - height - 0.025,
                    width,
                    height,
                ),
            ];
            let Some(candidate) = candidates
                .into_iter()
                .filter(|candidate| center_distance(center(*candidate), teacher_center) <= 0.30)
                .filter(|candidate| {
                    route_box_candidate_is_clear_except(
                        id,
                        *candidate,
                        &box_map,
                        &HashSet::from([teacher_id.clone()]),
                    )
                })
                .min_by(|left, right| {
                    center_distance(center(*left), center(current))
                        .total_cmp(&center_distance(center(*right), center(current)))
                })
            else {
                return None;
            };
            Some((id.clone(), candidate))
        })
        .collect::<Vec<_>>();

    for (id, bbox) in updates {
        set_object_bbox(plan, &id, bbox);
    }
}

fn is_teacher_state_annotation_label(id: &str, text: &str) -> bool {
    let identity = format!("{} {}", id.to_lowercase(), text.to_lowercase());
    let phrase = normalized_annotation_phrase(text);
    (identity.contains("training") && identity.contains("only"))
        || matches!(phrase.as_str(), "frozen" | "teacher frozen")
        || id.to_lowercase().contains("freeze")
}

fn inference_note_like_object_bbox(plan: &DrawPlan) -> Option<[f64; 4]> {
    plan.objects.iter().find_map(|object| match object {
        DrawObject::Box { id, bbox, text, .. } | DrawObject::Text { id, bbox, text, .. } => {
            let searchable = format!("{} {}", id.to_lowercase(), text.to_lowercase());
            (searchable.contains("inference") && searchable.contains("student")).then_some(*bbox)
        }
        _ => None,
    })
}

fn connector_points_by_endpoint<'a>(
    plan: &'a DrawPlan,
    from_id: &str,
    to_id: &str,
) -> Option<&'a Vec<[f64; 2]>> {
    plan.objects.iter().find_map(|object| {
        let DrawObject::Connector {
            from, to, points, ..
        } = object
        else {
            return None;
        };
        (from.as_deref() == Some(from_id) && to.as_deref() == Some(to_id)).then_some(points)
    })
}

fn upsert_or_replace_box_object(
    plan: &mut DrawPlan,
    target_id: &str,
    bbox: [f64; 4],
    text: &str,
    role: &str,
    style: &str,
) {
    for object in &mut plan.objects {
        match object {
            DrawObject::Box {
                id,
                bbox: object_bbox,
                text: object_text,
                role: object_role,
                style: object_style,
                ..
            } if id == target_id => {
                *object_bbox = bbox;
                *object_text = text.to_string();
                *object_role = role.to_string();
                *object_style = style.to_string();
                return;
            }
            DrawObject::Text { id, z, .. } if id == target_id => {
                let object_z = *z;
                *object = DrawObject::Box {
                    id: target_id.to_string(),
                    bbox,
                    text: text.to_string(),
                    role: role.to_string(),
                    style: style.to_string(),
                    z: object_z,
                };
                return;
            }
            _ => {}
        }
    }
    plan.objects.push(DrawObject::Box {
        id: target_id.to_string(),
        bbox,
        text: text.to_string(),
        role: role.to_string(),
        style: style.to_string(),
        z: next_z(plan),
    });
}

fn restore_missing_inference_components_as_annotations(
    plan: &mut DrawPlan,
    figure_plan: &FigurePlan,
) {
    if !figure_plan_has_residual_task_feedback_pattern(figure_plan) {
        return;
    }
    let Some(student_id) = find_box_id(plan, |id, text, _| {
        id.contains("student") || text.contains("student")
    }) else {
        return;
    };
    let Some(student) = current_box_map(plan).get(&student_id).copied() else {
        return;
    };
    for component in &figure_plan.components {
        let signature = format!("{} {}", component.id, component.label).to_lowercase();
        if !signature.contains("inference") && !signature.contains("student only") {
            continue;
        }
        let bbox = box_from_top_left_inside(
            student[0],
            student[3] + 0.035,
            box_width(student).clamp(0.16, 0.22),
            0.060,
        );
        let text = fallback_label(&component.label, "Inference: student only");
        let mut restored_existing = false;
        for object in &mut plan.objects {
            match object {
                DrawObject::Text {
                    id,
                    bbox: object_bbox,
                    text: object_text,
                    style,
                    ..
                } if id == &component.id => {
                    *object_bbox = bbox;
                    *object_text = text.clone();
                    *style = "annotation".to_string();
                    restored_existing = true;
                    break;
                }
                DrawObject::Box { id, z, .. } if id == &component.id => {
                    let object_z = *z;
                    *object = DrawObject::Text {
                        id: component.id.clone(),
                        bbox,
                        text: text.clone(),
                        style: "annotation".to_string(),
                        z: object_z,
                    };
                    restored_existing = true;
                    break;
                }
                _ => {}
            }
        }
        if restored_existing {
            continue;
        }
        plan.objects.push(DrawObject::Text {
            id: component.id.clone(),
            bbox,
            text,
            style: "annotation".to_string(),
            z: next_z(plan),
        });
    }
}

fn figure_plan_has_residual_task_feedback_pattern(figure_plan: &FigurePlan) -> bool {
    let component_text = figure_plan
        .components
        .iter()
        .map(|component| {
            (
                component.id.as_str(),
                format!("{} {}", component.id, component.label).to_lowercase(),
            )
        })
        .collect::<Vec<_>>();
    let Some(student_id) = component_text
        .iter()
        .find_map(|(id, text)| text.contains("student").then_some(*id))
    else {
        return false;
    };
    let Some(residual_id) = component_text
        .iter()
        .find_map(|(id, text)| text.contains("residual").then_some(*id))
    else {
        return false;
    };
    let Some(task_loss_id) = component_text.iter().find_map(|(id, text)| {
        (text.contains("taskloss") || text.contains("task loss") || text.contains("l_task"))
            .then_some(*id)
    }) else {
        return false;
    };
    figure_plan_has_edge(Some(figure_plan), residual_id, student_id)
        && figure_plan_has_edge(Some(figure_plan), task_loss_id, student_id)
}

fn restore_missing_inference_components_as_boxes(plan: &mut DrawPlan, figure_plan: &FigurePlan) {
    if figure_plan_has_residual_task_feedback_pattern(figure_plan) {
        return;
    }
    if !figure_plan_has_student_self_loop_loss(figure_plan) {
        return;
    }
    let Some(student_id) = find_box_id(plan, |id, text, _| {
        id.contains("student") || text.contains("student")
    }) else {
        return;
    };
    let Some(student) = current_box_map(plan).get(&student_id).copied() else {
        return;
    };
    let mut restored_any = false;
    for component in &figure_plan.components {
        let signature = format!("{} {}", component.id, component.label).to_lowercase();
        if !signature.contains("inference") && !signature.contains("student only") {
            continue;
        }
        if plan
            .objects
            .iter()
            .any(|object| draw_object_id(object) == component.id)
        {
            continue;
        }
        let bbox = box_from_top_left_inside(
            student[0],
            student[3] + 0.035,
            box_width(student).clamp(0.16, 0.22),
            0.070,
        );
        plan.objects.push(DrawObject::Box {
            id: component.id.clone(),
            bbox,
            text: fallback_label(&component.label, "Inference: student only"),
            role: "context".to_string(),
            style: "muted_module".to_string(),
            z: next_z(plan),
        });
        restored_any = true;
    }
    if restored_any {
        plan.objects.retain(|object| match object {
            DrawObject::Text { id, text, .. } => {
                let lower = format!("{} {}", id.to_lowercase(), text.to_lowercase());
                !(lower.contains("inference") || lower.contains("outside main flow"))
            }
            _ => true,
        });
    }
}

fn figure_plan_has_student_self_loop_loss(figure_plan: &FigurePlan) -> bool {
    let Some(student_id) = figure_plan.components.iter().find_map(|component| {
        let text = format!("{} {}", component.id, component.label).to_lowercase();
        text.contains("student").then(|| component.id.as_str())
    }) else {
        return false;
    };
    figure_plan.edges.iter().any(|edge| {
        edge.from == student_id
            && edge.to == student_id
            && (edge.semantic == EdgeSemantic::Loss || edge.label.to_lowercase().contains("loss"))
    })
}

fn remove_duplicate_inference_note_boxes_when_annotation_exists(plan: &mut DrawPlan) {
    if existing_inference_text_annotation_index(plan).is_none() {
        return;
    }
    let connected_ids = connector_endpoint_ids(plan);
    let duplicate_note_ids = plan
        .objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Box {
                id,
                bbox,
                text,
                role,
                style,
                ..
            } = object
            else {
                return None;
            };
            (!connected_ids.contains(id.as_str())
                && is_standalone_inference_note_box(id, text, role, style)
                && duplicate_inference_note_box_should_yield_to_annotation(id, *bbox))
            .then(|| id.clone())
        })
        .collect::<HashSet<_>>();
    if duplicate_note_ids.is_empty() {
        return;
    }
    plan.objects.retain(|object| match object {
        DrawObject::Box { id, .. } => !duplicate_note_ids.contains(id),
        DrawObject::Connector { from, to, .. } => {
            !from
                .as_deref()
                .is_some_and(|id| duplicate_note_ids.contains(id))
                && !to
                    .as_deref()
                    .is_some_and(|id| duplicate_note_ids.contains(id))
        }
        _ => true,
    });
}

fn duplicate_inference_note_box_should_yield_to_annotation(id: &str, bbox: [f64; 4]) -> bool {
    let id = id.to_lowercase();
    id.contains("inference")
        && (id.contains("note") || id.contains("tag"))
        && (box_area(bbox) <= 0.05 || bbox[1] >= 0.65)
}

fn remove_auxiliary_inference_note_connectors(plan: &mut DrawPlan) {
    let inference_note_ids = plan
        .objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Box {
                id,
                text,
                role,
                style,
                ..
            } = object
            else {
                return None;
            };
            is_standalone_inference_note_box(id, text, role, style).then(|| id.clone())
        })
        .collect::<HashSet<_>>();
    if inference_note_ids.is_empty() {
        return;
    }

    plan.objects.retain(|object| {
        let DrawObject::Connector {
            id,
            from,
            to,
            style,
            label,
            ..
        } = object
        else {
            return true;
        };
        let touches_inference_note = from
            .as_deref()
            .is_some_and(|id| inference_note_ids.contains(id))
            || to
                .as_deref()
                .is_some_and(|id| inference_note_ids.contains(id));
        if !touches_inference_note {
            return true;
        }
        let descriptor = format!(
            "{} {} {}",
            id.to_lowercase(),
            style.to_lowercase(),
            label
                .as_ref()
                .map(|label| label.text.to_lowercase())
                .unwrap_or_default()
        );
        !(descriptor.contains("hint")
            || descriptor.contains("reference")
            || descriptor.contains("dash"))
    });
}

fn remove_redundant_phase_loss_and_inference_notes(plan: &mut DrawPlan) {
    let student_label_already_carries_inference =
        plan_has_student_box_with_inference_only_semantics(plan);
    let semantic_box_text = normalized_semantic_box_text(plan);
    let duplicate_inference_note_ids = if student_label_already_carries_inference {
        plan.objects
            .iter()
            .filter_map(|object| {
                let DrawObject::Box {
                    id,
                    text,
                    role,
                    style,
                    ..
                } = object
                else {
                    return None;
                };
                is_standalone_inference_note_box(id, text, role, style).then(|| id.clone())
            })
            .collect::<HashSet<_>>()
    } else {
        HashSet::new()
    };

    plan.objects.retain(|object| match object {
        DrawObject::Box { id, .. } => !duplicate_inference_note_ids.contains(id),
        DrawObject::Connector { from, to, .. } => {
            !from
                .as_deref()
                .is_some_and(|id| duplicate_inference_note_ids.contains(id))
                && !to
                    .as_deref()
                    .is_some_and(|id| duplicate_inference_note_ids.contains(id))
        }
        DrawObject::Text {
            id, text, style, ..
        } => {
            if !is_annotation_like_text(id, style) {
                true
            } else if (is_phase_only_annotation_label(text)
                && annotation_phrase_is_already_in_semantic_boxes(text, &semantic_box_text))
                || (is_redundant_frozen_annotation_label(text)
                    && semantic_box_text.contains("frozen"))
                || is_generic_task_loss_annotation_label(text)
                || is_generic_path_signal_annotation_label(text)
                || (is_redundant_residual_signal_annotation_label(text)
                    && semantic_box_text.contains("residual"))
            {
                false
            } else {
                !(student_label_already_carries_inference
                    && annotation_label_is_inference_specific(text))
            }
        }
        _ => true,
    });
}

fn remove_inference_annotations_overlapping_other_annotations(plan: &mut DrawPlan) {
    let other_annotation_boxes = plan
        .objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Text {
                text, bbox, style, ..
            } = object
            else {
                return None;
            };
            (style.to_lowercase().contains("annotation")
                && !annotation_label_is_inference_specific(text))
            .then_some(*bbox)
        })
        .collect::<Vec<_>>();
    if other_annotation_boxes.is_empty() {
        return;
    }
    plan.objects.retain(|object| {
        let DrawObject::Text {
            text, bbox, style, ..
        } = object
        else {
            return true;
        };
        !(style.to_lowercase().contains("annotation")
            && annotation_label_is_inference_specific(text)
            && other_annotation_boxes
                .iter()
                .any(|other| intersection_area(*bbox, *other) > 0.001))
    });
}

fn remove_duplicate_connector_label_annotations(plan: &mut DrawPlan) {
    let connector_label_phrases = plan
        .objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Connector { label, .. } = object else {
                return None;
            };
            let label = label.as_ref()?;
            let phrase = normalized_annotation_phrase(&label.text);
            (!phrase.is_empty()).then_some(phrase)
        })
        .collect::<HashSet<_>>();
    if connector_label_phrases.is_empty() {
        return;
    }
    plan.objects.retain(|object| {
        let DrawObject::Text { text, style, .. } = object else {
            return true;
        };
        let phrase = normalized_annotation_phrase(text);
        !(style.to_lowercase().contains("annotation")
            && connector_label_phrases.contains(&phrase)
            && !annotation_label_is_inference_specific(text))
    });
}

fn normalized_semantic_box_text(plan: &DrawPlan) -> String {
    plan.objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Box {
                id,
                text,
                role,
                style,
                ..
            } = object
            else {
                return None;
            };
            (!is_standalone_inference_note_box(id, text, role, style))
                .then(|| normalized_annotation_phrase(&format!("{id} {text} {role} {style}")))
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn annotation_phrase_is_already_in_semantic_boxes(label: &str, semantic_box_text: &str) -> bool {
    let phrase = normalized_annotation_phrase(label);
    !phrase.is_empty() && semantic_box_text.contains(&phrase)
}

fn plan_has_student_box_with_inference_only_semantics(plan: &DrawPlan) -> bool {
    plan.objects.iter().any(|object| {
        let DrawObject::Box {
            id,
            text,
            role,
            style,
            ..
        } = object
        else {
            return false;
        };
        !is_standalone_inference_note_box(id, text, role, style)
            && is_student_or_primary_box(id, text, role, style)
            && contains_inference_only_semantics(&format!("{id} {text}"))
    })
}

fn is_student_or_primary_box(id: &str, text: &str, role: &str, style: &str) -> bool {
    let haystack = format!("{id} {text} {role} {style}").to_lowercase();
    haystack.contains("student")
        || haystack.contains("main")
        || haystack.contains("primary")
        || haystack.contains("compact")
}

fn contains_inference_only_semantics(value: &str) -> bool {
    let normalized = normalized_annotation_phrase(value);
    normalized.contains("student only")
        || normalized.contains("only student")
        || normalized.contains("inference only")
}

fn is_annotation_like_text(id: &str, style: &str) -> bool {
    let id = id.to_lowercase();
    let style = style.to_lowercase();
    style.contains("annotation")
        || style.contains("phase")
        || id.starts_with("ann_")
        || id.starts_with("anno_")
        || id.starts_with("phase_")
        || id.starts_with("a_")
}

fn is_phase_only_annotation_label(label: &str) -> bool {
    let words = normalized_annotation_words(label);
    if words.is_empty() {
        return false;
    }
    let mut has_phase_word = false;
    words.iter().all(|word| {
        let is_phase = matches!(
            word.as_str(),
            "training" | "train" | "inference" | "infer" | "testing" | "test"
        );
        if is_phase {
            has_phase_word = true;
        }
        is_phase || matches!(word.as_str(), "only" | "phase" | "mode")
    }) && has_phase_word
}

fn is_redundant_frozen_annotation_label(label: &str) -> bool {
    matches!(normalized_annotation_phrase(label).as_str(), "frozen")
}

fn is_generic_task_loss_annotation_label(label: &str) -> bool {
    matches!(
        normalized_annotation_phrase(label).as_str(),
        "task loss" | "loss"
    )
}

fn is_generic_path_signal_annotation_label(label: &str) -> bool {
    matches!(
        normalized_annotation_phrase(label).as_str(),
        "auxiliary training signal"
            | "training signal"
            | "main inference path"
            | "inference path"
            | "supervision"
            | "supervision signal"
            | "latent supervision"
            | "residual supervision"
            | "main supervision"
    )
}

fn is_redundant_residual_signal_annotation_label(label: &str) -> bool {
    let words = normalized_annotation_words(label);
    words.iter().any(|word| word == "residual")
        && words.iter().all(|word| {
            matches!(
                word.as_str(),
                "residual" | "signal" | "supervision" | "supervise" | "latent" | "feature"
            )
        })
}

fn normalized_annotation_phrase(value: &str) -> String {
    normalized_annotation_words(value).join(" ")
}

fn normalized_annotation_words(value: &str) -> Vec<String> {
    value
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_alphanumeric() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

fn inference_annotation_bbox_near_student(student_bbox: [f64; 4]) -> [f64; 4] {
    let student_bbox = normalize_box(student_bbox);
    let width = 0.28;
    let height = 0.06;
    let x1 = center_x(student_bbox).clamp(0.06 + width / 2.0, 0.94 - width / 2.0) - width / 2.0;
    let y1 = if student_bbox[3] + height + 0.04 <= 0.94 {
        student_bbox[3] + 0.04
    } else {
        student_bbox[1] - height - 0.04
    }
    .clamp(0.06, 0.94 - height);
    [x1, y1, x1 + width, y1 + height]
}

fn upsert_meaningful_annotations_from_figure_plan(plan: &mut DrawPlan, figure_plan: &FigurePlan) {
    for annotation in &figure_plan.annotations {
        if fold_generic_task_loss_annotation_into_connector_label(
            plan,
            figure_plan,
            &annotation.label,
            annotation.target_id.as_deref(),
        ) {
            continue;
        }
        let Some(bbox) = annotation.bbox else {
            continue;
        };
        if !is_meaningful_figure_plan_annotation(&annotation.label) {
            continue;
        }
        let bbox = anchored_figure_plan_annotation_bbox(
            plan,
            &annotation.label,
            annotation.target_id.as_deref(),
            bbox,
        );
        let existing_index = plan
            .objects
            .iter()
            .position(|object| draw_object_id(object) == annotation.id)
            .or_else(|| {
                annotation_label_is_inference_specific(&annotation.label)
                    .then(|| existing_inference_text_annotation_index(plan))
                    .flatten()
            });
        if let Some(index) = existing_index {
            let z = draw_object_z(&plan.objects[index]);
            plan.objects[index] = DrawObject::Text {
                id: annotation.id.clone(),
                bbox,
                text: annotation.label.clone(),
                style: "annotation".to_string(),
                z,
            };
        } else {
            plan.objects.push(DrawObject::Text {
                id: annotation.id.clone(),
                bbox,
                text: annotation.label.clone(),
                style: "annotation".to_string(),
                z: next_z(plan),
            });
        }
    }
}

fn fold_generic_task_loss_annotation_into_connector_label(
    plan: &mut DrawPlan,
    figure_plan: &FigurePlan,
    label: &str,
    target_id: Option<&str>,
) -> bool {
    if !is_generic_task_loss_annotation_label(label) || plan_has_independent_task_loss_box(plan) {
        return false;
    }
    let Some(connector_id) = task_loss_annotation_target_connector_id(plan, figure_plan, target_id)
    else {
        return false;
    };
    set_connector_label_by_id(
        plan,
        &connector_id,
        generic_task_loss_connector_label(label),
    )
}

fn plan_has_independent_task_loss_box(plan: &DrawPlan) -> bool {
    current_box_route_info_map(plan)
        .iter()
        .any(|(id, info)| is_task_loss_route_box(id, info) && !is_main_route_box(info))
}

fn task_loss_annotation_target_connector_id(
    plan: &DrawPlan,
    figure_plan: &FigurePlan,
    target_id: Option<&str>,
) -> Option<String> {
    let target_id = target_id?;
    if draw_plan_has_connector(plan, target_id) {
        return Some(target_id.to_string());
    }
    let edge = figure_plan.edges.iter().find(|edge| edge.id == target_id)?;
    plan.objects.iter().find_map(|object| {
        let DrawObject::Connector { id, from, to, .. } = object else {
            return None;
        };
        (id == &edge.id
            || (from.as_deref() == Some(edge.from.as_str())
                && to.as_deref() == Some(edge.to.as_str())))
        .then(|| id.clone())
    })
}

fn draw_plan_has_connector(plan: &DrawPlan, connector_id: &str) -> bool {
    plan.objects
        .iter()
        .any(|object| matches!(object, DrawObject::Connector { id, .. } if id == connector_id))
}

fn generic_task_loss_connector_label(label: &str) -> String {
    if normalized_annotation_phrase(label) == "loss" {
        "Task Loss".to_string()
    } else {
        label.trim().to_string()
    }
}

fn set_connector_label_by_id(plan: &mut DrawPlan, connector_id: &str, text: String) -> bool {
    for object in &mut plan.objects {
        let DrawObject::Connector {
            id, points, label, ..
        } = object
        else {
            continue;
        };
        if id != connector_id {
            continue;
        }
        let start = points.first().copied().unwrap_or([0.45, 0.45]);
        let end = points.last().copied().unwrap_or([0.55, 0.55]);
        *label = Some(DrawLabel {
            text,
            bbox: connector_label_bbox(start, end),
        });
        return true;
    }
    false
}

fn move_annotations_off_components(plan: &mut DrawPlan) {
    let component_boxes = current_box_map(plan);
    if component_boxes.is_empty() {
        return;
    }

    let annotation_updates = plan
        .objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Text {
                id, bbox, style, ..
            } = object
            else {
                return None;
            };
            if !style.to_lowercase().contains("annotation") {
                return None;
            }
            let (component_id, component_bbox) =
                most_overlapped_component(*bbox, &component_boxes)?;
            let clear_bbox =
                clear_annotation_bbox_near_component(*bbox, component_bbox, &component_boxes);
            (!boxes_nearly_equal(clear_bbox, *bbox)).then(|| (id.clone(), clear_bbox, component_id))
        })
        .collect::<Vec<_>>();

    for (id, bbox, _) in annotation_updates {
        if let Some(DrawObject::Text {
            bbox: object_bbox, ..
        }) = plan
            .objects
            .iter_mut()
            .find(|object| draw_object_id(object) == id)
        {
            *object_bbox = bbox;
        }
    }
}

fn move_annotations_off_edges(plan: &mut DrawPlan) {
    let segment_boxes = connector_segment_boxes(plan, 0.026);
    if segment_boxes.is_empty() {
        return;
    }
    let obstacle_boxes = plan
        .objects
        .iter()
        .filter_map(|object| match object {
            DrawObject::Box { bbox, .. } | DrawObject::Image { bbox, .. } => Some(*bbox),
            _ => None,
        })
        .collect::<Vec<_>>();
    let text_boxes = plan
        .objects
        .iter()
        .filter_map(|object| match object {
            DrawObject::Text { id, bbox, .. } => Some((id.clone(), *bbox)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let annotation_updates = plan
        .objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Text {
                id, bbox, style, ..
            } = object
            else {
                return None;
            };
            if !style.to_lowercase().contains("annotation")
                || !annotation_bbox_overlaps_segments(*bbox, &segment_boxes)
            {
                return None;
            }
            let route = most_overlapped_connector_route(plan, *bbox)?;
            let placed_text_boxes = text_boxes
                .iter()
                .filter_map(|(text_id, text_bbox)| (text_id != id).then_some(*text_bbox))
                .collect::<Vec<_>>();
            let moved = connector_label_candidates_near_route(*bbox, route.as_slice())
                .into_iter()
                .find(|candidate| {
                    connector_label_candidate_clear(
                        *candidate,
                        &obstacle_boxes,
                        &segment_boxes,
                        &placed_text_boxes,
                    )
                })
                .unwrap_or_else(|| place_label_outside_edge(*bbox, route.as_slice()));
            Some((id.clone(), moved))
        })
        .collect::<Vec<_>>();

    for (id, bbox) in annotation_updates {
        if let Some(DrawObject::Text {
            bbox: object_bbox, ..
        }) = plan
            .objects
            .iter_mut()
            .find(|object| draw_object_id(object) == id)
        {
            *object_bbox = bbox;
        }
    }
}

fn move_inference_annotations_out_of_teacher_student_corridors(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let branch_pairs = teacher_student_branch_pairs(&box_map);
    if branch_pairs.is_empty() {
        return;
    }
    let Some((student_id, student)) = student_anchor_for_inference_notes(plan) else {
        return;
    };
    let segment_boxes = connector_segment_boxes(plan, 0.018);
    let obstacle_boxes = plan
        .objects
        .iter()
        .filter_map(|object| match object {
            DrawObject::Box { bbox, .. } | DrawObject::Image { bbox, .. } => Some(*bbox),
            _ => None,
        })
        .collect::<Vec<_>>();
    let text_boxes = plan
        .objects
        .iter()
        .filter_map(|object| match object {
            DrawObject::Text { id, bbox, .. } => Some((id.clone(), *bbox)),
            _ => None,
        })
        .collect::<Vec<_>>();

    let updates = plan
        .objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Text {
                id,
                bbox,
                text,
                style,
                ..
            } = object
            else {
                return None;
            };
            if !is_student_inference_note_like(id, text, "", style) {
                return None;
            }
            let in_corridor = inference_note_is_in_teacher_student_corridor(*bbox, &branch_pairs);
            let too_close_to_edge = annotation_bbox_overlaps_segments(*bbox, &segment_boxes);
            if !in_corridor && !too_close_to_edge {
                return None;
            }
            let placed_text_boxes = text_boxes
                .iter()
                .filter_map(|(text_id, text_bbox)| (text_id != id).then_some(*text_bbox))
                .collect::<Vec<_>>();
            let candidate =
                inference_annotation_corridor_escape_candidates(*bbox, text, student.bbox)
                    .into_iter()
                    .find(|candidate| {
                        !inference_note_is_in_teacher_student_corridor(*candidate, &branch_pairs)
                            && candidate_is_near_inference_anchor(*candidate, &[student.bbox])
                            && connector_label_candidate_clear(
                                *candidate,
                                &obstacle_boxes,
                                &segment_boxes,
                                &placed_text_boxes,
                            )
                            && route_box_candidate_is_clear(id, *candidate, &box_map)
                    })?;
            Some((id.clone(), candidate))
        })
        .collect::<Vec<_>>();

    for (id, bbox) in updates {
        if id == student_id {
            continue;
        }
        set_object_bbox(plan, &id, bbox);
    }
}

fn inference_annotation_corridor_escape_candidates(
    current: [f64; 4],
    text: &str,
    student_bbox: [f64; 4],
) -> Vec<[f64; 4]> {
    let width = (0.075 + visible_text_len(text) as f64 * 0.0055).clamp(0.16, 0.21);
    let height = box_height(current).clamp(0.052, 0.070);
    let mut candidates = Vec::new();
    let centered_x = center_x(student_bbox) - width / 2.0;
    let current_x = current[0];
    let left_x = student_bbox[0] - 0.055;
    let right_x = student_bbox[2] + 0.035;
    let side_y = center_y(student_bbox) - height / 2.0;

    push_unique_label_candidate(
        &mut candidates,
        clamp_label_bbox(right_x, side_y, width, height),
    );
    for y in [
        student_bbox[3] + 0.025,
        student_bbox[3] + 0.055,
        student_bbox[3] + 0.085,
    ] {
        for x in [centered_x, left_x, current_x, student_bbox[0]] {
            push_unique_label_candidate(&mut candidates, clamp_label_bbox(x, y, width, height));
        }
    }
    let left_side_x = student_bbox[0] - 0.035 - width;
    push_unique_label_candidate(
        &mut candidates,
        clamp_label_bbox(left_side_x, side_y, width, height),
    );
    candidates
}

fn compact_floating_inference_excluded_annotations(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let Some((_teacher_id, teacher)) = box_map
        .iter()
        .find(|(id, info)| is_teacher_or_context_route_box(id, info))
    else {
        return;
    };
    let mut updates = Vec::new();
    for object in &plan.objects {
        let DrawObject::Text {
            id,
            bbox,
            text,
            style,
            ..
        } = object
        else {
            continue;
        };
        if !floating_inference_excluded_annotation(id, text, style, *bbox) {
            continue;
        }
        let width = (0.08 + visible_text_len(text) as f64 * 0.007).clamp(0.14, 0.22);
        let height = 0.055;
        let centered_x = (center_x(teacher.bbox) - width / 2.0).clamp(0.02, 0.98 - width);
        let above_y = teacher.bbox[1] - height - 0.025;
        let above = [centered_x, above_y, centered_x + width, above_y + height];
        let right_x = (teacher.bbox[2] + 0.035).clamp(0.02, 0.98 - width);
        let right_y = (teacher.bbox[1] + 0.012).clamp(0.02, 0.98 - height);
        let right = [right_x, right_y, right_x + width, right_y + height];
        let candidate = [above, right]
            .into_iter()
            .map(normalize_box)
            .find(|candidate| route_box_candidate_is_clear(id, *candidate, &box_map))
            .unwrap_or_else(|| normalize_box(above));
        updates.push((id.clone(), candidate));
    }

    for (id, bbox) in updates {
        set_object_bbox(plan, &id, bbox);
    }
}

fn floating_inference_excluded_annotation(
    id: &str,
    text: &str,
    style: &str,
    bbox: [f64; 4],
) -> bool {
    if !style.to_lowercase().contains("annotation") {
        return false;
    }
    let identity = format!("{} {}", id.to_lowercase(), text.to_lowercase());
    let semantic_note = identity.contains("inference excluded")
        || identity.contains("teacher_frozen")
        || identity.contains("frozen note");
    semantic_note && (box_area(bbox) > 0.018 || box_width(bbox) > 0.24)
}

fn annotation_bbox_overlaps_segments(bbox: [f64; 4], segment_boxes: &[[f64; 4]]) -> bool {
    segment_boxes
        .iter()
        .any(|segment_box| intersection_area(bbox, *segment_box) > 0.0001)
}

fn most_overlapped_connector_route(
    plan: &DrawPlan,
    annotation_bbox: [f64; 4],
) -> Option<Vec<[f64; 2]>> {
    plan.objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Connector { points, .. } = object else {
                return None;
            };
            let score = points
                .windows(2)
                .map(|window| {
                    let segment_box = expand_box(
                        [
                            window[0][0].min(window[1][0]),
                            window[0][1].min(window[1][1]),
                            window[0][0].max(window[1][0]),
                            window[0][1].max(window[1][1]),
                        ],
                        0.026,
                    );
                    intersection_area(annotation_bbox, segment_box)
                })
                .sum::<f64>();
            (score > 0.0001).then(|| (points.clone(), score))
        })
        .max_by(|left, right| left.1.total_cmp(&right.1))
        .map(|(points, _)| points)
}

fn most_overlapped_component(
    annotation_bbox: [f64; 4],
    component_boxes: &HashMap<String, [f64; 4]>,
) -> Option<(String, [f64; 4])> {
    component_boxes
        .iter()
        .filter_map(|(id, bbox)| {
            let overlap = intersection_area(annotation_bbox, *bbox);
            (overlap > 0.0001).then(|| (id.clone(), *bbox, overlap))
        })
        .max_by(|left, right| left.2.total_cmp(&right.2))
        .map(|(id, bbox, _)| (id, bbox))
}

fn clear_annotation_bbox_near_component(
    annotation_bbox: [f64; 4],
    component_bbox: [f64; 4],
    component_boxes: &HashMap<String, [f64; 4]>,
) -> [f64; 4] {
    annotation_bbox_candidates_near_target(annotation_bbox, component_bbox)
        .into_iter()
        .find(|candidate| annotation_candidate_is_clear(*candidate, component_boxes))
        .unwrap_or_else(|| place_annotation_below_component(annotation_bbox, component_bbox))
}

fn place_annotation_below_component(
    annotation_bbox: [f64; 4],
    component_bbox: [f64; 4],
) -> [f64; 4] {
    let annotation_bbox = normalize_box(annotation_bbox);
    let width = box_width(annotation_bbox).clamp(0.14, 0.26);
    let height = box_height(annotation_bbox).clamp(0.05, 0.08);
    let x1 = (center_x(component_bbox) - width / 2.0).clamp(0.06, 0.94 - width);
    let y1 = (component_bbox[3] + 0.018).clamp(0.06, 0.94 - height);
    [x1, y1, x1 + width, y1 + height]
}

fn anchored_figure_plan_annotation_bbox(
    plan: &DrawPlan,
    label: &str,
    target_id: Option<&str>,
    bbox: [f64; 4],
) -> [f64; 4] {
    let bbox = normalize_box(bbox);
    if !annotation_label_is_inference_specific(label) {
        return bbox;
    }
    let Some(target_id) = target_id else {
        return bbox;
    };
    let Some(target_bbox) = current_box_map(plan).get(target_id).copied() else {
        return bbox;
    };
    clear_annotation_bbox_near_target(plan, bbox, target_bbox)
}

fn anchored_annotation_bbox_near_target(
    annotation_bbox: [f64; 4],
    target_bbox: [f64; 4],
) -> [f64; 4] {
    let annotation_bbox = normalize_box(annotation_bbox);
    let target_bbox = normalize_box(target_bbox);
    let width = box_width(annotation_bbox).clamp(0.14, 0.26);
    let height = box_height(annotation_bbox).clamp(0.05, 0.08);
    let x1 = (center_x(target_bbox) - width / 2.0).clamp(0.06, 0.94 - width);
    let above_y = target_bbox[1] - height - 0.012;
    let y1 = if above_y >= 0.06 {
        above_y
    } else {
        target_bbox[3] + 0.012
    }
    .clamp(0.06, 0.94 - height);
    [x1, y1, x1 + width, y1 + height]
}

fn clear_annotation_bbox_near_target(
    plan: &DrawPlan,
    annotation_bbox: [f64; 4],
    target_bbox: [f64; 4],
) -> [f64; 4] {
    let fallback = anchored_annotation_bbox_near_target(annotation_bbox, target_bbox);
    let box_map = current_box_map(plan);
    annotation_bbox_candidates_near_target(annotation_bbox, target_bbox)
        .into_iter()
        .find(|candidate| annotation_candidate_is_clear(*candidate, &box_map))
        .unwrap_or(fallback)
}

fn annotation_bbox_candidates_near_target(
    annotation_bbox: [f64; 4],
    target_bbox: [f64; 4],
) -> Vec<[f64; 4]> {
    let target_bbox = normalize_box(target_bbox);
    let annotation_bbox = normalize_box(annotation_bbox);
    let width = box_width(annotation_bbox).clamp(0.14, 0.26);
    let height = box_height(annotation_bbox).clamp(0.05, 0.08);
    let center_x = (center_x(target_bbox) - width / 2.0).clamp(0.06, 0.94 - width);
    let center_y = (center_y(target_bbox) - height / 2.0).clamp(0.06, 0.94 - height);
    let above_y = target_bbox[1] - height - 0.012;
    let below_y = target_bbox[3] + 0.012;
    let left_x = target_bbox[0] - width - 0.012;
    let right_x = target_bbox[2] + 0.012;
    let top_x = (0.5 - width / 2.0).clamp(0.06, 0.94 - width);
    vec![
        [center_x, above_y, center_x + width, above_y + height],
        [center_x, below_y, center_x + width, below_y + height],
        [
            left_x.clamp(0.06, 0.94 - width),
            center_y,
            left_x.clamp(0.06, 0.94 - width) + width,
            center_y + height,
        ],
        [
            right_x.clamp(0.06, 0.94 - width),
            center_y,
            right_x.clamp(0.06, 0.94 - width) + width,
            center_y + height,
        ],
        [top_x, 0.06, top_x + width, 0.06 + height],
        [top_x, 0.94 - height, top_x + width, 0.94],
    ]
    .into_iter()
    .filter(|candidate| {
        candidate[0] >= 0.06 && candidate[1] >= 0.06 && candidate[2] <= 0.94 && candidate[3] <= 0.94
    })
    .collect()
}

fn annotation_candidate_is_clear(candidate: [f64; 4], box_map: &HashMap<String, [f64; 4]>) -> bool {
    let candidate = normalize_box(candidate);
    box_map
        .values()
        .all(|bbox| intersection_area(candidate, expand_box(*bbox, 0.002)) <= 0.0001)
}

fn compact_oversized_short_annotations(plan: &mut DrawPlan) {
    let component_boxes = current_box_map(plan);
    let route_boxes = current_box_route_info_map(plan);
    let mut updates = Vec::new();
    for object in &plan.objects {
        let DrawObject::Text {
            id,
            bbox,
            text,
            style,
            ..
        } = object
        else {
            continue;
        };
        if !style.to_lowercase().contains("annotation")
            || visible_text_len(text) > 12
            || box_area(*bbox) <= 0.025
        {
            continue;
        }
        let Some(anchor) = short_annotation_anchor_bbox(text, *bbox, &route_boxes) else {
            continue;
        };
        let width = (0.070 + visible_text_len(text) as f64 * 0.010).clamp(0.10, 0.17);
        let height = box_height(*bbox).clamp(0.045, 0.060);
        let candidates = short_annotation_candidates(anchor, width, height);
        if let Some(candidate) = candidates
            .into_iter()
            .find(|candidate| annotation_candidate_is_clear(*candidate, &component_boxes))
        {
            updates.push((id.clone(), candidate));
        }
    }
    for (id, bbox) in updates {
        set_object_bbox(plan, &id, bbox);
    }
}

fn short_annotation_anchor_bbox(
    text: &str,
    annotation_bbox: [f64; 4],
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<[f64; 4]> {
    let lower = text.to_lowercase();
    if lower.contains("frozen")
        || lower.contains("teacher")
        || lower.contains("training")
        || lower.contains("train")
    {
        if let Some((_, info)) = box_map
            .iter()
            .filter(|(id, info)| is_teacher_or_context_route_box(id, info))
            .min_by(|left, right| {
                box_center_distance(annotation_bbox, left.1.bbox)
                    .total_cmp(&box_center_distance(annotation_bbox, right.1.bbox))
            })
        {
            return Some(info.bbox);
        }
    }
    box_map
        .iter()
        .min_by(|left, right| {
            box_center_distance(annotation_bbox, left.1.bbox)
                .total_cmp(&box_center_distance(annotation_bbox, right.1.bbox))
        })
        .map(|(_, info)| info.bbox)
}

fn short_annotation_candidates(anchor: [f64; 4], width: f64, height: f64) -> Vec<[f64; 4]> {
    let centered_x = center_x(anchor) - width / 2.0;
    let above_high = anchor[1] - 0.095;
    let above = anchor[1] - 0.025 - height;
    let right_x = anchor[2] + 0.020;
    let side_y = center_y(anchor) - height / 2.0;
    vec![
        box_from_top_left_inside(centered_x, above_high, width, height),
        box_from_top_left_inside(centered_x, above, width, height),
        box_from_top_left_inside(right_x, side_y, width, height),
        box_from_top_left_inside(anchor[0] - 0.020 - width, side_y, width, height),
    ]
}

fn is_meaningful_figure_plan_annotation(label: &str) -> bool {
    let lower = label.trim().to_lowercase();
    if lower.is_empty()
        || is_template_reference_annotation_label(&lower)
        || is_line_style_legend_annotation_label(&lower)
        || is_path_branch_annotation_label(&lower)
        || matches!(
            lower.as_str(),
            "training" | "train" | "inference" | "infer" | "testing" | "test"
        )
    {
        return false;
    }
    annotation_label_is_inference_specific(label)
        || lower.contains("residual")
        || lower.contains("loss")
        || lower.contains("alignment")
        || lower.contains("frozen")
        || lower.split_whitespace().count() > 1
}

fn annotation_label_is_inference_specific(label: &str) -> bool {
    let lower = label.trim().to_lowercase();
    lower.contains("inference:") || lower.contains("student only") || lower.contains("only student")
}

fn draw_object_z(object: &DrawObject) -> i32 {
    match object {
        DrawObject::Box { z, .. }
        | DrawObject::Text { z, .. }
        | DrawObject::Connector { z, .. }
        | DrawObject::Image { z, .. }
        | DrawObject::Group { z, .. } => *z,
    }
}

fn noisy_connected_inference_note_ids(plan: &DrawPlan) -> HashSet<String> {
    plan.objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Box {
                id,
                bbox,
                text,
                role,
                style,
                ..
            } = object
            else {
                return None;
            };
            if !is_standalone_inference_note_box(id, text, role, style) {
                return None;
            }
            connected_inference_note_is_noisy(plan, id, *bbox).then(|| id.clone())
        })
        .collect()
}

fn connected_inference_note_is_noisy(plan: &DrawPlan, note_id: &str, note_bbox: [f64; 4]) -> bool {
    let connected_edges = plan
        .objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Connector {
                id,
                points,
                from,
                to,
                ..
            } = object
            else {
                return None;
            };
            let touches_note = from.as_deref() == Some(note_id) || to.as_deref() == Some(note_id);
            touches_note.then(|| (id.as_str(), points.as_slice()))
        })
        .collect::<Vec<_>>();
    if connected_edges.is_empty() {
        return false;
    }
    box_is_outer_note(note_bbox)
        || connected_edges
            .iter()
            .any(|(_, points)| polyline_length(points) > 0.35)
        || connected_edges
            .iter()
            .any(|(edge_id, points)| connector_crosses_other_connector(plan, edge_id, points))
}

fn box_is_outer_note(bbox: [f64; 4]) -> bool {
    let bbox = normalize_box(bbox);
    bbox[0] < 0.08 || bbox[1] < 0.08 || bbox[2] > 0.93 || bbox[3] > 0.93
}

fn connector_crosses_other_connector(plan: &DrawPlan, edge_id: &str, points: &[[f64; 2]]) -> bool {
    plan.objects.iter().any(|object| {
        let DrawObject::Connector {
            id: other_id,
            points: other_points,
            ..
        } = object
        else {
            return false;
        };
        other_id != edge_id && polylines_cross(points, other_points)
    })
}

fn student_id_for_connected_notes(plan: &DrawPlan, note_ids: &HashSet<String>) -> Option<String> {
    plan.objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Connector { from, to, .. } = object else {
                return None;
            };
            let neighbor_id = if from.as_deref().is_some_and(|id| note_ids.contains(id)) {
                to.as_deref()
            } else if to.as_deref().is_some_and(|id| note_ids.contains(id)) {
                from.as_deref()
            } else {
                None
            }?;
            student_box_rank(plan, neighbor_id).map(|rank| (neighbor_id.to_string(), rank))
        })
        .min_by_key(|(_, rank)| *rank)
        .map(|(id, _)| id)
}

fn student_box_rank(plan: &DrawPlan, id: &str) -> Option<i32> {
    plan.objects.iter().find_map(|object| {
        let DrawObject::Box {
            id: object_id,
            text,
            role,
            style,
            ..
        } = object
        else {
            return None;
        };
        if object_id != id {
            return None;
        }
        let haystack = format!(
            "{} {} {} {}",
            object_id.to_lowercase(),
            text.to_lowercase(),
            role.to_lowercase(),
            style.to_lowercase()
        );
        if haystack.contains("student") {
            Some(0)
        } else if haystack.contains("main") || haystack.contains("primary") {
            Some(1)
        } else {
            None
        }
    })
}

fn remove_duplicate_connectors(plan: &mut DrawPlan) {
    let mut best_by_key: HashMap<String, (usize, i32)> = HashMap::new();
    let mut remove_indices = HashSet::new();
    for (index, object) in plan.objects.iter().enumerate() {
        let DrawObject::Connector {
            id,
            points,
            from,
            to,
            style,
            label,
            ..
        } = object
        else {
            continue;
        };
        let Some(key) = duplicate_connector_key(from.as_deref(), to.as_deref(), points) else {
            continue;
        };
        let score = duplicate_connector_score(id, style, label.as_ref());
        if let Some((best_index, best_score)) = best_by_key.get_mut(&key) {
            if score > *best_score {
                remove_indices.insert(*best_index);
                *best_index = index;
                *best_score = score;
            } else {
                remove_indices.insert(index);
            }
        } else {
            best_by_key.insert(key, (index, score));
        }
    }
    if remove_indices.is_empty() {
        return;
    }
    let mut index = 0usize;
    plan.objects.retain(|_| {
        let keep = !remove_indices.contains(&index);
        index += 1;
        keep
    });
}

fn duplicate_connector_key(
    from: Option<&str>,
    to: Option<&str>,
    points: &[[f64; 2]],
) -> Option<String> {
    let (Some(from), Some(to)) = (from, to) else {
        return None;
    };
    Some(format!("{from}->{to}|{}", connector_points_key(points)))
}

fn connector_points_key(points: &[[f64; 2]]) -> String {
    points
        .iter()
        .map(|point| {
            let x = (point[0] * 1000.0).round() as i32;
            let y = (point[1] * 1000.0).round() as i32;
            format!("{x}:{y}")
        })
        .collect::<Vec<_>>()
        .join(";")
}

fn duplicate_connector_score(id: &str, style: &str, label: Option<&DrawLabel>) -> i32 {
    let text = format!(
        "{} {} {}",
        id.to_lowercase(),
        style.to_lowercase(),
        label
            .map(|label| label.text.to_lowercase())
            .unwrap_or_default()
    );
    let mut score = 0;
    if text.contains("loss") || text.contains("residual") || text.contains("supervision") {
        score += 8;
    }
    if text.contains("main") || text.contains("dashed") {
        score += 3;
    }
    if text.contains("back") || text.contains("duplicate") {
        score -= 10;
    }
    score
}

fn connector_endpoint_ids(plan: &DrawPlan) -> HashSet<&str> {
    let mut ids = HashSet::new();
    for object in &plan.objects {
        if let DrawObject::Connector { from, to, .. } = object {
            if let Some(from) = from.as_deref() {
                ids.insert(from);
            }
            if let Some(to) = to.as_deref() {
                ids.insert(to);
            }
        }
    }
    ids
}

fn is_standalone_inference_note_box(id: &str, text: &str, role: &str, style: &str) -> bool {
    let id = id.to_lowercase();
    let text = text.to_lowercase();
    let role = role.to_lowercase();
    let style = style.to_lowercase();
    let note_like = id.contains("note")
        || role.contains("context")
        || style.contains("muted")
        || style.contains("neutral");
    (id.contains("inference") || text.contains("inference"))
        && (text.contains("only") || note_like)
        && note_like
}

fn is_inference_note_component(component: &Component) -> bool {
    let role = format!("{:?}", component.role).to_lowercase();
    let id = component.id.to_lowercase();
    // 带有明确语义锚点的组件应保留为 box；其余“推断式车道/注记”才会被折叠。
    if role.contains("output")
        || id.contains("note")
        || id.contains("badge")
        || id.contains("_only")
    {
        return false;
    }

    is_standalone_inference_note_box(
        &component.id,
        &component.label,
        &role,
        &component_style(component),
    )
}

fn is_standalone_inference_note_text(id: &str, text: &str, style: &str) -> bool {
    let id = id.to_lowercase();
    let text = text.to_lowercase();
    let style = style.to_lowercase();
    let note_like = id.contains("note")
        || id.starts_with("ann_")
        || id.starts_with("anno_")
        || style.contains("annotation")
        || style.contains("muted")
        || style.contains("context")
        || style.contains("neutral");
    let is_inference_note = id.contains("inference")
        || id.contains("ann_inference")
        || text.contains("inference")
        || style.contains("inference");
    is_inference_note && note_like && (text.contains("only") || text.contains("student"))
}

fn inference_note_boxes_and_text(plan: &DrawPlan) -> HashMap<String, [f64; 4]> {
    plan.objects
        .iter()
        .filter_map(|object| match object {
            DrawObject::Box {
                id,
                bbox,
                text,
                role,
                style,
                ..
            } if is_standalone_inference_note_box(id, text, role, style) => {
                Some((id.clone(), *bbox))
            }
            DrawObject::Text {
                id,
                bbox,
                text,
                style,
                ..
            } if is_standalone_inference_note_text(id, text, style) => Some((id.clone(), *bbox)),
            _ => None,
        })
        .collect()
}

fn largest_student_box_id(plan: &DrawPlan, excluded_ids: &HashSet<String>) -> Option<String> {
    plan.objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Box {
                id,
                bbox,
                text,
                role,
                ..
            } = object
            else {
                return None;
            };
            let id_lower = id.to_lowercase();
            let text_lower = text.to_lowercase();
            let role_lower = role.to_lowercase();
            (!excluded_ids.contains(id)
                && (id_lower.contains("student")
                    || text_lower.contains("student")
                    || role_lower.contains("student")))
            .then(|| (id.clone(), box_area(*bbox)))
        })
        .max_by(|left, right| left.1.total_cmp(&right.1))
        .map(|(id, _)| id)
}

fn remove_asymmetric_branch_annotations(plan: &mut DrawPlan) {
    let has_teacher_annotation = plan
        .objects
        .iter()
        .any(|object| is_asymmetric_branch_annotation_text(object, "teacher"));
    let has_student_annotation = plan
        .objects
        .iter()
        .any(|object| is_asymmetric_branch_annotation_text(object, "student"));
    if has_teacher_annotation == has_student_annotation {
        return;
    }

    plan.objects.retain(|object| {
        !is_asymmetric_branch_annotation_text(object, "teacher")
            && !is_asymmetric_branch_annotation_text(object, "student")
    });
}

fn remove_template_reference_and_overlapping_path_annotations(plan: &mut DrawPlan) {
    let boxes = plan
        .objects
        .iter()
        .filter_map(|object| match object {
            DrawObject::Box { bbox, .. } => Some(*bbox),
            _ => None,
        })
        .collect::<Vec<_>>();
    plan.objects.retain(|object| {
        !is_template_reference_annotation(object)
            && !is_overlapping_path_annotation(object, boxes.as_slice())
    });
}

fn is_template_reference_annotation(object: &DrawObject) -> bool {
    let DrawObject::Text {
        id, text, style, ..
    } = object
    else {
        return false;
    };
    let style = style.to_lowercase();
    if !style.contains("annotation") {
        return false;
    }
    let haystack = format!("{} {}", id.to_lowercase(), text.to_lowercase());
    is_template_reference_annotation_label(&haystack)
        || is_line_style_legend_annotation_label(&haystack)
}

fn is_overlapping_path_annotation(object: &DrawObject, boxes: &[[f64; 4]]) -> bool {
    let DrawObject::Text {
        text, bbox, style, ..
    } = object
    else {
        return false;
    };
    style.to_lowercase().contains("annotation")
        && is_path_branch_annotation_label(text)
        && boxes
            .iter()
            .any(|box_bbox| intersection_area(*bbox, *box_bbox) > 0.001)
}

fn is_template_reference_annotation_label(label: &str) -> bool {
    let lower = label.to_lowercase();
    lower.contains("template")
        || lower.contains("adapted:")
        || lower.contains("_branch")
        || lower.contains("simclr_")
}

fn is_line_style_legend_annotation_label(label: &str) -> bool {
    let lower = label.to_lowercase();
    (lower.contains("dashed") || lower.contains("solid") || lower.contains("dash"))
        && (lower.contains('=') || lower.contains("means") || lower.contains("legend"))
}

fn is_path_branch_annotation_label(label: &str) -> bool {
    let lower = label.to_lowercase();
    (lower.contains("teacher") || lower.contains("student"))
        && (lower.contains("train")
            || lower.contains("infer")
            || lower.contains("path")
            || lower.contains("frozen"))
        && lower.contains('(')
}

fn is_branch_annotation_text(text: &str, style: &str, branch: &str) -> bool {
    let text = text.to_lowercase();
    let style = style.to_lowercase();
    style.contains("annotation") && text.contains(branch)
}

fn is_asymmetric_branch_annotation_text(object: &DrawObject, branch: &str) -> bool {
    let DrawObject::Text {
        text, bbox, style, ..
    } = object
    else {
        return false;
    };
    if annotation_label_is_inference_specific(text) && bbox[1] > 0.16 {
        return false;
    }
    is_branch_annotation_text(text, style, branch)
}

fn avoid_connector_label_collisions(plan: &mut DrawPlan) {
    let component_boxes = plan
        .objects
        .iter()
        .filter_map(|object| match object {
            DrawObject::Box { bbox, .. } => Some(*bbox),
            _ => None,
        })
        .collect::<Vec<_>>();
    let segment_boxes = connector_segment_boxes(plan, 0.006);
    let mut label_boxes: Vec<[f64; 4]> = Vec::new();

    for object in &mut plan.objects {
        let DrawObject::Connector { label, .. } = object else {
            continue;
        };
        let Some(label) = label else {
            continue;
        };
        for _ in 0..12 {
            let hits_component = component_boxes
                .iter()
                .any(|bbox| boxes_overlap(label.bbox, *bbox));
            let hits_segment = segment_boxes
                .iter()
                .any(|bbox| intersection_area(label.bbox, *bbox) > 0.0001);
            let hits_label = label_boxes
                .iter()
                .any(|bbox| boxes_overlap(label.bbox, *bbox));
            if !hits_component && !hits_segment && !hits_label {
                break;
            }
            label.bbox = shift_label(label.bbox, 0.04);
        }
        label_boxes.push(label.bbox);
    }
}

fn snap_connector_labels_to_final_routes(plan: &mut DrawPlan) {
    let obstacle_boxes = plan
        .objects
        .iter()
        .filter_map(|object| match object {
            DrawObject::Box { bbox, .. }
            | DrawObject::Text { bbox, .. }
            | DrawObject::Image { bbox, .. } => Some(*bbox),
            _ => None,
        })
        .collect::<Vec<_>>();
    let segment_boxes = connector_segment_boxes(plan, 0.006);
    let connector_labels = plan
        .objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Connector {
                id, points, label, ..
            } = object
            else {
                return None;
            };
            let label = label.as_ref()?;
            Some((id.clone(), points.clone(), label.text.clone(), label.bbox))
        })
        .collect::<Vec<_>>();

    let mut placed_labels: Vec<[f64; 4]> = Vec::new();
    let mut updates: Vec<(String, [f64; 4])> = Vec::new();
    for (id, points, label_text, label_bbox) in connector_labels {
        let snapped = if connector_label_distance_to_route(label_bbox, &points) <= 0.09
            && connector_label_candidate_clear(
                label_bbox,
                &obstacle_boxes,
                &segment_boxes,
                &placed_labels,
            ) {
            label_bbox
        } else {
            let candidates = connector_label_candidates_near_route(label_bbox, &points);
            let preferred_lower = prefer_lower_connector_label_candidates(
                id.as_str(),
                label_text.as_str(),
                label_bbox,
                &points,
            );
            candidates
                .iter()
                .copied()
                .filter(|candidate| {
                    !preferred_lower || connector_label_is_below_route(*candidate, &points)
                })
                .find(|candidate| {
                    connector_label_candidate_clear(
                        *candidate,
                        &obstacle_boxes,
                        &segment_boxes,
                        &placed_labels,
                    )
                })
                .or_else(|| {
                    candidates
                        .iter()
                        .copied()
                        .filter(|candidate| {
                            !preferred_lower || connector_label_is_below_route(*candidate, &points)
                        })
                        .find(|candidate| {
                            connector_label_candidate_line_clear(
                                *candidate,
                                &segment_boxes,
                                &placed_labels,
                            )
                        })
                })
                .or_else(|| {
                    candidates.iter().copied().find(|candidate| {
                        connector_label_candidate_clear(
                            *candidate,
                            &obstacle_boxes,
                            &segment_boxes,
                            &placed_labels,
                        )
                    })
                })
                .or_else(|| {
                    candidates.iter().copied().find(|candidate| {
                        connector_label_candidate_line_clear(
                            *candidate,
                            &segment_boxes,
                            &placed_labels,
                        )
                    })
                })
                .unwrap_or_else(|| place_label_outside_edge(label_bbox, &points))
        };
        placed_labels.push(snapped);
        updates.push((id, snapped));
    }

    for object in &mut plan.objects {
        let DrawObject::Connector { id, label, .. } = object else {
            continue;
        };
        let Some(label) = label else {
            continue;
        };
        if let Some((_, snapped_bbox)) = updates.iter().find(|(update_id, _)| update_id == id) {
            label.bbox = *snapped_bbox;
        }
    }
}

fn prefer_lower_connector_label_candidates(
    id: &str,
    label_text: &str,
    label_bbox: [f64; 4],
    points: &[[f64; 2]],
) -> bool {
    let text = format!("{} {}", id.to_lowercase(), label_text.to_lowercase());
    let is_task_or_loss = text.contains("loss") || text.contains("task");
    let route_bbox = points_to_box(points);
    let already_below_route = center_y(label_bbox) > center_y(route_bbox) + 0.025;
    let feedback_like_route = points.len() >= 4 && box_height(route_bbox) > 0.06;
    is_task_or_loss && (already_below_route || feedback_like_route)
}

fn connector_label_is_below_route(label_bbox: [f64; 4], points: &[[f64; 2]]) -> bool {
    let route_y =
        dominant_horizontal_segment_y(points).unwrap_or_else(|| center_y(points_to_box(points)));
    center_y(label_bbox) >= route_y + 0.02
}

fn dominant_horizontal_segment_y(points: &[[f64; 2]]) -> Option<f64> {
    points
        .windows(2)
        .filter_map(|window| {
            let dx = (window[1][0] - window[0][0]).abs();
            let dy = (window[1][1] - window[0][1]).abs();
            (dx >= dy && dx > 0.01).then(|| (((window[0][1] + window[1][1]) / 2.0), dx))
        })
        .max_by(|left, right| left.1.total_cmp(&right.1))
        .map(|(y, _)| y)
}

fn tighten_short_connector_labels_near_routes(plan: &mut DrawPlan) {
    let updates = plan
        .objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Connector {
                id, points, label, ..
            } = object
            else {
                return None;
            };
            let label = label.as_ref()?;
            let tightened = tighten_short_connector_label_bbox(label, points)?;
            Some((id.clone(), tightened))
        })
        .collect::<Vec<_>>();

    for object in &mut plan.objects {
        let DrawObject::Connector { id, label, .. } = object else {
            continue;
        };
        let Some(label) = label else {
            continue;
        };
        if let Some((_, bbox)) = updates.iter().find(|(update_id, _)| update_id == id) {
            label.bbox = *bbox;
        }
    }
}

fn tighten_short_connector_label_bbox(label: &DrawLabel, points: &[[f64; 2]]) -> Option<[f64; 4]> {
    let visible_chars = visible_text_len(&label.text);
    if visible_chars == 0 || visible_chars > 8 {
        return None;
    }
    let bbox = normalize_box(label.bbox);
    let target_width = short_connector_label_width(visible_chars).min(box_width(bbox));
    if target_width >= box_width(bbox) - 0.002 {
        return None;
    }
    let height = box_height(bbox).clamp(0.035, 0.07);
    let center = [center_x(bbox), center_y(bbox)];
    let (start, end) = nearest_connector_segment(points, center)?;
    let gap = 0.024;
    let dx = (end[0] - start[0]).abs();
    let dy = (end[1] - start[1]).abs();

    if dy > dx {
        let x = (start[0] + end[0]) / 2.0;
        let top = start[1].min(end[1]);
        let bottom = start[1].max(end[1]);
        let min_y = top + height / 2.0;
        let max_y = bottom - height / 2.0;
        let y_center = if min_y <= max_y {
            center[1].clamp(min_y, max_y)
        } else {
            (top + bottom) / 2.0
        };
        let x1 = if center[0] >= x {
            x + gap
        } else {
            x - gap - target_width
        };
        return Some(clamp_label_bbox(
            x1,
            y_center - height / 2.0,
            target_width,
            height,
        ));
    }

    if dx > dy {
        let y = (start[1] + end[1]) / 2.0;
        let left = start[0].min(end[0]);
        let right = start[0].max(end[0]);
        let min_x = left + target_width / 2.0;
        let max_x = right - target_width / 2.0;
        let x_center = if min_x <= max_x {
            center[0].clamp(min_x, max_x)
        } else {
            (left + right) / 2.0
        };
        let y1 = if center[1] <= y {
            y - gap - height
        } else {
            y + gap
        };
        return Some(clamp_label_bbox(
            x_center - target_width / 2.0,
            y1,
            target_width,
            height,
        ));
    }

    Some(box_with_size_preserving_center(bbox, target_width, height))
}

fn short_connector_label_width(visible_chars: usize) -> f64 {
    if visible_chars <= 2 {
        return 0.036;
    }
    (0.045 + visible_chars as f64 * 0.009).clamp(0.06, 0.12)
}

fn nearest_connector_segment(points: &[[f64; 2]], point: [f64; 2]) -> Option<([f64; 2], [f64; 2])> {
    points
        .windows(2)
        .filter(|window| {
            ((window[1][0] - window[0][0]).powi(2) + (window[1][1] - window[0][1]).powi(2)).sqrt()
                > 0.01
        })
        .min_by(|left, right| {
            point_to_segment_distance(point, left[0], left[1])
                .total_cmp(&point_to_segment_distance(point, right[0], right[1]))
        })
        .map(|window| (window[0], window[1]))
}

fn point_to_segment_distance(point: [f64; 2], start: [f64; 2], end: [f64; 2]) -> f64 {
    let vx = end[0] - start[0];
    let vy = end[1] - start[1];
    let length_sq = vx * vx + vy * vy;
    if length_sq <= 0.000001 {
        return ((point[0] - start[0]).powi(2) + (point[1] - start[1]).powi(2)).sqrt();
    }
    let t = (((point[0] - start[0]) * vx + (point[1] - start[1]) * vy) / length_sq).clamp(0.0, 1.0);
    let projection = [start[0] + t * vx, start[1] + t * vy];
    ((point[0] - projection[0]).powi(2) + (point[1] - projection[1]).powi(2)).sqrt()
}

fn move_connector_labels_off_components(plan: &mut DrawPlan) {
    let obstacle_boxes = plan
        .objects
        .iter()
        .filter_map(|object| match object {
            DrawObject::Box { bbox, .. } | DrawObject::Image { bbox, .. } => Some(*bbox),
            _ => None,
        })
        .collect::<Vec<_>>();
    if obstacle_boxes.is_empty() {
        return;
    }
    let segment_boxes = connector_segment_boxes(plan, 0.006);
    let label_snapshots = plan
        .objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Connector {
                id, points, label, ..
            } = object
            else {
                return None;
            };
            let label = label.as_ref()?;
            Some((id.clone(), points.clone(), label.bbox))
        })
        .collect::<Vec<_>>();

    let mut placed_labels: Vec<[f64; 4]> = Vec::new();
    let mut updates = Vec::new();
    for (id, points, label_bbox) in label_snapshots {
        let Some(component_bbox) = most_overlapped_bbox(label_bbox, &obstacle_boxes) else {
            placed_labels.push(label_bbox);
            continue;
        };
        let candidates = connector_label_candidates_near_route(label_bbox, &points)
            .into_iter()
            .chain(annotation_bbox_candidates_near_target(
                label_bbox,
                component_bbox,
            ));
        let moved = candidates
            .into_iter()
            .find(|candidate| {
                connector_label_candidate_clear(
                    *candidate,
                    &obstacle_boxes,
                    &segment_boxes,
                    &placed_labels,
                )
            })
            .unwrap_or_else(|| place_annotation_below_component(label_bbox, component_bbox));
        placed_labels.push(moved);
        updates.push((id, moved));
    }

    for (id, bbox) in updates {
        if let Some(DrawObject::Connector {
            label: Some(label), ..
        }) = plan
            .objects
            .iter_mut()
            .find(|object| draw_object_id(object) == id)
        {
            label.bbox = bbox;
        }
    }
}

fn snap_compact_task_loss_labels_near_short_output_edges(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let obstacle_boxes = plan
        .objects
        .iter()
        .filter_map(|object| match object {
            DrawObject::Box { bbox, .. } | DrawObject::Image { bbox, .. } => Some(*bbox),
            _ => None,
        })
        .collect::<Vec<_>>();
    let segment_boxes = connector_segment_boxes(plan, 0.006);
    let label_snapshots = plan
        .objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Connector {
                id,
                points,
                from,
                to,
                label,
                ..
            } = object
            else {
                return None;
            };
            let label = label.as_ref()?;
            let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
                return None;
            };
            let (Some(from_box), Some(to_box)) = (box_map.get(from_id), box_map.get(to_id)) else {
                return None;
            };
            (is_compact_task_loss_edge_label(&label.text)
                && is_main_route_box(from_box)
                && (is_output_route_box(to_id, to_box)
                    || is_task_loss_route_box(to_id, to_box)
                    || is_loss_or_objective_box(to_id, to_box))
                && points_to_box(points)[2] - points_to_box(points)[0] <= 0.16
                && points_to_box(points)[3] - points_to_box(points)[1] <= 0.12)
                .then(|| {
                    (
                        id.clone(),
                        points.clone(),
                        label.bbox,
                        from_box.bbox,
                        to_box.bbox,
                    )
                })
        })
        .collect::<Vec<_>>();

    let mut updates = Vec::new();
    for (id, points, label_bbox, from_bbox, to_bbox) in label_snapshots {
        let Some(candidate) =
            compact_task_loss_label_candidates(label_bbox, &points, from_bbox, to_bbox)
                .into_iter()
                .filter(|candidate| {
                    connector_label_candidate_clear(
                        *candidate,
                        &obstacle_boxes,
                        &segment_boxes,
                        &[],
                    ) && connector_label_distance_to_route(*candidate, &points) <= 0.082
                })
                .min_by(|left, right| {
                    let left_score = connector_label_distance_to_route(*left, &points)
                        + box_center_distance(*left, label_bbox) * 0.05;
                    let right_score = connector_label_distance_to_route(*right, &points)
                        + box_center_distance(*right, label_bbox) * 0.05;
                    left_score.total_cmp(&right_score)
                })
        else {
            continue;
        };
        updates.push((id, candidate));
    }

    for (id, bbox) in updates {
        if let Some(DrawObject::Connector {
            label: Some(label), ..
        }) = plan
            .objects
            .iter_mut()
            .find(|object| draw_object_id(object) == id)
        {
            label.bbox = bbox;
        }
    }
}

fn is_compact_task_loss_edge_label(text: &str) -> bool {
    let phrase = normalized_annotation_phrase(text);
    let compact = phrase
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .collect::<String>();
    matches!(phrase.as_str(), "task loss" | "loss")
        || (phrase.contains("loss") && visible_text_len(text) <= 10)
        || (compact.contains("ltask") && visible_text_len(text) <= 10)
}

fn compact_task_loss_label_candidates(
    label_bbox: [f64; 4],
    points: &[[f64; 2]],
    from_bbox: [f64; 4],
    to_bbox: [f64; 4],
) -> Vec<[f64; 4]> {
    let height = box_height(label_bbox).clamp(0.035, 0.040);
    let width = box_width(label_bbox).clamp(0.085, 0.10);
    let edge_box = points_to_box(points);
    let route_y = center_y(edge_box);
    let route_left = edge_box[0].min(edge_box[2]);
    let route_right = edge_box[0].max(edge_box[2]);
    let mut candidates = Vec::new();

    let x_candidates = [
        to_bbox[0] - width * 0.45,
        to_bbox[0] - width * 0.55,
        from_bbox[2] + 0.006,
        route_left,
        (route_left + route_right) / 2.0 - width / 2.0,
        route_right - width / 2.0,
    ];
    let y_candidates = [
        to_bbox[1] - height - 0.002,
        route_y - height - 0.012,
        from_bbox[1] - height - 0.002,
        to_bbox[3] + 0.008,
    ];

    for y1 in y_candidates {
        for x1 in x_candidates {
            push_unique_label_candidate(&mut candidates, clamp_label_bbox(x1, y1, width, height));
        }
    }
    candidates
}

fn connector_label_distance_to_route(label_bbox: [f64; 4], points: &[[f64; 2]]) -> f64 {
    let center = center(label_bbox);
    points
        .windows(2)
        .map(|window| point_to_segment_distance(center, window[0], window[1]))
        .fold(f64::INFINITY, f64::min)
}

fn most_overlapped_bbox(label_bbox: [f64; 4], boxes: &[[f64; 4]]) -> Option<[f64; 4]> {
    boxes
        .iter()
        .filter_map(|bbox| {
            let overlap = intersection_area(label_bbox, *bbox);
            (overlap > 0.0001).then_some((*bbox, overlap))
        })
        .max_by(|left, right| left.1.total_cmp(&right.1))
        .map(|(bbox, _)| bbox)
}

fn connector_segment_boxes(plan: &DrawPlan, margin: f64) -> Vec<[f64; 4]> {
    plan.objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Connector { points, .. } = object else {
                return None;
            };
            Some(points.windows(2).map(move |window| {
                expand_box(
                    [
                        window[0][0].min(window[1][0]),
                        window[0][1].min(window[1][1]),
                        window[0][0].max(window[1][0]),
                        window[0][1].max(window[1][1]),
                    ],
                    margin,
                )
            }))
        })
        .flatten()
        .collect()
}

fn connector_label_candidates_near_route(
    label_bbox: [f64; 4],
    points: &[[f64; 2]],
) -> Vec<[f64; 4]> {
    let label_bbox = normalize_box(label_bbox);
    let width = box_width(label_bbox).clamp(0.04, 0.16);
    let height = box_height(label_bbox).clamp(0.04, 0.07);
    let gap = 0.018;
    let mut segments = points
        .windows(2)
        .map(|window| {
            let start = window[0];
            let end = window[1];
            let length = ((end[0] - start[0]).powi(2) + (end[1] - start[1]).powi(2)).sqrt();
            (start, end, length)
        })
        .filter(|(_, _, length)| *length > 0.01)
        .collect::<Vec<_>>();
    segments.sort_by(|left, right| {
        right
            .2
            .partial_cmp(&left.2)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut candidates = Vec::new();
    for (start, end, _) in segments.into_iter().take(4) {
        let mid_x = (start[0] + end[0]) / 2.0;
        let mid_y = (start[1] + end[1]) / 2.0;
        let dx = (end[0] - start[0]).abs();
        let dy = (end[1] - start[1]).abs();
        if dx >= dy {
            let y = mid_y;
            let left_x = start[0].min(end[0]);
            let right_x = start[0].max(end[0]);
            let above = clamp_label_bbox(mid_x - width / 2.0, y - height - gap, width, height);
            let below_y = if y + gap < 0.09 { 0.09 } else { y + gap };
            let below = clamp_label_bbox(mid_x - width / 2.0, below_y, width, height);
            if y - height - gap >= 0.08 || y + gap + height > 0.98 {
                push_unique_label_candidate(&mut candidates, above);
                push_unique_label_candidate(&mut candidates, below);
            } else {
                push_unique_label_candidate(&mut candidates, below);
                push_unique_label_candidate(&mut candidates, above);
            }
            push_unique_label_candidate(
                &mut candidates,
                clamp_label_bbox(left_x - width - gap, y - height / 2.0, width, height),
            );
            push_unique_label_candidate(
                &mut candidates,
                clamp_label_bbox(right_x + gap, y - height / 2.0, width, height),
            );
        } else {
            let x = mid_x;
            let top_y = start[1].min(end[1]);
            let bottom_y = start[1].max(end[1]);
            push_unique_label_candidate(
                &mut candidates,
                clamp_label_bbox(x + gap, mid_y - height / 2.0, width, height),
            );
            push_unique_label_candidate(
                &mut candidates,
                clamp_label_bbox(x - width - gap, mid_y - height / 2.0, width, height),
            );
            push_unique_label_candidate(
                &mut candidates,
                clamp_label_bbox(x + gap, top_y - height - gap, width, height),
            );
            push_unique_label_candidate(
                &mut candidates,
                clamp_label_bbox(x - width - gap, top_y - height - gap, width, height),
            );
            push_unique_label_candidate(
                &mut candidates,
                clamp_label_bbox(x + gap, bottom_y + gap, width, height),
            );
            push_unique_label_candidate(
                &mut candidates,
                clamp_label_bbox(x - width - gap, bottom_y + gap, width, height),
            );
        }
    }
    candidates
}

fn connector_label_candidate_clear(
    candidate: [f64; 4],
    obstacle_boxes: &[[f64; 4]],
    segment_boxes: &[[f64; 4]],
    placed_labels: &[[f64; 4]],
) -> bool {
    connector_label_candidate_line_clear(candidate, segment_boxes, placed_labels)
        && obstacle_boxes
            .iter()
            .all(|bbox| !boxes_overlap(candidate, expand_box(*bbox, 0.002)))
}

fn connector_label_candidate_line_clear(
    candidate: [f64; 4],
    segment_boxes: &[[f64; 4]],
    placed_labels: &[[f64; 4]],
) -> bool {
    label_inside_safe_area(candidate)
        && segment_boxes
            .iter()
            .all(|bbox| intersection_area(candidate, *bbox) <= 0.0001)
        && placed_labels
            .iter()
            .all(|bbox| !boxes_overlap(candidate, expand_box(*bbox, 0.004)))
}

fn clamp_label_bbox(x1: f64, y1: f64, width: f64, height: f64) -> [f64; 4] {
    let x1 = x1.clamp(0.02, 0.98 - width);
    let y1 = y1.clamp(0.02, 0.98 - height);
    [x1, y1, x1 + width, y1 + height]
}

fn push_unique_label_candidate(candidates: &mut Vec<[f64; 4]>, bbox: [f64; 4]) {
    let bbox = normalize_box(bbox);
    if candidates
        .iter()
        .any(|existing| bbox_distance(*existing, bbox) < 0.001)
    {
        return;
    }
    candidates.push(bbox);
}

fn bbox_distance(left: [f64; 4], right: [f64; 4]) -> f64 {
    left.iter()
        .zip(right.iter())
        .map(|(left, right)| (left - right).abs())
        .sum()
}

fn remove_line_overlapping_annotations(plan: &mut DrawPlan) {
    let segment_boxes = connector_segment_boxes(plan, 0.006);
    if segment_boxes.is_empty() {
        return;
    }

    plan.objects
        .retain(|object| !is_annotation_text_over_connector(object, &segment_boxes));
}

fn is_annotation_text_over_connector(object: &DrawObject, segment_boxes: &[[f64; 4]]) -> bool {
    let DrawObject::Text {
        id,
        bbox,
        text,
        style,
        ..
    } = object
    else {
        return false;
    };
    if annotation_label_is_inference_specific(text) {
        return false;
    }
    let id = id.to_lowercase();
    let style = style.to_lowercase();
    let annotation_like = style.contains("annotation")
        || style.contains("phase")
        || id.starts_with("a_")
        || id.starts_with("ann_")
        || id.starts_with("anno_");
    annotation_like
        && segment_boxes
            .iter()
            .any(|segment_box| intersection_area(*bbox, *segment_box) > 0.0001)
}

fn is_phase_only_text_annotation(object: &DrawObject) -> bool {
    let DrawObject::Text {
        id, text, style, ..
    } = object
    else {
        return false;
    };
    let id = id.to_lowercase();
    let text = text.trim().to_lowercase();
    let style = style.to_lowercase();
    let is_annotation = style.contains("annotation")
        || style.contains("phase")
        || id.contains("anno")
        || id.contains("phase");
    is_annotation
        && matches!(
            text.as_str(),
            "training" | "train" | "inference" | "infer" | "testing" | "test"
        )
}

fn expand_tiny_model_boxes(plan: &mut DrawPlan) {
    for object in &mut plan.objects {
        let DrawObject::Box { bbox, .. } = object else {
            continue;
        };
        if box_width(*bbox) < 0.10 || box_height(*bbox) < 0.10 {
            *bbox = expand_bbox_to_min_size(*bbox, 0.10, 0.10);
        }
    }
}

fn compact_oversized_loss_or_objective_boxes(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut moved_ids = HashSet::new();
    let mut updates = Vec::new();

    for (id, info) in &box_map {
        if !is_loss_or_objective_box(id, info) || !short_loss_label_should_compact(info) {
            continue;
        }
        let current_width = box_width(info.bbox);
        let current_height = box_height(info.bbox);
        if current_width <= 0.20 && current_height <= 0.14 {
            continue;
        }
        let target_width = compact_loss_box_width(info).min(current_width).max(0.10);
        let target_height = compact_loss_box_height(info).min(current_height).max(0.10);
        if current_width - target_width < 0.015 && current_height - target_height < 0.015 {
            continue;
        }
        updates.push((
            id.clone(),
            box_with_size_preserving_center(info.bbox, target_width, target_height),
        ));
    }

    for (id, bbox) in updates {
        set_box_bbox(plan, &id, bbox);
        moved_ids.insert(id);
    }
    if !moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    }
}

fn is_loss_or_objective_box(id: &str, info: &BoxRouteInfo) -> bool {
    is_task_loss_route_box(id, info) || is_objective_route_box(id, info)
}

fn short_loss_label_should_compact(info: &BoxRouteInfo) -> bool {
    let visible_chars = visible_text_len(&info.text);
    !info.text.trim().is_empty() && visible_chars <= 26
}

fn compact_loss_box_width(info: &BoxRouteInfo) -> f64 {
    let max_line_chars = info.text.lines().map(visible_text_len).max().unwrap_or(0) as f64;
    (0.10 + max_line_chars * 0.008).clamp(0.12, 0.20)
}

fn compact_loss_box_height(info: &BoxRouteInfo) -> f64 {
    let lines = info
        .text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    if lines > 1 {
        0.13
    } else {
        0.11
    }
}

fn widen_loss_or_objective_boxes_for_readability(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut updates = Vec::new();
    let mut moved_ids = HashSet::new();

    for (id, info) in &box_map {
        if !is_loss_or_objective_box(id, info) {
            continue;
        }
        let target_width = readable_loss_or_objective_width(info);
        if box_width(info.bbox) + 0.001 >= target_width {
            continue;
        }
        let Some(candidate) = readable_width_candidate(id, info.bbox, target_width, &box_map)
        else {
            continue;
        };
        updates.push((id.clone(), candidate));
    }

    for (id, bbox) in updates {
        set_box_bbox(plan, &id, bbox);
        moved_ids.insert(id);
    }
    if !moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    }
}

fn readable_loss_or_objective_width(info: &BoxRouteInfo) -> f64 {
    let max_token_chars = info
        .text
        .split(|ch: char| ch.is_whitespace() || ch == '_' || ch == '-')
        .map(|token| token.chars().count())
        .max()
        .unwrap_or(0) as f64;
    (0.045 + max_token_chars * 0.011).clamp(0.10, 0.18)
}

fn readable_width_candidate(
    id: &str,
    bbox: [f64; 4],
    target_width: f64,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<[f64; 4]> {
    readable_width_candidates(id, bbox, target_width, box_map)
        .into_iter()
        .filter(|candidate| route_box_candidate_is_clear(id, *candidate, box_map))
        .min_by(|left, right| {
            let left_score = readable_width_candidate_score(id, *left, bbox, box_map);
            let right_score = readable_width_candidate_score(id, *right, bbox, box_map);
            left_score.total_cmp(&right_score)
        })
}

fn readable_width_candidates(
    id: &str,
    bbox: [f64; 4],
    target_width: f64,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Vec<[f64; 4]> {
    let bbox = normalize_box(bbox);
    let height = box_height(bbox);
    let width = target_width.clamp(box_width(bbox), 0.94);
    let max_x1 = 0.98 - width;
    let centered = box_with_size_preserving_center(bbox, width, height);
    let expand_left_x1 = (bbox[2] - width).clamp(0.02, max_x1);
    let expand_left = [expand_left_x1, bbox[1], expand_left_x1 + width, bbox[3]];
    let expand_right_x1 = bbox[0].clamp(0.02, max_x1);
    let expand_right = [expand_right_x1, bbox[1], expand_right_x1 + width, bbox[3]];
    let mut candidates = vec![
        normalize_box(centered),
        normalize_box(expand_left),
        normalize_box(expand_right),
    ];
    for (other_id, other) in box_map {
        let (_, vertical_overlap) = intersection_dimensions(bbox, other.bbox);
        if other_id == id || vertical_overlap <= 0.0 {
            continue;
        }
        if center_x(other.bbox) >= center_x(bbox) {
            let x2 = (other.bbox[0] - 0.006).clamp(0.02 + width, 0.98);
            candidates.push(normalize_box([x2 - width, bbox[1], x2, bbox[3]]));
        } else {
            let x1 = (other.bbox[2] + 0.006).clamp(0.02, max_x1);
            candidates.push(normalize_box([x1, bbox[1], x1 + width, bbox[3]]));
        }
    }
    candidates
}

fn readable_width_candidate_score(
    id: &str,
    candidate: [f64; 4],
    original: [f64; 4],
    box_map: &HashMap<String, BoxRouteInfo>,
) -> f64 {
    let overlap_penalty = box_map
        .iter()
        .filter(|(other_id, _)| other_id.as_str() != id)
        .map(|(_, other)| intersection_area(candidate, other.bbox))
        .sum::<f64>();
    overlap_penalty * 100.0 + box_center_distance(candidate, original)
}

fn compact_oversized_short_content_boxes(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut updates = Vec::new();
    let mut moved_ids = HashSet::new();

    for (id, info) in &box_map {
        if is_loss_or_objective_box(id, info) {
            continue;
        }
        if !is_content_box_that_can_compact(id, info) || !short_content_box_is_oversized(id, info) {
            continue;
        }
        let target_width = compact_content_box_width(info).min(box_width(info.bbox));
        let target_height = compact_content_box_height(info).min(box_height(info.bbox));
        if box_width(info.bbox) - target_width < 0.04
            && box_height(info.bbox) - target_height < 0.06
        {
            continue;
        }
        let candidate = box_with_size_preserving_center(info.bbox, target_width, target_height);
        if route_box_candidate_is_clear(id, candidate, &box_map) {
            updates.push((id.clone(), candidate));
        }
    }

    for (id, bbox) in updates {
        set_box_bbox(plan, &id, bbox);
        moved_ids.insert(id);
    }
    if !moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    }
}

fn is_content_box_that_can_compact(id: &str, info: &BoxRouteInfo) -> bool {
    let text = route_box_text(id, info);
    is_input_route_box(id, info)
        || is_main_route_box(info)
        || is_teacher_or_context_route_box(id, info)
        || is_output_route_box(id, info)
        || text.contains("module")
        || route_box_text_matches_head_patterns(&info.text, &info.role, &info.style, id)
}

fn short_content_box_is_oversized(id: &str, info: &BoxRouteInfo) -> bool {
    let visible_chars = visible_text_len(&info.text);
    if info.text.trim().is_empty() || visible_chars > 24 {
        return false;
    }
    let width = box_width(info.bbox);
    let height = box_height(info.bbox);
    let is_head = route_box_text_matches_head_patterns(&info.text, &info.role, &info.style, id);
    if is_head {
        return (box_area(info.bbox) > 0.035)
            || (width > 0.18 && height > 0.13)
            || (width > 0.16 && height > 0.16);
    }
    let tall_short_box = height > 0.28 && height > width * 1.45 && box_area(info.bbox) > 0.035;
    box_area(info.bbox) > 0.085 || (width > 0.28 && height > 0.24) || tall_short_box
}

fn compact_content_box_width(info: &BoxRouteInfo) -> f64 {
    let max_line_chars = info.text.lines().map(visible_text_len).max().unwrap_or(0) as f64;
    (0.10 + max_line_chars * 0.014).clamp(0.16, 0.24)
}

fn compact_content_box_height(info: &BoxRouteInfo) -> f64 {
    let line_count = info
        .text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count()
        .max(1) as f64;
    (0.09 + line_count * 0.045).clamp(0.12, 0.22)
}

fn widen_short_main_boxes_for_readability(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut updates = Vec::new();
    let mut moved_ids = HashSet::new();

    for (id, info) in &box_map {
        if !is_main_route_box(info) || is_loss_or_objective_box(id, info) {
            continue;
        }
        let target_width = readable_short_main_box_width(info);
        if box_width(info.bbox) + 0.001 >= target_width {
            continue;
        }
        let height = box_height(info.bbox);
        let candidates = widened_box_candidates(info.bbox, target_width, height);
        if let Some(candidate) = candidates.into_iter().find(|candidate| {
            route_box_candidate_is_clear(id, *candidate, &box_map)
                && candidate[0] >= 0.02
                && candidate[2] <= 0.98
        }) {
            updates.push((id.clone(), candidate));
        }
    }

    for (id, bbox) in updates {
        set_box_bbox(plan, &id, bbox);
        moved_ids.insert(id);
    }
    if !moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    }
}

fn readable_short_main_box_width(info: &BoxRouteInfo) -> f64 {
    let max_line_chars = info.text.lines().map(visible_text_len).max().unwrap_or(0) as f64;
    (0.06 + max_line_chars * 0.011).clamp(0.13, 0.18)
}

fn widen_short_input_boxes_for_readability(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut updates = Vec::new();
    let mut moved_ids = HashSet::new();

    for (id, info) in &box_map {
        if !is_short_input_phrase_box(id, info) {
            continue;
        }
        let target_width = readable_short_input_box_width(info);
        if box_width(info.bbox) + 0.001 >= target_width {
            continue;
        }
        let height = box_height(info.bbox);
        let mut candidates = widened_box_candidates(info.bbox, target_width, height);
        push_unique_box(
            &mut candidates,
            normalize_box([
                info.bbox[0].max(0.02),
                info.bbox[1],
                info.bbox[0].max(0.02) + target_width,
                info.bbox[3],
            ]),
        );
        if let Some(candidate) = candidates.into_iter().find(|candidate| {
            route_box_candidate_is_clear(id, *candidate, &box_map)
                && candidate[0] >= 0.0
                && candidate[2] <= 0.98
        }) {
            updates.push((id.clone(), candidate));
        }
    }

    for (id, bbox) in updates {
        set_box_bbox(plan, &id, bbox);
        moved_ids.insert(id);
    }
    if !moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    }
}

fn is_short_input_phrase_box(id: &str, info: &BoxRouteInfo) -> bool {
    if info.text.contains('\n') || info.text.split_whitespace().count() < 2 {
        return false;
    }
    let identity = format!("{} {} {} {}", id, info.text, info.role, info.style).to_lowercase();
    (identity.contains("input") || identity.contains("source"))
        && visible_text_len(&info.text) <= 24
        && (info.bbox[0] <= 0.04 || info.bbox[2] >= 0.96)
        && !is_loss_or_objective_box(id, info)
}

fn readable_short_input_box_width(info: &BoxRouteInfo) -> f64 {
    let max_line_chars = info.text.lines().map(visible_text_len).max().unwrap_or(0) as f64;
    (0.060 + max_line_chars * 0.010).clamp(0.145, 0.18)
}

fn widened_box_candidates(bbox: [f64; 4], width: f64, height: f64) -> Vec<[f64; 4]> {
    let bbox = normalize_box(bbox);
    let mut candidates = Vec::new();
    push_unique_box(
        &mut candidates,
        box_with_size_preserving_center(bbox, width, height),
    );
    push_unique_box(
        &mut candidates,
        [bbox[0], bbox[1], bbox[0] + width, bbox[3]],
    );
    push_unique_box(
        &mut candidates,
        [bbox[2] - width, bbox[1], bbox[2], bbox[3]],
    );
    candidates
        .into_iter()
        .filter(|candidate| candidate[0] >= 0.02 && candidate[2] <= 0.98)
        .collect()
}

fn remove_embedded_task_loss_text_from_main_boxes(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let has_separate_task_loss = box_map
        .iter()
        .any(|(id, info)| is_task_loss_route_box(id, info) && !is_main_route_box(info));
    if !has_separate_task_loss {
        return;
    }

    for object in &mut plan.objects {
        let DrawObject::Box {
            id,
            text,
            role,
            style,
            ..
        } = object
        else {
            continue;
        };
        let info = BoxRouteInfo {
            bbox: [0.0, 0.0, 0.0, 0.0],
            text: text.clone(),
            role: role.clone(),
            style: style.clone(),
        };
        if !is_main_route_box(&info) || box_identity_is_loss_or_objective(id, role, style) {
            continue;
        }
        let cleaned = remove_task_loss_fragment_from_text(text);
        if cleaned != *text && !cleaned.trim().is_empty() {
            *text = cleaned;
        }
    }
}

fn remove_task_loss_fragment_from_text(text: &str) -> String {
    let mut cleaned_lines = Vec::new();
    for line in text.lines() {
        let lower = line.to_lowercase();
        if !lower.contains("task loss") {
            cleaned_lines.push(line.trim().to_string());
            continue;
        }
        let mut cleaned = line
            .replace("+ Task Loss", "")
            .replace("+ task loss", "")
            .replace("Task Loss", "")
            .replace("task loss", "")
            .replace("  ", " ");
        cleaned = cleaned
            .trim_matches(|ch: char| ch == '+' || ch.is_whitespace())
            .to_string();
        if !cleaned.is_empty() {
            cleaned_lines.push(cleaned);
        }
    }
    cleaned_lines.join("\n")
}

fn remove_redundant_inference_only_parentheticals_from_main_boxes(plan: &mut DrawPlan) {
    for object in &mut plan.objects {
        let DrawObject::Box {
            id,
            text,
            role,
            style,
            ..
        } = object
        else {
            continue;
        };
        let info = BoxRouteInfo {
            bbox: [0.0, 0.0, 0.0, 0.0],
            text: text.clone(),
            role: role.clone(),
            style: style.clone(),
        };
        if !is_main_route_box(&info) || is_output_route_box(id, &info) {
            continue;
        }
        let cleaned = remove_redundant_inference_only_parenthetical_from_text(text);
        if cleaned != *text && !cleaned.trim().is_empty() {
            *text = cleaned;
        }
    }
}

fn remove_redundant_inference_only_parenthetical_from_text(text: &str) -> String {
    let mut cleaned_lines = Vec::new();
    for line in text.lines() {
        if is_standalone_inference_only_parenthetical(line) {
            continue;
        }
        let cleaned = line.trim().to_string();
        if !cleaned.is_empty() {
            cleaned_lines.push(cleaned);
        }
    }

    if cleaned_lines.is_empty() {
        text.to_string()
    } else {
        cleaned_lines.join("\n")
    }
}

fn split_embedded_inference_notes_from_output_boxes(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut annotation_bbox = None;

    for object in &mut plan.objects {
        let DrawObject::Box {
            id,
            bbox,
            text,
            role,
            style,
            ..
        } = object
        else {
            continue;
        };
        let info = BoxRouteInfo {
            bbox: *bbox,
            text: text.clone(),
            role: role.clone(),
            style: style.clone(),
        };
        if !is_output_route_box(id, &info) {
            continue;
        }
        let (cleaned, extracted) = remove_embedded_student_inference_note_from_text(text);
        if !extracted || cleaned.trim().is_empty() {
            continue;
        }
        *text = cleaned;
        if annotation_bbox.is_none() {
            annotation_bbox = Some(embedded_inference_annotation_bbox(id, *bbox, &box_map));
        }
    }

    if let Some(bbox) = annotation_bbox {
        ensure_inference_annotation(plan, "Inference: student only", bbox);
    }
}

fn remove_embedded_student_inference_note_from_text(text: &str) -> (String, bool) {
    let mut extracted = false;
    let mut cleaned_lines = Vec::new();
    for line in text.lines() {
        if is_standalone_inference_only_parenthetical(line) {
            extracted = true;
            continue;
        }
        let cleaned = remove_student_inference_parenthetical_fragment(line);
        if cleaned != line.trim() {
            extracted = true;
        }
        if !cleaned.trim().is_empty() {
            cleaned_lines.push(cleaned.trim().to_string());
        }
    }

    if cleaned_lines.is_empty() {
        (text.to_string(), false)
    } else {
        (cleaned_lines.join("\n"), extracted)
    }
}

fn remove_student_inference_parenthetical_fragment(line: &str) -> String {
    let mut cleaned = line.trim().to_string();
    for pattern in [
        "(inference: student only)",
        "(Inference: student only)",
        "(inference student only)",
        "(Inference student only)",
        "(inference-only)",
        "(Inference-only)",
        "(inference only)",
        "(Inference only)",
    ] {
        cleaned = cleaned.replace(pattern, "");
    }
    cleaned
        .trim_matches(|ch: char| ch == '-' || ch == ':' || ch.is_whitespace())
        .trim()
        .to_string()
}

fn embedded_inference_annotation_bbox(
    anchor_id: &str,
    anchor_bbox: [f64; 4],
    box_map: &HashMap<String, BoxRouteInfo>,
) -> [f64; 4] {
    let width = 0.18;
    let height = 0.055;
    let candidates = [
        box_from_top_left_inside(
            center_x(anchor_bbox) - width / 2.0,
            anchor_bbox[3] + 0.025,
            width,
            height,
        ),
        box_from_top_left_inside(
            center_x(anchor_bbox) - width / 2.0,
            anchor_bbox[1] - 0.025 - height,
            width,
            height,
        ),
        box_from_top_left_inside(
            anchor_bbox[2] + 0.025,
            center_y(anchor_bbox) - height / 2.0,
            width,
            height,
        ),
        box_from_top_left_inside(
            anchor_bbox[0] - 0.025 - width,
            center_y(anchor_bbox) - height / 2.0,
            width,
            height,
        ),
    ];
    candidates
        .into_iter()
        .find(|candidate| {
            embedded_inference_annotation_candidate_is_clear(anchor_id, *candidate, box_map)
        })
        .unwrap_or(candidates[0])
}

fn embedded_inference_annotation_candidate_is_clear(
    anchor_id: &str,
    candidate: [f64; 4],
    box_map: &HashMap<String, BoxRouteInfo>,
) -> bool {
    box_map.iter().all(|(id, info)| {
        id == anchor_id
            || (!component_overlap_gate_fails(candidate, info.bbox)
                && !component_crowding_gate_fails_normalized(candidate, info.bbox))
    })
}

fn is_standalone_inference_only_parenthetical(line: &str) -> bool {
    let normalized = line
        .trim()
        .trim_matches(|ch| ch == '(' || ch == ')')
        .replace(['-', '_'], " ")
        .replace(':', " ")
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    matches!(
        normalized.as_str(),
        "inference only" | "inference student only" | "student only inference"
    )
}

fn simplify_main_to_output_connectors(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let immutable_plan = plan.clone();
    for object in &mut plan.objects {
        let DrawObject::Connector {
            id,
            points,
            from,
            to,
            style,
            ..
        } = object
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(source), Some(output)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        if points.len() <= 3
            || !is_main_route_box(source)
            || !is_output_route_box(to_id, output)
            || connector_points_are_orthogonal(points) && points.len() <= 3
        {
            continue;
        }
        let style = style.to_lowercase();
        if style.contains("dash") {
            continue;
        }

        let compact = if let Some(y) = common_horizontal_y(source.bbox, output.bbox) {
            horizontal_connector_points(source.bbox, output.bbox, y)
        } else if let Some(x) = common_vertical_x(source.bbox, output.bbox) {
            vertical_connector_points(source.bbox, output.bbox, x)
        } else {
            orthogonal_connector_points_between_boxes(source.bbox, output.bbox)
        };
        if compact.len() < 2 || compact.len() >= points.len() {
            continue;
        }
        if connector_route_bbox_is_clear_excluding_endpoints(
            points_to_box(&compact),
            from_id,
            to_id,
            &box_map,
        ) && !connector_points_intersect_intermediate_boxes(&compact, from_id, to_id, &box_map)
            && !connector_route_conflicts_with_other_connectors(
                &compact,
                id.as_str(),
                &immutable_plan,
            )
        {
            *points = compact;
        }
    }
}

fn connector_route_bbox_is_clear_excluding_endpoints(
    candidate: [f64; 4],
    from_id: &str,
    to_id: &str,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> bool {
    box_map.iter().all(|(id, info)| {
        id == from_id || id == to_id || !component_overlap_gate_fails(candidate, info.bbox)
    })
}

fn box_identity_is_loss_or_objective(id: &str, role: &str, style: &str) -> bool {
    let identity = format!(
        "{} {} {}",
        id.to_lowercase(),
        role.to_lowercase(),
        style.to_lowercase()
    );
    identity.contains("loss")
        || identity.contains("objective")
        || identity.contains("residual")
        || identity.contains("supervision")
        || identity.contains("accent")
}

fn fold_unconnected_residual_boxes_into_supervision_labels(plan: &mut DrawPlan) {
    let connected_ids = connector_endpoint_ids(plan)
        .into_iter()
        .map(str::to_string)
        .collect::<HashSet<_>>();
    let residual_boxes = plan
        .objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Box {
                id,
                text,
                role,
                style,
                ..
            } = object
            else {
                return None;
            };
            (!connected_ids.contains(id)
                && is_foldable_unconnected_residual_box(id, text, role, style))
            .then(|| (id.clone(), normalize_residual_edge_label(text)))
        })
        .collect::<Vec<_>>();
    if residual_boxes.is_empty() {
        return;
    }

    let residual_ids = residual_boxes
        .iter()
        .map(|(id, _)| id.clone())
        .collect::<HashSet<_>>();
    let mut folded_ids = HashSet::new();
    for (residual_id, label_text) in &residual_boxes {
        if let Some(edge_id) = best_residual_supervision_edge_for_label(plan, &residual_ids) {
            set_or_preserve_residual_edge_label(plan, &edge_id, label_text);
            simplify_residual_supervision_edge_route(plan, &edge_id);
            folded_ids.insert(residual_id.clone());
        }
    }
    if folded_ids.is_empty() {
        return;
    }

    plan.objects.retain(|object| match object {
        DrawObject::Box { id, .. } => !folded_ids.contains(id),
        DrawObject::Connector { from, to, .. } => {
            !from.as_deref().is_some_and(|id| folded_ids.contains(id))
                && !to.as_deref().is_some_and(|id| folded_ids.contains(id))
        }
        _ => true,
    });
}

fn is_foldable_unconnected_residual_box(id: &str, text: &str, role: &str, style: &str) -> bool {
    let descriptor = format!(
        "{} {} {} {}",
        id.to_lowercase(),
        text.to_lowercase(),
        role.to_lowercase(),
        style.to_lowercase()
    );
    if residual_text_looks_like_formula(text) {
        return false;
    }
    descriptor.contains("residual")
        && !descriptor.contains("task")
        && !descriptor.contains("input")
        && !descriptor.contains("output")
        && (descriptor.contains("loss")
            || descriptor.contains("accent")
            || descriptor.contains("supervision")
            || descriptor.contains("latent"))
}

fn residual_text_looks_like_formula(text: &str) -> bool {
    let text = text.to_lowercase();
    text.contains("h_")
        || text.contains("z_")
        || text.contains("w_")
        || text.contains('−')
        || text.contains('-')
        || text.contains('+')
        || text.contains('=')
        || text.contains("||")
        || text.contains("mse")
        || text.contains("ce(")
        || text.contains("cos")
        || text.contains("^")
}

fn fold_connected_residual_signal_boxes_between_branch_rows(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let folds = box_map
        .iter()
        .filter_map(|(residual_id, residual)| {
            if !is_foldable_connected_residual_signal_box(residual_id, residual) {
                return None;
            }
            let (teacher_id, teacher, student_id, student, keep_edge_id, remove_edge_ids) =
                connected_branch_pair_for_residual_signal(plan, residual_id, &box_map)?;
            if !simple_teacher_student_branch_route_pair(&teacher_id, teacher, &student_id, student)
            {
                return None;
            }
            if !residual_signal_sits_in_branch_gap(residual.bbox, teacher.bbox, student.bbox) {
                return None;
            }
            Some((
                residual_id.clone(),
                keep_edge_id,
                remove_edge_ids,
                teacher_id,
                teacher.bbox,
                student_id,
                student.bbox,
                normalize_residual_edge_label(&residual.text),
            ))
        })
        .collect::<Vec<_>>();
    if folds.is_empty() {
        return;
    }

    let mut removed_box_ids = HashSet::new();
    let mut removed_edge_ids = HashSet::new();
    for (
        residual_id,
        keep_edge_id,
        edge_ids_to_remove,
        teacher_id,
        teacher_bbox,
        student_id,
        student_bbox,
        label_text,
    ) in folds
    {
        if let Some(DrawObject::Connector {
            points,
            from,
            to,
            style,
            label,
            ..
        }) = plan.objects.iter_mut().find(
            |object| matches!(object, DrawObject::Connector { id, .. } if id == &keep_edge_id),
        ) {
            *from = Some(teacher_id);
            *to = Some(student_id);
            *style = "dashed_supervision".to_string();
            *points = residual_signal_bridge_route(teacher_bbox, student_bbox);
            *label = Some(DrawLabel {
                text: label_text,
                bbox: residual_signal_bridge_label_bbox(points),
            });
            removed_box_ids.insert(residual_id);
            removed_edge_ids.extend(edge_ids_to_remove);
        }
    }

    if removed_box_ids.is_empty() {
        return;
    }
    plan.objects.retain(|object| match object {
        DrawObject::Box { id, .. } => !removed_box_ids.contains(id),
        DrawObject::Connector { id, from, to, .. } => {
            !removed_edge_ids.contains(id)
                && !from
                    .as_deref()
                    .is_some_and(|endpoint| removed_box_ids.contains(endpoint))
                && !to
                    .as_deref()
                    .is_some_and(|endpoint| removed_box_ids.contains(endpoint))
        }
        _ => true,
    });
}

fn is_foldable_connected_residual_signal_box(id: &str, info: &BoxRouteInfo) -> bool {
    let descriptor = route_box_text(id, info);
    if !(descriptor.contains("residual") || descriptor.contains("latent")) {
        return false;
    }
    if descriptor.contains("task")
        || descriptor.contains("input")
        || descriptor.contains("output")
        || descriptor.contains("teacher")
        || descriptor.contains("student")
    {
        return false;
    }
    !residual_text_looks_like_formula(&info.text) && visible_text_len(&info.text) <= 24
}

fn connected_branch_pair_for_residual_signal<'a>(
    plan: &DrawPlan,
    residual_id: &str,
    box_map: &'a HashMap<String, BoxRouteInfo>,
) -> Option<(
    String,
    &'a BoxRouteInfo,
    String,
    &'a BoxRouteInfo,
    String,
    Vec<String>,
)> {
    let mut teacher: Option<(String, &'a BoxRouteInfo, String)> = None;
    let mut student: Option<(String, &'a BoxRouteInfo, String)> = None;
    let mut touching_edges = Vec::new();

    for object in &plan.objects {
        let DrawObject::Connector { id, from, to, .. } = object else {
            continue;
        };
        let opposite_id = if from.as_deref() == Some(residual_id) {
            to.as_deref()
        } else if to.as_deref() == Some(residual_id) {
            from.as_deref()
        } else {
            None
        };
        let Some(opposite_id) = opposite_id else {
            continue;
        };
        let Some(opposite) = box_map.get(opposite_id) else {
            continue;
        };
        touching_edges.push(id.clone());
        if is_teacher_or_context_route_box(opposite_id, opposite) {
            teacher.get_or_insert((opposite_id.to_string(), opposite, id.clone()));
        } else if is_student_or_main_route_box(opposite) {
            student.get_or_insert((opposite_id.to_string(), opposite, id.clone()));
        }
    }

    let (teacher_id, teacher_info, keep_edge_id) = teacher?;
    let (student_id, student_info, _) = student?;
    let remove_edge_ids = touching_edges
        .into_iter()
        .filter(|id| id != &keep_edge_id)
        .collect::<Vec<_>>();
    Some((
        teacher_id,
        teacher_info,
        student_id,
        student_info,
        keep_edge_id,
        remove_edge_ids,
    ))
}

fn simple_teacher_student_branch_route_pair(
    teacher_id: &str,
    teacher: &BoxRouteInfo,
    student_id: &str,
    student: &BoxRouteInfo,
) -> bool {
    simple_branch_route_label(teacher_id, teacher, "teacher")
        && simple_branch_route_label(student_id, student, "student")
}

fn simple_branch_route_label(id: &str, info: &BoxRouteInfo, keyword: &str) -> bool {
    let descriptor = route_box_text(id, info);
    if !descriptor.contains(keyword) {
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
    .any(|token| descriptor.contains(token))
}

fn residual_signal_sits_in_branch_gap(
    residual_bbox: [f64; 4],
    teacher_bbox: [f64; 4],
    student_bbox: [f64; 4],
) -> bool {
    if !objective_hub_sits_between_branch_rows(residual_bbox, &[teacher_bbox, student_bbox]) {
        return false;
    }
    let branch_span = union_box(teacher_bbox, student_bbox);
    horizontal_overlap(residual_bbox, branch_span) > 0.0
        || horizontal_separation(residual_bbox, branch_span) <= 0.09
}

fn residual_signal_bridge_route(teacher_bbox: [f64; 4], student_bbox: [f64; 4]) -> Vec<[f64; 2]> {
    let right_x = teacher_bbox[2].max(student_bbox[2]);
    let (start_x, rail_x) = if right_x + 0.035 <= 0.94 {
        (right_x, (right_x + 0.058).clamp(right_x + 0.035, 0.94))
    } else {
        let left_x = teacher_bbox[0].min(student_bbox[0]);
        let rail_max = (left_x - 0.035).max(0.02);
        let rail_x = if rail_max > 0.06 {
            (left_x - 0.058).clamp(0.06, rail_max)
        } else {
            rail_max
        };
        (left_x, rail_x)
    };
    let start = [start_x, center_y(teacher_bbox)];
    let end = [start_x, center_y(student_bbox)];
    remove_redundant_collinear_points(&[start, [rail_x, start[1]], [rail_x, end[1]], end])
}

fn residual_signal_bridge_label_bbox(points: &[[f64; 2]]) -> [f64; 4] {
    let edge_bbox = points_to_box(points);
    let width = 0.16;
    let height = 0.055;
    let x1 = (edge_bbox[2] + 0.026).clamp(0.02, 0.98 - width);
    let y1 = (center_y(edge_bbox) - height / 2.0).clamp(0.02, 0.98 - height);
    [x1, y1, x1 + width, y1 + height]
}

fn normalize_residual_edge_label(text: &str) -> String {
    let trimmed = text.trim().replace('\n', " ");
    if trimmed.is_empty() {
        "Latent Residual".to_string()
    } else {
        trimmed
    }
}

fn best_residual_supervision_edge_for_label(
    plan: &DrawPlan,
    residual_ids: &HashSet<String>,
) -> Option<String> {
    plan.objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Connector {
                id,
                from,
                to,
                style,
                label,
                ..
            } = object
            else {
                return None;
            };
            if from.as_deref().is_some_and(|id| residual_ids.contains(id))
                || to.as_deref().is_some_and(|id| residual_ids.contains(id))
            {
                return None;
            }
            let descriptor = format!(
                "{} {} {}",
                id.to_lowercase(),
                style.to_lowercase(),
                label
                    .as_ref()
                    .map(|label| label.text.to_lowercase())
                    .unwrap_or_default()
            );
            let mut score = 0;
            if descriptor.contains("residual") {
                score += 6;
            }
            if descriptor.contains("supervision") {
                score += 4;
            }
            if descriptor.contains("dash") || descriptor.contains("dashed") {
                score += 2;
            }
            (score > 0).then(|| (id.clone(), score))
        })
        .max_by_key(|(_, score)| *score)
        .map(|(id, _)| id)
}

fn set_or_preserve_residual_edge_label(plan: &mut DrawPlan, edge_id: &str, label_text: &str) {
    let Some(DrawObject::Connector { points, label, .. }) = plan
        .objects
        .iter_mut()
        .find(|object| matches!(object, DrawObject::Connector { id, .. } if id == edge_id))
    else {
        return;
    };
    let first = points.first().copied().unwrap_or([0.5, 0.5]);
    let last = points.last().copied().unwrap_or(first);
    match label {
        Some(existing) if !is_generic_residual_supervision_label(&existing.text) => {}
        Some(existing) => {
            existing.text = label_text.to_string();
            existing.bbox = connector_label_bbox(first, last);
        }
        None => {
            *label = Some(DrawLabel {
                text: label_text.to_string(),
                bbox: connector_label_bbox(first, last),
            });
        }
    }
}

fn simplify_residual_supervision_edge_route(plan: &mut DrawPlan, edge_id: &str) {
    let box_map = current_box_map(plan);
    let Some(DrawObject::Connector {
        points, from, to, ..
    }) = plan
        .objects
        .iter_mut()
        .find(|object| matches!(object, DrawObject::Connector { id, .. } if id == edge_id))
    else {
        return;
    };
    let (Some(from_box), Some(to_box)) = (
        from.as_deref().and_then(|id| box_map.get(id).copied()),
        to.as_deref().and_then(|id| box_map.get(id).copied()),
    ) else {
        return;
    };
    let start = anchor_point_towards(from_box, center(to_box));
    let end = anchor_point_towards(to_box, center(from_box));
    *points = remove_redundant_collinear_points(&[start, [end[0], start[1]], end]);
}

fn widen_output_boxes_for_long_tokens(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut updates = Vec::new();
    let mut moved_ids = HashSet::new();

    for (id, info) in &box_map {
        if !is_output_route_box(id, info) || is_task_loss_route_box(id, info) {
            continue;
        }
        let target_width = readable_output_width(info);
        if box_width(info.bbox) + 0.001 >= target_width {
            continue;
        }
        let candidate =
            box_with_size_preserving_center(info.bbox, target_width, box_height(info.bbox));
        if route_box_candidate_is_clear(id, candidate, &box_map) {
            updates.push((id.clone(), candidate));
        }
    }

    for (id, bbox) in updates {
        set_box_bbox(plan, &id, bbox);
        moved_ids.insert(id);
    }
    if !moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    }
}

fn readable_output_width(info: &BoxRouteInfo) -> f64 {
    let max_token_chars = info
        .text
        .split(|ch: char| ch.is_whitespace() || ch == '_' || ch == '-')
        .map(|token| token.chars().count())
        .max()
        .unwrap_or(0) as f64;
    (0.045 + max_token_chars * 0.011).clamp(0.10, 0.20)
}

fn compact_wide_residual_supervision_boxes(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut updates = Vec::new();
    let mut moved_ids = HashSet::new();

    for (id, info) in &box_map {
        if !is_wide_residual_supervision_box(id, info) {
            continue;
        }
        let target_width = box_width(info.bbox).clamp(0.24, 0.30);
        if box_width(info.bbox) <= target_width + 0.002 {
            continue;
        }
        let center_x = center_x(info.bbox);
        let x1 = (center_x - target_width / 2.0).clamp(0.02, 0.98 - target_width);
        let candidate = [x1, info.bbox[1], x1 + target_width, info.bbox[3]];
        if route_box_candidate_is_clear(id, candidate, &box_map) {
            updates.push((id.clone(), candidate));
        }
    }

    for (id, bbox) in updates {
        set_box_bbox(plan, &id, bbox);
        moved_ids.insert(id);
    }
    if !moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    }
}

fn is_wide_residual_supervision_box(id: &str, info: &BoxRouteInfo) -> bool {
    let text = route_box_text(id, info);
    box_width(info.bbox) > 0.34
        && box_height(info.bbox) <= 0.14
        && text.contains("residual")
        && (text.contains("supervision") || text.contains("l_res") || text.contains("latent"))
        && is_objective_route_box(id, info)
}

fn visible_text_len(text: &str) -> usize {
    text.chars()
        .filter(|ch| !ch.is_whitespace() && *ch != '_' && *ch != '-')
        .count()
}

fn box_with_size_preserving_center(bbox: [f64; 4], width: f64, height: f64) -> [f64; 4] {
    let bbox = normalize_box(bbox);
    let center_x = center_x(bbox).clamp(width / 2.0, 1.0 - width / 2.0);
    let center_y = center_y(bbox).clamp(height / 2.0, 1.0 - height / 2.0);
    normalize_box([
        center_x - width / 2.0,
        center_y - height / 2.0,
        center_x + width / 2.0,
        center_y + height / 2.0,
    ])
}

fn widen_short_context_note_boxes_for_text(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut updates = Vec::new();
    let mut moved_ids = HashSet::new();

    for (id, info) in &box_map {
        if !is_context_note_box(id, info) {
            continue;
        }
        let target_width = readable_context_note_width(info);
        if box_width(info.bbox) + 0.001 >= target_width {
            continue;
        }
        let candidate =
            box_with_size_preserving_center(info.bbox, target_width, box_height(info.bbox));
        if route_box_candidate_is_clear(id, candidate, &box_map) {
            updates.push((id.clone(), candidate));
        }
    }

    for (id, bbox) in updates {
        set_box_bbox(plan, &id, bbox);
        moved_ids.insert(id);
    }
    if !moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    }
}

fn is_context_note_box(id: &str, info: &BoxRouteInfo) -> bool {
    let text = format!(
        "{} {} {} {}",
        id.to_lowercase(),
        info.text.to_lowercase(),
        info.role.to_lowercase(),
        info.style.to_lowercase()
    );
    (text.contains("note") || text.contains("inference") || text.contains("student only"))
        && !is_loss_or_objective_box(id, info)
        && !is_output_route_box(id, info)
        && !is_input_route_box(id, info)
}

fn readable_context_note_width(info: &BoxRouteInfo) -> f64 {
    let longest_token = info
        .text
        .split_whitespace()
        .map(visible_text_len)
        .max()
        .unwrap_or(0) as f64;
    (0.08 + longest_token * 0.010).clamp(0.14, 0.24)
}

fn compact_single_line_flow_boxes_in_vertical_stacks(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut updates: HashMap<String, [f64; 4]> = HashMap::new();
    let target_gap = 0.055;

    for object in &plan.objects {
        let DrawObject::Connector { from, to, .. } = object else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(from_box), Some(to_box)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        if vertical_separation(from_box.bbox, to_box.bbox) <= 0.0
            || vertical_separation(from_box.bbox, to_box.bbox) >= target_gap
            || axis_overlap_ratio(
                from_box.bbox[0],
                from_box.bbox[2],
                to_box.bbox[0],
                to_box.bbox[2],
            ) <= 0.25
        {
            continue;
        }
        let (upper_id, upper, lower) = if center_y(from_box.bbox) <= center_y(to_box.bbox) {
            (from_id, from_box, to_box)
        } else {
            (to_id, to_box, from_box)
        };
        if updates.contains_key(upper_id) || !is_compactable_single_line_flow_box(upper_id, upper) {
            continue;
        }
        let Some(candidate) =
            compact_upper_box_to_vertical_gutter(upper.bbox, lower.bbox, target_gap)
        else {
            continue;
        };
        updates.insert(upper_id.to_string(), candidate);
    }

    for (upper_id, upper) in &box_map {
        if updates.contains_key(upper_id) || !is_compactable_single_line_flow_box(upper_id, upper) {
            continue;
        }
        for (lower_id, lower) in &box_map {
            if lower_id == upper_id
                || center_y(upper.bbox) >= center_y(lower.bbox)
                || vertical_separation(upper.bbox, lower.bbox) <= 0.0
                || vertical_separation(upper.bbox, lower.bbox) >= target_gap
                || axis_overlap_ratio(upper.bbox[0], upper.bbox[2], lower.bbox[0], lower.bbox[2])
                    <= 0.25
            {
                continue;
            }
            let Some(candidate) =
                compact_upper_box_to_vertical_gutter(upper.bbox, lower.bbox, target_gap)
            else {
                continue;
            };
            updates.insert(upper_id.clone(), candidate);
            break;
        }
    }

    if updates.is_empty() {
        return;
    }
    let mut moved_ids = HashSet::new();
    for (id, bbox) in updates {
        set_box_bbox(plan, &id, bbox);
        moved_ids.insert(id);
    }
    realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
}

fn is_compactable_single_line_flow_box(id: &str, info: &BoxRouteInfo) -> bool {
    let text = info.text.to_lowercase();
    let id_lower = id.to_lowercase();
    let is_student_head_like = id_lower.contains("student") && text.contains("head")
        || id_lower.contains("head") && (text.contains("student") || text.contains("task"));
    if text.contains('\n') && !is_student_head_like
        || box_height(info.bbox) <= 0.12
        || is_objective_route_box(id, info)
        || is_output_route_box(id, info)
        || is_context_note_box(id, info)
    {
        return false;
    }
    visible_text_len(&info.text) <= 24
}

fn compact_upper_box_to_vertical_gutter(
    upper: [f64; 4],
    lower: [f64; 4],
    target_gap: f64,
) -> Option<[f64; 4]> {
    let desired_y2 = lower[1] - target_gap;
    let min_height = 0.10;
    if desired_y2 <= upper[1] + min_height {
        return None;
    }
    let y2 = desired_y2.min(upper[3]);
    (y2 - upper[1] >= min_height).then_some([upper[0], upper[1], upper[2], y2])
}

#[derive(Clone, Debug)]
struct StudentBranchChain {
    encoder_id: String,
    latent_id: String,
    head_id: String,
    output_id: Option<String>,
}

fn stack_crowded_student_branch_chains(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let chains = student_branch_chains(plan, &box_map);
    if chains.is_empty() {
        return;
    }

    let mut moved_ids = HashSet::new();
    for chain in chains {
        let box_map = current_box_route_info_map(plan);
        let (Some(encoder), Some(latent), Some(head)) = (
            box_map.get(&chain.encoder_id),
            box_map.get(&chain.latent_id),
            box_map.get(&chain.head_id),
        ) else {
            continue;
        };
        if !student_branch_chain_needs_stack_repair(&chain, encoder, latent, head, plan, &box_map) {
            continue;
        }

        let chain_ids = student_branch_chain_ids(&chain);
        let branch_center_x =
            ((center_x(encoder.bbox) + center_x(latent.bbox) + center_x(head.bbox)) / 3.0)
                .clamp(0.18, 0.62);
        let encoder_width = box_width(encoder.bbox).clamp(0.16, 0.22);
        let latent_width = box_width(latent.bbox).clamp(0.08, 0.13);
        let head_width = box_width(head.bbox).clamp(0.10, 0.16);
        let encoder_height = box_height(encoder.bbox).clamp(0.095, 0.115);
        let latent_height = box_height(latent.bbox).clamp(0.085, 0.105);
        let head_height = box_height(head.bbox).clamp(0.095, 0.115);
        let gap = 0.060;
        let total_height = encoder_height + latent_height + head_height + gap * 2.0;
        let original_top = encoder.bbox[1].min(latent.bbox[1]).min(head.bbox[1]);
        let top = original_top.clamp(0.08, 0.94 - total_height);

        let encoder_bbox = box_from_center_size(
            branch_center_x,
            top + encoder_height / 2.0,
            encoder_width,
            encoder_height,
        );
        let latent_y1 = encoder_bbox[3] + gap;
        let latent_bbox = box_from_center_size(
            branch_center_x,
            latent_y1 + latent_height / 2.0,
            latent_width,
            latent_height,
        );
        let head_y1 = latent_bbox[3] + gap;
        let head_bbox = box_from_center_size(
            branch_center_x,
            head_y1 + head_height / 2.0,
            head_width,
            head_height,
        );

        let updates = [
            (chain.encoder_id.as_str(), encoder_bbox),
            (chain.latent_id.as_str(), latent_bbox),
            (chain.head_id.as_str(), head_bbox),
        ];
        if updates
            .iter()
            .all(|(id, bbox)| route_box_candidate_is_clear_except(id, *bbox, &box_map, &chain_ids))
        {
            for (id, bbox) in updates {
                set_box_bbox(plan, id, bbox);
                moved_ids.insert(id.to_string());
            }
        }

        if let Some(output_id) = &chain.output_id {
            let box_map = current_box_route_info_map(plan);
            if let (Some(head), Some(output)) =
                (box_map.get(&chain.head_id), box_map.get(output_id))
            {
                let output_width = compact_output_width(output_id, output).clamp(0.04, 0.075);
                let output_height = box_height(output.bbox).clamp(0.075, 0.10);
                let x1 = (head.bbox[2] + 0.055).clamp(0.02, 0.98 - output_width);
                let y1 =
                    (center_y(head.bbox) - output_height / 2.0).clamp(0.02, 0.98 - output_height);
                let candidate = [x1, y1, x1 + output_width, y1 + output_height];
                if route_box_candidate_is_clear_except(
                    output_id,
                    candidate,
                    &box_map,
                    &student_branch_chain_ids(&chain),
                ) {
                    set_box_bbox(plan, output_id, candidate);
                    moved_ids.insert(output_id.clone());
                }
            }
        }

        reroute_student_branch_chain(plan, &chain);
    }

    if !moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    }
}

fn student_branch_chains(
    plan: &DrawPlan,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Vec<StudentBranchChain> {
    let connectors = plan
        .objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Connector { from, to, .. } = object else {
                return None;
            };
            Some((from.as_deref()?, to.as_deref()?))
        })
        .collect::<Vec<_>>();
    let mut chains = Vec::new();
    for (encoder_id, latent_id) in &connectors {
        let (Some(encoder), Some(latent)) = (box_map.get(*encoder_id), box_map.get(*latent_id))
        else {
            continue;
        };
        if !is_student_chain_encoder(encoder_id, encoder)
            || !is_student_chain_latent(latent_id, latent)
        {
            continue;
        }
        let Some((_, head_id)) = connectors.iter().find(|(from, to)| {
            *from == *latent_id
                && box_map
                    .get(*to)
                    .is_some_and(|candidate| is_student_chain_head(to, candidate))
        }) else {
            continue;
        };
        let output_id = connectors
            .iter()
            .find(|(from, to)| {
                *from == *head_id
                    && box_map
                        .get(*to)
                        .is_some_and(|candidate| is_output_route_box(to, candidate))
            })
            .map(|(_, to)| (*to).to_string());
        chains.push(StudentBranchChain {
            encoder_id: (*encoder_id).to_string(),
            latent_id: (*latent_id).to_string(),
            head_id: (*head_id).to_string(),
            output_id,
        });
    }
    chains
}

fn stack_student_encoder_head_pairs_top_down(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut updates = Vec::new();
    let mut moved_ids = HashSet::new();

    for object in &plan.objects {
        let DrawObject::Connector { from, to, .. } = object else {
            continue;
        };
        let (Some(encoder_id), Some(head_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(encoder), Some(head)) = (box_map.get(encoder_id), box_map.get(head_id)) else {
            continue;
        };
        if !is_student_encoder_route_box(encoder_id, encoder)
            || !is_student_head_route_box(head_id, head)
            || center_y(encoder.bbox) < center_y(head.bbox)
        {
            continue;
        }
        let encoder_height = box_height(encoder.bbox);
        let head_height = box_height(head.bbox);
        let gap = 0.075;
        let top = encoder.bbox[1]
            .min(head.bbox[1])
            .clamp(0.06, 0.94 - encoder_height - gap - head_height);
        let encoder_candidate = [encoder.bbox[0], top, encoder.bbox[2], top + encoder_height];
        let head_top = top + encoder_height + gap;
        let head_candidate = [head.bbox[0], head_top, head.bbox[2], head_top + head_height];
        let ignored = HashSet::from([encoder_id.to_string(), head_id.to_string()]);
        if route_box_candidate_is_clear_except(encoder_id, encoder_candidate, &box_map, &ignored)
            && route_box_candidate_is_clear_except(head_id, head_candidate, &box_map, &ignored)
        {
            updates.push((encoder_id.to_string(), encoder_candidate));
            updates.push((head_id.to_string(), head_candidate));
            moved_ids.insert(encoder_id.to_string());
            moved_ids.insert(head_id.to_string());
        }
    }

    if updates.is_empty() {
        return;
    }
    for (id, bbox) in updates {
        set_box_bbox(plan, &id, bbox);
    }
    realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    reroute_student_encoder_head_connectors(plan, &moved_ids);
}

fn reroute_student_encoder_head_connectors(plan: &mut DrawPlan, moved_ids: &HashSet<String>) {
    let box_map = current_box_route_info_map(plan);
    for object in &mut plan.objects {
        let DrawObject::Connector {
            points, from, to, ..
        } = object
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        if !moved_ids.contains(from_id) && !moved_ids.contains(to_id) {
            continue;
        }
        let (Some(from_box), Some(to_box)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        if is_student_encoder_route_box(from_id, from_box)
            && is_student_head_route_box(to_id, to_box)
        {
            *points = orthogonal_connector_points_between_boxes(from_box.bbox, to_box.bbox);
        }
    }
}

fn is_student_encoder_route_box(id: &str, info: &BoxRouteInfo) -> bool {
    let text = route_box_text(id, info);
    text.contains("student") && text.contains("encoder")
}

fn is_student_head_route_box(id: &str, info: &BoxRouteInfo) -> bool {
    let text = route_box_text(id, info);
    text.contains("student") && text.contains("head")
}

fn student_branch_chain_needs_stack_repair(
    chain: &StudentBranchChain,
    encoder: &BoxRouteInfo,
    latent: &BoxRouteInfo,
    head: &BoxRouteInfo,
    plan: &DrawPlan,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> bool {
    if component_overlap_gate_fails(encoder.bbox, latent.bbox)
        || horizontal_separation(encoder.bbox, latent.bbox) < 0.035
            && axis_overlap_ratio(
                encoder.bbox[1],
                encoder.bbox[3],
                latent.bbox[1],
                latent.bbox[3],
            ) > 0.25
    {
        return true;
    }
    if center_y(encoder.bbox) >= center_y(latent.bbox)
        || center_y(latent.bbox) >= center_y(head.bbox)
        || vertical_separation(encoder.bbox, latent.bbox) < 0.055
        || vertical_separation(latent.bbox, head.bbox) < 0.055
    {
        return true;
    }
    if (center_x(latent.bbox) - center_x(head.bbox)).abs() > 0.08 {
        return true;
    }
    let Some(output_id) = &chain.output_id else {
        return false;
    };
    let Some(output) = box_map.get(output_id) else {
        return false;
    };
    if box_area(output.bbox) > 0.012 && visible_text_len(&output.text) <= 3 {
        return true;
    }
    if horizontal_separation(head.bbox, output.bbox) < 0.045
        && axis_overlap_ratio(head.bbox[1], head.bbox[3], output.bbox[1], output.bbox[3]) > 0.35
    {
        return true;
    }
    plan.objects.iter().any(|object| {
        matches!(
            object,
            DrawObject::Connector { from, to, points, .. }
                if from.as_deref() == Some(chain.head_id.as_str())
                    && to.as_deref() == Some(output_id.as_str())
                    && points.len() > 2
        )
    })
}

fn student_branch_chain_ids(chain: &StudentBranchChain) -> HashSet<String> {
    let mut ids = HashSet::from([
        chain.encoder_id.clone(),
        chain.latent_id.clone(),
        chain.head_id.clone(),
    ]);
    if let Some(output_id) = &chain.output_id {
        ids.insert(output_id.clone());
    }
    ids
}

fn is_student_chain_encoder(id: &str, info: &BoxRouteInfo) -> bool {
    let text = route_box_text(id, info);
    text.contains("student")
        && !text.contains("latent")
        && !text.contains("head")
        && !text.contains("output")
        && !text.contains("inference")
}

fn is_student_chain_latent(id: &str, info: &BoxRouteInfo) -> bool {
    let text = route_box_text(id, info);
    text.contains("student_latent")
        || text.contains("student latent")
        || text.contains("z_s")
        || text.contains("zs")
}

fn is_student_chain_head(id: &str, info: &BoxRouteInfo) -> bool {
    route_box_text(id, info).contains("head")
}

fn route_box_candidate_is_clear_except(
    moving_id: &str,
    candidate: [f64; 4],
    box_map: &HashMap<String, BoxRouteInfo>,
    ignored_ids: &HashSet<String>,
) -> bool {
    box_map.iter().all(|(id, info)| {
        id == moving_id
            || ignored_ids.contains(id)
            || !component_overlap_gate_fails(candidate, info.bbox)
    })
}

fn reroute_student_branch_chain(plan: &mut DrawPlan, chain: &StudentBranchChain) {
    let box_map = current_box_route_info_map(plan);
    for object in &mut plan.objects {
        let DrawObject::Connector {
            points, from, to, ..
        } = object
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let belongs_to_chain = (from_id == chain.encoder_id && to_id == chain.latent_id)
            || (from_id == chain.latent_id && to_id == chain.head_id)
            || chain
                .output_id
                .as_deref()
                .is_some_and(|output_id| from_id == chain.head_id && to_id == output_id);
        if !belongs_to_chain {
            continue;
        }
        let (Some(from_box), Some(to_box)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        *points = orthogonal_connector_points_between_boxes(from_box.bbox, to_box.bbox);
    }
}

fn box_from_center_size(center_x: f64, center_y: f64, width: f64, height: f64) -> [f64; 4] {
    [
        center_x - width / 2.0,
        center_y - height / 2.0,
        center_x + width / 2.0,
        center_y + height / 2.0,
    ]
}

fn move_context_notes_away_from_loss_boxes(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut updates = Vec::new();
    let mut moved_ids = HashSet::new();
    let target_gap = 0.055;

    for (note_id, note) in &box_map {
        if !is_context_note_box(note_id, note) {
            continue;
        }
        let Some((_loss_id, loss)) = box_map
            .iter()
            .filter(|(id, info)| {
                *id != note_id
                    && is_loss_or_objective_box(id, info)
                    && horizontal_overlap(note.bbox, info.bbox) > 0.02
                    && vertical_separation(note.bbox, info.bbox) < target_gap
            })
            .min_by(|left, right| {
                vertical_separation(note.bbox, left.1.bbox)
                    .total_cmp(&vertical_separation(note.bbox, right.1.bbox))
            })
        else {
            continue;
        };
        let candidates = context_note_clearance_candidates(note.bbox, loss.bbox, target_gap);
        if let Some(candidate) = candidates.into_iter().find(|candidate| {
            route_box_candidate_is_clear(note_id, *candidate, &box_map)
                && (horizontal_overlap(*candidate, loss.bbox) <= 0.01
                    || vertical_separation(*candidate, loss.bbox) >= target_gap)
        }) {
            updates.push((note_id.clone(), candidate));
        }
    }

    for (id, bbox) in updates {
        set_box_bbox(plan, &id, bbox);
        moved_ids.insert(id);
    }
    if !moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    }
}

fn context_note_clearance_candidates(
    note_bbox: [f64; 4],
    loss_bbox: [f64; 4],
    target_gap: f64,
) -> Vec<[f64; 4]> {
    let note_bbox = normalize_box(note_bbox);
    let loss_bbox = normalize_box(loss_bbox);
    let width = box_width(note_bbox);
    let height = box_height(note_bbox);
    let mut candidates = Vec::new();

    if center_y(note_bbox) <= center_y(loss_bbox) {
        let y2 = loss_bbox[1] - target_gap;
        candidates.push([note_bbox[0], y2 - height, note_bbox[2], y2]);
    } else {
        let y1 = loss_bbox[3] + target_gap;
        candidates.push([note_bbox[0], y1, note_bbox[2], y1 + height]);
    }

    let left_x2 = loss_bbox[0] - 0.035;
    candidates.push([
        left_x2 - width,
        note_bbox[1],
        left_x2,
        note_bbox[1] + height,
    ]);
    let right_x1 = loss_bbox[2] + 0.035;
    candidates.push([
        right_x1,
        note_bbox[1],
        right_x1 + width,
        note_bbox[1] + height,
    ]);

    candidates
        .into_iter()
        .filter(|bbox| bbox[0] >= 0.02 && bbox[1] >= 0.02 && bbox[2] <= 0.98 && bbox[3] <= 0.98)
        .map(normalize_box)
        .collect()
}

#[derive(Clone, Debug)]
struct BoxRouteInfo {
    bbox: [f64; 4],
    text: String,
    role: String,
    style: String,
}

fn align_output_boxes_with_sources(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut moved_ids = HashSet::new();
    let mut moves = Vec::new();
    for object in &plan.objects {
        let DrawObject::Connector { from, to, .. } = object else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(source), Some(target)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        if !is_output_route_box(to_id, target) || is_input_route_box(from_id, source) {
            continue;
        }
        let horizontal_gap = if center_x(source.bbox) <= center_x(target.bbox) {
            target.bbox[0] - source.bbox[2]
        } else {
            source.bbox[0] - target.bbox[2]
        };
        if horizontal_gap < 0.08 || (center_y(source.bbox) - center_y(target.bbox)).abs() < 0.06 {
            continue;
        }
        let height = box_height(target.bbox);
        let y1 = (center_y(source.bbox) - height / 2.0).clamp(0.06, 0.94 - height);
        let candidate = [target.bbox[0], y1, target.bbox[2], y1 + height];
        if route_box_candidate_is_clear(to_id, candidate, &box_map) {
            moves.push((to_id.to_string(), candidate));
        }
    }
    for (id, bbox) in moves {
        set_box_bbox(plan, &id, bbox);
        moved_ids.insert(id);
    }
    if !moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    }
}

fn compact_tall_output_boxes_for_short_labels(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut updates = Vec::new();
    let mut moved_ids = HashSet::new();

    for (id, output) in &box_map {
        if !is_output_route_box(id, output) || is_task_loss_route_box(id, output) {
            continue;
        }
        let visible_chars = visible_text_len(&output.text);
        let line_count = output
            .text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count();
        if visible_chars > 24 || line_count > 3 {
            continue;
        }
        let target_width = compact_output_target_width(visible_chars);
        let target_height = compact_output_target_height(line_count);
        let oversized = box_width(output.bbox) > target_width * 1.25
            || box_height(output.bbox) > target_height * 1.2;
        if !oversized {
            continue;
        }
        let terminal = incoming_connector_terminal_point(plan, id);
        let target_center_y = terminal
            .map(|point| point[1])
            .or_else(|| incoming_connector_terminal_y(plan, id))
            .unwrap_or_else(|| center_y(output.bbox))
            .clamp(0.02 + target_height / 2.0, 0.98 - target_height / 2.0);
        let (x1, x2) = compact_output_x_span(output.bbox, target_width, terminal.map(|p| p[0]));
        let candidate = [
            x1,
            target_center_y - target_height / 2.0,
            x2,
            target_center_y + target_height / 2.0,
        ];
        if route_box_candidate_is_clear(id, candidate, &box_map) {
            updates.push((id.clone(), candidate));
        }
    }

    for (id, bbox) in updates {
        set_box_bbox(plan, &id, bbox);
        moved_ids.insert(id);
    }
    if !moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    }
}

fn compact_output_target_width(visible_chars: usize) -> f64 {
    (0.08 + visible_chars as f64 * 0.008).clamp(0.10, 0.22)
}

fn compact_output_target_height(line_count: usize) -> f64 {
    match line_count {
        0 | 1 => 0.10,
        2 => 0.13,
        _ => 0.15,
    }
}

fn compact_output_x_span(bbox: [f64; 4], target_width: f64, terminal_x: Option<f64>) -> (f64, f64) {
    if box_width(bbox) <= target_width * 1.1 {
        return (bbox[0], bbox[2]);
    }
    if let Some(x) = terminal_x {
        if x <= center_x(bbox) {
            let x1 = x.clamp(0.02, 0.98 - target_width);
            return (x1, x1 + target_width);
        }
        let x2 = x.clamp(0.02 + target_width, 0.98);
        return (x2 - target_width, x2);
    }
    let center = center_x(bbox).clamp(0.02 + target_width / 2.0, 0.98 - target_width / 2.0);
    (center - target_width / 2.0, center + target_width / 2.0)
}

fn incoming_connector_terminal_point(plan: &DrawPlan, target_id: &str) -> Option<[f64; 2]> {
    plan.objects.iter().find_map(|object| {
        let DrawObject::Connector { points, to, .. } = object else {
            return None;
        };
        if to.as_deref() != Some(target_id) {
            return None;
        }
        points.last().copied()
    })
}

fn incoming_connector_terminal_y(plan: &DrawPlan, target_id: &str) -> Option<f64> {
    plan.objects.iter().find_map(|object| {
        let DrawObject::Connector { points, to, .. } = object else {
            return None;
        };
        if to.as_deref() != Some(target_id) {
            return None;
        }
        points.last().map(|point| point[1])
    })
}

fn move_student_inference_notes_near_student(plan: &mut DrawPlan) {
    let excluded_note_ids = HashSet::new();
    move_student_inference_notes_near_student_except(plan, &excluded_note_ids);
}

fn move_student_inference_notes_near_student_except(
    plan: &mut DrawPlan,
    excluded_note_ids: &HashSet<String>,
) {
    let box_map = current_box_route_info_map(plan);
    let Some((student_id, student)) = student_anchor_for_inference_notes(plan) else {
        return;
    };

    let mut updates = Vec::new();
    for object in &plan.objects {
        match object {
            DrawObject::Box {
                id,
                bbox,
                text,
                role,
                style,
                ..
            } if id.as_str() != student_id.as_str()
                && !excluded_note_ids.contains(id.as_str())
                && is_student_inference_note_like(id, text, role, style) =>
            {
                if let Some(candidate) =
                    student_inference_note_candidate(id, *bbox, text, student.bbox, &box_map, false)
                {
                    updates.push((id.clone(), candidate));
                }
            }
            DrawObject::Text {
                id,
                bbox,
                text,
                style,
                ..
            } if !excluded_note_ids.contains(id.as_str())
                && is_student_inference_note_like(id, text, "", style) =>
            {
                if let Some(candidate) =
                    student_inference_note_candidate(id, *bbox, text, student.bbox, &box_map, true)
                {
                    updates.push((id.clone(), candidate));
                }
            }
            _ => {}
        }
    }

    for (id, bbox) in updates {
        set_object_bbox(plan, &id, bbox);
    }
}

fn target_bound_inference_annotation_ids(figure_plan: &FigurePlan) -> HashSet<String> {
    figure_plan
        .annotations
        .iter()
        .filter(|annotation| {
            annotation.target_id.is_some()
                && annotation_label_is_inference_specific(&annotation.label)
        })
        .map(|annotation| annotation.id.clone())
        .collect()
}

fn student_anchor_for_inference_notes(plan: &DrawPlan) -> Option<(String, BoxRouteInfo)> {
    plan.objects.iter().find_map(|object| {
        let DrawObject::Box {
            id,
            bbox,
            text,
            role,
            style,
            ..
        } = object
        else {
            return None;
        };
        let info = BoxRouteInfo {
            bbox: normalize_box(*bbox),
            text: text.clone(),
            role: role.clone(),
            style: style.clone(),
        };
        is_real_student_anchor_for_inference_notes(id, &info).then(|| (id.clone(), info))
    })
}

fn is_real_student_anchor_for_inference_notes(id: &str, info: &BoxRouteInfo) -> bool {
    let text = route_box_text(id, info);
    let student_like = text.contains("student") || text.contains("compact");
    let actual_module =
        info.role.to_lowercase().contains("main") || info.style.to_lowercase().contains("primary");
    let note_like = text.contains("inference")
        || text.contains("note")
        || text.contains("badge")
        || text.contains("annotation")
        || text.contains("muted")
        || text.contains("context")
        || text.contains("only student")
        || text.contains("student only");
    student_like
        && actual_module
        && !note_like
        && !is_loss_or_objective_box(id, info)
        && !is_output_route_box(id, info)
        && !is_input_route_box(id, info)
}

fn is_student_inference_note_like(id: &str, text: &str, role: &str, style: &str) -> bool {
    let identity = format!(
        "{} {} {} {}",
        id.to_lowercase(),
        text.to_lowercase(),
        role.to_lowercase(),
        style.to_lowercase()
    );
    let note_like = identity.contains("note")
        || identity.contains("annotation")
        || identity.contains("muted")
        || identity.contains("context")
        || text.to_lowercase().contains("inference:");
    identity.contains("inference")
        && identity.contains("student")
        && note_like
        && !identity.contains("teacher")
        && !identity.contains("loss")
        && !identity.contains("output")
}

fn ensure_inference_note_boxes_readable(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let connected_ids = connector_endpoint_ids(plan);
    let mut updates = Vec::new();

    for object in &plan.objects {
        let DrawObject::Box {
            id,
            bbox,
            text,
            role,
            style,
            ..
        } = object
        else {
            continue;
        };
        if !is_inference_note_readability_guard_target(id, text, role, style) {
            continue;
        }
        if connected_ids.contains(id.as_str()) {
            continue;
        }
        let id_lower = id.to_lowercase();
        let current = normalize_box(*bbox);
        let max_width = if id_lower.contains("inference_note") {
            0.16
        } else {
            0.22
        };
        let target_width = box_width(current).clamp(0.13, max_width);
        let target_height = if id_lower.contains("inference_note") {
            box_height(current).clamp(0.080, 0.090)
        } else {
            box_height(current).clamp(0.085, 0.12)
        };
        if (box_width(current) - target_width).abs() < 0.001
            && (box_height(current) - target_height).abs() < 0.001
        {
            continue;
        }
        let center_x = center_x(current);
        let center_y = center_y(current);
        let candidates = [
            box_from_top_left_inside(
                center_x - target_width / 2.0,
                center_y - target_height / 2.0,
                target_width,
                target_height,
            ),
            box_from_top_left_inside(current[0], current[1], target_width, target_height),
            box_from_top_left_inside(
                current[2] - target_width,
                current[3] - target_height,
                target_width,
                target_height,
            ),
        ];
        let Some(candidate) = candidates
            .into_iter()
            .find(|candidate| route_box_candidate_is_clear(id, *candidate, &box_map))
        else {
            continue;
        };
        updates.push((id.clone(), candidate));
    }

    for (id, bbox) in updates {
        set_box_bbox(plan, &id, bbox);
    }
}

fn move_unanchored_inference_note_boxes_to_student_periphery(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let anchors = box_map
        .iter()
        .filter_map(|(id, info)| {
            is_student_or_output_inference_anchor(id, info).then_some((id.clone(), info.bbox))
        })
        .collect::<Vec<_>>();
    if anchors.is_empty() {
        return;
    }
    let branch_pairs = teacher_student_branch_pairs(&box_map);
    let connected_ids = connector_endpoint_ids(plan);
    let segment_boxes = connector_segment_boxes(plan, 0.014);
    let text_boxes = plan
        .objects
        .iter()
        .filter_map(|object| match object {
            DrawObject::Text { bbox, .. } => Some(*bbox),
            _ => None,
        })
        .collect::<Vec<_>>();

    let updates = plan
        .objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Box {
                id,
                bbox,
                text,
                role,
                style,
                ..
            } = object
            else {
                return None;
            };
            if connected_ids.contains(id.as_str())
                || !is_standalone_inference_note_box(id, text, role, style)
            {
                return None;
            }
            let current = normalize_box(*bbox);
            let anchored =
                compact_inference_note_is_anchored_to_student_or_output(current, &box_map);
            let in_corridor = inference_note_is_in_teacher_student_corridor(current, &branch_pairs);
            let conflicts = note_component_conflicts_with_connector_segments(plan, id, current);
            let needs_move = !anchored || (in_corridor && conflicts);
            if !needs_move {
                return None;
            }

            let ignored_ids = HashSet::from([id.clone()]);
            let obstacle_boxes = plan
                .objects
                .iter()
                .filter_map(|candidate_object| match candidate_object {
                    DrawObject::Box {
                        id: obstacle_id,
                        bbox,
                        ..
                    }
                    | DrawObject::Image {
                        id: obstacle_id,
                        bbox,
                        ..
                    } if obstacle_id != id => Some(*bbox),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let candidate = anchors
                .iter()
                .flat_map(|(_, anchor_bbox)| {
                    student_inference_note_candidates(current, text, *anchor_bbox)
                })
                .filter(|candidate| {
                    !inference_note_is_in_teacher_student_corridor(*candidate, &branch_pairs)
                        && anchors.iter().any(|(_, anchor)| {
                            candidate_is_near_inference_anchor(*candidate, &[*anchor])
                        })
                        && connector_label_candidate_clear(
                            *candidate,
                            &obstacle_boxes,
                            &segment_boxes,
                            &text_boxes,
                        )
                        && route_box_candidate_is_clear_except(
                            id,
                            *candidate,
                            &box_map,
                            &ignored_ids,
                        )
                })
                .min_by(|left, right| {
                    inference_note_anchor_candidate_score(*left, current, anchors.as_slice())
                        .total_cmp(&inference_note_anchor_candidate_score(
                            *right,
                            current,
                            anchors.as_slice(),
                        ))
                })?;
            Some((id.clone(), candidate))
        })
        .collect::<Vec<_>>();

    for (id, bbox) in updates {
        set_box_bbox(plan, &id, bbox);
    }
}

fn inference_note_anchor_candidate_score(
    candidate: [f64; 4],
    current: [f64; 4],
    anchors: &[(String, [f64; 4])],
) -> f64 {
    let candidate_center = center(candidate);
    let anchor_distance = anchors
        .iter()
        .map(|(_, anchor)| center_distance(candidate_center, center(*anchor)))
        .fold(f64::INFINITY, f64::min);
    let movement = center_distance(candidate_center, center(current));
    anchor_distance + movement * 0.15
}

fn convert_peripheral_inference_note_boxes_to_annotations(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let branch_pairs = teacher_student_branch_pairs(&box_map);
    let connected_ids = connector_endpoint_ids(plan);
    let convert_ids = plan
        .objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Box {
                id,
                bbox,
                text,
                role,
                style,
                ..
            } = object
            else {
                return None;
            };
            let bbox = normalize_box(*bbox);
            let compact = box_area(bbox) <= 0.018 && box_height(bbox) <= 0.085;
            let anchored_below_student_or_output =
                compact_inference_note_is_below_student_or_output(bbox, &box_map);
            let bottom_peripheral =
                id == "inference_note" && (bbox[1] >= 0.80 || anchored_below_student_or_output);
            let peripheral = !inference_note_is_in_teacher_student_corridor(bbox, &branch_pairs);
            let anchored = compact_inference_note_is_anchored_to_student_or_output(bbox, &box_map);
            (compact
                && bottom_peripheral
                && peripheral
                && anchored
                && !connected_ids.contains(id.as_str())
                && is_standalone_inference_note_box(id, text, role, style))
            .then(|| id.clone())
        })
        .collect::<HashSet<_>>();
    if convert_ids.is_empty() {
        return;
    }

    for object in &mut plan.objects {
        let DrawObject::Box {
            id, bbox, text, z, ..
        } = object
        else {
            continue;
        };
        if !convert_ids.contains(id) {
            continue;
        }
        *object = DrawObject::Text {
            id: id.clone(),
            bbox: *bbox,
            text: text.clone(),
            style: "annotation".to_string(),
            z: *z,
        };
    }
}

fn compact_inference_note_is_below_student_or_output(
    note_bbox: [f64; 4],
    box_map: &HashMap<String, BoxRouteInfo>,
) -> bool {
    note_bbox[1] >= 0.70
        && box_map.iter().any(|(id, info)| {
            is_student_or_output_inference_anchor(id, info)
                && note_bbox[1] >= info.bbox[3] + 0.035
                && horizontal_separation(note_bbox, info.bbox) <= 0.16
        })
}

fn is_inference_note_readability_guard_target(
    id: &str,
    text: &str,
    role: &str,
    style: &str,
) -> bool {
    let identity = format!(
        "{} {} {} {}",
        id.to_lowercase(),
        text.to_lowercase(),
        role.to_lowercase(),
        style.to_lowercase()
    );
    (id == "inference_note"
        || id.to_lowercase().contains("inference_note")
        || identity.contains("standalone_inference_note")
        || identity.contains("inference note"))
        && identity.contains("student")
        && !identity.contains("teacher")
        && !identity.contains("loss")
}

fn student_inference_note_candidate(
    id: &str,
    current: [f64; 4],
    text: &str,
    student_bbox: [f64; 4],
    box_map: &HashMap<String, BoxRouteInfo>,
    allow_loss_objective_crowding: bool,
) -> Option<[f64; 4]> {
    if !allow_loss_objective_crowding
        && inference_note_crowds_loss_or_objective_box_map(current, box_map)
    {
        return None;
    }
    if !student_inference_note_needs_move(current, student_bbox) {
        return None;
    }
    student_inference_note_candidates(current, text, student_bbox)
        .into_iter()
        .find(|candidate| route_box_candidate_is_clear(id, *candidate, box_map))
}

fn student_inference_note_needs_move(current: [f64; 4], student_bbox: [f64; 4]) -> bool {
    component_overlap_gate_fails(current, student_bbox)
        || box_height(current) > 0.095
        || box_area(current) > 0.022
        || center_y(current) + 0.08 < center_y(student_bbox)
        || (vertical_separation(current, student_bbox) > 0.10
            && horizontal_separation(current, student_bbox) > 0.08)
}

fn student_inference_note_candidates(
    current: [f64; 4],
    text: &str,
    student_bbox: [f64; 4],
) -> Vec<[f64; 4]> {
    let width = (0.075 + visible_text_len(text) as f64 * 0.0055).clamp(0.16, 0.18);
    let height: f64 = 0.080;
    let centered_x = (center_x(student_bbox) - width / 2.0).clamp(0.02, 0.98 - width);
    let current_x = current[0].clamp(0.02, 0.98 - width);
    let below_y = student_bbox[3] + 0.025;
    let above_y = student_bbox[1] - height - 0.025;
    let right_x = student_bbox[2] + 0.035;
    let side_y = (center_y(student_bbox) - height / 2.0).clamp(0.02, 0.98 - height);

    let mut candidates = Vec::new();
    if right_x + width <= 0.98 {
        candidates.push([right_x, side_y, right_x + width, side_y + height]);
    }
    if below_y + height <= 0.98 {
        candidates.push([centered_x, below_y, centered_x + width, below_y + height]);
        candidates.push([current_x, below_y, current_x + width, below_y + height]);
    }
    if above_y >= 0.02 {
        candidates.push([centered_x, above_y, centered_x + width, above_y + height]);
    }
    let left_x = student_bbox[0] - 0.035 - width;
    if left_x >= 0.02 {
        candidates.push([left_x, side_y, left_x + width, side_y + height]);
    }
    candidates.into_iter().map(normalize_box).collect()
}

fn inference_note_crowds_loss_or_objective_box_map(
    note_bbox: [f64; 4],
    box_map: &HashMap<String, BoxRouteInfo>,
) -> bool {
    let note_bbox = normalize_box(note_bbox);
    box_map.iter().any(|(id, info)| {
        if !is_loss_or_objective_box(id, info) {
            return false;
        }
        let (_, vertical_overlap) = intersection_dimensions(note_bbox, info.bbox);
        vertical_overlap > 0.025 && horizontal_separation(note_bbox, info.bbox) <= 0.20
    })
}

fn align_outer_margin_outputs_with_sources(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut moved_ids = HashSet::new();
    let mut moves = Vec::new();

    for object in &plan.objects {
        let DrawObject::Connector { from, to, .. } = object else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(source), Some(output)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        if !is_main_route_box(source) || !is_output_route_box(to_id, output) {
            continue;
        }
        if !output_needs_outer_margin_alignment(source.bbox, output.bbox) {
            continue;
        }
        let height = box_height(output.bbox);
        let y1 = (center_y(source.bbox) - height / 2.0).clamp(0.06, 0.94 - height);
        let (x1, x2) = output_x_span_with_min_gap(source.bbox, output.bbox);
        let candidate = [x1, y1, x2, y1 + height];
        if route_box_candidate_is_clear(to_id, candidate, &box_map) {
            moves.push((to_id.to_string(), candidate));
        }
    }

    for (id, bbox) in moves {
        set_box_bbox(plan, &id, bbox);
        moved_ids.insert(id);
    }
    if !moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    }
}

fn output_needs_outer_margin_alignment(source: [f64; 4], output: [f64; 4]) -> bool {
    if center_x(output) <= center_x(source) {
        return false;
    }
    vertical_separation(source, output) >= 0.22
        && (horizontal_separation(source, output) <= 0.10 || output[1] > 0.80 || output[3] < 0.20)
}

fn output_x_span_with_min_gap(source: [f64; 4], output: [f64; 4]) -> (f64, f64) {
    let min_gap = 0.045;
    let min_width = 0.075;
    let width = box_width(output);
    if center_x(source) <= center_x(output) {
        let desired_x1 = source[2] + min_gap;
        if desired_x1 + width <= 0.98 {
            return (desired_x1, desired_x1 + width);
        }
        if output[2] - desired_x1 >= min_width {
            return (desired_x1, output[2]);
        }
    } else {
        let desired_x2 = source[0] - min_gap;
        if desired_x2 - width >= 0.02 {
            return (desired_x2 - width, desired_x2);
        }
        if desired_x2 - output[0] >= min_width {
            return (output[0], desired_x2);
        }
    }
    (output[0], output[2])
}

fn straighten_adjacent_main_output_connectors(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    for object in &mut plan.objects {
        let DrawObject::Connector {
            points, from, to, ..
        } = object
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(source), Some(output)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        if !is_main_route_box(source) || !is_output_route_box(to_id, output) {
            continue;
        }
        if vertical_separation(source.bbox, output.bbox) > 0.02
            || horizontal_separation(source.bbox, output.bbox) < 0.035
            || horizontal_separation(source.bbox, output.bbox) > 0.18
        {
            continue;
        }
        if let Some(y) = common_horizontal_y(source.bbox, output.bbox) {
            *points = horizontal_connector_points(source.bbox, output.bbox, y);
        }
    }
}

fn move_student_only_inference_outputs_next_to_sources(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut moves = Vec::new();
    let mut moved_ids = HashSet::new();

    for object in &plan.objects {
        let DrawObject::Connector { from, to, .. } = object else {
            continue;
        };
        let (Some(source_id), Some(output_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(source), Some(output)) = (box_map.get(source_id), box_map.get(output_id)) else {
            continue;
        };
        if !is_main_route_box(source) || !is_student_only_inference_route_box(output_id, output) {
            continue;
        }
        let already_adjacent = vertical_separation(source.bbox, output.bbox) <= 0.02
            && (0.03..=0.28).contains(&horizontal_separation(source.bbox, output.bbox));
        if already_adjacent {
            continue;
        }
        if let Some(candidate) = student_only_inference_side_candidates(source.bbox, output.bbox)
            .into_iter()
            .find(|candidate| route_box_candidate_is_clear(output_id, *candidate, &box_map))
        {
            moves.push((output_id.to_string(), candidate));
        }
    }

    for (id, bbox) in moves {
        set_box_bbox(plan, &id, bbox);
        moved_ids.insert(id);
    }
    if !moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    }
}

fn is_student_only_inference_route_box(id: &str, info: &BoxRouteInfo) -> bool {
    let text = route_box_text(id, info);
    text.contains("inference")
        && text.contains("student")
        && !text.contains("note")
        && !text.contains("muted")
        && !text.contains("annotation")
        && !text.contains("loss")
        && !text.contains("objective")
        && !text.contains("input")
}

fn student_only_inference_side_candidates(source: [f64; 4], output: [f64; 4]) -> Vec<[f64; 4]> {
    let width = box_width(output).clamp(0.16, 0.24);
    // Student-only inference badges should read as a compact side note, not a second lane.
    let height = box_height(output).clamp(0.10, 0.12);
    let y1 = (center_y(source) - height / 2.0).clamp(0.04, 0.96 - height);
    let mut candidates = Vec::new();
    let right_x = source[2] + 0.045;
    if right_x + width <= 0.98 {
        candidates.push([right_x, y1, right_x + width, y1 + height]);
    }
    let left_x = source[0] - 0.045 - width;
    if left_x >= 0.02 {
        candidates.push([left_x, y1, left_x + width, y1 + height]);
    }
    let original_x = output[0].clamp(0.02, 0.98 - width);
    candidates.push([original_x, y1, original_x + width, y1 + height]);
    candidates.into_iter().map(normalize_box).collect()
}

fn align_shared_input_boxes_with_branch_targets(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut targets_by_input: HashMap<String, Vec<BoxRouteInfo>> = HashMap::new();
    for object in &plan.objects {
        let DrawObject::Connector { from, to, .. } = object else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(input), Some(target)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        if is_input_route_box(from_id, input) {
            targets_by_input
                .entry(from_id.to_string())
                .or_default()
                .push(target.clone());
        }
    }

    let mut moved_ids = HashSet::new();
    for (input_id, targets) in targets_by_input {
        if targets.len() < 2 {
            continue;
        }
        let Some(input) = box_map.get(input_id.as_str()) else {
            continue;
        };
        let has_main = targets.iter().any(is_student_or_main_route_box);
        let has_context = targets
            .iter()
            .any(|target| is_teacher_or_context_route_box("", target));
        if !has_main || !has_context {
            continue;
        }
        let target_min = targets
            .iter()
            .map(|target| center_y(target.bbox))
            .fold(f64::INFINITY, f64::min);
        let target_max = targets
            .iter()
            .map(|target| center_y(target.bbox))
            .fold(f64::NEG_INFINITY, f64::max);
        let current_y = center_y(input.bbox);
        let height = box_height(input.bbox);
        let desired_y =
            ((target_min + target_max) / 2.0).clamp(0.06 + height / 2.0, 0.94 - height / 2.0);
        if (current_y - desired_y).abs() < 0.08 {
            continue;
        }
        let y1 = desired_y - height / 2.0;
        let candidate = [input.bbox[0], y1, input.bbox[2], y1 + height];
        if route_box_candidate_is_clear(input_id.as_str(), candidate, &box_map) {
            set_box_bbox(plan, input_id.as_str(), candidate);
            moved_ids.insert(input_id);
        }
    }

    if !moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    }
}

fn move_shared_inputs_off_outer_margins(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut targets_by_input: HashMap<String, Vec<BoxRouteInfo>> = HashMap::new();
    for object in &plan.objects {
        let DrawObject::Connector { from, to, .. } = object else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(input), Some(target)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        if is_input_route_box(from_id, input) {
            targets_by_input
                .entry(from_id.to_string())
                .or_default()
                .push(target.clone());
        }
    }

    let mut moved_ids = HashSet::new();
    for (input_id, targets) in targets_by_input {
        if targets.len() < 2 {
            continue;
        }
        let Some(input) = box_map.get(input_id.as_str()) else {
            continue;
        };
        if input.bbox[1] <= 0.78 && input.bbox[3] >= 0.22 {
            continue;
        }
        let has_main = targets.iter().any(is_student_or_main_route_box);
        let has_context = targets
            .iter()
            .any(|target| is_teacher_or_context_route_box("", target));
        if !has_main || !has_context {
            continue;
        }
        let Some(candidate) = shared_input_outer_margin_candidates(input.bbox, &targets)
            .into_iter()
            .find(|candidate| {
                route_box_candidate_is_clear(input_id.as_str(), *candidate, &box_map)
            })
        else {
            continue;
        };
        set_box_bbox(plan, input_id.as_str(), candidate);
        moved_ids.insert(input_id);
    }

    if !moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    }
}

fn shared_input_outer_margin_candidates(
    input_bbox: [f64; 4],
    targets: &[BoxRouteInfo],
) -> Vec<[f64; 4]> {
    let target_union = targets
        .iter()
        .map(|target| target.bbox)
        .reduce(union_box)
        .unwrap_or(input_bbox);
    let input_bbox = normalize_box(input_bbox);
    let height = box_height(input_bbox);
    let gap = 0.035;
    let mut candidates = Vec::new();

    if center_y(input_bbox) >= center_y(target_union) {
        let y1 = target_union[3] + gap;
        candidates.push([input_bbox[0], y1, input_bbox[2], y1 + height]);
        let y2 = target_union[1] - gap;
        candidates.push([input_bbox[0], y2 - height, input_bbox[2], y2]);
    } else {
        let y2 = target_union[1] - gap;
        candidates.push([input_bbox[0], y2 - height, input_bbox[2], y2]);
        let y1 = target_union[3] + gap;
        candidates.push([input_bbox[0], y1, input_bbox[2], y1 + height]);
    }

    candidates
        .into_iter()
        .filter(|bbox| bbox[0] >= 0.02 && bbox[1] >= 0.02 && bbox[2] <= 0.98 && bbox[3] <= 0.98)
        .map(normalize_box)
        .collect()
}

fn stabilize_teacher_student_shared_inputs(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut targets_by_input: HashMap<String, Vec<(String, BoxRouteInfo)>> = HashMap::new();
    for object in &plan.objects {
        let DrawObject::Connector { from, to, .. } = object else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(input), Some(target)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        if is_input_route_box(from_id, input) {
            targets_by_input
                .entry(from_id.to_string())
                .or_default()
                .push((to_id.to_string(), target.clone()));
        }
    }

    let mut moved_ids = HashSet::new();
    for (input_id, targets) in targets_by_input {
        if targets.len() < 2 {
            continue;
        }
        let Some(input) = box_map.get(input_id.as_str()) else {
            continue;
        };
        if !shared_input_targets_teacher_and_student(targets.as_slice()) {
            continue;
        }
        if !shared_teacher_student_input_needs_topology_repair(input.bbox, targets.as_slice()) {
            continue;
        }
        let Some(candidate) =
            shared_teacher_student_input_candidate(&input_id, input, targets.as_slice(), &box_map)
        else {
            continue;
        };
        set_box_bbox(plan, input_id.as_str(), candidate);
        moved_ids.insert(input_id);
    }

    if moved_ids.is_empty() {
        return;
    }
    realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    reroute_shared_input_branch_connectors(plan, &moved_ids);
}

fn shared_input_targets_teacher_and_student(targets: &[(String, BoxRouteInfo)]) -> bool {
    let has_student = targets
        .iter()
        .any(|(_, target)| is_student_or_main_route_box(target));
    let has_teacher = targets
        .iter()
        .any(|(target_id, target)| is_teacher_or_context_route_box(target_id, target));
    has_student && has_teacher
}

fn shared_teacher_student_input_needs_topology_repair(
    input: [f64; 4],
    targets: &[(String, BoxRouteInfo)],
) -> bool {
    let target_union = targets
        .iter()
        .map(|(_, target)| target.bbox)
        .reduce(union_box)
        .unwrap_or(input);
    let target_min_y = targets
        .iter()
        .map(|(_, target)| center_y(target.bbox))
        .fold(f64::INFINITY, f64::min);
    let target_max_y = targets
        .iter()
        .map(|(_, target)| center_y(target.bbox))
        .fold(f64::NEG_INFINITY, f64::max);
    let desired_y = (target_min_y + target_max_y) / 2.0;
    center_y(input) > target_union[3] + 0.12
        || center_y(input) + 0.12 < target_union[1]
        || (center_y(input) - desired_y).abs() > 0.16
        || input[2] > target_union[0] - 0.015
        || shared_input_crowds_branch_target(input, targets)
}

fn shared_input_crowds_branch_target(input: [f64; 4], targets: &[(String, BoxRouteInfo)]) -> bool {
    targets.iter().any(|(_, target)| {
        let horizontal_gap = horizontal_separation(input, target.bbox);
        horizontal_gap > 0.0
            && horizontal_gap < 0.060
            && axis_overlap_ratio(input[1], input[3], target.bbox[1], target.bbox[3]) > 0.25
    })
}

fn shared_teacher_student_input_candidate(
    input_id: &str,
    input: &BoxRouteInfo,
    targets: &[(String, BoxRouteInfo)],
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<[f64; 4]> {
    let target_union = targets
        .iter()
        .map(|(_, target)| target.bbox)
        .reduce(union_box)
        .unwrap_or(input.bbox);
    let target_min_y = targets
        .iter()
        .map(|(_, target)| center_y(target.bbox))
        .fold(f64::INFINITY, f64::min);
    let target_max_y = targets
        .iter()
        .map(|(_, target)| center_y(target.bbox))
        .fold(f64::NEG_INFINITY, f64::max);
    let desired_center_y = (target_min_y + target_max_y) / 2.0;
    let height = box_height(input.bbox).clamp(0.10, 0.15);
    let preferred_width = readable_shared_input_width(input);
    let available_left_width = (target_union[0] - 0.055 - 0.02).max(0.0);
    let width = preferred_width
        .min(available_left_width.max(0.10))
        .clamp(0.09, 0.16);
    let y1 = (desired_center_y - height / 2.0).clamp(0.02, 0.98 - height);
    let left_x = (target_union[0] - 0.060 - width).clamp(0.02, 0.98 - width);
    let current_x = input.bbox[0].clamp(0.02, 0.98 - width);
    let mut candidates = Vec::new();
    if let Some(candidate) = shared_input_middle_slot_candidate(
        input_id,
        input.bbox,
        targets,
        box_map,
        width,
        height,
        desired_center_y,
    ) {
        candidates.push((candidate, true));
    }
    candidates.extend([
        ([left_x, y1, left_x + width, y1 + height], false),
        ([0.02, y1, 0.02 + width, y1 + height], false),
        ([current_x, y1, current_x + width, y1 + height], false),
    ]);
    candidates
        .into_iter()
        .map(|(candidate, require_gutter)| (normalize_box(candidate), require_gutter))
        .find_map(|(candidate, require_gutter)| {
            let clear = if require_gutter {
                shared_input_candidate_is_clear(input_id, candidate, box_map)
            } else {
                route_box_candidate_is_clear(input_id, candidate, box_map)
            };
            clear.then_some(candidate)
        })
}

fn shared_input_middle_slot_candidate(
    input_id: &str,
    input_bbox: [f64; 4],
    targets: &[(String, BoxRouteInfo)],
    box_map: &HashMap<String, BoxRouteInfo>,
    width: f64,
    height: f64,
    desired_center_y: f64,
) -> Option<[f64; 4]> {
    let left_target = targets
        .iter()
        .min_by(|left, right| center_x(left.1.bbox).total_cmp(&center_x(right.1.bbox)))?;
    let right_target = targets
        .iter()
        .max_by(|left, right| center_x(left.1.bbox).total_cmp(&center_x(right.1.bbox)))?;
    if input_bbox[0] < left_target.1.bbox[2] - 0.010 {
        return None;
    }
    if right_target.1.bbox[0] <= left_target.1.bbox[2] + width + 0.090 {
        return None;
    }

    let x1 = (left_target.1.bbox[2] + 0.055)
        .min(right_target.1.bbox[0] - 0.055 - width)
        .max(input_bbox[0])
        .clamp(0.02, 0.98 - width);
    let mut y1 = (desired_center_y - height / 2.0).clamp(0.02, 0.98 - height);
    let mut candidate = box_from_top_left_inside(x1, y1, width, height);
    for (id, info) in box_map {
        if id == input_id
            || targets
                .iter()
                .any(|(target_id, _)| target_id.as_str() == id.as_str())
            || !is_residual_or_supervision_hub_box(id, info)
            || horizontal_overlap(candidate, info.bbox) <= 0.02
            || vertical_separation(candidate, info.bbox) >= 0.060
        {
            continue;
        }
        if center_y(candidate) <= center_y(info.bbox) {
            y1 = y1.min(info.bbox[1] - 0.060 - height);
        } else {
            y1 = y1.max(info.bbox[3] + 0.060);
        }
        candidate = box_from_top_left_inside(x1, y1, width, height);
    }
    Some(candidate)
}

fn shared_input_candidate_is_clear(
    input_id: &str,
    candidate: [f64; 4],
    box_map: &HashMap<String, BoxRouteInfo>,
) -> bool {
    box_map.iter().all(|(id, info)| {
        id == input_id
            || (!component_overlap_gate_fails(candidate, info.bbox)
                && !component_crowding_gate_fails_normalized(candidate, info.bbox))
    })
}

fn readable_shared_input_width(input: &BoxRouteInfo) -> f64 {
    let visible_chars = visible_text_len(&input.text) as f64;
    (0.060 + visible_chars * 0.010).clamp(0.10, 0.16)
}

fn reroute_shared_input_branch_connectors(plan: &mut DrawPlan, moved_input_ids: &HashSet<String>) {
    let box_map = current_box_route_info_map(plan);
    let mut updates = Vec::new();
    for object in &plan.objects {
        let DrawObject::Connector { id, from, to, .. } = object else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        if !moved_input_ids.contains(from_id) {
            continue;
        }
        let (Some(input), Some(target)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        if !is_student_or_main_route_box(target) && !is_teacher_or_context_route_box(to_id, target)
        {
            continue;
        }
        let candidate =
            compact_input_context_route(id, from_id, input, to_id, target, plan, &box_map)
                .unwrap_or_else(|| input_branch_connector_points(input.bbox, target.bbox));
        if !connector_points_intersect_intermediate_boxes(
            candidate.as_slice(),
            from_id,
            to_id,
            &box_map,
        ) {
            updates.push((id.clone(), candidate));
        }
    }

    for (connector_id, candidate) in updates {
        if let Some(DrawObject::Connector { points, .. }) = plan
            .objects
            .iter_mut()
            .find(|object| draw_object_id(object) == connector_id)
        {
            *points = candidate;
        }
    }
}

fn repair_branch_input_output_gutters(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut moves = Vec::new();
    for object in &plan.objects {
        let DrawObject::Connector { from, to, .. } = object else {
            continue;
        };
        let (Some(input_id), Some(encoder_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(input), Some(encoder)) = (box_map.get(input_id), box_map.get(encoder_id)) else {
            continue;
        };
        if !is_input_route_box(input_id, input) || !is_encoder_like_route_box(encoder_id, encoder) {
            continue;
        }
        let Some((output_id, output)) =
            branch_output_above_input(plan, encoder_id, input.bbox, &box_map)
        else {
            continue;
        };
        if vertical_separation(output.bbox, input.bbox) >= 0.055 {
            continue;
        }
        let height = box_height(input.bbox);
        let candidate = box_from_top_left_inside(
            input.bbox[0],
            output.bbox[3] + 0.060,
            box_width(input.bbox),
            height,
        );
        let ignored_ids = HashSet::from([encoder_id.to_string(), output_id]);
        if route_box_candidate_is_clear_except(input_id, candidate, &box_map, &ignored_ids) {
            moves.push((input_id.to_string(), candidate));
        }
    }

    if moves.is_empty() {
        return;
    }
    let mut moved_ids = HashSet::new();
    for (id, bbox) in moves {
        set_box_bbox(plan, &id, bbox);
        moved_ids.insert(id);
    }
    realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    reroute_branch_residual_connectors_around_inputs(plan);
}

fn is_encoder_like_route_box(id: &str, info: &BoxRouteInfo) -> bool {
    route_box_text(id, info).contains("encoder")
}

fn branch_output_above_input(
    plan: &DrawPlan,
    encoder_id: &str,
    input_bbox: [f64; 4],
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<(String, BoxRouteInfo)> {
    plan.objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Connector { from, to, .. } = object else {
                return None;
            };
            let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
                return None;
            };
            let output_id = if from_id == encoder_id {
                to_id
            } else if to_id == encoder_id {
                from_id
            } else {
                return None;
            };
            let output = box_map.get(output_id)?;
            if !is_output_route_box(output_id, output)
                || center_y(output.bbox) >= center_y(input_bbox)
            {
                return None;
            }
            Some((output_id.to_string(), output.clone()))
        })
        .min_by(|left, right| {
            vertical_separation(left.1.bbox, input_bbox)
                .total_cmp(&vertical_separation(right.1.bbox, input_bbox))
        })
}

fn reroute_branch_residual_connectors_around_inputs(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let input_boxes = box_map
        .iter()
        .filter_map(|(id, info)| is_input_route_box(id, info).then_some((id.as_str(), info.bbox)))
        .collect::<Vec<_>>();
    if input_boxes.is_empty() {
        return;
    }
    let mut updates = Vec::new();
    for object in &plan.objects {
        let DrawObject::Connector {
            id,
            points,
            from,
            to,
            ..
        } = object
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(from_box), Some(to_box)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        if !(is_output_route_box(from_id, from_box)
            && is_residual_or_supervision_hub_box(to_id, to_box)
            || is_residual_or_supervision_hub_box(from_id, from_box)
                && is_output_route_box(to_id, to_box))
        {
            continue;
        }
        let blockers = input_boxes
            .iter()
            .filter_map(|(input_id, bbox)| {
                (*input_id != from_id
                    && *input_id != to_id
                    && label_near_any_segment_for_plan(*bbox, points, 0.004))
                .then_some(*bbox)
            })
            .collect::<Vec<_>>();
        if blockers.is_empty() {
            continue;
        }
        let candidate =
            residual_connector_route_above_blockers(from_box.bbox, to_box.bbox, &blockers);
        if !connector_points_intersect_intermediate_boxes(&candidate, from_id, to_id, &box_map) {
            updates.push((id.clone(), candidate));
        }
    }
    for (id, candidate) in updates {
        if let Some(DrawObject::Connector { points, .. }) = plan
            .objects
            .iter_mut()
            .find(|object| draw_object_id(object) == id)
        {
            *points = candidate;
        }
    }
}

fn label_near_any_segment_for_plan(bbox: [f64; 4], points: &[[f64; 2]], margin: f64) -> bool {
    points.windows(2).any(|window| {
        let segment_box = expand_box(
            [
                window[0][0].min(window[1][0]),
                window[0][1].min(window[1][1]),
                window[0][0].max(window[1][0]),
                window[0][1].max(window[1][1]),
            ],
            margin,
        );
        intersection_area(bbox, segment_box) > 0.0001
    })
}

fn residual_connector_route_above_blockers(
    from_box: [f64; 4],
    to_box: [f64; 4],
    blockers: &[[f64; 4]],
) -> Vec<[f64; 2]> {
    let start = anchor_point_towards(from_box, center(to_box));
    let end = anchor_point_towards(to_box, center(from_box));
    let blocker_top = blockers
        .iter()
        .map(|bbox| bbox[1])
        .fold(from_box[1].min(to_box[1]), f64::min);
    let rail_y = (from_box[1].min(to_box[1]).min(blocker_top) - 0.035).clamp(0.05, 0.95);
    remove_redundant_collinear_points(&[start, [start[0], rail_y], [end[0], rail_y], end])
}

fn move_task_losses_to_clear_blocked_vertical_output_lanes(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut updates = Vec::new();
    for object in &plan.objects {
        let DrawObject::Connector {
            points, from, to, ..
        } = object
        else {
            continue;
        };
        let (Some(output_id), Some(loss_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(output), Some(loss)) = (box_map.get(output_id), box_map.get(loss_id)) else {
            continue;
        };
        if !is_output_route_box(output_id, output)
            || !is_task_loss_route_box(loss_id, loss)
            || vertical_separation(output.bbox, loss.bbox) < 0.08
            || horizontal_overlap(output.bbox, loss.bbox) < 0.02
        {
            continue;
        }
        let Some(x) = common_vertical_x(output.bbox, loss.bbox) else {
            continue;
        };
        let direct = vertical_connector_points(output.bbox, loss.bbox, x);
        if !connector_points_intersect_intermediate_boxes(&direct, output_id, loss_id, &box_map)
            && points.len() <= 2
        {
            continue;
        }
        let blockers = vertical_lane_blockers(&direct, output_id, loss_id, &box_map);
        let Some(candidate) = task_loss_clear_vertical_lane_candidate(
            loss_id,
            loss.bbox,
            output.bbox,
            &blockers,
            &box_map,
        ) else {
            continue;
        };
        updates.push((loss_id.to_string(), candidate));
    }
    if updates.is_empty() {
        return;
    }
    let mut moved_ids = HashSet::new();
    for (id, bbox) in updates {
        set_box_bbox(plan, &id, bbox);
        moved_ids.insert(id);
    }
    realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
}

fn vertical_lane_blockers(
    points: &[[f64; 2]],
    from_id: &str,
    to_id: &str,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Vec<[f64; 4]> {
    box_map
        .iter()
        .filter_map(|(id, info)| {
            (id != from_id
                && id != to_id
                && label_near_any_segment_for_plan(info.bbox, points, 0.004))
            .then_some(info.bbox)
        })
        .collect()
}

fn task_loss_clear_vertical_lane_candidate(
    loss_id: &str,
    loss: [f64; 4],
    output: [f64; 4],
    blockers: &[[f64; 4]],
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<[f64; 4]> {
    let width = box_width(loss).clamp(0.09, 0.16);
    let height = box_height(loss).clamp(0.085, 0.12);
    let mut lane_xs = Vec::new();
    for blocker in blockers {
        lane_xs.push(blocker[0] - 0.025);
        lane_xs.push(blocker[2] + 0.025);
    }
    lane_xs.push(output[0] + 0.025);
    lane_xs.push(output[2] - 0.025);
    lane_xs
        .into_iter()
        .filter(|x| coordinate_inside(*x, output[0] + 0.010, output[2] - 0.010))
        .map(|x| box_from_top_left_inside(x - width / 2.0, loss[1], width, height))
        .find(|candidate| objective_hub_candidate_is_clear(loss_id, *candidate, box_map))
}

fn repair_same_row_teacher_student_shared_input_collapse(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut repairs = Vec::new();

    for object in &plan.objects {
        let DrawObject::Connector {
            from, to, points, ..
        } = object
        else {
            continue;
        };
        let (Some(input_id), Some(student_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(input), Some(student)) = (box_map.get(input_id), box_map.get(student_id)) else {
            continue;
        };
        if !is_input_route_box(input_id, input)
            || !is_student_or_main_route_box(student)
            || points.len() < 4
        {
            continue;
        }

        let Some((teacher_id, teacher)) =
            same_row_teacher_target_for_shared_input(plan, input_id, student_id, &box_map)
        else {
            continue;
        };
        if !same_row_teacher_student_collapse(input.bbox, teacher.bbox, student.bbox) {
            continue;
        }
        let residual_id =
            residual_objective_between_same_row_branches(plan, &teacher_id, student_id, &box_map);
        let teacher_height = box_height(teacher.bbox);
        let teacher_gap = 0.070;
        let teacher_y2 = student.bbox[1] - teacher_gap;
        if teacher_y2 <= 0.08 + teacher_height {
            continue;
        }
        let teacher_candidate = [
            teacher.bbox[0],
            teacher_y2 - teacher_height,
            teacher.bbox[2],
            teacher_y2,
        ];

        let mut ignored_ids = HashSet::from([teacher_id.clone()]);
        if let Some(residual_id) = &residual_id {
            ignored_ids.insert(residual_id.clone());
        }
        if !route_box_candidate_is_clear_except(
            &teacher_id,
            teacher_candidate,
            &box_map,
            &ignored_ids,
        ) {
            continue;
        }

        let residual_candidate = residual_id.as_ref().and_then(|residual_id| {
            let residual = box_map.get(residual_id)?;
            same_row_residual_repair_candidate(
                residual_id,
                residual,
                teacher_candidate,
                student.bbox,
                &box_map,
                &ignored_ids,
            )
        });
        repairs.push((
            input_id.to_string(),
            teacher_id,
            student_id.to_string(),
            residual_id,
            teacher_candidate,
            residual_candidate,
        ));
    }

    if repairs.is_empty() {
        return;
    }

    let mut moved_ids = HashSet::new();
    for (_, teacher_id, _, residual_id, teacher_candidate, residual_candidate) in &repairs {
        set_box_bbox(plan, teacher_id, *teacher_candidate);
        moved_ids.insert(teacher_id.clone());
        if let (Some(residual_id), Some(residual_candidate)) = (residual_id, residual_candidate) {
            set_box_bbox(plan, residual_id, *residual_candidate);
            moved_ids.insert(residual_id.clone());
        }
    }
    if !moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    }
    for (input_id, teacher_id, student_id, residual_id, _, _) in repairs {
        reroute_same_row_teacher_student_repair_connectors(
            plan,
            &input_id,
            &teacher_id,
            &student_id,
            residual_id.as_deref(),
        );
    }
}

fn same_row_teacher_target_for_shared_input(
    plan: &DrawPlan,
    input_id: &str,
    student_id: &str,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<(String, BoxRouteInfo)> {
    plan.objects.iter().find_map(|object| {
        let DrawObject::Connector { from, to, .. } = object else {
            return None;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            return None;
        };
        if from_id != input_id || to_id == student_id {
            return None;
        }
        let target = box_map.get(to_id)?;
        is_teacher_or_context_route_box(to_id, target).then(|| (to_id.to_string(), target.clone()))
    })
}

fn same_row_teacher_student_collapse(
    input: [f64; 4],
    teacher: [f64; 4],
    student: [f64; 4],
) -> bool {
    input[2] <= teacher[0] + 0.02
        && teacher[2] + 0.10 < student[0]
        && (center_y(teacher) - center_y(student)).abs() < 0.045
        && axis_overlap_ratio(teacher[1], teacher[3], student[1], student[3]) > 0.45
}

fn residual_objective_between_same_row_branches(
    plan: &DrawPlan,
    teacher_id: &str,
    student_id: &str,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<String> {
    plan.objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Connector { from, to, .. } = object else {
                return None;
            };
            let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
                return None;
            };
            let other_id = if from_id == teacher_id || from_id == student_id {
                to_id
            } else if to_id == teacher_id || to_id == student_id {
                from_id
            } else {
                return None;
            };
            let other = box_map.get(other_id)?;
            is_loss_or_objective_box(other_id, other).then(|| other_id.to_string())
        })
        .next()
}

fn same_row_residual_repair_candidate(
    residual_id: &str,
    residual: &BoxRouteInfo,
    teacher: [f64; 4],
    student: [f64; 4],
    box_map: &HashMap<String, BoxRouteInfo>,
    ignored_ids: &HashSet<String>,
) -> Option<[f64; 4]> {
    let width = box_width(residual.bbox).clamp(0.12, 0.17);
    let height = box_height(residual.bbox).clamp(0.09, 0.12);
    let x1 = (teacher[2] + 0.055).clamp(0.02, 0.98 - width);
    let y1 = (center_y(teacher) - height / 2.0).clamp(0.02, 0.98 - height);
    let candidate = [x1, y1, x1 + width, y1 + height];
    if horizontal_separation(teacher, candidate) < 0.045
        || horizontal_separation(candidate, student) < 0.035
        || !route_box_candidate_is_clear_except(residual_id, candidate, box_map, ignored_ids)
    {
        return None;
    }
    Some(candidate)
}

fn reroute_same_row_teacher_student_repair_connectors(
    plan: &mut DrawPlan,
    input_id: &str,
    teacher_id: &str,
    student_id: &str,
    residual_id: Option<&str>,
) {
    let box_map = current_box_route_info_map(plan);
    for object in &mut plan.objects {
        let DrawObject::Connector {
            points, from, to, ..
        } = object
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let should_reroute = (from_id == input_id && (to_id == teacher_id || to_id == student_id))
            || residual_id.is_some_and(|residual_id| {
                (from_id == residual_id && (to_id == teacher_id || to_id == student_id))
                    || (to_id == residual_id && (from_id == teacher_id || from_id == student_id))
            });
        if !should_reroute {
            continue;
        }
        let (Some(from_box), Some(to_box)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        *points = orthogonal_connector_points_between_boxes(from_box.bbox, to_box.bbox);
    }
}

fn align_single_input_boxes_with_main_targets(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut targets_by_input: HashMap<String, Vec<(String, BoxRouteInfo)>> = HashMap::new();
    for object in &plan.objects {
        let DrawObject::Connector { from, to, .. } = object else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(input), Some(target)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        if is_input_route_box(from_id, input) {
            targets_by_input
                .entry(from_id.to_string())
                .or_default()
                .push((to_id.to_string(), target.clone()));
        }
    }

    let mut moved_ids = HashSet::new();
    for (input_id, targets) in targets_by_input {
        if targets.len() != 1 {
            continue;
        }
        let Some(input) = box_map.get(input_id.as_str()) else {
            continue;
        };
        let (_, target) = &targets[0];
        if !is_main_route_box(target) || vertical_separation(input.bbox, target.bbox) < 0.06 {
            continue;
        }
        let height = box_height(input.bbox);
        let y1 = (center_y(target.bbox) - height / 2.0).clamp(0.02, 0.98 - height);
        let candidate = [input.bbox[0], y1, input.bbox[2], y1 + height];
        if route_box_candidate_is_clear(input_id.as_str(), candidate, &box_map) {
            set_box_bbox(plan, input_id.as_str(), candidate);
            moved_ids.insert(input_id);
        }
    }

    if !moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    }
}

fn align_task_loss_boxes_with_outputs(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut moved_ids = HashSet::new();
    let mut moves = Vec::new();
    for object in &plan.objects {
        let DrawObject::Connector { from, to, .. } = object else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(output), Some(task_loss)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        if !is_output_route_box(from_id, output) || !is_task_loss_route_box(to_id, task_loss) {
            continue;
        }
        if vertical_separation(output.bbox, task_loss.bbox) < 0.03
            || (center_x(output.bbox) - center_x(task_loss.bbox)).abs() < 0.04
        {
            continue;
        }
        let width = box_width(task_loss.bbox);
        let x1 = (center_x(output.bbox) - width / 2.0).clamp(0.06, 0.94 - width);
        let candidate = [x1, task_loss.bbox[1], x1 + width, task_loss.bbox[3]];
        if route_box_candidate_is_clear(to_id, candidate, &box_map) {
            moves.push((to_id.to_string(), candidate));
        }
    }
    for (id, bbox) in moves {
        set_box_bbox(plan, &id, bbox);
        moved_ids.insert(id);
    }
    if !moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    }
}

fn align_touching_task_loss_boxes_with_sources(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut moved_ids = HashSet::new();
    let mut moves = Vec::new();
    for object in &plan.objects {
        let DrawObject::Connector { from, to, .. } = object else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(source), Some(task_loss)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        if !is_task_loss_route_box(to_id, task_loss) || is_input_route_box(from_id, source) {
            continue;
        }
        let horizontal_gap = horizontal_separation(source.bbox, task_loss.bbox);
        if horizontal_gap > 0.03 || (center_y(source.bbox) - center_y(task_loss.bbox)).abs() > 0.08
        {
            continue;
        }
        let width = box_width(task_loss.bbox);
        let height = box_height(task_loss.bbox);
        let x1 = (center_x(source.bbox) - width / 2.0).clamp(0.06, 0.94 - width);
        let below_y = source.bbox[3] + 0.05;
        let above_y = source.bbox[1] - 0.05 - height;
        let candidates = [
            [x1, below_y, x1 + width, below_y + height],
            [x1, above_y, x1 + width, above_y + height],
        ];
        if let Some(candidate) = candidates.into_iter().find(|candidate| {
            candidate[1] >= 0.06
                && candidate[3] <= 0.94
                && route_box_candidate_is_clear(to_id, *candidate, &box_map)
        }) {
            moves.push((to_id.to_string(), candidate));
        }
    }
    for (id, bbox) in moves {
        set_box_bbox(plan, &id, bbox);
        moved_ids.insert(id);
    }
    if !moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    }
}

fn move_task_loss_boxes_out_of_output_main_corridors(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let output_to_loss = plan
        .objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Connector { from, to, .. } = object else {
                return None;
            };
            let (Some(output_id), Some(loss_id)) = (from.as_deref(), to.as_deref()) else {
                return None;
            };
            let output = box_map.get(output_id)?;
            let loss = box_map.get(loss_id)?;
            (is_output_route_box(output_id, output) && is_task_loss_route_box(loss_id, loss))
                .then(|| (output_id.to_string(), loss_id.to_string()))
        })
        .collect::<Vec<_>>();
    if output_to_loss.is_empty() {
        return;
    }

    let mut updates = Vec::new();
    for (output_id, loss_id) in output_to_loss {
        let (Some(output), Some(task_loss)) = (box_map.get(&output_id), box_map.get(&loss_id))
        else {
            continue;
        };
        let Some(source) = plan.objects.iter().find_map(|object| {
            let DrawObject::Connector { from, to, .. } = object else {
                return None;
            };
            let (Some(source_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
                return None;
            };
            if to_id != output_id {
                return None;
            }
            let source = box_map.get(source_id)?;
            is_main_route_box(source).then_some(source)
        }) else {
            continue;
        };
        if !task_loss_sits_between_output_and_main(task_loss.bbox, output.bbox, source.bbox) {
            continue;
        }
        let Some(candidate) =
            task_loss_side_candidate(&loss_id, task_loss.bbox, output.bbox, &box_map)
        else {
            continue;
        };
        updates.push((loss_id, candidate));
    }

    if updates.is_empty() {
        return;
    }
    let mut moved_ids = HashSet::new();
    for (id, bbox) in updates {
        set_box_bbox(plan, &id, bbox);
        moved_ids.insert(id);
    }
    realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
}

fn task_loss_sits_between_output_and_main(
    task_loss: [f64; 4],
    output: [f64; 4],
    main: [f64; 4],
) -> bool {
    let output_above = output[3] <= main[1];
    let main_above = main[3] <= output[1];
    if !output_above && !main_above {
        return false;
    }
    let corridor_y1 = output[3].min(main[3]);
    let corridor_y2 = output[1].max(main[1]);
    let task_center_y = center_y(task_loss);
    task_center_y > corridor_y1
        && task_center_y < corridor_y2
        && horizontal_overlap(task_loss, union_box(output, main)) > 0.02
}

fn task_loss_side_candidate(
    loss_id: &str,
    task_loss: [f64; 4],
    output: [f64; 4],
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<[f64; 4]> {
    let width = box_width(task_loss);
    let height = box_height(task_loss);
    let y1 = center_y(output) - height / 2.0;
    let candidates = [
        box_from_top_left_inside(output[0] - 0.05 - width, y1, width, height),
        box_from_top_left_inside(output[2] + 0.05, y1, width, height),
    ];
    candidates
        .into_iter()
        .filter(|candidate| objective_hub_candidate_is_clear(loss_id, *candidate, box_map))
        .min_by(|left, right| {
            box_center_distance(*left, task_loss).total_cmp(&box_center_distance(*right, task_loss))
        })
}

fn move_task_loss_boxes_out_of_main_output_horizontal_corridors(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut updates = Vec::new();

    for object in &plan.objects {
        let DrawObject::Connector { from, to, .. } = object else {
            continue;
        };
        let (Some(main_id), Some(output_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(main), Some(output)) = (box_map.get(main_id), box_map.get(output_id)) else {
            continue;
        };
        if !is_main_route_box(main) || !is_output_route_box(output_id, output) {
            continue;
        }
        for loss_id in task_loss_ids_connected_from_source(plan, &box_map, main_id) {
            let Some(task_loss) = box_map.get(loss_id.as_str()) else {
                continue;
            };
            if !task_loss_blocks_main_output_horizontal_lane(task_loss.bbox, main.bbox, output.bbox)
            {
                continue;
            }
            let Some(candidate) = task_loss_vertical_side_candidate(
                loss_id.as_str(),
                task_loss.bbox,
                main.bbox,
                output.bbox,
                &box_map,
            ) else {
                continue;
            };
            updates.push((loss_id, candidate));
        }
    }

    if updates.is_empty() {
        return;
    }
    let mut moved_ids = HashSet::new();
    for (id, bbox) in updates {
        set_box_bbox(plan, &id, bbox);
        moved_ids.insert(id);
    }
    realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
}

fn task_loss_ids_connected_from_source(
    plan: &DrawPlan,
    box_map: &HashMap<String, BoxRouteInfo>,
    source_id: &str,
) -> Vec<String> {
    plan.objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Connector { from, to, .. } = object else {
                return None;
            };
            let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
                return None;
            };
            let task_loss = box_map.get(to_id)?;
            (from_id == source_id && is_task_loss_route_box(to_id, task_loss))
                .then(|| to_id.to_string())
        })
        .collect()
}

fn task_loss_blocks_main_output_horizontal_lane(
    task_loss: [f64; 4],
    main: [f64; 4],
    output: [f64; 4],
) -> bool {
    if vertical_separation(main, output) >= 0.04 {
        return false;
    }
    let main_left = center_x(main) <= center_x(output);
    let lane_x1 = if main_left { main[2] } else { output[2] };
    let lane_x2 = if main_left { output[0] } else { main[0] };
    if !ranges_overlap(lane_x1, lane_x2, task_loss[0], task_loss[2]) {
        return false;
    }
    let lane_y = center_y(main).clamp(output[1], output[3]);
    task_loss[1] < lane_y && task_loss[3] > lane_y
}

fn task_loss_vertical_side_candidate(
    loss_id: &str,
    task_loss: [f64; 4],
    main: [f64; 4],
    output: [f64; 4],
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<[f64; 4]> {
    let width = box_width(task_loss);
    let height = box_height(task_loss);
    let corridor = union_box(main, output);
    let x1 = task_loss[0].clamp(0.04, 0.96 - width);
    let above_y = corridor[1] - 0.035 - height;
    let below_y = corridor[3] + 0.035;
    let mut candidates = Vec::new();
    if above_y >= 0.04 {
        candidates.push([x1, above_y, x1 + width, above_y + height]);
    }
    if below_y + height <= 0.96 {
        candidates.push([x1, below_y, x1 + width, below_y + height]);
    }
    candidates
        .into_iter()
        .filter(|candidate| objective_hub_candidate_is_clear(loss_id, *candidate, box_map))
        .min_by(|left, right| {
            box_center_distance(*left, task_loss).total_cmp(&box_center_distance(*right, task_loss))
        })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MultistageTeacherStudentStage {
    Encoder,
    Projection,
    Output,
}

fn balance_simple_teacher_student_y_branch_layout(plan: &mut DrawPlan) {
    let Some((teacher_id, student_id, residual_id)) = simple_teacher_student_y_branch_ids(plan)
    else {
        return;
    };

    ensure_simple_y_teacher_is_muted_dashed(plan, &teacher_id);
    style_teacher_residual_branch_as_supervision(plan, &teacher_id, &residual_id);

    let mut moved_ids = HashSet::new();
    let box_map = current_box_route_info_map(plan);
    if let (Some(teacher), Some(student)) = (box_map.get(&teacher_id), box_map.get(&student_id)) {
        if let Some(candidate) = simple_y_teacher_above_student_candidate(
            &teacher_id,
            teacher.bbox,
            student.bbox,
            &box_map,
        )
        .or_else(|| {
            simple_y_balanced_teacher_candidate(&teacher_id, teacher.bbox, student.bbox, &box_map)
        }) {
            set_box_bbox(plan, &teacher_id, candidate);
            moved_ids.insert(teacher_id.clone());
        }
    }

    let box_map = current_box_route_info_map(plan);
    if let (Some(teacher), Some(student), Some(residual)) = (
        box_map.get(&teacher_id),
        box_map.get(&student_id),
        box_map.get(&residual_id),
    ) {
        if let Some(candidate) = simple_y_readable_residual_candidate(
            plan,
            &student_id,
            &residual_id,
            teacher.bbox,
            student.bbox,
            residual.bbox,
            &box_map,
        ) {
            set_box_bbox(plan, &residual_id, candidate);
            moved_ids.insert(residual_id.clone());
        }
    }

    if !moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
        reroute_connectors_touching_box_ids_orthogonally(plan, &moved_ids);
    }

    let box_map = current_box_route_info_map(plan);
    if let Some(student) = box_map.get(&student_id) {
        move_simple_y_inference_caption_below_student(plan, student.bbox);
    }
}

fn simple_teacher_student_y_branch_ids(plan: &DrawPlan) -> Option<(String, String, String)> {
    let box_map = current_box_route_info_map(plan);
    if multistage_teacher_student_stage_pairs(&box_map).len() >= 2 {
        return None;
    }

    let teacher_candidates: Vec<_> = box_map
        .iter()
        .filter(|(id, info)| is_simple_y_teacher_box(id, info))
        .map(|(id, info)| (id.clone(), info.bbox))
        .collect();
    let student_candidates: Vec<_> = box_map
        .iter()
        .filter(|(id, info)| is_simple_y_student_box(id, info))
        .map(|(id, info)| (id.clone(), info.bbox))
        .collect();
    if teacher_candidates.len() != 1 || student_candidates.len() != 1 {
        return None;
    }

    let teacher_id = teacher_candidates[0].0.clone();
    let student_id = student_candidates[0].0.clone();
    let residual_id = box_map
        .iter()
        .filter(|(id, info)| {
            is_residual_or_supervision_hub_box(id, info)
                && connector_between_ids(plan, &teacher_id, id)
                && connector_between_ids(plan, &student_id, id)
        })
        .min_by(|left, right| {
            let left_bbox = left.1.bbox;
            let right_bbox = right.1.bbox;
            box_area(left_bbox).total_cmp(&box_area(right_bbox))
        })
        .map(|(id, _)| id.clone())?;

    Some((teacher_id, student_id, residual_id))
}

fn is_simple_y_teacher_box(id: &str, info: &BoxRouteInfo) -> bool {
    let text = route_box_text(id, info);
    text.contains("teacher")
        && is_teacher_or_context_route_box(id, info)
        && is_simple_y_branch_box(&text)
}

fn is_simple_y_student_box(id: &str, info: &BoxRouteInfo) -> bool {
    let text = route_box_text(id, info);
    text.contains("student") && is_student_or_main_route_box(info) && is_simple_y_branch_box(&text)
}

fn is_simple_y_branch_box(text: &str) -> bool {
    ![
        "encoder",
        "decoder",
        "projection",
        "projector",
        "head",
        "latent",
        "output",
        "prediction",
        "input",
        "loss",
        "residual",
        "inference",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

fn connector_between_ids(plan: &DrawPlan, left_id: &str, right_id: &str) -> bool {
    plan.objects.iter().any(|object| {
        let DrawObject::Connector { from, to, .. } = object else {
            return false;
        };
        matches!(
            (from.as_deref(), to.as_deref()),
            (Some(from_id), Some(to_id))
                if (from_id == left_id && to_id == right_id)
                    || (from_id == right_id && to_id == left_id)
        )
    })
}

fn ensure_simple_y_teacher_is_muted_dashed(plan: &mut DrawPlan, teacher_id: &str) {
    for object in &mut plan.objects {
        let DrawObject::Box { id, style, .. } = object else {
            continue;
        };
        if id != teacher_id {
            continue;
        }
        let style_lower = style.to_lowercase();
        if !style_lower.contains("muted") {
            if !style.is_empty() {
                style.push('_');
            }
            style.push_str("muted");
        }
        if !style.to_lowercase().contains("dash") {
            if !style.is_empty() {
                style.push('_');
            }
            style.push_str("dashed");
        }
    }
}

fn style_teacher_residual_branch_as_supervision(
    plan: &mut DrawPlan,
    teacher_id: &str,
    residual_id: &str,
) {
    for object in &mut plan.objects {
        let DrawObject::Connector {
            from, to, style, ..
        } = object
        else {
            continue;
        };
        let touches_teacher_residual = matches!(
            (from.as_deref(), to.as_deref()),
            (Some(from_id), Some(to_id))
                if (from_id == teacher_id && to_id == residual_id)
                    || (from_id == residual_id && to_id == teacher_id)
        );
        if touches_teacher_residual && !style.to_lowercase().contains("dash") {
            *style = "dashed_supervision".to_string();
        }
    }
}

fn simple_y_balanced_teacher_candidate(
    teacher_id: &str,
    teacher_bbox: [f64; 4],
    student_bbox: [f64; 4],
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<[f64; 4]> {
    let teacher_width = box_width(teacher_bbox);
    let student_width = box_width(student_bbox);
    let target_width = if teacher_width > student_width * 1.15 {
        student_width * 1.08
    } else if teacher_width < student_width * 0.82 {
        student_width * 0.92
    } else {
        return None;
    }
    .clamp(0.20, (student_width * 1.15).max(0.20));
    let x1 = (center_x(teacher_bbox) - target_width / 2.0).clamp(0.02, 0.98 - target_width);
    let candidate = [x1, teacher_bbox[1], x1 + target_width, teacher_bbox[3]];
    route_box_candidate_is_clear(teacher_id, candidate, box_map).then_some(candidate)
}

fn simple_y_teacher_above_student_candidate(
    teacher_id: &str,
    teacher_bbox: [f64; 4],
    student_bbox: [f64; 4],
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<[f64; 4]> {
    let (_, vertical_overlap) = intersection_dimensions(teacher_bbox, student_bbox);
    if vertical_overlap < 0.08 || horizontal_separation(teacher_bbox, student_bbox) < 0.12 {
        return None;
    }
    let student_width = box_width(student_bbox);
    let width = student_width
        .mul_add(0.92, 0.0)
        .max(box_width(teacher_bbox))
        .clamp(0.20, (student_width * 1.08).max(0.20));
    let height = box_height(teacher_bbox).clamp(0.15, 0.20);
    let y1 = student_bbox[1] - 0.070 - height;
    let x_positions = [
        center_x(student_bbox) - width / 2.0,
        student_bbox[0],
        center_x(union_box(teacher_bbox, student_bbox)) - width / 2.0,
    ];
    x_positions
        .into_iter()
        .map(|x1| box_from_top_left_inside(x1, y1, width, height))
        .find(|candidate| route_box_candidate_is_clear(teacher_id, *candidate, box_map))
}

fn simple_y_readable_residual_candidate(
    plan: &DrawPlan,
    student_id: &str,
    residual_id: &str,
    teacher_bbox: [f64; 4],
    student_bbox: [f64; 4],
    residual_bbox: [f64; 4],
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<[f64; 4]> {
    let route_height_tall =
        connector_route_height_between_ids(plan, student_id, residual_id).unwrap_or(0.0) > 0.25;
    let needs_repair = box_width(residual_bbox) < 0.17
        || vertical_separation(residual_bbox, teacher_bbox) < 0.05
        || vertical_separation(residual_bbox, student_bbox) < 0.05
        || route_height_tall;
    if !needs_repair {
        return None;
    }

    let width = box_width(residual_bbox).max(0.18).min(0.22);
    let height = box_height(residual_bbox).max(0.11).min(0.14);
    let lower_slot_y = (student_bbox[1] - 0.055 - height).max(teacher_bbox[3] + 0.075);
    let x_positions = [
        student_bbox[2] + 0.09,
        residual_bbox[0],
        teacher_bbox[2] + 0.08,
        center_x(student_bbox) + 0.18,
    ];
    x_positions
        .into_iter()
        .map(|x1| box_from_top_left_inside(x1, lower_slot_y, width, height))
        .find(|candidate| {
            objective_hub_candidate_is_clear(residual_id, *candidate, box_map)
                && vertical_separation(*candidate, student_bbox) >= 0.05
        })
}

fn connector_route_height_between_ids(
    plan: &DrawPlan,
    left_id: &str,
    right_id: &str,
) -> Option<f64> {
    plan.objects.iter().find_map(|object| {
        let DrawObject::Connector {
            points, from, to, ..
        } = object
        else {
            return None;
        };
        let is_match = matches!(
            (from.as_deref(), to.as_deref()),
            (Some(from_id), Some(to_id))
                if (from_id == left_id && to_id == right_id)
                    || (from_id == right_id && to_id == left_id)
        );
        is_match.then(|| {
            let route_box = points_to_box(points);
            box_height(route_box)
        })
    })
}

fn move_simple_y_inference_caption_below_student(plan: &mut DrawPlan, student_bbox: [f64; 4]) {
    let component_boxes = current_box_map(plan);
    let mut moves = Vec::new();
    for object in &plan.objects {
        let DrawObject::Text { id, bbox, text, .. } = object else {
            continue;
        };
        let label = format!("{} {}", id.to_lowercase(), text.to_lowercase());
        if !label.contains("inference") || bbox[1] >= student_bbox[3] + 0.08 {
            continue;
        }
        let width = box_width(*bbox).clamp(0.18, 0.24);
        let height = box_height(*bbox).clamp(0.05, 0.065);
        let candidates = simple_y_inference_caption_candidates(student_bbox, width, height);
        if let Some(candidate) = candidates
            .into_iter()
            .find(|candidate| annotation_candidate_is_clear(*candidate, &component_boxes))
        {
            moves.push((id.clone(), candidate));
        }
    }
    for (id, candidate) in moves {
        set_object_bbox(plan, &id, candidate);
    }
}

fn move_simple_y_branch_inference_caption_to_periphery(plan: &mut DrawPlan) {
    let Some((_teacher_id, student_id, _residual_id)) = simple_teacher_student_y_branch_ids(plan)
    else {
        return;
    };
    let box_map = current_box_route_info_map(plan);
    if let Some(student) = box_map.get(&student_id) {
        move_simple_y_inference_caption_below_student(plan, student.bbox);
    }
}

fn simple_y_inference_caption_candidates(
    student_bbox: [f64; 4],
    width: f64,
    height: f64,
) -> Vec<[f64; 4]> {
    let side_x = student_bbox[2] + 0.035;
    let side_y = center_y(student_bbox) - height / 2.0;
    let below_y = student_bbox[3] + 0.09;
    let center_x = center_x(student_bbox) - width / 2.0;
    vec![
        box_from_top_left_inside(center_x, below_y, width, height),
        box_from_top_left_inside(side_x, side_y, width, height),
        box_from_top_left_inside(side_x, student_bbox[3] + 0.040, width, height),
        box_from_top_left_inside(student_bbox[0] - 0.035 - width, side_y, width, height),
    ]
}

fn move_bottom_margin_inference_texts_near_student(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let Some((_student_id, student)) = student_anchor_for_inference_notes(plan) else {
        return;
    };
    let obstacle_boxes = plan
        .objects
        .iter()
        .filter_map(|object| match object {
            DrawObject::Box { bbox, .. } | DrawObject::Image { bbox, .. } => {
                Some(normalize_box(*bbox))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let segment_boxes = connector_segment_boxes(plan, 0.014);
    let text_boxes = plan
        .objects
        .iter()
        .filter_map(|object| match object {
            DrawObject::Text { id, bbox, .. } => Some((id.clone(), normalize_box(*bbox))),
            _ => None,
        })
        .collect::<Vec<_>>();

    let updates = plan
        .objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Text {
                id,
                bbox,
                text,
                style,
                ..
            } = object
            else {
                return None;
            };
            let bbox = normalize_box(*bbox);
            if !bottom_margin_inference_text_guard_target(id)
                || !bottom_margin_inference_text_needs_anchor(bbox, student.bbox)
                || (!is_student_inference_note_like(id, text, "", style)
                    && !annotation_label_is_inference_specific(text))
            {
                return None;
            }
            let placed_text_boxes = text_boxes
                .iter()
                .filter_map(|(text_id, text_bbox)| (text_id != id).then_some(*text_bbox))
                .collect::<Vec<_>>();
            let candidate =
                bottom_margin_student_inference_text_candidates(bbox, text, student.bbox)
                    .into_iter()
                    .find(|candidate| {
                        candidate[3] <= 0.90
                            && candidate_is_near_inference_anchor(*candidate, &[student.bbox])
                            && connector_label_candidate_clear(
                                *candidate,
                                &obstacle_boxes,
                                &segment_boxes,
                                &placed_text_boxes,
                            )
                            && route_box_candidate_is_clear(id, *candidate, &box_map)
                    })?;
            Some((id.clone(), candidate))
        })
        .collect::<Vec<_>>();

    for (id, bbox) in updates {
        set_object_bbox(plan, &id, bbox);
    }
}

fn bottom_margin_inference_text_guard_target(id: &str) -> bool {
    let id = id.to_lowercase();
    id == "inference_note" || id.contains("inference_note")
}

fn bottom_margin_inference_text_needs_anchor(bbox: [f64; 4], student_bbox: [f64; 4]) -> bool {
    let bbox = normalize_box(bbox);
    let student_already_uses_lower_periphery = student_bbox[3] >= 0.84;
    let right_edge_low = bbox[2] >= 0.97
        && bbox[1] >= student_bbox[3] + 0.08
        && horizontal_separation(bbox, student_bbox) > 0.14;
    (!student_already_uses_lower_periphery && (bbox[1] >= 0.88 || bbox[3] >= 0.95))
        || right_edge_low
}

fn bottom_margin_student_inference_text_candidates(
    current: [f64; 4],
    text: &str,
    student_bbox: [f64; 4],
) -> Vec<[f64; 4]> {
    let width = (0.075 + visible_text_len(text) as f64 * 0.0055)
        .clamp(0.16, 0.21)
        .max(box_width(current).min(0.21));
    let height = box_height(current).clamp(0.052, 0.070);
    let side_y = (center_y(student_bbox) - height / 2.0).clamp(0.06, 0.90 - height);
    let centered_x = center_x(student_bbox) - width / 2.0;
    let left_x = student_bbox[0] - 0.035 - width;
    let right_x = student_bbox[2] + 0.035;
    let above_y = student_bbox[1] - height - 0.025;
    let below_y = student_bbox[3] + 0.035;

    let mut candidates = Vec::new();
    if right_x + width <= 0.98 {
        candidates.push(box_from_top_left_inside(right_x, side_y, width, height));
    }
    if below_y + height <= 0.90 {
        candidates.push(box_from_top_left_inside(centered_x, below_y, width, height));
    }
    if left_x >= 0.02 {
        candidates.push(box_from_top_left_inside(left_x, side_y, width, height));
    }
    if above_y >= 0.02 {
        candidates.push(box_from_top_left_inside(centered_x, above_y, width, height));
    }
    candidates.into_iter().map(normalize_box).collect()
}

fn balance_direct_teacher_student_supervision_branches(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut moves = Vec::new();
    for (teacher_id, student_id) in direct_teacher_student_supervision_pairs(plan, &box_map) {
        let (Some(teacher), Some(student)) = (box_map.get(&teacher_id), box_map.get(&student_id))
        else {
            continue;
        };
        let (_, vertical_overlap) = intersection_dimensions(teacher.bbox, student.bbox);
        let teacher_is_below_or_same_row =
            teacher.bbox[3] > student.bbox[1] - 0.045 || vertical_overlap > 0.04;
        if !teacher_is_below_or_same_row {
            continue;
        }
        if let Some(candidate) = direct_teacher_above_student_candidate(
            &teacher_id,
            teacher.bbox,
            student.bbox,
            &box_map,
        ) {
            moves.push((teacher_id, candidate));
        }
    }

    if moves.is_empty() {
        return;
    }
    let mut moved_ids = HashSet::new();
    for (id, bbox) in moves {
        ensure_simple_y_teacher_is_muted_dashed(plan, &id);
        set_box_bbox(plan, &id, bbox);
        moved_ids.insert(id);
    }
    realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    reroute_connectors_touching_box_ids_orthogonally(plan, &moved_ids);
}

fn direct_teacher_student_supervision_pairs(
    plan: &DrawPlan,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for object in &plan.objects {
        let DrawObject::Connector {
            from,
            to,
            style,
            label,
            ..
        } = object
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(from_box), Some(to_box)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        let label_text = label
            .as_ref()
            .map(|label| label.text.to_lowercase())
            .unwrap_or_default();
        let supervision_like = style.to_lowercase().contains("dash")
            || style.to_lowercase().contains("supervision")
            || label_text.contains("residual")
            || label_text.contains("supervision");
        if !supervision_like {
            continue;
        }
        if is_simple_y_teacher_box(from_id, from_box) && is_simple_y_student_box(to_id, to_box) {
            pairs.push((from_id.to_string(), to_id.to_string()));
        } else if is_simple_y_teacher_box(to_id, to_box)
            && is_simple_y_student_box(from_id, from_box)
        {
            pairs.push((to_id.to_string(), from_id.to_string()));
        }
    }
    pairs
}

fn direct_teacher_above_student_candidate(
    teacher_id: &str,
    teacher_bbox: [f64; 4],
    student_bbox: [f64; 4],
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<[f64; 4]> {
    let width = box_width(teacher_bbox)
        .max(box_width(student_bbox) * 0.88)
        .clamp(0.20, box_width(student_bbox) * 1.08);
    let height = box_height(teacher_bbox).clamp(0.13, 0.18);
    let y1 = student_bbox[1] - 0.070 - height;
    let x_positions = [
        center_x(student_bbox) - width / 2.0,
        student_bbox[0],
        student_bbox[0] - 0.030,
    ];
    x_positions
        .into_iter()
        .map(|x1| box_from_top_left_inside(x1, y1, width, height))
        .find(|candidate| route_box_candidate_is_clear(teacher_id, *candidate, box_map))
}

#[derive(Clone, Debug)]
struct MultistageTeacherStudentPair {
    stage: MultistageTeacherStudentStage,
    teacher_id: String,
    teacher_bbox: [f64; 4],
    student_id: String,
    student_bbox: [f64; 4],
}

fn balance_multistage_teacher_student_branches(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let pairs = multistage_teacher_student_stage_pairs(&box_map);
    if pairs.len() < 2 {
        return;
    }
    if !multistage_pairs_include_projection(&pairs) {
        repair_projectionless_multistage_teacher_student_layout(plan, &pairs);
        return;
    }

    let branch_ids = multistage_teacher_student_branch_ids(&pairs);
    let stage_moved_ids =
        balance_multistage_teacher_student_stage_sizes(plan, &pairs, &branch_ids, &box_map);
    if !stage_moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &stage_moved_ids);
    }

    let box_map = current_box_route_info_map(plan);
    let pairs = multistage_teacher_student_stage_pairs(&box_map);
    if pairs.len() < 2 || !multistage_pairs_include_projection(&pairs) {
        return;
    }
    let branch_ids = multistage_teacher_student_branch_ids(&pairs);
    let mut objective_moved_ids =
        move_multistage_residual_hubs_between_branch_rows(plan, &pairs, &branch_ids);
    objective_moved_ids.extend(move_multistage_task_losses_below_student_outputs(
        plan, &pairs,
    ));
    if !objective_moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &objective_moved_ids);
        reroute_connectors_touching_box_ids_orthogonally(plan, &objective_moved_ids);
    }
}

fn multistage_teacher_student_stage_pairs(
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Vec<MultistageTeacherStudentPair> {
    [
        MultistageTeacherStudentStage::Encoder,
        MultistageTeacherStudentStage::Projection,
        MultistageTeacherStudentStage::Output,
    ]
    .into_iter()
    .filter_map(|stage| {
        let teacher = best_multistage_branch_box(box_map, stage, true)?;
        let student = best_multistage_branch_box(box_map, stage, false)?;
        Some(MultistageTeacherStudentPair {
            stage,
            teacher_id: teacher.0,
            teacher_bbox: teacher.1,
            student_id: student.0,
            student_bbox: student.1,
        })
    })
    .collect()
}

fn best_multistage_branch_box(
    box_map: &HashMap<String, BoxRouteInfo>,
    stage: MultistageTeacherStudentStage,
    teacher: bool,
) -> Option<(String, [f64; 4])> {
    box_map
        .iter()
        .filter_map(|(id, info)| {
            if !is_multistage_branch_box(id, info, stage, teacher) {
                return None;
            }
            let score = multistage_branch_box_score(id, info, stage, teacher);
            Some((id.clone(), info.bbox, score))
        })
        .max_by(|left, right| left.2.total_cmp(&right.2))
        .map(|(id, bbox, _)| (id, bbox))
}

fn is_multistage_branch_box(
    id: &str,
    info: &BoxRouteInfo,
    stage: MultistageTeacherStudentStage,
    teacher: bool,
) -> bool {
    let text = route_box_text(id, info);
    let branch_matches = if teacher {
        text.contains("teacher")
    } else {
        text.contains("student")
    };
    if !branch_matches || is_task_loss_route_box(id, info) || text.contains("inference") {
        return false;
    }
    match stage {
        MultistageTeacherStudentStage::Encoder => {
            text.contains("encoder") || text.contains("backbone")
        }
        MultistageTeacherStudentStage::Projection => {
            text.contains("projection") || text.contains("projector") || text.contains("head")
        }
        MultistageTeacherStudentStage::Output => {
            text.contains("output")
                || text.contains("prediction")
                || text.contains("latent")
                || text.contains("z_t")
                || text.contains("z_s")
                || text.contains('ŷ')
        }
    }
}

fn multistage_branch_box_score(
    id: &str,
    info: &BoxRouteInfo,
    stage: MultistageTeacherStudentStage,
    teacher: bool,
) -> f64 {
    let text = route_box_text(id, info);
    let mut score = 0.0;
    let branch_token = if teacher { "teacher" } else { "student" };
    if id.to_lowercase().contains(branch_token) {
        score += 10.0;
    }
    match stage {
        MultistageTeacherStudentStage::Encoder => {
            if text.contains("encoder") {
                score += 8.0;
            }
        }
        MultistageTeacherStudentStage::Projection => {
            if text.contains("projection") || text.contains("projector") {
                score += 8.0;
            }
        }
        MultistageTeacherStudentStage::Output => {
            if text.contains("output") || text.contains("latent") || text.contains('ŷ') {
                score += 8.0;
            }
        }
    }
    score - edge_pressure(info.bbox)
}

fn multistage_teacher_student_branch_ids(
    pairs: &[MultistageTeacherStudentPair],
) -> HashSet<String> {
    let mut ids = HashSet::new();
    for pair in pairs {
        ids.insert(pair.teacher_id.clone());
        ids.insert(pair.student_id.clone());
    }
    ids
}

fn multistage_pairs_include_projection(pairs: &[MultistageTeacherStudentPair]) -> bool {
    pairs
        .iter()
        .any(|pair| pair.stage == MultistageTeacherStudentStage::Projection)
}

fn repair_projectionless_multistage_teacher_student_layout(
    plan: &mut DrawPlan,
    pairs: &[MultistageTeacherStudentPair],
) {
    let Some(encoder_pair) = pairs
        .iter()
        .find(|pair| pair.stage == MultistageTeacherStudentStage::Encoder)
        .cloned()
    else {
        return;
    };
    let Some(latent_pair) = pairs
        .iter()
        .find(|pair| pair.stage == MultistageTeacherStudentStage::Output)
        .cloned()
    else {
        return;
    };

    let box_map = current_box_route_info_map(plan);
    let Some(student_head_id) =
        connected_student_head_after_latent(plan, &latent_pair.student_id, &box_map)
    else {
        return;
    };

    let mut moved_ids = HashSet::new();
    moved_ids.extend(compact_projectionless_teacher_student_stage_boxes(
        plan,
        &encoder_pair,
        &latent_pair,
        &student_head_id,
    ));

    let box_map = current_box_route_info_map(plan);
    let residual_ids = connected_residual_hubs_for_latent_pair(
        plan,
        &latent_pair.teacher_id,
        &latent_pair.student_id,
        &box_map,
    );
    let mut objective_moved_ids = HashSet::new();
    for residual_id in residual_ids {
        let (Some(residual), Some(teacher_latent), Some(student_latent)) = (
            box_map.get(&residual_id),
            box_map.get(&latent_pair.teacher_id),
            box_map.get(&latent_pair.student_id),
        ) else {
            continue;
        };
        let candidate = projectionless_residual_side_candidate(
            residual.bbox,
            teacher_latent.bbox,
            student_latent.bbox,
        );
        if objective_hub_candidate_is_clear(&residual_id, candidate, &box_map) {
            set_box_bbox(plan, &residual_id, candidate);
            objective_moved_ids.insert(residual_id);
        }
    }

    let box_map = current_box_route_info_map(plan);
    let output_id = connected_output_after_student_head(plan, &student_head_id, &box_map);
    if let Some(task_loss_id) = connected_task_loss_for_source(plan, &student_head_id, &box_map) {
        if let (Some(task_loss), Some(student_head)) =
            (box_map.get(&task_loss_id), box_map.get(&student_head_id))
        {
            let output = output_id
                .as_deref()
                .and_then(|output_id| box_map.get(output_id));
            if let Some(candidate) = projectionless_task_loss_side_candidate(
                &task_loss_id,
                task_loss.bbox,
                student_head.bbox,
                output.map(|info| info.bbox),
                &box_map,
            ) {
                set_box_bbox(plan, &task_loss_id, candidate);
                objective_moved_ids.insert(task_loss_id);
            }
        }
    }

    moved_ids.extend(objective_moved_ids.iter().cloned());
    if !moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
        reroute_connectors_touching_box_ids_orthogonally(plan, &moved_ids);
    }
    reroute_projectionless_multistage_objective_connectors(
        plan,
        &latent_pair,
        &student_head_id,
        output_id.as_deref(),
    );
}

fn repair_two_stage_teacher_student_branch_layout(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let Some(student_encoder_id) = find_two_stage_branch_box(&box_map, "student", "encoder") else {
        return;
    };
    let Some(student_head_id) = find_two_stage_branch_box(&box_map, "student", "head") else {
        return;
    };
    let Some(teacher_encoder_id) = find_two_stage_branch_box(&box_map, "teacher", "encoder") else {
        return;
    };
    let Some(teacher_head_id) = find_two_stage_branch_box(&box_map, "teacher", "head") else {
        return;
    };

    let mut moved_ids = HashSet::new();
    if let (Some(student_encoder), Some(student_head)) = (
        box_map.get(&student_encoder_id),
        box_map.get(&student_head_id),
    ) {
        if student_encoder.bbox[3] <= student_head.bbox[1]
            && vertical_separation(student_encoder.bbox, student_head.bbox) < 0.055
        {
            let candidate =
                encoder_box_above_head_with_gutter(student_encoder.bbox, student_head.bbox);
            set_box_bbox(plan, &student_encoder_id, candidate);
            moved_ids.insert(student_encoder_id.clone());
        }
    }

    if let Some(teacher_encoder) = box_map.get(&teacher_encoder_id) {
        if box_height(teacher_encoder.bbox) > 0.22 {
            set_box_bbox(
                plan,
                &teacher_encoder_id,
                box_with_centered_height_inside(teacher_encoder.bbox, 0.19),
            );
            moved_ids.insert(teacher_encoder_id.clone());
        }
    }

    let mut box_map = current_box_route_info_map(plan);
    if !moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
        reroute_connectors_touching_box_ids_orthogonally(plan, &moved_ids);
        box_map = current_box_route_info_map(plan);
    }

    let task_loss_id = connected_task_loss_for_source(plan, &student_head_id, &box_map)
        .or_else(|| first_task_loss_box(&box_map));
    if let (Some(task_loss_id), Some(student_head)) =
        (task_loss_id.as_deref(), box_map.get(&student_head_id))
    {
        if let Some(task_loss) = box_map.get(task_loss_id) {
            let residual = connected_residual_hub_near_two_stage_branch(plan, &box_map);
            let crowds_student = box_map
                .get(&student_encoder_id)
                .is_some_and(|student_encoder| {
                    vertical_separation(task_loss.bbox, student_encoder.bbox) < 0.055
                        && horizontal_overlap(task_loss.bbox, student_encoder.bbox) > 0.01
                });
            let crowds_residual = residual.as_ref().is_some_and(|(_, residual)| {
                vertical_separation(task_loss.bbox, residual.bbox) < 0.045
                    && horizontal_separation(task_loss.bbox, residual.bbox) < 0.055
            });
            if crowds_student || crowds_residual {
                let candidate =
                    task_loss_below_student_head_candidate(task_loss.bbox, student_head.bbox);
                if route_box_candidate_is_clear_except(
                    task_loss_id,
                    candidate,
                    &box_map,
                    &HashSet::new(),
                ) {
                    set_box_bbox(plan, task_loss_id, candidate);
                    moved_ids.insert(task_loss_id.to_string());
                }
            }
        }
    }

    if !moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
        reroute_connectors_touching_box_ids_orthogonally(plan, &moved_ids);
    }

    reroute_two_stage_input_to_teacher(plan, &teacher_encoder_id, &student_head_id);
    reroute_two_stage_student_task_loss(plan, &student_head_id);
    move_two_stage_inference_caption_to_periphery(plan, &student_head_id);
    repair_two_stage_head_output_and_feedback_routes(
        plan,
        &student_encoder_id,
        &student_head_id,
        &teacher_head_id,
    );
    remove_detached_two_stage_objective_labels(plan);
}

fn find_two_stage_branch_box(
    box_map: &HashMap<String, BoxRouteInfo>,
    branch: &str,
    stage: &str,
) -> Option<String> {
    box_map
        .iter()
        .filter(|(_, info)| {
            let text = route_box_text("", info);
            text.contains(branch) && text.contains(stage)
        })
        .min_by(|left, right| {
            two_stage_branch_box_score(left.0, left.1, branch, stage)
                .total_cmp(&two_stage_branch_box_score(right.0, right.1, branch, stage))
        })
        .map(|(id, _)| id.clone())
}

fn two_stage_branch_box_score(id: &str, info: &BoxRouteInfo, branch: &str, stage: &str) -> f64 {
    let identity = route_box_text(id, info);
    let mut score = 0.0;
    if !id.to_lowercase().contains(branch) {
        score += 1.0;
    }
    if !id.to_lowercase().contains(stage) {
        score += 1.0;
    }
    if is_task_loss_route_box(id, info) || is_residual_or_supervision_hub_box(id, info) {
        score += 10.0;
    }
    if identity.contains("inference") {
        score += 5.0;
    }
    score
}

fn encoder_box_above_head_with_gutter(encoder: [f64; 4], head: [f64; 4]) -> [f64; 4] {
    let width = box_width(encoder);
    let height = box_height(encoder).min(0.19).max(0.12);
    let y2 = (head[1] - 0.065).clamp(0.06 + height, 0.94);
    let y1 = y2 - height;
    normalize_box([encoder[0], y1, encoder[0] + width, y2])
}

fn first_task_loss_box(box_map: &HashMap<String, BoxRouteInfo>) -> Option<String> {
    box_map
        .iter()
        .find_map(|(id, info)| is_task_loss_route_box(id, info).then(|| id.clone()))
}

fn connected_residual_hub_near_two_stage_branch<'a>(
    plan: &DrawPlan,
    box_map: &'a HashMap<String, BoxRouteInfo>,
) -> Option<(String, &'a BoxRouteInfo)> {
    for object in &plan.objects {
        let DrawObject::Connector { from, to, .. } = object else {
            continue;
        };
        for id in [from.as_deref(), to.as_deref()].into_iter().flatten() {
            let Some(info) = box_map.get(id) else {
                continue;
            };
            if is_residual_or_supervision_hub_box(id, info) {
                return Some((id.to_string(), info));
            }
        }
    }
    None
}

fn task_loss_below_student_head_candidate(task_loss: [f64; 4], student_head: [f64; 4]) -> [f64; 4] {
    let width = box_width(task_loss).clamp(0.13, 0.16);
    let height = box_height(task_loss).clamp(0.085, 0.11);
    box_from_top_left_inside(
        center_x(student_head) - width / 2.0,
        student_head[3] + 0.065,
        width,
        height,
    )
}

fn reroute_two_stage_input_to_teacher(
    plan: &mut DrawPlan,
    teacher_encoder_id: &str,
    student_head_id: &str,
) {
    let box_map = current_box_route_info_map(plan);
    let Some(teacher_encoder) = box_map.get(teacher_encoder_id) else {
        return;
    };
    let student_head = box_map.get(student_head_id).map(|info| info.bbox);
    for object in &mut plan.objects {
        let DrawObject::Connector {
            points, from, to, ..
        } = object
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(from_box), Some(to_box)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        if to_id != teacher_encoder_id || !is_input_route_box(from_id, from_box) {
            continue;
        }
        let start = [from_box.bbox[2], center_y(from_box.bbox)];
        let mut rail_y = center_y(teacher_encoder.bbox);
        if let Some(student_head) = student_head {
            rail_y = rail_y.max(student_head[3] + 0.025);
        }
        rail_y = rail_y
            .min(teacher_encoder.bbox[3] - 0.030)
            .max(from_box.bbox[1] + 0.035)
            .clamp(0.06, 0.94);
        *points = remove_redundant_collinear_points(&[
            start,
            [start[0], rail_y],
            [to_box.bbox[0], rail_y],
        ]);
    }
}

fn reroute_two_stage_student_task_loss(plan: &mut DrawPlan, student_head_id: &str) {
    let box_map = current_box_route_info_map(plan);
    for object in &mut plan.objects {
        let DrawObject::Connector {
            points, from, to, ..
        } = object
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let loss_id = if from_id == student_head_id {
            to_id
        } else if to_id == student_head_id {
            from_id
        } else {
            continue;
        };
        let (Some(student_head), Some(task_loss)) =
            (box_map.get(student_head_id), box_map.get(loss_id))
        else {
            continue;
        };
        if !is_task_loss_route_box(loss_id, task_loss) {
            continue;
        }
        if task_loss.bbox[1] >= student_head.bbox[3]
            && horizontal_overlap(task_loss.bbox, student_head.bbox) > 0.01
        {
            let min_x = student_head.bbox[0] + 0.015;
            let max_x = student_head.bbox[2] - 0.015;
            let x = if min_x <= max_x {
                center_x(task_loss.bbox).clamp(min_x, max_x)
            } else {
                center_x(student_head.bbox)
            };
            *points = vec![[x, student_head.bbox[3]], [x, task_loss.bbox[1]]];
        }
    }
}

fn move_two_stage_inference_caption_to_periphery(plan: &mut DrawPlan, student_head_id: &str) {
    let box_map = current_box_route_info_map(plan);
    let Some(student_head) = box_map.get(student_head_id) else {
        return;
    };
    let task_loss =
        first_task_loss_box(&box_map).and_then(|id| box_map.get(&id).map(|info| info.bbox));
    let component_boxes = current_box_map(plan);
    let updates = plan
        .objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Text {
                id,
                bbox,
                text,
                style,
                ..
            } = object
            else {
                return None;
            };
            if !style.to_lowercase().contains("annotation")
                || !annotation_label_is_inference_specific(text)
            {
                return None;
            }
            let width = box_width(*bbox).clamp(0.16, 0.20);
            let height = box_height(*bbox).clamp(0.055, 0.080);
            let anchor_bottom = task_loss
                .map(|bbox| bbox[3])
                .unwrap_or(student_head.bbox[3]);
            let candidates = [
                box_from_top_left_inside(
                    center_x(student_head.bbox) - width / 2.0,
                    anchor_bottom + 0.045,
                    width,
                    height,
                ),
                box_from_top_left_inside(
                    student_head.bbox[2] + 0.040,
                    center_y(student_head.bbox) - height / 2.0,
                    width,
                    height,
                ),
            ];
            candidates
                .into_iter()
                .find(|candidate| annotation_candidate_is_clear(*candidate, &component_boxes))
                .map(|candidate| (id.clone(), candidate))
        })
        .collect::<Vec<_>>();
    for (id, bbox) in updates {
        set_object_bbox(plan, &id, bbox);
    }
}

fn repair_two_stage_head_output_and_feedback_routes(
    plan: &mut DrawPlan,
    student_encoder_id: &str,
    student_head_id: &str,
    teacher_head_id: &str,
) {
    let box_map = current_box_route_info_map(plan);
    if !box_map.contains_key(student_encoder_id) || !box_map.contains_key(student_head_id) {
        return;
    }
    let student_output_id = connected_output_after_student_head(plan, student_head_id, &box_map);
    let task_loss_id = first_task_loss_box(&box_map);
    let residual_id =
        connected_residual_hub_near_two_stage_branch(plan, &box_map).map(|(id, _)| id);
    let teacher_output_id = connected_output_after_student_head(plan, teacher_head_id, &box_map);

    let mut moved_ids = HashSet::new();
    if let Some(residual_id) = residual_id.as_deref() {
        if let Some(residual) = box_map.get(residual_id) {
            let candidate = readable_two_stage_residual_candidate(residual.bbox);
            if !boxes_nearly_equal(candidate, residual.bbox)
                && route_box_candidate_is_clear_except(
                    residual_id,
                    candidate,
                    &box_map,
                    &HashSet::from([residual_id.to_string()]),
                )
            {
                set_box_bbox(plan, residual_id, candidate);
                moved_ids.insert(residual_id.to_string());
            }
        }
    }
    if let Some(teacher_output_id) = teacher_output_id.as_deref() {
        if let Some(teacher_output) = box_map.get(teacher_output_id) {
            if let Some(candidate) =
                compact_tiny_output_candidate(teacher_output.bbox, teacher_output)
            {
                if route_box_candidate_is_clear_except(
                    teacher_output_id,
                    candidate,
                    &box_map,
                    &HashSet::from([teacher_output_id.to_string()]),
                ) {
                    set_box_bbox(plan, teacher_output_id, candidate);
                    moved_ids.insert(teacher_output_id.to_string());
                }
            }
        }
    }
    if !moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    }

    move_two_stage_inference_note_box_to_student_periphery(plan, student_head_id);

    let box_map = current_box_route_info_map(plan);
    let Some(student_encoder) = box_map.get(student_encoder_id) else {
        return;
    };
    let Some(student_head) = box_map.get(student_head_id) else {
        return;
    };
    let student_output = student_output_id
        .as_deref()
        .and_then(|id| box_map.get(id).map(|info| (id, info)));
    let task_loss = task_loss_id
        .as_deref()
        .and_then(|id| box_map.get(id).map(|info| (id, info)));
    let residual = residual_id
        .as_deref()
        .and_then(|id| box_map.get(id).map(|info| (id, info)));

    for object in &mut plan.objects {
        let DrawObject::Connector {
            points,
            from,
            to,
            label,
            ..
        } = object
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        if let Some((output_id, output)) = student_output {
            if connector_matches_pair(from_id, to_id, student_head_id, output_id) {
                *points = two_stage_head_to_output_route(student_head.bbox, output.bbox);
                continue;
            }
        }
        if let Some((loss_id, loss)) = task_loss {
            if connector_matches_pair(from_id, to_id, loss_id, student_encoder_id) {
                *points = two_stage_task_feedback_to_student_route(loss.bbox, student_encoder.bbox);
                continue;
            }
        }
        if let Some((residual_id, residual)) = residual {
            if connector_matches_pair(from_id, to_id, residual_id, student_encoder_id) {
                *points = two_stage_residual_to_student_encoder_route(
                    residual.bbox,
                    student_encoder.bbox,
                );
                continue;
            }
            if connector_matches_pair(from_id, to_id, student_head_id, residual_id) {
                if let Some(current_label) = label.as_mut() {
                    current_label.bbox =
                        two_stage_student_residual_label_bbox(student_head.bbox, residual.bbox);
                }
            }
        }
    }
}

fn readable_two_stage_residual_candidate(residual: [f64; 4]) -> [f64; 4] {
    let width = box_width(residual).max(0.23).min(0.28);
    let height = box_height(residual).max(0.09).min(0.12);
    box_from_center_size(center_x(residual), center_y(residual), width, height)
}

fn compact_tiny_output_candidate(bbox: [f64; 4], info: &BoxRouteInfo) -> Option<[f64; 4]> {
    let text = route_box_text("", info);
    let compact_label = text.chars().filter(|ch| !ch.is_whitespace()).count() <= 4;
    if !compact_label || box_width(bbox) <= 0.17 {
        return None;
    }
    Some(box_from_center_size(
        center_x(bbox),
        center_y(bbox),
        0.14,
        box_height(bbox).clamp(0.075, 0.095),
    ))
}

fn move_two_stage_inference_note_box_to_student_periphery(
    plan: &mut DrawPlan,
    student_head_id: &str,
) {
    let box_map = current_box_route_info_map(plan);
    let Some(student_head) = box_map.get(student_head_id) else {
        return;
    };
    let component_boxes = current_box_map(plan);
    let updates = plan
        .objects
        .iter()
        .filter_map(|object| {
            let (id, bbox, haystack) = match object {
                DrawObject::Box {
                    id,
                    bbox,
                    text,
                    role,
                    style,
                    ..
                } => (
                    id,
                    bbox,
                    format!(
                        "{} {} {} {}",
                        id.to_lowercase(),
                        text.to_lowercase(),
                        role.to_lowercase(),
                        style.to_lowercase()
                    ),
                ),
                DrawObject::Text {
                    id,
                    bbox,
                    text,
                    style,
                    ..
                } => (
                    id,
                    bbox,
                    format!(
                        "{} {} {}",
                        id.to_lowercase(),
                        text.to_lowercase(),
                        style.to_lowercase()
                    ),
                ),
                _ => return None,
            };
            if !haystack.contains("inference") {
                return None;
            }
            let width = box_width(*bbox).clamp(0.18, 0.22);
            let height = box_height(*bbox).clamp(0.075, 0.095);
            let candidates = [
                box_from_top_left_inside(
                    student_head.bbox[2] + 0.040,
                    student_head.bbox[3] + 0.065,
                    width,
                    height,
                ),
                box_from_top_left_inside(
                    student_head.bbox[2] + 0.050,
                    center_y(student_head.bbox) - height / 2.0,
                    width,
                    height,
                ),
            ];
            candidates
                .into_iter()
                .find(|candidate| annotation_candidate_is_clear(*candidate, &component_boxes))
                .map(|candidate| (id.clone(), candidate))
        })
        .collect::<Vec<_>>();
    for (id, bbox) in updates {
        set_object_bbox(plan, &id, bbox);
    }
}

fn connector_matches_pair(from_id: &str, to_id: &str, left_id: &str, right_id: &str) -> bool {
    (from_id == left_id && to_id == right_id) || (from_id == right_id && to_id == left_id)
}

fn two_stage_head_to_output_route(head: [f64; 4], output: [f64; 4]) -> Vec<[f64; 2]> {
    let x = center_x(output).clamp(head[0] + 0.015, head[2] - 0.015);
    let start_y = if center_y(head) > center_y(output) {
        head[1]
    } else {
        head[3]
    };
    let end_y = if center_y(head) > center_y(output) {
        output[3]
    } else {
        output[1]
    };
    vec![[x, start_y], [x, end_y]]
}

fn two_stage_task_feedback_to_student_route(
    task_loss: [f64; 4],
    student_encoder: [f64; 4],
) -> Vec<[f64; 2]> {
    let min_rail_x = student_encoder[2] + 0.035;
    let max_rail_x = task_loss[0] - 0.020;
    let rail_x = if min_rail_x <= max_rail_x {
        (student_encoder[2] + 0.055).clamp(min_rail_x, max_rail_x)
    } else {
        (student_encoder[2] + task_loss[0]) / 2.0
    };
    remove_redundant_collinear_points(&[
        [task_loss[0], center_y(task_loss)],
        [rail_x, center_y(task_loss)],
        [rail_x, student_encoder[1]],
        [student_encoder[2], student_encoder[1]],
    ])
}

fn two_stage_residual_to_student_encoder_route(
    residual: [f64; 4],
    student_encoder: [f64; 4],
) -> Vec<[f64; 2]> {
    let rail_x = student_encoder[2] + 0.025;
    let y = center_y(residual);
    remove_redundant_collinear_points(&[
        [residual[0], y],
        [rail_x, y],
        [rail_x, student_encoder[1]],
        [student_encoder[2], student_encoder[1]],
    ])
}

fn two_stage_student_residual_label_bbox(student_head: [f64; 4], residual: [f64; 4]) -> [f64; 4] {
    let width = 0.050;
    let height = 0.050;
    let x1 = ((student_head[2] + residual[0]) / 2.0 - width / 2.0).clamp(0.04, 0.96 - width);
    let y1 = (student_head[1] - 0.060).clamp(0.04, 0.96 - height);
    [x1, y1, x1 + width, y1 + height]
}

fn remove_detached_two_stage_objective_labels(plan: &mut DrawPlan) {
    for object in &mut plan.objects {
        let DrawObject::Connector {
            points,
            from,
            to,
            label,
            ..
        } = object
        else {
            continue;
        };
        let Some(current_label) = label.as_ref() else {
            continue;
        };
        let phrase = normalized_annotation_phrase(&current_label.text);
        let objective_label = phrase == "y"
            || phrase == "ŷ"
            || phrase.contains("h t")
            || phrase.contains("h s")
            || phrase.contains("res")
            || phrase.contains("task")
            || phrase.contains("loss")
            || phrase.contains("r h");
        if !objective_label {
            continue;
        }
        let endpoint_identity = format!(
            "{} {}",
            from.as_deref().unwrap_or("").to_lowercase(),
            to.as_deref().unwrap_or("").to_lowercase()
        );
        let objective_edge = endpoint_identity.contains("loss")
            || endpoint_identity.contains("residual")
            || endpoint_identity.contains("align");
        if !objective_edge {
            continue;
        }
        if phrase == "y"
            || phrase == "ŷ"
            || connector_label_distance_to_route(current_label.bbox, points) > 0.12
        {
            *label = None;
        }
    }
}

fn move_bottom_edge_objectives_and_inference_notes_inside_safe_area(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut moved_box_ids = HashSet::new();
    let mut box_updates = Vec::new();
    for (id, info) in &box_map {
        if info.bbox[3] <= 0.965 || !is_task_loss_route_box(id, info) {
            continue;
        }
        let shift = 0.94 - info.bbox[3];
        box_updates.push((id.clone(), clamp_shifted_box(info.bbox, 0.0, shift)));
    }
    for (id, bbox) in box_updates {
        set_box_bbox(plan, &id, bbox);
        moved_box_ids.insert(id);
    }
    if !moved_box_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &moved_box_ids);
        reroute_connectors_touching_box_ids_orthogonally(plan, &moved_box_ids);
    }

    let component_boxes = current_box_map(plan);
    let student_anchor = student_anchor_for_inference_notes(plan)
        .map(|(_, info)| info.bbox)
        .or_else(|| {
            box_map
                .iter()
                .find_map(|(id, info)| is_main_route_box(info).then(|| (id, info.bbox)))
                .map(|(_, bbox)| bbox)
        });
    let mut updates = Vec::new();
    for object in &plan.objects {
        let DrawObject::Text {
            id,
            bbox,
            text,
            style,
            ..
        } = object
        else {
            continue;
        };
        if bbox[3] <= 0.965
            || !style.to_lowercase().contains("annotation")
            || !annotation_label_is_inference_specific(text)
        {
            continue;
        }
        let width = box_width(*bbox).clamp(0.18, 0.24);
        let height = box_height(*bbox).clamp(0.055, 0.070);
        let mut candidates = Vec::new();
        if let Some(student) = student_anchor {
            candidates.push(box_from_top_left_inside(
                center_x(student) - width / 2.0,
                student[1] - height - 0.035,
                width,
                height,
            ));
            candidates.push(box_from_top_left_inside(
                student[2] + 0.035,
                center_y(student) - height / 2.0,
                width,
                height,
            ));
        }
        candidates.push(box_from_top_left_inside(
            bbox[0],
            0.94 - height,
            width,
            height,
        ));
        if let Some(candidate) = candidates
            .into_iter()
            .find(|candidate| annotation_candidate_is_clear(*candidate, &component_boxes))
        {
            updates.push((id.clone(), candidate));
        }
    }
    for (id, bbox) in updates {
        set_object_bbox(plan, &id, bbox);
    }
}

fn compact_projectionless_teacher_student_stage_boxes(
    plan: &mut DrawPlan,
    encoder_pair: &MultistageTeacherStudentPair,
    latent_pair: &MultistageTeacherStudentPair,
    student_head_id: &str,
) -> HashSet<String> {
    let box_map = current_box_route_info_map(plan);
    let (
        Some(teacher_encoder),
        Some(teacher_latent),
        Some(student_encoder),
        Some(student_latent),
        Some(student_head),
    ) = (
        box_map.get(&encoder_pair.teacher_id),
        box_map.get(&latent_pair.teacher_id),
        box_map.get(&encoder_pair.student_id),
        box_map.get(&latent_pair.student_id),
        box_map.get(student_head_id),
    )
    else {
        return HashSet::new();
    };

    let mut candidate_updates = Vec::new();
    if let Some((teacher_encoder_box, teacher_latent_box)) =
        compact_projectionless_teacher_stack_candidates(
            teacher_encoder.bbox,
            teacher_latent.bbox,
            student_encoder.bbox,
            student_latent.bbox,
        )
    {
        candidate_updates.push((encoder_pair.teacher_id.as_str(), teacher_encoder_box));
        candidate_updates.push((latent_pair.teacher_id.as_str(), teacher_latent_box));
    }

    let (student_encoder_box, student_latent_box, student_head_box) =
        compact_projectionless_student_stage_candidates(
            student_encoder.bbox,
            student_latent.bbox,
            student_head.bbox,
        );
    candidate_updates.push((encoder_pair.student_id.as_str(), student_encoder_box));
    candidate_updates.push((latent_pair.student_id.as_str(), student_latent_box));
    candidate_updates.push((student_head_id, student_head_box));

    let ignored_ids = HashSet::from([
        encoder_pair.teacher_id.clone(),
        latent_pair.teacher_id.clone(),
        encoder_pair.student_id.clone(),
        latent_pair.student_id.clone(),
        student_head_id.to_string(),
    ]);

    if !candidate_updates.iter().all(|(id, bbox)| {
        let Some(current) = box_map.get(*id) else {
            return false;
        };
        boxes_nearly_equal(*bbox, current.bbox)
            || route_box_candidate_is_clear_except(id, *bbox, &box_map, &ignored_ids)
    }) {
        return HashSet::new();
    }

    let mut moved_ids = HashSet::new();
    for (id, bbox) in candidate_updates {
        let Some(current) = box_map.get(id) else {
            continue;
        };
        if boxes_nearly_equal(bbox, current.bbox) {
            continue;
        }
        set_box_bbox(plan, id, bbox);
        moved_ids.insert(id.to_string());
    }
    moved_ids
}

fn compact_projectionless_teacher_stack_candidates(
    teacher_encoder: [f64; 4],
    teacher_latent: [f64; 4],
    student_encoder: [f64; 4],
    student_latent: [f64; 4],
) -> Option<([f64; 4], [f64; 4])> {
    let width = box_width(teacher_encoder)
        .min(box_width(teacher_latent))
        .clamp(0.24, 0.32);
    let height = box_height(teacher_encoder)
        .min(box_height(teacher_latent))
        .clamp(0.13, 0.16);
    let gap = 0.060;
    let student_top = student_encoder[1].min(student_latent[1]);
    let total_height = height * 2.0 + gap;
    if student_top <= 0.06 + total_height + gap {
        return None;
    }
    let max_top = student_top - gap - total_height;
    let top = teacher_encoder[1].min(max_top).max(0.06);
    let center_x = ((center_x(teacher_encoder) + center_x(teacher_latent)) / 2.0)
        .clamp(0.02 + width / 2.0, 0.98 - width / 2.0);
    let x1 = center_x - width / 2.0;
    let teacher_encoder_box = box_from_top_left_inside(x1, top, width, height);
    let teacher_latent_box =
        box_from_top_left_inside(x1, teacher_encoder_box[3] + gap, width, height);
    Some((teacher_encoder_box, teacher_latent_box))
}

fn compact_projectionless_student_stage_candidates(
    student_encoder: [f64; 4],
    student_latent: [f64; 4],
    student_head: [f64; 4],
) -> ([f64; 4], [f64; 4], [f64; 4]) {
    let encoder_height = box_height(student_encoder).clamp(0.10, 0.115);
    let latent_height = box_height(student_latent).clamp(0.09, 0.115);
    let head_height = box_height(student_head).clamp(0.10, 0.115);
    let encoder_width = box_width(student_encoder).clamp(0.14, 0.18);
    let latent_width = box_width(student_latent).clamp(0.12, 0.18);
    let head_width = box_width(student_head).clamp(0.13, 0.17);

    let encoder_box = box_from_top_left_inside(
        center_x(student_encoder) - encoder_width / 2.0,
        center_y(student_encoder) - encoder_height / 2.0,
        encoder_width,
        encoder_height,
    );
    let latent_box = box_from_top_left_inside(
        center_x(student_latent) - latent_width / 2.0,
        center_y(student_latent) - latent_height / 2.0,
        latent_width,
        latent_height,
    );
    let mut head_box = box_from_top_left_inside(
        center_x(student_head) - head_width / 2.0,
        center_y(student_head) - head_height / 2.0,
        head_width,
        head_height,
    );
    if vertical_separation(encoder_box, head_box) < 0.060 {
        head_box =
            box_from_top_left_inside(head_box[0], encoder_box[3] + 0.060, head_width, head_height);
    }
    (encoder_box, latent_box, head_box)
}

fn connected_student_head_after_latent(
    plan: &DrawPlan,
    latent_id: &str,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<String> {
    plan.objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Connector { from, to, .. } = object else {
                return None;
            };
            let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
                return None;
            };
            let candidate_id = if from_id == latent_id {
                to_id
            } else if to_id == latent_id {
                from_id
            } else {
                return None;
            };
            let candidate = box_map.get(candidate_id)?;
            (is_main_route_box(candidate) && is_student_chain_head(candidate_id, candidate))
                .then(|| (candidate_id.to_string(), center_y(candidate.bbox)))
        })
        .min_by(|left, right| left.1.total_cmp(&right.1))
        .map(|(id, _)| id)
}

fn connected_output_after_student_head(
    plan: &DrawPlan,
    student_head_id: &str,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<String> {
    plan.objects.iter().find_map(|object| {
        let DrawObject::Connector { from, to, .. } = object else {
            return None;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            return None;
        };
        let candidate_id = if from_id == student_head_id {
            to_id
        } else if to_id == student_head_id {
            from_id
        } else {
            return None;
        };
        let candidate = box_map.get(candidate_id)?;
        (is_output_route_box(candidate_id, candidate)
            && !is_task_loss_route_box(candidate_id, candidate))
        .then(|| candidate_id.to_string())
    })
}

fn connected_task_loss_for_source(
    plan: &DrawPlan,
    source_id: &str,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<String> {
    plan.objects.iter().find_map(|object| {
        let DrawObject::Connector { from, to, .. } = object else {
            return None;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            return None;
        };
        let candidate_id = if from_id == source_id {
            to_id
        } else if to_id == source_id {
            from_id
        } else {
            return None;
        };
        let candidate = box_map.get(candidate_id)?;
        is_task_loss_route_box(candidate_id, candidate).then(|| candidate_id.to_string())
    })
}

fn connected_residual_hubs_for_latent_pair(
    plan: &DrawPlan,
    teacher_latent_id: &str,
    student_latent_id: &str,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Vec<String> {
    let mut scores: HashMap<String, usize> = HashMap::new();
    for object in &plan.objects {
        let DrawObject::Connector { from, to, .. } = object else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let other_id = if from_id == teacher_latent_id || from_id == student_latent_id {
            to_id
        } else if to_id == teacher_latent_id || to_id == student_latent_id {
            from_id
        } else {
            continue;
        };
        let Some(other) = box_map.get(other_id) else {
            continue;
        };
        if is_residual_or_supervision_hub_box(other_id, other) {
            *scores.entry(other_id.to_string()).or_insert(0) += 1;
        }
    }
    let mut residual_ids = scores
        .into_iter()
        .filter(|(_, score)| *score >= 1)
        .map(|(id, score)| (id, score))
        .collect::<Vec<_>>();
    residual_ids.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    residual_ids.into_iter().map(|(id, _)| id).collect()
}

fn projectionless_residual_side_candidate(
    residual: [f64; 4],
    teacher_latent: [f64; 4],
    student_latent: [f64; 4],
) -> [f64; 4] {
    let width = box_width(residual).clamp(0.13, 0.16);
    let height = box_height(residual).clamp(0.095, 0.12);
    let x1 = teacher_latent[2].max(student_latent[2]) + 0.055;
    let y1 = (center_y(teacher_latent) + center_y(student_latent)) / 2.0 - height / 2.0;
    box_from_top_left_inside(x1, y1, width, height)
}

fn projectionless_task_loss_side_candidate(
    loss_id: &str,
    task_loss: [f64; 4],
    student_head: [f64; 4],
    output: Option<[f64; 4]>,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<[f64; 4]> {
    let width = box_width(task_loss).clamp(0.13, 0.16);
    let height = box_height(task_loss).clamp(0.10, 0.12);
    let mut candidates = Vec::new();
    let branch_right = box_map
        .iter()
        .filter(|(id, info)| *id != loss_id && is_main_route_box(info))
        .map(|(_, info)| info.bbox[2])
        .fold(student_head[2], f64::max);
    if let Some(output) = output {
        push_unique_box(
            &mut candidates,
            box_from_top_left_inside(
                branch_right + 0.045,
                center_y(output) - height / 2.0,
                width,
                height,
            ),
        );
        push_unique_box(
            &mut candidates,
            box_from_top_left_inside(
                output[2] + 0.050,
                center_y(output) - height / 2.0,
                width,
                height,
            ),
        );
        push_unique_box(
            &mut candidates,
            box_from_top_left_inside(output[2] + 0.050, output[1] - 0.045 - height, width, height),
        );
    }
    push_unique_box(
        &mut candidates,
        box_from_top_left_inside(
            student_head[2] + 0.055,
            center_y(student_head) - height / 2.0,
            width,
            height,
        ),
    );
    candidates.into_iter().find(|candidate| {
        candidate[0] >= student_head[2] + 0.035
            && objective_hub_candidate_is_clear(loss_id, *candidate, box_map)
    })
}

fn reroute_projectionless_multistage_objective_connectors(
    plan: &mut DrawPlan,
    latent_pair: &MultistageTeacherStudentPair,
    student_head_id: &str,
    output_id: Option<&str>,
) {
    let box_map = current_box_route_info_map(plan);
    for object in &mut plan.objects {
        let DrawObject::Connector {
            points, from, to, ..
        } = object
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(from_box), Some(to_box)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        let touches_latent_pair = from_id == latent_pair.teacher_id
            || from_id == latent_pair.student_id
            || to_id == latent_pair.teacher_id
            || to_id == latent_pair.student_id;
        if touches_latent_pair
            && (is_residual_or_supervision_hub_box(from_id, from_box)
                || is_residual_or_supervision_hub_box(to_id, to_box))
        {
            *points = orthogonal_connector_points_between_boxes(from_box.bbox, to_box.bbox);
            continue;
        }
        if from_id == student_head_id && is_task_loss_route_box(to_id, to_box) {
            let output = output_id.and_then(|output_id| box_map.get(output_id));
            *points = projectionless_student_head_to_task_loss_route(
                from_box.bbox,
                to_box.bbox,
                output.map(|info| info.bbox),
            );
        }
    }
}

fn projectionless_student_head_to_task_loss_route(
    student_head: [f64; 4],
    task_loss: [f64; 4],
    output: Option<[f64; 4]>,
) -> Vec<[f64; 2]> {
    if let Some(output) = output {
        if student_head[2] <= output[0] && output[2] <= task_loss[0] {
            let start = [student_head[2], center_y(student_head)];
            let end = [task_loss[0], center_y(task_loss)];
            let rail_x = (output[0] - 0.025)
                .max(student_head[2] + 0.020)
                .min(output[0] - 0.010);
            let rail_y = (output[1] - 0.035).clamp(0.03, 0.97);
            return remove_redundant_collinear_points(&[
                start,
                [rail_x, start[1]],
                [rail_x, rail_y],
                [end[0], rail_y],
                end,
            ]);
        }
    }
    local_student_task_loss_route(student_head, task_loss)
}

fn balance_multistage_teacher_student_stage_sizes(
    plan: &mut DrawPlan,
    pairs: &[MultistageTeacherStudentPair],
    branch_ids: &HashSet<String>,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> HashSet<String> {
    let mut moved_ids = HashSet::new();
    for pair in pairs {
        if pair.stage == MultistageTeacherStudentStage::Output {
            continue;
        }
        let teacher_height = box_height(pair.teacher_bbox);
        let student_height = box_height(pair.student_bbox);
        if (teacher_height - student_height).abs() < 0.065 {
            continue;
        }

        let target_height = teacher_height.min(student_height).clamp(0.11, 0.19);
        let mut teacher_candidate = pair.teacher_bbox;
        let mut student_candidate = pair.student_bbox;
        if teacher_height > target_height + 0.025 {
            teacher_candidate = box_with_centered_height_inside(pair.teacher_bbox, target_height);
        }
        if student_height > target_height + 0.025 {
            student_candidate = box_with_centered_height_inside(pair.student_bbox, target_height);
        }
        if vertical_separation(teacher_candidate, student_candidate) < 0.055 {
            let missing = 0.055 - vertical_separation(teacher_candidate, student_candidate);
            if center_y(teacher_candidate) <= center_y(student_candidate) {
                student_candidate = clamp_shifted_box(student_candidate, 0.0, missing);
            } else {
                teacher_candidate = clamp_shifted_box(teacher_candidate, 0.0, missing);
            }
        }

        let mut ignored_ids = branch_ids.clone();
        ignored_ids.insert(pair.teacher_id.clone());
        ignored_ids.insert(pair.student_id.clone());
        if !boxes_nearly_equal(teacher_candidate, pair.teacher_bbox)
            && route_box_candidate_is_clear_except(
                &pair.teacher_id,
                teacher_candidate,
                box_map,
                &ignored_ids,
            )
        {
            set_box_bbox(plan, &pair.teacher_id, teacher_candidate);
            moved_ids.insert(pair.teacher_id.clone());
        }
        if !boxes_nearly_equal(student_candidate, pair.student_bbox)
            && route_box_candidate_is_clear_except(
                &pair.student_id,
                student_candidate,
                box_map,
                &ignored_ids,
            )
        {
            set_box_bbox(plan, &pair.student_id, student_candidate);
            moved_ids.insert(pair.student_id.clone());
        }
    }
    moved_ids
}

fn repair_multistage_projector_encoder_overlap_layout(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let pairs = multistage_teacher_student_stage_pairs(&box_map);
    let Some(encoder_pair) = pairs
        .iter()
        .find(|pair| pair.stage == MultistageTeacherStudentStage::Encoder)
        .cloned()
    else {
        return;
    };
    let Some(projection_pair) = pairs
        .iter()
        .find(|pair| pair.stage == MultistageTeacherStudentStage::Projection)
        .cloned()
    else {
        return;
    };
    let output_pair = pairs
        .iter()
        .find(|pair| pair.stage == MultistageTeacherStudentStage::Output)
        .cloned();

    if !multistage_projector_encoder_layout_needs_repair(
        plan,
        &encoder_pair,
        &projection_pair,
        &box_map,
    ) {
        return;
    }

    let encoder_height = box_height(encoder_pair.teacher_bbox)
        .min(box_height(encoder_pair.student_bbox))
        .clamp(0.14, 0.18);
    let projection_height = box_height(projection_pair.teacher_bbox)
        .min(box_height(projection_pair.student_bbox))
        .clamp(0.12, 0.16);
    let gap = 0.055;
    let output_bottom = output_pair
        .as_ref()
        .map(|pair| pair.teacher_bbox[3].max(pair.student_bbox[3]))
        .unwrap_or(0.0);
    let total_stack_height = encoder_height + gap + projection_height;
    let min_encoder_bottom = output_bottom + 0.055 + total_stack_height;
    if min_encoder_bottom > 0.94 {
        return;
    }
    let encoder_bottom = encoder_pair
        .teacher_bbox
        .get(3)
        .copied()
        .unwrap_or(0.86)
        .max(encoder_pair.student_bbox[3])
        .clamp(min_encoder_bottom, 0.94);

    let student_boxes = compact_multistage_projector_encoder_stack_boxes(
        encoder_pair.student_bbox,
        projection_pair.student_bbox,
        output_pair.as_ref().map(|pair| pair.student_bbox),
        encoder_bottom,
        encoder_height,
        projection_height,
        gap,
    );
    let teacher_boxes = compact_multistage_projector_encoder_stack_boxes(
        encoder_pair.teacher_bbox,
        projection_pair.teacher_bbox,
        output_pair.as_ref().map(|pair| pair.teacher_bbox),
        encoder_bottom,
        encoder_height,
        projection_height,
        gap,
    );

    let candidate_updates = vec![
        (encoder_pair.student_id.as_str(), student_boxes.0),
        (projection_pair.student_id.as_str(), student_boxes.1),
        (encoder_pair.teacher_id.as_str(), teacher_boxes.0),
        (projection_pair.teacher_id.as_str(), teacher_boxes.1),
    ];
    let ignored_ids = HashSet::from([
        encoder_pair.student_id.clone(),
        projection_pair.student_id.clone(),
        encoder_pair.teacher_id.clone(),
        projection_pair.teacher_id.clone(),
    ]);

    if !candidate_updates.iter().all(|(id, bbox)| {
        let Some(current) = box_map.get(*id) else {
            return false;
        };
        boxes_nearly_equal(*bbox, current.bbox)
            || route_box_candidate_is_clear_except(id, *bbox, &box_map, &ignored_ids)
    }) {
        return;
    }

    let mut moved_ids = HashSet::new();
    for (id, bbox) in candidate_updates {
        let Some(current) = box_map.get(id) else {
            continue;
        };
        if boxes_nearly_equal(bbox, current.bbox) {
            continue;
        }
        set_box_bbox(plan, id, bbox);
        moved_ids.insert(id.to_string());
    }

    if !moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
        reroute_connectors_touching_box_ids_orthogonally(plan, &moved_ids);
    }
    move_multistage_inference_note_to_student_periphery(plan, &encoder_pair.student_id);
    reroute_multistage_input_to_teacher_locally(
        plan,
        &encoder_pair.teacher_id,
        &encoder_pair.student_id,
    );
}

fn multistage_projector_encoder_layout_needs_repair(
    plan: &DrawPlan,
    encoder_pair: &MultistageTeacherStudentPair,
    projection_pair: &MultistageTeacherStudentPair,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> bool {
    let teacher_projection_above_encoder = center_y(projection_pair.teacher_bbox)
        <= center_y(encoder_pair.teacher_bbox)
        || component_overlap_gate_fails(encoder_pair.teacher_bbox, projection_pair.teacher_bbox);
    let student_projection_above_encoder = center_y(projection_pair.student_bbox)
        <= center_y(encoder_pair.student_bbox)
        || component_overlap_gate_fails(encoder_pair.student_bbox, projection_pair.student_bbox);
    if !teacher_projection_above_encoder || !student_projection_above_encoder {
        return false;
    }

    [
        encoder_pair.teacher_bbox,
        encoder_pair.student_bbox,
        projection_pair.teacher_bbox,
        projection_pair.student_bbox,
    ]
    .into_iter()
    .any(|bbox| box_width(bbox) > 0.26 || box_height(bbox) > 0.22)
        || [
            (encoder_pair.teacher_bbox, projection_pair.teacher_bbox),
            (encoder_pair.student_bbox, projection_pair.student_bbox),
        ]
        .into_iter()
        .any(|(encoder, projection)| {
            component_overlap_gate_fails(encoder, projection)
                || vertical_separation(encoder, projection) < 0.045
        })
        || multistage_input_teacher_route_has_external_detour(
            plan,
            &encoder_pair.teacher_id,
            box_map,
        )
}

fn compact_multistage_projector_encoder_stack_boxes(
    encoder: [f64; 4],
    projection: [f64; 4],
    output: Option<[f64; 4]>,
    encoder_bottom: f64,
    encoder_height: f64,
    projection_height: f64,
    gap: f64,
) -> ([f64; 4], [f64; 4]) {
    let center_x = multistage_branch_center_x(encoder, projection, output);
    let encoder_width = box_width(encoder).clamp(0.18, 0.22);
    let projection_width = box_width(projection).clamp(0.18, 0.22);
    let encoder_box = box_from_top_left_inside(
        center_x - encoder_width / 2.0,
        encoder_bottom - encoder_height,
        encoder_width,
        encoder_height,
    );
    let projection_bottom = encoder_box[1] - gap;
    let projection_box = box_from_top_left_inside(
        center_x - projection_width / 2.0,
        projection_bottom - projection_height,
        projection_width,
        projection_height,
    );
    (encoder_box, projection_box)
}

fn multistage_branch_center_x(
    encoder: [f64; 4],
    projection: [f64; 4],
    output: Option<[f64; 4]>,
) -> f64 {
    let center = output
        .map(|output| (center_x(encoder) + center_x(projection) + center_x(output)) / 3.0)
        .unwrap_or_else(|| (center_x(encoder) + center_x(projection)) / 2.0);
    center.clamp(0.13, 0.87)
}

fn multistage_input_teacher_route_has_external_detour(
    plan: &DrawPlan,
    teacher_encoder_id: &str,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> bool {
    plan.objects.iter().any(|object| {
        let DrawObject::Connector {
            points, from, to, ..
        } = object
        else {
            return false;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            return false;
        };
        if to_id != teacher_encoder_id {
            return false;
        }
        let Some(from_box) = box_map.get(from_id) else {
            return false;
        };
        is_input_route_box(from_id, from_box)
            && (points.iter().any(|point| point[0] < 0.10)
                || points_to_box(points)[3]
                    > box_map
                        .get(teacher_encoder_id)
                        .map(|info| info.bbox[3] + 0.08)
                        .unwrap_or(0.95))
    })
}

fn move_multistage_inference_note_to_student_periphery(
    plan: &mut DrawPlan,
    student_encoder_id: &str,
) {
    let box_map = current_box_route_info_map(plan);
    let Some(student_encoder) = box_map.get(student_encoder_id) else {
        return;
    };
    let component_boxes = current_box_map(plan);
    let updates = plan
        .objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Text {
                id,
                bbox,
                text,
                style,
                ..
            } = object
            else {
                return None;
            };
            let haystack = format!(
                "{} {} {}",
                id.to_lowercase(),
                text.to_lowercase(),
                style.to_lowercase()
            );
            if !haystack.contains("inference") {
                return None;
            }
            if compact_inference_note_is_anchored_to_student_or_output(*bbox, &box_map)
                && bbox[1] >= student_encoder.bbox[3] + 0.030
            {
                return None;
            }
            let width = box_width(*bbox).clamp(0.14, 0.18);
            let height = box_height(*bbox).clamp(0.045, 0.050);
            let candidates = [
                box_from_top_left_inside(
                    center_x(student_encoder.bbox) - width / 2.0,
                    student_encoder.bbox[3] + 0.035,
                    width,
                    height,
                ),
                box_from_top_left_inside(
                    student_encoder.bbox[0] - width - 0.035,
                    center_y(student_encoder.bbox) - height / 2.0,
                    width,
                    height,
                ),
            ];
            candidates
                .into_iter()
                .find(|candidate| annotation_candidate_is_clear(*candidate, &component_boxes))
                .map(|candidate| (id.clone(), candidate))
        })
        .collect::<Vec<_>>();
    for (id, bbox) in updates {
        set_object_bbox(plan, &id, bbox);
    }
}

fn reroute_multistage_input_to_teacher_locally(
    plan: &mut DrawPlan,
    teacher_encoder_id: &str,
    student_encoder_id: &str,
) {
    let box_map = current_box_route_info_map(plan);
    let (Some(teacher_encoder), student_encoder) = (
        box_map.get(teacher_encoder_id),
        box_map.get(student_encoder_id).map(|info| info.bbox),
    ) else {
        return;
    };
    for object in &mut plan.objects {
        let DrawObject::Connector {
            points, from, to, ..
        } = object
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        if to_id != teacher_encoder_id {
            continue;
        }
        let Some(from_box) = box_map.get(from_id) else {
            continue;
        };
        if !is_input_route_box(from_id, from_box) {
            continue;
        }
        let start = [from_box.bbox[2].max(0.10), from_box.bbox[1].max(0.06)];
        let end = [
            teacher_encoder.bbox[0],
            (teacher_encoder.bbox[1] + 0.050).min(teacher_encoder.bbox[3] - 0.020),
        ];
        let preferred_detour_x = student_encoder
            .map(|student| student[2] + 0.045)
            .unwrap_or((start[0] + end[0]) / 2.0);
        let min_detour_x = start[0] + 0.060;
        let max_detour_x = end[0] - 0.040;
        let detour_x = if min_detour_x <= max_detour_x {
            preferred_detour_x.clamp(min_detour_x, max_detour_x)
        } else {
            (start[0] + end[0]) / 2.0
        };
        let detour_y = (start[1].min(teacher_encoder.bbox[1]) - 0.035).clamp(0.06, 0.94);
        *points = vec![start, [detour_x, detour_y], end];
    }
}

fn box_with_centered_height_inside(bbox: [f64; 4], height: f64) -> [f64; 4] {
    let height = height.clamp(0.04, 0.96);
    let y1 = (center_y(bbox) - height / 2.0).clamp(0.02, 0.98 - height);
    [bbox[0], y1, bbox[2], y1 + height]
}

fn move_multistage_residual_hubs_between_branch_rows(
    plan: &mut DrawPlan,
    pairs: &[MultistageTeacherStudentPair],
    branch_ids: &HashSet<String>,
) -> HashSet<String> {
    let box_map = current_box_route_info_map(plan);
    let Some(output_pair) = pairs
        .iter()
        .find(|pair| pair.stage == MultistageTeacherStudentStage::Output)
    else {
        return HashSet::new();
    };
    let mut moved_ids = HashSet::new();
    let residual_ids = box_map
        .iter()
        .filter_map(|(id, info)| is_residual_or_supervision_hub_box(id, info).then(|| id.clone()))
        .collect::<Vec<_>>();

    for residual_id in residual_ids {
        let Some(residual) = box_map.get(&residual_id) else {
            continue;
        };
        let Some((upper_branch, lower_branch)) =
            connected_multistage_residual_branches(plan, &residual_id, &box_map, output_pair)
        else {
            continue;
        };
        if residual_sits_between_multistage_branches(residual.bbox, upper_branch, lower_branch) {
            continue;
        }
        let Some(candidate) =
            multistage_residual_between_branch_candidate(residual.bbox, upper_branch, lower_branch)
        else {
            continue;
        };
        let mut ignored_ids = branch_ids.clone();
        ignored_ids.insert(residual_id.clone());
        let overlaps_branch = branch_ids.iter().any(|id| {
            box_map
                .get(id)
                .is_some_and(|branch| intersection_area(candidate, branch.bbox) > 0.0001)
        });
        if overlaps_branch
            || !route_box_candidate_is_clear_except(&residual_id, candidate, &box_map, &ignored_ids)
        {
            continue;
        }
        set_box_bbox(plan, &residual_id, candidate);
        moved_ids.insert(residual_id);
    }
    moved_ids
}

fn connected_multistage_residual_branches(
    plan: &DrawPlan,
    residual_id: &str,
    box_map: &HashMap<String, BoxRouteInfo>,
    output_pair: &MultistageTeacherStudentPair,
) -> Option<([f64; 4], [f64; 4])> {
    let mut teacher_branches = Vec::new();
    let mut student_branches = Vec::new();
    for object in &plan.objects {
        let DrawObject::Connector { from, to, .. } = object else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let other_id = if from_id == residual_id {
            to_id
        } else if to_id == residual_id {
            from_id
        } else {
            continue;
        };
        let Some(other) = box_map.get(other_id) else {
            continue;
        };
        if is_teacher_branch_box(other_id, other) {
            teacher_branches.push(other.bbox);
        } else if is_student_like_branch_box(other_id, other) || is_main_route_box(other) {
            student_branches.push(other.bbox);
        }
    }
    let upper = teacher_branches
        .into_iter()
        .min_by(|left, right| center_y(*left).total_cmp(&center_y(*right)))
        .unwrap_or(output_pair.teacher_bbox);
    let lower = student_branches
        .into_iter()
        .min_by(|left, right| center_y(*left).total_cmp(&center_y(*right)))
        .unwrap_or(output_pair.student_bbox);
    (center_y(upper) < center_y(lower)).then_some((upper, lower))
}

fn residual_sits_between_multistage_branches(
    residual: [f64; 4],
    upper_branch: [f64; 4],
    lower_branch: [f64; 4],
) -> bool {
    center_y(residual) > center_y(upper_branch)
        && center_y(residual) < center_y(lower_branch)
        && residual[1] >= upper_branch[3] + 0.015
        && residual[3] <= lower_branch[1] - 0.015
}

fn multistage_residual_between_branch_candidate(
    residual: [f64; 4],
    upper_branch: [f64; 4],
    lower_branch: [f64; 4],
) -> Option<[f64; 4]> {
    if lower_branch[1] <= upper_branch[3] + 0.09 {
        return None;
    }
    let width = box_width(residual).clamp(0.13, 0.17);
    let available_height = (lower_branch[1] - 0.025) - (upper_branch[3] + 0.025);
    let height = box_height(residual)
        .min(0.085)
        .min(available_height)
        .max(0.055);
    if height > available_height {
        return None;
    }
    let y1 = upper_branch[3] + 0.025 + (available_height - height) / 2.0;
    let preferred_x2 = upper_branch[0].min(0.98) - 0.055;
    let x1 = if preferred_x2 - width >= 0.02 {
        preferred_x2 - width
    } else {
        (center_x(upper_branch).min(center_x(lower_branch)) - width / 2.0).clamp(0.02, 0.98 - width)
    };
    Some(normalize_box([x1, y1, x1 + width, y1 + height]))
}

fn move_multistage_task_losses_below_student_outputs(
    plan: &mut DrawPlan,
    pairs: &[MultistageTeacherStudentPair],
) -> HashSet<String> {
    let box_map = current_box_route_info_map(plan);
    let Some(output_pair) = pairs
        .iter()
        .find(|pair| pair.stage == MultistageTeacherStudentStage::Output)
    else {
        return HashSet::new();
    };
    let Some(student_output) = box_map.get(&output_pair.student_id) else {
        return HashSet::new();
    };
    let mut moved_ids = HashSet::new();
    let mut updates = Vec::new();
    for object in &plan.objects {
        let DrawObject::Connector {
            from, to, points, ..
        } = object
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let loss_id = if from_id == output_pair.student_id {
            to_id
        } else if to_id == output_pair.student_id {
            from_id
        } else {
            continue;
        };
        let Some(task_loss) = box_map.get(loss_id) else {
            continue;
        };
        if !is_task_loss_route_box(loss_id, task_loss) {
            continue;
        }
        let max_segment = points
            .windows(2)
            .map(|window| segment_length((window[0], window[1])))
            .fold(0.0_f64, f64::max);
        if (vertical_separation(student_output.bbox, task_loss.bbox) >= 0.055
            || horizontal_separation(student_output.bbox, task_loss.bbox) >= 0.035)
            && max_segment >= 0.045
        {
            continue;
        }
        let Some(candidate) = multistage_task_loss_below_output_candidate(
            loss_id,
            task_loss.bbox,
            student_output.bbox,
            &box_map,
        ) else {
            continue;
        };
        updates.push((loss_id.to_string(), candidate));
    }

    for (id, bbox) in updates {
        set_box_bbox(plan, &id, bbox);
        moved_ids.insert(id);
    }
    moved_ids
}

fn multistage_task_loss_below_output_candidate(
    loss_id: &str,
    task_loss: [f64; 4],
    output: [f64; 4],
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<[f64; 4]> {
    let width = box_width(task_loss).clamp(0.10, 0.14);
    let height = box_height(task_loss).clamp(0.085, 0.11);
    let below = box_from_top_left_inside(
        center_x(output) - width / 2.0,
        output[3] + 0.055,
        width,
        height,
    );
    let side = box_from_top_left_inside(
        output[2] + 0.045,
        center_y(output) - height / 2.0,
        width,
        height,
    );
    [below, side]
        .into_iter()
        .find(|candidate| objective_hub_candidate_is_clear(loss_id, *candidate, box_map))
}

fn reroute_connectors_touching_box_ids_orthogonally(
    plan: &mut DrawPlan,
    touched_ids: &HashSet<String>,
) {
    let box_map = current_box_route_info_map(plan);
    for object in &mut plan.objects {
        let DrawObject::Connector {
            points, from, to, ..
        } = object
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        if !touched_ids.contains(from_id) && !touched_ids.contains(to_id) {
            continue;
        }
        let (Some(from_box), Some(to_box)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        *points = orthogonal_connector_points_between_boxes(from_box.bbox, to_box.bbox);
    }
}

fn move_task_loss_boxes_out_of_teacher_student_branch_corridors(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut updates = Vec::new();
    let mut planned_ids = HashSet::new();

    for object in &plan.objects {
        let DrawObject::Connector { from, to, .. } = object else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let Some((student_id, student, loss_id, task_loss)) =
            student_task_loss_pair(from_id, to_id, &box_map)
        else {
            continue;
        };
        if planned_ids.contains(loss_id) {
            continue;
        }
        let branch_conflict = box_map.iter().any(|(teacher_id, teacher)| {
            teacher_id != student_id
                && teacher_id != loss_id
                && is_teacher_branch_box(teacher_id, teacher)
                && task_loss_sits_between_teacher_student_branch_rows(
                    task_loss.bbox,
                    teacher.bbox,
                    student.bbox,
                )
        });
        if !branch_conflict {
            continue;
        }
        let Some(candidate) =
            task_loss_student_side_candidate(loss_id, task_loss.bbox, student.bbox, &box_map)
        else {
            continue;
        };
        updates.push((loss_id.to_string(), candidate));
        planned_ids.insert(loss_id.to_string());
    }

    if updates.is_empty() {
        return;
    }
    let mut moved_ids = HashSet::new();
    for (id, bbox) in updates {
        set_box_bbox(plan, &id, bbox);
        moved_ids.insert(id);
    }
    realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
}

fn student_task_loss_pair<'a>(
    from_id: &'a str,
    to_id: &'a str,
    box_map: &'a HashMap<String, BoxRouteInfo>,
) -> Option<(&'a str, &'a BoxRouteInfo, &'a str, &'a BoxRouteInfo)> {
    let from = box_map.get(from_id)?;
    let to = box_map.get(to_id)?;
    if is_main_route_box(from) && is_task_loss_route_box(to_id, to) {
        Some((from_id, from, to_id, to))
    } else if is_task_loss_route_box(from_id, from) && is_main_route_box(to) {
        Some((to_id, to, from_id, from))
    } else {
        None
    }
}

fn is_teacher_branch_box(id: &str, info: &BoxRouteInfo) -> bool {
    let text = route_box_text(id, info);
    text.contains("teacher") || text.contains("frozen")
}

fn task_loss_sits_between_teacher_student_branch_rows(
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
    axis_overlap_ratio(task_loss[0], task_loss[2], branch_span[0], branch_span[2]) > 0.10
}

fn task_loss_student_side_candidate(
    loss_id: &str,
    task_loss: [f64; 4],
    student: [f64; 4],
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<[f64; 4]> {
    let width = box_width(task_loss);
    let height = box_height(task_loss);
    let same_row_y = center_y(student) - height / 2.0;
    let below_y = student[3] + 0.045;
    let candidates = [
        box_from_top_left_inside(student[2] + 0.045, same_row_y, width, height),
        box_from_top_left_inside(center_x(student) - width / 2.0, below_y, width, height),
        box_from_top_left_inside(student[0] - 0.045 - width, same_row_y, width, height),
        box_from_top_left_inside(student[2] + 0.045, below_y, width, height),
    ];
    candidates.into_iter().find(|candidate| {
        (horizontal_separation(*candidate, student) >= 0.03
            || vertical_separation(*candidate, student) >= 0.055)
            && objective_hub_candidate_is_clear(loss_id, *candidate, box_map)
    })
}

fn separate_task_loss_boxes_from_main_modules(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut updates = Vec::new();
    let target_gap = 0.055;

    for object in &plan.objects {
        let DrawObject::Connector { from, to, .. } = object else {
            continue;
        };
        let (Some(source_id), Some(loss_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(source), Some(task_loss)) = (box_map.get(source_id), box_map.get(loss_id)) else {
            continue;
        };
        if !is_main_route_box(source)
            || is_output_or_head_like_source_for_task_loss(source_id, source)
            || !is_task_loss_route_box(loss_id, task_loss)
            || horizontal_overlap(source.bbox, task_loss.bbox) < 0.02
            || vertical_separation(source.bbox, task_loss.bbox) >= target_gap
        {
            continue;
        }
        let Some(candidate) = task_loss_main_gutter_candidate(
            loss_id,
            task_loss.bbox,
            source.bbox,
            target_gap,
            &box_map,
        ) else {
            continue;
        };
        updates.push((loss_id.to_string(), candidate));
    }

    if updates.is_empty() {
        return;
    }
    let mut moved_ids = HashSet::new();
    for (id, bbox) in updates {
        set_box_bbox(plan, &id, bbox);
        moved_ids.insert(id);
    }
    realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
}

fn pull_far_student_task_loss_boxes_near_output_path(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let output_targets_by_source = student_output_targets_by_source(plan, &box_map);
    let mut updates = Vec::new();

    for object in &plan.objects {
        let DrawObject::Connector {
            id,
            points,
            from,
            to,
            label,
            ..
        } = object
        else {
            continue;
        };
        let (Some(source_id), Some(loss_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        if updates.iter().any(
            |(existing_loss_id, _, _, _, _): &(String, String, [f64; 4], Vec<[f64; 2]>, bool)| {
                existing_loss_id == loss_id
            },
        ) {
            continue;
        }
        let (Some(source), Some(task_loss)) = (box_map.get(source_id), box_map.get(loss_id)) else {
            continue;
        };
        if !is_student_task_loss_source(source_id, source)
            || !is_task_loss_route_box(loss_id, task_loss)
            || !student_task_loss_route_is_far(points, source.bbox, task_loss.bbox)
        {
            continue;
        }
        let outputs = output_targets_by_source
            .get(source_id)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let Some((candidate, route)) = best_far_student_task_loss_candidate(
            loss_id,
            source_id,
            source.bbox,
            task_loss.bbox,
            points,
            outputs,
            &box_map,
            id,
            plan,
        ) else {
            continue;
        };
        let remove_label = label
            .as_ref()
            .is_some_and(|label| is_generic_prediction_target_loss_label(&label.text));
        updates.push((
            loss_id.to_string(),
            id.clone(),
            candidate,
            route,
            remove_label,
        ));
    }

    if updates.is_empty() {
        return;
    }

    let mut moved_ids = HashSet::new();
    for (loss_id, _, candidate, _, _) in &updates {
        set_box_bbox(plan, loss_id, *candidate);
        moved_ids.insert(loss_id.clone());
    }
    realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    for (_, connector_id, _, route, remove_label) in updates {
        if let Some(DrawObject::Connector { points, label, .. }) = plan
            .objects
            .iter_mut()
            .find(|object| draw_object_id(object) == connector_id)
        {
            *points = route;
            if remove_label {
                *label = None;
            }
        }
    }
}

fn pull_top_edge_task_losses_near_outputs(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut updates = Vec::new();
    let mut planned_loss_ids = HashSet::new();

    for object in &plan.objects {
        let DrawObject::Connector {
            id,
            points,
            from,
            to,
            ..
        } = object
        else {
            continue;
        };
        let (Some(source_id), Some(loss_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        if planned_loss_ids.contains(loss_id) {
            continue;
        }
        let (Some(source), Some(task_loss)) = (box_map.get(source_id), box_map.get(loss_id)) else {
            continue;
        };
        if !is_task_loss_route_box(loss_id, task_loss)
            || !(is_output_route_box(source_id, source)
                || is_student_task_loss_source(source_id, source))
            || !top_edge_task_loss_needs_pull(points, source.bbox, task_loss.bbox)
        {
            continue;
        }
        let Some((candidate, route)) = best_top_edge_task_loss_candidate(
            loss_id,
            source_id,
            source,
            task_loss.bbox,
            &box_map,
            id,
            plan,
        ) else {
            continue;
        };
        updates.push((loss_id.to_string(), id.clone(), candidate, route));
        planned_loss_ids.insert(loss_id.to_string());
    }

    if updates.is_empty() {
        return;
    }

    let mut moved_ids = HashSet::new();
    for (loss_id, _, candidate, _) in &updates {
        set_box_bbox(plan, loss_id, *candidate);
        moved_ids.insert(loss_id.clone());
    }
    realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    reroute_connectors_for_moved_objectives(plan, &moved_ids);
    for (_, connector_id, _, route) in updates {
        if let Some(DrawObject::Connector { points, .. }) = plan
            .objects
            .iter_mut()
            .find(|object| draw_object_id(object) == connector_id)
        {
            *points = route;
        }
    }
}

fn move_output_task_losses_out_of_branch_corridors(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut updates = Vec::new();

    for object in &plan.objects {
        let DrawObject::Connector { id, from, to, .. } = object else {
            continue;
        };
        let (Some(output_id), Some(loss_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(output), Some(task_loss)) = (box_map.get(output_id), box_map.get(loss_id)) else {
            continue;
        };
        if !is_output_route_box(output_id, output)
            || !is_task_loss_route_box(loss_id, task_loss)
            || !output_task_loss_sits_in_branch_corridor(output.bbox, task_loss.bbox)
        {
            continue;
        }
        let Some(candidate) =
            output_task_loss_periphery_candidate(loss_id, output.bbox, task_loss.bbox, &box_map)
        else {
            continue;
        };
        let route = orthogonal_connector_points_between_boxes(output.bbox, candidate);
        updates.push((loss_id.to_string(), id.clone(), candidate, route));
    }

    if updates.is_empty() {
        return;
    }
    let mut moved_ids = HashSet::new();
    for (loss_id, _, candidate, _) in &updates {
        set_box_bbox(plan, loss_id, *candidate);
        moved_ids.insert(loss_id.clone());
    }
    realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    for (_, connector_id, _, route) in updates {
        if let Some(DrawObject::Connector { points, .. }) = plan
            .objects
            .iter_mut()
            .find(|object| draw_object_id(object) == connector_id)
        {
            *points = route;
        }
    }
}

fn output_task_loss_sits_in_branch_corridor(output: [f64; 4], task_loss: [f64; 4]) -> bool {
    task_loss[1] > 0.24
        && task_loss[3] + 0.12 < output[1]
        && horizontal_separation(task_loss, output) <= 0.08
}

fn output_task_loss_periphery_candidate(
    loss_id: &str,
    output: [f64; 4],
    task_loss: [f64; 4],
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<[f64; 4]> {
    let width = box_width(task_loss).clamp(0.14, 0.16);
    let height = box_height(task_loss).clamp(0.09, 0.12);
    let centered_x = center_x(output) - width / 2.0;
    let candidates = [
        box_from_top_left_inside(centered_x, output[3] + 0.035, width, height),
        box_from_top_left_inside(
            output[0] - 0.035 - width,
            center_y(output) - height / 2.0,
            width,
            height,
        ),
        box_from_top_left_inside(output[0] - 0.035 - width, output[3] + 0.035, width, height),
    ];
    candidates
        .into_iter()
        .filter(|candidate| {
            vertical_separation(*candidate, output) >= 0.025
                || horizontal_separation(*candidate, output) >= 0.025
        })
        .find(|candidate| route_box_candidate_is_clear(loss_id, *candidate, box_map))
}

fn top_edge_task_loss_needs_pull(
    points: &[[f64; 2]],
    source_bbox: [f64; 4],
    loss_bbox: [f64; 4],
) -> bool {
    if horizontal_separation(source_bbox, loss_bbox) <= 0.04 {
        return false;
    }
    let route_box = points_to_box(points);
    loss_bbox[1] < 0.16 && center_y(source_bbox) > loss_bbox[3] + 0.14
        || box_height(route_box) > 0.30
        || vertical_separation(source_bbox, loss_bbox) > 0.26
}

fn best_top_edge_task_loss_candidate(
    loss_id: &str,
    source_id: &str,
    source: &BoxRouteInfo,
    loss_bbox: [f64; 4],
    box_map: &HashMap<String, BoxRouteInfo>,
    connector_id: &str,
    plan: &DrawPlan,
) -> Option<([f64; 4], Vec<[f64; 2]>)> {
    let width = box_width(loss_bbox).clamp(0.10, 0.17);
    let height = box_height(loss_bbox).clamp(0.09, 0.13);
    top_edge_task_loss_candidates(source.bbox, loss_bbox, width, height)
        .into_iter()
        .filter_map(|candidate| {
            if candidate[1] < 0.18 || !objective_hub_candidate_is_clear(loss_id, candidate, box_map)
            {
                return None;
            }
            let route = if is_student_task_loss_source(source_id, source) {
                local_student_task_loss_route(source.bbox, candidate)
            } else {
                orthogonal_connector_points_between_boxes(source.bbox, candidate)
            };
            let mut candidate_box_map = box_map.clone();
            if let Some(loss_info) = candidate_box_map.get_mut(loss_id) {
                loss_info.bbox = candidate;
            }
            if connector_points_intersect_intermediate_boxes(
                route.as_slice(),
                source_id,
                loss_id,
                &candidate_box_map,
            ) {
                return None;
            }
            let conflict_penalty = if connector_route_conflicts_with_other_connectors(
                route.as_slice(),
                connector_id,
                plan,
            ) {
                2.0
            } else {
                0.0
            };
            Some((
                candidate,
                route,
                top_edge_task_loss_candidate_score(candidate, source.bbox, loss_bbox)
                    + conflict_penalty,
            ))
        })
        .min_by(|left, right| left.2.total_cmp(&right.2))
        .map(|(candidate, route, _)| (candidate, route))
}

fn top_edge_task_loss_candidates(
    source_bbox: [f64; 4],
    loss_bbox: [f64; 4],
    width: f64,
    height: f64,
) -> Vec<[f64; 4]> {
    let gap = 0.035;
    let same_row_y = center_y(source_bbox) - height / 2.0;
    let mut candidates = Vec::new();
    push_unique_box(
        &mut candidates,
        box_from_top_left_inside(source_bbox[0] - gap - width, same_row_y, width, height),
    );
    push_unique_box(
        &mut candidates,
        box_from_top_left_inside(source_bbox[2] + gap, same_row_y, width, height),
    );
    push_unique_box(
        &mut candidates,
        box_from_top_left_inside(
            center_x(source_bbox) - width / 2.0,
            source_bbox[3] + gap,
            width,
            height,
        ),
    );
    push_unique_box(
        &mut candidates,
        box_from_top_left_inside(
            center_x(source_bbox) - width / 2.0,
            source_bbox[1] - gap - height,
            width,
            height,
        ),
    );
    push_unique_box(
        &mut candidates,
        box_from_top_left_inside(
            loss_bbox[0],
            center_y(source_bbox) - height / 2.0,
            width,
            height,
        ),
    );
    candidates
}

fn top_edge_task_loss_candidate_score(
    candidate: [f64; 4],
    source_bbox: [f64; 4],
    original: [f64; 4],
) -> f64 {
    vertical_separation(candidate, source_bbox) * 1.4
        + horizontal_separation(candidate, source_bbox) * 0.8
        + edge_pressure(candidate) * 0.4
        + box_center_distance(candidate, original) * 0.15
}

fn student_output_targets_by_source(
    plan: &DrawPlan,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> HashMap<String, Vec<BoxRouteInfo>> {
    let mut output_targets_by_source: HashMap<String, Vec<BoxRouteInfo>> = HashMap::new();
    for object in &plan.objects {
        let DrawObject::Connector { from, to, .. } = object else {
            continue;
        };
        let (Some(source_id), Some(target_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(source), Some(target)) = (box_map.get(source_id), box_map.get(target_id)) else {
            continue;
        };
        if is_student_task_loss_source(source_id, source) && is_output_route_box(target_id, target)
        {
            output_targets_by_source
                .entry(source_id.to_string())
                .or_default()
                .push(target.clone());
        }
    }
    output_targets_by_source
}

fn is_student_task_loss_source(source_id: &str, source: &BoxRouteInfo) -> bool {
    let text = route_box_text(source_id, source);
    is_main_route_box(source) && (text.contains("student") || text.contains("compact"))
}

fn student_task_loss_route_is_far(
    points: &[[f64; 2]],
    source: [f64; 4],
    task_loss: [f64; 4],
) -> bool {
    let route_box = points_to_box(points);
    horizontal_separation(source, task_loss) > 0.28
        || box_width(route_box) > 0.40
        || has_long_near_horizontal_segment(points, 0.30)
}

fn best_far_student_task_loss_candidate(
    loss_id: &str,
    source_id: &str,
    source_bbox: [f64; 4],
    loss_bbox: [f64; 4],
    current_points: &[[f64; 2]],
    outputs: &[BoxRouteInfo],
    box_map: &HashMap<String, BoxRouteInfo>,
    connector_id: &str,
    plan: &DrawPlan,
) -> Option<([f64; 4], Vec<[f64; 2]>)> {
    let width = box_width(loss_bbox).clamp(0.10, 0.16);
    let height = box_height(loss_bbox).clamp(0.08, 0.13);
    let current_gap = horizontal_separation(source_bbox, loss_bbox);
    let current_route_width = box_width(points_to_box(current_points));
    let mut candidates = Vec::new();

    for output in outputs {
        for candidate in
            task_loss_candidates_near_output(source_bbox, loss_bbox, output.bbox, width, height)
        {
            push_unique_box(&mut candidates, candidate);
        }
    }
    for candidate in task_loss_candidates_near_student_source(source_bbox, loss_bbox, width, height)
    {
        push_unique_box(&mut candidates, candidate);
    }

    candidates
        .into_iter()
        .filter_map(|candidate| {
            if !route_box_candidate_is_clear(loss_id, candidate, box_map) {
                return None;
            }
            if horizontal_separation(source_bbox, candidate) + 0.01 >= current_gap {
                return None;
            }
            let route = local_student_task_loss_route(source_bbox, candidate);
            if box_width(points_to_box(route.as_slice())) + 0.02 >= current_route_width {
                return None;
            }
            let mut candidate_box_map = box_map.clone();
            if let Some(loss_info) = candidate_box_map.get_mut(loss_id) {
                loss_info.bbox = candidate;
            }
            if connector_points_intersect_intermediate_boxes(
                route.as_slice(),
                source_id,
                loss_id,
                &candidate_box_map,
            ) || connector_route_conflicts_with_other_connectors(
                route.as_slice(),
                connector_id,
                plan,
            ) {
                return None;
            }
            Some((
                candidate,
                route,
                far_student_task_loss_candidate_score(candidate, source_bbox, outputs),
            ))
        })
        .min_by(|left, right| left.2.total_cmp(&right.2))
        .map(|(candidate, route, _)| (candidate, route))
}

fn task_loss_candidates_near_output(
    source_bbox: [f64; 4],
    loss_bbox: [f64; 4],
    output_bbox: [f64; 4],
    width: f64,
    height: f64,
) -> Vec<[f64; 4]> {
    let gap = 0.035;
    let preferred_x = (output_bbox[0] - width * 0.55).clamp(source_bbox[2] + 0.055, 0.98 - width);
    let left_of_output_x = output_bbox[0] - gap - width;
    let source_side_x = if center_x(source_bbox) <= center_x(output_bbox) {
        source_bbox[2] + 0.055
    } else {
        source_bbox[0] - gap - width
    };
    vec![
        box_from_top_left_inside(preferred_x, output_bbox[1] - gap - height, width, height),
        box_from_top_left_inside(preferred_x, output_bbox[3] + gap, width, height),
        box_from_top_left_inside(
            left_of_output_x,
            center_y(output_bbox) - height / 2.0,
            width,
            height,
        ),
        box_from_top_left_inside(
            source_side_x,
            center_y(source_bbox) - height / 2.0,
            width,
            height,
        ),
        box_from_top_left_inside(
            source_side_x,
            center_y(loss_bbox) - height / 2.0,
            width,
            height,
        ),
    ]
}

fn task_loss_candidates_near_student_source(
    source_bbox: [f64; 4],
    loss_bbox: [f64; 4],
    width: f64,
    height: f64,
) -> Vec<[f64; 4]> {
    let gap = 0.055;
    let right_x = source_bbox[2] + gap;
    let left_x = source_bbox[0] - gap - width;
    vec![
        box_from_top_left_inside(right_x, center_y(source_bbox) - height / 2.0, width, height),
        box_from_top_left_inside(right_x, source_bbox[1] - gap - height, width, height),
        box_from_top_left_inside(right_x, source_bbox[3] + gap, width, height),
        box_from_top_left_inside(left_x, center_y(loss_bbox) - height / 2.0, width, height),
    ]
}

fn far_student_task_loss_candidate_score(
    candidate: [f64; 4],
    source_bbox: [f64; 4],
    outputs: &[BoxRouteInfo],
) -> f64 {
    let mut score = horizontal_separation(source_bbox, candidate) * 1.8
        + vertical_separation(source_bbox, candidate) * 0.35
        + edge_pressure(candidate) * 0.2;
    if let Some(output) = outputs.iter().min_by(|left, right| {
        box_center_distance(candidate, left.bbox)
            .total_cmp(&box_center_distance(candidate, right.bbox))
    }) {
        score += horizontal_separation(candidate, output.bbox) * 0.8;
        score += vertical_separation(candidate, output.bbox) * 0.45;
    }
    score
}

fn has_long_near_horizontal_segment(points: &[[f64; 2]], min_len: f64) -> bool {
    points.windows(2).any(|window| {
        (window[0][1] - window[1][1]).abs() < 0.006 && (window[0][0] - window[1][0]).abs() > min_len
    })
}

fn task_loss_main_gutter_candidate(
    loss_id: &str,
    task_loss: [f64; 4],
    main: [f64; 4],
    target_gap: f64,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<[f64; 4]> {
    let width = box_width(task_loss);
    let height = box_height(task_loss);
    let same_x = task_loss[0].clamp(0.02, 0.98 - width);
    let below_y = main[3] + target_gap;
    let above_y = main[1] - target_gap - height;
    let y1_same_side = if center_y(task_loss) >= center_y(main) {
        below_y
    } else {
        above_y
    };
    let y1_other_side = if center_y(task_loss) >= center_y(main) {
        above_y
    } else {
        below_y
    };
    let side_y = center_y(task_loss) - height / 2.0;
    let candidates = [
        [same_x, y1_same_side, same_x + width, y1_same_side + height],
        [
            same_x,
            y1_other_side,
            same_x + width,
            y1_other_side + height,
        ],
        [
            main[2] + 0.045,
            side_y,
            main[2] + 0.045 + width,
            side_y + height,
        ],
        [
            main[0] - 0.045 - width,
            side_y,
            main[0] - 0.045,
            side_y + height,
        ],
    ];
    candidates
        .into_iter()
        .map(normalize_box)
        .filter(|candidate| candidate[0] >= 0.02 && candidate[1] >= 0.02)
        .filter(|candidate| candidate[2] <= 0.98 && candidate[3] <= 0.98)
        .filter(|candidate| {
            (vertical_separation(*candidate, main) >= target_gap
                || horizontal_separation(*candidate, main) >= 0.03)
                && route_box_candidate_is_clear(loss_id, *candidate, box_map)
        })
        .min_by(|left, right| {
            box_center_distance(*left, task_loss).total_cmp(&box_center_distance(*right, task_loss))
        })
}

fn is_output_or_head_like_source_for_task_loss(id: &str, info: &BoxRouteInfo) -> bool {
    let text = route_box_text(id, info);
    let id = id.to_lowercase();
    let text_lower = text.to_lowercase();
    let is_output_source = is_output_route_box(&id, info) || text_lower.contains("output");
    if is_output_source {
        return true;
    }
    if !text_lower.contains("head") {
        return false;
    }
    let is_student_head_like =
        id.contains("student") || text_lower.contains("student") || text_lower.contains("task");
    !is_student_head_like
}

fn repair_right_edge_output_collisions(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut moves: Vec<(String, [f64; 4])> = Vec::new();
    let mut moved_ids = HashSet::new();

    for object in &plan.objects {
        let DrawObject::Connector { from, to, .. } = object else {
            continue;
        };
        let (Some(source_id), Some(output_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(source), Some(output)) = (box_map.get(source_id), box_map.get(output_id)) else {
            continue;
        };
        if !is_output_route_box(output_id, output)
            || is_input_route_box(source_id, source)
            || center_x(output.bbox) <= center_x(source.bbox)
        {
            continue;
        }

        let same_row = vertical_separation(source.bbox, output.bbox) < 0.04;
        let too_short = same_row && horizontal_separation(source.bbox, output.bbox) < 0.04;
        let colliding = component_overlap_gate_fails(source.bbox, output.bbox);
        let near_right_edge = output.bbox[2] > 0.97;
        if !colliding && !(too_short && near_right_edge) {
            continue;
        }

        let output_width = compact_output_width(output_id, output);
        let output_height = box_height(output.bbox).clamp(0.08, 0.14);
        let output_x2 = 0.98;
        let output_x1 = output_x2 - output_width;
        let output_center_y =
            center_y(source.bbox).clamp(0.02 + output_height / 2.0, 0.98 - output_height / 2.0);
        let output_candidate = [
            output_x1,
            output_center_y - output_height / 2.0,
            output_x2,
            output_center_y + output_height / 2.0,
        ];

        let source_width = box_width(source.bbox);
        let max_source_x2 = output_x1 - 0.04;
        let source_candidate = if source.bbox[2] > max_source_x2 {
            let source_x2 = max_source_x2.max(0.02 + source_width);
            [
                source_x2 - source_width,
                source.bbox[1],
                source_x2,
                source.bbox[3],
            ]
        } else {
            source.bbox
        };

        if source_candidate[0] >= 0.02
            && route_box_candidate_is_clear(source_id, source_candidate, &box_map)
        {
            moves.push((source_id.to_string(), source_candidate));
        }
        moves.push((output_id.to_string(), output_candidate));
    }

    for (id, bbox) in moves {
        set_box_bbox(plan, &id, bbox);
        moved_ids.insert(id);
    }
    if !moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    }
}

fn separate_crowded_outputs_from_sources(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut moves = Vec::new();
    let mut moved_ids = HashSet::new();

    for object in &plan.objects {
        let DrawObject::Connector { from, to, .. } = object else {
            continue;
        };
        let (Some(source_id), Some(output_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(source), Some(output)) = (box_map.get(source_id), box_map.get(output_id)) else {
            continue;
        };
        if !is_output_route_box(output_id, output) || is_input_route_box(source_id, source) {
            continue;
        }
        if vertical_separation(source.bbox, output.bbox) >= 0.03
            || horizontal_overlap(source.bbox, output.bbox) < 0.02
        {
            continue;
        }
        let width = box_width(output.bbox);
        let height = box_height(output.bbox);
        let below_y = source.bbox[3] + 0.035;
        let above_y = source.bbox[1] - 0.035 - height;
        let output_below = center_y(output.bbox) >= center_y(source.bbox);
        let candidates = if output_below {
            [
                [
                    output.bbox[0],
                    below_y,
                    output.bbox[0] + width,
                    below_y + height,
                ],
                [
                    output.bbox[0],
                    above_y,
                    output.bbox[0] + width,
                    above_y + height,
                ],
            ]
        } else {
            [
                [
                    output.bbox[0],
                    above_y,
                    output.bbox[0] + width,
                    above_y + height,
                ],
                [
                    output.bbox[0],
                    below_y,
                    output.bbox[0] + width,
                    below_y + height,
                ],
            ]
        };
        if let Some(candidate) = candidates.into_iter().find(|candidate| {
            candidate[1] >= 0.02
                && candidate[3] <= 0.98
                && route_box_candidate_is_clear(output_id, *candidate, &box_map)
        }) {
            moves.push((output_id.to_string(), candidate));
        }
    }

    for (id, bbox) in moves {
        set_box_bbox(plan, &id, bbox);
        moved_ids.insert(id);
    }
    if !moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    }
}

fn separate_outputs_from_residual_supervision_hubs(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut moves = Vec::new();
    let target_gap = 0.060;

    for (output_id, output) in &box_map {
        if !is_output_route_box(output_id, output) || is_task_loss_route_box(output_id, output) {
            continue;
        }
        let Some((hub_id, hub)) = box_map
            .iter()
            .filter(|(hub_id, hub)| {
                *hub_id != output_id
                    && is_residual_or_supervision_hub_box(hub_id, hub)
                    && horizontal_overlap(output.bbox, hub.bbox) > 0.02
                    && vertical_separation(output.bbox, hub.bbox) > 0.0
                    && vertical_separation(output.bbox, hub.bbox) < target_gap
            })
            .min_by(|left, right| {
                vertical_separation(output.bbox, left.1.bbox)
                    .total_cmp(&vertical_separation(output.bbox, right.1.bbox))
            })
        else {
            continue;
        };

        let candidates = output_residual_hub_spacing_candidates(output.bbox, hub.bbox, target_gap);
        if let Some(candidate) = candidates.into_iter().find(|candidate| {
            route_box_candidate_is_clear(output_id, *candidate, &box_map)
                && vertical_separation(*candidate, hub.bbox) >= target_gap - 0.001
        }) {
            moves.push((output_id.clone(), candidate));
            continue;
        }

        let hub_candidates =
            residual_hub_output_spacing_candidates(hub.bbox, output.bbox, target_gap);
        if let Some(candidate) = hub_candidates.into_iter().find(|candidate| {
            objective_hub_candidate_is_clear(hub_id, *candidate, &box_map)
                && vertical_separation(*candidate, output.bbox) >= target_gap - 0.001
        }) {
            moves.push((hub_id.clone(), candidate));
        }
    }

    if moves.is_empty() {
        return;
    }
    let mut moved_ids = HashSet::new();
    for (id, bbox) in moves {
        set_box_bbox(plan, &id, bbox);
        moved_ids.insert(id);
    }
    reroute_connectors_touching_box_ids_orthogonally(plan, &moved_ids);
}

fn output_residual_hub_spacing_candidates(
    output_bbox: [f64; 4],
    hub_bbox: [f64; 4],
    target_gap: f64,
) -> Vec<[f64; 4]> {
    let output_bbox = normalize_box(output_bbox);
    let hub_bbox = normalize_box(hub_bbox);
    let width = box_width(output_bbox);
    let height = box_height(output_bbox);
    let mut candidates = Vec::new();
    if center_y(output_bbox) >= center_y(hub_bbox) {
        let y1 = hub_bbox[3] + target_gap;
        candidates.push([output_bbox[0], y1, output_bbox[0] + width, y1 + height]);
        let y2 = hub_bbox[1] - target_gap;
        candidates.push([output_bbox[0], y2 - height, output_bbox[0] + width, y2]);
    } else {
        let y2 = hub_bbox[1] - target_gap;
        candidates.push([output_bbox[0], y2 - height, output_bbox[0] + width, y2]);
        let y1 = hub_bbox[3] + target_gap;
        candidates.push([output_bbox[0], y1, output_bbox[0] + width, y1 + height]);
    }
    candidates
        .into_iter()
        .filter(|bbox| bbox[1] >= 0.02 && bbox[3] <= 0.98)
        .map(normalize_box)
        .collect()
}

fn residual_hub_output_spacing_candidates(
    hub_bbox: [f64; 4],
    output_bbox: [f64; 4],
    target_gap: f64,
) -> Vec<[f64; 4]> {
    let hub_bbox = normalize_box(hub_bbox);
    let output_bbox = normalize_box(output_bbox);
    let width = box_width(hub_bbox);
    let height = box_height(hub_bbox);
    let mut candidates = Vec::new();
    if center_y(hub_bbox) <= center_y(output_bbox) {
        let y2 = output_bbox[1] - target_gap;
        candidates.push([hub_bbox[0], y2 - height, hub_bbox[0] + width, y2]);
        let y1 = output_bbox[3] + target_gap;
        candidates.push([hub_bbox[0], y1, hub_bbox[0] + width, y1 + height]);
    } else {
        let y1 = output_bbox[3] + target_gap;
        candidates.push([hub_bbox[0], y1, hub_bbox[0] + width, y1 + height]);
        let y2 = output_bbox[1] - target_gap;
        candidates.push([hub_bbox[0], y2 - height, hub_bbox[0] + width, y2]);
    }
    candidates
        .into_iter()
        .filter(|bbox| bbox[1] >= 0.02 && bbox[3] <= 0.98)
        .map(normalize_box)
        .collect()
}

fn separate_sibling_task_loss_and_output_boxes(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut moves = Vec::new();
    let target_gap = 0.055;

    for (_source_id, task_loss_id, output_id) in task_loss_output_sibling_pairs(plan, &box_map) {
        let (Some(task_loss), Some(output)) = (box_map.get(&task_loss_id), box_map.get(&output_id))
        else {
            continue;
        };
        let (_, vertical_overlap) = intersection_dimensions(task_loss.bbox, output.bbox);
        if horizontal_separation(task_loss.bbox, output.bbox) >= target_gap
            || vertical_overlap < 0.02
        {
            continue;
        }

        let output_candidate = box_from_top_left_inside(
            task_loss.bbox[2] + target_gap,
            output.bbox[1],
            box_width(output.bbox),
            box_height(output.bbox),
        );
        if route_box_candidate_is_clear(&output_id, output_candidate, &box_map)
            && horizontal_separation(task_loss.bbox, output_candidate) >= target_gap - 0.001
        {
            moves.push((output_id, output_candidate));
            continue;
        }

        let loss_candidate = box_from_top_left_inside(
            output.bbox[0] - target_gap - box_width(task_loss.bbox),
            task_loss.bbox[1],
            box_width(task_loss.bbox),
            box_height(task_loss.bbox),
        );
        if route_box_candidate_is_clear(&task_loss_id, loss_candidate, &box_map)
            && horizontal_separation(loss_candidate, output.bbox) >= target_gap - 0.001
        {
            moves.push((task_loss_id, loss_candidate));
        }
    }

    if moves.is_empty() {
        return;
    }
    let mut moved_ids = HashSet::new();
    for (id, bbox) in moves {
        set_box_bbox(plan, &id, bbox);
        moved_ids.insert(id);
    }
    reroute_connectors_touching_box_ids_orthogonally(plan, &moved_ids);
}

fn task_loss_output_sibling_pairs(
    plan: &DrawPlan,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Vec<(String, String, String)> {
    let mut by_source: HashMap<String, (Vec<String>, Vec<String>)> = HashMap::new();
    for object in &plan.objects {
        let DrawObject::Connector { from, to, .. } = object else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let Some(to_box) = box_map.get(to_id) else {
            continue;
        };
        if is_task_loss_route_box(to_id, to_box) {
            by_source
                .entry(from_id.to_string())
                .or_default()
                .0
                .push(to_id.to_string());
        } else if is_output_route_box(to_id, to_box) {
            by_source
                .entry(from_id.to_string())
                .or_default()
                .1
                .push(to_id.to_string());
        }
    }

    let mut pairs = Vec::new();
    for (source_id, (loss_ids, output_ids)) in by_source {
        for loss_id in &loss_ids {
            for output_id in &output_ids {
                pairs.push((source_id.clone(), loss_id.clone(), output_id.clone()));
            }
        }
    }
    pairs
}

fn separate_right_side_task_loss_output_and_inference_corridor(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let pairs = right_side_task_loss_output_pairs(plan, &box_map);
    if pairs.is_empty() {
        return;
    }

    let mut moved_ids = HashSet::new();
    for (student_id, task_loss_id, output_id) in &pairs {
        let (Some(student), Some(task_loss), Some(output)) = (
            box_map.get(student_id),
            box_map.get(task_loss_id),
            box_map.get(output_id),
        ) else {
            continue;
        };
        if !right_side_task_loss_output_pair_is_crowded(task_loss.bbox, output.bbox) {
            continue;
        }
        if let Some(candidate) = right_side_task_loss_output_candidate(
            task_loss_id,
            task_loss.bbox,
            output.bbox,
            student.bbox,
            &box_map,
        ) {
            set_box_bbox(plan, task_loss_id, candidate);
            moved_ids.insert(task_loss_id.clone());
        }
    }

    if !moved_ids.is_empty() {
        reroute_connectors_touching_box_ids_orthogonally(plan, &moved_ids);
    }

    move_inference_annotations_out_of_right_side_student_corridor(plan, &pairs);
}

fn right_side_task_loss_output_pairs(
    plan: &DrawPlan,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Vec<(String, String, String)> {
    let mut pairs = Vec::new();
    let mut seen = HashSet::new();
    for object in &plan.objects {
        let DrawObject::Connector { from, to, .. } = object else {
            continue;
        };
        let (Some(student_id), Some(output_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(student), Some(output)) = (box_map.get(student_id), box_map.get(output_id))
        else {
            continue;
        };
        if !is_main_route_box(student)
            || !is_output_route_box(output_id, output)
            || center_x(output.bbox) <= student.bbox[2]
        {
            continue;
        }
        for task_loss_id in task_loss_ids_connected_with_main(plan, box_map, student_id) {
            let Some(task_loss) = box_map.get(&task_loss_id) else {
                continue;
            };
            if center_x(task_loss.bbox) <= student.bbox[2] {
                continue;
            }
            let key = (
                student_id.to_string(),
                task_loss_id.clone(),
                output_id.to_string(),
            );
            if seen.insert(key.clone()) {
                pairs.push(key);
            }
        }
    }
    pairs
}

fn task_loss_ids_connected_with_main(
    plan: &DrawPlan,
    box_map: &HashMap<String, BoxRouteInfo>,
    main_id: &str,
) -> Vec<String> {
    plan.objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Connector { from, to, .. } = object else {
                return None;
            };
            let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
                return None;
            };
            if from_id == main_id {
                let to_box = box_map.get(to_id)?;
                is_task_loss_route_box(to_id, to_box).then(|| to_id.to_string())
            } else if to_id == main_id {
                let from_box = box_map.get(from_id)?;
                is_task_loss_route_box(from_id, from_box).then(|| from_id.to_string())
            } else {
                None
            }
        })
        .collect()
}

fn right_side_task_loss_output_pair_is_crowded(task_loss: [f64; 4], output: [f64; 4]) -> bool {
    let horizontal_gap = horizontal_separation(task_loss, output);
    let vertical_gap = vertical_separation(task_loss, output);
    let (_, vertical_overlap) = intersection_dimensions(task_loss, output);
    horizontal_gap < 0.050 && (vertical_gap < 0.055 || vertical_overlap > 0.015)
}

fn right_side_task_loss_output_candidate(
    task_loss_id: &str,
    task_loss: [f64; 4],
    output: [f64; 4],
    student: [f64; 4],
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<[f64; 4]> {
    let width = box_width(task_loss);
    let height = box_height(task_loss);
    let target_h_gap = 0.055;
    let target_v_gap = 0.035;
    let min_x = student[2] + target_h_gap;
    let max_x = 0.96 - width;
    let desired_x = output[0] - target_h_gap - width;
    let x1 = if min_x <= max_x {
        desired_x.clamp(min_x, max_x)
    } else {
        desired_x.clamp(0.02, (0.98 - width).max(0.02))
    };

    let mut candidates = Vec::new();
    let below_y = output[3] + target_v_gap;
    if below_y + height <= 0.94 {
        candidates.push([x1, below_y, x1 + width, below_y + height]);
    }
    let original_y = if height <= 0.92 {
        task_loss[1].clamp(0.02, 0.94 - height)
    } else {
        task_loss[1]
    };
    candidates.push([x1, original_y, x1 + width, original_y + height]);
    let above_y = output[1] - target_v_gap - height;
    if above_y >= 0.02 {
        candidates.push([x1, above_y, x1 + width, above_y + height]);
    }

    candidates
        .into_iter()
        .map(normalize_box)
        .filter(|candidate| {
            candidate[1] >= 0.02
                && candidate[3] <= 0.94
                && !right_side_task_loss_output_pair_is_crowded(*candidate, output)
                && objective_hub_candidate_is_clear(task_loss_id, *candidate, box_map)
        })
        .min_by(|left, right| {
            objective_hub_candidate_score(*left, task_loss)
                .total_cmp(&objective_hub_candidate_score(*right, task_loss))
        })
}

fn move_inference_annotations_out_of_right_side_student_corridor(
    plan: &mut DrawPlan,
    pairs: &[(String, String, String)],
) {
    let box_map = current_box_route_info_map(plan);
    let mut updates = Vec::new();
    let mut planned = HashSet::new();
    for (student_id, task_loss_id, output_id) in pairs {
        let (Some(student), Some(task_loss), Some(output)) = (
            box_map.get(student_id),
            box_map.get(task_loss_id),
            box_map.get(output_id),
        ) else {
            continue;
        };
        let cluster = union_box(task_loss.bbox, output.bbox);
        for object in &plan.objects {
            let DrawObject::Text {
                id,
                bbox,
                text,
                style,
                ..
            } = object
            else {
                continue;
            };
            if !planned.insert(id.clone()) {
                continue;
            }
            let style_text = style.to_lowercase();
            if !annotation_label_is_inference_specific(text)
                || !style_text.contains("annotation")
                || !inference_annotation_sits_in_student_underflow_corridor(*bbox, student.bbox)
            {
                continue;
            }
            let width = box_width(*bbox).clamp(0.18, 0.20);
            let height = box_height(*bbox).clamp(0.050, 0.065);
            if let Some(candidate) =
                right_side_inference_annotation_candidate(width, height, cluster, &box_map)
            {
                updates.push((id.clone(), candidate));
            }
        }
    }

    for (id, bbox) in updates {
        set_object_bbox(plan, &id, bbox);
    }
}

fn inference_annotation_sits_in_student_underflow_corridor(
    annotation: [f64; 4],
    student: [f64; 4],
) -> bool {
    annotation[1] >= student[3] + 0.025
        && annotation[1] <= student[3] + 0.20
        && annotation[0] < student[2] + 0.08
}

fn right_side_inference_annotation_candidate(
    width: f64,
    height: f64,
    cluster: [f64; 4],
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<[f64; 4]> {
    let right_x = (0.98 - width).max(0.02);
    let bottom_y = if height <= 0.92 {
        (0.94 - height).max(0.02)
    } else {
        0.02
    };
    let cluster_y = if height <= 0.92 {
        (cluster[3] + 0.035).clamp(0.02, 0.94 - height)
    } else {
        cluster[3] + 0.035
    };
    let cluster_x = if width <= 0.96 {
        cluster[2].clamp(0.02, 0.98 - width)
    } else {
        0.02
    };
    let candidates = [
        [right_x, bottom_y, right_x + width, bottom_y + height],
        [cluster_x, cluster_y, cluster_x + width, cluster_y + height],
    ];
    candidates.into_iter().map(normalize_box).find(|candidate| {
        box_map.values().all(|info| {
            intersection_area(*candidate, info.bbox) == 0.0
                && !component_crowding_gate_fails_normalized(*candidate, info.bbox)
        })
    })
}

#[derive(Clone, Debug)]
struct LeftTeacherObjectiveTopology {
    input_id: String,
    teacher_id: String,
    student_id: String,
    output_id: String,
    latent_id: String,
    objective_id: Option<String>,
}

fn repair_left_teacher_central_student_objective_topology(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let Some(topology) = left_teacher_central_student_objective_topology(plan, &box_map) else {
        return;
    };
    let (Some(teacher), Some(student)) = (
        box_map.get(&topology.teacher_id),
        box_map.get(&topology.student_id),
    ) else {
        return;
    };
    let Some(teacher_candidate) = left_teacher_peer_branch_candidate(
        &topology.teacher_id,
        teacher.bbox,
        student.bbox,
        &box_map,
    ) else {
        return;
    };

    ensure_simple_y_teacher_is_muted_dashed(plan, &topology.teacher_id);
    set_box_bbox(plan, &topology.teacher_id, teacher_candidate);

    reroute_left_teacher_objective_topology_edges(plan, &topology);
    move_left_teacher_topology_inference_annotation(plan, &topology);
}

fn left_teacher_central_student_objective_topology(
    plan: &DrawPlan,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<LeftTeacherObjectiveTopology> {
    for (student_id, student) in box_map {
        if !is_main_route_box(student) || !route_box_text(student_id, student).contains("student") {
            continue;
        }
        let Some(output_id) = output_connected_from_main(plan, box_map, student_id) else {
            continue;
        };
        let Some((teacher_id, _teacher)) = box_map.iter().find(|(teacher_id, teacher)| {
            is_teacher_or_context_route_box(teacher_id, teacher)
                && route_box_text(teacher_id, teacher).contains("teacher")
                && teacher.bbox[2] <= student.bbox[0] - 0.08
                && center_y(teacher.bbox) >= student.bbox[1] - 0.05
        }) else {
            continue;
        };
        let Some(input_id) = input_connected_to_main(plan, box_map, student_id) else {
            continue;
        };
        let Some(latent_id) =
            residual_or_latent_connected_from_teacher(plan, box_map, teacher_id.as_str())
        else {
            continue;
        };
        let objective_id = objective_connected_from_branch(plan, box_map, student_id)
            .or_else(|| objective_connected_from_branch(plan, box_map, teacher_id));
        return Some(LeftTeacherObjectiveTopology {
            input_id,
            teacher_id: teacher_id.clone(),
            student_id: student_id.clone(),
            output_id,
            latent_id,
            objective_id,
        });
    }
    None
}

fn output_connected_from_main(
    plan: &DrawPlan,
    box_map: &HashMap<String, BoxRouteInfo>,
    main_id: &str,
) -> Option<String> {
    plan.objects.iter().find_map(|object| {
        let DrawObject::Connector { from, to, .. } = object else {
            return None;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            return None;
        };
        if from_id != main_id {
            return None;
        }
        let to_box = box_map.get(to_id)?;
        is_output_route_box(to_id, to_box).then(|| to_id.to_string())
    })
}

fn input_connected_to_main(
    plan: &DrawPlan,
    box_map: &HashMap<String, BoxRouteInfo>,
    main_id: &str,
) -> Option<String> {
    plan.objects.iter().find_map(|object| {
        let DrawObject::Connector { from, to, .. } = object else {
            return None;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            return None;
        };
        if to_id != main_id {
            return None;
        }
        let from_box = box_map.get(from_id)?;
        is_input_route_box(from_id, from_box).then(|| from_id.to_string())
    })
}

fn residual_or_latent_connected_from_teacher(
    plan: &DrawPlan,
    box_map: &HashMap<String, BoxRouteInfo>,
    teacher_id: &str,
) -> Option<String> {
    plan.objects.iter().find_map(|object| {
        let DrawObject::Connector { from, to, .. } = object else {
            return None;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            return None;
        };
        if from_id != teacher_id {
            return None;
        }
        let to_box = box_map.get(to_id)?;
        is_residual_or_supervision_hub_box(to_id, to_box).then(|| to_id.to_string())
    })
}

fn objective_connected_from_branch(
    plan: &DrawPlan,
    box_map: &HashMap<String, BoxRouteInfo>,
    branch_id: &str,
) -> Option<String> {
    plan.objects.iter().find_map(|object| {
        let DrawObject::Connector { from, to, .. } = object else {
            return None;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            return None;
        };
        if from_id != branch_id {
            return None;
        }
        let to_box = box_map.get(to_id)?;
        (is_task_loss_route_box(to_id, to_box) || is_objective_route_box(to_id, to_box))
            .then(|| to_id.to_string())
    })
}

fn left_teacher_peer_branch_candidate(
    teacher_id: &str,
    teacher_bbox: [f64; 4],
    student_bbox: [f64; 4],
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<[f64; 4]> {
    let width = box_width(teacher_bbox)
        .max(box_width(student_bbox) * 0.95)
        .clamp(0.16, 0.24);
    let height = box_height(teacher_bbox).clamp(0.14, 0.20);
    let x1 = (center_x(student_bbox) - width / 2.0).clamp(0.04, 0.96 - width);
    let y1 = (student_bbox[1] - 0.055 - height).clamp(0.04, 0.94 - height);
    let candidate = [x1, y1, x1 + width, y1 + height];
    route_box_candidate_is_clear(teacher_id, candidate, box_map).then_some(candidate)
}

fn reroute_left_teacher_objective_topology_edges(
    plan: &mut DrawPlan,
    topology: &LeftTeacherObjectiveTopology,
) {
    let box_map = current_box_route_info_map(plan);
    let input = box_map.get(&topology.input_id).map(|info| info.bbox);
    let teacher = box_map.get(&topology.teacher_id).map(|info| info.bbox);
    let student = box_map.get(&topology.student_id).map(|info| info.bbox);
    let output = box_map.get(&topology.output_id).map(|info| info.bbox);
    let latent = box_map.get(&topology.latent_id).map(|info| info.bbox);
    let objective = topology
        .objective_id
        .as_ref()
        .and_then(|id| box_map.get(id).map(|info| info.bbox));

    for object in &mut plan.objects {
        let DrawObject::Connector {
            points, from, to, ..
        } = object
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        match (from_id, to_id) {
            (from_id, to_id) if from_id == topology.input_id && to_id == topology.student_id => {
                if let (Some(input), Some(student)) = (input, student) {
                    *points = input_to_student_low_l_route(input, student);
                }
            }
            (from_id, to_id) if from_id == topology.input_id && to_id == topology.teacher_id => {
                if let (Some(input), Some(teacher)) = (input, teacher) {
                    *points = orthogonal_connector_points_between_boxes(input, teacher);
                }
            }
            (from_id, to_id) if from_id == topology.student_id && to_id == topology.output_id => {
                if let (Some(student), Some(output)) = (student, output) {
                    if let Some(x) = common_vertical_x(student, output) {
                        *points = vertical_connector_points(student, output, x);
                    }
                }
            }
            (from_id, to_id) if from_id == topology.teacher_id && to_id == topology.latent_id => {
                if let (Some(teacher), Some(latent)) = (teacher, latent) {
                    *points = orthogonal_connector_points_between_boxes(teacher, latent);
                }
            }
            (from_id, to_id)
                if topology
                    .objective_id
                    .as_deref()
                    .is_some_and(|objective_id| {
                        from_id == topology.student_id && to_id == objective_id
                    }) =>
            {
                if let (Some(student), Some(objective)) = (student, objective) {
                    *points = orthogonal_connector_points_between_boxes(student, objective);
                }
            }
            (from_id, to_id)
                if topology
                    .objective_id
                    .as_deref()
                    .is_some_and(|objective_id| {
                        from_id == topology.teacher_id && to_id == objective_id
                    }) =>
            {
                if let (Some(teacher), Some(objective)) = (teacher, objective) {
                    *points = teacher_to_objective_outer_right_route(teacher, objective);
                }
            }
            _ => {}
        }
    }
}

fn input_to_student_low_l_route(input: [f64; 4], student: [f64; 4]) -> Vec<[f64; 2]> {
    let start = [input[2], center_y(input)];
    let end_y = center_y(student).clamp(student[1] + 0.035, student[3] - 0.035);
    let end = [student[0], end_y];
    remove_redundant_collinear_points(&[start, [start[0], end[1]], end])
}

fn teacher_to_objective_outer_right_route(teacher: [f64; 4], objective: [f64; 4]) -> Vec<[f64; 2]> {
    let start = [teacher[2], center_y(teacher)];
    let outer_x = (objective[2] + 0.035).min(0.96);
    let end = [objective[2], center_y(objective)];
    remove_redundant_collinear_points(&[start, [outer_x, start[1]], [outer_x, end[1]], end])
}

fn move_left_teacher_topology_inference_annotation(
    plan: &mut DrawPlan,
    topology: &LeftTeacherObjectiveTopology,
) {
    let box_map = current_box_route_info_map(plan);
    let Some(student) = box_map.get(&topology.student_id).map(|info| info.bbox) else {
        return;
    };
    let Some(output) = box_map.get(&topology.output_id).map(|info| info.bbox) else {
        return;
    };
    let cluster = topology
        .objective_id
        .as_ref()
        .and_then(|id| box_map.get(id).map(|info| union_box(output, info.bbox)))
        .unwrap_or(output);
    let mut updates = Vec::new();
    for object in &plan.objects {
        let DrawObject::Text {
            id,
            bbox,
            text,
            style,
            ..
        } = object
        else {
            continue;
        };
        if !style.to_lowercase().contains("annotation")
            || !annotation_label_is_inference_specific(text)
            || !inference_annotation_sits_in_student_underflow_corridor(*bbox, student)
        {
            continue;
        }
        let width = box_width(*bbox).clamp(0.18, 0.20);
        let height = box_height(*bbox).clamp(0.050, 0.065);
        if let Some(candidate) =
            right_side_inference_annotation_candidate(width, height, cluster, &box_map)
        {
            updates.push((id.clone(), candidate));
        }
    }
    for (id, bbox) in updates {
        set_object_bbox(plan, &id, bbox);
    }
}

fn separate_horizontally_crowded_connected_boxes(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut updates = Vec::new();

    for object in &plan.objects {
        let DrawObject::Connector { from, to, .. } = object else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(from_box), Some(to_box)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        let target_gap = connected_pair_horizontal_target_gap(from_id, from_box, to_id, to_box);
        if !connected_pair_needs_horizontal_gutter(from_id, from_box, to_id, to_box, target_gap) {
            continue;
        }
        let Some((moving_id, candidate)) =
            horizontal_gutter_candidate(from_id, from_box, to_id, to_box, target_gap, &box_map)
        else {
            continue;
        };
        updates.push((moving_id, candidate));
    }

    if updates.is_empty() {
        return;
    }
    let mut moved_ids = HashSet::new();
    for (id, bbox) in updates {
        set_box_bbox(plan, &id, bbox);
        moved_ids.insert(id);
    }
    realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
}

fn connected_pair_horizontal_target_gap(
    from_id: &str,
    from_box: &BoxRouteInfo,
    to_id: &str,
    to_box: &BoxRouteInfo,
) -> f64 {
    let head_to_output = (is_student_chain_head(from_id, from_box)
        && is_output_route_box(to_id, to_box))
        || (is_student_chain_head(to_id, to_box) && is_output_route_box(from_id, from_box));
    if head_to_output {
        0.045
    } else {
        0.035
    }
}

fn connected_pair_needs_horizontal_gutter(
    from_id: &str,
    from_box: &BoxRouteInfo,
    to_id: &str,
    to_box: &BoxRouteInfo,
    target_gap: f64,
) -> bool {
    if is_input_route_box(from_id, from_box)
        || is_input_route_box(to_id, to_box)
        || is_note_like_connected_box(from_id, from_box)
        || is_note_like_connected_box(to_id, to_box)
        || is_context_note_box(from_id, from_box)
        || is_context_note_box(to_id, to_box)
        || is_loss_or_objective_box(from_id, from_box)
        || is_loss_or_objective_box(to_id, to_box)
    {
        return false;
    }
    let gap = horizontal_separation(from_box.bbox, to_box.bbox);
    gap > 0.0
        && gap < target_gap
        && axis_overlap_ratio(
            from_box.bbox[1],
            from_box.bbox[3],
            to_box.bbox[1],
            to_box.bbox[3],
        ) > 0.25
}

fn is_note_like_connected_box(id: &str, info: &BoxRouteInfo) -> bool {
    let text = route_box_text(id, info);
    text.contains("note")
        || text.contains("inference")
        || text.contains("student only")
        || text.contains("student-only")
        || text.contains("muted")
}

fn horizontal_gutter_candidate(
    from_id: &str,
    from_box: &BoxRouteInfo,
    to_id: &str,
    to_box: &BoxRouteInfo,
    target_gap: f64,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<(String, [f64; 4])> {
    let from_left = center_x(from_box.bbox) <= center_x(to_box.bbox);
    let (left_id, left_box, right_id, right_box) = if from_left {
        (from_id, from_box, to_id, to_box)
    } else {
        (to_id, to_box, from_id, from_box)
    };
    let gap = horizontal_separation(left_box.bbox, right_box.bbox);
    let shift = (target_gap - gap).max(0.0);
    if shift <= 0.001 {
        return None;
    }

    let mut candidates = Vec::new();
    if let Some(candidate) = shifted_box_inside(right_box.bbox, shift, 0.0) {
        candidates.push((right_id.to_string(), candidate));
    }
    if let Some(candidate) = shifted_box_inside(left_box.bbox, -shift, 0.0) {
        candidates.push((left_id.to_string(), candidate));
    }

    candidates
        .into_iter()
        .filter(|(moving_id, candidate)| {
            route_box_candidate_has_gutter(moving_id, *candidate, box_map)
        })
        .min_by(|(left_id, left), (right_id, right)| {
            horizontal_gutter_candidate_score(left_id, *left, box_map).total_cmp(
                &horizontal_gutter_candidate_score(right_id, *right, box_map),
            )
        })
}

fn route_box_candidate_has_gutter(
    moving_id: &str,
    candidate: [f64; 4],
    box_map: &HashMap<String, BoxRouteInfo>,
) -> bool {
    box_map.iter().all(|(id, info)| {
        id == moving_id
            || (!component_overlap_gate_fails(candidate, info.bbox)
                && !component_crowding_gate_fails_normalized(candidate, info.bbox))
    })
}

fn horizontal_gutter_candidate_score(
    moving_id: &str,
    candidate: [f64; 4],
    box_map: &HashMap<String, BoxRouteInfo>,
) -> f64 {
    let original = box_map
        .get(moving_id)
        .map(|info| info.bbox)
        .unwrap_or(candidate);
    box_center_distance(candidate, original) + edge_pressure(candidate) * 0.05
}

fn separate_note_like_connected_boxes_from_sources(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut updates = Vec::new();
    let target_gap = 0.025;

    for object in &plan.objects {
        let DrawObject::Connector { from, to, .. } = object else {
            continue;
        };
        let (Some(source_id), Some(note_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(source), Some(note)) = (box_map.get(source_id), box_map.get(note_id)) else {
            continue;
        };
        if !is_note_like_connected_box(note_id, note)
            || axis_overlap_ratio(source.bbox[1], source.bbox[3], note.bbox[1], note.bbox[3])
                <= 0.25
            || horizontal_separation(source.bbox, note.bbox) >= target_gap
        {
            continue;
        }
        let Some(candidate) = note_like_source_gutter_candidate(
            note_id,
            note.bbox,
            source.bbox,
            target_gap,
            &box_map,
        ) else {
            continue;
        };
        updates.push((note_id.to_string(), candidate));
    }

    if updates.is_empty() {
        return;
    }
    let mut moved_ids = HashSet::new();
    for (id, bbox) in updates {
        set_box_bbox(plan, &id, bbox);
        moved_ids.insert(id);
    }
    realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
}

fn note_like_source_gutter_candidate(
    note_id: &str,
    note_bbox: [f64; 4],
    source_bbox: [f64; 4],
    target_gap: f64,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<[f64; 4]> {
    let note_right_of_source = center_x(note_bbox) >= center_x(source_bbox);
    let current_gap = horizontal_separation(note_bbox, source_bbox);
    let shift = (target_gap - current_gap).max(0.0);
    if shift <= 0.001 {
        return None;
    }
    let dx = if note_right_of_source { shift } else { -shift };
    shifted_box_inside(note_bbox, dx, 0.0)
        .filter(|candidate| route_box_candidate_is_clear(note_id, *candidate, box_map))
}

fn horizontal_overlap(left: [f64; 4], right: [f64; 4]) -> f64 {
    let left = normalize_box(left);
    let right = normalize_box(right);
    (left[2].min(right[2]) - left[0].max(right[0])).max(0.0)
}

fn separate_crowded_loss_or_objective_boxes(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let ids = box_map.keys().cloned().collect::<Vec<_>>();
    let mut updates = Vec::new();
    let mut moved_ids = HashSet::new();
    let target_gap = 0.055;

    for left_index in 0..ids.len() {
        for right_id in ids.iter().skip(left_index + 1) {
            let left_id = &ids[left_index];
            let (Some(left), Some(right)) = (box_map.get(left_id), box_map.get(right_id)) else {
                continue;
            };
            if !is_loss_or_objective_box(left_id, left)
                || !is_loss_or_objective_box(right_id, right)
                || horizontal_overlap(left.bbox, right.bbox) < 0.02
            {
                continue;
            }
            let (upper_id, upper, lower_id, lower) = if center_y(left.bbox) <= center_y(right.bbox)
            {
                (left_id.as_str(), left, right_id.as_str(), right)
            } else {
                (right_id.as_str(), right, left_id.as_str(), left)
            };
            let gap = lower.bbox[1] - upper.bbox[3];
            if gap >= target_gap {
                continue;
            }
            let shift = target_gap - gap;
            let upper_candidate = shifted_box_inside(upper.bbox, 0.0, -shift);
            let lower_candidate = shifted_box_inside(lower.bbox, 0.0, shift);
            let chosen = upper_candidate
                .filter(|candidate| route_box_candidate_is_clear(upper_id, *candidate, &box_map))
                .map(|candidate| (upper_id.to_string(), candidate))
                .or_else(|| {
                    lower_candidate
                        .filter(|candidate| {
                            route_box_candidate_is_clear(lower_id, *candidate, &box_map)
                        })
                        .map(|candidate| (lower_id.to_string(), candidate))
                });
            if let Some(update) = chosen {
                updates.push(update);
            }
        }
    }

    for (id, bbox) in updates {
        set_box_bbox(plan, &id, bbox);
        moved_ids.insert(id);
    }
    if !moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    }
}

fn separate_teacher_context_objective_gutters(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut updates = Vec::new();
    let target_gap = 0.065;

    for (context_id, context) in &box_map {
        if !is_teacher_or_context_route_box(context_id, context) {
            continue;
        }
        for (objective_id, objective) in &box_map {
            if objective_id == context_id
                || !is_objective_near_teacher_context_box(objective_id, objective)
            {
                continue;
            }
            let Some(candidate) = teacher_context_objective_gutter_candidate(
                objective_id,
                objective.bbox,
                context.bbox,
                target_gap,
                &box_map,
            ) else {
                continue;
            };
            updates.push((objective_id.clone(), candidate));
        }
    }

    if updates.is_empty() {
        return;
    }
    let mut moved_ids = HashSet::new();
    for (id, bbox) in updates {
        set_box_bbox(plan, &id, bbox);
        moved_ids.insert(id);
    }
    realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
}

fn is_objective_near_teacher_context_box(id: &str, info: &BoxRouteInfo) -> bool {
    is_objective_route_box(id, info)
        && !is_task_loss_route_box(id, info)
        && !is_main_route_box(info)
}

fn teacher_context_objective_gutter_candidate(
    objective_id: &str,
    objective: [f64; 4],
    context: [f64; 4],
    target_gap: f64,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<[f64; 4]> {
    let width = box_width(objective);
    let height = box_height(objective);
    let mut candidates = Vec::new();

    if horizontal_overlap(objective, context) > 0.02
        && vertical_separation(objective, context) < target_gap
    {
        if center_y(objective) <= center_y(context) {
            let y2 = context[1] - target_gap;
            candidates.push([objective[0], y2 - height, objective[2], y2]);
        } else {
            let y1 = context[3] + target_gap;
            candidates.push([objective[0], y1, objective[2], y1 + height]);
        }
    }

    if axis_overlap_ratio(objective[1], objective[3], context[1], context[3]) > 0.20
        && horizontal_separation(objective, context) < target_gap
    {
        if center_x(objective) <= center_x(context) {
            let x2 = context[0] - target_gap;
            candidates.push([x2 - width, objective[1], x2, objective[3]]);
        } else {
            let x1 = context[2] + target_gap;
            candidates.push([x1, objective[1], x1 + width, objective[3]]);
        }
    }

    candidates
        .into_iter()
        .map(normalize_box)
        .filter(|candidate| {
            candidate[0] >= 0.02
                && candidate[1] >= 0.02
                && candidate[2] <= 0.98
                && candidate[3] <= 0.98
                && route_box_candidate_has_gutter(objective_id, *candidate, box_map)
        })
        .min_by(|left, right| {
            box_center_distance(*left, objective).total_cmp(&box_center_distance(*right, objective))
        })
}

fn pull_top_residual_losses_near_sources(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut updates = Vec::new();
    let mut planned_ids = HashSet::new();

    for object in &plan.objects {
        let DrawObject::Connector {
            id,
            points,
            from,
            to,
            ..
        } = object
        else {
            continue;
        };
        let (Some(source_id), Some(objective_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        if planned_ids.contains(objective_id) {
            continue;
        }
        let (Some(source), Some(objective)) = (box_map.get(source_id), box_map.get(objective_id))
        else {
            continue;
        };
        if !is_top_residual_objective(objective_id, objective)
            || !top_residual_objective_needs_pull(points, source.bbox, objective.bbox)
        {
            continue;
        }
        let Some((candidate, route)) = best_top_residual_objective_candidate(
            objective_id,
            source_id,
            source.bbox,
            objective.bbox,
            &box_map,
            id,
            plan,
        ) else {
            continue;
        };
        updates.push((objective_id.to_string(), id.clone(), candidate, route));
        planned_ids.insert(objective_id.to_string());
    }

    if updates.is_empty() {
        return;
    }

    let mut moved_ids = HashSet::new();
    for (objective_id, _, candidate, _) in &updates {
        set_box_bbox(plan, objective_id, *candidate);
        moved_ids.insert(objective_id.clone());
    }
    realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    reroute_connectors_for_moved_objectives(plan, &moved_ids);
    for (_, connector_id, _, route) in updates {
        if let Some(DrawObject::Connector { points, .. }) = plan
            .objects
            .iter_mut()
            .find(|object| draw_object_id(object) == connector_id)
        {
            *points = route;
        }
    }
}

fn is_top_residual_objective(id: &str, info: &BoxRouteInfo) -> bool {
    let text = route_box_text(id, info);
    info.bbox[1] < 0.16
        && is_objective_route_box(id, info)
        && !is_task_loss_route_box(id, info)
        && (text.contains("residual")
            || text.contains("latent")
            || text.contains("supervision")
            || text.contains("alignment"))
}

fn top_residual_objective_needs_pull(
    points: &[[f64; 2]],
    source_bbox: [f64; 4],
    objective_bbox: [f64; 4],
) -> bool {
    if horizontal_separation(source_bbox, objective_bbox) <= 0.04 {
        return false;
    }
    let route_box = points_to_box(points);
    center_y(source_bbox) > objective_bbox[3] + 0.14
        || box_height(route_box) > 0.32
        || vertical_separation(source_bbox, objective_bbox) > 0.24
}

fn best_top_residual_objective_candidate(
    objective_id: &str,
    source_id: &str,
    source_bbox: [f64; 4],
    objective_bbox: [f64; 4],
    box_map: &HashMap<String, BoxRouteInfo>,
    connector_id: &str,
    plan: &DrawPlan,
) -> Option<([f64; 4], Vec<[f64; 2]>)> {
    let width = box_width(objective_bbox).clamp(0.12, 0.21);
    let height = box_height(objective_bbox).clamp(0.09, 0.13);
    top_residual_objective_candidates(source_bbox, objective_bbox, width, height)
        .into_iter()
        .filter_map(|candidate| {
            if candidate[1] < 0.18
                || !objective_hub_candidate_is_clear(objective_id, candidate, box_map)
            {
                return None;
            }
            let route = orthogonal_connector_points_between_boxes(source_bbox, candidate);
            let mut candidate_box_map = box_map.clone();
            if let Some(objective) = candidate_box_map.get_mut(objective_id) {
                objective.bbox = candidate;
            }
            if connector_points_intersect_intermediate_boxes(
                route.as_slice(),
                source_id,
                objective_id,
                &candidate_box_map,
            ) {
                return None;
            }
            let conflict_penalty = if connector_route_conflicts_with_other_connectors(
                route.as_slice(),
                connector_id,
                plan,
            ) {
                2.0
            } else {
                0.0
            };
            Some((
                candidate,
                route,
                top_residual_objective_candidate_score(candidate, source_bbox, objective_bbox)
                    + conflict_penalty,
            ))
        })
        .min_by(|left, right| left.2.total_cmp(&right.2))
        .map(|(candidate, route, _)| (candidate, route))
}

fn top_residual_objective_candidates(
    source_bbox: [f64; 4],
    objective_bbox: [f64; 4],
    width: f64,
    height: f64,
) -> Vec<[f64; 4]> {
    let gap = 0.035;
    let mut candidates = Vec::new();
    push_unique_box(
        &mut candidates,
        box_from_top_left_inside(
            center_x(source_bbox) - width / 2.0,
            source_bbox[1] - gap - height,
            width,
            height,
        ),
    );
    push_unique_box(
        &mut candidates,
        box_from_top_left_inside(
            source_bbox[2] + gap,
            center_y(source_bbox) - height / 2.0,
            width,
            height,
        ),
    );
    push_unique_box(
        &mut candidates,
        box_from_top_left_inside(
            source_bbox[0] - gap - width,
            center_y(source_bbox) - height / 2.0,
            width,
            height,
        ),
    );
    push_unique_box(
        &mut candidates,
        box_from_top_left_inside(
            center_x(source_bbox) - width / 2.0,
            source_bbox[3] + gap,
            width,
            height,
        ),
    );
    push_unique_box(
        &mut candidates,
        box_from_top_left_inside(
            objective_bbox[0],
            center_y(source_bbox) - height / 2.0,
            width,
            height,
        ),
    );
    candidates
}

fn top_residual_objective_candidate_score(
    candidate: [f64; 4],
    source_bbox: [f64; 4],
    original: [f64; 4],
) -> f64 {
    vertical_separation(candidate, source_bbox) * 0.9
        + horizontal_separation(candidate, source_bbox) * 0.7
        + edge_pressure(candidate) * 0.5
        + box_center_distance(candidate, original) * 0.12
}

fn reroute_connectors_for_moved_objectives(
    plan: &mut DrawPlan,
    moved_objective_ids: &HashSet<String>,
) {
    let box_map = current_box_route_info_map(plan);
    for object in &mut plan.objects {
        let DrawObject::Connector {
            points, from, to, ..
        } = object
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        if !moved_objective_ids.contains(from_id) && !moved_objective_ids.contains(to_id) {
            continue;
        }
        let (Some(from_box), Some(to_box)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        let candidate =
            moved_objective_connector_route(from_id, from_box, to_id, to_box, moved_objective_ids);
        if !connector_points_intersect_intermediate_boxes(
            candidate.as_slice(),
            from_id,
            to_id,
            &box_map,
        ) {
            *points = candidate;
        }
    }
}

fn moved_objective_connector_route(
    from_id: &str,
    from_box: &BoxRouteInfo,
    to_id: &str,
    to_box: &BoxRouteInfo,
    moved_objective_ids: &HashSet<String>,
) -> Vec<[f64; 2]> {
    if moved_objective_ids.contains(to_id) && is_task_loss_route_box(to_id, to_box) {
        if is_student_task_loss_source(from_id, from_box) {
            return local_student_task_loss_route(from_box.bbox, to_box.bbox);
        }
        return orthogonal_connector_points_between_boxes(from_box.bbox, to_box.bbox);
    }
    if moved_objective_ids.contains(from_id) && is_task_loss_route_box(from_id, from_box) {
        if is_student_task_loss_source(to_id, to_box) {
            let mut route = local_student_task_loss_route(to_box.bbox, from_box.bbox);
            route.reverse();
            return route;
        }
        return orthogonal_connector_points_between_boxes(from_box.bbox, to_box.bbox);
    }
    if moved_objective_ids.contains(from_id)
        && is_objective_route_box(from_id, from_box)
        && is_main_route_box(to_box)
    {
        return objective_to_main_connector_points(from_box.bbox, to_box.bbox);
    }
    if moved_objective_ids.contains(to_id)
        && is_objective_route_box(to_id, to_box)
        && is_main_route_box(from_box)
    {
        return orthogonal_connector_points_between_boxes(from_box.bbox, to_box.bbox);
    }
    orthogonal_connector_points_between_boxes(from_box.bbox, to_box.bbox)
}

fn move_objective_hubs_out_of_branch_gap_crowding(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut updates = Vec::new();

    for (hub_id, hub) in &box_map {
        if !is_residual_or_supervision_hub_box(hub_id, hub) {
            continue;
        }
        if multistage_residual_hub_has_valid_branch_slot(plan, hub_id, hub, &box_map) {
            continue;
        }
        let nearby_branches = box_map
            .iter()
            .filter(|(other_id, other)| {
                *other_id != hub_id
                    && is_branch_module_for_objective_hub(other_id, other)
                    && objective_hub_branch_in_same_column(hub.bbox, other.bbox)
            })
            .map(|(_, other)| other.bbox)
            .collect::<Vec<_>>();
        let crowded_branches = nearby_branches
            .iter()
            .copied()
            .filter(|branch| objective_hub_too_close_to_branch(hub.bbox, *branch))
            .collect::<Vec<_>>();
        let branch_context = if crowded_branches.len() >= 2 {
            crowded_branches
        } else if !crowded_branches.is_empty()
            && objective_hub_sits_between_branch_rows(hub.bbox, &nearby_branches)
        {
            nearby_branches
        } else {
            Vec::new()
        };
        if branch_context.len() < 2 {
            continue;
        }

        let candidates = objective_hub_clearance_candidates(hub.bbox, &branch_context);
        let Some(best_candidate) = candidates
            .into_iter()
            .filter(|candidate| objective_hub_candidate_is_clear(hub_id, *candidate, &box_map))
            .min_by(|left, right| {
                objective_hub_candidate_score(*left, hub.bbox)
                    .total_cmp(&objective_hub_candidate_score(*right, hub.bbox))
            })
        else {
            continue;
        };
        updates.push((hub_id.clone(), best_candidate));
    }

    if updates.is_empty() {
        return;
    }
    let mut moved_ids = HashSet::new();
    for (id, bbox) in updates {
        set_box_bbox(plan, &id, bbox);
        moved_ids.insert(id);
    }
    realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
}

fn multistage_residual_hub_has_valid_branch_slot(
    plan: &DrawPlan,
    hub_id: &str,
    hub: &BoxRouteInfo,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> bool {
    let pairs = multistage_teacher_student_stage_pairs(box_map);
    if pairs.len() < 2 || !multistage_pairs_include_projection(&pairs) {
        return false;
    }
    let Some(output_pair) = pairs
        .iter()
        .find(|pair| pair.stage == MultistageTeacherStudentStage::Output)
    else {
        return false;
    };
    let Some((upper_branch, lower_branch)) =
        connected_multistage_residual_branches(plan, hub_id, box_map, output_pair)
    else {
        return false;
    };
    residual_sits_between_multistage_branches(hub.bbox, upper_branch, lower_branch)
}

fn objective_hub_branch_in_same_column(hub_bbox: [f64; 4], branch_bbox: [f64; 4]) -> bool {
    horizontal_overlap(hub_bbox, branch_bbox) > 0.02
        || horizontal_separation(hub_bbox, branch_bbox) < 0.08
}

fn objective_hub_too_close_to_branch(hub_bbox: [f64; 4], branch_bbox: [f64; 4]) -> bool {
    component_crowding_gate_fails_normalized(hub_bbox, branch_bbox)
        || (vertical_separation(hub_bbox, branch_bbox) <= 0.055
            && axis_overlap_ratio(hub_bbox[0], hub_bbox[2], branch_bbox[0], branch_bbox[2]) > 0.25)
        || (horizontal_separation(hub_bbox, branch_bbox) < 0.03
            && axis_overlap_ratio(hub_bbox[1], hub_bbox[3], branch_bbox[1], branch_bbox[3]) > 0.25)
}

fn objective_hub_sits_between_branch_rows(hub_bbox: [f64; 4], branch_bboxes: &[[f64; 4]]) -> bool {
    let hub_y = center_y(hub_bbox);
    let has_upper = branch_bboxes
        .iter()
        .any(|branch| center_y(*branch) < hub_y && branch[3] <= hub_bbox[3] + 0.02);
    let has_lower = branch_bboxes
        .iter()
        .any(|branch| center_y(*branch) > hub_y && branch[1] >= hub_bbox[1] - 0.02);
    has_upper && has_lower
}

fn is_residual_or_supervision_hub_box(id: &str, info: &BoxRouteInfo) -> bool {
    let text = route_box_text(id, info);
    (text.contains("residual") || text.contains("supervision") || text.contains("alignment"))
        && is_objective_route_box(id, info)
        && !is_task_loss_route_box(id, info)
}

fn is_branch_module_for_objective_hub(id: &str, info: &BoxRouteInfo) -> bool {
    is_main_route_box(info) || is_teacher_or_context_route_box(id, info)
}

fn objective_hub_clearance_candidates(
    hub_bbox: [f64; 4],
    crowded_branches: &[[f64; 4]],
) -> Vec<[f64; 4]> {
    let branch_union = crowded_branches
        .iter()
        .copied()
        .reduce(union_box)
        .unwrap_or(hub_bbox);
    let width = box_width(hub_bbox);
    let height = box_height(hub_bbox);
    let gap = 0.055;
    let mut candidates = Vec::new();
    let x_positions = [
        hub_bbox[0],
        center_x(branch_union) - width / 2.0,
        branch_union[2] - width,
        branch_union[0],
    ];

    let above_y1 = branch_union[1] - gap - height;
    let below_y1 = branch_union[3] + gap;
    for x1 in x_positions {
        push_unique_box(
            &mut candidates,
            box_from_top_left_inside(x1, above_y1, width, height),
        );
        push_unique_box(
            &mut candidates,
            box_from_top_left_inside(x1, below_y1, width, height),
        );
    }

    let center_y1 = center_y(branch_union) - height / 2.0;
    push_unique_box(
        &mut candidates,
        box_from_top_left_inside(branch_union[2] + 0.035, center_y1, width, height),
    );
    push_unique_box(
        &mut candidates,
        box_from_top_left_inside(branch_union[0] - 0.035 - width, center_y1, width, height),
    );
    candidates
}

fn box_from_top_left_inside(x1: f64, y1: f64, width: f64, height: f64) -> [f64; 4] {
    let x1 = x1.clamp(0.02, 0.98 - width);
    let y1 = y1.clamp(0.02, 0.98 - height);
    [x1, y1, x1 + width, y1 + height]
}

fn objective_hub_candidate_is_clear(
    moving_id: &str,
    candidate: [f64; 4],
    box_map: &HashMap<String, BoxRouteInfo>,
) -> bool {
    box_map.iter().all(|(id, info)| {
        id == moving_id
            || (!component_overlap_gate_fails(candidate, info.bbox)
                && !component_crowding_gate_fails_normalized(candidate, info.bbox))
    })
}

fn objective_hub_candidate_score(candidate: [f64; 4], original: [f64; 4]) -> f64 {
    box_center_distance(candidate, original) + edge_pressure(candidate) * 0.2
}

fn component_crowding_gate_fails_normalized(left: [f64; 4], right: [f64; 4]) -> bool {
    let vertical_gap = vertical_separation(left, right);
    if vertical_gap > 0.0
        && vertical_gap < 0.055
        && axis_overlap_ratio(left[0], left[2], right[0], right[2]) > 0.25
    {
        return true;
    }

    let horizontal_gap = horizontal_separation(left, right);
    horizontal_gap > 0.0
        && horizontal_gap < 0.03
        && axis_overlap_ratio(left[1], left[3], right[1], right[3]) > 0.25
}

fn axis_overlap_ratio(left_start: f64, left_end: f64, right_start: f64, right_end: f64) -> f64 {
    let overlap = (left_end.min(right_end) - left_start.max(right_start)).max(0.0);
    let left_len = (left_end - left_start).abs();
    let right_len = (right_end - right_start).abs();
    overlap / left_len.min(right_len).max(0.0001)
}

fn separate_stacked_context_boxes(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let context_pairs = stacked_context_connector_pairs(plan, &box_map);
    if context_pairs.is_empty() {
        return;
    }

    let mut moved_ids = HashSet::new();
    for (upper_id, lower_id) in context_pairs {
        let box_map = current_box_route_info_map(plan);
        if !box_map.contains_key(&upper_id) || !box_map.contains_key(&lower_id) {
            continue;
        }
        if let Some((id, bbox)) = context_stack_gap_candidate(&upper_id, &lower_id, &box_map) {
            set_box_bbox(plan, &id, bbox);
            moved_ids.insert(id);
        }

        let box_map = current_box_route_info_map(plan);
        let (Some(upper), Some(lower)) = (box_map.get(&upper_id), box_map.get(&lower_id)) else {
            continue;
        };
        if let Some((upper_bbox, lower_bbox)) =
            context_stack_shift_away_from_main(&upper_id, upper, &lower_id, lower, &box_map)
        {
            set_box_bbox(plan, &upper_id, upper_bbox);
            set_box_bbox(plan, &lower_id, lower_bbox);
            moved_ids.insert(upper_id.clone());
            moved_ids.insert(lower_id.clone());

            let box_map = current_box_route_info_map(plan);
            if let Some((id, bbox)) = context_stack_gap_candidate(&upper_id, &lower_id, &box_map) {
                set_box_bbox(plan, &id, bbox);
                moved_ids.insert(id);
            }
        }
    }

    if !moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    }
}

fn context_stack_gap_candidate(
    upper_id: &str,
    lower_id: &str,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<(String, [f64; 4])> {
    let upper = box_map.get(upper_id)?;
    let lower = box_map.get(lower_id)?;
    let gap = lower.bbox[1] - upper.bbox[3];
    let target_gap = 0.04;
    if gap >= target_gap {
        return None;
    }
    let shift = target_gap - gap;
    let lower_candidate = shifted_box_inside(lower.bbox, 0.0, shift);
    let upper_candidate = shifted_box_inside(upper.bbox, 0.0, -shift);
    lower_candidate
        .filter(|candidate| route_box_candidate_is_clear(lower_id, *candidate, box_map))
        .map(|candidate| (lower_id.to_string(), candidate))
        .or_else(|| {
            upper_candidate
                .filter(|candidate| route_box_candidate_is_clear(upper_id, *candidate, box_map))
                .map(|candidate| (upper_id.to_string(), candidate))
        })
}

fn stacked_context_connector_pairs(
    plan: &DrawPlan,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for object in &plan.objects {
        let DrawObject::Connector { from, to, .. } = object else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(from), Some(to)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        if !is_teacher_or_context_route_box(from_id, from)
            || !is_teacher_or_context_route_box(to_id, to)
            || horizontal_overlap(from.bbox, to.bbox) < 0.06
            || (center_x(from.bbox) - center_x(to.bbox)).abs() > 0.08
        {
            continue;
        }
        if center_y(from.bbox) <= center_y(to.bbox) {
            pairs.push((from_id.to_string(), to_id.to_string()));
        } else {
            pairs.push((to_id.to_string(), from_id.to_string()));
        }
    }
    pairs
}

fn context_stack_shift_away_from_main(
    upper_id: &str,
    upper: &BoxRouteInfo,
    lower_id: &str,
    lower: &BoxRouteInfo,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<([f64; 4], [f64; 4])> {
    let stack = union_box(upper.bbox, lower.bbox);
    let is_crowded_with_main = box_map.iter().any(|(id, info)| {
        id != upper_id
            && id != lower_id
            && is_main_route_box(info)
            && horizontal_overlap(stack, info.bbox) > 0.015
            && vertical_separation(stack, info.bbox) < 0.06
    });
    if !is_crowded_with_main {
        return None;
    }

    let mut shifts = Vec::new();
    for (id, info) in box_map {
        if id == upper_id || id == lower_id || !is_main_route_box(info) {
            continue;
        }
        if horizontal_overlap(stack, info.bbox) <= 0.015
            || vertical_separation(stack, info.bbox) >= 0.06
        {
            continue;
        }
        shifts.push(info.bbox[2] + 0.035 - stack[0]);
        shifts.push(info.bbox[0] - 0.035 - stack[2]);
    }

    shifts
        .into_iter()
        .filter_map(|dx| {
            let upper_candidate = shifted_box_horizontally_inside(upper.bbox, dx)?;
            let lower_candidate = shifted_box_horizontally_inside(lower.bbox, dx)?;
            stack_shift_is_clear(
                upper_id,
                upper_candidate,
                lower_id,
                lower_candidate,
                box_map,
            )
            .then(|| {
                (
                    upper_candidate,
                    lower_candidate,
                    stack_shift_score(
                        upper_id,
                        upper_candidate,
                        lower_id,
                        lower_candidate,
                        box_map,
                    ),
                )
            })
        })
        .min_by(|left, right| left.2.total_cmp(&right.2))
        .map(|(upper_bbox, lower_bbox, _)| (upper_bbox, lower_bbox))
}

fn stack_shift_is_clear(
    upper_id: &str,
    upper_candidate: [f64; 4],
    lower_id: &str,
    lower_candidate: [f64; 4],
    box_map: &HashMap<String, BoxRouteInfo>,
) -> bool {
    box_map.iter().all(|(id, info)| {
        id == upper_id
            || id == lower_id
            || (!component_overlap_gate_fails(upper_candidate, info.bbox)
                && !component_overlap_gate_fails(lower_candidate, info.bbox))
    })
}

fn stack_shift_score(
    upper_id: &str,
    upper_candidate: [f64; 4],
    lower_id: &str,
    lower_candidate: [f64; 4],
    box_map: &HashMap<String, BoxRouteInfo>,
) -> f64 {
    let mut score = edge_pressure(upper_candidate) + edge_pressure(lower_candidate);
    for (id, info) in box_map {
        if id == upper_id || id == lower_id {
            continue;
        }
        for candidate in [upper_candidate, lower_candidate] {
            if horizontal_overlap(candidate, info.bbox) > 0.01
                && vertical_separation(candidate, info.bbox) < 0.06
            {
                score += 10.0;
            }
            score += intersection_area(candidate, info.bbox) * 100.0;
        }
    }
    score
}

fn union_box(left: [f64; 4], right: [f64; 4]) -> [f64; 4] {
    let left = normalize_box(left);
    let right = normalize_box(right);
    [
        left[0].min(right[0]),
        left[1].min(right[1]),
        left[2].max(right[2]),
        left[3].max(right[3]),
    ]
}

fn compact_output_width(id: &str, output: &BoxRouteInfo) -> f64 {
    let visible_chars = output
        .text
        .chars()
        .filter(|ch| !ch.is_whitespace() && *ch != '_' && *ch != '-')
        .count();
    if visible_chars <= 3 || id.to_lowercase().contains("pred") {
        0.04
    } else {
        box_width(output.bbox).clamp(0.06, 0.10)
    }
}

fn route_box_candidate_is_clear(
    moving_id: &str,
    candidate: [f64; 4],
    box_map: &HashMap<String, BoxRouteInfo>,
) -> bool {
    box_map
        .iter()
        .all(|(id, info)| id == moving_id || !component_overlap_gate_fails(candidate, info.bbox))
}

fn improve_connector_routes_against_boxes(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    for object in &mut plan.objects {
        let DrawObject::Connector {
            points, from, to, ..
        } = object
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(from_box), Some(to_box)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        if let Some(clean_points) =
            clean_connector_points_for_boxes(from_id, from_box, to_id, to_box)
        {
            *points = clean_points;
        }
    }
}

fn reroute_connectors_around_intermediate_boxes(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    for object in &mut plan.objects {
        let DrawObject::Connector {
            points, from, to, ..
        } = object
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(from_box), Some(to_box)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        if is_objective_route_box(from_id, from_box) && is_main_route_box(to_box) {
            continue;
        }
        if !connector_points_intersect_intermediate_boxes(points, from_id, to_id, &box_map) {
            continue;
        }
        if let Some(candidate) =
            connector_points_around_intermediate_boxes(from_id, from_box, to_id, to_box, &box_map)
        {
            *points = candidate;
        }
    }
}

fn reroute_connectors_around_crossing_edges(plan: &mut DrawPlan) {
    for _ in 0..8 {
        let Some((moving_id, blocker_id)) = crossing_connector_pair_to_repair(plan) else {
            break;
        };
        let box_map = current_box_route_info_map(plan);
        let Some(candidate) =
            candidate_route_avoiding_crossing_edges(&moving_id, &blocker_id, plan, &box_map)
        else {
            break;
        };
        if let Some(DrawObject::Connector { points, .. }) = plan
            .objects
            .iter_mut()
            .find(|object| draw_object_id(object) == moving_id)
        {
            *points = candidate;
        } else {
            break;
        }
    }
}

fn reroute_supervision_connectors_around_main_edges(plan: &mut DrawPlan) {
    for _ in 0..4 {
        let connectors = connector_route_snapshots(plan);
        let Some(moving) = connectors.iter().find(|candidate| {
            is_supervision_connector_route(candidate)
                && connectors.iter().any(|other| {
                    other.id != candidate.id
                        && is_main_connector_route(other)
                        && connector_routes_conflict(&candidate.points, &other.points)
                })
        }) else {
            break;
        };
        let moving_id = moving.id.clone();
        let box_map = current_box_route_info_map(plan);
        let Some(candidate) = supervision_route_avoiding_main_edges(moving, plan, &box_map) else {
            break;
        };
        if let Some(DrawObject::Connector { points, .. }) = plan
            .objects
            .iter_mut()
            .find(|object| draw_object_id(object) == moving_id)
        {
            *points = candidate;
        } else {
            break;
        }
    }
}

fn reroute_residual_objective_connectors_around_main_crossings(plan: &mut DrawPlan) {
    for _ in 0..4 {
        let connectors = connector_route_snapshots(plan);
        let Some((moving, blocker)) = connectors.iter().find_map(|candidate| {
            if !is_residual_objective_connector_route(candidate) {
                return None;
            }
            connectors
                .iter()
                .find(|other| {
                    other.id != candidate.id
                        && is_main_connector_route(other)
                        && connector_routes_conflict(&candidate.points, &other.points)
                })
                .map(|other| (candidate, other))
        }) else {
            break;
        };
        let moving_id = moving.id.clone();
        let box_map = current_box_route_info_map(plan);
        let Some(candidate) =
            residual_objective_route_around_main_crossing(moving, blocker, plan, &box_map)
        else {
            break;
        };
        if let Some(DrawObject::Connector { points, .. }) = plan
            .objects
            .iter_mut()
            .find(|object| draw_object_id(object) == moving_id)
        {
            *points = candidate;
        } else {
            break;
        }
    }
}

fn reroute_task_loss_connectors_around_supervision_edges(plan: &mut DrawPlan) {
    for _ in 0..4 {
        let connectors = connector_route_snapshots(plan);
        let Some(moving) = connectors.iter().find(|candidate| {
            is_task_or_loss_connector_route(candidate)
                && connectors.iter().any(|other| {
                    other.id != candidate.id
                        && is_supervision_connector_route(other)
                        && connector_routes_conflict(&candidate.points, &other.points)
                })
        }) else {
            break;
        };
        let moving_id = moving.id.clone();
        let box_map = current_box_route_info_map(plan);
        let Some(candidate) = task_loss_route_avoiding_supervision_edges(moving, plan, &box_map)
        else {
            break;
        };
        if let Some(DrawObject::Connector { points, .. }) = plan
            .objects
            .iter_mut()
            .find(|object| draw_object_id(object) == moving_id)
        {
            *points = candidate;
        } else {
            break;
        }
    }
}

fn repair_outer_input_context_detours(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let repairs = plan
        .objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Connector {
                id,
                points,
                from,
                to,
                ..
            } = object
            else {
                return None;
            };
            let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
                return None;
            };
            let (Some(from_box), Some(to_box)) = (box_map.get(from_id), box_map.get(to_id)) else {
                return None;
            };
            if !is_input_route_box(from_id, from_box)
                || !(is_teacher_or_context_route_box(to_id, to_box)
                    || is_student_or_main_route_box(to_box))
                || !input_context_route_needs_repair(points, from_box.bbox, to_box.bbox)
            {
                return None;
            }
            let candidate =
                compact_input_context_route(id, from_id, from_box, to_id, to_box, plan, &box_map)?;
            Some((id.clone(), candidate))
        })
        .collect::<Vec<_>>();

    for (id, candidate) in repairs {
        if let Some(DrawObject::Connector { points, .. }) = plan
            .objects
            .iter_mut()
            .find(|object| draw_object_id(object) == id)
        {
            *points = candidate;
        }
    }
}

fn input_context_route_needs_repair(
    points: &[[f64; 2]],
    from_box: [f64; 4],
    to_box: [f64; 4],
) -> bool {
    if points.len() < 4 {
        return false;
    }
    let route_box = points_to_box(points);
    let endpoint_box = union_box(from_box, to_box);
    let direct = box_center_distance(from_box, to_box).max(0.001);
    route_box[1] < endpoint_box[1] - 0.12
        || route_box[3] > endpoint_box[3] + 0.12
        || route_box[0] < endpoint_box[0] - 0.08
        || polyline_length(points) / direct > 1.55
}

fn compact_input_context_route(
    connector_id: &str,
    from_id: &str,
    from_box: &BoxRouteInfo,
    to_id: &str,
    to_box: &BoxRouteInfo,
    plan: &DrawPlan,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<Vec<[f64; 2]>> {
    let mut candidates = Vec::new();
    if let Some(candidate) =
        connector_points_around_intermediate_boxes(from_id, from_box, to_id, to_box, box_map)
    {
        candidates.push(candidate);
    }
    candidates.extend(input_context_edge_dogleg_candidates(
        from_box.bbox,
        to_box.bbox,
    ));
    candidates.push(input_branch_connector_points(from_box.bbox, to_box.bbox));
    candidates.extend(horizontal_dogleg_candidates_around_boxes(
        from_box.bbox,
        to_box.bbox,
        from_id,
        to_id,
        box_map,
    ));

    let candidates = candidates
        .into_iter()
        .map(|candidate| remove_redundant_collinear_points(&candidate))
        .filter(|candidate| {
            candidate.len() >= 2
                && !connector_points_intersect_intermediate_boxes(
                    candidate, from_id, to_id, box_map,
                )
        })
        .collect::<Vec<_>>();

    candidates
        .iter()
        .filter(|candidate| {
            !connector_route_conflicts_with_other_connectors(candidate, connector_id, plan)
        })
        .min_by(|left, right| connector_route_score(left).total_cmp(&connector_route_score(right)))
        .cloned()
        .or_else(|| {
            candidates.into_iter().min_by(|left, right| {
                connector_route_score(left).total_cmp(&connector_route_score(right))
            })
        })
}

fn input_context_edge_dogleg_candidates(
    from_box: [f64; 4],
    to_box: [f64; 4],
) -> Vec<Vec<[f64; 2]>> {
    if horizontal_separation(from_box, to_box) <= 0.04 {
        return Vec::new();
    }
    let from_left = center_x(from_box) < center_x(to_box);
    let start_x = if from_left { from_box[2] } else { from_box[0] };
    let end_x = if from_left { to_box[0] } else { to_box[2] };
    let start_y = center_y(from_box);
    let target_top = to_box[1];
    let target_bottom = to_box[3];
    let width_gap = 0.025;
    let mut candidates = Vec::new();

    let bottom_y = target_bottom + width_gap;
    if bottom_y <= 0.96 {
        candidates.push(remove_redundant_collinear_points(&[
            [start_x, start_y],
            [start_x, bottom_y],
            [end_x, bottom_y],
            [end_x, target_bottom],
        ]));
    }

    let top_y = target_top - width_gap;
    if top_y >= 0.04 {
        candidates.push(remove_redundant_collinear_points(&[
            [start_x, start_y],
            [start_x, top_y],
            [end_x, top_y],
            [end_x, target_top],
        ]));
    }

    candidates
}

fn orthogonalize_student_only_inference_connectors(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    for object in &mut plan.objects {
        let DrawObject::Connector {
            points, from, to, ..
        } = object
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(source), Some(output)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        if !is_main_route_box(source) || !is_student_only_inference_route_box(to_id, output) {
            continue;
        }
        if connector_points_are_orthogonal(points) && points.len() <= 3 {
            continue;
        }
        *points = orthogonal_connector_points_between_boxes(source.bbox, output.bbox);
    }
}

fn connector_points_are_orthogonal(points: &[[f64; 2]]) -> bool {
    points
        .windows(2)
        .all(|window| same_x(window[0], window[1]) || same_y(window[0], window[1]))
}

fn orthogonal_connector_points_between_boxes(
    from_box: [f64; 4],
    to_box: [f64; 4],
) -> Vec<[f64; 2]> {
    if horizontal_separation(from_box, to_box) > 0.02 {
        if let Some(y) = common_horizontal_y(from_box, to_box) {
            return horizontal_connector_points(from_box, to_box, y);
        }
    }
    if vertical_separation(from_box, to_box) > 0.02 {
        if let Some(x) = common_vertical_x(from_box, to_box) {
            return vertical_connector_points(from_box, to_box, x);
        }
    }
    let start = anchor_point_towards(from_box, center(to_box));
    let end = anchor_point_towards(to_box, center(from_box));
    remove_redundant_collinear_points(&[start, [end[0], start[1]], end])
}

fn residual_objective_route_around_main_crossing(
    moving: &ConnectorRouteSnapshot,
    blocker: &ConnectorRouteSnapshot,
    plan: &DrawPlan,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<Vec<[f64; 2]>> {
    let (Some(from_id), Some(to_id)) = (moving.from.as_deref(), moving.to.as_deref()) else {
        return None;
    };
    let (Some(from_box), Some(to_box)) = (box_map.get(from_id), box_map.get(to_id)) else {
        return None;
    };
    let from_left = center_x(from_box.bbox) <= center_x(to_box.bbox);
    let start = if from_left {
        [from_box.bbox[2], center_y(from_box.bbox)]
    } else {
        [from_box.bbox[0], center_y(from_box.bbox)]
    };
    let end = if from_left {
        [to_box.bbox[0], center_y(to_box.bbox)]
    } else {
        [to_box.bbox[2], center_y(to_box.bbox)]
    };
    let entry_x = if from_left {
        (from_box.bbox[2] + 0.04).min(to_box.bbox[0] - 0.035)
    } else {
        (from_box.bbox[0] - 0.04).max(to_box.bbox[2] + 0.035)
    }
    .clamp(0.04, 0.96);
    let exit_x = if from_left {
        (to_box.bbox[2] + 0.035).min(0.96)
    } else {
        (to_box.bbox[0] - 0.035).max(0.04)
    };
    let route_box = union_box(from_box.bbox, to_box.bbox);
    let blocker_box = points_to_box(blocker.points.as_slice());
    let mut rail_y_values = vec![
        route_box[1] - 0.055,
        route_box[3] + 0.055,
        blocker_box[1] - 0.055,
        blocker_box[3] + 0.055,
    ];
    for info in box_map.values() {
        rail_y_values.push(info.bbox[1] - 0.04);
        rail_y_values.push(info.bbox[3] + 0.04);
    }
    rail_y_values.retain(|y| (0.03..=0.97).contains(y));
    rail_y_values.sort_by(|left, right| left.total_cmp(right));
    rail_y_values.dedup_by(|left, right| (*left - *right).abs() < 0.002);

    rail_y_values
        .into_iter()
        .flat_map(|rail_y| {
            [
                vec![
                    start,
                    [entry_x, start[1]],
                    [entry_x, rail_y],
                    [exit_x, rail_y],
                    [exit_x, end[1]],
                    end,
                ],
                vec![
                    start,
                    [entry_x, start[1]],
                    [entry_x, rail_y],
                    [end[0], rail_y],
                    end,
                ],
            ]
        })
        .map(|candidate| remove_redundant_collinear_points(&candidate))
        .filter(|candidate| {
            candidate.len() >= 2
                && !connector_points_intersect_intermediate_boxes(
                    candidate, from_id, to_id, box_map,
                )
                && !connector_route_conflicts_with_other_connectors(candidate, &moving.id, plan)
        })
        .min_by(|left, right| connector_route_score(left).total_cmp(&connector_route_score(right)))
}

fn straighten_residual_alignment_rails(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    for object in &mut plan.objects {
        let DrawObject::Connector {
            id,
            points,
            from,
            to,
            style,
            label,
            ..
        } = object
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(from_box), Some(to_box)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        if !is_residual_alignment_connector(id, style, label.as_ref())
            || is_objective_route_box(from_id, from_box)
            || !is_main_route_box(to_box)
            || from_box.bbox[3] + 0.04 >= to_box.bbox[1]
            || horizontal_separation(from_box.bbox, to_box.bbox) <= 0.04
        {
            continue;
        }
        let from_left = center_x(from_box.bbox) < center_x(to_box.bbox);
        let y = (from_box.bbox[3] + 0.025).min(to_box.bbox[1] - 0.045);
        let start_x = if from_left {
            from_box.bbox[2]
        } else {
            from_box.bbox[0]
        };
        let end_x = if from_left {
            to_box.bbox[0]
        } else {
            to_box.bbox[2]
        };
        *points = vec![[start_x, y], [end_x, y]];
        if let Some(label) = label {
            label.bbox = residual_alignment_label_bbox(label.bbox, points.as_slice());
        }
    }
}

fn simplify_teacher_alignment_stair_step_connectors(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    for object in &mut plan.objects {
        let DrawObject::Connector {
            id,
            points,
            from,
            to,
            style,
            label,
            ..
        } = object
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(from_box), Some(to_box)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        if !is_residual_alignment_connector(id, style, label.as_ref())
            || !is_teacher_or_context_route_box(from_id, from_box)
            || !is_residual_or_supervision_hub_box(to_id, to_box)
            || !route_box_text(to_id, to_box).contains("align")
            || !teacher_alignment_route_has_redundant_stair_step(points.as_slice())
        {
            continue;
        }
        let candidate =
            teacher_alignment_simplified_elbow_route(from_box.bbox, to_box.bbox).into_iter();
        let candidate = remove_redundant_collinear_points(&candidate.collect::<Vec<_>>());
        if candidate.len() < 2
            || connector_points_intersect_intermediate_boxes(
                candidate.as_slice(),
                from_id,
                to_id,
                &box_map,
            )
        {
            continue;
        }
        *points = candidate;
        if let Some(label) = label {
            label.bbox = teacher_alignment_elbow_label_bbox(label.bbox, points.as_slice());
        }
    }
}

fn teacher_alignment_route_has_redundant_stair_step(points: &[[f64; 2]]) -> bool {
    if points.len() > 4 {
        return true;
    }
    points.windows(2).any(|window| {
        let dx = (window[0][0] - window[1][0]).abs();
        let dy = (window[0][1] - window[1][1]).abs();
        (dx > 0.0 && dx < 0.035 && dy < 0.006) || (dy > 0.0 && dy < 0.035 && dx < 0.006)
    })
}

fn teacher_alignment_simplified_elbow_route(
    teacher_bbox: [f64; 4],
    alignment_bbox: [f64; 4],
) -> Vec<[f64; 2]> {
    let start = anchor_point_towards(teacher_bbox, center(alignment_bbox));
    let end = anchor_point_towards(alignment_bbox, center(teacher_bbox));
    if same_y(start, end) || same_x(start, end) {
        return vec![start, end];
    }
    let min_x = start[0].min(end[0]) + 0.035;
    let max_x = start[0].max(end[0]) - 0.035;
    let mid_x = (start[0] + end[0]) / 2.0;
    let elbow_x = if min_x <= max_x {
        mid_x.clamp(min_x, max_x)
    } else {
        mid_x
    };
    vec![start, [elbow_x, start[1]], [elbow_x, end[1]], end]
}

fn teacher_alignment_elbow_label_bbox(label_bbox: [f64; 4], points: &[[f64; 2]]) -> [f64; 4] {
    let width = box_width(label_bbox).clamp(0.04, 0.08);
    let height = box_height(label_bbox).clamp(0.04, 0.055);
    if points.len() >= 4 {
        let x = points[1][0] + 0.012;
        let y = (points[1][1] + points[2][1]) / 2.0 - height / 2.0;
        return box_from_top_left_inside(x, y, width, height);
    }
    residual_alignment_label_bbox(label_bbox, points)
}

fn is_residual_alignment_connector(id: &str, style: &str, label: Option<&DrawLabel>) -> bool {
    let label_text = label.map(|label| label.text.as_str()).unwrap_or("");
    let text = format!(
        "{} {} {}",
        id.to_lowercase(),
        style.to_lowercase(),
        label_text.to_lowercase()
    );
    text.contains("residual") || text.contains("align") || text.contains("supervision")
}

fn residual_alignment_label_bbox(label_bbox: [f64; 4], points: &[[f64; 2]]) -> [f64; 4] {
    let width = box_width(label_bbox).clamp(0.08, 0.16);
    let height = box_height(label_bbox).clamp(0.04, 0.07);
    let rail_y = points[0][1];
    let center_x = ((points[0][0] + points[1][0]) / 2.0).clamp(width / 2.0, 1.0 - width / 2.0);
    let above_y = rail_y - height - 0.02;
    let y1 = if above_y >= 0.02 {
        above_y
    } else {
        rail_y + 0.02
    }
    .clamp(0.02, 0.98 - height);
    [
        center_x - width / 2.0,
        y1,
        center_x + width / 2.0,
        y1 + height,
    ]
}

fn simplify_residual_objective_to_main_edges(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    for object in &mut plan.objects {
        let DrawObject::Connector {
            id,
            points,
            from,
            to,
            style,
            ..
        } = object
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(from_box), Some(to_box)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        if !is_residual_to_main_supervision_edge(id, style, from_id, from_box, to_box)
            || points.len() <= 3
            || vertical_separation(from_box.bbox, to_box.bbox) < 0.04
            || horizontal_separation(from_box.bbox, to_box.bbox) < 0.03
        {
            continue;
        }
        *points = compact_objective_to_main_route(from_box.bbox, to_box.bbox);
    }
}

fn remove_redundant_residual_supervision_labels(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    for object in &mut plan.objects {
        let DrawObject::Connector {
            id,
            from,
            to,
            style,
            label,
            ..
        } = object
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(from_box), Some(to_box)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        if is_residual_to_main_supervision_edge(id, style, from_id, from_box, to_box)
            && label
                .as_ref()
                .is_some_and(|label| is_generic_residual_supervision_label(&label.text))
        {
            *label = None;
        }
    }
}

fn remove_redundant_task_loss_connector_labels(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    for object in &mut plan.objects {
        let DrawObject::Connector {
            from, to, label, ..
        } = object
        else {
            continue;
        };
        let Some(label_text) = label.as_ref().map(|label| label.text.as_str()) else {
            continue;
        };
        let endpoint_boxes = [from.as_deref(), to.as_deref()]
            .into_iter()
            .flatten()
            .filter_map(|id| box_map.get(id))
            .collect::<Vec<_>>();
        if endpoint_boxes.iter().any(|info| {
            is_task_loss_route_box("", info)
                && task_loss_box_already_contains_edge_label(&info.text, label_text)
        }) {
            *label = None;
        }
    }
}

fn task_loss_box_already_contains_edge_label(loss_text: &str, label_text: &str) -> bool {
    let loss = loss_text.to_lowercase();
    let label = label_text.to_lowercase();
    let label_phrase = normalized_annotation_phrase(label_text);
    if label_phrase.is_empty() {
        return true;
    }
    if matches!(label_phrase.as_str(), "task loss" | "loss") && loss.contains("loss") {
        return true;
    }
    let label_mentions_targets =
        label.contains('y') || label.contains('ŷ') || label.contains("hat");
    let loss_mentions_targets = loss.contains('y') || loss.contains('ŷ') || loss.contains("hat");
    label_mentions_targets
        && loss_mentions_targets
        && (loss.contains("ce") || loss.contains("loss"))
}

fn is_residual_to_main_supervision_edge(
    id: &str,
    style: &str,
    from_id: &str,
    from_box: &BoxRouteInfo,
    to_box: &BoxRouteInfo,
) -> bool {
    let descriptor = format!(
        "{} {} {}",
        id.to_lowercase(),
        style.to_lowercase(),
        route_box_text(from_id, from_box)
    );
    descriptor.contains("residual")
        && (descriptor.contains("supervision")
            || descriptor.contains("supervise")
            || descriptor.contains("dash"))
        && is_objective_route_box(from_id, from_box)
        && is_main_route_box(to_box)
}

fn is_generic_residual_supervision_label(label: &str) -> bool {
    matches!(
        normalized_annotation_phrase(label).as_str(),
        "supervise" | "supervision" | "residual signal" | "latent signal"
    )
}

fn compact_objective_to_main_route(from_box: [f64; 4], to_box: [f64; 4]) -> Vec<[f64; 2]> {
    let from_left = center_x(from_box) < center_x(to_box);
    let from_above = center_y(from_box) < center_y(to_box);
    let start_x = if from_left { from_box[2] } else { from_box[0] };
    let end_x = if from_left { to_box[0] } else { to_box[2] };
    let start_y = if from_above { from_box[3] } else { from_box[1] };
    let end_y = if from_above { to_box[1] } else { to_box[3] };
    vec![[start_x, start_y], [end_x, start_y], [end_x, end_y]]
}

fn simplify_main_to_residual_supervision_edges(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    for object in &mut plan.objects {
        let DrawObject::Connector {
            id,
            points,
            from,
            to,
            style,
            ..
        } = object
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(from_box), Some(to_box)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        if !is_main_to_residual_supervision_edge(id, style, from_box, to_id, to_box)
            || points.len() <= 3
        {
            continue;
        }
        *points = compact_main_to_residual_supervision_route(from_box.bbox, to_box.bbox);
    }
}

fn is_main_to_residual_supervision_edge(
    id: &str,
    style: &str,
    from_box: &BoxRouteInfo,
    to_id: &str,
    to_box: &BoxRouteInfo,
) -> bool {
    let descriptor = format!(
        "{} {} {}",
        id.to_lowercase(),
        style.to_lowercase(),
        route_box_text(to_id, to_box)
    );
    is_main_route_box(from_box)
        && descriptor.contains("residual")
        && (descriptor.contains("supervision")
            || descriptor.contains("l_res")
            || descriptor.contains("dash"))
}

fn compact_main_to_residual_supervision_route(
    from_box: [f64; 4],
    to_box: [f64; 4],
) -> Vec<[f64; 2]> {
    let from_above = center_y(from_box) <= center_y(to_box);
    let start = if from_above {
        [center_x(from_box), from_box[3]]
    } else {
        [center_x(from_box), from_box[1]]
    };
    let end = if center_x(from_box) <= center_x(to_box) {
        [to_box[0], center_y(to_box)]
    } else {
        [to_box[2], center_y(to_box)]
    };
    remove_redundant_collinear_points(&[start, [start[0], end[1]], end])
}

fn simplify_adjacent_output_loss_connectors(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    for object in &mut plan.objects {
        let DrawObject::Connector {
            points, from, to, ..
        } = object
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(output), Some(loss)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        if !is_output_route_box(from_id, output)
            || !is_task_loss_route_box(to_id, loss)
            || points.len() <= 3
        {
            continue;
        }
        let vertically_aligned = vertical_separation(output.bbox, loss.bbox) > 0.03
            && horizontal_separation(output.bbox, loss.bbox) <= 0.02;
        let adjacent = horizontal_separation(output.bbox, loss.bbox) <= 0.18
            && vertical_separation(output.bbox, loss.bbox) <= 0.03;
        if !vertically_aligned && !adjacent {
            continue;
        }
        if vertically_aligned {
            if let Some(x) = common_vertical_x(output.bbox, loss.bbox) {
                let candidate = vertical_connector_points(output.bbox, loss.bbox, x);
                if !connector_points_intersect_intermediate_boxes(
                    &candidate, from_id, to_id, &box_map,
                ) {
                    *points = candidate;
                }
            } else {
                let candidate = orthogonal_connector_points_between_boxes(output.bbox, loss.bbox);
                if !connector_points_intersect_intermediate_boxes(
                    &candidate, from_id, to_id, &box_map,
                ) {
                    *points = candidate;
                }
            }
        } else if let Some(y) = common_horizontal_y(output.bbox, loss.bbox) {
            let candidate = horizontal_connector_points(output.bbox, loss.bbox, y);
            if !connector_points_intersect_intermediate_boxes(&candidate, from_id, to_id, &box_map)
            {
                *points = candidate;
            }
        } else {
            let candidate = orthogonal_connector_points_between_boxes(output.bbox, loss.bbox);
            if !connector_points_intersect_intermediate_boxes(&candidate, from_id, to_id, &box_map)
            {
                *points = candidate;
            }
        }
    }
}

fn simplify_student_to_task_loss_connectors(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let mut updates = Vec::new();
    for object in &plan.objects {
        let DrawObject::Connector {
            id,
            points,
            from,
            to,
            label,
            ..
        } = object
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(student), Some(loss)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        if !is_main_route_box(student)
            || !route_box_text(from_id, student).contains("student")
            || !is_task_loss_route_box(to_id, loss)
        {
            continue;
        }

        let mut replacement_points = None;
        if student_task_loss_route_needs_simplification(points, student.bbox, loss.bbox) {
            let candidate = local_student_task_loss_route(student.bbox, loss.bbox);
            if !connector_points_intersect_intermediate_boxes(
                candidate.as_slice(),
                from_id,
                to_id,
                &box_map,
            ) && !connector_route_conflicts_with_other_connectors(candidate.as_slice(), id, plan)
            {
                replacement_points = Some(candidate);
            }
        }

        let remove_label = replacement_points.is_some()
            && label
                .as_ref()
                .is_some_and(|label| is_generic_prediction_target_loss_label(&label.text))
            && route_box_text(to_id, loss).contains("loss");
        if replacement_points.is_some() || remove_label {
            updates.push((id.clone(), replacement_points, remove_label));
        }
    }

    for (target_id, replacement_points, remove_label) in updates {
        let Some(DrawObject::Connector { points, label, .. }) = plan
            .objects
            .iter_mut()
            .find(|object| draw_object_id(object) == target_id)
        else {
            continue;
        };
        if let Some(next_points) = replacement_points {
            *points = next_points;
        }
        if remove_label {
            *label = None;
        }
    }
}

fn student_task_loss_route_needs_simplification(
    points: &[[f64; 2]],
    student: [f64; 4],
    loss: [f64; 4],
) -> bool {
    let route_box = points_to_box(points);
    let local_box = union_box(student, loss);
    let outer_detour = route_box[2] > local_box[2] + 0.08
        || route_box[0] + 0.08 < local_box[0]
        || route_box[3] > local_box[3] + 0.08
        || route_box[1] + 0.08 < local_box[1];
    let excessive_extent = box_width(route_box) > box_width(local_box) + 0.12
        || box_height(route_box) > box_height(local_box) + 0.12;
    outer_detour || (points.len() > 4 && excessive_extent)
}

fn local_student_task_loss_route(student: [f64; 4], loss: [f64; 4]) -> Vec<[f64; 2]> {
    let loss_y = center_y(loss);
    let loss_to_right = center_x(student) <= center_x(loss);
    if loss_to_right {
        let end = [loss[0], loss_y];
        let start = if loss_y < student[1] {
            [(student[2] - 0.045).max(student[0] + 0.035), student[1]]
        } else if loss_y > student[3] {
            [(student[2] - 0.045).max(student[0] + 0.035), student[3]]
        } else {
            [student[2], loss_y]
        };
        let elbow_x = if loss[0] - student[2] > 0.055 {
            ((student[2] + loss[0]) / 2.0).clamp(student[2] + 0.025, loss[0] - 0.025)
        } else {
            (student[2] + 0.03).min(0.96)
        };
        return remove_redundant_collinear_points(&[
            start,
            [elbow_x, start[1]],
            [elbow_x, end[1]],
            end,
        ]);
    }

    let end = [loss[2], loss_y];
    let start = if loss_y < student[1] {
        [(student[0] + 0.045).min(student[2] - 0.035), student[1]]
    } else if loss_y > student[3] {
        [(student[0] + 0.045).min(student[2] - 0.035), student[3]]
    } else {
        [student[0], loss_y]
    };
    let elbow_x = if student[0] - loss[2] > 0.055 {
        ((student[0] + loss[2]) / 2.0).clamp(loss[2] + 0.025, student[0] - 0.025)
    } else {
        (student[0] - 0.03).max(0.04)
    };
    remove_redundant_collinear_points(&[start, [elbow_x, start[1]], [elbow_x, end[1]], end])
}

fn is_generic_prediction_target_loss_label(text: &str) -> bool {
    let normalized = text
        .to_lowercase()
        .replace('ŷ', "yhat")
        .replace('–', "-")
        .replace('—', "-");
    let compact = normalized
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .collect::<String>();
    compact.len() <= 10 && compact.contains('y') && (compact.contains("hat") || text.contains('ŷ'))
}

fn task_loss_route_avoiding_supervision_edges(
    moving: &ConnectorRouteSnapshot,
    plan: &DrawPlan,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<Vec<[f64; 2]>> {
    let (Some(from_id), Some(to_id)) = (moving.from.as_deref(), moving.to.as_deref()) else {
        return None;
    };
    let (Some(from_box), Some(to_box)) = (box_map.get(from_id), box_map.get(to_id)) else {
        return None;
    };
    let candidates = task_loss_side_route_candidates(
        moving.points.as_slice(),
        from_box.bbox,
        to_box.bbox,
        box_map,
    )
    .into_iter()
    .map(|candidate| remove_redundant_collinear_points(&candidate))
    .filter(|candidate| candidate.len() >= 2)
    .collect::<Vec<_>>();
    let strict = candidates
        .iter()
        .filter(|candidate| {
            !connector_points_intersect_intermediate_boxes(candidate, from_id, to_id, box_map)
                && !connector_route_conflicts_with_other_connectors(candidate, &moving.id, plan)
        })
        .min_by(|left, right| connector_route_score(left).total_cmp(&connector_route_score(right)))
        .cloned();
    if strict.is_some() {
        return strict;
    }

    let blockers = connector_route_snapshots(plan)
        .into_iter()
        .filter(|other| {
            other.id != moving.id
                && (connector_routes_conflict(&moving.points, &other.points)
                    || is_student_residual_connector_route(other))
        })
        .collect::<Vec<_>>();
    candidates
        .into_iter()
        .filter(|candidate| {
            !connector_points_intersect_intermediate_boxes(candidate, from_id, to_id, box_map)
                && blockers
                    .iter()
                    .all(|blocker| !connector_routes_conflict(candidate, &blocker.points))
        })
        .min_by(|left, right| connector_route_score(left).total_cmp(&connector_route_score(right)))
}

fn task_loss_side_route_candidates(
    current_points: &[[f64; 2]],
    from_box: [f64; 4],
    to_box: [f64; 4],
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Vec<Vec<[f64; 2]>> {
    let start = current_points
        .first()
        .copied()
        .unwrap_or_else(|| anchor_point_towards(from_box, center(to_box)));
    let current_end = current_points
        .last()
        .copied()
        .unwrap_or_else(|| anchor_point_towards(to_box, center(from_box)));
    let mut end_points = vec![
        current_end,
        [to_box[0], center_y(to_box)],
        [to_box[2], center_y(to_box)],
        [center_x(to_box), to_box[1]],
        [center_x(to_box), to_box[3]],
        [to_box[0], to_box[1]],
        [to_box[2], to_box[1]],
        [to_box[0], to_box[3]],
        [to_box[2], to_box[3]],
    ];
    end_points.dedup_by(|left, right| {
        (left[0] - right[0]).abs() < 0.002 && (left[1] - right[1]).abs() < 0.002
    });
    let mut candidates = Vec::new();
    for end in end_points {
        for x in vertical_rail_x_candidates(box_map) {
            candidates.push(vec![start, [x, start[1]], [x, end[1]], end]);
            let rail_y_values = if end[1] < start[1] {
                [0.03, 0.06, 0.10]
            } else {
                [0.97, 0.94, 0.90]
            };
            for rail_y in rail_y_values {
                candidates.push(vec![
                    start,
                    [x, start[1]],
                    [x, rail_y],
                    [end[0], rail_y],
                    end,
                ]);
            }
        }
    }
    candidates
}

fn supervision_route_avoiding_main_edges(
    moving: &ConnectorRouteSnapshot,
    plan: &DrawPlan,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<Vec<[f64; 2]>> {
    let (Some(from_id), Some(to_id)) = (moving.from.as_deref(), moving.to.as_deref()) else {
        return None;
    };
    let (Some(from_box), Some(to_box)) = (box_map.get(from_id), box_map.get(to_id)) else {
        return None;
    };
    supervision_rail_route_candidates(
        moving.points.as_slice(),
        from_box.bbox,
        to_box.bbox,
        box_map,
    )
    .into_iter()
    .map(|candidate| remove_redundant_collinear_points(&candidate))
    .filter(|candidate| {
        candidate.len() >= 2
            && !connector_points_intersect_intermediate_boxes(candidate, from_id, to_id, box_map)
            && !connector_route_conflicts_with_other_connectors(candidate, &moving.id, plan)
    })
    .min_by(|left, right| connector_route_score(left).total_cmp(&connector_route_score(right)))
}

fn supervision_rail_route_candidates(
    current_points: &[[f64; 2]],
    from_box: [f64; 4],
    to_box: [f64; 4],
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Vec<Vec<[f64; 2]>> {
    let start = current_points
        .first()
        .copied()
        .unwrap_or_else(|| anchor_point_towards(from_box, center(to_box)));
    let current_end = current_points
        .last()
        .copied()
        .unwrap_or_else(|| anchor_point_towards(to_box, center(from_box)));
    let mut end_points = vec![
        current_end,
        [to_box[0], center_y(to_box)],
        [to_box[2], center_y(to_box)],
        [center_x(to_box), to_box[1]],
        [center_x(to_box), to_box[3]],
        [to_box[0], to_box[1]],
        [to_box[2], to_box[1]],
        [to_box[0], to_box[3]],
        [to_box[2], to_box[3]],
    ];
    end_points.dedup_by(|left, right| {
        (left[0] - right[0]).abs() < 0.002 && (left[1] - right[1]).abs() < 0.002
    });
    let mut candidates = Vec::new();
    for end in end_points {
        let rail_y = if end[1] < start[1] { 0.06 } else { 0.94 };
        let backup_rail_y = if end[1] < start[1] { 0.10 } else { 0.90 };
        for x in vertical_rail_x_candidates(box_map) {
            candidates.push(vec![
                start,
                [x, start[1]],
                [x, rail_y],
                [end[0], rail_y],
                end,
            ]);
            candidates.push(vec![
                start,
                [x, start[1]],
                [x, backup_rail_y],
                [end[0], backup_rail_y],
                end,
            ]);
            candidates.push(vec![start, [x, start[1]], [x, end[1]], end]);
        }
    }
    candidates
}

fn vertical_rail_x_candidates(box_map: &HashMap<String, BoxRouteInfo>) -> Vec<f64> {
    let mut values = vec![0.04, 0.08, 0.92, 0.96];
    let mut intervals = box_map
        .values()
        .map(|info| {
            let bbox = normalize_box(info.bbox);
            values.push(bbox[0] - 0.035);
            values.push(bbox[2] + 0.035);
            (bbox[0], bbox[2])
        })
        .collect::<Vec<_>>();
    intervals.sort_by(|left, right| left.0.total_cmp(&right.0));
    let mut cursor = 0.02;
    for (start, end) in intervals {
        if start - cursor > 0.012 {
            values.push((cursor + start) / 2.0);
        }
        cursor = cursor.max(end);
    }
    if 0.98 - cursor > 0.012 {
        values.push((cursor + 0.98) / 2.0);
    }
    values.retain(|x| (0.02..=0.98).contains(x));
    values.sort_by(|left, right| left.total_cmp(right));
    values.dedup_by(|left, right| (*left - *right).abs() < 0.002);
    values
}

fn is_supervision_connector_route(route: &ConnectorRouteSnapshot) -> bool {
    let text = format!(
        "{} {} {} {}",
        route.id.to_lowercase(),
        route.style.to_lowercase(),
        route.from.as_deref().unwrap_or_default().to_lowercase(),
        route.to.as_deref().unwrap_or_default().to_lowercase()
    );
    (text.contains("supervision")
        || text.contains("residual")
        || text.contains("teacher")
        || text.contains("dash"))
        && !is_main_connector_route(route)
}

fn is_main_connector_route(route: &ConnectorRouteSnapshot) -> bool {
    let text = format!(
        "{} {} {} {}",
        route.id.to_lowercase(),
        route.style.to_lowercase(),
        route.from.as_deref().unwrap_or_default().to_lowercase(),
        route.to.as_deref().unwrap_or_default().to_lowercase()
    );
    text.contains("main")
        || text.contains("output")
        || text.contains("head_out")
        || text.contains("student_head")
}

fn is_task_or_loss_connector_route(route: &ConnectorRouteSnapshot) -> bool {
    let text = format!(
        "{} {} {} {}",
        route.id.to_lowercase(),
        route.style.to_lowercase(),
        route.from.as_deref().unwrap_or_default().to_lowercase(),
        route.to.as_deref().unwrap_or_default().to_lowercase()
    );
    text.contains("task") || text.contains("loss")
}

fn is_student_residual_connector_route(route: &ConnectorRouteSnapshot) -> bool {
    let text = format!(
        "{} {} {} {}",
        route.id.to_lowercase(),
        route.style.to_lowercase(),
        route.from.as_deref().unwrap_or_default().to_lowercase(),
        route.to.as_deref().unwrap_or_default().to_lowercase()
    );
    text.contains("student") && text.contains("residual")
}

fn is_residual_objective_connector_route(route: &ConnectorRouteSnapshot) -> bool {
    let text = format!(
        "{} {} {} {}",
        route.id.to_lowercase(),
        route.style.to_lowercase(),
        route.from.as_deref().unwrap_or_default().to_lowercase(),
        route.to.as_deref().unwrap_or_default().to_lowercase()
    );
    (text.contains("residual") || text.contains("supervision") || text.contains("dash"))
        && (text.contains("loss") || text.contains("objective") || text.contains("residual"))
        && !(text.contains("task") && text.contains("loss"))
        && !is_main_connector_route(route)
}

fn crossing_connector_pair_to_repair(plan: &DrawPlan) -> Option<(String, String)> {
    let connectors = connector_route_snapshots(plan);
    for (left_index, left) in connectors.iter().enumerate() {
        for right in connectors.iter().skip(left_index + 1) {
            if !connector_routes_conflict(left.points.as_slice(), right.points.as_slice()) {
                continue;
            }
            let moving = if left.stability <= right.stability {
                left
            } else {
                right
            };
            let blocker = if moving.id == left.id { right } else { left };
            return Some((moving.id.clone(), blocker.id.clone()));
        }
    }
    None
}

fn candidate_route_avoiding_crossing_edges(
    moving_id: &str,
    blocker_id: &str,
    plan: &DrawPlan,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<Vec<[f64; 2]>> {
    let moving = connector_route_snapshot(plan, moving_id)?;
    let (Some(from_id), Some(to_id)) = (moving.from.as_deref(), moving.to.as_deref()) else {
        return None;
    };
    let (Some(from_box), Some(to_box)) = (box_map.get(from_id), box_map.get(to_id)) else {
        return None;
    };
    let mut candidates = Vec::new();
    if let Some(clean_points) = clean_connector_points_for_boxes(from_id, from_box, to_id, to_box) {
        candidates.push(clean_points);
    }
    candidates.extend(horizontal_dogleg_candidates_around_boxes(
        from_box.bbox,
        to_box.bbox,
        from_id,
        to_id,
        box_map,
    ));
    candidates.extend(vertical_dogleg_candidates_around_boxes(
        from_box.bbox,
        to_box.bbox,
        from_id,
        to_id,
        box_map,
    ));
    candidates.extend(connector_crossing_escape_candidates(
        from_box.bbox,
        to_box.bbox,
        plan,
        moving_id,
        blocker_id,
    ));

    candidates
        .into_iter()
        .map(|candidate| remove_redundant_collinear_points(&candidate))
        .filter(|candidate| {
            candidate.len() >= 2
                && !connector_points_intersect_intermediate_boxes(
                    candidate, from_id, to_id, box_map,
                )
                && !connector_route_conflicts_with_other_connectors(candidate, moving_id, plan)
        })
        .min_by(|left, right| connector_route_score(left).total_cmp(&connector_route_score(right)))
}

fn connector_crossing_escape_candidates(
    from_box: [f64; 4],
    to_box: [f64; 4],
    plan: &DrawPlan,
    moving_id: &str,
    blocker_id: &str,
) -> Vec<Vec<[f64; 2]>> {
    let Some(blocker) = connector_route_snapshot(plan, blocker_id) else {
        return Vec::new();
    };
    let from_left = center_x(from_box) < center_x(to_box);
    let start_x = if from_left { from_box[2] } else { from_box[0] };
    let end_x = if from_left { to_box[0] } else { to_box[2] };
    let start_y = center_y(from_box);
    let end_y = center_y(to_box);
    let from_above = center_y(from_box) < center_y(to_box);
    let start_v_y = if from_above { from_box[3] } else { from_box[1] };
    let end_v_y = if from_above { to_box[1] } else { to_box[3] };
    let start_v_x = center_x(from_box);
    let end_v_x = center_x(to_box);
    let blocker_box = points_to_box(blocker.points.as_slice());
    let outer_x = if center_x(to_box) < center_x(from_box) {
        0.04
    } else {
        0.96
    };
    let outer_y = if center_y(to_box) < center_y(from_box) {
        0.04
    } else {
        0.96
    };
    let mut candidates = Vec::new();

    let mut y_values = vec![blocker_box[1] - 0.035, blocker_box[3] + 0.035, 0.08, 0.92];
    let mut x_values = vec![blocker_box[0] - 0.035, blocker_box[2] + 0.035, 0.08, 0.92];
    for object in &plan.objects {
        let DrawObject::Box { bbox, .. } = object else {
            continue;
        };
        y_values.push(bbox[1] - 0.035);
        y_values.push(bbox[3] + 0.035);
        x_values.push(bbox[0] - 0.035);
        x_values.push(bbox[2] + 0.035);
    }
    y_values.sort_by(|left, right| left.total_cmp(right));
    y_values.dedup_by(|left, right| (*left - *right).abs() < 0.002);
    x_values.sort_by(|left, right| left.total_cmp(right));
    x_values.dedup_by(|left, right| (*left - *right).abs() < 0.002);

    for y in y_values {
        if !(0.02..=0.98).contains(&y) {
            continue;
        }
        candidates.push(remove_redundant_collinear_points(&[
            [start_x, start_y],
            [start_x, y],
            [end_x, y],
            [end_x, end_y],
        ]));
        candidates.push(remove_redundant_collinear_points(&[
            [start_x, start_y],
            [start_x, y],
            [outer_x, y],
            [outer_x, end_y],
            [end_x, end_y],
        ]));
    }
    for x in x_values {
        if !(0.02..=0.98).contains(&x) {
            continue;
        }
        candidates.push(remove_redundant_collinear_points(&[
            [start_v_x, start_v_y],
            [x, start_v_y],
            [x, end_v_y],
            [end_v_x, end_v_y],
        ]));
        candidates.push(remove_redundant_collinear_points(&[
            [start_v_x, start_v_y],
            [x, start_v_y],
            [x, outer_y],
            [end_v_x, outer_y],
            [end_v_x, end_v_y],
        ]));
    }
    if moving_id.contains("task") || moving_id.contains("loss") {
        candidates.push(remove_redundant_collinear_points(&[
            [start_x, start_y],
            [start_x, 0.92],
            [end_x, 0.92],
            [end_x, end_y],
        ]));
    }
    candidates
}

fn connector_route_conflicts_with_other_connectors(
    candidate: &[[f64; 2]],
    moving_id: &str,
    plan: &DrawPlan,
) -> bool {
    connector_route_snapshots(plan)
        .into_iter()
        .any(|other| other.id != moving_id && connector_routes_conflict(candidate, &other.points))
}

fn connector_routes_conflict(left: &[[f64; 2]], right: &[[f64; 2]]) -> bool {
    left.windows(2).any(|left_window| {
        right.windows(2).any(|right_window| {
            segments_cross(
                (left_window[0], left_window[1]),
                (right_window[0], right_window[1]),
            ) || collinear_segment_overlap(
                left_window[0],
                left_window[1],
                right_window[0],
                right_window[1],
            )
        })
    })
}

fn collinear_segment_overlap(
    left_start: [f64; 2],
    left_end: [f64; 2],
    right_start: [f64; 2],
    right_end: [f64; 2],
) -> bool {
    if same_y(left_start, left_end)
        && same_y(right_start, right_end)
        && (left_start[1] - right_start[1]).abs() < 0.0001
    {
        let overlap_start = left_start[0]
            .min(left_end[0])
            .max(right_start[0].min(right_end[0]));
        let overlap_end = left_start[0]
            .max(left_end[0])
            .min(right_start[0].max(right_end[0]));
        return overlap_end - overlap_start > 0.012;
    }
    if same_x(left_start, left_end)
        && same_x(right_start, right_end)
        && (left_start[0] - right_start[0]).abs() < 0.0001
    {
        let overlap_start = left_start[1]
            .min(left_end[1])
            .max(right_start[1].min(right_end[1]));
        let overlap_end = left_start[1]
            .max(left_end[1])
            .min(right_start[1].max(right_end[1]));
        return overlap_end - overlap_start > 0.012;
    }
    false
}

#[derive(Clone, Debug)]
struct ConnectorRouteSnapshot {
    id: String,
    points: Vec<[f64; 2]>,
    from: Option<String>,
    to: Option<String>,
    style: String,
    stability: i32,
}

fn connector_route_snapshots(plan: &DrawPlan) -> Vec<ConnectorRouteSnapshot> {
    plan.objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Connector {
                id,
                points,
                from,
                to,
                style,
                ..
            } = object
            else {
                return None;
            };
            Some(ConnectorRouteSnapshot {
                id: id.clone(),
                points: points.clone(),
                from: from.clone(),
                to: to.clone(),
                style: style.clone(),
                stability: connector_route_stability(id, style, from.as_deref(), to.as_deref()),
            })
        })
        .collect()
}

fn connector_route_snapshot(plan: &DrawPlan, id: &str) -> Option<ConnectorRouteSnapshot> {
    connector_route_snapshots(plan)
        .into_iter()
        .find(|connector| connector.id == id)
}

fn connector_route_stability(id: &str, style: &str, from: Option<&str>, to: Option<&str>) -> i32 {
    let text = format!(
        "{} {} {} {}",
        id.to_lowercase(),
        style.to_lowercase(),
        from.unwrap_or_default().to_lowercase(),
        to.unwrap_or_default().to_lowercase()
    );
    let mut score = 0;
    if text.contains("main") || text.contains("output") || text.contains("head_out") {
        score += 12;
    }
    if text.contains("input") {
        score += 5;
    }
    if text.contains("residual") || text.contains("supervision") || text.contains("teacher") {
        score += 4;
    }
    if text.contains("task") || text.contains("loss") {
        score -= 4;
    }
    score
}

fn connector_points_around_intermediate_boxes(
    from_id: &str,
    from_box: &BoxRouteInfo,
    to_id: &str,
    to_box: &BoxRouteInfo,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Option<Vec<[f64; 2]>> {
    let mut candidates = Vec::new();
    candidates.extend(horizontal_dogleg_candidates_around_boxes(
        from_box.bbox,
        to_box.bbox,
        from_id,
        to_id,
        box_map,
    ));
    candidates.extend(vertical_dogleg_candidates_around_boxes(
        from_box.bbox,
        to_box.bbox,
        from_id,
        to_id,
        box_map,
    ));

    candidates
        .into_iter()
        .filter(|points| {
            points.len() >= 2
                && !connector_points_intersect_intermediate_boxes(points, from_id, to_id, box_map)
        })
        .min_by(|left, right| connector_route_score(left).total_cmp(&connector_route_score(right)))
}

fn horizontal_dogleg_candidates_around_boxes(
    from_box: [f64; 4],
    to_box: [f64; 4],
    from_id: &str,
    to_id: &str,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Vec<Vec<[f64; 2]>> {
    let from_left = center_x(from_box) < center_x(to_box);
    let start_x = if from_left { from_box[2] } else { from_box[0] };
    let end_x = if from_left { to_box[0] } else { to_box[2] };
    let start_y = center_y(from_box);
    let end_y = center_y(to_box);
    let min_x = start_x.min(end_x);
    let max_x = start_x.max(end_x);
    let mut y_candidates = vec![start_y, end_y, (start_y + end_y) / 2.0, 0.08, 0.92];
    for (id, info) in box_map {
        if id == from_id || id == to_id || !ranges_overlap(min_x, max_x, info.bbox[0], info.bbox[2])
        {
            continue;
        }
        y_candidates.push(info.bbox[1] - 0.025);
        y_candidates.push(info.bbox[3] + 0.025);
    }

    y_candidates
        .into_iter()
        .filter(|y| *y >= 0.02 && *y <= 0.98)
        .map(|y| {
            remove_redundant_collinear_points(&[
                [start_x, start_y],
                [start_x, y],
                [end_x, y],
                [end_x, end_y],
            ])
        })
        .collect()
}

fn vertical_dogleg_candidates_around_boxes(
    from_box: [f64; 4],
    to_box: [f64; 4],
    from_id: &str,
    to_id: &str,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> Vec<Vec<[f64; 2]>> {
    let from_above = center_y(from_box) < center_y(to_box);
    let start_y = if from_above { from_box[3] } else { from_box[1] };
    let end_y = if from_above { to_box[1] } else { to_box[3] };
    let start_x = center_x(from_box);
    let end_x = center_x(to_box);
    let min_y = start_y.min(end_y);
    let max_y = start_y.max(end_y);
    let mut x_candidates = vec![start_x, end_x, (start_x + end_x) / 2.0, 0.08, 0.92];
    for (id, info) in box_map {
        if id == from_id || id == to_id || !ranges_overlap(min_y, max_y, info.bbox[1], info.bbox[3])
        {
            continue;
        }
        x_candidates.push(info.bbox[0] - 0.025);
        x_candidates.push(info.bbox[2] + 0.025);
    }

    x_candidates
        .into_iter()
        .filter(|x| *x >= 0.02 && *x <= 0.98)
        .map(|x| {
            remove_redundant_collinear_points(&[
                [start_x, start_y],
                [x, start_y],
                [x, end_y],
                [end_x, end_y],
            ])
        })
        .collect()
}

fn ranges_overlap(left_start: f64, left_end: f64, right_start: f64, right_end: f64) -> bool {
    left_start.max(right_start) < left_end.min(right_end)
}

fn connector_route_score(points: &[[f64; 2]]) -> f64 {
    polyline_length(points)
        + edge_pressure(points_to_box(points)) * 0.2
        + points.len() as f64 * 0.01
}

fn reroute_objective_feedback_away_from_reverse_shared_segments(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    let segments = connector_segments_with_ids(plan);
    for object in &mut plan.objects {
        let DrawObject::Connector {
            id,
            points,
            from,
            to,
            ..
        } = object
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(from_box), Some(to_box)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        if !is_objective_route_box(from_id, from_box) || !is_main_route_box(to_box) {
            continue;
        }
        if connector_has_reverse_shared_segment(id, points, &segments) {
            *points = objective_to_main_vertical_first_connector_points(from_box.bbox, to_box.bbox);
        }
    }
}

fn reroute_output_to_task_loss_around_intermediate_boxes(plan: &mut DrawPlan) {
    let box_map = current_box_route_info_map(plan);
    for object in &mut plan.objects {
        let DrawObject::Connector {
            points, from, to, ..
        } = object
        else {
            continue;
        };
        let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
            continue;
        };
        let (Some(output), Some(task_loss)) = (box_map.get(from_id), box_map.get(to_id)) else {
            continue;
        };
        if !is_output_route_box(from_id, output) || !is_task_loss_route_box(to_id, task_loss) {
            continue;
        }
        if vertical_separation(output.bbox, task_loss.bbox) < 0.10
            || horizontal_separation(output.bbox, task_loss.bbox) > 0.02
        {
            continue;
        }
        if connector_points_intersect_intermediate_boxes(points, from_id, to_id, &box_map) {
            *points = output_to_task_loss_side_connector_points(output.bbox, task_loss.bbox);
        }
    }
}

fn connector_points_intersect_intermediate_boxes(
    points: &[[f64; 2]],
    from_id: &str,
    to_id: &str,
    box_map: &HashMap<String, BoxRouteInfo>,
) -> bool {
    points.windows(2).any(|window| {
        let segment_box = expand_box(
            [
                window[0][0].min(window[1][0]),
                window[0][1].min(window[1][1]),
                window[0][0].max(window[1][0]),
                window[0][1].max(window[1][1]),
            ],
            0.006,
        );
        box_map.iter().any(|(id, info)| {
            id != from_id && id != to_id && intersection_area(segment_box, info.bbox) > 0.0001
        })
    })
}

fn output_to_task_loss_side_connector_points(
    output_box: [f64; 4],
    task_loss_box: [f64; 4],
) -> Vec<[f64; 2]> {
    let use_right_side = output_box[2].max(task_loss_box[2]) <= 0.92;
    let side_x = if use_right_side {
        (output_box[2].max(task_loss_box[2]) + 0.03).min(0.96)
    } else {
        (output_box[0].min(task_loss_box[0]) - 0.03).max(0.04)
    };
    let start_x = if use_right_side {
        output_box[2]
    } else {
        output_box[0]
    };
    let end_x = if use_right_side {
        task_loss_box[2]
    } else {
        task_loss_box[0]
    };
    let start_y = center_y(output_box);
    let end_y = center_y(task_loss_box);
    remove_redundant_collinear_points(&[
        [start_x, start_y],
        [side_x, start_y],
        [side_x, end_y],
        [end_x, end_y],
    ])
}

fn connector_segments_with_ids(plan: &DrawPlan) -> Vec<(String, [f64; 2], [f64; 2])> {
    plan.objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Connector { id, points, .. } = object else {
                return None;
            };
            Some(
                points
                    .windows(2)
                    .map(|window| (id.clone(), window[0], window[1]))
                    .collect::<Vec<_>>(),
            )
        })
        .flatten()
        .collect()
}

fn connector_has_reverse_shared_segment(
    connector_id: &str,
    points: &[[f64; 2]],
    segments: &[(String, [f64; 2], [f64; 2])],
) -> bool {
    points.windows(2).any(|window| {
        segments.iter().any(|(other_id, start, end)| {
            other_id != connector_id
                && reversed_collinear_segment_overlap(window[0], window[1], *start, *end)
        })
    })
}

fn reversed_collinear_segment_overlap(
    left_start: [f64; 2],
    left_end: [f64; 2],
    right_start: [f64; 2],
    right_end: [f64; 2],
) -> bool {
    if same_y(left_start, left_end)
        && same_y(right_start, right_end)
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
    if same_x(left_start, left_end)
        && same_x(right_start, right_end)
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

fn clean_connector_points_for_boxes(
    from_id: &str,
    from_box: &BoxRouteInfo,
    _to_id: &str,
    to_box: &BoxRouteInfo,
) -> Option<Vec<[f64; 2]>> {
    if vertical_separation(from_box.bbox, to_box.bbox) > 0.04 {
        if let Some(x) = common_vertical_x(from_box.bbox, to_box.bbox) {
            return Some(vertical_connector_points(from_box.bbox, to_box.bbox, x));
        }
        if is_objective_route_box(from_id, from_box) && is_main_route_box(to_box) {
            return Some(objective_to_main_connector_points(
                from_box.bbox,
                to_box.bbox,
            ));
        }
        if is_input_route_box(from_id, from_box) {
            return Some(input_branch_connector_points(from_box.bbox, to_box.bbox));
        }
    }
    if horizontal_separation(from_box.bbox, to_box.bbox) > 0.04 {
        if let Some(y) = common_horizontal_y(from_box.bbox, to_box.bbox) {
            return Some(horizontal_connector_points(from_box.bbox, to_box.bbox, y));
        }
    }
    None
}

fn input_branch_connector_points(from_box: [f64; 4], to_box: [f64; 4]) -> Vec<[f64; 2]> {
    if horizontal_separation(from_box, to_box) > 0.04 {
        let from_left = center_x(from_box) < center_x(to_box);
        let start_x = if from_left { from_box[2] } else { from_box[0] };
        let end_x = if from_left { to_box[0] } else { to_box[2] };
        let start_y = center_y(from_box);
        let end_y = center_y(to_box);
        return remove_redundant_collinear_points(&[
            [start_x, start_y],
            [start_x, end_y],
            [end_x, end_y],
        ]);
    }
    let start_x = center_x(from_box);
    let end_x = center_x(to_box);
    let from_above = center_y(from_box) < center_y(to_box);
    let start_y = if from_above { from_box[3] } else { from_box[1] };
    let end_y = if from_above { to_box[1] } else { to_box[3] };
    let elbow_y = (start_y + end_y) / 2.0;
    remove_redundant_collinear_points(&[
        [start_x, start_y],
        [start_x, elbow_y],
        [end_x, elbow_y],
        [end_x, end_y],
    ])
}

fn objective_to_main_connector_points(from_box: [f64; 4], to_box: [f64; 4]) -> Vec<[f64; 2]> {
    let from_left = center_x(from_box) < center_x(to_box);
    let start_x = if from_left { from_box[2] } else { from_box[0] };
    let start_y = center_y(from_box);
    let end_x = if from_left { to_box[0] } else { to_box[2] };
    let from_above = center_y(from_box) < center_y(to_box);
    let end_y = if from_above { to_box[1] } else { to_box[3] };
    remove_redundant_collinear_points(&[[start_x, start_y], [end_x, start_y], [end_x, end_y]])
}

fn objective_to_main_vertical_first_connector_points(
    from_box: [f64; 4],
    to_box: [f64; 4],
) -> Vec<[f64; 2]> {
    let from_left = center_x(from_box) < center_x(to_box);
    let start_x = if from_left { from_box[2] } else { from_box[0] };
    let from_above = center_y(from_box) < center_y(to_box);
    let start_y = if from_above { from_box[3] } else { from_box[1] };
    let end_x = if from_left { to_box[0] } else { to_box[2] };
    let end_y = center_y(to_box);
    remove_redundant_collinear_points(&[[start_x, start_y], [start_x, end_y], [end_x, end_y]])
}

fn vertical_connector_points(from_box: [f64; 4], to_box: [f64; 4], x: f64) -> Vec<[f64; 2]> {
    let from_above = center_y(from_box) < center_y(to_box);
    let start_y = if from_above { from_box[3] } else { from_box[1] };
    let end_y = if from_above { to_box[1] } else { to_box[3] };
    vec![clamp_point([x, start_y]), clamp_point([x, end_y])]
}

fn horizontal_connector_points(from_box: [f64; 4], to_box: [f64; 4], y: f64) -> Vec<[f64; 2]> {
    let from_left = center_x(from_box) < center_x(to_box);
    let start_x = if from_left { from_box[2] } else { from_box[0] };
    let end_x = if from_left { to_box[0] } else { to_box[2] };
    vec![clamp_point([start_x, y]), clamp_point([end_x, y])]
}

fn common_vertical_x(from_box: [f64; 4], to_box: [f64; 4]) -> Option<f64> {
    let to_x = center_x(to_box);
    if coordinate_inside(to_x, from_box[0], from_box[2]) {
        return Some(to_x);
    }
    let from_x = center_x(from_box);
    if coordinate_inside(from_x, to_box[0], to_box[2]) {
        return Some(from_x);
    }
    let overlap_start = from_box[0].max(to_box[0]);
    let overlap_end = from_box[2].min(to_box[2]);
    (overlap_end - overlap_start > 0.01).then_some((overlap_start + overlap_end) / 2.0)
}

fn common_horizontal_y(from_box: [f64; 4], to_box: [f64; 4]) -> Option<f64> {
    let to_y = center_y(to_box);
    if coordinate_inside(to_y, from_box[1], from_box[3]) {
        return Some(to_y);
    }
    let from_y = center_y(from_box);
    if coordinate_inside(from_y, to_box[1], to_box[3]) {
        return Some(from_y);
    }
    let overlap_start = from_box[1].max(to_box[1]);
    let overlap_end = from_box[3].min(to_box[3]);
    (overlap_end - overlap_start > 0.01).then_some((overlap_start + overlap_end) / 2.0)
}

fn coordinate_inside(value: f64, start: f64, end: f64) -> bool {
    value >= start - 0.0001 && value <= end + 0.0001
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

fn is_input_route_box(id: &str, info: &BoxRouteInfo) -> bool {
    let text = route_box_text(id, info);
    text.contains("input") || text.contains("source")
}

fn is_output_route_box(id: &str, info: &BoxRouteInfo) -> bool {
    is_output_like(id, &info.text, &info.role)
}

fn is_objective_route_box(id: &str, info: &BoxRouteInfo) -> bool {
    let text = route_box_text(id, info);
    text.contains("loss")
        || text.contains("residual")
        || text.contains("objective")
        || text.contains("alignment")
        || text.contains("accent")
}

fn is_main_route_box(info: &BoxRouteInfo) -> bool {
    let text = format!(
        "{} {} {}",
        info.text.to_lowercase(),
        info.role.to_lowercase(),
        info.style.to_lowercase()
    );
    text.contains("student") || text.contains("main") || text.contains("primary")
}

fn is_student_or_main_route_box(info: &BoxRouteInfo) -> bool {
    is_main_route_box(info)
}

fn is_teacher_or_context_route_box(id: &str, info: &BoxRouteInfo) -> bool {
    let text = route_box_text(id, info);
    text.contains("teacher")
        || text.contains("context")
        || text.contains("muted")
        || text.contains("frozen")
}

fn is_task_loss_route_box(id: &str, info: &BoxRouteInfo) -> bool {
    let text = route_box_text(id, info);
    text.contains("task") && text.contains("loss")
}

fn route_box_text(id: &str, info: &BoxRouteInfo) -> String {
    format!(
        "{} {} {} {}",
        id.to_lowercase(),
        info.text.to_lowercase(),
        info.role.to_lowercase(),
        info.style.to_lowercase()
    )
}

fn current_box_route_info_map(plan: &DrawPlan) -> HashMap<String, BoxRouteInfo> {
    plan.objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Box {
                id,
                bbox,
                text,
                role,
                style,
                ..
            } = object
            else {
                return None;
            };
            Some((
                id.clone(),
                BoxRouteInfo {
                    bbox: normalize_box(*bbox),
                    text: text.clone(),
                    role: role.clone(),
                    style: style.clone(),
                },
            ))
        })
        .collect()
}

fn rebalance_vertically_underutilized_main_group(plan: &mut DrawPlan) {
    let boxes = current_box_route_info_map(plan);
    let main_boxes = boxes
        .iter()
        .filter_map(|(id, info)| main_vertical_balance_box(id, info).then_some(info.bbox))
        .collect::<Vec<_>>();
    if main_boxes.len() < 4 {
        return;
    }
    let Some(union) = main_boxes.into_iter().reduce(union_box) else {
        return;
    };
    let x_span = box_width(union);
    let y_span = box_height(union);
    let top_gap = union[1].max(0.0);
    let bottom_gap = (1.0 - union[3]).max(0.0);
    let bottom_heavy = top_gap > 0.24 && top_gap > bottom_gap + 0.18;
    if x_span < 0.65 || y_span >= 0.70 || !bottom_heavy {
        return;
    }

    let desired_top = (1.0 - y_span) / 2.0;
    let desired_dy = desired_top - union[1];
    if !desired_dy.is_sign_negative() || desired_dy.abs() < 0.04 {
        return;
    }
    let Some((min_y, max_y)) = draw_plan_vertical_extent(plan) else {
        return;
    };
    let dy = desired_dy.clamp(0.02 - min_y, 0.98 - max_y);
    if dy.abs() < 0.025 {
        return;
    }
    translate_draw_plan_y(plan, dy);
}

fn main_vertical_balance_box(id: &str, info: &BoxRouteInfo) -> bool {
    let text = route_box_text(id, info);
    if text.contains("inference") || text.contains("note") || text.contains("annotation") {
        return false;
    }
    is_input_route_box(id, info)
        || is_student_or_main_route_box(info)
        || is_teacher_or_context_route_box(id, info)
        || is_output_route_box(id, info)
        || is_objective_route_box(id, info)
}

fn draw_plan_vertical_extent(plan: &DrawPlan) -> Option<(f64, f64)> {
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for object in &plan.objects {
        match object {
            DrawObject::Box { bbox, .. }
            | DrawObject::Text { bbox, .. }
            | DrawObject::Image { bbox, .. }
            | DrawObject::Group { bbox, .. } => {
                let bbox = normalize_box(*bbox);
                min_y = min_y.min(bbox[1]);
                max_y = max_y.max(bbox[3]);
            }
            DrawObject::Connector { points, label, .. } => {
                for point in points {
                    min_y = min_y.min(point[1]);
                    max_y = max_y.max(point[1]);
                }
                if let Some(label) = label {
                    let bbox = normalize_box(label.bbox);
                    min_y = min_y.min(bbox[1]);
                    max_y = max_y.max(bbox[3]);
                }
            }
        }
    }
    min_y.is_finite().then_some((min_y, max_y))
}

fn translate_draw_plan_y(plan: &mut DrawPlan, dy: f64) {
    for object in &mut plan.objects {
        match object {
            DrawObject::Box { bbox, .. }
            | DrawObject::Text { bbox, .. }
            | DrawObject::Image { bbox, .. }
            | DrawObject::Group { bbox, .. } => {
                *bbox = translate_bbox_y(*bbox, dy);
            }
            DrawObject::Connector { points, label, .. } => {
                for point in points {
                    point[1] += dy;
                }
                if let Some(label) = label {
                    label.bbox = translate_bbox_y(label.bbox, dy);
                }
            }
        }
    }
}

fn translate_bbox_y(bbox: [f64; 4], dy: f64) -> [f64; 4] {
    [bbox[0], bbox[1] + dy, bbox[2], bbox[3] + dy]
}

#[derive(Clone, Debug)]
struct ModelBoxSnapshot {
    id: String,
    bbox: [f64; 4],
    stability: i32,
}

fn resolve_model_box_overlaps(plan: &mut DrawPlan) {
    let mut moved_ids = HashSet::new();
    for _ in 0..12 {
        let boxes = model_box_snapshots(plan);
        let mut moved_this_pass = false;
        'pairs: for (left_index, left) in boxes.iter().enumerate() {
            for right in boxes.iter().skip(left_index + 1) {
                if !component_overlap_gate_fails(left.bbox, right.bbox) {
                    continue;
                }
                let (moving, blocker) = box_to_move_for_overlap(left, right);
                if let Some(candidate) = best_overlap_resolution_candidate(moving, blocker, &boxes)
                {
                    if !boxes_nearly_equal(candidate, moving.bbox) {
                        set_box_bbox(plan, &moving.id, candidate);
                        moved_ids.insert(moving.id.clone());
                        moved_this_pass = true;
                        break 'pairs;
                    }
                }
            }
        }
        if !moved_this_pass {
            break;
        }
    }

    if !moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    }
}

fn model_box_snapshots(plan: &DrawPlan) -> Vec<ModelBoxSnapshot> {
    plan.objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Box {
                id,
                bbox,
                text,
                role,
                style,
                ..
            } = object
            else {
                return None;
            };
            Some(ModelBoxSnapshot {
                id: id.clone(),
                bbox: normalize_box(*bbox),
                stability: box_stability_score(id, text, role, style),
            })
        })
        .collect()
}

fn box_stability_score(id: &str, text: &str, role: &str, style: &str) -> i32 {
    let haystack = format!(
        "{} {} {} {}",
        id.to_lowercase(),
        text.to_lowercase(),
        role.to_lowercase(),
        style.to_lowercase()
    );
    let mut score = 0;
    if haystack.contains("primary") || haystack.contains("main") {
        score += 80;
    }
    if haystack.contains("teacher")
        || haystack.contains("student")
        || haystack.contains("encoder")
        || haystack.contains("decoder")
    {
        score += 40;
    }
    if haystack.contains("output") || haystack.contains('ŷ') || haystack.contains("y_hat") {
        score += 25;
    }
    if haystack.contains("loss") {
        score -= 10;
    }
    if haystack.contains("context") || haystack.contains("muted") || haystack.contains("note") {
        score -= 20;
    }
    score
}

fn box_to_move_for_overlap<'a>(
    left: &'a ModelBoxSnapshot,
    right: &'a ModelBoxSnapshot,
) -> (&'a ModelBoxSnapshot, &'a ModelBoxSnapshot) {
    if left.stability < right.stability {
        (left, right)
    } else if right.stability < left.stability {
        (right, left)
    } else if box_area(left.bbox) <= box_area(right.bbox) {
        (left, right)
    } else {
        (right, left)
    }
}

fn best_overlap_resolution_candidate(
    moving: &ModelBoxSnapshot,
    blocker: &ModelBoxSnapshot,
    boxes: &[ModelBoxSnapshot],
) -> Option<[f64; 4]> {
    let moving_box = normalize_box(moving.bbox);
    let blocker_box = normalize_box(blocker.bbox);
    let overlap_x = moving_box[2].min(blocker_box[2]) - moving_box[0].max(blocker_box[0]);
    let overlap_y = moving_box[3].min(blocker_box[3]) - moving_box[1].max(blocker_box[1]);
    if overlap_x <= 0.0 || overlap_y <= 0.0 {
        return None;
    }

    let gap = 0.025;
    let shifts = [
        [0.0, overlap_y + gap],
        [0.0, -(overlap_y + gap)],
        [overlap_x + gap, 0.0],
        [-(overlap_x + gap), 0.0],
        [blocker_box[2] - moving_box[0] + gap, 0.0],
        [blocker_box[0] - moving_box[2] - gap, 0.0],
        [0.0, blocker_box[3] - moving_box[1] + gap],
        [0.0, blocker_box[1] - moving_box[3] - gap],
    ];
    let mut candidates = Vec::new();
    for [dx, dy] in shifts {
        if let Some(candidate) = shifted_box_inside(moving_box, dx, dy) {
            push_unique_box(&mut candidates, candidate);
        } else {
            push_unique_box(&mut candidates, clamp_shifted_box(moving_box, dx, dy));
        }
    }

    let current_score = overlap_resolution_score(moving_box, moving_box, &moving.id, boxes);
    candidates
        .into_iter()
        .min_by(|left, right| {
            let left_score = overlap_resolution_score(*left, moving_box, &moving.id, boxes);
            let right_score = overlap_resolution_score(*right, moving_box, &moving.id, boxes);
            left_score.total_cmp(&right_score)
        })
        .filter(|candidate| {
            overlap_resolution_score(*candidate, moving_box, &moving.id, boxes) + 0.0001
                < current_score
        })
}

fn shifted_box_inside(bbox: [f64; 4], dx: f64, dy: f64) -> Option<[f64; 4]> {
    let shifted = [bbox[0] + dx, bbox[1] + dy, bbox[2] + dx, bbox[3] + dy];
    (shifted[0] >= 0.02 && shifted[1] >= 0.02 && shifted[2] <= 0.98 && shifted[3] <= 0.98)
        .then_some(shifted)
}

fn shifted_box_horizontally_inside(bbox: [f64; 4], dx: f64) -> Option<[f64; 4]> {
    let bbox = normalize_box(bbox);
    let shifted = [bbox[0] + dx, bbox[1], bbox[2] + dx, bbox[3]];
    (shifted[0] >= 0.02 && shifted[2] <= 0.98).then_some(shifted)
}

fn clamp_shifted_box(bbox: [f64; 4], dx: f64, dy: f64) -> [f64; 4] {
    let width = box_width(bbox);
    let height = box_height(bbox);
    let max_x1 = (0.98 - width).max(0.02);
    let max_y1 = (0.98 - height).max(0.02);
    let x1 = (bbox[0] + dx).clamp(0.02, max_x1);
    let y1 = (bbox[1] + dy).clamp(0.02, max_y1);
    [x1, y1, x1 + width, y1 + height]
}

fn push_unique_box(boxes: &mut Vec<[f64; 4]>, bbox: [f64; 4]) {
    if !boxes
        .iter()
        .any(|existing| boxes_nearly_equal(*existing, bbox))
    {
        boxes.push(normalize_box(bbox));
    }
}

fn overlap_resolution_score(
    candidate: [f64; 4],
    original: [f64; 4],
    moving_id: &str,
    boxes: &[ModelBoxSnapshot],
) -> f64 {
    let mut score = 0.0;
    for other in boxes.iter().filter(|other| other.id != moving_id) {
        let overlap = intersection_area(candidate, other.bbox);
        if overlap <= 0.0 {
            continue;
        }
        let ratio = overlap / box_area(candidate).min(box_area(other.bbox)).max(0.0001);
        if component_overlap_gate_fails(candidate, other.bbox) {
            score += 1000.0 + ratio * 100.0 + overlap * 10.0;
        } else {
            score += overlap;
        }
    }
    score + box_center_distance(candidate, original) * 0.1 + edge_pressure(candidate) * 0.01
}

fn box_center_distance(left: [f64; 4], right: [f64; 4]) -> f64 {
    let dx = center_x(left) - center_x(right);
    let dy = center_y(left) - center_y(right);
    (dx * dx + dy * dy).sqrt()
}

fn edge_pressure(bbox: [f64; 4]) -> f64 {
    let bbox = normalize_box(bbox);
    [bbox[0], bbox[1], 1.0 - bbox[2], 1.0 - bbox[3]]
        .into_iter()
        .map(|distance| (0.04 - distance).max(0.0))
        .sum()
}

fn component_overlap_gate_fails(left: [f64; 4], right: [f64; 4]) -> bool {
    let overlap = intersection_area(left, right);
    if overlap <= 0.0 {
        return false;
    }
    let (overlap_width, overlap_height) = intersection_dimensions(left, right);
    let overlap_ratio = overlap / box_area(left).min(box_area(right)).max(0.0001);
    (overlap_width >= 0.01 && overlap_height >= 0.02 && overlap_ratio > 0.08)
        || (overlap_width >= 0.07 && overlap_height >= 0.012 && overlap_ratio > 0.12)
        || (overlap > 0.003 && overlap_ratio > 0.15)
}

fn set_box_bbox(plan: &mut DrawPlan, id: &str, next_bbox: [f64; 4]) {
    for object in &mut plan.objects {
        let DrawObject::Box {
            id: object_id,
            bbox,
            ..
        } = object
        else {
            continue;
        };
        if object_id == id {
            *bbox = normalize_box(next_bbox);
            return;
        }
    }
}

fn set_object_bbox(plan: &mut DrawPlan, id: &str, next_bbox: [f64; 4]) {
    let next_bbox = normalize_box(next_bbox);
    for object in &mut plan.objects {
        match object {
            DrawObject::Box {
                id: object_id,
                bbox,
                ..
            } if object_id == id => {
                *bbox = next_bbox;
                return;
            }
            DrawObject::Text {
                id: object_id,
                bbox,
                ..
            } if object_id == id => {
                *bbox = next_bbox;
                return;
            }
            _ => {}
        }
    }
}

fn boxes_nearly_equal(left: [f64; 4], right: [f64; 4]) -> bool {
    left.iter()
        .zip(right)
        .all(|(left, right)| (left - right).abs() < 0.0001)
}

fn realign_connector_endpoints_for_moved_boxes(plan: &mut DrawPlan, moved_ids: &HashSet<String>) {
    let box_map = current_box_map(plan);
    for object in &mut plan.objects {
        let DrawObject::Connector {
            points, from, to, ..
        } = object
        else {
            continue;
        };
        if points.len() < 2 {
            continue;
        }
        if let Some(from_id) = from.as_deref() {
            if moved_ids.contains(from_id) {
                if let Some(from_box) = box_map.get(from_id) {
                    let target = to
                        .as_deref()
                        .and_then(|to_id| box_map.get(to_id).copied())
                        .map(center)
                        .unwrap_or(points[1]);
                    points[0] = anchor_point_towards(*from_box, target);
                }
            }
        }
        if let Some(to_id) = to.as_deref() {
            if moved_ids.contains(to_id) {
                if let Some(to_box) = box_map.get(to_id) {
                    let target = from
                        .as_deref()
                        .and_then(|from_id| box_map.get(from_id).copied())
                        .map(center)
                        .unwrap_or_else(|| points[points.len() - 2]);
                    let last_index = points.len() - 1;
                    points[last_index] = anchor_point_towards(*to_box, target);
                }
            }
        }
    }
}

fn repair_degenerate_connector_points_from_boxes(plan: &mut DrawPlan) {
    let box_map = current_box_map(plan);
    for object in &mut plan.objects {
        let DrawObject::Connector {
            points, from, to, ..
        } = object
        else {
            continue;
        };
        if points.len() < 2 {
            let repaired =
                match (from.as_deref(), to.as_deref()) {
                    (Some(from_id), Some(to_id)) => box_map
                        .get(from_id)
                        .zip(box_map.get(to_id))
                        .map(|(from_box, to_box)| {
                            repaired_connector_points_between_boxes(*from_box, *to_box)
                        }),
                    _ => None,
                }
                .unwrap_or_else(|| {
                    let start = points.first().copied().unwrap_or([0.48, 0.50]);
                    let end_x = if start[0] <= 0.92 {
                        (start[0] + 0.06).min(0.98)
                    } else {
                        (start[0] - 0.06).max(0.02)
                    };
                    vec![clamp_point(start), [end_x, start[1].clamp(0.02, 0.98)]]
                });
            *points = repaired;
            continue;
        }

        let repaired = polished_connector_points(points);
        if repaired.len() < 2 {
            continue;
        }
        let original_intersects = connector_points_crosses_boxes(
            points.as_slice(),
            from.as_deref(),
            to.as_deref(),
            &box_map,
        );
        let candidate_intersects =
            connector_points_crosses_boxes(&repaired, from.as_deref(), to.as_deref(), &box_map);
        let original_length = polyline_length(points.as_slice());
        let candidate_length = polyline_length(&repaired);
        if (!candidate_intersects && candidate_length <= original_length)
            || (original_intersects && candidate_length <= original_length * 0.90)
            || (repaired != *points && candidate_length < original_length * 0.5)
        {
            *points = repaired;
        }
    }
}

fn connector_points_crosses_boxes(
    points: &[[f64; 2]],
    from_id: Option<&str>,
    to_id: Option<&str>,
    box_map: &HashMap<String, [f64; 4]>,
) -> bool {
    if points.len() < 2 {
        return false;
    }
    points.windows(2).any(|window| {
        let segment_box = [
            window[0][0].min(window[1][0]),
            window[0][1].min(window[1][1]),
            window[0][0].max(window[1][0]),
            window[0][1].max(window[1][1]),
        ];
        let padded = expand_box(segment_box, 0.002);
        box_map.iter().any(|(id, bbox)| {
            id != from_id.unwrap_or("")
                && id != to_id.unwrap_or("")
                && intersection_area(padded, *bbox) > 0.0001
        })
    })
}

fn repaired_connector_points_between_boxes(from_box: [f64; 4], to_box: [f64; 4]) -> Vec<[f64; 2]> {
    let start = anchor_point_towards(from_box, center(to_box));
    let end = anchor_point_towards(to_box, center(from_box));
    if (start[0] - end[0]).abs() < 0.0001 && (start[1] - end[1]).abs() < 0.0001 {
        let end_x = if start[0] <= 0.92 {
            (start[0] + 0.06).min(0.98)
        } else {
            (start[0] - 0.06).max(0.02)
        };
        vec![start, [end_x, start[1]]]
    } else {
        vec![start, end]
    }
}

fn current_box_map(plan: &DrawPlan) -> HashMap<String, [f64; 4]> {
    plan.objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Box { id, bbox, .. } = object else {
                return None;
            };
            Some((id.clone(), normalize_box(*bbox)))
        })
        .collect()
}

fn anchor_point_towards(bbox: [f64; 4], target: [f64; 2]) -> [f64; 2] {
    let bbox = normalize_box(bbox);
    let center = center(bbox);
    let dx = target[0] - center[0];
    let dy = target[1] - center[1];
    if dx.abs() >= dy.abs() {
        let x = if dx >= 0.0 { bbox[2] } else { bbox[0] };
        [x, center[1]]
    } else {
        let y = if dy >= 0.0 { bbox[3] } else { bbox[1] };
        [center[0], y]
    }
}

fn expand_bbox_to_min_size(bbox: [f64; 4], min_width: f64, min_height: f64) -> [f64; 4] {
    let bbox = normalize_box(bbox);
    let center_x = center_x(bbox);
    let center_y = center_y(bbox);
    let width = box_width(bbox).max(min_width);
    let height = box_height(bbox).max(min_height);
    let mut x1 = center_x - width / 2.0;
    let mut x2 = center_x + width / 2.0;
    let mut y1 = center_y - height / 2.0;
    let mut y2 = center_y + height / 2.0;
    if x1 < 0.0 {
        x2 = (x2 - x1).min(1.0);
        x1 = 0.0;
    }
    if x2 > 1.0 {
        x1 = (x1 - (x2 - 1.0)).max(0.0);
        x2 = 1.0;
    }
    if y1 < 0.0 {
        y2 = (y2 - y1).min(1.0);
        y1 = 0.0;
    }
    if y2 > 1.0 {
        y1 = (y1 - (y2 - 1.0)).max(0.0);
        y2 = 1.0;
    }
    [x1, y1, x2, y2]
}

fn prune_teacher_student_to_figure_plan(plan: &mut DrawPlan, figure_plan: &FigurePlan) {
    if figure_plan.layout.template != Template::TeacherStudent {
        return;
    }

    let component_ids = figure_plan
        .components
        .iter()
        .map(|component| component.id.as_str())
        .collect::<HashSet<_>>();

    let extra_box_ids = plan
        .objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Box { id, text, role, .. } = object else {
                return None;
            };
            (!component_ids.contains(id.as_str()) && is_teacher_student_extra_box(id, text, role))
                .then(|| id.clone())
        })
        .collect::<HashSet<_>>();

    if !extra_box_ids.is_empty() {
        plan.objects.retain(|object| match object {
            DrawObject::Box { id, .. } => !extra_box_ids.contains(id),
            DrawObject::Connector { from, to, .. } => {
                !from.as_deref().is_some_and(|id| extra_box_ids.contains(id))
                    && !to.as_deref().is_some_and(|id| extra_box_ids.contains(id))
            }
            _ => true,
        });
    }

    for edge in &figure_plan.edges {
        if !component_ids.contains(edge.from.as_str()) || !component_ids.contains(edge.to.as_str())
        {
            continue;
        }
        let style = figure_plan_edge_style(edge.semantic, edge.style, edge.importance);
        if plan.objects.iter().any(|object| {
            matches!(
                object,
                DrawObject::Connector { from, to, .. }
                    if from.as_deref() == Some(edge.from.as_str())
                        && to.as_deref() == Some(edge.to.as_str())
            )
        }) {
            continue;
        }
        let label = (!edge.label.trim().is_empty()).then(|| DrawLabel {
            text: edge.label.clone(),
            bbox: [0.42, 0.30, 0.58, 0.36],
        });
        let connector_id = unique_draw_object_id(plan, &edge.id);
        plan.objects.push(DrawObject::Connector {
            id: connector_id,
            points: vec![[0.0, 0.0], [1.0, 1.0]],
            from: Some(edge.from.clone()),
            to: Some(edge.to.clone()),
            style: style.to_string(),
            label,
            z: next_z(plan),
        });
    }
}

fn is_teacher_student_extra_box(id: &str, text: &str, role: &str) -> bool {
    let id = id.to_lowercase();
    let text = text.to_lowercase();
    let role = role.to_lowercase();
    id.contains("latent")
        || id.contains("residual")
        || id.contains("inference")
        || id.contains("output_pred")
        || text.contains("latent residual")
        || text.contains("inference only")
        || role.contains("inference")
}

fn figure_plan_edge_style(
    _semantic: EdgeSemantic,
    style: EdgeStyle,
    importance: EdgeImportance,
) -> &'static str {
    if matches!(style, EdgeStyle::Dash | EdgeStyle::LongDash) {
        "dashed_supervision"
    } else {
        match importance {
            EdgeImportance::Main => "main_flow",
            EdgeImportance::Normal => "normal_flow",
            EdgeImportance::Aux => "aux_flow",
        }
    }
}

pub fn preserve_semantic_draw_objects(previous: &DrawPlan, revised: &mut DrawPlan) {
    for object in &previous.objects {
        if matches!(object, DrawObject::Text { .. }) {
            continue;
        }
        let id = draw_object_id(object);
        if revised
            .objects
            .iter()
            .any(|candidate| draw_object_id(candidate) == id)
        {
            continue;
        }
        revised.objects.push(object.clone());
    }
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

fn repair_teacher_student_lanes(plan: &mut DrawPlan, figure_plan: Option<&FigurePlan>) {
    let Some(input_id) = find_box_id(plan, |id, text, role| {
        id.contains("input") || role.contains("input") || text == "x" || text.contains("input")
    }) else {
        return;
    };
    let Some(teacher_id) = find_box_id(plan, |id, text, _role| {
        id.contains("teacher") || text.contains("teacher")
    }) else {
        return;
    };
    let Some(student_id) = find_box_id(plan, |id, text, role| {
        (id.contains("student") || text.contains("student")) && !role.contains("context")
    }) else {
        return;
    };
    let input_box = [0.05, 0.38, 0.18, 0.56];
    let teacher_box = [0.24, 0.22, 0.44, 0.38];
    let student_box = [0.24, 0.46, 0.44, 0.62];
    let teacher_output_box = [0.64, 0.10, 0.74, 0.26];
    let latent_box = [0.50, 0.22, 0.62, 0.38];
    let latent_teacher_box = [0.58, 0.24, 0.68, 0.36];
    let latent_student_box = [0.64, 0.40, 0.74, 0.52];
    let mut output_box = [0.76, 0.46, 0.87, 0.62];
    let loss_box = [0.92, 0.46, 0.99, 0.62];
    let student_loss_box = [0.82, 0.46, 0.98, 0.62];
    let loss_feedback_box = [0.82, 0.64, 0.94, 0.76];
    let residual_loss_box = [0.76, 0.24, 0.90, 0.36];
    let inference_student_box = [0.78, 0.46, 0.90, 0.62];
    let inference_input_box = [0.05, 0.74, 0.18, 0.90];
    let inference_output_box = [0.92, 0.46, 0.99, 0.62];
    let inference_badge_box = [0.62, 0.72, 0.88, 0.80];
    let figure_component_ids = figure_plan
        .map(|plan| {
            plan.components
                .iter()
                .map(|component| component.id.as_str())
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();
    let latent_pair = figure_plan_latent_pair(figure_plan, &teacher_id, &student_id);
    let residual_box = figure_plan_residual_box(figure_plan, &teacher_id, &student_id);
    let teacher_latent_id = figure_plan_teacher_latent_id(figure_plan, &teacher_id);
    let explicit_inference_student_id = figure_plan_inference_student_id(figure_plan, &student_id);
    let synthesize_inference_from_annotation = explicit_inference_student_id.is_none()
        && figure_plan_inference_annotation_requests_lane(figure_plan, &student_id);
    let explicit_inference_input_id = figure_plan_inference_input_id(
        figure_plan,
        &input_id,
        explicit_inference_student_id.as_deref(),
    );
    let explicit_inference_output_id =
        figure_plan_inference_output_id(figure_plan, explicit_inference_student_id.as_deref());
    let inference_label_id = figure_plan_inference_label_id(figure_plan);

    let explicit_latent_id = residual_box
        .as_ref()
        .map(|residual| residual.box_id.clone())
        .or_else(|| {
            find_box_id(plan, |id, text, role| {
                id != teacher_id
                    && id != student_id
                    && !id.contains("teacher")
                    && !id.contains("student")
                    && !id.contains("output")
                    && !id.contains("loss")
                    && !text.contains("loss")
                    && !role.contains("output")
                    && !role.contains("loss")
                    && !role.contains("context")
                    && (id.contains("latent")
                        || id.contains("residual")
                        || text.contains("latent")
                        || text.contains("residual"))
            })
        });
    let teacher_output_id = find_teacher_output_id(plan, &teacher_id);
    let latent_id = explicit_latent_id;
    let teacher_output_id_lower = teacher_output_id.as_ref().map(|id| id.to_lowercase());
    let mut output_id = find_student_output_id(plan, &student_id).or_else(|| {
        find_box_id(plan, |id, text, role| {
            teacher_output_id_lower.as_deref() != Some(id)
                && !id.contains("inference")
                && !id.contains("teacher")
                && !text.contains("teacher")
                && !text.contains("student")
                && (id.contains("output")
                    || role.contains("output")
                    || text.contains('ŷ')
                    || text == "y_hat")
        })
    });
    let mut loss_id = find_box_id(plan, |id, text, _role| {
        id.contains("task_loss") || text.contains("task loss")
    })
    .or_else(|| {
        find_box_id(plan, |id, text, role| {
            !id.contains("latent")
                && !id.contains("residual")
                && !text.contains("latent")
                && !text.contains("residual")
                && (id.contains("loss") || role.contains("loss") || text.contains("loss"))
        })
    });
    if output_id.is_none() && loss_id.is_some() && teacher_output_id.is_none() {
        let id = upsert_box(
            plan,
            "output_pred",
            output_box,
            "ŷ",
            "output",
            "neutral_module",
        );
        output_id = Some(id);
    }
    let loss_feedback_label =
        figure_plan_loss_feedback_label(figure_plan, output_id.as_deref(), &student_id);
    if let (Some(_), Some(loss_id_to_remove)) = (&loss_feedback_label, loss_id.clone()) {
        // FigurePlan 的 edge 语义优先于孤立 loss component；否则会把反馈监督误画成终端节点。
        plan.objects.retain(|object| match object {
            DrawObject::Box { id, .. } => id != &loss_id_to_remove,
            DrawObject::Connector { from, to, .. } => {
                from.as_deref() != Some(loss_id_to_remove.as_str())
                    && to.as_deref() != Some(loss_id_to_remove.as_str())
            }
            _ => true,
        });
        loss_id = None;
    }
    let direct_residual_label =
        figure_plan_supervision_label(figure_plan, &teacher_id, &student_id);
    let residual_loss_id =
        figure_plan_residual_loss_id(figure_plan, &student_id, latent_id.as_deref());
    let has_student_loss_edge = loss_id
        .as_deref()
        .is_some_and(|loss_id| figure_plan_has_edge(figure_plan, &student_id, loss_id));
    let has_loss_student_edge = loss_id
        .as_deref()
        .is_some_and(|loss_id| figure_plan_has_edge(figure_plan, loss_id, &student_id));
    let has_output_loss_edge =
        if let (Some(output_id), Some(loss_id)) = (output_id.as_deref(), loss_id.as_deref()) {
            figure_plan_has_edge(figure_plan, output_id, loss_id)
        } else {
            false
        };
    let has_student_output_edge = output_id
        .as_deref()
        .is_some_and(|output_id| figure_plan_has_edge(figure_plan, &student_id, output_id));
    let has_explicit_inference_output_edge = if let (Some(inference_id), Some(output_id)) = (
        explicit_inference_student_id.as_deref(),
        output_id.as_deref(),
    ) {
        figure_plan_has_edge(figure_plan, inference_id, output_id)
    } else {
        false
    };
    let loss_like_ids = loss_like_ids(plan);
    let raw_inference_student_id = inference_student_id(plan, &student_id)
        .filter(|id| !figure_component_ids.contains(id.as_str()));
    let has_inference = raw_inference_student_id.is_some()
        || explicit_inference_student_id.is_some()
        || synthesize_inference_from_annotation;
    if has_inference {
        output_box = [0.66, 0.46, 0.76, 0.62];
    }
    let active_loss_box = if has_loss_student_edge || (has_inference && has_output_loss_edge) {
        loss_feedback_box
    } else if has_student_loss_edge {
        student_loss_box
    } else {
        loss_box
    };
    let mut inference_student_ids = inference_student_ids(plan, &student_id);
    inference_student_ids.retain(|id| !figure_component_ids.contains(id.as_str()));
    let mut removable_ids = inference_student_ids.clone();
    removable_ids.extend(duplicate_inference_output_ids(
        plan,
        output_id.as_deref(),
        &inference_student_ids,
    ));
    removable_ids.extend(duplicate_training_output_ids(
        plan,
        output_id.as_deref(),
        &student_id,
    ));
    if let Some(latent_id) = latent_id.as_deref() {
        removable_ids.extend(duplicate_residual_ids(plan, latent_id));
    }
    let mut required_ids = vec![input_id.as_str(), teacher_id.as_str(), student_id.as_str()];
    if let Some(latent_id) = latent_id.as_deref() {
        required_ids.push(latent_id);
    }
    removable_ids.extend(auxiliary_hidden_state_ids(
        plan,
        &required_ids,
        &[
            output_id.as_deref(),
            loss_id.as_deref(),
            teacher_output_id.as_deref(),
        ],
    ));
    removable_ids.retain(|id| !figure_component_ids.contains(id.as_str()));
    if !removable_ids.is_empty() {
        plan.objects.retain(|object| match object {
            DrawObject::Box { id, .. } => !removable_ids.contains(id),
            DrawObject::Connector { from, to, .. } => {
                !from.as_deref().is_some_and(|id| removable_ids.contains(id))
                    && !to.as_deref().is_some_and(|id| removable_ids.contains(id))
            }
            _ => true,
        });
    }
    if let Some(inference_label_id) = inference_label_id.as_deref() {
        plan.objects.retain(|object| match object {
            DrawObject::Box { id, .. } => id != inference_label_id,
            DrawObject::Connector { from, to, .. } => {
                from.as_deref() != Some(inference_label_id)
                    && to.as_deref() != Some(inference_label_id)
            }
            _ => true,
        });
    }
    plan.objects
        .retain(|object| !is_redundant_inference_text(object));
    if figure_plan.is_some() {
        plan.objects.retain(|object| {
            let DrawObject::Connector { from, to, .. } = object else {
                return true;
            };
            let (Some(from_id), Some(to_id)) = (from.as_deref(), to.as_deref()) else {
                return true;
            };
            let connects_declared_components =
                figure_component_ids.contains(from_id) && figure_component_ids.contains(to_id);
            !connects_declared_components || figure_plan_has_edge(figure_plan, from_id, to_id)
        });
    }
    plan.objects.retain(|object| {
        !matches!(
            object,
            DrawObject::Connector { from, to, .. }
                if from.as_deref() == Some(input_id.as_str())
                    && to.as_deref().is_some_and(|id| loss_like_ids.contains(id))
        )
    });
    plan.objects.retain(|object| {
        !matches!(
            object,
            DrawObject::Connector { from, to, .. }
                if from.as_deref() == Some(teacher_id.as_str())
                    && to.as_deref() == Some(student_id.as_str())
                    && (latent_id.is_some() || teacher_output_id.is_some() || latent_pair.is_some())
        )
    });

    if output_id.is_some() {
        if let Some(loss_id) = loss_id.as_deref() {
            if !has_student_loss_edge {
                plan.objects.retain(|object| {
                    !matches!(
                        object,
                        DrawObject::Connector { from, to, .. }
                            if from.as_deref() == Some(student_id.as_str())
                                && to.as_deref() == Some(loss_id)
                    )
                });
            }
        }
    }

    for object in &mut plan.objects {
        if let DrawObject::Box {
            id,
            bbox,
            text,
            role,
            style,
            ..
        } = object
        {
            if *id == input_id {
                *bbox = input_box;
                *role = "input".to_string();
                *style = "neutral_module".to_string();
            } else if *id == teacher_id {
                *bbox = teacher_box;
                *text = "Teacher\n(training)".to_string();
                *role = "main".to_string();
                *style = "primary_module_regular".to_string();
            } else if *id == student_id {
                *bbox = student_box;
                if has_inference {
                    *text = "Student\n(training)".to_string();
                } else {
                    *text = "Student\ntraining + inference".to_string();
                }
                *role = "main".to_string();
                *style = "primary_module_regular".to_string();
            } else if latent_id.as_ref() == Some(id) {
                *bbox = latent_box;
                *text = "Latent Residual\nh_T - h_S".to_string();
                *role = "module".to_string();
                *style = "accent_module".to_string();
            } else if explicit_inference_student_id.as_ref() == Some(id) {
                *bbox = inference_student_box;
                *text = "Student\n(inference)".to_string();
                *role = "main".to_string();
                *style = "primary_module_regular".to_string();
            } else if explicit_inference_input_id.as_ref() == Some(id) {
                *bbox = inference_input_box;
                *role = "input".to_string();
                *style = "neutral_module".to_string();
            } else if explicit_inference_output_id
                .as_ref()
                .is_some_and(|output_id| output_id == id)
            {
                *bbox = inference_output_box;
                *role = "output".to_string();
                *style = "neutral_module".to_string();
            } else if teacher_latent_id.as_ref() == Some(id) {
                *bbox = latent_teacher_box;
                *text = "Latent h_T".to_string();
                *role = "context".to_string();
                *style = "neutral_module".to_string();
            } else if latent_pair
                .as_ref()
                .is_some_and(|pair| pair.teacher_latent_id == *id)
            {
                *bbox = latent_teacher_box;
                *text = "h_T".to_string();
                *role = "module".to_string();
                *style = "muted_module".to_string();
            } else if latent_pair
                .as_ref()
                .is_some_and(|pair| pair.student_latent_id == *id)
            {
                *bbox = latent_student_box;
                *text = "h_S".to_string();
                *role = "module".to_string();
                *style = "muted_module".to_string();
            } else if teacher_output_id.as_ref() == Some(id) {
                *bbox = teacher_output_box;
                *role = "output".to_string();
                *style = "neutral_module".to_string();
            } else if output_id.as_ref() == Some(id) {
                *bbox = output_box;
                *role = "output".to_string();
                *style = "neutral_module".to_string();
            } else if loss_id.as_ref() == Some(id) {
                *bbox = active_loss_box;
                *role = "loss".to_string();
                *style = "neutral_module".to_string();
            } else if residual_loss_id.as_ref() == Some(id) {
                *bbox = residual_loss_box;
                *role = "loss".to_string();
                *style = "neutral_module".to_string();
            } else if id.contains("latent_loss") || text.to_lowercase().contains("latent loss") {
                *bbox = [0.78, 0.82, 0.92, 0.92];
                *role = "loss".to_string();
                *style = "neutral_module".to_string();
            } else if figure_component_ids.contains(id.as_str()) && id.contains("inference") {
                *bbox = inference_badge_box;
                *role = "context".to_string();
                *style = "neutral_module".to_string();
            }
        }
    }

    let should_create_canonical_inference =
        raw_inference_student_id.is_some() || synthesize_inference_from_annotation;
    let should_create_canonical_inference_output = raw_inference_student_id.is_some();
    let canonical_inference_student_id = if should_create_canonical_inference {
        let id = upsert_box(
            plan,
            "inference_student",
            inference_student_box,
            "Student\n(inference only)",
            "main",
            "primary_module_regular",
        );
        if should_create_canonical_inference_output {
            upsert_box(
                plan,
                "inference_output",
                inference_output_box,
                "ŷ",
                "output",
                "neutral_module",
            );
        }
        Some(id)
    } else {
        None
    };
    let canonical_inference_output_id = if should_create_canonical_inference_output {
        Some(
            find_box_id(plan, |id, _text, role| {
                id == "inference_output" || (id.contains("inference") && role.contains("output"))
            })
            .unwrap_or_else(|| "inference_output".to_string()),
        )
    } else {
        None
    };

    for object in &mut plan.objects {
        if let DrawObject::Connector {
            from,
            to,
            points,
            style,
            label,
            ..
        } = object
        {
            let Some(from_id) = from.as_deref() else {
                continue;
            };
            let Some(to_id) = to.as_deref() else {
                continue;
            };
            if from_id == input_id && to_id == teacher_id {
                *points = vec![
                    [input_box[2], 0.43],
                    [0.18, 0.43],
                    [0.18, center_y(teacher_box)],
                    [teacher_box[0], center_y(teacher_box)],
                ];
                *style = "normal_flow".to_string();
                *label = None;
            } else if from_id == input_id && to_id == student_id {
                *points = vec![
                    [input_box[2], center_y(student_box)],
                    [student_box[0], center_y(student_box)],
                ];
                *style = "main_flow".to_string();
                *label = None;
            } else if explicit_inference_student_id
                .as_deref()
                .is_some_and(|inference_id| from_id == input_id && to_id == inference_id)
            {
                *points = vec![
                    [input_box[2], center_y(input_box)],
                    [input_box[2], center_y(inference_student_box)],
                    [inference_student_box[0], center_y(inference_student_box)],
                ];
                *style = "main_flow".to_string();
                *label = None;
            } else if explicit_inference_student_id
                .as_deref()
                .is_some_and(|inference_id| {
                    explicit_inference_input_id
                        .as_deref()
                        .is_some_and(|input_id| from_id == input_id && to_id == inference_id)
                })
            {
                *points = vec![
                    [inference_input_box[2], center_y(inference_input_box)],
                    [inference_student_box[0], center_y(inference_student_box)],
                ];
                *style = "main_flow".to_string();
                *label = None;
            } else if explicit_inference_student_id
                .as_deref()
                .is_some_and(|inference_id| {
                    explicit_inference_output_id
                        .as_deref()
                        .is_some_and(|output_id| from_id == inference_id && to_id == output_id)
                })
            {
                *points = vec![
                    [inference_student_box[2], center_y(inference_student_box)],
                    [inference_output_box[0], center_y(inference_student_box)],
                ];
                *style = "main_flow".to_string();
                *label = None;
            } else if teacher_output_id.as_deref() == Some(to_id) && from_id == teacher_id {
                *points = vec![
                    [teacher_box[2], center_y(teacher_box)],
                    [teacher_output_box[0], center_y(teacher_output_box)],
                ];
                *style = "normal_flow".to_string();
                *label = None;
            } else if teacher_latent_id
                .as_deref()
                .is_some_and(|teacher_latent_id| {
                    from_id == teacher_id && to_id == teacher_latent_id
                })
            {
                *points = vec![
                    [center_x(teacher_box), teacher_box[3]],
                    [center_x(latent_teacher_box), latent_teacher_box[1]],
                ];
                *style = "normal_flow".to_string();
                *label = None;
            } else if teacher_latent_id
                .as_deref()
                .is_some_and(|teacher_latent_id| {
                    latent_id
                        .as_deref()
                        .is_some_and(|latent_id| from_id == teacher_latent_id && to_id == latent_id)
                })
            {
                *points = vec![
                    [latent_teacher_box[2], center_y(latent_teacher_box)],
                    [latent_box[0], center_y(latent_teacher_box)],
                    [latent_box[0], center_y(latent_box)],
                ];
                *style = "normal_flow".to_string();
                *label = None;
            } else if latent_pair
                .as_ref()
                .is_some_and(|pair| from_id == teacher_id && to_id == pair.teacher_latent_id)
            {
                *points = vec![
                    [teacher_box[2], center_y(teacher_box)],
                    [latent_teacher_box[0], center_y(latent_teacher_box)],
                ];
                *style = "normal_flow".to_string();
                *label = None;
            } else if latent_pair
                .as_ref()
                .is_some_and(|pair| from_id == student_id && to_id == pair.student_latent_id)
            {
                *points = vec![
                    [student_box[2], student_box[1]],
                    [0.55, student_box[1]],
                    [0.55, center_y(latent_student_box)],
                    [latent_student_box[0], center_y(latent_student_box)],
                ];
                *style = "normal_flow".to_string();
                *label = None;
            } else if latent_pair.as_ref().is_some_and(|pair| {
                from_id == pair.teacher_latent_id && to_id == pair.student_latent_id
            }) {
                let pair = latent_pair.as_ref().expect("checked above");
                *points = vec![
                    [center_x(latent_teacher_box), latent_teacher_box[3]],
                    [center_x(latent_teacher_box), latent_student_box[1]],
                ];
                *style = "dashed_supervision".to_string();
                *label = Some(DrawLabel {
                    text: pair.residual_label.clone(),
                    bbox: [0.69, 0.25, 0.86, 0.31],
                });
            } else if latent_id.as_deref() == Some(to_id) && from_id == teacher_id {
                *points = vec![
                    [teacher_box[2], center_y(teacher_box)],
                    [latent_box[0], center_y(teacher_box)],
                    [latent_box[0], center_y(latent_box)],
                ];
                *style = "dashed_supervision".to_string();
                *label = None;
            } else if latent_id.as_deref() == Some(to_id) && from_id == student_id {
                *points = vec![
                    [student_box[2], student_box[1]],
                    [latent_box[2], student_box[1]],
                    [latent_box[2], latent_box[3]],
                ];
                *style = "normal_flow".to_string();
                *label = None;
            } else if latent_id.as_deref() == Some(from_id) && to_id == student_id {
                *points = vec![
                    [center_x(latent_box), latent_box[3]],
                    [center_x(latent_box), student_box[1]],
                    [student_box[2], student_box[1]],
                ];
                *style = "dashed_supervision".to_string();
                *label = None;
            } else if teacher_output_id.as_deref() == Some(from_id) && to_id == student_id {
                *points = vec![
                    [teacher_output_box[0], center_y(teacher_output_box)],
                    [0.58, center_y(teacher_output_box)],
                    [0.58, student_box[1]],
                    [student_box[2], student_box[1]],
                ];
                *style = "dashed_supervision".to_string();
                *label = None;
            } else if latent_id.is_none() && from_id == teacher_id && to_id == student_id {
                *points = vec![
                    [teacher_box[2], center_y(teacher_box)],
                    [0.62, center_y(teacher_box)],
                    [0.62, student_box[1]],
                    [student_box[2], student_box[1]],
                ];
                *style = "dashed_supervision".to_string();
                *label = direct_residual_label.clone().map(|text| DrawLabel {
                    text,
                    bbox: [0.64, 0.30, 0.80, 0.36],
                });
            } else if output_id.as_deref() == Some(to_id) && from_id == student_id {
                *points = vec![
                    [student_box[2], center_y(student_box)],
                    [output_box[0], center_y(output_box)],
                ];
                *style = "main_flow".to_string();
                *label = None;
            } else if loss_feedback_label.is_some()
                && output_id.as_deref() == Some(from_id)
                && to_id == student_id
            {
                let feedback_y = 0.68;
                *points = vec![
                    [center_x(output_box), output_box[3]],
                    [center_x(output_box), feedback_y],
                    [center_x(student_box), feedback_y],
                    [center_x(student_box), student_box[3]],
                ];
                *style = "dashed_supervision".to_string();
                *label = loss_feedback_label.clone().map(|text| DrawLabel {
                    text,
                    bbox: [0.52, 0.72, 0.70, 0.78],
                });
            } else if loss_id.as_deref() == Some(to_id) && from_id == student_id {
                *points = vec![
                    [student_box[2], center_y(active_loss_box)],
                    [active_loss_box[0], center_y(active_loss_box)],
                ];
                *style = "normal_flow".to_string();
                *label = None;
            } else if loss_id.as_deref() == Some(from_id) && to_id == student_id {
                *points = vec![
                    [active_loss_box[0], center_y(active_loss_box)],
                    [center_x(student_box), center_y(active_loss_box)],
                    [center_x(student_box), student_box[3]],
                ];
                *style = "dashed_supervision".to_string();
                *label = None;
            } else if residual_loss_id
                .as_deref()
                .is_some_and(|residual_loss_id| from_id == residual_loss_id && to_id == student_id)
            {
                *points = vec![
                    [residual_loss_box[0], center_y(residual_loss_box)],
                    [center_x(latent_box), center_y(residual_loss_box)],
                    [center_x(latent_box), student_box[1]],
                    [student_box[2], student_box[1]],
                ];
                *style = "dashed_supervision".to_string();
                *label = None;
            } else if loss_like_ids.contains(to_id) {
                *label = None;
            } else if let Some(output_id) = output_id.as_deref() {
                if let Some(loss_id) = loss_id.as_deref() {
                    if from_id == output_id && to_id == loss_id {
                        *points = vec![
                            [output_box[2], center_y(output_box)],
                            [0.84, center_y(output_box)],
                            [0.84, center_y(active_loss_box)],
                            [active_loss_box[0], center_y(active_loss_box)],
                        ];
                        *style = "normal_flow".to_string();
                        *label = None;
                    }
                }
            }
        }
    }

    if let Some(latent_id) = latent_id.as_deref() {
        upsert_connector(
            plan,
            "e_teacher_latent",
            &teacher_id,
            latent_id,
            vec![
                [teacher_box[2], center_y(teacher_box)],
                [latent_box[0], center_y(teacher_box)],
                [latent_box[0], center_y(latent_box)],
            ],
            "dashed_supervision",
        );
        upsert_connector(
            plan,
            "e_latent_student",
            latent_id,
            &student_id,
            vec![
                [center_x(latent_box), latent_box[3]],
                [center_x(latent_box), student_box[1]],
                [student_box[2], student_box[1]],
            ],
            "dashed_supervision",
        );
    }
    if let Some(output_id) = output_id.as_deref() {
        let should_upsert_student_output =
            figure_plan.is_none() || has_student_output_edge || !has_explicit_inference_output_edge;
        if should_upsert_student_output {
            upsert_connector(
                plan,
                figure_plan_edge_id(figure_plan, &student_id, output_id)
                    .as_deref()
                    .unwrap_or("e_student_output"),
                &student_id,
                output_id,
                vec![
                    [student_box[2], center_y(student_box)],
                    [output_box[0], center_y(output_box)],
                ],
                "main_flow",
            );
        }
        if let Some(label_text) = loss_feedback_label.clone() {
            let feedback_y = 0.68;
            upsert_connector(
                plan,
                "e_task_loss",
                output_id,
                &student_id,
                vec![
                    [center_x(output_box), output_box[3]],
                    [center_x(output_box), feedback_y],
                    [center_x(student_box), feedback_y],
                    [center_x(student_box), student_box[3]],
                ],
                "dashed_supervision",
            );
            set_connector_label(
                plan,
                output_id,
                &student_id,
                Some(DrawLabel {
                    text: label_text,
                    bbox: [0.52, 0.72, 0.70, 0.78],
                }),
            );
        }
        if let Some(loss_id) = loss_id.as_deref() {
            if has_student_loss_edge {
                upsert_connector(
                    plan,
                    figure_plan_edge_id(figure_plan, &student_id, loss_id)
                        .as_deref()
                        .unwrap_or("e_student_task_loss"),
                    &student_id,
                    loss_id,
                    vec![
                        [student_box[2], center_y(active_loss_box)],
                        [active_loss_box[0], center_y(active_loss_box)],
                    ],
                    "normal_flow",
                );
                set_connector_label(plan, &student_id, loss_id, None);
                plan.objects.retain(|object| {
                    !matches!(
                        object,
                        DrawObject::Connector { from, to, .. }
                            if from.as_deref() == Some(output_id)
                                && to.as_deref() == Some(loss_id)
                    )
                });
            } else if figure_plan.is_none() || has_output_loss_edge {
                upsert_connector(
                    plan,
                    figure_plan_edge_id(figure_plan, output_id, loss_id)
                        .as_deref()
                        .unwrap_or("e_output_task_loss"),
                    output_id,
                    loss_id,
                    vec![
                        [output_box[2], center_y(output_box)],
                        [active_loss_box[0], center_y(active_loss_box)],
                    ],
                    "normal_flow",
                );
                set_connector_label(plan, output_id, loss_id, None);
            }
            plan.objects.retain(|object| {
                let DrawObject::Connector {
                    id,
                    from,
                    to,
                    label,
                    ..
                } = object
                else {
                    return true;
                };
                let is_loss_feedback = from.as_deref() == Some(loss_id)
                    && to.as_deref() == Some(student_id.as_str())
                    && !has_loss_student_edge;
                let is_duplicate_output_feedback = from.as_deref() == Some(output_id)
                    && to.as_deref() == Some(student_id.as_str())
                    && connector_says_loss(id, label.as_ref());
                !is_loss_feedback && !is_duplicate_output_feedback
            });
        }
    }
    if let Some(inference_student_id) = canonical_inference_student_id.as_deref() {
        upsert_connector(
            plan,
            "e_student_inference",
            &student_id,
            inference_student_id,
            vec![
                [student_box[2], student_box[1]],
                [inference_student_box[0], inference_student_box[1]],
            ],
            "normal_flow",
        );
        set_connector_label(plan, &student_id, inference_student_id, None);
        if let Some(inference_output_id) = canonical_inference_output_id.as_deref() {
            upsert_connector(
                plan,
                "e_inference_student_output",
                inference_student_id,
                inference_output_id,
                vec![
                    [inference_student_box[2], center_y(inference_student_box)],
                    [inference_output_box[0], center_y(inference_output_box)],
                ],
                "main_flow",
            );
            set_connector_label(plan, inference_student_id, inference_output_id, None);
        }
    }
    if let Some(latent_id) = latent_id.as_deref() {
        set_connector_label(plan, &teacher_id, latent_id, None);
    }
    if let Some(teacher_output_id) = teacher_output_id.as_deref() {
        set_connector_label(plan, &teacher_id, teacher_output_id, None);
        set_connector_label(plan, teacher_output_id, &student_id, None);
    }
    if let Some(output_id) = output_id.as_deref() {
        set_connector_label(plan, &student_id, output_id, None);
    }
}

fn find_student_output_id(plan: &DrawPlan, student_id: &str) -> Option<String> {
    find_connected_output_id(plan, student_id)
}

fn find_teacher_output_id(plan: &DrawPlan, teacher_id: &str) -> Option<String> {
    find_connected_output_id(plan, teacher_id).or_else(|| {
        find_box_id(plan, |id, text, role| {
            (id.contains("teacher_out") || id.contains("teacher_output"))
                && is_output_like(id, text, role)
        })
    })
}

fn find_connected_output_id(plan: &DrawPlan, source_id: &str) -> Option<String> {
    plan.objects.iter().find_map(|object| {
        let DrawObject::Connector { from, to, .. } = object else {
            return None;
        };
        if from.as_deref() != Some(source_id) {
            return None;
        }
        let target = to.as_deref()?;
        is_output_box_id(plan, target).then(|| target.to_string())
    })
}

fn is_output_box_id(plan: &DrawPlan, target_id: &str) -> bool {
    plan.objects.iter().any(|object| {
        let DrawObject::Box { id, text, role, .. } = object else {
            return false;
        };
        id == target_id && is_output_like(id, text, role)
    })
}

fn connector_says_loss(id: &str, label: Option<&DrawLabel>) -> bool {
    let id = id.to_lowercase();
    let label_text = label
        .map(|label| label.text.to_lowercase())
        .unwrap_or_default();
    id.contains("loss") || label_text.contains("loss")
}

fn figure_plan_loss_feedback_label(
    figure_plan: Option<&FigurePlan>,
    output_id: Option<&str>,
    student_id: &str,
) -> Option<String> {
    let figure_plan = figure_plan?;
    let output_id = output_id?;
    figure_plan.edges.iter().find_map(|edge| {
        let is_loss_feedback = edge.from == output_id
            && edge.to == student_id
            && (edge.semantic == EdgeSemantic::Loss || edge.label.to_lowercase().contains("loss"));
        is_loss_feedback.then(|| fallback_label(&edge.label, "Task Loss"))
    })
}

fn figure_plan_supervision_label(
    figure_plan: Option<&FigurePlan>,
    from_id: &str,
    to_id: &str,
) -> Option<String> {
    let figure_plan = figure_plan?;
    figure_plan.edges.iter().find_map(|edge| {
        let is_supervision_edge = edge.from == from_id
            && edge.to == to_id
            && (edge.semantic == EdgeSemantic::Supervision
                || edge.label.to_lowercase().contains("residual"));
        is_supervision_edge.then(|| fallback_label(&edge.label, "Latent Residual"))
    })
}

fn fallback_label(label: &str, fallback: &str) -> String {
    let label = label.trim();
    if label.is_empty() {
        fallback.to_string()
    } else {
        label.to_string()
    }
}

fn figure_plan_has_edge(figure_plan: Option<&FigurePlan>, from_id: &str, to_id: &str) -> bool {
    figure_plan
        .map(|plan| {
            plan.edges
                .iter()
                .any(|edge| edge.from == from_id && edge.to == to_id)
        })
        .unwrap_or(false)
}

fn figure_plan_edge_id(
    figure_plan: Option<&FigurePlan>,
    from_id: &str,
    to_id: &str,
) -> Option<String> {
    figure_plan?
        .edges
        .iter()
        .find_map(|edge| (edge.from == from_id && edge.to == to_id).then(|| edge.id.clone()))
}

#[derive(Clone, Debug)]
struct FigurePlanResidualBox {
    box_id: String,
}

fn figure_plan_residual_box(
    figure_plan: Option<&FigurePlan>,
    teacher_id: &str,
    student_id: &str,
) -> Option<FigurePlanResidualBox> {
    let figure_plan = figure_plan?;
    let component_ids = figure_plan
        .components
        .iter()
        .map(|component| component.id.as_str())
        .collect::<HashSet<_>>();

    figure_plan.edges.iter().find_map(|teacher_edge| {
        let is_residual_target = teacher_edge.to.to_lowercase().contains("residual")
            || figure_plan.components.iter().any(|component| {
                component.id == teacher_edge.to
                    && component.label.to_lowercase().contains("residual")
            });
        let is_teacher_residual = teacher_edge.from == teacher_id
            && teacher_edge.to != student_id
            && component_ids.contains(teacher_edge.to.as_str())
            && (teacher_edge.semantic == EdgeSemantic::Supervision || is_residual_target);
        if !is_teacher_residual {
            return None;
        }
        let has_student_supervision = figure_plan.edges.iter().any(|student_edge| {
            student_edge.from == teacher_edge.to
                && student_edge.to == student_id
                && student_edge.semantic == EdgeSemantic::Supervision
        });
        has_student_supervision.then(|| FigurePlanResidualBox {
            box_id: teacher_edge.to.clone(),
        })
    })
}

fn figure_plan_inference_student_id(
    figure_plan: Option<&FigurePlan>,
    canonical_student_id: &str,
) -> Option<String> {
    let figure_plan = figure_plan?;
    figure_plan.components.iter().find_map(|component| {
        let id = component.id.to_lowercase();
        let label = component.label.to_lowercase();
        let region = component.region.to_lowercase();
        let is_inference =
            id.contains("inf") || label.contains("infer") || region.contains("infer");
        let is_phase_badge = id.contains("badge") || id.contains("label");
        let is_student = id.contains("student") || label.trim() == "student";
        (component.id != canonical_student_id && is_inference && is_student && !is_phase_badge)
            .then(|| component.id.clone())
    })
}

fn figure_plan_teacher_latent_id(
    figure_plan: Option<&FigurePlan>,
    teacher_id: &str,
) -> Option<String> {
    let figure_plan = figure_plan?;
    figure_plan.edges.iter().find_map(|edge| {
        if edge.from != teacher_id {
            return None;
        }
        figure_plan.components.iter().find_map(|component| {
            let is_target = component.id == edge.to;
            let text = format!("{} {}", component.id, component.label).to_lowercase();
            let is_teacher_latent = text.contains("teacher_latent")
                || text.contains("latent h_t")
                || text.contains("latent hₜ");
            (is_target && is_teacher_latent).then(|| component.id.clone())
        })
    })
}

fn figure_plan_inference_label_id(figure_plan: Option<&FigurePlan>) -> Option<String> {
    let figure_plan = figure_plan?;
    figure_plan.components.iter().find_map(|component| {
        let id = component.id.to_lowercase();
        let label = component.label.trim().to_lowercase();
        let region = component.region.to_lowercase();
        let is_phase_label =
            id.contains("inference_label") || (label == "inference" && region.contains("infer"));
        is_phase_label.then(|| component.id.clone())
    })
}

fn figure_plan_inference_annotation_requests_lane(
    figure_plan: Option<&FigurePlan>,
    student_id: &str,
) -> bool {
    let Some(figure_plan) = figure_plan else {
        return false;
    };
    figure_plan.annotations.iter().any(|annotation| {
        let label = annotation.label.to_lowercase();
        let targets_student = annotation.target_id.as_deref() == Some(student_id);
        label.contains("inference") && (label.contains("only") || targets_student)
    })
}

fn figure_plan_inference_input_id(
    figure_plan: Option<&FigurePlan>,
    canonical_input_id: &str,
    inference_student_id: Option<&str>,
) -> Option<String> {
    let figure_plan = figure_plan?;
    let inference_student_id = inference_student_id?;
    figure_plan.edges.iter().find_map(|edge| {
        if edge.to != inference_student_id || edge.from == canonical_input_id {
            return None;
        }
        figure_plan.components.iter().find_map(|component| {
            let is_source = component.id == edge.from;
            let text =
                format!("{} {} {:?}", component.id, component.label, component.role).to_lowercase();
            let is_input = text.contains("input") || text.contains("task input");
            (is_source && is_input).then(|| component.id.clone())
        })
    })
}

fn figure_plan_inference_output_id(
    figure_plan: Option<&FigurePlan>,
    inference_student_id: Option<&str>,
) -> Option<String> {
    let figure_plan = figure_plan?;
    let inference_student_id = inference_student_id?;
    figure_plan.edges.iter().find_map(|edge| {
        if edge.from != inference_student_id {
            return None;
        }
        figure_plan.components.iter().find_map(|component| {
            let is_target = component.id == edge.to;
            let is_output = format!("{:?}", component.role)
                .to_lowercase()
                .contains("output")
                || component.id.to_lowercase().contains("out")
                || component.label.trim() == "ŷ";
            (is_target && is_output).then(|| component.id.clone())
        })
    })
}

fn figure_plan_residual_loss_id(
    figure_plan: Option<&FigurePlan>,
    student_id: &str,
    canonical_residual_id: Option<&str>,
) -> Option<String> {
    let figure_plan = figure_plan?;
    figure_plan.edges.iter().find_map(|edge| {
        if edge.to != student_id || edge.semantic != EdgeSemantic::Loss {
            return None;
        }
        if canonical_residual_id == Some(edge.from.as_str()) {
            return None;
        }
        figure_plan.components.iter().find_map(|component| {
            let is_source = component.id == edge.from;
            let text = format!("{} {}", component.id, component.label).to_lowercase();
            let is_residual_loss = text.contains("residual") && text.contains("loss");
            (is_source && is_residual_loss).then(|| component.id.clone())
        })
    })
}

#[derive(Clone, Debug)]
struct FigurePlanLatentPair {
    teacher_latent_id: String,
    student_latent_id: String,
    residual_label: String,
}

fn figure_plan_latent_pair(
    figure_plan: Option<&FigurePlan>,
    teacher_id: &str,
    student_id: &str,
) -> Option<FigurePlanLatentPair> {
    let figure_plan = figure_plan?;
    let component_ids = figure_plan
        .components
        .iter()
        .map(|component| component.id.as_str())
        .collect::<HashSet<_>>();

    figure_plan.edges.iter().find_map(|edge| {
        let from_component = figure_plan
            .components
            .iter()
            .find(|component| component.id == edge.from)?;
        let to_component = figure_plan
            .components
            .iter()
            .find(|component| component.id == edge.to)?;
        let from_text = format!("{} {}", from_component.id, from_component.label).to_lowercase();
        let to_text = format!("{} {}", to_component.id, to_component.label).to_lowercase();
        let from_is_teacher_latent = from_text.contains("latent")
            && (from_text.contains("teacher")
                || from_text.contains("h_t")
                || from_text.contains("hₜ"));
        let to_is_student_latent = to_text.contains("latent")
            && (to_text.contains("student") || to_text.contains("h_s") || to_text.contains("hₛ"));
        let is_latent_pair = edge.semantic == EdgeSemantic::Supervision
            && component_ids.contains(edge.from.as_str())
            && component_ids.contains(edge.to.as_str())
            && edge.from != teacher_id
            && edge.to != student_id
            && from_is_teacher_latent
            && to_is_student_latent;
        is_latent_pair.then(|| FigurePlanLatentPair {
            teacher_latent_id: edge.from.clone(),
            student_latent_id: edge.to.clone(),
            residual_label: fallback_label(&edge.label, "r = h_T - h_S"),
        })
    })
}

fn inference_student_id(plan: &DrawPlan, canonical_student_id: &str) -> Option<String> {
    plan.objects.iter().find_map(|object| {
        let DrawObject::Box { id, text, role, .. } = object else {
            return None;
        };
        is_inference_student_box(id, text, role, canonical_student_id).then(|| id.clone())
    })
}

fn inference_student_ids(plan: &DrawPlan, canonical_student_id: &str) -> HashSet<String> {
    plan.objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Box { id, text, role, .. } = object else {
                return None;
            };
            is_inference_student_box(id, text, role, canonical_student_id).then(|| id.clone())
        })
        .collect()
}

fn is_inference_student_box(id: &str, text: &str, role: &str, canonical_student_id: &str) -> bool {
    if id == canonical_student_id {
        return false;
    }
    let id_lower = id.to_lowercase();
    let text_lower = text.to_lowercase();
    let role_lower = role.to_lowercase();
    let is_inference = id_lower.contains("inference")
        || id_lower.contains("_inf")
        || id_lower.ends_with("inf")
        || role_lower.contains("inference")
        || text_lower.contains("inference");
    let is_student = id_lower.contains("student")
        || role_lower.contains("student")
        || text_lower.contains("student");
    is_inference && is_student
}

fn duplicate_inference_output_ids(
    plan: &DrawPlan,
    canonical_output_id: Option<&str>,
    duplicate_inference_ids: &HashSet<String>,
) -> HashSet<String> {
    if duplicate_inference_ids.is_empty() {
        return HashSet::new();
    }

    let connected_ids = plan
        .objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Connector { from, to, .. } = object else {
                return None;
            };
            let from_id = from.as_deref()?;
            let to_id = to.as_deref()?;
            if duplicate_inference_ids.contains(from_id) {
                Some(to_id.to_string())
            } else if duplicate_inference_ids.contains(to_id) {
                Some(from_id.to_string())
            } else {
                None
            }
        })
        .collect::<HashSet<_>>();

    plan.objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Box { id, text, role, .. } = object else {
                return None;
            };
            if canonical_output_id == Some(id.as_str()) || !connected_ids.contains(id) {
                return None;
            }
            is_output_like(id, text, role).then(|| id.clone())
        })
        .collect()
}

fn duplicate_training_output_ids(
    plan: &DrawPlan,
    canonical_output_id: Option<&str>,
    canonical_student_id: &str,
) -> HashSet<String> {
    let connected_to_student = plan
        .objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Connector { from, to, .. } = object else {
                return None;
            };
            let from_id = from.as_deref()?;
            let to_id = to.as_deref()?;
            if from_id == canonical_student_id {
                Some(to_id.to_string())
            } else {
                None
            }
        })
        .collect::<HashSet<_>>();

    plan.objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Box { id, text, role, .. } = object else {
                return None;
            };
            if canonical_output_id == Some(id.as_str()) || !connected_to_student.contains(id) {
                return None;
            }
            is_output_like(id, text, role).then(|| id.clone())
        })
        .collect()
}

fn duplicate_residual_ids(plan: &DrawPlan, canonical_residual_id: &str) -> HashSet<String> {
    plan.objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Box { id, text, .. } = object else {
                return None;
            };
            if id == canonical_residual_id {
                return None;
            }
            let id_lower = id.to_lowercase();
            let text_lower = text.to_lowercase();
            ((id_lower.contains("residual") || text_lower.contains("residual"))
                && !id_lower.contains("loss")
                && !text_lower.contains("loss"))
            .then(|| id.clone())
        })
        .collect()
}

fn loss_like_ids(plan: &DrawPlan) -> HashSet<String> {
    plan.objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Box { id, text, role, .. } = object else {
                return None;
            };
            let id_lower = id.to_lowercase();
            let text_lower = text.to_lowercase();
            let role_lower = role.to_lowercase();
            (id_lower.contains("loss")
                || text_lower.contains("loss")
                || role_lower.contains("loss"))
            .then(|| id.clone())
        })
        .collect()
}

fn auxiliary_hidden_state_ids(
    plan: &DrawPlan,
    required_ids: &[&str],
    optional_ids: &[Option<&str>],
) -> HashSet<String> {
    let canonical_ids = required_ids
        .iter()
        .copied()
        .chain(optional_ids.iter().copied().flatten())
        .collect::<HashSet<_>>();
    plan.objects
        .iter()
        .filter_map(|object| {
            let DrawObject::Box { id, text, .. } = object else {
                return None;
            };
            if canonical_ids.contains(id.as_str()) {
                return None;
            }
            is_hidden_state_box(id, text).then(|| id.clone())
        })
        .collect()
}

fn is_hidden_state_box(id: &str, text: &str) -> bool {
    let id = id.to_lowercase();
    let text = text.trim().to_lowercase();
    id.starts_with("h_")
        || id.contains("_h_")
        || id.contains("teacher_latent")
        || id.contains("student_latent")
        || text.contains("latent h")
        || text.contains("h_t")
        || text.contains("h_s")
        || matches!(
            text.as_str(),
            "h_t" | "h_s" | "hₜ" | "hₛ" | "h teacher" | "h student"
        )
}

fn is_output_like(id: &str, text: &str, role: &str) -> bool {
    let id = id.to_lowercase();
    let text = text.trim().to_lowercase();
    let role = role.to_lowercase();
    id.contains("output")
        || id.contains("_out")
        || role.contains("output")
        || matches!(text.as_str(), "ŷ" | "y_hat" | "yout" | "y_out")
}

fn is_redundant_inference_text(object: &DrawObject) -> bool {
    let DrawObject::Text {
        id, text, style, ..
    } = object
    else {
        return false;
    };
    let id = id.to_lowercase();
    let text = text.to_lowercase();
    let style = style.to_lowercase();
    id.contains("only_student")
        || text.contains("only student")
        || (style.contains("badge") && (text.contains("training") || text.contains("inference")))
        || ((id.contains("inference") || text.contains("inference"))
            && (style.contains("badge") || style.contains("annotation") || style.contains("phase")))
}

fn upsert_box(
    plan: &mut DrawPlan,
    preferred_id: &str,
    bbox: [f64; 4],
    text: &str,
    role: &str,
    style: &str,
) -> String {
    if let Some(existing_id) = plan.objects.iter().find_map(|object| {
        let DrawObject::Box { id, .. } = object else {
            return None;
        };
        (id == preferred_id).then(|| id.clone())
    }) {
        for object in &mut plan.objects {
            let DrawObject::Box {
                id,
                bbox: existing_bbox,
                text: existing_text,
                role: existing_role,
                style: existing_style,
                ..
            } = object
            else {
                continue;
            };
            if *id == existing_id {
                *existing_bbox = bbox;
                *existing_text = text.to_string();
                *existing_role = role.to_string();
                *existing_style = style.to_string();
                break;
            }
        }
        return existing_id;
    }

    let id = unique_draw_object_id(plan, preferred_id);
    plan.objects.push(DrawObject::Box {
        id: id.clone(),
        bbox,
        text: text.to_string(),
        role: role.to_string(),
        style: style.to_string(),
        z: next_z(plan),
    });
    id
}

fn upsert_connector(
    plan: &mut DrawPlan,
    id: &str,
    from_id: &str,
    to_id: &str,
    points: Vec<[f64; 2]>,
    style: &str,
) {
    for object in &mut plan.objects {
        let DrawObject::Connector {
            from,
            to,
            points: existing_points,
            style: existing_style,
            ..
        } = object
        else {
            continue;
        };
        if from.as_deref() == Some(from_id) && to.as_deref() == Some(to_id) {
            *existing_points = points;
            *existing_style = style.to_string();
            return;
        }
    }

    let connector_id = unique_draw_object_id(plan, id);
    plan.objects.push(DrawObject::Connector {
        id: connector_id,
        points,
        from: Some(from_id.to_string()),
        to: Some(to_id.to_string()),
        style: style.to_string(),
        label: None,
        z: next_z(plan),
    });
}

fn set_connector_label(
    plan: &mut DrawPlan,
    from_id: &str,
    to_id: &str,
    new_label: Option<DrawLabel>,
) {
    for object in &mut plan.objects {
        let DrawObject::Connector {
            from, to, label, ..
        } = object
        else {
            continue;
        };
        if from.as_deref() == Some(from_id) && to.as_deref() == Some(to_id) {
            *label = new_label.clone();
        }
    }
}

fn unique_draw_object_id(plan: &DrawPlan, preferred: &str) -> String {
    if !plan
        .objects
        .iter()
        .any(|object| draw_object_id(object) == preferred)
    {
        return preferred.to_string();
    }
    let mut index = 1;
    loop {
        let candidate = format!("{preferred}_{index}");
        if !plan
            .objects
            .iter()
            .any(|object| draw_object_id(object) == candidate)
        {
            return candidate;
        }
        index += 1;
    }
}

fn next_z(plan: &DrawPlan) -> i32 {
    plan.objects
        .iter()
        .map(|object| match object {
            DrawObject::Box { z, .. }
            | DrawObject::Text { z, .. }
            | DrawObject::Connector { z, .. }
            | DrawObject::Image { z, .. }
            | DrawObject::Group { z, .. } => *z,
        })
        .max()
        .unwrap_or(0)
        + 1
}

fn find_box_id<F>(plan: &DrawPlan, predicate: F) -> Option<String>
where
    F: Fn(&str, &str, &str) -> bool,
{
    plan.objects.iter().find_map(|object| {
        let DrawObject::Box { id, text, role, .. } = object else {
            return None;
        };
        let id_lower = id.to_lowercase();
        let text_lower = text.to_lowercase();
        let role_lower = role.to_lowercase();
        predicate(&id_lower, &text_lower, &role_lower).then(|| id.clone())
    })
}

fn packed_component_boxes(plan: &FigurePlan) -> HashMap<&str, [f64; 4]> {
    if let Some(boxes) = packed_multimodal_fusion_boxes(plan) {
        return boxes;
    }

    let mut by_region: BTreeMap<&str, Vec<&Component>> = BTreeMap::new();
    for component in &plan.components {
        by_region
            .entry(component.region.as_str())
            .or_default()
            .push(component);
    }

    let mut boxes = HashMap::new();
    for region in &plan.layout.regions {
        let Some(components) = by_region.get(region.id.as_str()) else {
            continue;
        };
        let packed = pack_region(region.bbox, components.len());
        for (component, bbox) in components.iter().zip(packed) {
            boxes.insert(component.id.as_str(), bbox);
        }
    }
    boxes
}

fn packed_multimodal_fusion_boxes(plan: &FigurePlan) -> Option<HashMap<&str, [f64; 4]>> {
    if plan.layout.template != Template::MultimodalFusion || plan.components.len() != 4 {
        return None;
    }
    let component_by_id = plan
        .components
        .iter()
        .map(|component| (component.id.as_str(), component))
        .collect::<HashMap<_, _>>();
    let vision = component_by_id.get("vision_encoder").copied()?;
    let text = component_by_id.get("text_encoder").copied()?;
    let fusion = component_by_id.get("fusion").copied()?;
    let head = component_by_id.get("head").copied()?;
    let region = plan
        .layout
        .regions
        .iter()
        .find(|region| region.id == fusion.region)
        .or_else(|| plan.layout.regions.first())?;

    let [x1, y1, x2, y2] = inset_box(region.bbox, adaptive_padding(region.bbox));
    let inner_width = (x2 - x1).max(0.001);
    let inner_height = (y2 - y1).max(0.001);
    let box_width = (inner_width * 0.23).clamp(0.14, 0.22);
    let box_height = (inner_height * 0.32).clamp(0.12, 0.20);
    let left_x = x1;
    let fusion_x = (x1 + inner_width * 0.52 - box_width / 2.0).clamp(x1, x2 - box_width);
    let head_x = (x2 - box_width).clamp(x1, x2 - box_width);
    let top_center_y = y1 + inner_height * 0.25;
    let mid_center_y = y1 + inner_height * 0.50;
    let bottom_center_y = y1 + inner_height * 0.75;

    let mut boxes = HashMap::new();
    boxes.insert(
        vision.id.as_str(),
        box_from_left_center(
            left_x,
            top_center_y,
            box_width,
            box_height,
            [x1, y1, x2, y2],
        ),
    );
    boxes.insert(
        text.id.as_str(),
        box_from_left_center(
            left_x,
            bottom_center_y,
            box_width,
            box_height,
            [x1, y1, x2, y2],
        ),
    );
    boxes.insert(
        fusion.id.as_str(),
        box_from_left_center(
            fusion_x,
            mid_center_y,
            box_width,
            box_height,
            [x1, y1, x2, y2],
        ),
    );
    boxes.insert(
        head.id.as_str(),
        box_from_left_center(
            head_x,
            mid_center_y,
            box_width,
            box_height,
            [x1, y1, x2, y2],
        ),
    );
    Some(boxes)
}

fn box_from_left_center(
    left: f64,
    center_y: f64,
    width: f64,
    height: f64,
    bounds: [f64; 4],
) -> [f64; 4] {
    let left = left.clamp(bounds[0], bounds[2] - width);
    let top = (center_y - height / 2.0).clamp(bounds[1], bounds[3] - height);
    normalize_box([left, top, left + width, top + height])
}

fn pack_region(region: [f64; 4], count: usize) -> Vec<[f64; 4]> {
    if count == 0 {
        return vec![];
    }
    if count == 1 {
        return vec![inset_box(region, adaptive_padding(region))];
    }

    let width = box_width(region);
    let height = box_height(region);
    let gap = adaptive_gap(region);
    let columns = if width >= height * 1.3 {
        count
    } else if count == 2 {
        if width >= height {
            2
        } else {
            1
        }
    } else {
        let aspect = width / height.max(0.001);
        ((count as f64 * aspect).sqrt().ceil() as usize)
            .max(1)
            .min(count)
    };
    let rows = count.div_ceil(columns);
    let [x1, y1, x2, y2] = inset_box(region, adaptive_padding(region));
    let inner_width = (x2 - x1).max(0.001);
    let inner_height = (y2 - y1).max(0.001);
    let cell_width =
        ((inner_width - gap * (columns.saturating_sub(1)) as f64) / columns as f64).max(0.001);
    let cell_height =
        ((inner_height - gap * (rows.saturating_sub(1)) as f64) / rows as f64).max(0.001);

    let mut boxes = Vec::with_capacity(count);
    for index in 0..count {
        let row = index / columns;
        let column = index % columns;
        let cell = [
            x1 + column as f64 * (cell_width + gap),
            y1 + row as f64 * (cell_height + gap),
            x1 + column as f64 * (cell_width + gap) + cell_width,
            y1 + row as f64 * (cell_height + gap) + cell_height,
        ];
        boxes.push(inset_box(cell, (gap * 0.35).min(0.01)));
    }
    boxes
}

fn adaptive_padding(bbox: [f64; 4]) -> f64 {
    0.028_f64
        .min(box_width(bbox) * 0.14)
        .min(box_height(bbox) * 0.14)
}

fn adaptive_gap(bbox: [f64; 4]) -> f64 {
    0.026_f64
        .min(box_width(bbox) * 0.09)
        .min(box_height(bbox) * 0.09)
}

fn inset_box(bbox: [f64; 4], padding: f64) -> [f64; 4] {
    let [x1, y1, x2, y2] = normalize_box(bbox);
    let max_x = ((x2 - x1 - 0.025) / 2.0).max(0.0);
    let max_y = ((y2 - y1 - 0.025) / 2.0).max(0.0);
    let px = padding.min(max_x);
    let py = padding.min(max_y);
    normalize_box([x1 + px, y1 + py, x2 - px, y2 - py])
}

fn normalize_box(bbox: [f64; 4]) -> [f64; 4] {
    let x1 = bbox[0].clamp(0.0, 1.0);
    let y1 = bbox[1].clamp(0.0, 1.0);
    let x2 = bbox[2].clamp(0.0, 1.0);
    let y2 = bbox[3].clamp(0.0, 1.0);
    [x1.min(x2), y1.min(y2), x1.max(x2), y1.max(y2)]
}

fn shift_box_inside_canvas(bbox: [f64; 4]) -> [f64; 4] {
    if bbox.iter().all(|value| value.is_finite())
        && bbox.iter().all(|value| (0.0..=1.0).contains(value))
        && bbox[2] > bbox[0]
        && bbox[3] > bbox[1]
    {
        return bbox;
    }
    let x1 = finite_or(bbox[0], 0.0);
    let y1 = finite_or(bbox[1], 0.0);
    let x2 = finite_or(bbox[2], x1 + 0.12);
    let y2 = finite_or(bbox[3], y1 + 0.08);
    let left = x1.min(x2);
    let right = x1.max(x2);
    let top = y1.min(y2);
    let bottom = y1.max(y2);
    let width = (right - left).clamp(0.02, 0.96);
    let height = (bottom - top).clamp(0.02, 0.96);
    let center_x = finite_or((left + right) / 2.0, 0.5).clamp(width / 2.0, 1.0 - width / 2.0);
    let center_y = finite_or((top + bottom) / 2.0, 0.5).clamp(height / 2.0, 1.0 - height / 2.0);
    [
        center_x - width / 2.0,
        center_y - height / 2.0,
        center_x + width / 2.0,
        center_y + height / 2.0,
    ]
}

fn finite_or(value: f64, fallback: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
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

fn is_marginal_annotation(object: &DrawObject, component_union: Option<[f64; 4]>) -> bool {
    let DrawObject::Text {
        id,
        text,
        bbox,
        style,
        ..
    } = object
    else {
        return false;
    };
    let style_lower = style.to_lowercase();
    if !style_lower.contains("annotation")
        && !style_lower.contains("phase")
        && !style_lower.contains("section")
    {
        return false;
    }
    if style_lower.contains("annotation")
        && (id == "ann_inference" || id.starts_with("ann_inference_"))
        && annotation_label_is_inference_specific(text)
    {
        return false;
    }
    if style_lower.contains("annotation") && is_teacher_state_annotation_label(id, text) {
        return false;
    }
    if style_lower.contains("section") {
        return true;
    }
    if box_area(*bbox) > 0.05 {
        return true;
    }
    if style_lower.contains("phase")
        || id.starts_with("ann_")
        || id.starts_with("anno_")
        || id.starts_with("phase_")
        || id.starts_with("a_")
    {
        if bbox[0] < 0.08 || bbox[1] < 0.08 || bbox[2] > 0.92 || bbox[3] > 0.92 {
            return true;
        }
    }
    if let Some(component_union) = component_union {
        let expanded = expand_box(component_union, 0.06);
        let keep = boxes_overlap(expanded, *bbox);
        return !keep;
    }
    false
}

fn polished_connector_points(points: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let points = orthogonalized_points(points);
    simplify_dogleg_points(&points)
}

fn orthogonalized_points(points: &[[f64; 2]]) -> Vec<[f64; 2]> {
    if points.len() < 2 {
        return points.to_vec();
    }
    let mut repaired = vec![clamp_point(points[0])];
    for point in points.iter().skip(1).copied().map(clamp_point) {
        let start = *repaired.last().expect("repaired has first point");
        if is_diagonal_segment(start, point) {
            let mid_x = (start[0] + point[0]) / 2.0;
            push_distinct(&mut repaired, [mid_x, start[1]]);
            push_distinct(&mut repaired, [mid_x, point[1]]);
        }
        push_distinct(&mut repaired, point);
    }
    repaired
}

fn simplify_dogleg_points(points: &[[f64; 2]]) -> Vec<[f64; 2]> {
    if points.len() != 4 {
        return remove_redundant_collinear_points(points);
    }
    let [p0, p1, p2, p3] = [points[0], points[1], points[2], points[3]];
    if same_y(p0, p1) && same_x(p1, p2) && same_y(p2, p3) {
        return remove_redundant_collinear_points(&[p0, [p0[0], p3[1]], p3]);
    }
    if same_x(p0, p1) && same_y(p1, p2) && same_x(p2, p3) {
        return remove_redundant_collinear_points(&[p0, [p3[0], p0[1]], p3]);
    }
    remove_redundant_collinear_points(points)
}

fn remove_redundant_collinear_points(points: &[[f64; 2]]) -> Vec<[f64; 2]> {
    if points.len() <= 2 {
        return points.to_vec();
    }
    let mut cleaned = Vec::with_capacity(points.len());
    for point in points {
        cleaned.push(*point);
        while cleaned.len() >= 3 {
            let len = cleaned.len();
            let a = cleaned[len - 3];
            let b = cleaned[len - 2];
            let c = cleaned[len - 1];
            if (same_x(a, b) && same_x(b, c)) || (same_y(a, b) && same_y(b, c)) {
                cleaned.remove(len - 2);
            } else {
                break;
            }
        }
    }
    cleaned
}

fn polyline_length(points: &[[f64; 2]]) -> f64 {
    points
        .windows(2)
        .map(|window| {
            let dx = window[1][0] - window[0][0];
            let dy = window[1][1] - window[0][1];
            (dx * dx + dy * dy).sqrt()
        })
        .sum()
}

fn polylines_cross(left: &[[f64; 2]], right: &[[f64; 2]]) -> bool {
    left.windows(2).any(|left_window| {
        right.windows(2).any(|right_window| {
            segments_cross(
                (left_window[0], left_window[1]),
                (right_window[0], right_window[1]),
            )
        })
    })
}

fn segments_cross(a: ([f64; 2], [f64; 2]), b: ([f64; 2], [f64; 2])) -> bool {
    if segment_length(a) < 0.04 || segment_length(b) < 0.04 {
        return false;
    }
    let p1 = (a.0[0], a.0[1]);
    let p2 = (a.1[0], a.1[1]);
    let q1 = (b.0[0], b.0[1]);
    let q2 = (b.1[0], b.1[1]);

    if crossing_points_close(p1, q1)
        || crossing_points_close(p1, q2)
        || crossing_points_close(p2, q1)
        || crossing_points_close(p2, q2)
    {
        return false;
    }

    let o1 = crossing_orientation(p1, p2, q1);
    let o2 = crossing_orientation(p1, p2, q2);
    let o3 = crossing_orientation(q1, q2, p1);
    let o4 = crossing_orientation(q1, q2, p2);
    o1 * o2 < 0.0 && o3 * o4 < 0.0
}

fn segment_length(segment: ([f64; 2], [f64; 2])) -> f64 {
    let dx = segment.1[0] - segment.0[0];
    let dy = segment.1[1] - segment.0[1];
    (dx * dx + dy * dy).sqrt()
}

fn crossing_orientation(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> f64 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

fn crossing_points_close(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 0.01 && (a.1 - b.1).abs() < 0.01
}

fn same_x(left: [f64; 2], right: [f64; 2]) -> bool {
    (left[0] - right[0]).abs() < 0.0001
}

fn same_y(left: [f64; 2], right: [f64; 2]) -> bool {
    (left[1] - right[1]).abs() < 0.0001
}

fn place_label_outside_edge(label_bbox: [f64; 4], points: &[[f64; 2]]) -> [f64; 4] {
    let edge_bbox = points_to_box(points);
    let label_bbox = normalize_box(label_bbox);
    if !label_needs_offset(label_bbox, edge_bbox)
        && label_inside_safe_area(label_bbox)
        && label_near_edge(label_bbox, edge_bbox)
    {
        return label_bbox;
    }
    let width = box_width(label_bbox).clamp(0.04, 0.16);
    let height = box_height(label_bbox).clamp(0.04, 0.07);
    if box_width(edge_bbox) < 0.01 && box_height(edge_bbox) > 0.04 {
        let gap = 0.035;
        let y1 = (center_y(edge_bbox) - height / 2.0).clamp(0.02, 0.98 - height);
        let right_x = edge_bbox[2] + gap;
        if right_x + width <= 0.98 {
            return [right_x, y1, right_x + width, y1 + height];
        }
        let left_x = (edge_bbox[0] - gap - width).max(0.02);
        return [left_x, y1, left_x + width, y1 + height];
    }
    let center_x = ((edge_bbox[0] + edge_bbox[2]) / 2.0).clamp(width / 2.0, 1.0 - width / 2.0);
    let above_y = edge_bbox[1] - height - 0.02;
    let below_y = edge_bbox[3] + 0.02;
    let y1 = if above_y >= 0.02 && (edge_bbox[1] > 0.14 || below_y + height > 0.98) {
        above_y
    } else {
        below_y
    }
    .clamp(0.02, 0.98 - height);
    [
        center_x - width / 2.0,
        y1,
        center_x + width / 2.0,
        y1 + height,
    ]
}

fn label_inside_safe_area(bbox: [f64; 4]) -> bool {
    let bbox = normalize_box(bbox);
    bbox[0] >= 0.02 && bbox[1] >= 0.02 && bbox[2] <= 0.98 && bbox[3] <= 0.98
}

fn label_needs_offset(label_bbox: [f64; 4], edge_bbox: [f64; 4]) -> bool {
    let expanded_edge = expand_box(edge_bbox, 0.02);
    boxes_overlap(label_bbox, expanded_edge)
}

fn label_near_edge(label_bbox: [f64; 4], edge_bbox: [f64; 4]) -> bool {
    boxes_overlap(label_bbox, expand_box(edge_bbox, 0.16))
}

fn shift_label(bbox: [f64; 4], amount: f64) -> [f64; 4] {
    let bbox = normalize_box(bbox);
    let height = box_height(bbox);
    let y1 = if bbox[3] + amount <= 0.96 {
        bbox[1] + amount
    } else {
        (bbox[1] - amount).max(0.02)
    };
    [bbox[0], y1, bbox[2], y1 + height]
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

fn clamp_point(point: [f64; 2]) -> [f64; 2] {
    [
        finite_or(point[0], 0.5).clamp(0.0, 1.0),
        finite_or(point[1], 0.5).clamp(0.0, 1.0),
    ]
}

fn push_distinct(points: &mut Vec<[f64; 2]>, point: [f64; 2]) {
    if points
        .last()
        .map(|last| (last[0] - point[0]).abs() < 0.0001 && (last[1] - point[1]).abs() < 0.0001)
        .unwrap_or(false)
    {
        return;
    }
    points.push(point);
}

fn is_diagonal_segment(start: [f64; 2], end: [f64; 2]) -> bool {
    (start[0] - end[0]).abs() > 0.025 && (start[1] - end[1]).abs() > 0.025
}

fn expand_box(bbox: [f64; 4], margin: f64) -> [f64; 4] {
    [
        (bbox[0] - margin).clamp(0.0, 1.0),
        (bbox[1] - margin).clamp(0.0, 1.0),
        (bbox[2] + margin).clamp(0.0, 1.0),
        (bbox[3] + margin).clamp(0.0, 1.0),
    ]
}

fn boxes_overlap(a: [f64; 4], b: [f64; 4]) -> bool {
    let a = normalize_box(a);
    let b = normalize_box(b);
    a[0].max(b[0]) < a[2].min(b[2]) && a[1].max(b[1]) < a[3].min(b[3])
}

fn intersection_area(a: [f64; 4], b: [f64; 4]) -> f64 {
    let a = normalize_box(a);
    let b = normalize_box(b);
    let x1 = a[0].max(b[0]);
    let y1 = a[1].max(b[1]);
    let x2 = a[2].min(b[2]);
    let y2 = a[3].min(b[3]);
    ((x2 - x1).max(0.0)) * ((y2 - y1).max(0.0))
}

fn intersection_dimensions(a: [f64; 4], b: [f64; 4]) -> (f64, f64) {
    let a = normalize_box(a);
    let b = normalize_box(b);
    let x1 = a[0].max(b[0]);
    let y1 = a[1].max(b[1]);
    let x2 = a[2].min(b[2]);
    let y2 = a[3].min(b[3]);
    ((x2 - x1).max(0.0), (y2 - y1).max(0.0))
}

fn box_area(bbox: [f64; 4]) -> f64 {
    box_width(bbox) * box_height(bbox)
}

fn box_width(bbox: [f64; 4]) -> f64 {
    let bbox = normalize_box(bbox);
    bbox[2] - bbox[0]
}

fn box_height(bbox: [f64; 4]) -> f64 {
    let bbox = normalize_box(bbox);
    bbox[3] - bbox[1]
}

fn center_x(bbox: [f64; 4]) -> f64 {
    let bbox = normalize_box(bbox);
    (bbox[0] + bbox[2]) / 2.0
}

fn center_y(bbox: [f64; 4]) -> f64 {
    let bbox = normalize_box(bbox);
    (bbox[1] + bbox[3]) / 2.0
}

pub fn generate_draw_plan_typescript(
    draw_plan: &DrawPlan,
    style: &StyleSpec,
    round_dir: &Path,
    renderer_root: &Path,
    asset_paths: &BTreeMap<String, PathBuf>,
) -> Result<String> {
    write_draw_plan_renderer_payload(draw_plan, style, round_dir, asset_paths)?;
    let runtime_path = absolutize(renderer_root).join("src/runtime.ts");
    Ok(format!(
        r#"import {{ createDrawPlanRuntimeFromEnv }} from "{}";

async function main() {{
  const runtime = createDrawPlanRuntimeFromEnv();
  await runtime.renderDrawPlan();
}}

main().catch((error) => {{
  console.error(error);
  process.exit(1);
}});
"#,
        escape_ts_path(&runtime_path)
    ))
}

pub fn write_draw_plan_renderer_payload(
    draw_plan: &DrawPlan,
    style: &StyleSpec,
    round_dir: &Path,
    asset_paths: &BTreeMap<String, PathBuf>,
) -> Result<PathBuf> {
    fs::create_dir_all(round_dir)?;
    let payload = DrawRenderPayload {
        out_dir: absolutize(round_dir),
        draw_plan,
        style,
        asset_paths,
    };
    let path = round_dir.join("renderer_payload.json");
    fs::write(&path, serde_json::to_vec_pretty(&payload)?)?;
    Ok(path)
}

#[derive(Serialize)]
struct DrawRenderPayload<'a> {
    out_dir: PathBuf,
    draw_plan: &'a DrawPlan,
    style: &'a StyleSpec,
    asset_paths: &'a BTreeMap<String, PathBuf>,
}

fn center(bbox: [f64; 4]) -> [f64; 2] {
    [(bbox[0] + bbox[2]) / 2.0, (bbox[1] + bbox[3]) / 2.0]
}

fn inset_asset_box(bbox: [f64; 4]) -> [f64; 4] {
    let width = bbox[2] - bbox[0];
    let height = bbox[3] - bbox[1];
    let size = width.min(height) * 0.30;
    [
        bbox[0] + width * 0.08,
        bbox[1] + height * 0.12,
        bbox[0] + width * 0.08 + size,
        bbox[1] + height * 0.12 + size,
    ]
}

fn connector_label_bbox(start: [f64; 2], end: [f64; 2]) -> [f64; 4] {
    let mx = (start[0] + end[0]) / 2.0;
    let my = (start[1] + end[1]) / 2.0;
    let y_offset = if my > 0.5 { -0.08 } else { 0.08 };
    [
        (mx - 0.08).clamp(0.0, 0.96),
        (my + y_offset - 0.025).clamp(0.0, 0.95),
        (mx + 0.08).clamp(0.04, 1.0),
        (my + y_offset + 0.025).clamp(0.05, 1.0),
    ]
}

fn escape_ts_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

fn absolutize(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readable_shared_input_width_handles_float_width_below_floor() {
        let input = BoxRouteInfo {
            bbox: [0.10, 0.20, 0.19999999999999998, 0.30],
            text: "Input x".to_string(),
            role: "input".to_string(),
            style: "neutral_module".to_string(),
        };

        let width = readable_shared_input_width(&input);

        assert!(
            (0.10..=0.16).contains(&width),
            "readable shared input width should stay inside the intended readability range: {width}"
        );
    }

    #[test]
    fn simple_y_teacher_candidate_handles_narrow_student_width_below_floor() {
        let teacher_bbox = [0.10, 0.60, 0.24, 0.76];
        let student_bbox = [0.40, 0.62, 0.54, 0.82];
        let box_map = HashMap::from([
            (
                "teacher".to_string(),
                BoxRouteInfo {
                    bbox: teacher_bbox,
                    text: "Teacher".to_string(),
                    role: "context".to_string(),
                    style: "muted_module".to_string(),
                },
            ),
            (
                "student".to_string(),
                BoxRouteInfo {
                    bbox: student_bbox,
                    text: "Student".to_string(),
                    role: "main".to_string(),
                    style: "primary_module".to_string(),
                },
            ),
        ]);

        let candidate = simple_y_teacher_above_student_candidate(
            "teacher",
            teacher_bbox,
            student_bbox,
            &box_map,
        );

        assert!(
            candidate.is_some(),
            "narrow student width should expand to the readability floor instead of panicking"
        );
    }

    #[test]
    fn simple_y_balanced_teacher_candidate_handles_narrow_student_width_below_floor() {
        let teacher_bbox = [0.10, 0.60, 0.36, 0.74];
        let student_bbox = [0.52, 0.60, 0.66, 0.74];
        let box_map = HashMap::from([
            (
                "teacher".to_string(),
                BoxRouteInfo {
                    bbox: teacher_bbox,
                    text: "Teacher".to_string(),
                    role: "context".to_string(),
                    style: "muted_module".to_string(),
                },
            ),
            (
                "student".to_string(),
                BoxRouteInfo {
                    bbox: student_bbox,
                    text: "Student".to_string(),
                    role: "main".to_string(),
                    style: "primary_module".to_string(),
                },
            ),
        ]);

        let candidate =
            simple_y_balanced_teacher_candidate("teacher", teacher_bbox, student_bbox, &box_map);

        assert!(
            candidate.is_some(),
            "balanced simple-Y repair should not panic when student width is below readability floor"
        );
    }
}
