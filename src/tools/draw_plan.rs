use std::collections::{BTreeMap, HashMap, HashSet};
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

pub fn repair_draw_plan_geometry(plan: &mut DrawPlan) {
    repair_draw_plan_geometry_inner(plan, None);
}

pub fn repair_draw_plan_geometry_with_figure_plan(plan: &mut DrawPlan, figure_plan: &FigurePlan) {
    repair_draw_plan_geometry_inner(plan, Some(figure_plan));
}

pub fn polish_model_draw_plan_geometry(plan: &mut DrawPlan) {
    polish_model_draw_plan_geometry_inner(plan, &HashSet::new());
}

fn polish_model_draw_plan_geometry_inner(
    plan: &mut DrawPlan,
    protected_note_ids: &HashSet<String>,
) {
    plan.objects
        .retain(|object| !is_phase_only_text_annotation(object));
    remove_asymmetric_branch_annotations(plan);
    remove_template_reference_and_overlapping_path_annotations(plan);
    fold_standalone_inference_notes_into_student_annotations(plan, protected_note_ids);
    expand_tiny_model_boxes(plan);
    align_shared_input_boxes_with_branch_targets(plan);
    align_output_boxes_with_sources(plan);
    align_task_loss_boxes_with_outputs(plan);
    align_touching_task_loss_boxes_with_sources(plan);
    resolve_model_box_overlaps(plan);
    repair_degenerate_connector_points_from_boxes(plan);
    improve_connector_routes_against_boxes(plan);
    polish_labels_and_marginal_annotations(plan, true);
    improve_connector_routes_against_boxes(plan);
    reroute_objective_feedback_away_from_reverse_shared_segments(plan);
    reroute_output_to_task_loss_around_intermediate_boxes(plan);
    remove_duplicate_connectors(plan);
    fold_noisy_connected_inference_notes_into_student_annotations(plan, protected_note_ids);
    remove_duplicate_inference_text_when_note_component_exists(plan);
    move_inference_note_boxes_out_of_flow_corridors(plan);
    snap_connector_labels_to_final_routes(plan);
}

pub fn polish_model_draw_plan_geometry_with_figure_plan(
    plan: &mut DrawPlan,
    figure_plan: &FigurePlan,
) {
    normalize_figure_plan_component_objects(plan, figure_plan);
    add_missing_component_boxes_from_figure_plan(plan, figure_plan);
    sync_connector_styles_from_figure_plan(plan, figure_plan);
    remove_connectors_absent_from_figure_plan(plan, figure_plan);
    reposition_note_components_near_sources(plan, figure_plan);
    add_missing_connectors_from_figure_plan(plan, figure_plan);
    reposition_note_components_near_sources(plan, figure_plan);
    let protected_note_ids = figure_plan
        .components
        .iter()
        .map(|component| component.id.clone())
        .collect::<HashSet<_>>();
    polish_model_draw_plan_geometry_inner(plan, &protected_note_ids);
    upsert_meaningful_annotations_from_figure_plan(plan, figure_plan);
    remove_duplicate_inference_text_when_note_component_exists(plan);
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
    let box_map = current_box_map(plan);
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
            } if is_standalone_inference_note_box(id, text, role, style) => Some(id.clone()),
            _ => None,
        })
        .collect::<HashSet<_>>();
    if note_ids.is_empty() {
        return;
    }
    let note_id_refs = note_ids.iter().map(String::as_str).collect::<HashSet<_>>();
    let mut moved_ids = HashSet::new();
    for note_id in &note_ids {
        let Some(note_bbox) = box_map.get(note_id.as_str()).copied() else {
            continue;
        };
        if !note_component_conflicts_with_connector_segments(plan, note_id, note_bbox) {
            continue;
        }
        let Some(source_box) = largest_student_box_id(plan, &note_ids)
            .and_then(|id| box_map.get(id.as_str()).copied())
        else {
            continue;
        };
        let next_bbox = clear_adjacent_note_box(
            note_id,
            note_bbox,
            source_box,
            &box_map,
            &note_id_refs,
            plan,
        )
        .unwrap_or_else(|| adjacent_note_box(note_bbox, source_box));
        if !boxes_nearly_equal(note_bbox, next_bbox) {
            set_box_bbox(plan, note_id, next_bbox);
            moved_ids.insert(note_id.clone());
        }
    }
    if !moved_ids.is_empty() {
        realign_connector_endpoints_for_moved_boxes(plan, &moved_ids);
    }
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
    if lower.contains("student only") || lower.contains("only student") {
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

fn inference_annotation_bbox_near_student(student_bbox: [f64; 4]) -> [f64; 4] {
    let student_bbox = normalize_box(student_bbox);
    let width = 0.28;
    let height = 0.06;
    let x1 = center_x(student_bbox).clamp(0.06 + width / 2.0, 0.94 - width / 2.0) - width / 2.0;
    let y1 = if student_bbox[1] > 0.16 {
        student_bbox[1] - height - 0.04
    } else {
        student_bbox[3] + 0.04
    }
    .clamp(0.06, 0.94 - height);
    [x1, y1, x1 + width, y1 + height]
}

fn upsert_meaningful_annotations_from_figure_plan(plan: &mut DrawPlan, figure_plan: &FigurePlan) {
    for annotation in &figure_plan.annotations {
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
    let has_teacher_annotation = plan.objects.iter().any(|object| {
        matches!(
            object,
            DrawObject::Text { text, style, .. }
                if is_branch_annotation_text(text, style, "teacher")
        )
    });
    let has_student_annotation = plan.objects.iter().any(|object| {
        matches!(
            object,
            DrawObject::Text { text, style, .. }
                if is_branch_annotation_text(text, style, "student")
        )
    });
    if has_teacher_annotation == has_student_annotation {
        return;
    }

    plan.objects.retain(|object| {
        !matches!(
            object,
            DrawObject::Text { text, style, .. }
                if is_branch_annotation_text(text, style, "teacher")
                    || is_branch_annotation_text(text, style, "student")
        )
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
            Some((id.clone(), points.clone(), label.bbox))
        })
        .collect::<Vec<_>>();

    let mut placed_labels: Vec<[f64; 4]> = Vec::new();
    let mut updates: Vec<(String, [f64; 4])> = Vec::new();
    for (id, points, label_bbox) in connector_labels {
        let candidates = connector_label_candidates_near_route(label_bbox, &points);
        let snapped = candidates
            .iter()
            .copied()
            .find(|candidate| {
                connector_label_candidate_clear(
                    *candidate,
                    &obstacle_boxes,
                    &segment_boxes,
                    &placed_labels,
                )
            })
            .or_else(|| {
                candidates.iter().copied().find(|candidate| {
                    connector_label_candidate_line_clear(*candidate, &segment_boxes, &placed_labels)
                })
            })
            .unwrap_or_else(|| place_label_outside_edge(label_bbox, &points));
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
        id, bbox, style, ..
    } = object
    else {
        return false;
    };
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
        if current_y >= target_min - 0.08 && current_y <= target_max + 0.08 {
            continue;
        }
        let height = box_height(input.bbox);
        let desired_y =
            ((target_min + target_max) / 2.0).clamp(0.06 + height / 2.0, 0.94 - height / 2.0);
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
        if overlap > 0.003 && ratio > 0.15 {
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
    overlap > 0.003 && overlap / box_area(left).min(box_area(right)).max(0.0001) > 0.15
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
        if points.len() >= 2 {
            continue;
        }
        let repaired = match (from.as_deref(), to.as_deref()) {
            (Some(from_id), Some(to_id)) => {
                box_map
                    .get(from_id)
                    .zip(box_map.get(to_id))
                    .map(|(from_box, to_box)| {
                        repaired_connector_points_between_boxes(*from_box, *to_box)
                    })
            }
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
    }
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
        id, bbox, style, ..
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
        return !boxes_overlap(expand_box(component_union, 0.06), *bbox);
    }
    false
}

fn polished_connector_points(points: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let points = orthogonalized_points(points);
    let points = simplify_dogleg_points(&points);
    expand_short_connector_points(&points)
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

fn expand_short_connector_points(points: &[[f64; 2]]) -> Vec<[f64; 2]> {
    if points.len() != 2 || polyline_length(points) >= 0.05 {
        return points.to_vec();
    }
    let start = points[0];
    let end = points[1];
    if same_x(start, end) {
        let offset_x = offset_coordinate(start[0]);
        return vec![start, [offset_x, start[1]], [offset_x, end[1]], end];
    }
    if same_y(start, end) {
        let offset_y = offset_coordinate(start[1]);
        return vec![start, [start[0], offset_y], [end[0], offset_y], end];
    }
    points.to_vec()
}

fn offset_coordinate(value: f64) -> f64 {
    if value <= 0.92 {
        (value + 0.04).min(1.0)
    } else {
        (value - 0.04).max(0.0)
    }
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
    [point[0].clamp(0.0, 1.0), point[1].clamp(0.0, 1.0)]
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
    let runtime_path = absolutize(renderer_root).join("src/runtime.ts");
    let out_dir = absolutize(round_dir);
    let payload = DrawRenderPayload {
        out_dir,
        draw_plan,
        style,
        asset_paths,
    };
    let payload_json = serde_json::to_string_pretty(&payload)?;
    Ok(format!(
        r#"import {{ createDrawPlanRuntime }} from "{}";

const payload = {};
async function main() {{
  const runtime = createDrawPlanRuntime(payload);
  await runtime.renderDrawPlan();
}}

main().catch((error) => {{
  console.error(error);
  process.exit(1);
}});
"#,
        escape_ts_path(&runtime_path),
        payload_json
    ))
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
