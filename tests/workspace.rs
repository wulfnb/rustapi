use rustapi::{models::*, storage::*, workspace::*};
#[test]
fn storage_identity_rename_import_copy_and_atomic_failure() {
    let dir = tempfile::tempdir().unwrap();
    std::env::set_var("RUSTAPI_DATA_DIR", dir.path());
    open_workspace(dir.path()).unwrap();
    init_storage().unwrap();
    let a = Collection::new("same");
    let b = Collection::new("same");
    let pa = save_collection(&a, None).unwrap();
    let pb = save_collection(&b, None).unwrap();
    assert_ne!(pa, pb);
    assert_eq!(load_all_collections().len(), 2);
    delete_collection(&a).unwrap();
    assert!(!pa.exists());
    let mut e = Environment::new("old");
    save_environment(&e).unwrap();
    e.name = "new".into();
    save_environment(&e).unwrap();
    assert_eq!(load_all_environments().len(), 1);
    delete_environment(&e).unwrap();
    assert!(load_all_environments().is_empty());
    let before = std::fs::read(&pb).unwrap();
    let mut copy = import_collection(&pb).unwrap();
    assert!(copy.file_path.is_none());
    copy.id = uuid::Uuid::new_v4().to_string();
    copy.name = "edited".into();
    save_collection(&copy, None).unwrap();
    assert_eq!(std::fs::read(&pb).unwrap(), before);
    assert!(atomic_write(dir.path(), b"bad").is_err());
    assert_eq!(std::fs::read(&pb).unwrap(), before);
    std::env::remove_var("RUSTAPI_DATA_DIR");
}
#[test]
fn manifest_schema_validation_and_git_ignores() {
    let dir = tempfile::tempdir().unwrap();
    open_workspace(dir.path()).unwrap();
    let ignore = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
    assert!(ignore.contains("/.local/"));
    std::fs::write(
        dir.path().join("rustapi.json"),
        r#"{"schema_version":99,"name":"future"}"#,
    )
    .unwrap();
    assert!(open_workspace(dir.path()).is_err());
}
#[test]
fn vault_roundtrip_wrong_password_and_tamper_detection() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vault");
    let values = std::collections::BTreeMap::from([("token".into(), "never-plaintext".into())]);
    rustapi::secrets::save(&path, "correct password 123", &values).unwrap();
    assert!(!String::from_utf8_lossy(&std::fs::read(&path).unwrap()).contains("never-plaintext"));
    assert_eq!(
        rustapi::secrets::load(&path, "correct password 123").unwrap(),
        values
    );
    assert!(rustapi::secrets::load(&path, "wrong password 123").is_err());
    let mut json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let n = json["ciphertext"][0].as_u64().unwrap();
    json["ciphertext"][0] = ((n + 1) % 256).into();
    std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
    assert!(rustapi::secrets::load(&path, "correct password 123").is_err());
}
#[test]
fn history_redacts_auth_body_response_and_sensitive_query() {
    let mut history = RequestHistory::new(10);
    let r = ApiRequest {
        url: "http://local/?access_token=never-plaintext".into(),
        body: "never-plaintext".into(),
        auth: AuthConfig {
            auth_type: AuthType::BearerToken,
            bearer_token: "never-plaintext".into(),
            ..Default::default()
        },
        ..Default::default()
    };
    history.add(
        r,
        Some(ApiResponse {
            body: "never-plaintext".into(),
            raw_body: b"never-plaintext".to_vec(),
            ..Default::default()
        }),
    );
    assert!(!serde_json::to_string(&history)
        .unwrap()
        .contains("never-plaintext"));
}
