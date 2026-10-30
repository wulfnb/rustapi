mod support;
use rustapi::{
    http::{Cancellation, HttpService},
    models::*,
    variables::Variables,
};

#[test]
fn imported_path_variables_are_encoded_once_and_shared_by_execution_and_snippets() {
    let server = support::Server::new(1, |_, request| {
        assert!(request
            .head
            .starts_with("GET /static%2Fpart/a%2Fb%20%3F%23%25%CE%B1?name=A%20B HTTP/1.1"));
        support::Reply::json("{}")
    });
    let source = serde_json::json!({"info":{"name":"Paths","schema":"v2.1.0"},"item":[{"name":"Get",
        "request":{"url":{"raw":format!("{}/static%2Fpart/:id#anchor",server.url),
        "variable":[{"key":"id","value":"{{identifier}}","description":"Identifier"}],
        "query":[{"key":"name","value":"A B"}]}}}]});
    let imported = rustapi::import::json(&serde_json::to_vec(&source).unwrap()).unwrap();
    assert!(!imported.requires_confirmation());
    let collection = imported.collection.unwrap();
    let request = collection.ordered_requests(None).remove(0);
    assert_eq!(request.path_params[0].description, "Identifier");
    let mut vars = Variables::default();
    vars.local.insert("identifier".into(), "a/b ?#%α".into());
    let prepared = vars.resolve_request(&request).unwrap();
    assert!(prepared
        .url
        .ends_with("/static%2Fpart/a%2Fb%20%3F%23%25%CE%B1#anchor"));
    for language in rustapi::http::CodeGenLanguage::all() {
        let sample = rustapi::http::generate_code(&prepared, *language, None);
        assert!(
            sample.contains("/static%2Fpart/a%2Fb%20%3F%23%25%CE%B1?name=A%20B"),
            "{sample}"
        );
    }
    let response = rustapi::runner::execute(
        &HttpService::default(),
        &request,
        &mut vars,
        &Cancellation::default(),
    );
    assert!(response.error.is_none(), "{:?}", response.error);
    server.finish();
    let copy = rustapi::import::json(&serde_json::to_vec(&collection).unwrap())
        .unwrap()
        .collection
        .unwrap();
    assert_eq!(copy.requests[0].path_params[0].value, "{{identifier}}");
}

#[test]
fn disabled_missing_duplicate_and_dot_path_variables_fail_before_network_io() {
    let vars = Variables::default();
    let mut request = ApiRequest {
        url: "https://example.test/items/:id".into(),
        path_params: vec![KeyValue::new("other", "value")],
        ..Default::default()
    };
    assert!(vars.resolve_request(&request).is_err());
    request.path_params = vec![KeyValue::new("id", "..")];
    assert!(vars.resolve_request(&request).is_err());
    request.path_params[0].value = "value".into();
    request.path_params[0].enabled = false;
    assert!(vars.resolve_request(&request).is_err());
    request.path_params[0].enabled = true;
    request.path_params.push(KeyValue::new("id", "second"));
    assert!(vars.resolve_request(&request).is_err());
}
