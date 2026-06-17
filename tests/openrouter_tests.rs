use methodfig::config::ImageConfig;
use methodfig::llm::openrouter_image::build_openrouter_image_payload;
use methodfig::llm::ImageRequest;

#[test]
fn openrouter_payload_defaults_to_image_only_modality() {
    let config = ImageConfig {
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        api_key: Some("key".into()),
        model: Some("model".into()),
        modalities: vec!["image".into()],
    };
    let payload = build_openrouter_image_payload(
        config.model.as_deref().unwrap(),
        &ImageRequest {
            prompt: "icon".into(),
            aspect_ratio: "1:1".into(),
            image_size: "1K".into(),
        },
        &config.modalities,
    );

    assert_eq!(payload["modalities"], serde_json::json!(["image"]));
    assert_eq!(payload["image_config"]["aspect_ratio"], "1:1");
}

#[test]
fn openrouter_payload_supports_text_and_image_modality() {
    let payload = build_openrouter_image_payload(
        "model",
        &ImageRequest {
            prompt: "icon".into(),
            aspect_ratio: "1:1".into(),
            image_size: "1K".into(),
        },
        &["image".into(), "text".into()],
    );

    assert_eq!(payload["modalities"], serde_json::json!(["image", "text"]));
}
