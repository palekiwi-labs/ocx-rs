use std::process::Command;

fn ocx_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ocx"))
}

#[test]
fn test_ocx_help() {
    let output = ocx_cmd()
        .arg("--help")
        .output()
        .expect("Failed to execute ocx");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"));
}

#[test]
fn test_ocx_config_help() {
    let output = ocx_cmd()
        .args(["config", "--help"])
        .output()
        .expect("Failed to execute ocx config");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("config"));
}

#[test]
fn test_ocx_config_runs() {
    let output = ocx_cmd()
        .arg("config")
        .output()
        .expect("Failed to execute ocx config");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // For now, just verify it prints something
    assert!(!stdout.is_empty());
}
