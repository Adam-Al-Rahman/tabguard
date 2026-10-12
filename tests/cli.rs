use std::process::{Command, Output};

fn run_tabguard(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tabguard"))
        .args(args)
        .output()
        .expect("failed to executed Tabguard")
}

#[test]
fn help_displays_usage_and_available_options() {
    let output = run_tabguard(&["--help"]);

    assert!(
        output.status.success(),
        "help command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("Usage: tabguard"));
    assert!(stdout.contains("--help"));
    assert!(stdout.contains("--version"));
}

#[test]
fn version_displays_package_version() {
    let output = run_tabguard(&["--version"]);

    assert!(
        output.status.success(),
        "version command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let expected = format!("tabguard {}", env!("CARGO_PKG_VERSION"));
    let actual = String::from_utf8_lossy(&output.stdout);

    assert_eq!(actual.trim(), expected);
}

#[test]
fn invocation_without_arguments_displays_help_and_returns_usage_error() {
    let output = run_tabguard(&[]);
    assert_eq!(output.status.code(), Some(2));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stdout.contains("Usage: tabguard") || stderr.contains("Usage: tabguard"),
        "expected usage information in stdout or stderr"
    );
}

#[test]
fn unknown_argument_is_rejected() {
    let output = run_tabguard(&["--unknown"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--unknown"));
}
