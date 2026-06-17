use crate::schema::{
    IssueSeverity, LocalizedIssue, PatchExecutor, PatchOperation, PatchOperationType, PatchPlan,
    PatchStopReason, Review, ReviewScores,
};

pub fn review_passes_threshold(review: &Review) -> bool {
    review.blocking_issues.is_empty()
        && review.scores.semantic_fidelity >= 8
        && review.scores.story_clarity >= 8
        && review.scores.visual_hierarchy >= 8
        && review.scores.paper_readability >= 8
        && review.scores.layout_cleanliness >= 8
        && review.scores.arrow_routing >= 8
        && review.scores.aesthetic_quality >= 7
        && review.scores.wps_editability >= 9
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
