mod support;
use rustapi::{import, models::*, v3};
use serde_json::json;
#[test]
fn v3_directory_roundtrip_restores_native_metadata_and_requires_review_after_yaml_edits() {
    let mut c = Collection::new("Portable v3");
    c.description = "# API docs".into();
    c.variables
        .insert("host".into(), "https://example.test".into());
    let mut folder = Folder::new("Nested");
    let mut request = ApiRequest {
        url: "{{host}}/:id".into(),
        path_params: vec![KeyValue::new("id", "a/b")],
        headers: vec![
            KeyValue::new("X-Repeat", "one"),
            KeyValue::new("X-Repeat", "two"),
        ],
        body_type: BodyType::FormData,
        form_data: vec![KeyValue {
            file: true,
            content_type: "application/custom".into(),
            ..KeyValue::new("upload", "one.bin")
        }],
        pre_script: "pm.variables.set('x','yes')".into(),
        ..Default::default()
    };
    request.checks.push(Assertion::default());
    request.settings.timeout_ms = 4321;
    request.examples.push(
        RequestExample::capture(
            "Saved".into(),
            &request,
            &ApiResponse {
                status: 201,
                status_text: "Created".into(),
                body: "{\"id\":9}".into(),
                ..Default::default()
            },
        )
        .unwrap(),
    );
    folder.requests.push(request);
    c.folders.push(folder);
    c.requests.push(ApiRequest {
        url: "https://example.test/last".into(),
        ..Default::default()
    });
    c.order = vec![c.folders[0].id.clone(), c.requests[0].id.clone()];
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("export");
    v3::export_directory(&c, &out, true).unwrap();
    let folder_definition = v3::parse_yaml(
        &std::fs::read_to_string(out.join("0001-Nested/.resources/definition.yaml")).unwrap(),
    )
    .unwrap();
    assert!(folder_definition.get("auth").is_none());
    assert!(folder_definition.get("variables").is_none());
    let copy = v3::import_directory(&out).unwrap();
    assert!(!copy.requires_confirmation());
    let copy = copy.collection.unwrap();
    assert_eq!(copy.folders[0].requests[0].settings.timeout_ms, 4321);
    assert_eq!(copy.folders[0].requests[0].examples[0].response.status, 201);
    assert_eq!(copy.folders[0].requests[0].checks.len(), 1);
    assert_eq!(copy.ordered_requests(None)[0].url, "{{host}}/:id");
    assert_ne!(copy.id, c.id);
    assert!(v3::export_directory(&c, &out, true).is_err());
    let file = std::fs::read_dir(out.join("0001-Nested"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|ext| ext == "yaml"))
        .unwrap();
    let text = std::fs::read_to_string(&file).unwrap();
    std::fs::write(&file, text.replace("{{host}}/:id", "{{host}}/edited/:id")).unwrap();
    let imported = v3::import_directory(&out).unwrap();
    assert!(imported.requires_confirmation());
    assert!(imported
        .diagnostics
        .iter()
        .any(|item| item.feature == "stale_native_metadata"));
    assert!(imported.collection.unwrap().folders[0].requests[0]
        .url
        .contains("edited"));
}
#[test]
fn plain_v3_yaml_and_directory_support_order_examples_scripts_and_alias_extension() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".resources")).unwrap();
    std::fs::write(dir.path().join(".resources/definition.yaml"),"$kind: collection\nname: Plain\nvariables:\n  - key: host\n    value: https://example.test\nscripts:\n  - type: http:beforeRequest\n    code: pm.variables.set('scope', 'root')\n").unwrap();
    for (name, order) in [("first", 2.5), ("second", 1.2)] {
        std::fs::write(
            dir.path().join(format!("{name}.request.yml")),
            format!(
                "$kind: http-request\nmethod: GET\nurl: '{{{{host}}}}/{name}'\norder: {order}\n"
            ),
        )
        .unwrap();
    }
    std::fs::create_dir_all(dir.path().join("first.resources/examples")).unwrap();
    std::fs::write(dir.path().join("first.resources/examples/ok.example.yaml"),"$kind: http-example\nname: OK\nresponse:\n  statusCode: 200\n  statusText: OK\n  body:\n    type: json\n    content: '{\"ok\":true}'\n").unwrap();
    let imported = import::path(dir.path()).unwrap();
    assert!(!imported.requires_confirmation());
    let c = imported.collection.unwrap();
    let requests = c.ordered_requests(None);
    assert!(requests[0].url.ends_with("second"));
    assert!(requests[0].pre_script.contains("root"));
    assert_eq!(requests[1].examples.len(), 1);
    let doc = json!({"$kind":"collection","name":"Inline","items":[{"$kind":"http-request","name":"GET","url":"https://example.test","method":"GET"}]});
    assert!(v3::import_document(&doc).is_ok());
    assert!(import::json(&serde_json::to_vec(&doc).unwrap()).is_ok());
}
#[test]
fn v3_rejects_unsupported_resources_ambiguous_names_and_yaml_expansion() {
    for text in [
        "$kind: collection\nname: One\nname: Two",
        "root: &anchor value\ncopy: *anchor",
        "root: !include /etc/passwd",
        "---\na: 1\n---\nb: 2",
    ] {
        assert!(v3::parse_yaml(text).is_err(), "{text}");
    }
    let value = json!({"$kind":"collection","name":"Unsupported","items":[{"$kind":"grpc-request","name":"RPC"}]});
    assert!(v3::import_document(&value).is_err());
    let value = json!({"$kind":"collection","name":"Secret","variables":[{"key":"token","secret":true,"source":{"provider":"aws"}}]});
    assert!(v3::import_document(&value).is_err());
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("A.request.yaml"),
        "$kind: http-request\nurl: https://example.test\n",
    )
    .unwrap();
    std::fs::write(
        root.path().join("a.request.yml"),
        "$kind: http-request\nurl: https://example.test\n",
    )
    .unwrap();
    assert!(v3::import_directory(root.path()).is_err());
}
#[cfg(unix)]
#[test]
fn v3_rejects_symlink_resources_and_cli_exports_imports_without_mutating_sources() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("file.yaml"), "private").unwrap();
    symlink(
        outside.path().join("file.yaml"),
        root.path().join("escape.request.yaml"),
    )
    .unwrap();
    assert!(v3::import_directory(root.path()).is_err());
    std::fs::remove_file(root.path().join("escape.request.yaml")).unwrap();
    let mut c = Collection::new("CLI");
    c.requests.push(ApiRequest {
        url: "https://example.test".into(),
        ..Default::default()
    });
    let source = root.path().join("native.json");
    let output = root.path().join("v3");
    let workspace = root.path().join("workspace");
    std::fs::write(&source, serde_json::to_vec(&c).unwrap()).unwrap();
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_rustapi"))
        .args([
            "export",
            source.to_str().unwrap(),
            output.to_str().unwrap(),
            "--format",
            "v3",
            "--workspace",
            workspace.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_rustapi"))
        .args([
            "import",
            output.to_str().unwrap(),
            "--workspace",
            workspace.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        std::fs::read_dir(workspace.join("collections"))
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn cli_v3_run_uses_shared_variable_script_and_http_pipeline() {
    let server = support::Server::new(1, |_, request| {
        assert!(
            request.head.starts_with("POST /a%2Fb?q=a%20b "),
            "{}",
            request.head
        );
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&request.body).unwrap()["value"],
            "ready"
        );
        support::Reply::json("{\"id\":7}")
    });
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("collection");
    let workspace = root.path().join("workspace");
    std::fs::create_dir_all(&source).unwrap();
    let doc = json!({"$kind":"http-request","name":"Send","method":"POST","url":format!("{}/:id",server.url),"pathVariables":[{"key":"id","value":"a/b"}],"queryParams":[{"key":"q","value":"a b"}],"body":{"type":"json","content":"{\"value\":\"{{marker}}\"}"},"scripts":[{"type":"beforeRequest","code":"pm.variables.set('marker','ready')"},{"type":"afterResponse","code":"pm.test('status',()=>pm.response.to.have.status(200))"}]});
    std::fs::write(
        source.join("Send.request.yaml"),
        serde_json::to_vec(&doc).unwrap(),
    )
    .unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_rustapi"))
        .args([
            "run",
            source.to_str().unwrap(),
            "--workspace",
            workspace.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["entries"][0]["status"], 200);
    server.finish();
}
