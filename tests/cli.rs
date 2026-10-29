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
#[cfg(target_os = "linux")]
fn missing_display_returns_a_startup_error_without_hanging() {
    let profile = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rustapi"))
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env("RUSTAPI_DATA_DIR", profile.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Error:"));
}
