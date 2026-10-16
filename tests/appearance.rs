use fyle::appearance::{Appearance, SettingsStore};
#[test]
fn appearance_defaults_preserve_original_and_round_trip_bounded_settings() {
    let dir = tempfile::tempdir().unwrap();
    let store = SettingsStore::new(dir.path().join("appearance.json"));
    let mut settings = store.load().unwrap();
    assert_eq!(settings.highlight, [0, 212, 255]);
    assert!(settings.effects);
    settings.intensity = 5.0;
    settings.scale = 0.01;
    store.save(&settings).unwrap();
    let loaded = store.load().unwrap();
    assert_eq!(loaded.intensity, 1.0);
    assert_eq!(loaded.scale, 0.85);
    store.save(&Appearance::default()).unwrap();
    assert_eq!(store.load().unwrap(), Appearance::default());
    std::fs::write(&store.path, "{broken").unwrap();
    assert!(store.load().is_err());
}
