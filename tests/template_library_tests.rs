use methodfig::tools::template_library::method_template_pack_json;

#[test]
fn method_template_pack_contains_pdf_derived_classic_overview_templates() {
    let pack = method_template_pack_json().expect("template pack should load");
    let json: serde_json::Value =
        serde_json::from_str(&pack).expect("template pack should be valid JSON");
    let templates = json["templates"]
        .as_array()
        .expect("templates should be an array");

    for id in [
        "simclr_contrastive_y_branch",
        "unet_skip_encoder_decoder",
        "ddpm_bidirectional_chain",
        "transformer_encoder_decoder_stack",
    ] {
        assert!(
            templates.iter().any(|template| template["id"] == id),
            "missing template {id}"
        );
    }

    let simclr = templates
        .iter()
        .find(|template| template["id"] == "simclr_contrastive_y_branch")
        .expect("SimCLR template should exist");
    assert_eq!(
        simclr["source"]["pdf_url"],
        "https://arxiv.org/pdf/2002.05709"
    );
    assert_eq!(simclr["extraction"]["page"], 2);
    assert_eq!(simclr["extraction"]["embedded_bitmap_count"], 0);
    assert_eq!(simclr["extraction"]["derived_from_pdf_vector_page"], true);
    assert!(simclr["layout_pattern"]["slots"]
        .as_array()
        .expect("slots should exist")
        .iter()
        .any(|slot| slot["role"] == "shared_source"));
    assert!(simclr["adaptation_guidelines"]
        .as_array()
        .expect("guidelines should exist")
        .iter()
        .any(|line| line
            .as_str()
            .unwrap_or_default()
            .contains("two correlated views")));
}

#[test]
fn method_template_pack_routes_distillation_to_simclr_two_branch_template() {
    let pack = method_template_pack_json().expect("template pack should load");
    let json: serde_json::Value =
        serde_json::from_str(&pack).expect("template pack should be valid JSON");
    let selection_rules = json["selection_rules"]
        .as_array()
        .expect("selection_rules should be an array");
    let distillation_rule = selection_rules
        .iter()
        .find(|rule| rule["id"] == "teacher_student_distillation")
        .expect("distillation selection rule should exist");

    assert_eq!(
        distillation_rule["prefer_template"],
        "simclr_contrastive_y_branch"
    );
    let triggers = distillation_rule["when_method_mentions"]
        .as_array()
        .expect("triggers should be an array");
    for trigger in ["teacher", "student", "distillation", "residual"] {
        assert!(
            triggers.iter().any(|value| value == trigger),
            "missing trigger {trigger}"
        );
    }
    assert!(distillation_rule["adaptation"]
        .as_str()
        .unwrap_or_default()
        .contains("teacher and student as two correlated branches"));
    let anti_patterns = distillation_rule["avoid"]
        .as_array()
        .expect("distillation rule should declare anti-patterns");
    for expected in [
        "bottom-heavy separate inference lane",
        "residual as standalone node",
        "floating phase labels",
        "long input detours",
        "standalone inference note component",
        "asymmetric branch annotations",
        "connector-overlapping labels",
    ] {
        assert!(
            anti_patterns
                .iter()
                .any(|value| value.as_str().unwrap_or_default().contains(expected)),
            "missing anti-pattern {expected}"
        );
    }
}
