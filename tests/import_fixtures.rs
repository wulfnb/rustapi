use rustapi::{import, models::BodyType};
use std::process::Command;

const BODIES: &[u8] = include_bytes!("fixtures/import/body-modes.json");
const ENVIRONMENT: &[u8] = include_bytes!("fixtures/import/environment.json");
const METADATA: &[u8] = include_bytes!("fixtures/import/metadata-gaps.json");

#[test]
fn known_omissions_report_exact_source_locations_without_values() {
    let source: serde_json::Value = serde_json::from_slice(METADATA).unwrap();
    let imported = import::json(METADATA).unwrap();
    assert!(imported.requires_confirmation());
    let paths: std::collections::BTreeSet<_> = imported
        .diagnostics
        .iter()
        .map(|d| d.path.as_str())
        .collect();
    assert_eq!(
        paths,
        [
            "/item/0/description",
            "/item/0/item/0/protocolProfileBehavior",
            "/item/0/item/0/response",
            "/item/0/item/0/request/description",
            "/item/0/item/0/request/url/variable",
            "/item/0/item/0/request/header/0/description",
            "/item/0/item/0/request/url/query/0/description",
            "/item/0/item/0/request/body/formdata/0/description",
            "/item/0/item/0/request/body/formdata/0/contentType",
            "/item/0/item/0/request/body/formdata/0/src",
        ]
        .into_iter()
        .collect()
    );
    for path in paths {
        assert!(source.pointer(path).is_some(), "{path}");
    }
    let report = serde_json::to_string(&imported.report()).unwrap();
    for private in [
        "never-log-example-body",
        "42",
        "first.bin",
        "Request documentation",
        "Metadata fixture",
    ] {
        assert!(!report.contains(private), "{private}");
    }
    let report: serde_json::Value = serde_json::from_str(&report).unwrap();
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["requires_confirmation"], true);
    assert!(imported
        .diagnostics
        .iter()
        .any(|d| d.disposition == import::ImportDisposition::PartiallyPreserved));
}

#[test]
fn supported_imports_do_not_need_loss_confirmation() {
    for input in [
        BODIES,
        ENVIRONMENT,
        include_bytes!("fixtures/collection-v2.1.json").as_slice(),
    ] {
        assert!(!import::json(input).unwrap().requires_confirmation());
    }
    assert!(!import::curl("curl https://example.test")
        .unwrap()
        .requires_confirmation());
    let native = serde_json::to_vec(&rustapi::models::Collection::new("Native")).unwrap();
    assert!(!import::json(&native).unwrap().requires_confirmation());
}

#[test]
fn environment_type_metadata_and_report_limits_are_visible() {
    let environment = br#"{"name":"Local","values":[{"key":"credential","value":"sensitive-fixture-value","type":"secret","description":"private-note"}]}"#;
    let imported = import::json(environment).unwrap();
    assert!(imported.requires_confirmation());
    assert_eq!(imported.diagnostics.len(), 2);
    let report = serde_json::to_string(&imported.report()).unwrap();
    assert!(!report.contains("sensitive-fixture-value"));
    assert!(!report.contains("private-note"));
    let mut value: serde_json::Value = serde_json::from_slice(BODIES).unwrap();
    value["item"][0]["request"]["header"] = serde_json::json!(vec![
        serde_json::json!({"key":"X-Test","value":"private-value","description":"private-note"});
        600
    ]);
    let imported = import::json(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(imported.requires_confirmation());
    assert_eq!(imported.diagnostics.len(), 257);
    assert_eq!(imported.diagnostics.last().unwrap().feature, "report_limit");
}

#[test]
fn cli_requires_explicit_loss_acceptance_and_emits_json_report() {
    for allow in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.json");
        let workspace = directory.path().join("workspace");
        std::fs::write(&source, METADATA).unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_rustapi"));
        command
            .arg("import")
            .arg(&source)
            .arg("--workspace")
            .arg(&workspace)
            .arg("--import-report");
        if allow {
            command.arg("--allow-lossy-import");
        }
        let output = command.output().unwrap();
        assert_eq!(
            output.status.code(),
            Some(if allow { 0 } else { 2 }),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["requires_confirmation"], true);
        assert_eq!(report["diagnostics"].as_array().unwrap().len(), 10);
        assert_eq!(
            std::fs::read_dir(workspace.join("collections"))
                .unwrap()
                .count(),
            usize::from(allow)
        );
        assert_eq!(std::fs::read(&source).unwrap(), METADATA);
        assert!(!String::from_utf8_lossy(&output.stdout).contains("never-log-example-body"));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("never-log-example-body"));
    }
}

#[test]
fn cli_run_and_environment_import_apply_the_same_omission_policy() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.json");
    let workspace = directory.path().join("workspace");
    std::fs::write(&source, METADATA).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rustapi"))
        .arg("run")
        .arg(&source)
        .arg("--workspace")
        .arg(&workspace)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("--allow-lossy-import"));
    let mut value: serde_json::Value = serde_json::from_slice(ENVIRONMENT).unwrap();
    value["values"][0]["description"] = "Environment description".into();
    std::fs::write(&source, serde_json::to_vec(&value).unwrap()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rustapi"))
        .arg("import")
        .arg(&source)
        .arg("--workspace")
        .arg(&workspace)
        .arg("--import-report")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        std::fs::read_dir(workspace.join("environments"))
            .unwrap()
            .count(),
        0
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["diagnostics"][0]["path"], "/values/0/description");
}

#[test]
fn cli_run_reviews_environment_before_executing_requests() {
    let directory = tempfile::tempdir().unwrap();
    let environment = directory.path().join("environment.json");
    let collection = directory.path().join("collection.json");
    let mut value: serde_json::Value = serde_json::from_slice(ENVIRONMENT).unwrap();
    value["values"][0]["type"] = "secret".into();
    std::fs::write(&environment, serde_json::to_vec(&value).unwrap()).unwrap();
    let mut native = rustapi::models::Collection::new("Run fixture");
    native.requests.push(rustapi::models::ApiRequest {
        url: "http://127.0.0.1:1".into(),
        ..Default::default()
    });
    std::fs::write(&collection, serde_json::to_vec(&native).unwrap()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rustapi"))
        .arg("run")
        .arg(collection)
        .arg("--environment")
        .arg(environment)
        .arg("--workspace")
        .arg(directory.path().join("workspace"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("/values/0/type"));
    assert!(output.stdout.is_empty());
}

#[test]
fn body_mode_fixtures_preserve_supported_request_data() {
    let collection = import::json(BODIES).unwrap().collection.unwrap();
    let requests = collection.ordered_requests(None);
    assert_eq!(
        requests.iter().map(|r| &r.body_type).collect::<Vec<_>>(),
        [
            BodyType::Json,
            BodyType::Text,
            BodyType::UrlEncoded,
            BodyType::FormData,
            BodyType::Binary,
            BodyType::None
        ]
        .iter()
        .collect::<Vec<_>>()
    );
    assert_eq!(requests[0].body, r#"{"ok":true,"count":2}"#);
    assert_eq!(requests[1].body, "hello\nworld");
    assert_eq!(requests[2].form_data[0].value, "Ada Lovelace");
    assert!(!requests[2].form_data[1].enabled);
    assert_eq!(requests[3].form_data[0].value, "fixture");
    assert!(requests[3].form_data[1].file);
    assert_eq!(requests[3].form_data[1].value, "files/sample.bin");
    assert_eq!(requests[4].binary_file, "files/sample.bin");
    for request in requests {
        assert_eq!(request.url, "https://example.test/items");
        assert_eq!(request.headers[0].value, "application/json");
        assert!(!request.headers[1].enabled);
    }
}

#[test]
fn environment_fixture_preserves_values_and_disabled_flags() {
    let environment = import::json(ENVIRONMENT).unwrap().environment.unwrap();
    assert_eq!(environment.get("base"), Some("https://example.test"));
    assert_eq!(environment.get("count"), Some("2"));
    assert_eq!(environment.get("off"), None);
    assert_eq!(environment.get("also_off"), None);
    assert_eq!(environment.variables.len(), 4);
}

#[test]
fn cli_import_copies_supported_fixtures_without_modifying_sources() {
    for (bytes, folder) in [(BODIES, "collections"), (ENVIRONMENT, "environments")] {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.json");
        let workspace = directory.path().join("workspace");
        std::fs::write(&source, bytes).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_rustapi"))
            .args([
                "import",
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
        assert_eq!(std::fs::read(&source).unwrap(), bytes);
        let files: Vec<_> = std::fs::read_dir(workspace.join(folder)).unwrap().collect();
        assert_eq!(files.len(), 1);
        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(files[0].as_ref().unwrap().path()).unwrap())
                .unwrap();
        assert!(saved["name"].is_string());
    }
}

#[test]
fn cli_rejects_unsupported_inputs_without_saving_project_data() {
    let mut inputs = vec![include_bytes!("fixtures/import/unsupported-v3.yaml").to_vec()];
    for version in ["v1.0.0", "v2.0.0", "v3.0.0"] {
        let mut value: serde_json::Value = serde_json::from_slice(BODIES).unwrap();
        value["info"]["schema"] =
            format!("https://example.test/collection/{version}/schema.json").into();
        inputs.push(serde_json::to_vec(&value).unwrap());
    }
    for mode in ["graphql", "unknown"] {
        let mut value: serde_json::Value = serde_json::from_slice(BODIES).unwrap();
        value["item"][0]["request"]["body"]["mode"] = mode.into();
        inputs.push(serde_json::to_vec(&value).unwrap());
    }
    for bytes in inputs {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.json");
        let workspace = directory.path().join("workspace");
        std::fs::write(&source, &bytes).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_rustapi"))
            .args([
                "import",
                source.to_str().unwrap(),
                "--workspace",
                workspace.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(2),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(std::fs::read(source).unwrap(), bytes);
        for folder in ["collections", "environments"] {
            assert_eq!(
                std::fs::read_dir(workspace.join(folder)).unwrap().count(),
                0
            );
        }
    }
}
