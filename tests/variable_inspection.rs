use rustapi::models::*;
#[test]
fn variable_origins_follow_nearest_enabled_scope_and_keep_secret_hints() {
    let mut c = Collection::new("Root");
    c.variables.insert("host".into(), "root".into());
    c.variables.insert("credential".into(), "private".into());
    c.variable_metadata.push(KeyValue {
        value_type: "secret".into(),
        ..KeyValue::new("credential", "private")
    });
    let mut folder = Folder::new("Nested");
    folder.variables.insert("host".into(), "folder".into());
    folder.variables.insert("off".into(), "disabled".into());
    folder.variable_metadata.push(KeyValue {
        enabled: false,
        ..KeyValue::new("off", "disabled")
    });
    let request = ApiRequest::default();
    let rid = request.id.clone();
    folder.requests.push(request);
    c.folders.push(folder);
    let origins = c.variable_origins(&rid);
    assert_eq!(origins["host"].0, "Folder: Nested");
    assert!(origins["credential"].1);
    assert!(!origins.contains_key("off"));
    assert!(c.variable_origins("missing").is_empty());
}
