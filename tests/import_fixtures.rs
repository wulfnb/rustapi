use rustapi::{import, models::BodyType};
use std::process::Command;

const BODIES: &[u8] = include_bytes!("fixtures/import/body-modes.json");
const ENVIRONMENT: &[u8] = include_bytes!("fixtures/import/environment.json");

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
