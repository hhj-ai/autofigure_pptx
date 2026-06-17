use methodfig::config::AppConfig;

#[test]
fn env_parsing_reads_separate_role_configs() {
    std::env::set_var("METHODFIG_REASONER_BASE_URL", "https://reasoner.test/v1");
    std::env::set_var("METHODFIG_REASONER_API_KEY", "reasoner-key");
    std::env::set_var("METHODFIG_REASONER_MODEL", "reasoner-model");
    std::env::set_var("METHODFIG_CODER_API_KEY", "coder-key");
    std::env::set_var("METHODFIG_CODER_MODEL", "coder-model");
    std::env::set_var("METHODFIG_IMAGE_PROVIDER", "openrouter");
    std::env::set_var("METHODFIG_IMAGE_BASE_URL", "https://openrouter.ai/api/v1");
    std::env::set_var("METHODFIG_IMAGE_API_KEY", "image-key");
    std::env::set_var("METHODFIG_IMAGE_MODEL", "image-model");
    std::env::set_var("METHODFIG_IMAGE_MODALITIES", "image,text");

    let config = AppConfig::from_env().expect("env should parse");

    assert_eq!(config.reasoner.base_url, "https://reasoner.test/v1");
    assert_eq!(config.reasoner.model.as_deref(), Some("reasoner-model"));
    assert!(config.reasoner.is_configured());
    assert!(config.coder.is_configured());
    assert_eq!(config.image.provider, "openrouter");
    assert_eq!(config.image.modalities, vec!["image", "text"]);
    assert!(config.image.is_configured());
}
