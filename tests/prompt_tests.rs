use methodfig::agent::{
    build_draw_plan_revision_prompt, build_initial_plan_prompt, build_patch_prompt,
    build_review_prompt, build_review_retry_prompt,
};
use methodfig::prompts::{TOP_TIER_FIGURE_DIRECTIVE, VISION_REVIEWER};
use methodfig::schema::{CanvasAspect, ReferencePreviewMode, ReferenceSelection, StyleName};

#[test]
fn initial_plan_prompt_includes_schema_and_required_top_level_keys() {
    let prompt = build_initial_plan_prompt(
        "Teacher guides student with latent residuals.",
        StyleName::WpsClean,
        CanvasAspect::PaperWide,
        85,
        &teacher_student_reference(),
    )
    .expect("prompt should build");

    assert!(prompt.contains("\"FigurePlan\""));
    assert!(prompt.contains("\"canvas\""));
    assert!(prompt.contains("\"components\""));
    assert!(prompt.contains("Required top-level keys"));
    assert!(prompt.contains("version"));
    assert!(prompt.contains("globally unique"));
    assert!(prompt.contains("stable id"));
    assert!(prompt.contains("high space utilization"));
    assert!(prompt.contains("Never let text cover lines or arrows"));
    assert!(prompt.contains(TOP_TIER_FIGURE_DIRECTIVE));
    assert!(prompt.contains("Selected visual reference"));
    assert!(prompt.contains("ReferenceSelection"));
    assert!(prompt.contains("simclr_contrastive_y_branch"));
    assert!(prompt.contains("read-only preview"));
    assert!(prompt.contains("Do not copy source artwork"));
    assert!(!prompt.contains("\"references\""));
    assert!(!prompt.contains("unet_skip_encoder_decoder"));
}

#[test]
fn initial_plan_prompt_uses_only_selected_reference_context() {
    let prompt = build_initial_plan_prompt(
        "Image and text encoders are aligned with contrastive supervision.",
        StyleName::WpsClean,
        CanvasAspect::PaperWide,
        85,
        &clip_reference(),
    )
    .expect("prompt should build");

    assert!(prompt.contains("clip_dual_encoder_contrastive"));
    assert!(prompt.contains("two balanced towers"));
    assert!(prompt.contains("unbalanced branches"));
    assert!(prompt.contains("quality_targets"));
    assert!(!prompt.contains("method_templates.json"));
    assert!(!prompt.contains("\"references\""));
    assert!(!prompt.contains("bert_pretrain_finetune"));
}

#[test]
fn legacy_method_template_prompt_expectations_are_now_in_selected_reference() {
    let prompt = build_initial_plan_prompt(
        "Teacher guides student with latent residuals.",
        StyleName::WpsClean,
        CanvasAspect::PaperWide,
        85,
        &teacher_student_reference(),
    )
    .expect("prompt should build");

    assert!(prompt.contains("simclr_contrastive_y_branch"));
    assert!(prompt.contains("teacher_student_distillation"));
    assert!(prompt.contains("teacher and student as two correlated branches"));
    assert!(prompt.contains("avoid list as hard anti-patterns"));
    assert!(prompt.contains("bottom-heavy separate inference lane"));
    assert!(prompt.contains("standalone inference note component"));
    assert!(prompt.contains("connector-overlapping labels"));
}

#[test]
fn review_prompt_includes_schema_and_all_score_fields() {
    let prompt = build_review_prompt(
        "{\"version\":\"0.1\"}",
        "{\"version\":\"0.2\",\"objects\":[]}",
        "{\"objects\":[]}",
        &teacher_student_reference(),
    )
    .expect("review prompt should build");

    assert!(prompt.contains("\"Review\""));
    assert!(prompt.contains("semantic_fidelity"));
    assert!(prompt.contains("wps_editability"));
    assert!(prompt.contains("Return exactly one Review object"));
    assert!(prompt.contains("Every rejection must include localized_issues"));
    assert!(prompt.contains("actionable"));
    assert!(prompt.contains("simclr_contrastive_y_branch"));
    assert!(prompt.contains("DrawPlan is the rendered source of truth"));
    assert!(prompt.contains("Do not report FigurePlan annotations as missing"));
    assert!(prompt.contains("Selected visual reference"));
    assert!(prompt.contains("quality target"));
    assert!(prompt.contains("native boxes, text, and connectors"));
    assert!(prompt.contains("wps_editability 9 or 10"));
    assert!(prompt.contains("do not embed quotation marks"));
    assert!(!prompt.contains(TOP_TIER_FIGURE_DIRECTIVE));
}

#[test]
fn review_retry_prompt_is_strict_about_json_only_output() {
    let prompt = build_review_retry_prompt(
        "{\"version\":\"0.1\"}",
        "{\"version\":\"0.2\",\"objects\":[]}",
        "{\"objects\":[]}",
        &teacher_student_reference(),
    )
    .expect("retry prompt should build");

    assert!(prompt.contains("not valid JSON"));
    assert!(prompt.contains("strict JSON only"));
    assert!(prompt.contains("avoid embedded quotation marks"));
    assert!(prompt.contains("semantic_fidelity"));
    assert!(prompt.contains("DrawPlan is the rendered source of truth"));
}

#[test]
fn patch_prompt_includes_schema_and_required_fields() {
    let prompt = build_patch_prompt("{\"version\":\"0.1\"}", "{\"passed\":false}")
        .expect("patch prompt should build");

    assert!(prompt.contains("\"PatchPlan\""));
    assert!(prompt.contains("operations"));
    assert!(prompt.contains("stop_reason"));
    assert!(prompt.contains("Return exactly one PatchPlan object"));
    assert!(prompt.contains("final bbox"));
    assert!(prompt.contains("Operations without executable coordinates"));
    assert!(prompt.contains("high space utilization"));
    assert!(prompt.contains(TOP_TIER_FIGURE_DIRECTIVE));
}

#[test]
fn vision_reviewer_mentions_the_new_hard_constraints() {
    assert!(VISION_REVIEWER.contains("Reject any figure that wastes canvas space"));
    assert!(VISION_REVIEWER.contains("text on top of a line"));
    assert!(VISION_REVIEWER.contains("marginal explanatory notes"));
}

#[test]
fn draw_plan_revision_prompt_uses_autofigure_style_visual_optimization_contract() {
    let prompt = build_draw_plan_revision_prompt(
        "{\"version\":\"0.2\",\"objects\":[]}",
        "{\"objects\":[]}",
        "{\"passed\":false}",
        "{\"errors\":[\"label overlaps edge\"]}",
        &teacher_student_reference(),
        "{\"actions\":[{\"target_id\":\"student\",\"expected_visible_effect\":\"move label off edge\"}]}",
    )
    .expect("draw plan revision prompt should build");

    assert!(prompt.contains("\"DrawPlan\""));
    assert!(prompt.contains("Return exactly one DrawPlan object"));
    assert!(prompt.contains("current rendered overlay image"));
    assert!(prompt.contains("POSITION"));
    assert!(prompt.contains("STYLE"));
    assert!(prompt.contains("Text positions"));
    assert!(prompt.contains("Arrows"));
    assert!(prompt.contains("Keep stable ids"));
    assert!(prompt.contains("visual optimizer, not a semantic replanner"));
    assert!(prompt.contains("Do not invent new semantic modules"));
    assert!(prompt.contains("Do not expand an inference note"));
    assert!(prompt.contains("Do not add an output-to-student task-loss feedback edge"));
    assert!(prompt.contains("prefer a direct dashed residual edge"));
    assert!(prompt.contains("Do not return TypeScript"));
    assert!(!prompt.contains("method_templates.json"));
    assert!(prompt.contains("simclr_contrastive_y_branch"));
    assert!(prompt.contains("Selected visual reference"));
    assert!(prompt.contains("RoundImprovementPlan"));
    assert!(prompt.contains("materially change"));
    assert!(prompt.contains("derived from extracted PDF/SVG"));
    assert!(prompt.contains("teacher_student_distillation"));
    assert!(prompt.contains("teacher and student as two correlated branches"));
    assert!(prompt.contains("avoid list as hard anti-patterns"));
    assert!(prompt.contains("remove or redesign it instead of merely moving it"));
    assert!(prompt.contains("bottom-heavy separate inference lane"));
    assert!(prompt.contains("standalone inference note component"));
    assert!(prompt.contains("asymmetric branch annotations"));
    assert!(prompt.contains("connector-overlapping labels"));
}

fn teacher_student_reference() -> ReferenceSelection {
    ReferenceSelection {
        version: "0.1".to_string(),
        selected_reference_id: "simclr_contrastive_y_branch".to_string(),
        selected_reference_name: "Two-view contrastive Y-branch".to_string(),
        source_paper: "A Simple Framework for Contrastive Learning of Visual Representations"
            .to_string(),
        source_url: "https://arxiv.org/pdf/2002.05709".to_string(),
        preview_path: Some(
            "templates/method_overview/reference_figures/assets/simclr_contrastive_y_branch.png"
                .to_string(),
        ),
        preview_mode: ReferencePreviewMode::Auto,
        why_fit: "teacher_student_distillation".to_string(),
        adaptation_rules: vec![
            "Treat teacher and student as two correlated branches".to_string(),
            "Use the avoid list as hard anti-patterns".to_string(),
        ],
        anti_patterns: vec![
            "bottom-heavy separate inference lane".to_string(),
            "standalone inference note component".to_string(),
            "asymmetric branch annotations".to_string(),
            "connector-overlapping labels".to_string(),
        ],
        quality_targets: vec!["balanced two-branch layout".to_string()],
    }
}

fn clip_reference() -> ReferenceSelection {
    ReferenceSelection {
        version: "0.1".to_string(),
        selected_reference_id: "clip_dual_encoder_contrastive".to_string(),
        selected_reference_name: "CLIP dual encoder contrastive alignment".to_string(),
        source_paper: "Learning Transferable Visual Models From Natural Language Supervision"
            .to_string(),
        source_url: "https://arxiv.org/pdf/2103.00020".to_string(),
        preview_path: Some(
            "templates/method_overview/reference_figures/assets/clip_dual_encoder_contrastive.png"
                .to_string(),
        ),
        preview_mode: ReferencePreviewMode::Auto,
        why_fit: "image text contrastive alignment".to_string(),
        adaptation_rules: vec!["Use two balanced towers".to_string()],
        anti_patterns: vec!["unbalanced branches".to_string()],
        quality_targets: vec!["symmetric encoder columns".to_string()],
    }
}
