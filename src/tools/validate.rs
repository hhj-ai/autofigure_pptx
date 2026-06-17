use anyhow::{anyhow, Result};

use crate::schema::FigurePlan;
use crate::style::{validate_figure_style, StyleSpec};

const MIN_NORMALIZED_BOX_WIDTH: f64 = 0.04;
const MIN_NORMALIZED_BOX_HEIGHT: f64 = 0.06;

pub fn normalize_plan_for_render(plan: &mut FigurePlan) {
    normalize_editable_text(plan);

    if !plan.canvas.safe_margin.is_finite()
        || plan.canvas.safe_margin < 0.0
        || plan.canvas.safe_margin > 0.2
    {
        plan.canvas.safe_margin = 0.06;
    }

    let columns = plan.layout.grid.columns.max(1) as f64;
    let rows = plan.layout.grid.rows.max(1) as f64;
    let regions_use_grid_units = plan
        .layout
        .regions
        .iter()
        .any(|region| bbox_looks_like_grid_box(region.bbox));

    if regions_use_grid_units {
        for region in &mut plan.layout.regions {
            region.bbox = grid_box_to_normalized(region.bbox, columns, rows);
        }
    } else {
        for region in &mut plan.layout.regions {
            region.bbox = clamp_normalized_box(region.bbox);
        }
    }

    for annotation in &mut plan.annotations {
        if let Some(bbox) = annotation.bbox {
            annotation.bbox = Some(if bbox_looks_like_grid_box(bbox) {
                grid_box_to_normalized(bbox, columns, rows)
            } else {
                clamp_normalized_box(bbox)
            });
        }
    }
}

fn normalize_editable_text(plan: &mut FigurePlan) {
    for component in &mut plan.components {
        normalize_label(&mut component.label);
    }
    for edge in &mut plan.edges {
        normalize_label(&mut edge.label);
    }
    for annotation in &mut plan.annotations {
        normalize_label(&mut annotation.label);
    }
}

fn normalize_label(label: &mut String) {
    if label.contains("\\n") {
        *label = label.replace("\\n", "\n");
    }
}

pub fn validate_plan_for_render(plan: &FigurePlan, style: &StyleSpec) -> Result<()> {
    let report = validate_figure_style(plan, style);
    if report.errors.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(
            "FigurePlan validation failed: {}",
            report.errors.join("; ")
        ))
    }
}

fn bbox_looks_like_grid_box(bbox: [f64; 4]) -> bool {
    bbox.iter()
        .any(|value| !value.is_finite() || *value > 1.2 || *value < -0.2)
        || bbox[2] < bbox[0]
        || bbox[3] < bbox[1]
}

fn grid_box_to_normalized(bbox: [f64; 4], columns: f64, rows: f64) -> [f64; 4] {
    let x = finite_or_zero(bbox[0]);
    let y = finite_or_zero(bbox[1]);
    let width = finite_or_zero(bbox[2]).max(0.5);
    let height = finite_or_zero(bbox[3]).max(0.5);
    clamp_normalized_box([
        x / columns,
        y / rows,
        (x + width) / columns,
        (y + height) / rows,
    ])
}

fn clamp_normalized_box(bbox: [f64; 4]) -> [f64; 4] {
    let x1 = finite_or_zero(bbox[0]).clamp(0.0, 1.0);
    let y1 = finite_or_zero(bbox[1]).clamp(0.0, 1.0);
    let x2 = finite_or_zero(bbox[2]).clamp(0.0, 1.0);
    let y2 = finite_or_zero(bbox[3]).clamp(0.0, 1.0);
    let (x1, x2) = expand_axis_to_min_size(x1.min(x2), x1.max(x2), MIN_NORMALIZED_BOX_WIDTH);
    let (y1, y2) = expand_axis_to_min_size(y1.min(y2), y1.max(y2), MIN_NORMALIZED_BOX_HEIGHT);
    [x1, y1, x2, y2]
}

fn expand_axis_to_min_size(start: f64, end: f64, min_size: f64) -> (f64, f64) {
    if end - start >= min_size {
        return (start, end);
    }
    let center = (start + end) / 2.0;
    let mut expanded_start = center - min_size / 2.0;
    let mut expanded_end = center + min_size / 2.0;
    if expanded_start < 0.0 {
        expanded_end = (expanded_end - expanded_start).min(1.0);
        expanded_start = 0.0;
    }
    if expanded_end > 1.0 {
        expanded_start = (expanded_start - (expanded_end - 1.0)).max(0.0);
        expanded_end = 1.0;
    }
    (expanded_start, expanded_end)
}

fn finite_or_zero(value: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}
