use rustapi::{models::KeyValue, storage::*};
#[test]
fn bulk_and_presets_preserve_rows_and_validate_credentials() {
    let root = tempfile::tempdir().unwrap();
    let mut row = KeyValue::new("X-Repeat", "first");
    row.description = "Comment".into();
    row.extensions.insert("id".into(), "source-id".into());
    let mut disabled = row.clone();
    disabled.value = "second".into();
    disabled.enabled = false;
    let rows = vec![row, disabled];
    let json = serde_json::to_string(&rows).unwrap();
    assert_eq!(
        serde_json::to_value(parse_bulk_rows(&json).unwrap()).unwrap(),
        serde_json::to_value(&rows).unwrap()
    );
    assert!(parse_bulk_rows("{broken").is_err());
    assert!(parse_bulk_rows(&" ".repeat(1024 * 1024 + 1)).is_err());
    let mut presets = HeaderPresets::default();
    presets.add("Repeated", &rows).unwrap();
    presets
        .add(
            "Auth",
            &[KeyValue::new("Authorization", "Bearer {{vault:access}}")],
        )
        .unwrap();
    assert!(presets.add("Repeated", &rows).is_err());
    for value in [
        "private-secret",
        "{{token}} literal {{other}}",
        "Bearer {{token}} leaked",
    ] {
        assert!(presets
            .add("Bad", &[KeyValue::new("Authorization", value)])
            .is_err());
    }
    presets.save(root.path()).unwrap();
    let stored = HeaderPresets::load(root.path()).unwrap();
    assert_eq!(stored.presets.len(), 2);
    assert_eq!(
        serde_json::to_value(&stored.presets[0].headers).unwrap(),
        serde_json::to_value(rows).unwrap()
    );
    let path = root.path().join(".local/header-presets.json");
    std::fs::write(path, br#"{"schema_version":99,"presets":[]}"#).unwrap();
    assert!(HeaderPresets::load(root.path()).is_err());
}
