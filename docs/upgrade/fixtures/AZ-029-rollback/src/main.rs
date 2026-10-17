fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    let config = fixture.path().join("config.json");
    let baseline = old_kernel::store::settings::Settings {
        config_path: config.clone(),
        openai_api_key: "rollback-fixture-api-key".into(),
        vision_api_key: "rollback-fixture-vision-key".into(),
        ..Default::default()
    };
    baseline.save()?;
    let backup = std::fs::read(&config)?;
    let old_read = old_kernel::store::settings::Settings::load(&config)?;
    assert_eq!(old_read.openai_api_key, baseline.openai_api_key);
    assert_eq!(old_read.vision_api_key, baseline.vision_api_key);
    let candidate = candidate_kernel::store::settings::Settings::load(&config)?;
    assert_eq!(candidate.vision_api_key, baseline.vision_api_key);
    candidate.save()?;
    let encrypted = std::fs::read_to_string(&config)?;
    assert!(!encrypted.contains(&baseline.openai_api_key));
    assert!(!encrypted.contains(&baseline.vision_api_key));
    let downgraded = old_kernel::store::settings::Settings::load(&config)?;
    assert_eq!(downgraded.openai_api_key, baseline.openai_api_key);
    assert_ne!(downgraded.vision_api_key, baseline.vision_api_key);
    println!("CONFIRMED: direct downgrade does not recover candidate-encrypted vision key");
    std::fs::write(&config, &backup)?;
    assert_eq!(std::fs::read(&config)?, backup);
    let restored = old_kernel::store::settings::Settings::load(&config)?;
    assert_eq!(restored.openai_api_key, baseline.openai_api_key);
    assert_eq!(restored.vision_api_key, baseline.vision_api_key);
    println!("PASS: exact pre-upgrade config restored both fixture keys with original key file");
    Ok(())
}
