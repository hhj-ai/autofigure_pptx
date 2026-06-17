use methodfig::schema::{AssetStatus, AssetType, FigurePlan, StyleName};
use methodfig::style::{style_by_name, validate_figure_style, validate_style_spec};

#[test]
fn wps_clean_uses_goal_fonts_and_safe_line_widths() {
    let style = style_by_name(StyleName::WpsClean);
    assert_eq!(style.fonts.font_cjk, "Microsoft YaHei");
    assert_eq!(style.fonts.font_latin, "Arial");
    assert!(style.fonts.fallback_cjk.contains(&"DengXian".to_string()));
    assert!(style.line_widths.auxiliary >= 0.75);
    assert!(style.line_widths.normal >= 0.75);
}

#[test]
fn style_validator_warns_about_too_thin_lines_and_many_colors() {
    let mut style = style_by_name(StyleName::WpsClean);
    style.line_widths.normal = 0.5;
    style
        .palette
        .extra
        .insert("extra_1".into(), "123456".into());
    style
        .palette
        .extra
        .insert("extra_2".into(), "654321".into());
    style
        .palette
        .extra
        .insert("extra_3".into(), "ABCDEF".into());

    let report = validate_style_spec(&style);
    assert!(report
        .warnings
        .iter()
        .any(|warning| warning.contains("0.75 pt")));
    assert!(report
        .warnings
        .iter()
        .any(|warning| warning.contains("more than 4 semantic colors")));
}

#[test]
fn figure_style_validation_warns_when_asset_prompt_allows_text() {
    let mut plan = FigurePlan::mock_from_method(
        "Vision and language encoders fuse features.",
        StyleName::WpsClean,
        methodfig::schema::CanvasAspect::PaperWide,
        85,
    );
    plan.assets[0].asset_type = AssetType::GeneratedIcon;
    plan.assets[0].status = AssetStatus::Missing;
    plan.assets[0].style_constraints.no_text = false;
    plan.assets[0].negative_prompt = "watermark".into();

    let report = validate_figure_style(&plan, &style_by_name(StyleName::WpsClean));
    assert!(report
        .warnings
        .iter()
        .any(|warning| warning.contains("image asset may contain text")));
}
