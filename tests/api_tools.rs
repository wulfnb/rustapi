mod support;
use rustapi::{
    http::{Cancellation, HttpService},
    models::*,
    openapi::Specification,
};
use serde_json::json;
fn specification(base: &str) -> serde_json::Value {
    json!({"openapi":"3.0.3","info":{"title":"Pets","version":"1"},"servers":[{"url":base}],"paths":{"/pets/{id}":{"parameters":[{"name":"id","in":"path","required":true,"schema":{"type":"integer"}}],"get":{"operationId":"getPet","tags":["Pets"],"responses":{"200":{"description":"Found","content":{"application/json":{"schema":{"$ref":"#/components/schemas/Pet"}}}},"404":{"description":"Missing"}}}}},"components":{"schemas":{"Pet":{"type":"object","required":["id"],"properties":{"id":{"type":"integer"},"name":{"type":"string","nullable":true}}}}}})
}
#[test]
fn openapi_import_resolves_parameters_references_and_runs_response_contracts() {
    let server = support::Server::new(2, |index, request| {
        assert!(request.head.starts_with("GET /pets/1 "));
        support::Reply::json(if index == 0 {
            r#"{"id":1,"name":null}"#
        } else {
            r#"{"id":"wrong"}"#
        })
    });
    let spec = Specification::parse(&specification(&server.url).to_string()).unwrap();
    assert!(spec.lint().is_empty());
    let (collection, issues) = spec.import().unwrap();
    assert!(issues.is_empty());
    let request = collection.ordered_requests(None).remove(0);
    assert!(request.url.ends_with("/pets/:id"));
    assert_eq!(request.path_params[0].value, "1");
    let mut vars = rustapi::variables::Variables::default();
    let service = HttpService::default();
    let first = rustapi::runner::execute(&service, &request, &mut vars, &Cancellation::default());
    assert!(first.tests.iter().all(|t| t.passed), "{:?}", first.tests);
    let second = rustapi::runner::execute(&service, &request, &mut vars, &Cancellation::default());
    assert!(!second.tests[0].passed);
    server.finish();
    let imported =
        rustapi::import::json(specification("https://example.test").to_string().as_bytes())
            .unwrap();
    assert!(imported.collection.is_some() && !imported.requires_confirmation());
}
#[test]
fn lint_and_drift_find_invalid_contracts_and_resolved_schema_changes() {
    let before = specification("https://example.test");
    let spec = Specification::parse(&before.to_string()).unwrap();
    let mut changed = before.clone();
    changed["components"]["schemas"]["Pet"]["properties"]["id"]["type"] = "string".into();
    let drift = spec
        .drift(&Specification::parse(&changed.to_string()).unwrap())
        .unwrap();
    assert_eq!(drift.len(), 1);
    assert_eq!(drift[0].change, "changed");
    assert!(drift[0].requires_review);
    changed["paths"]["/pets/{id}"]["parameters"][0]["required"] = false.into();
    assert!(Specification::parse(&changed.to_string())
        .unwrap()
        .lint()
        .iter()
        .any(|i| i.severity == "error"));
    changed["components"]["schemas"]["Pet"] = json!({"$ref":"https://evil.test/schema.json"});
    let spec = Specification::parse(&changed.to_string()).unwrap();
    assert!(spec.import().is_err());
    assert!(Specification::parse("openapi: 2.0.0").is_err());
    let recursive = json!({"$ref":"#/components/schemas/Pet"});
    assert!(spec.resolve(&recursive).is_err());
}
#[test]
fn response_status_media_type_and_json_failures_are_explicit() {
    let responses=json!({"2XX":{"description":"success","content":{"application/json":{"schema":{"type":"object"}}}},"default":{"description":"fallback"}}).to_string();
    let mut response = ApiResponse {
        status: 201,
        headers: vec![(
            "Content-Type".into(),
            "application/json; charset=utf-8".into(),
        )],
        body: "{}".into(),
        ..Default::default()
    };
    assert!(rustapi::openapi::response_contract(&responses, &response)
        .unwrap()
        .is_empty());
    response.body = "bad".into();
    assert!(rustapi::openapi::response_contract(&responses, &response).is_err());
    response.status = 400;
    assert!(rustapi::openapi::response_contract(&responses, &response)
        .unwrap()
        .is_empty());
    assert!(rustapi::openapi::response_contract("{}", &response).is_err());
}
fn collection() -> Collection {
    let mut c = Collection::new("Docs <script>alert(1)</script>");
    let mut r = ApiRequest::new("GET <img onerror=alert(1)>");
    r.url = "{{baseUrl}}/pets/:id".into();
    r.headers
        .push(KeyValue::new("Authorization", "private-token"));
    r.body = "line\n```\n<script>bad</script>\n".into();
    r.examples.push(
        RequestExample::capture(
            "Example".into(),
            &r,
            &ApiResponse {
                status: 200,
                headers: vec![("Content-Type".into(), "application/json".into())],
                body: r#"{"token":"private-response","id":1}"#.into(),
                ..Default::default()
            },
        )
        .unwrap(),
    );
    c.requests.push(r);
    c
}
#[test]
fn standalone_docs_escape_html_redact_credentials_and_never_replace_existing_output() {
    let c = collection();
    let docs = rustapi::documentation::generate(&c, false).unwrap();
    assert!(!docs.html.contains("<script>"));
    assert!(docs.html.contains("&lt;script&gt;"));
    assert!(docs.html.contains("Content-Security-Policy"));
    for secret in ["private-token", "private-response"] {
        assert!(!docs.html.contains(secret));
        assert!(!docs.markdown.contains(secret));
    }
    assert!(docs.markdown.contains("````\nline\n```"));
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("docs");
    rustapi::documentation::write(&c, &out, false).unwrap();
    assert!(out.join("index.html").exists());
    assert!(out.join("README.md").exists());
    assert!(rustapi::documentation::write(&c, &out, false).is_err());
}
#[test]
fn mock_examples_match_query_headers_json_and_simulate_delay_status_disconnect() {
    use base64::Engine;
    let mut config = rustapi::mocks::MockConfig::from_examples(&collection()).unwrap();
    assert_eq!(config.rules[0].path, "/pets/:id");
    let rule = &mut config.rules[0];
    rule.query.insert("mode".into(), "full".into());
    rule.headers.insert("X-Mode".into(), "yes".into());
    rule.method = "POST".into();
    rule.json.insert("/id".into(), json!(1));
    rule.status = 503;
    rule.delay_ms = 50;
    rule.body_base64 = base64::engine::general_purpose::STANDARD.encode(b"simulated error");
    let server = rustapi::mocks::MockServer::start(config.clone()).unwrap();
    let client = reqwest::blocking::Client::new();
    let url = format!("http://{}/pets/7?mode=full", server.server.address);
    assert_eq!(client.get(&url).send().unwrap().status(), 404);
    let start = std::time::Instant::now();
    let response = client
        .post(&url)
        .header("X-Mode", "yes")
        .json(&json!({"id":1}))
        .send()
        .unwrap();
    assert_eq!(response.status(), 503);
    assert_eq!(response.text().unwrap(), "simulated error");
    assert!(start.elapsed().as_millis() >= 50);
    assert_eq!(
        server.unmatched.load(std::sync::atomic::Ordering::Relaxed),
        1
    );
    drop(server);
    config.port = 0;
    config.rules[0].disconnect = true;
    let server = rustapi::mocks::MockServer::start(config).unwrap();
    assert!(client
        .post(format!("http://{}/pets/9?mode=full", server.server.address))
        .header("X-Mode", "yes")
        .json(&json!({"id":1}))
        .send()
        .is_err());
}
