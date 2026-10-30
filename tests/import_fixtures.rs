use rustapi::{import, models::BodyType};
use std::process::Command;

const BODIES: &[u8] = include_bytes!("fixtures/import/body-modes.json");
const ENVIRONMENT: &[u8] = include_bytes!("fixtures/import/environment.json");
const METADATA: &[u8] = include_bytes!("fixtures/import/metadata-gaps.json");

#[test]
fn request_folder_descriptions_survive_storage_duplication_and_legacy_loading() {
    let imported = import::json(METADATA).unwrap();
    let original = imported.collection.unwrap();
    assert_eq!(original.folders[0].description, "Folder documentation");
    assert_eq!(
        original.folders[0].requests[0].description,
        "Request documentation"
    );
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("native.json");
    rustapi::storage::export_collection(&original, &path).unwrap();
    let saved = rustapi::storage::load_collection(&path).unwrap();
    let copy = import::json(&std::fs::read(path).unwrap())
        .unwrap()
        .collection
        .unwrap();
    for collection in [saved, copy] {
        assert_eq!(collection.folders[0].description, "Folder documentation");
        let request = &collection.folders[0].requests[0];
        assert_eq!(request.description, "Request documentation");
        assert_eq!(request.duplicate().description, request.description);
    }
    let mut legacy = serde_json::to_value(original).unwrap();
    legacy["folders"][0]
        .as_object_mut()
        .unwrap()
        .remove("description");
    legacy["folders"][0]["requests"][0]
        .as_object_mut()
        .unwrap()
        .remove("description");
    let legacy = import::json(&serde_json::to_vec(&legacy).unwrap())
        .unwrap()
        .collection
        .unwrap();
    assert!(legacy.folders[0].description.is_empty());
    assert!(legacy.folders[0].requests[0].description.is_empty());
}

#[test]
fn row_descriptions_survive_cli_import_storage_and_native_collection_reimport() {
    let mut collection: serde_json::Value = serde_json::from_slice(BODIES).unwrap();
    collection["item"][0]["request"]["header"][0]["description"] =
        "Accepted format α\nSecond line".into();
    collection["item"][0]["request"]["header"][1]["description"] = "Disabled header".into();
    collection["item"][0]["request"]["url"] = serde_json::json!({
        "raw":"https://example.test/items?limit=10",
        "query":[{"key":"limit","value":"10","description":"Page size"}]
    });
    collection["item"][2]["request"]["body"]["urlencoded"][0]["description"] =
        "Display name".into();
    collection["item"][2]["request"]["body"]["urlencoded"][1]["description"] =
        "Disabled field".into();
    collection["item"][3]["request"]["body"]["formdata"][0]["description"] = "Upload label".into();
    collection["item"][3]["request"]["body"]["formdata"][1]["description"] =
        "File description".into();
    let mut environment: serde_json::Value = serde_json::from_slice(ENVIRONMENT).unwrap();
    environment["values"][0]["description"] = "Base endpoint β\nSecond line".into();
    environment["values"][2]["description"] = "Disabled variable".into();

    for (document, folder) in [(collection, "collections"), (environment, "environments")] {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.json");
        let workspace = directory.path().join("workspace");
        let original = serde_json::to_vec(&document).unwrap();
        std::fs::write(&source, &original).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_rustapi"))
            .arg("import")
            .arg(&source)
            .arg("--workspace")
            .arg(&workspace)
            .arg("--import-report")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["requires_confirmation"], false);
        assert_eq!(report["diagnostics"], serde_json::json!([]));
        assert_eq!(std::fs::read(source).unwrap(), original);
        let files: Vec<_> = std::fs::read_dir(workspace.join(folder)).unwrap().collect();
        assert_eq!(files.len(), 1);
        let path = files[0].as_ref().unwrap().path();
        if folder == "collections" {
            let saved = rustapi::storage::load_collection(&path).unwrap();
            let copy = import::json(&serde_json::to_vec(&saved).unwrap()).unwrap();
            assert!(!copy.requires_confirmation());
            for collection in [saved, copy.collection.unwrap()] {
                let requests = &collection.requests;
                assert_eq!(
                    requests[0].headers[0].description,
                    "Accepted format α\nSecond line"
                );
                assert_eq!(requests[0].headers[1].description, "Disabled header");
                assert!(!requests[0].headers[1].enabled);
                assert_eq!(requests[0].query_params[0].description, "Page size");
                assert_eq!(requests[2].form_data[0].description, "Display name");
                assert_eq!(requests[2].form_data[1].description, "Disabled field");
                assert!(!requests[2].form_data[1].enabled);
                assert_eq!(requests[3].form_data[0].description, "Upload label");
                assert_eq!(requests[3].form_data[1].description, "File description");
                assert!(requests[3].form_data[1].file);
                assert_eq!(requests[3].form_data[1].value, "files/sample.bin");
            }
        } else {
            let saved = rustapi::storage::load_environment(&path).unwrap();
            assert_eq!(
                saved.variables[0].description,
                "Base endpoint β\nSecond line"
            );
            assert_eq!(saved.variables[1].description, "");
            assert_eq!(saved.variables[2].description, "Disabled variable");
            assert!(!saved.variables[2].enabled);
        }
    }
}

#[test]
fn scope_variable_and_structured_row_descriptions_are_preserved() {
    let mut source: serde_json::Value = serde_json::from_slice(METADATA).unwrap();
    source["variable"] = serde_json::json!([{"key":"base","value":"https://example.test","description":"Collection note","type":"string"}]);
    source["item"][0]["variable"] = serde_json::json!([{"key":"limit","value":"10","description":"Folder note","disabled":true}]);
    source["item"][0]["item"][0]["request"]["header"][0]["description"] =
        serde_json::json!({"content":"Structured private note","type":"text/markdown"});
    let imported = import::json(&serde_json::to_vec(&source).unwrap()).unwrap();
    assert!(!imported.diagnostics.iter().any(|d| matches!(
        d.feature.as_str(),
        "row_description" | "variable_type" | "disabled_scope_variable"
    )));
    let collection = imported.collection.as_ref().unwrap();
    assert_eq!(
        collection.variable_metadata[0].description,
        "Collection note"
    );
    assert_eq!(collection.variable_metadata[0].value_type, "string");
    assert!(!collection.folders[0].variable_metadata[0].enabled);
    let header = &collection.folders[0].requests[0].headers[0];
    assert_eq!(header.description, "Structured private note");
    assert_eq!(
        rustapi::models::encode_description(&header.description, &header.description_metadata),
        source["item"][0]["item"][0]["request"]["header"][0]["description"]
    );
    let report = serde_json::to_string(&imported.report()).unwrap();
    for note in ["Collection note", "Folder note", "Structured private note"] {
        assert!(!report.contains(note));
    }
}

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
            "/item/0/item/0/protocolProfileBehavior",
            "/item/0/item/0/response",
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
    assert!(!imported.requires_confirmation());
    assert!(imported.diagnostics.is_empty());
    assert_eq!(
        imported.environment.as_ref().unwrap().variables[0].value_type,
        "secret"
    );
    let report = serde_json::to_string(&imported.report()).unwrap();
    assert!(!report.contains("sensitive-fixture-value"));
    assert!(!report.contains("private-note"));
    let mut value: serde_json::Value = serde_json::from_slice(BODIES).unwrap();
    value["item"][0]["request"]["header"] = serde_json::json!(vec![
        serde_json::json!({"key":"X-Test","value":"private-value","description":{"content":false}});
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
        assert_eq!(report["diagnostics"].as_array().unwrap().len(), 4);
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
    value["values"][0]["description"] = serde_json::json!({"content":false});
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
    value["values"][0]["description"] = serde_json::json!({"content":false});
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
    assert!(String::from_utf8_lossy(&output.stderr).contains("/values/0/description"));
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
