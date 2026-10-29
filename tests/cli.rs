mod support;
use std::process::Command;

#[test]
fn version_is_available_without_a_display() {
    let output = Command::new(env!("CARGO_BIN_EXE_rustapi"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        concat!("RustAPI ", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn help_is_available_without_a_display() {
    let output = Command::new(env!("CARGO_BIN_EXE_rustapi"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("--smoke-test"));
}

#[test]
fn unknown_arguments_fail_without_opening_a_window() {
    let output = Command::new(env!("CARGO_BIN_EXE_rustapi"))
        .arg("--unknown")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8(output.stderr).unwrap().contains("Usage:"));
}

#[test]
fn headless_runs_emit_reports_and_failure_exit_codes() {
    use rustapi::models::*;
    let profile = tempfile::tempdir().unwrap();
    let file = profile.path().join("collection.json");
    let report = profile.path().join("result.json");
    let junit = profile.path().join("result.xml");
    let mut c = Collection::new("CLI fixture");
    let mut r = ApiRequest {
        url: "http://127.0.0.1:1".into(),
        ..Default::default()
    };
    r.settings.timeout_ms = 100;
    c.requests.push(r);
    std::fs::write(&file, serde_json::to_vec(&c).unwrap()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rustapi"))
        .arg("run")
        .arg(&file)
        .arg("--workspace")
        .arg(profile.path())
        .arg("--report-json")
        .arg(&report)
        .arg("--report-junit")
        .arg(&junit)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(report).unwrap()).unwrap();
    assert_eq!(json["entries"].as_array().unwrap().len(), 1);
    assert!(std::fs::read_to_string(junit)
        .unwrap()
        .contains("<failure>"));
    assert!(profile.path().join("rustapi.json").exists());
}
#[test]
fn invalid_import_is_transactional_and_bad_cli_returns_two() {
    let profile = tempfile::tempdir().unwrap();
    let source = profile.path().join("bad.json");
    std::fs::write(&source, "not json").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rustapi"))
        .arg("import")
        .arg(&source)
        .arg("--workspace")
        .arg(profile.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        std::fs::read_dir(profile.path().join("collections"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn headless_success_uses_the_shared_assertion_engine() {
    let server = support::Server::new(1, |_, _| support::Reply::json(r#"{"ok":true}"#));
    let profile = tempfile::tempdir().unwrap();
    let path = profile.path().join("c.json");
    let mut c = rustapi::models::Collection::new("CLI success");
    let mut r = rustapi::models::ApiRequest {
        url: server.url.clone(),
        ..Default::default()
    };
    r.checks.push(rustapi::models::Assertion::default());
    c.requests.push(r);
    std::fs::write(&path, serde_json::to_vec(&c).unwrap()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rustapi"))
        .arg("run")
        .arg(path)
        .env("RUSTAPI_DATA_DIR", profile.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: rustapi::runner::RunReport = serde_json::from_slice(&output.stdout).unwrap();
    assert!(report.passed());
    server.finish();
}
