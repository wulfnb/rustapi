use rustapi::{export, import, models::*};
use serde_json::{json, Value};
#[test]
fn portable_collection_roundtrip_preserves_order_metadata_examples_and_native_extensions() {
    let source = json!({"info":{"name":"Portable","schema":"v2.1.0","description":{"content":"# Guide","type":"text/markdown"}},"variable":[{"key":"base","value":"https://example.test","type":"string","description":"base URL"},{"key":"off","value":"disabled","disabled":true}],"item":[{"name":"Folder","item":[{"name":"Send","request":{"method":"POST","url":{"raw":"{{base}}/:id?other=encoded%2Fpair","variable":[{"key":"id","value":"a/b"}],"query":[{"key":"q","value":"a b","disabled":false}]},"header":[{"key":"X-Repeat","value":"a"},{"key":"X-Repeat","value":"b","description":{"content":"note","type":"text/plain"}}],"body":{"mode":"formdata","formdata":[{"key":"upload","type":"file","src":["one.bin","two.bin"],"contentType":"application/custom"},{"key":"text","type":"text","value":"literal"}]}},"event":[{"listen":"test","script":{"exec":["pm.test('ok',()=>pm.expect(true).to.be.true)"]}}],"response":[{"name":"Created","code":201,"body":"{\"id\":7}","header":[]}]}]},{"name":"Last","request":{"url":"https://example.test/last"}}]});
    let mut original = import::json(&serde_json::to_vec(&source).unwrap())
        .unwrap()
        .collection
        .unwrap();
    let r = &mut original.folders[0].requests[0];
    r.settings.proxy = "http://localhost:8080".into();
    r.checks.push(Assertion::default());
    r.extractors.push(Extractor {
        variable: "id".into(),
        pointer: "/id".into(),
    });
    let binary = ApiResponse {
        status: 200,
        raw_body: vec![0, 255, 128],
        body: "binary".into(),
        ..Default::default()
    };
    r.examples
        .push(RequestExample::capture("Binary".into(), r, &binary).unwrap());
    let exported = export::collection(&original, true).unwrap();
    assert!(!exported.warnings.is_empty());
    let imported = import::json(&serde_json::to_vec(&exported.document).unwrap()).unwrap();
    assert!(!imported.requires_confirmation());
    let copy = imported.collection.unwrap();
    assert_eq!(
        copy.ordered_requests(None)
            .iter()
            .map(|r| r.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Send", "Last"]
    );
    let a = &original.folders[0].requests[0];
    let b = &copy.folders[0].requests[0];
    assert_eq!(
        serde_json::to_value(&a.form_data).unwrap(),
        serde_json::to_value(&b.form_data).unwrap()
    );
    assert_eq!(a.url, b.url);
    assert_eq!(
        a.headers[1].description_metadata,
        b.headers[1].description_metadata
    );
    assert_eq!(a.settings, b.settings);
    assert_eq!(b.checks.len(), 1);
    assert_eq!(b.extractors[0].pointer, "/id");
    assert_eq!(a.post_script, b.post_script);
    assert_eq!(
        b.examples[1].response().unwrap().raw_body,
        vec![0, 255, 128]
    );
    assert_eq!(copy.variable_metadata[1].value, "disabled");
    assert!(!copy.variable_metadata[1].enabled);
    let again = export::collection(&copy, true).unwrap();
    assert_eq!(exported.document, again.document);
}
#[test]
fn environment_cli_export_masks_typed_secrets_and_preserves_disabled_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("environment.json");
    let destination = dir.path().join("exported.json");
    let environment = Environment {
        variables: vec![
            KeyValue {
                value_type: "secret".into(),
                ..KeyValue::new("credential", "private-typed")
            },
            KeyValue {
                enabled: false,
                description: "kept".into(),
                ..KeyValue::new("off", "disabled")
            },
        ],
        ..Default::default()
    };
    std::fs::write(&source, serde_json::to_vec(&environment).unwrap()).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_rustapi"))
        .args([
            "export",
            source.to_str().unwrap(),
            destination.to_str().unwrap(),
            "--format",
            "environment",
            "--workspace",
            dir.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = std::fs::read(&destination).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("private-typed"));
    let copy = import::json(&bytes).unwrap().environment.unwrap();
    assert_eq!(copy.variables[1].description, "kept");
    assert!(!copy.variables[1].enabled);
    let original: Value = serde_json::from_slice(&std::fs::read(&source).unwrap()).unwrap();
    assert_eq!(original["variables"][0]["value"], "private-typed");
}
#[test]
fn native_extension_tls_override_requires_review() {
    let mut c = Collection::new("Unsafe");
    c.requests.push(ApiRequest {
        url: "https://example.test".into(),
        ..Default::default()
    });
    let mut exported = export::collection(&c, true).unwrap();
    exported.document["item"][0]["request"]["x-rustapi"]["settings"]["verify_tls"] = false.into();
    assert!(
        import::json(&serde_json::to_vec(&exported.document).unwrap())
            .unwrap()
            .requires_confirmation()
    );
}
