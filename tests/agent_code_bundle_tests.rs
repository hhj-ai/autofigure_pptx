use anyhow::anyhow;
use methodfig::agent::code_bundle_or_draw_plan_runtime;

#[test]
fn coder_error_falls_back_to_deterministic_draw_plan_runtime_bundle() {
    let deterministic = "import { createDrawPlanRuntime } from './runtime';".to_string();

    let bundle =
        code_bundle_or_draw_plan_runtime(deterministic.clone(), Err(anyhow!("HTTP 500")), "coder")
            .expect("coder failure should degrade to deterministic DrawPlan runtime");

    assert_eq!(bundle.files[0].content, deterministic);
    assert!(bundle.notes.contains("coder failed"));
    assert!(bundle.notes.contains("deterministic DrawPlan runtime"));
}

#[test]
fn unusable_coder_bundle_falls_back_to_deterministic_draw_plan_runtime_bundle() {
    let deterministic = "import { createDrawPlanRuntime } from './runtime';".to_string();

    let bundle = code_bundle_or_draw_plan_runtime(
        deterministic.clone(),
        Ok("{\"files\":[]}".to_string()),
        "coder",
    )
    .expect("invalid coder bundle should degrade to deterministic DrawPlan runtime");

    assert_eq!(bundle.files[0].content, deterministic);
    assert!(bundle.notes.contains("coder returned unusable output"));
}

#[test]
fn raw_coder_typescript_falls_back_to_deterministic_draw_plan_runtime_bundle() {
    let deterministic = "import { createDrawPlanRuntime } from './runtime';".to_string();

    let bundle = code_bundle_or_draw_plan_runtime(
        deterministic.clone(),
        Ok("const payload = {".to_string()),
        "coder",
    )
    .expect("raw TypeScript should not be trusted as the renderer contract");

    assert_eq!(bundle.files[0].content, deterministic);
    assert!(bundle.notes.contains("coder returned unusable output"));
}
