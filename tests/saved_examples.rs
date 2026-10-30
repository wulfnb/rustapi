use rustapi::{import, models::*, secrets, storage};
use serde_json::json;
#[test]
fn saved_examples_import_edit_restart_and_redact() {
    let source = json!({"info":{"name":"Examples","schema":"v2.1.0"},"item":[{"name":"Request","request":{"url":"https://example.test/live"},"response":[{"name":"Saved","code":201,"status":"Created","originalRequest":{"method":"POST","url":"https://example.test/example","body":{"mode":"raw","raw":"payload"}},"header":[{"key":"Set-Cookie","value":"private-cookie"},{"key":"X-Multi","value":"a"},{"key":"X-Multi","value":"b"}],"body":"{\"token\":\"private-token\",\"id\":7}","_previewlanguage":"json"}]}]});
    let imported = import::json(&serde_json::to_vec(&source).unwrap()).unwrap();
    assert!(!imported.requires_confirmation());
    let mut collection = imported.collection.unwrap();
    let example = &mut collection.requests[0].examples[0];
    assert_eq!(example.request.method, HttpMethod::POST);
    assert_eq!(
        example.response().unwrap().raw_body,
        example.response.body.as_bytes()
    );
    assert_eq!(example.response.headers.len(), 3);
    assert_eq!(example.metadata["_previewlanguage"], "json");
    example.name = "Edited".into();
    example.set_text("{\"token\":\"private-token\",\"id\":8}".into());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("saved.json");
    storage::export_collection(&collection, &path).unwrap();
    let loaded = storage::load_collection(&path).unwrap();
    assert_eq!(loaded.requests[0].examples[0].name, "Edited");
    assert!(loaded.requests[0].examples[0]
        .response()
        .unwrap()
        .body
        .contains('8'));
    let duplicate = loaded.requests[0].duplicate();
    collection.requests.push(duplicate);
    validate_collection(&collection).unwrap();
    let redacted = secrets::redact_collection(&loaded);
    let text = serde_json::to_string(&redacted).unwrap();
    assert!(!text.contains("private-cookie"));
    assert!(!text.contains("private-token"));
    assert!(!String::from_utf8(
        redacted.requests[0].examples[0]
            .response()
            .unwrap()
            .raw_body
    )
    .unwrap()
    .contains("private-token"));
    let mut history = RequestHistory::new(5);
    history.add(loaded.requests[0].clone(), None);
    assert!(history.entries[0].request.examples.is_empty());
}
#[test]
fn binary_examples_keep_exact_bytes_and_reject_oversized_or_nested_data() {
    let request = ApiRequest::default();
    let response = ApiResponse {
        status: 200,
        raw_body: vec![0, 255, 10, 128],
        body: "binary preview".into(),
        ..Default::default()
    };
    let mut example = RequestExample::capture("Binary".into(), &request, &response).unwrap();
    assert_eq!(example.response().unwrap().raw_body, response.raw_body);
    example.request.examples.push(example.clone());
    assert!(example.validate().is_err());
    let response = ApiResponse {
        status: 200,
        raw_body: vec![0; 1024 * 1024 + 1],
        ..Default::default()
    };
    assert!(RequestExample::capture("large".into(), &request, &response).is_err());
}
