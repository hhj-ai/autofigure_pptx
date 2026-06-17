use methodfig::agent::{
    build_initial_plan_prompt, build_patch_prompt, build_review_prompt, build_review_retry_prompt,
};
use methodfig::prompts::{TOP_TIER_FIGURE_DIRECTIVE, VISION_REVIEWER};
use methodfig::schema::{CanvasAspect, StyleName};

#[test]
fn initial_plan_prompt_includes_schema_and_required_top_level_keys() {
    let prompt = build_initial_plan_prompt(
        "Teacher guides student with latent residuals.",
        StyleName::WpsClean,
        CanvasAspect::PaperWide,
        85,
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
}

#[test]
fn review_prompt_includes_schema_and_all_score_fields() {
    let prompt = build_review_prompt("{\"version\":\"0.1\"}", "{\"objects\":[]}")
        .expect("review prompt should build");

    assert!(prompt.contains("\"Review\""));
    assert!(prompt.contains("semantic_fidelity"));
    assert!(prompt.contains("wps_editability"));
    assert!(prompt.contains("Return exactly one Review object"));
    assert!(prompt.contains("do not embed quotation marks"));
    assert!(!prompt.contains(TOP_TIER_FIGURE_DIRECTIVE));
}

#[test]
fn review_retry_prompt_is_strict_about_json_only_output() {
    let prompt = build_review_retry_prompt("{\"version\":\"0.1\"}", "{\"objects\":[]}")
        .expect("retry prompt should build");

    assert!(prompt.contains("not valid JSON"));
    assert!(prompt.contains("strict JSON only"));
    assert!(prompt.contains("avoid embedded quotation marks"));
    assert!(prompt.contains("semantic_fidelity"));
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
