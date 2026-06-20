use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::Result;
use serde::Deserialize;

use crate::schema::{
    FigurePlan, IssueSeverity, LocalizedIssue, PatchExecutor, PatchOperation, PatchOperationType,
    PatchPlan, PatchStopReason, Review, ReviewScores,
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

pub fn render_quality_issues(layout_map_path: &Path) -> Result<Vec<String>> {
    let layout_map: LayoutMap = serde_json::from_slice(&fs::read(layout_map_path)?)?;
    Ok(render_quality_issues_from_map(&layout_map))
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

fn render_quality_issues_from_map(layout_map: &LayoutMap) -> Vec<String> {
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

    for component in &components {
        let (width, height) = box_size(component.bbox);
        if width < 0.035 || height < 0.08 {
            issues.push(format!(
                "render quality failed: component {} is too small or collapsed",
                component.id
            ));
        }
    }

    for (index, left) in components.iter().enumerate() {
        for right in components.iter().skip(index + 1) {
            let overlap = intersection_area(left.bbox, right.bbox);
            if overlap <= 0.0 {
                continue;
            }
            let left_area = area(left.bbox);
            let right_area = area(right.bbox);
            let ratio = overlap / left_area.min(right_area).max(0.0001);
            if overlap > 0.003 && ratio > 0.15 {
                issues.push(format!(
                    "render quality failed: component overlap between {} and {}",
                    left.id, right.id
                ));
            }
        }
    }

    if let Some(union) = union_bbox(components.iter().map(|object| object.bbox)) {
        let utilization = area(union);
        if utilization < 0.28 {
            issues.push(format!(
                "render quality failed: canvas is under-utilized by main content (occupied bbox area {:.3})",
                utilization
            ));
        }
    }

    let component_union = union_bbox(components.iter().map(|object| object.bbox));
    for label in &labels {
        let label_area = area(label.bbox);
        if label_area < 0.0005 {
            continue;
        }
        let target_edge_id = label.id.strip_suffix("_label");
        if target_edge_id.is_none() {
            if let Some(component_union) = component_union {
                let expanded = expand_box(component_union, 0.08);
                if !boxes_overlap(expanded, label.bbox) {
                    issues.push(format!(
                        "render quality failed: label {} sits outside the main figure area",
                        label.id
                    ));
                }
            }
        }
        for edge in &edges {
            if let Some(target_edge_id) = target_edge_id {
                if edge.id != target_edge_id {
                    continue;
                }
            }
            if label_overlaps_edge(label.bbox, edge) {
                issues.push(format!(
                    "render quality failed: label {} overlaps edge {}",
                    label.id, edge.id
                ));
                break;
            }
        }
    }

    for edge in &edges {
        if edge_length(edge) < 0.04 {
            issues.push(format!(
                "render quality failed: degenerate edge {} is too short",
                edge.id
            ));
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
                issues.push(format!(
                    "render quality failed: edge crossing between {} and {}",
                    left.id, right.id
                ));
            }
        }
    }

    issues
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
    objects: Vec<LayoutObject>,
}

#[derive(Debug, Deserialize)]
struct LayoutObject {
    id: String,
    kind: String,
    bbox: [f64; 4],
    points: Option<Vec<[f64; 2]>>,
}

fn box_size(bbox: [f64; 4]) -> (f64, f64) {
    ((bbox[2] - bbox[0]).max(0.0), (bbox[3] - bbox[1]).max(0.0))
}

fn area(bbox: [f64; 4]) -> f64 {
    let (width, height) = box_size(bbox);
    width * height
}

fn intersection_area(a: [f64; 4], b: [f64; 4]) -> f64 {
    let x1 = a[0].max(b[0]);
    let y1 = a[1].max(b[1]);
    let x2 = a[2].min(b[2]);
    let y2 = a[3].min(b[3]);
    ((x2 - x1).max(0.0)) * ((y2 - y1).max(0.0))
}

fn edge_length(edge: &LayoutObject) -> f64 {
    edge_segments(edge)
        .iter()
        .map(|segment| segment_length(*segment))
        .sum()
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
        let segment_bbox = expand_box(segment_bbox(*start, *end), 0.006);
        let overlap = intersection_area(label_bbox, segment_bbox);
        overlap > 0.001 && overlap / label_area > 0.2
    })
}

fn segment_bbox(start: [f64; 2], end: [f64; 2]) -> [f64; 4] {
    [
        start[0].min(end[0]),
        start[1].min(end[1]),
        start[0].max(end[0]),
        start[1].max(end[1]),
    ]
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

fn boxes_overlap(a: [f64; 4], b: [f64; 4]) -> bool {
    intersection_area(a, b) > 0.0
}

fn boxes_close(a: [f64; 4], b: [f64; 4]) -> bool {
    (a[0] - b[0]).abs() < 0.01
        && (a[1] - b[1]).abs() < 0.01
        && (a[2] - b[2]).abs() < 0.01
        && (a[3] - b[3]).abs() < 0.01
}
