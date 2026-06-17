use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::schema::{validate_stable_ids, AssetType, FigurePlan, StyleName};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StyleSpec {
    pub name: StyleName,
    pub fonts: FontSpec,
    pub palette: PaletteSpec,
    pub line_widths: LineWidths,
    pub corner_radius: CornerRadius,
    pub spacing: Spacing,
    pub font_sizes: FontSizes,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FontSpec {
    pub font_cjk: String,
    pub font_cjk_display_name: String,
    pub font_latin: String,
    pub font_mono: String,
    pub fallback_cjk: Vec<String>,
    pub fallback_latin: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaletteSpec {
    pub background: String,
    pub text: String,
    pub muted_text: String,
    pub stroke: String,
    pub muted_fill: String,
    pub primary: String,
    pub accent: String,
    pub warning: String,
    #[serde(default)]
    pub extra: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LineWidths {
    pub auxiliary: f64,
    pub normal: f64,
    pub main_flow: f64,
    pub strong_focus: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CornerRadius {
    pub module: f64,
    pub group: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Spacing {
    pub safe_margin: f64,
    pub lane_gap: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FontSizes {
    pub module_label: f64,
    pub auxiliary_label: f64,
    pub section_label: f64,
    pub min_label: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ValidationReport {
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

impl ValidationReport {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

pub fn style_by_name(name: StyleName) -> StyleSpec {
    match name {
        StyleName::WpsClean => base_style(name, "2F6F9F", "4B9A72", "C2410C"),
        StyleName::CvprClean => {
            let mut style = base_style(name, "1F5A96", "6C7A89", "B45309");
            style.font_sizes.module_label = 8.5;
            style.spacing.lane_gap = 0.045;
            style
        }
        StyleName::NeuripsMinimal => {
            let mut style = base_style(name, "2B2F36", "737A84", "B42318");
            style.palette.muted_fill = "F7F7F8".to_string();
            style.font_sizes.section_label = 10.0;
            style
        }
    }
}

fn base_style(name: StyleName, primary: &str, accent: &str, warning: &str) -> StyleSpec {
    StyleSpec {
        name,
        fonts: FontSpec {
            font_cjk: "Microsoft YaHei".to_string(),
            font_cjk_display_name: "微软雅黑".to_string(),
            font_latin: "Arial".to_string(),
            font_mono: "Consolas".to_string(),
            fallback_cjk: vec![
                "DengXian".to_string(),
                "SimHei".to_string(),
                "SimSun".to_string(),
            ],
            fallback_latin: vec!["Calibri".to_string(), "Arial".to_string()],
        },
        palette: PaletteSpec {
            background: "FFFFFF".to_string(),
            text: "1F2328".to_string(),
            muted_text: "5B6770".to_string(),
            stroke: "A8B0B8".to_string(),
            muted_fill: "F4F6F8".to_string(),
            primary: primary.to_string(),
            accent: accent.to_string(),
            warning: warning.to_string(),
            extra: BTreeMap::new(),
        },
        line_widths: LineWidths {
            auxiliary: 0.75,
            normal: 1.0,
            main_flow: 1.5,
            strong_focus: 2.0,
        },
        corner_radius: CornerRadius {
            module: 0.12,
            group: 0.08,
        },
        spacing: Spacing {
            safe_margin: 0.06,
            lane_gap: 0.06,
        },
        font_sizes: FontSizes {
            module_label: 9.0,
            auxiliary_label: 7.5,
            section_label: 11.0,
            min_label: 6.5,
        },
    }
}

pub fn validate_style_spec(style: &StyleSpec) -> ValidationReport {
    let mut report = ValidationReport::default();

    let semantic_colors = 3 + style.palette.extra.len();
    if semantic_colors > 4 {
        report.warnings.push(format!(
            "style uses more than 4 semantic colors plus background/text: {semantic_colors}"
        ));
    }

    for (name, width) in [
        ("auxiliary", style.line_widths.auxiliary),
        ("normal", style.line_widths.normal),
        ("main_flow", style.line_widths.main_flow),
        ("strong_focus", style.line_widths.strong_focus),
    ] {
        if width < 0.75 {
            report
                .warnings
                .push(format!("line width {name} is below 0.75 pt: {width}"));
        }
    }

    let supported_fonts = [
        "Microsoft YaHei",
        "Arial",
        "Consolas",
        "DengXian",
        "SimHei",
        "SimSun",
        "Calibri",
    ];
    for font in [
        &style.fonts.font_cjk,
        &style.fonts.font_latin,
        &style.fonts.font_mono,
    ] {
        if !supported_fonts.contains(&font.as_str()) {
            report
                .warnings
                .push(format!("unsupported or uncommon font name: {font}"));
        }
    }

    let sizes = [
        style.font_sizes.module_label,
        style.font_sizes.auxiliary_label,
        style.font_sizes.section_label,
        style.font_sizes.min_label,
    ];
    if sizes.iter().any(|size| *size < 6.5) {
        report
            .warnings
            .push("font size below 6.5 pt may fail paper-width readability".to_string());
    }

    report
}

pub fn validate_figure_style(plan: &FigurePlan, style: &StyleSpec) -> ValidationReport {
    let mut report = validate_style_spec(style);

    if let Err(error) = validate_stable_ids(plan) {
        report.errors.push(error.to_string());
    }

    for asset in &plan.assets {
        if matches!(
            asset.asset_type,
            AssetType::GeneratedIcon | AssetType::GeneratedTexture
        ) && (!asset.style_constraints.no_text
            || !asset.negative_prompt.to_lowercase().contains("text")
            || !asset.negative_prompt.to_lowercase().contains("letters"))
        {
            report.warnings.push(format!(
                "image asset may contain text and violate editability policy: {}",
                asset.id
            ));
        }
    }

    for component in &plan.components {
        let too_many_words = component.label.split_whitespace().count() > 6;
        let too_many_chars = component.label.chars().count() > 24;
        if too_many_words || too_many_chars {
            report.warnings.push(format!(
                "component label is long for paper width: {}",
                component.id
            ));
        }
    }

    report
}
