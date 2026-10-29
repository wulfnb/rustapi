mod support;
use rustapi::{
    http::{Cancellation, HttpService},
    models::*,
    runner::{self, RunOptions},
    variables::Variables,
};
use std::collections::BTreeMap;
use support::*;

#[test]
fn variable_precedence_nested_resolution_and_cycles() {
    let mut vars = Variables::default();
    vars.collection.insert("host".into(), "collection".into());
    vars.environment.insert("host".into(), "env".into());
    vars.data.insert("host".into(), "data".into());
    vars.local.insert("host".into(), "local".into());
    vars.local.insert("base".into(), "http://{{host}}".into());
    vars.secrets.insert("token".into(), "private".into());
    assert_eq!(
        vars.resolve("{{base}}/{{vault:token}}").unwrap(),
        "http://local/private"
    );
    assert!(vars.resolve("{{missing}}").is_err());
    vars.local.insert("loop".into(), "{{loop}}".into());
    assert!(vars.resolve("{{loop}}").is_err());
}
#[test]
fn nested_collection_import_preserves_order_auth_variables_and_scripts() {
    let import = rustapi::import::json(include_bytes!("fixtures/collection-v2.1.json")).unwrap();
    let c = import.collection.unwrap();
    let all = c.ordered_requests(None);
    assert_eq!(
        all.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
        ["Login", "Profile", "Last"]
    );
    assert_eq!(all[1].auth.auth_type, AuthType::BearerToken);
    assert_eq!(all[1].auth.bearer_token, "{{token}}");
    assert_eq!(all[1].inherited_variables["base"], "{{host}}");
    assert!(all[0].post_script.contains("pm.environment.set"));
    assert!(!import.warnings.is_empty());
    assert_eq!(c.ordered_requests(Some(&c.folders[0].id)).len(), 1);
}
#[test]
fn import_rejects_incomplete_input_and_unsupported_body() {
    let mut v: serde_json::Value =
        serde_json::from_slice(include_bytes!("fixtures/collection-v2.1.json")).unwrap();
    v["item"][0]["request"]["body"]["mode"] = "unknown".into();
    assert!(rustapi::import::json(&serde_json::to_vec(&v).unwrap()).is_err());
    assert!(rustapi::import::json(br#"{"info":{"schema":"v1"}}"#).is_err());
    assert!(rustapi::import::curl("curl --unsupported http://local").is_err());
}
#[test]
fn curl_import_has_no_shell_execution_and_preserves_files() {
    let c=rustapi::import::curl("curl -X POST 'http://local/?x=1' -H 'X-Test: $(do-not-execute)' -F 'upload=@test.bin' -F 'name=hello world'").unwrap().collection.unwrap();
    let r = &c.requests[0];
    assert_eq!(r.headers[0].value, "$(do-not-execute)");
    assert!(r.form_data[0].file);
    assert_eq!(r.form_data[0].value, "test.bin");
    assert_eq!(r.form_data[1].value, "hello world");
}
#[test]
fn scripts_assert_extract_and_reject_unknown_apis() {
    let mut vars = Variables::default();
    let response = ApiResponse {
        status: 200,
        body: r#"{"token":"abc","ok":true}"#.into(),
        ..Default::default()
    };
    let tests=rustapi::scripts::execute("pm.environment.set('token',pm.response.json().token); pm.test('status',()=>{pm.response.to.have.status(200)}); pm.test('json',()=>{pm.expect(pm.response.json().ok).to.be.true}); pm.test('fails',()=>{pm.expect(1).to.eql(2)});",&mut vars,Some(&response),&Cancellation::default()).unwrap();
    assert_eq!(vars.environment["token"], "abc");
    assert_eq!(
        tests.iter().map(|t| t.passed).collect::<Vec<_>>(),
        [true, true, false]
    );
    assert!(rustapi::scripts::execute(
        "pm.sendRequest('http://forbidden')",
        &mut vars,
        None,
        &Cancellation::default()
    )
    .is_err());
    assert!(rustapi::scripts::execute(
        "require('node:fs')",
        &mut vars,
        None,
        &Cancellation::default()
    )
    .is_err());
}
#[test]
fn script_cpu_and_memory_limits_terminate_execution() {
    let start = std::time::Instant::now();
    let mut vars = Variables::default();
    assert!(
        rustapi::scripts::execute("while(true){}", &mut vars, None, &Cancellation::default())
            .is_err()
    );
    assert!(start.elapsed() < std::time::Duration::from_secs(3));
    assert!(rustapi::scripts::execute(
        "'x'.repeat(40*1024*1024)",
        &mut vars,
        None,
        &Cancellation::default()
    )
    .is_err());
    assert!(rustapi::scripts::execute(
        "Atomics.wait(new Int32Array(new SharedArrayBuffer(4)),0,0)",
        &mut vars,
        None,
        &Cancellation::default()
    )
    .is_err());
}
#[test]
fn login_extract_auth_chain_and_assertions_use_same_pipeline() {
    let server = Server::new(2, |i, r| {
        if i == 0 {
            Reply::json(r#"{"token":"session-token"}"#)
        } else {
            assert!(r
                .head
                .to_ascii_lowercase()
                .contains("authorization: bearer session-token"));
            Reply::json(r#"{"name":"Ada"}"#)
        }
    });
    let mut c = Collection::new("chain");
    let mut login = ApiRequest {
        url: format!("{}/login", server.url),
        ..Default::default()
    };
    login.extractors.push(Extractor {
        variable: "token".into(),
        pointer: "/token".into(),
    });
    login.checks.push(Assertion::default());
    let mut profile = ApiRequest {
        url: format!("{}/profile", server.url),
        ..Default::default()
    };
    profile.auth = AuthConfig {
        auth_type: AuthType::BearerToken,
        bearer_token: "{{token}}".into(),
        ..Default::default()
    };
    profile.checks.push(Assertion {
        name: "Name".into(),
        kind: AssertionKind::Json,
        target: "/name".into(),
        expected: "Ada".into(),
    });
    c.requests = vec![login, profile];
    let report = runner::run(
        &HttpService::default(),
        &c,
        None,
        &RunOptions::default(),
        BTreeMap::new(),
        &Cancellation::default(),
    )
    .unwrap();
    assert!(report.passed(), "{report:?}");
    assert_eq!(report.entries.len(), 2);
    assert!(!serde_json::to_string(&report)
        .unwrap()
        .contains("session-token"));
    server.finish();
}
#[test]
fn runner_data_iterations_stop_on_failure_and_junit() {
    let server = Server::new(1, |_, r| {
        assert!(r.head.contains("/Alice"));
        Reply::json("{}")
    });
    let mut c = Collection::new("data");
    let mut r = ApiRequest {
        url: format!("{}/{{{{name}}}}", server.url),
        ..Default::default()
    };
    r.name = "<unsafe & name>".into();
    r.checks.push(Assertion {
        expected: "201".into(),
        ..Default::default()
    });
    c.requests.push(r);
    let options = RunOptions {
        stop_on_failure: true,
        data: vec![
            BTreeMap::from([("name".into(), "Alice".into())]),
            BTreeMap::from([("name".into(), "Bob".into())]),
        ],
        ..Default::default()
    };
    let report = runner::run(
        &HttpService::default(),
        &c,
        None,
        &options,
        BTreeMap::new(),
        &Cancellation::default(),
    )
    .unwrap();
    assert!(!report.passed());
    assert_eq!(report.entries.len(), 1);
    assert!(report.junit().contains("&lt;unsafe &amp; name&gt;"));
    server.finish();
}
#[test]
fn data_csv_quotes_and_json_scalars() {
    let dir = tempfile::tempdir().unwrap();
    let csv = dir.path().join("data.csv");
    std::fs::write(&csv, "name,n\n\"Ada, A\",2\n").unwrap();
    assert_eq!(runner::load_data(&csv).unwrap()[0]["name"], "Ada, A");
    let json = dir.path().join("data.json");
    std::fs::write(&json, r#"[{"n":2,"ok":true}]"#).unwrap();
    assert_eq!(runner::load_data(&json).unwrap()[0]["ok"], "true");
}
#[test]
fn collection_mutations_resolve_stable_ids_after_deletion() {
    let mut c = Collection::new("IDs");
    let a = ApiRequest::new("A");
    let b = ApiRequest::new("B");
    let z = ApiRequest::new("C");
    let bid = b.id.clone();
    let zid = z.id.clone();
    c.requests = vec![a.clone(), b, z];
    c.remove_request(&a.id);
    c.request_mut(&bid).unwrap().url = "http://edited".into();
    assert_eq!(c.request(&zid).unwrap().name, "C");
    let mut f = Folder::new("nested");
    f.requests.push(c.remove_request(&bid).unwrap());
    c.folders.push(f);
    assert_eq!(c.request(&bid).unwrap().url, "http://edited");
}

#[test]
fn native_assertions_and_extraction_use_complete_response_not_preview() {
    let payload = format!(
        "{{\"padding\":\"{}\",\"token\":\"tail-value\"}}",
        "x".repeat(1024 * 1024)
    );
    let server = Server::new(1, move |_, _| Reply::json(&payload));
    let mut r = ApiRequest {
        url: server.url.clone(),
        ..Default::default()
    };
    r.checks.push(Assertion {
        name: "tail".into(),
        kind: AssertionKind::BodyContains,
        target: String::new(),
        expected: "tail-value".into(),
    });
    r.extractors.push(Extractor {
        variable: "token".into(),
        pointer: "/token".into(),
    });
    let mut vars = Variables::default();
    let response = runner::execute(
        &HttpService::default(),
        &r,
        &mut vars,
        &Cancellation::default(),
    );
    assert!(response.error.is_none(), "{:?}", response.error);
    assert!(response.tests[0].passed);
    assert_eq!(vars.local["token"], "tail-value");
    assert!(response.size > response.body.len());
    server.finish();
}
#[test]
fn collection_environment_disabled_flags_and_unsupported_auth_are_explicit() {
    let e=rustapi::import::json(br#"{"name":"dev","values":[{"key":"off","value":"x","enabled":false},{"key":"on","value":"y","enabled":true}]}"#).unwrap().environment.unwrap();
    assert!(!e.variables[0].enabled);
    let mut v: serde_json::Value =
        serde_json::from_slice(include_bytes!("fixtures/collection-v2.1.json")).unwrap();
    v["item"][0]["request"]["auth"] = serde_json::json!({"type":"digest"});
    let c = rustapi::import::json(&serde_json::to_vec(&v).unwrap())
        .unwrap()
        .collection
        .unwrap();
    assert_eq!(c.requests[0].auth.auth_type, AuthType::Unsupported);
}

#[test]
fn duplicate_nested_request_retains_its_parent_auth_scope() {
    let mut c = Collection::new("nested");
    let mut f = Folder::new("folder");
    f.auth = AuthConfig {
        auth_type: AuthType::BearerToken,
        bearer_token: "folder-token".into(),
        ..Default::default()
    };
    let mut r = ApiRequest::new("request");
    r.auth.auth_type = AuthType::Inherit;
    let id = r.id.clone();
    f.requests.push(r);
    c.folders.push(f);
    let copy = c.duplicate_request(&id).unwrap();
    assert!(c.requests.is_empty());
    assert_eq!(c.folders[0].requests.len(), 2);
    let effective = c
        .ordered_requests(None)
        .into_iter()
        .find(|r| r.id == copy.id)
        .unwrap();
    assert_eq!(effective.auth.bearer_token, "folder-token");
}

#[test]
fn script_collection_unset_is_retained_for_the_rest_of_the_run() {
    let server = Server::new(2, |_, _| Reply::json("{}"));
    let mut c = Collection::new("scopes");
    c.variables.insert("remove_me".into(), "original".into());
    c.requests.push(ApiRequest {
        url: server.url.clone(),
        post_script: "pm.collectionVariables.unset('remove_me');".into(),
        ..Default::default()
    });
    c.requests.push(ApiRequest{url:server.url.clone(),pre_script:"pm.test('removed',()=>{pm.expect(pm.collectionVariables.has('remove_me')).to.be.false});".into(),..Default::default()});
    let report = runner::run(
        &HttpService::default(),
        &c,
        None,
        &RunOptions::default(),
        Default::default(),
        &Cancellation::default(),
    )
    .unwrap();
    assert!(report.passed(), "{report:?}");
    server.finish();
}
