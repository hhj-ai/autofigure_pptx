use anyhow::{anyhow, Result};

use crate::schema::FigurePlan;
use crate::style::{validate_figure_style, StyleSpec};

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
