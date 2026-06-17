use methodfig::agent::apply_patch_plan_to_figure;
use methodfig::schema::{CanvasAspect, StyleName, VisualWeight};
use methodfig::tools::review::{mock_patch_plan, mock_review, review_passes_threshold};

#[test]
fn acceptance_thresholds_reject_first_mock_review_and_accept_second() {
    let first = mock_review(0);
    assert!(!review_passes_threshold(&first));
    assert!(!first.blocking_issues.is_empty());

    let second = mock_review(1);
    assert!(review_passes_threshold(&second));
    assert!(second.blocking_issues.is_empty());
}

#[test]
fn patch_routing_keeps_reasoner_layout_patch_for_main_module() {
    let patch = mock_patch_plan();
    assert_eq!(
        patch.operations[0].executor,
        methodfig::schema::PatchExecutor::Reasoner
    );
    assert_eq!(
        patch.operations[0].operation_type,
        methodfig::schema::PatchOperationType::LayoutPatch
    );

    let mut plan = methodfig::schema::FigurePlan::mock_from_method(
        "Teacher guides student with latent residuals.",
        StyleName::WpsClean,
        CanvasAspect::PaperWide,
        85,
    );
    apply_patch_plan_to_figure(&mut plan);
    assert!(plan
        .components
        .iter()
        .any(|component| component.visual_weight == VisualWeight::Strong));
    assert!(plan
        .story
        .visual_focus
        .contains(&"main contribution path emphasized".to_string()));
}
