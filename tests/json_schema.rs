mod support;
use rustapi::{http::*, models::*, runner, schema, variables::Variables};
use serde_json::json;
#[test]
fn maintained_schema_validator_handles_refs_formats_composition_and_safe_errors() {
    let schema = json!({"$schema":"http://json-schema.org/draft-07/schema#","type":"object","required":["user"],"properties":{"user":{"$ref":"#/definitions/user"}},"definitions":{"user":{"type":"object","required":["email","age"],"properties":{"email":{"type":"string","format":"email"},"age":{"type":"integer","minimum":18}},"additionalProperties":false}}});
    assert!(schema::validate(
        &schema.to_string(),
        &json!({"user":{"email":"a@example.test","age":30}})
    )
    .unwrap()
    .is_empty());
    let errors = schema::validate(
        &schema.to_string(),
        &json!({"user":{"email":"private-invalid-value","age":3}}),
    )
    .unwrap();
    assert!(!errors.is_empty());
    assert!(errors.iter().any(|message| message.contains("/user/")));
    assert!(!errors.join(" ").contains("private-invalid-value"));
    for schema in [
        json!({"$ref":"https://127.0.0.1:1/private"}),
        json!({"$ref":"file:///private/secret.json"}),
        json!({"$ref":"#"}),
        json!({"type":"string","pattern":"(?=unsupported)"}),
    ] {
        assert!(schema::validate(&schema.to_string(), &json!("value")).is_err());
    }
    assert!(schema::validate(&" ".repeat(256 * 1024 + 1), &json!({})).is_err());
    assert!(schema::validate("{invalid", &json!({})).is_err());
    assert!(schema::validate(
        &json!({"const":{"$ref":"literal data"}}).to_string(),
        &json!({"$ref":"literal data"})
    )
    .unwrap()
    .is_empty());
    assert!(schema::validate(
        &json!({"oneOf":[{"type":"integer"},{"type":"string"}]}).to_string(),
        &json!(7)
    )
    .unwrap()
    .is_empty());
}
#[test]
fn schema_assertions_share_interactive_and_cli_execution_and_survive_exports() {
    let server = support::Server::new(2, |_, _| {
        support::Reply::json("{\"user\":{\"id\":7,\"email\":\"private-invalid\"}}")
    });
    let mut request=ApiRequest{url:server.url.clone(),checks:vec![Assertion{name:"Valid ID".into(),kind:AssertionKind::JsonSchema,target:"/user".into(),expected:json!({"type":"object","required":["id"],"properties":{"id":{"type":"integer"}}}).to_string()},Assertion{name:"Email schema".into(),kind:AssertionKind::JsonSchema,target:"/user".into(),expected:json!({"properties":{"email":{"format":"email"}}}).to_string()}],..Default::default()};
    let response = runner::execute(
        &HttpService::default(),
        &request,
        &mut Variables::default(),
        &Cancellation::default(),
    );
    assert!(response.error.is_none());
    assert!(response.tests[0].passed);
    assert!(!response.tests[1].passed);
    assert!(!serde_json::to_string(&response.tests)
        .unwrap()
        .contains("private-invalid"));
    request.name = "Schema request".into();
    let mut collection = Collection::new("Schemas");
    collection.requests.push(request);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("schema.json");
    std::fs::write(&path, serde_json::to_vec(&collection).unwrap()).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_rustapi"))
        .args([
            "run",
            path.to_str().unwrap(),
            "--workspace",
            dir.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["entries"][0]["tests"][0]["passed"], true);
    assert_eq!(report["entries"][0]["tests"][1]["passed"], false);
    assert!(!String::from_utf8_lossy(&output.stdout).contains("private-invalid"));
    let portable = rustapi::export::collection(&collection, true).unwrap();
    let copy = rustapi::import::json(&serde_json::to_vec(&portable.document).unwrap())
        .unwrap()
        .collection
        .unwrap();
    assert_eq!(copy.requests[0].checks[0].kind, AssertionKind::JsonSchema);
    server.finish();
}
