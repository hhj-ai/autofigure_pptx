use methodfig::schema::{ReferencePreviewMode, ReferenceSelection};
use methodfig::tools::template_library::{
    reference_pack_json, select_reference_for_method, selected_reference_json,
};
use std::path::Path;

#[test]
fn reference_pack_contains_classic_and_award_reference_grammar() {
    let pack = reference_pack_json().expect("reference pack should load");
    let json: serde_json::Value =
        serde_json::from_str(&pack).expect("reference pack should be valid JSON");
    let references = json["references"]
        .as_array()
        .expect("references should be an array");

    for id in [
        "vit_patch_transformer",
        "clip_dual_encoder_contrastive",
        "bert_pretrain_finetune",
        "neurips_2025_gated_attention_award",
    ] {
        assert!(
            references.iter().any(|entry| entry["id"] == id),
            "missing reference {id}"
        );
    }

    let clip = references
        .iter()
        .find(|entry| entry["id"] == "clip_dual_encoder_contrastive")
        .expect("CLIP reference should exist");
    assert_eq!(
        clip["source"]["pdf_url"],
        "https://arxiv.org/pdf/2103.00020"
    );
    assert!(clip["layout_pattern"]["slots"]
        .as_array()
        .unwrap()
        .iter()
        .any(|slot| slot["role"] == "paired_encoder"));
    assert!(clip["anti_patterns"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value
            .as_str()
            .unwrap_or_default()
            .contains("unbalanced branches")));

    let gated = references
        .iter()
        .find(|entry| entry["id"] == "neurips_2025_gated_attention_award")
        .expect("NeurIPS award reference should exist");
    assert!(gated["source"]["award_url"]
        .as_str()
        .unwrap_or_default()
        .contains("blog.neurips.cc/2025"));
    assert_eq!(
        gated["preview"]["local_path"],
        "templates/method_overview/reference_figures/assets/neurips_2025_gated_attention_award.png"
    );

    for reference in references {
        let path = reference["preview"]["local_path"]
            .as_str()
            .expect("each reference should declare a preview path");
        assert!(
            Path::new(path).exists(),
            "declared reference preview should exist in git-trackable templates: {path}"
        );
    }
}

#[test]
fn reference_selector_prefers_task_specific_classic_templates() {
    let clip = select_reference_for_method(
        "We align image and text encoders with contrastive natural language supervision.",
        ReferencePreviewMode::Auto,
    )
    .expect("CLIP-like method should select a reference");
    assert_eq!(clip.selected_reference_id, "clip_dual_encoder_contrastive");
    assert!(clip
        .adaptation_rules
        .iter()
        .any(|rule| rule.contains("two towers")));

    let vit = select_reference_for_method(
        "Images are split into patches, embedded as tokens, and processed by a Transformer encoder.",
        ReferencePreviewMode::Auto,
    )
    .expect("ViT-like method should select a reference");
    assert_eq!(vit.selected_reference_id, "vit_patch_transformer");
}

#[test]
fn selected_reference_json_is_small_and_excludes_full_reference_pack() {
    let selection = ReferenceSelection {
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
        why_fit: "method has image text contrastive alignment".to_string(),
        adaptation_rules: vec!["Use two balanced towers".to_string()],
        anti_patterns: vec!["unbalanced branches".to_string()],
        quality_targets: vec!["symmetric encoder columns".to_string()],
    };

    let selected = selected_reference_json(&selection).expect("selection should serialize");

    assert!(selected.contains("clip_dual_encoder_contrastive"));
    assert!(selected.contains("preview_path"));
    assert!(!selected.contains("\"references\""));
    assert!(!selected.contains("full-slide raster"));
}
