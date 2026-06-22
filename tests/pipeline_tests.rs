use std::fs;
use std::time::Duration;

use methodfig::pipeline::{
    renderer_uses_generated_code_bundle, resume_pipeline, revision_source_round_index,
    run_pipeline, select_renderer_code, RunOptions,
};
use methodfig::pipeline::{should_replace_best_review, should_replace_best_round_quality};
use methodfig::schema::{
    CanvasAspect, ImageProviderKind, ReferencePreviewMode, Review, ReviewScores, StyleName,
};
use methodfig::tools::review::{QualityIssue, QualityReport};

#[test]
fn mock_pipeline_fails_once_patches_then_writes_final_artifacts() {
    let temp = tempfile::tempdir().expect("tempdir");
    let method_path = temp.path().join("method.md");
    let out_dir = temp.path().join("run");
    fs::write(
        &method_path,
        "# Method\n\nA teacher model supervises a compact student model with latent residuals.",
    )
    .expect("write method");

    let result = run_pipeline(RunOptions {
        method_path,
        out_dir: out_dir.clone(),
        style: StyleName::WpsClean,
        aspect: CanvasAspect::PaperWide,
        target_width_mm: 85,
        max_iterations: 3,
        max_cost_usd: 3.0,
        max_minutes: 20,
        image_provider: ImageProviderKind::None,
        reference_previews: ReferencePreviewMode::Auto,
        mock_models: true,
        keep_intermediate: true,
        renderer_timeout: Duration::from_secs(20),
    })
    .expect("mock pipeline should complete");

    assert!(result.accepted);
    assert_eq!(result.rounds, 2);
    assert!(out_dir.join("round_000/review.json").exists());
    assert!(out_dir
        .join("round_001/workspace/readable/previous_code/figure.ts")
        .exists());
    assert!(out_dir
        .join("round_001/workspace/writable/code/figure.ts")
        .exists());
    assert!(!out_dir.join("round_001/patch_plan.json").exists());
    assert!(out_dir.join("final/figure.pptx").exists());
    assert!(out_dir.join("final/figure.ts").exists());
    assert!(out_dir.join("final/figure.pdf").exists());
    assert!(out_dir.join("final/figure.png").exists());
    assert!(out_dir.join("final/figure_plan.json").exists());
    assert!(out_dir.join("final/review.json").exists());
    assert!(out_dir.join("final/regression_report.json").exists());
}

#[test]
fn nonmock_renderer_uses_generated_code_with_deterministic_fallback() {
    let generated_code = "model authored TypeScript with its own geometry";
    let deterministic_code = "deterministic DrawPlan runtime payload";

    let selection = select_renderer_code(false, generated_code, deterministic_code);

    assert_eq!(selection.primary_code, generated_code);
    assert_eq!(selection.fallback_code, Some(deterministic_code));
    assert_eq!(selection.source_on_success, "model_generated_code");
}

#[test]
fn nonmock_draw_plan_contract_requests_generated_code_bundle() {
    assert!(
        renderer_uses_generated_code_bundle(false),
        "non-mock runs must give the coder model a chance to revise renderer code from review context"
    );
    assert!(
        renderer_uses_generated_code_bundle(true),
        "mock runs keep generated-code artifacts to exercise feedback workspace behavior"
    );
}

#[test]
fn mock_renderer_keeps_generated_code_with_deterministic_fallback() {
    let generated_code = "mock generated TypeScript";
    let deterministic_code = "deterministic DrawPlan runtime payload";

    let selection = select_renderer_code(true, generated_code, deterministic_code);

    assert_eq!(selection.primary_code, generated_code);
    assert_eq!(selection.fallback_code, Some(deterministic_code));
    assert_eq!(selection.source_on_success, "mock_generated_code");
}

#[test]
fn best_review_selection_keeps_blocker_free_round_over_later_regression() {
    let current = review_with_scores(false, 5, 4, vec![]);
    let later_regression = review_with_scores(
        false,
        8,
        8,
        vec![
            "edge crossing".to_string(),
            "text on line".to_string(),
            "missing semantic edge".to_string(),
        ],
    );

    assert!(
        !should_replace_best_review(Some(&current), &later_regression),
        "later high-score regressions with blockers should not replace a blocker-free round"
    );
}

#[test]
fn best_review_selection_replaces_with_stronger_blocker_free_round() {
    let current = review_with_scores(false, 5, 4, vec![]);
    let better = review_with_scores(false, 7, 6, vec![]);

    assert!(should_replace_best_review(Some(&current), &better));
}

#[test]
fn best_round_quality_keeps_higher_quality_round_over_later_review_score() {
    let current_review = review_with_scores(false, 5, 5, vec!["route detour".to_string()]);
    let later_review = review_with_scores(false, 9, 9, vec!["short edge".to_string()]);
    let current_quality = quality_report_with_issues(
        40,
        vec![
            ("excessive_internal_whitespace", "major"),
            ("excessive_internal_whitespace", "major"),
            ("text_wrap_risk", "major"),
            ("route_detour", "major"),
        ],
    );
    let later_quality = quality_report_with_issues(
        0,
        vec![
            ("excessive_internal_whitespace", "major"),
            ("excessive_internal_whitespace", "major"),
            ("text_wrap_risk", "major"),
            ("route_detour", "major"),
            ("degenerate_edge", "blocking"),
            ("degenerate_edge", "blocking"),
        ],
    );

    assert!(
        !should_replace_best_round_quality(
            Some((&current_review, &current_quality)),
            (&later_review, &later_quality)
        ),
        "quality_report must protect a better local render from a later higher-review regression"
    );
}

#[test]
fn best_round_quality_replaces_when_quality_score_improves() {
    let current_review = review_with_scores(false, 7, 7, vec!["crowding".to_string()]);
    let later_review = review_with_scores(false, 6, 6, vec!["minor detour".to_string()]);
    let current_quality = quality_report_with_issues(
        15,
        vec![
            ("component_crowding", "major"),
            ("degenerate_edge", "blocking"),
        ],
    );
    let later_quality = quality_report_with_issues(70, vec![("route_detour", "major")]);

    assert!(
        should_replace_best_round_quality(
            Some((&current_review, &current_quality)),
            (&later_review, &later_quality)
        ),
        "local quality improvement should replace best even if review score is slightly lower"
    );
}

#[test]
fn best_round_quality_rejects_score_gain_that_adds_blockers() {
    let current_review = review_with_scores(false, 5, 5, vec!["one blocker".to_string()]);
    let later_review = review_with_scores(false, 8, 8, vec!["two blockers".to_string()]);
    let current_quality = quality_report_with_issues(
        64,
        vec![
            ("component_overlap", "blocking"),
            ("component_crowding", "major"),
            ("component_crowding", "major"),
            ("component_crowding", "major"),
            ("excessive_internal_whitespace", "major"),
        ],
    );
    let later_quality = quality_report_with_issues(
        70,
        vec![
            ("label_overlaps_component", "blocking"),
            ("edge_crossing", "blocking"),
            ("component_crowding", "major"),
        ],
    );

    assert!(
        !should_replace_best_round_quality(
            Some((&current_review, &current_quality)),
            (&later_review, &later_quality)
        ),
        "a higher diagnostic score must not replace best-so-far when it increases blocking issues"
    );
}

#[test]
fn revision_source_prefers_best_round_over_last_round() {
    assert_eq!(revision_source_round_index(0, None), None);
    assert_eq!(revision_source_round_index(1, None), Some(0));
    assert_eq!(revision_source_round_index(3, None), Some(2));
    assert_eq!(revision_source_round_index(3, Some(0)), Some(0));
    assert_eq!(revision_source_round_index(3, Some(2)), Some(2));
}

#[test]
fn resume_pipeline_uses_existing_run_directory() {
    let temp = tempfile::tempdir().expect("tempdir");
    let method_path = temp.path().join("method.md");
    let out_dir = temp.path().join("run");
    fs::write(
        &method_path,
        "# Method\n\nTwo encoders fuse image and text features before classification.",
    )
    .expect("write method");

    run_pipeline(RunOptions {
        method_path,
        out_dir: out_dir.clone(),
        style: StyleName::WpsClean,
        aspect: CanvasAspect::PaperWide,
        target_width_mm: 85,
        max_iterations: 2,
        max_cost_usd: 3.0,
        max_minutes: 20,
        image_provider: ImageProviderKind::None,
        reference_previews: ReferencePreviewMode::Auto,
        mock_models: true,
        keep_intermediate: true,
        renderer_timeout: Duration::from_secs(20),
    })
    .expect("initial run");

    let resumed = resume_pipeline(out_dir.clone()).expect("resume should inspect existing run");
    assert!(resumed.accepted);
    assert_eq!(resumed.rounds, 2);
    assert_eq!(resumed.run_dir, out_dir);
    assert!(!resumed.run_dir.join("round_002").exists());
}

#[test]
fn resume_pipeline_continues_rejected_run_directory() {
    let temp = tempfile::tempdir().expect("tempdir");
    let method_path = temp.path().join("method.md");
    let out_dir = temp.path().join("run");
    fs::write(
        &method_path,
        "# Method\n\nA teacher supervises a student, but one iteration is not enough.",
    )
    .expect("write method");

    let initial = run_pipeline(RunOptions {
        method_path,
        out_dir: out_dir.clone(),
        style: StyleName::WpsClean,
        aspect: CanvasAspect::PaperWide,
        target_width_mm: 85,
        max_iterations: 1,
        max_cost_usd: 3.0,
        max_minutes: 20,
        image_provider: ImageProviderKind::None,
        reference_previews: ReferencePreviewMode::Auto,
        mock_models: true,
        keep_intermediate: true,
        renderer_timeout: Duration::from_secs(20),
    })
    .expect("initial capped run");

    assert!(!initial.accepted);
    assert!(out_dir.join("round_000/review.json").exists());
    assert!(!out_dir.join("round_001").exists());
    fs::create_dir_all(out_dir.join("round_001")).expect("create partial round");
    fs::write(out_dir.join("round_001/figure_plan.json"), "{}").expect("write partial artifact");
    let round_000_before = fs::read(out_dir.join("round_000/figure.ts")).unwrap();

    let resumed = resume_pipeline(out_dir.clone()).expect("resume should append another round");

    assert!(resumed.accepted);
    assert_eq!(resumed.rounds, 2);
    assert_eq!(resumed.run_dir, out_dir);
    assert!(resumed.run_dir.join("round_001/review.json").exists());
    assert!(
        !resumed.run_dir.join("round_002").exists(),
        "resume should reuse incomplete round directories instead of skipping them"
    );
    assert_eq!(
        fs::read(resumed.run_dir.join("round_000/figure.ts")).unwrap(),
        round_000_before,
        "resume must append rounds instead of rewriting round_000"
    );
    let status: serde_json::Value =
        serde_json::from_slice(&fs::read(resumed.run_dir.join("final/status.json")).unwrap())
            .unwrap();
    assert_eq!(status["accepted"], true);
}

#[test]
fn resume_pipeline_without_final_respects_existing_iteration_cap() {
    let temp = tempfile::tempdir().expect("tempdir");
    let method_path = temp.path().join("method.md");
    let out_dir = temp.path().join("run");
    fs::write(
        &method_path,
        "# Method\n\nA teacher supervises a student, but one iteration is not enough.",
    )
    .expect("write method");

    let initial = run_pipeline(RunOptions {
        method_path,
        out_dir: out_dir.clone(),
        style: StyleName::WpsClean,
        aspect: CanvasAspect::PaperWide,
        target_width_mm: 85,
        max_iterations: 1,
        max_cost_usd: 3.0,
        max_minutes: 20,
        image_provider: ImageProviderKind::None,
        reference_previews: ReferencePreviewMode::Auto,
        mock_models: true,
        keep_intermediate: true,
        renderer_timeout: Duration::from_secs(20),
    })
    .expect("initial capped run");

    assert!(!initial.accepted);
    assert!(out_dir.join("round_000/review.json").exists());
    fs::remove_dir_all(out_dir.join("final")).expect("simulate interrupted run without final");

    let resumed = resume_pipeline(out_dir.clone()).expect("resume should finalize capped run");

    assert!(!resumed.accepted);
    assert_eq!(resumed.rounds, 1);
    assert!(!resumed.run_dir.join("round_001").exists());
    let status: serde_json::Value =
        serde_json::from_slice(&fs::read(resumed.run_dir.join("final/status.json")).unwrap())
            .unwrap();
    assert_eq!(status["accepted"], false);
}

fn review_with_scores(
    passed: bool,
    semantic_fidelity: u8,
    story_clarity: u8,
    blocking_issues: Vec<String>,
) -> Review {
    Review {
        passed,
        scores: ReviewScores {
            semantic_fidelity,
            story_clarity,
            visual_hierarchy: 5,
            paper_readability: 5,
            layout_cleanliness: 5,
            arrow_routing: 5,
            color_semantics: 5,
            aesthetic_quality: 5,
            wps_editability: 5,
        },
        blocking_issues,
        localized_issues: vec![],
        accepted_assets: vec![],
        rejected_assets: vec![],
    }
}

fn quality_report_with_issues(score: u32, issue_specs: Vec<(&str, &str)>) -> QualityReport {
    QualityReport {
        version: "0.1".to_string(),
        passed: issue_specs
            .iter()
            .all(|(_, severity)| *severity != "blocking" && *severity != "major"),
        score,
        issues: issue_specs
            .into_iter()
            .enumerate()
            .map(|(index, (issue_type, severity))| QualityIssue {
                issue_id: format!("quality_{index:03}"),
                issue_type: issue_type.to_string(),
                severity: severity.to_string(),
                target_ids: vec!["target".to_string()],
                evidence: format!("{issue_type} evidence"),
                suggested_action: "fix target geometry".to_string(),
            })
            .collect(),
    }
}

#[test]
fn mock_pipeline_writes_rejected_final_when_iteration_cap_reached() {
    let temp = tempfile::tempdir().expect("tempdir");
    let method_path = temp.path().join("method.md");
    let out_dir = temp.path().join("run");
    fs::write(
        &method_path,
        "# Method\n\nA teacher supervises a student, but one iteration is not enough.",
    )
    .expect("write method");

    let result = run_pipeline(RunOptions {
        method_path,
        out_dir: out_dir.clone(),
        style: StyleName::WpsClean,
        aspect: CanvasAspect::PaperWide,
        target_width_mm: 85,
        max_iterations: 1,
        max_cost_usd: 3.0,
        max_minutes: 20,
        image_provider: ImageProviderKind::None,
        reference_previews: ReferencePreviewMode::Auto,
        mock_models: true,
        keep_intermediate: true,
        renderer_timeout: Duration::from_secs(20),
    })
    .expect("mock pipeline should produce best available final");

    assert!(!result.accepted);
    assert!(out_dir.join("final/figure.pptx").exists());
    let status: serde_json::Value =
        serde_json::from_slice(&fs::read(out_dir.join("final/status.json")).unwrap()).unwrap();
    assert_eq!(status["accepted"], false);
}

#[test]
fn mock_pipeline_accepts_zero_max_iterations_as_until_passed() {
    let temp = tempfile::tempdir().expect("tempdir");
    let method_path = temp.path().join("method.md");
    let out_dir = temp.path().join("run");
    fs::write(
        &method_path,
        "# Method\n\nA teacher model supervises a compact student with latent residuals.",
    )
    .expect("write method");

    let result = run_pipeline(RunOptions {
        method_path,
        out_dir: out_dir.clone(),
        style: StyleName::WpsClean,
        aspect: CanvasAspect::PaperWide,
        target_width_mm: 85,
        max_iterations: 0,
        max_cost_usd: 3.0,
        max_minutes: 20,
        image_provider: ImageProviderKind::None,
        reference_previews: ReferencePreviewMode::Auto,
        mock_models: true,
        keep_intermediate: true,
        renderer_timeout: Duration::from_secs(20),
    })
    .expect("zero max_iterations should mean iterate until accepted");

    assert!(result.accepted);
    assert_eq!(result.rounds, 2);
    assert!(out_dir.join("round_001/review.json").exists());
}
