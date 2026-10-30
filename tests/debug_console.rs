mod support;
use rustapi::{debug::DebugLog, http::*, models::*, runner, variables::Variables};
#[test]
fn script_logs_and_network_metadata_are_ordered_bounded_and_redacted() {
    let server = support::Server::new(1, |_, _| {
        support::Reply::json("{\"token\":\"private-response\",\"custom\":\"private-custom\"}")
    });
    let request=ApiRequest{url:format!("{}/private-path?key=private-query",server.url),headers:vec![KeyValue::new("X-Custom","private-header")],pre_script:"console.log('before',pm.variables.get('credential'));console.warn({password:'private-object',count:12});".into(),post_script:"console.log('after',pm.response.text(),pm.response.json().custom);console.error('diagnostic');console.log({[pm.response.json().token]:42});".into(),..Default::default()};
    let mut vars = Variables::default();
    vars.local
        .insert("credential".into(), "private-variable".into());
    let service = HttpService::default();
    let response = runner::execute(&service, &request, &mut vars, &Cancellation::default());
    assert!(response.error.is_none());
    let events = service.debug.snapshot();
    assert_eq!(
        events
            .iter()
            .map(|event| event.kind.as_str())
            .collect::<Vec<_>>(),
        vec!["script", "script", "network", "network", "script", "script", "script"]
    );
    let encoded = serde_json::to_string(&events).unwrap();
    for secret in [
        "private-path",
        "private-query",
        "private-header",
        "private-variable",
        "private-response",
        "private-custom",
        "private-object",
    ] {
        assert!(!encoded.contains(secret), "{secret}: {encoded}");
    }
    assert!(encoded.contains("before"));
    assert!(encoded.contains("total_ms"));
    server.finish();
    for i in 0..300 {
        service
            .debug
            .push("test", "info", serde_json::json!({"i":i}));
    }
    assert_eq!(service.debug.snapshot().len(), 256);
    service.debug.clear();
    assert!(service.debug.snapshot().is_empty());
}
#[test]
fn logs_before_script_failure_survive_without_applying_variable_changes() {
    let service = HttpService::default();
    let mut vars = Variables::default();
    let request=ApiRequest{url:"http://127.0.0.1:1/unused".into(),pre_script:"console.log('before failure');pm.variables.set('temporary','private-set');console.warn('private-set');throw new Error('private-error');".into(),..Default::default()};
    let response = runner::execute(&service, &request, &mut vars, &Cancellation::default());
    assert!(response.error.is_some());
    assert!(!vars.local.contains_key("temporary"));
    let text = serde_json::to_string(&service.debug.snapshot()).unwrap();
    assert!(text.contains("before failure"));
    assert!(!text.contains("private-set"));
    assert!(!text.contains("private-error"));
    assert!(!text.contains("network"));
}
#[test]
fn debug_queue_limits_event_size_and_cli_export_is_explicit() {
    let log = DebugLog::default();
    log.push(
        "test",
        "info",
        serde_json::json!({"large":"x".repeat(5000)}),
    );
    assert!(serde_json::to_vec(&log.snapshot()).unwrap().len() < 1000);
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("collection.json");
    let debug = root.path().join("debug.json");
    let mut c = Collection::new("Debug");
    c.requests.push(ApiRequest {
        url: "http://127.0.0.1:1/unused?token=private-query".into(),
        pre_script: "console.log('skip');pm.execution.skipRequest();".into(),
        ..Default::default()
    });
    std::fs::write(&source, serde_json::to_vec(&c).unwrap()).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_rustapi"))
        .args([
            "run",
            source.to_str().unwrap(),
            "--workspace",
            root.path().to_str().unwrap(),
            "--debug-report",
            debug.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = std::fs::read(debug).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("private-query"));
    let report: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["events"].as_array().unwrap().len(), 2);
}

#[test]
fn script_log_capture_bounds_count_and_handles_circular_values() {
    let service = HttpService::default();
    let request=ApiRequest{url:"http://127.0.0.1:1/never".into(),pre_script:"const circular={};circular.self=circular;for(let i=0;i<300;i++)console.log(i,circular);pm.execution.skipRequest();".into(),..Default::default()};
    let response = runner::execute(
        &service,
        &request,
        &mut Variables::default(),
        &Cancellation::default(),
    );
    assert!(response.skipped);
    assert!(response.error.is_none());
    let events = service.debug.snapshot();
    assert_eq!(events.len(), 101);
    assert_eq!(
        events.iter().filter(|event| event.kind == "script").count(),
        100
    );
    assert!(serde_json::to_vec(&events).unwrap().len() < 512 * 1024);
}
