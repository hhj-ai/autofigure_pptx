use methodfig::schema::{AssetSpec, StyleName};
use methodfig::tools::asset_gen::{asset_cache_key, build_asset_prompt, materialize_assets};

#[test]
fn asset_cache_key_is_stable_for_same_spec() {
    let spec = AssetSpec::generated_icon("encoder_icon", "encoder module");
    let first = asset_cache_key(&spec).expect("hash should succeed");
    let second = asset_cache_key(&spec).expect("hash should succeed");
    assert_eq!(first, second);
    assert_eq!(first.len(), 64);
}

#[test]
fn asset_cache_key_changes_when_prompt_changes() {
    let first = AssetSpec::generated_icon("encoder_icon", "encoder module");
    let second = AssetSpec::generated_icon("encoder_icon", "memory module");
    assert_ne!(
        asset_cache_key(&first).unwrap(),
        asset_cache_key(&second).unwrap()
    );
}

#[test]
fn built_asset_prompt_enforces_no_text_and_small_local_asset_policy() {
    let spec = AssetSpec::generated_icon("robot_icon", "robot policy module");
    let prompt = build_asset_prompt(&spec, StyleName::WpsClean);
    assert!(prompt.contains("minimal flat vector-style pictogram"));
    assert!(prompt.contains("no text"));
    assert!(prompt.contains("not a full pipeline diagram"));
}

#[test]
fn mock_asset_materialization_writes_round_asset_and_cache() {
    let temp = tempfile::tempdir().expect("tempdir");
    let run_dir = temp.path().join("run");
    let round_dir = run_dir.join("round_000");
    std::fs::create_dir_all(&round_dir).expect("round dir");
    let plan = methodfig::schema::FigurePlan::mock_from_method(
        "A method with a compact student module.",
        StyleName::WpsClean,
        methodfig::schema::CanvasAspect::PaperWide,
        85,
    );
    let config = methodfig::config::AppConfig {
        reasoner: methodfig::config::RoleConfig {
            base_url: "https://example.test/v1".into(),
            api_key: None,
            model: None,
        },
        coder: methodfig::config::RoleConfig {
            base_url: "https://example.test/v1".into(),
            api_key: None,
            model: None,
        },
        vision: methodfig::config::RoleConfig {
            base_url: "https://example.test/v1".into(),
            api_key: None,
            model: None,
        },
        image: methodfig::config::ImageConfig {
            provider: "openrouter".into(),
            base_url: "https://openrouter.ai/api/v1".into(),
            api_key: None,
            model: None,
            modalities: vec!["image".into()],
        },
    };

    let paths = materialize_assets(
        &plan,
        &run_dir,
        &round_dir,
        methodfig::schema::ImageProviderKind::OpenRouter,
        &config,
        true,
    )
    .expect("mock asset generation");

    assert!(!paths.is_empty());
    assert!(paths.values().all(|path| path.exists()));
    assert!(paths.values().all(|path| path.is_absolute()));
    assert!(std::fs::read_dir(run_dir.join("asset_cache"))
        .expect("cache dir")
        .next()
        .is_some());
}
