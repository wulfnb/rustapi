use rustapi::{import, models::*, storage};
use serde_json::json;

#[test]
fn structured_descriptions_and_variable_metadata_survive_native_storage() {
    let description = json!({"content":"# Endpoint α\nDocumentation", "type":"text/markdown", "extension":{"version":2}});
    let source = json!({"info":{"name":"Metadata", "schema":"v2.1.0", "description":description},
        "variable":[{"key":"base","value":"https://example.test","type":"string","description":description,"id":"variable-1","system":true},
                    {"key":"off","value":"disabled","disabled":true,"type":"secret","description":description}],
        "item":[{"name":"Folder","description":description,
            "variable":[{"key":"limit","value":"10","description":description,"disabled":true}],
            "item":[{"name":"Request","request":{"url":"https://example.test","description":description,
                "header":[{"key":"Accept","value":"text/plain","description":description}]}}]}]});
    let imported = import::json(&serde_json::to_vec(&source).unwrap()).unwrap();
    assert!(!imported.requires_confirmation());
    let original = imported.collection.unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("metadata.json");
    storage::export_collection(&original, &path).unwrap();
    let stored = storage::load_collection(&path).unwrap();
    let copy = import::json(&std::fs::read(&path).unwrap())
        .unwrap()
        .collection
        .unwrap();
    for collection in [stored, copy] {
        assert_eq!(
            encode_description(&collection.description, &collection.description_metadata),
            description
        );
        let folder = &collection.folders[0];
        assert_eq!(
            encode_description(&folder.description, &folder.description_metadata),
            description
        );
        let request = &folder.requests[0];
        assert_eq!(
            encode_description(&request.description, &request.description_metadata),
            description
        );
        let header = &request.headers[0];
        assert_eq!(
            encode_description(&header.description, &header.description_metadata),
            description
        );
        assert_eq!(collection.variable_metadata.len(), 2);
        assert_eq!(
            collection.variable_metadata[0].extensions["id"],
            "variable-1"
        );
        assert_eq!(collection.variable_metadata[0].extensions["system"], true);
        let rows = scope_variable_rows(&collection.variables, &collection.variable_metadata);
        assert_eq!(rows[1].value_type, "secret");
        assert!(!rows[1].enabled);
        assert_eq!(rows[1].value, "disabled");
        assert_eq!(
            encode_description(&rows[1].description, &rows[1].description_metadata),
            description
        );
        assert!(!folder.variable_metadata[0].enabled);
        let mut edited = request.clone();
        edited.description = "Edited documentation".into();
        let output = encode_description(&edited.description, &edited.description_metadata);
        assert_eq!(output["content"], "Edited documentation");
        assert_eq!(output["extension"], description["extension"]);
    }
}

#[test]
fn disabled_scope_rows_do_not_resolve_or_mask_enabled_ancestor_values() {
    let source = json!({"info":{"name":"Scopes","schema":"v2.1.0"},
        "variable":[{"key":"host","value":"parent"},{"key":"off","value":"hidden","disabled":true}],
        "item":[{"name":"Folder","variable":[{"key":"host","value":"ignored","disabled":true}],
            "item":[{"name":"Request","request":{"url":"https://example.test"}}]}]});
    let mut collection = import::json(&serde_json::to_vec(&source).unwrap())
        .unwrap()
        .collection
        .unwrap();
    // Old map-based editors must not accidentally activate a disabled metadata row.
    collection.variables.insert("off".into(), "edited".into());
    let request = collection.ordered_requests(None).remove(0);
    assert_eq!(request.inherited_variables["host"], "parent");
    assert!(!request.inherited_variables.contains_key("off"));
    collection.variables.remove("host");
    let rows = scope_variable_rows(&collection.variables, &collection.variable_metadata);
    assert!(rows.iter().filter(|r| r.key == "host").all(|r| !r.enabled));
    assert!(!collection.ordered_requests(None)[0]
        .inherited_variables
        .contains_key("host"));
}

#[test]
fn new_metadata_and_path_values_obey_default_export_and_history_redaction() {
    let mut collection = Collection::new("Redaction");
    collection
        .variables
        .insert("credential".into(), "private-typed".into());
    collection.variable_metadata.push(KeyValue {
        value_type: "secret".into(),
        ..KeyValue::new("credential", "private-typed")
    });
    let mut request = ApiRequest {
        path_params: vec![KeyValue::new("access_token", "private-path")],
        ..Default::default()
    };
    request
        .imported_settings
        .insert("custom-secret".into(), "private-setting".into());
    collection.requests.push(request.clone());
    let redacted =
        serde_json::to_string(&rustapi::secrets::redact_collection(&collection)).unwrap();
    for value in ["private-typed", "private-path", "private-setting"] {
        assert!(!redacted.contains(value));
    }
    assert_eq!(collection.variable_metadata[0].value, "private-typed");
    let mut history = RequestHistory::new(10);
    request.name = "History".into();
    history.add(request, None);
    let history = serde_json::to_string(&history).unwrap();
    assert!(!history.contains("private-path"));
    assert!(!history.contains("private-setting"));
}
