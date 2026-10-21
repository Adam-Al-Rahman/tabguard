use std::{
    path::Path,
    process::{Command, Output},
};

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

#[test]
fn inspect_reports_headers_and_record_count_without_printing_rows() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/customers.csv");

    let output = Command::new(env!("CARGO_BIN_EXE_tabguard"))
        .arg("inspect")
        .arg(&path)
        .output()
        .expect("failed to execute Tabguard");

    assert!(
        output.status.success(),
        "inspect failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("Columns: 3"));
    assert!(stdout.contains("  1. customer_id"));
    assert!(stdout.contains("  2. email"));
    assert!(stdout.contains("  3. age"));
    assert!(stdout.contains("Data records: 3"));

    assert!(
        !stdout.contains("ada@example.test"),
        "inspect must not print row values"
    );
}

#[test]
fn inspect_reports_duplicate_headers_and_column_positions() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/duplicate_headers.csv");

    let output = Command::new(env!("CARGO_BIN_EXE_tabguard"))
        .arg("inspect")
        .arg(&path)
        .output()
        .expect("failed to execute Tabguard");

    assert!(
        output.status.success(),
        "inspect failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("Columns: 3"));
    assert!(stdout.contains("Data records: 1"));
    assert!(stdout.contains("Duplicate headers:"));

    assert!(
        stdout
            .lines()
            .any(|line| line == r#" "id": columns 1 and 3"#),
        "expected duplicate header with column positions, actual output:\n{stdout}"
    );

    assert!(
        !stdout.contains("ada@example.test"),
        "insepct must not print row values"
    );
}

#[test]
fn inspect_handles_an_empty_file() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/empty.csv");

    let output = Command::new(env!("CARGO_BIN_EXE_tabguard"))
        .arg("inspect")
        .arg(&path)
        .output()
        .expect("failed to execute Tabguard");

    assert!(
        output.status.success(),
        "inpsect failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("Columns: 0"));
    assert!(stdout.contains("Headers: (none, input contains no header record)"));
    assert!(stdout.contains("Data records: 0"));
}

#[test]
fn inspect_reports_headers_for_headers_only_csv() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/header_only.csv");

    let output = Command::new(env!("CARGO_BIN_EXE_tabguard"))
        .arg("inspect")
        .arg(&path)
        .output()
        .expect("failed to execute Tabguard");

    assert!(
        output.status.success(),
        "inspect failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("Columns: 3"));
    assert!(stdout.contains("  1. customer_id"));
    assert!(stdout.contains("  2. email"));
    assert!(stdout.contains("  3. age"));
    assert!(stdout.contains("Data records: 0"));
}

#[test]
fn inspect_rejects_records_with_inconsistent_field_counts() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/inconsistent_records.csv");

    let output = Command::new(env!("CARGO_BIN_EXE_tabguard"))
        .arg("inspect")
        .arg(&path)
        .output()
        .expect("failed to execute Tabguard");

    assert_eq!(
        output.status.code(),
        Some(2),
        "expected an input-processing error"
    );

    assert!(
        output.stdout.is_empty(),
        "failed inspection must not print a successful report"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stderr.contains("tabguard:"),
        "expected an actionable error message; actual stderr:\n{stderr}"
    );

    assert!(
        stderr.contains("inconsistent_records.csv"),
        "error should identify the input file"
    );

    assert!(
        !stderr.contains("ada@example.test"),
        "error diagnostics must not expose cell values"
    );
}

#[test]
fn inspect_counts_multiline_quoted_fields_as_one_record() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/multiline_records.csv");

    let output = Command::new(env!("CARGO_BIN_EXE_tabguard"))
        .arg("inspect")
        .arg(&path)
        .output()
        .expect("failed to execute Tabguard");

    assert!(
        output.status.success(),
        "inspect failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("Columns: 2"));
    assert!(stdout.contains("Data records: 2"));

    assert!(
        !stdout.contains("first line"),
        "inspect must not print cell values"
    );

    assert!(
        !stdout.contains("ordinary note"),
        "inspect must not print cell values"
    );
}

#[test]
fn inspect_reports_a_missing_file_as_an_operational_error() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/missing.csv");

    assert!(
        !path.exists(),
        "the missing-file test fixture must not exist"
    );

    let output = Command::new(env!("CARGO_BIN_EXE_tabguard"))
        .arg("inspect")
        .arg(&path)
        .output()
        .expect("failed to execute Tabguard");

    assert_eq!(
        output.status.code(),
        Some(2),
        "expected exit code 2 for a missing input file"
    );

    assert!(
        output.stdout.is_empty(),
        "a failed inspection must not print a successful report"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stderr.contains("tabguard:"),
        "expected a contextual error; actual stderr:\n{stderr}"
    );

    assert!(
        stderr.contains("missing.csv"),
        "error should identify the input file; actual stderr:\n{stderr}"
    );
}

#[test]
fn inspect_rejects_an_unterminated_quoted_field() {
    let output = run_tabguard(&["inspect", "tests/fixtures/unterminated_quote.csv"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("tabguard:"));
    assert!(stderr.contains("unterminated_quote.csv"));
}

#[test]
fn inspect_accepts_escaped_quotes_inside_quoted_fields() {
    let output = run_tabguard(&["inspect", "tests/fixtures/escaped_quotes.csv"]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Data records: 1"));
    assert!(output.stderr.is_empty());
}

#[test]
fn inspect_rejects_a_quote_inside_an_unquoted_field() {
    let output = run_tabguard(&["inspect", "tests/fixtures/quote_in_unquoted_field.csv"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("tabguard:"));
    assert!(stderr.contains("quote_in_unquoted_field.csv"));
}

#[test]
fn inspect_rejects_characters_after_a_closing_quote() {
    let output = run_tabguard(&[
        "inspect",
        "tests/fixtures/characters_after_closing_quote.csv",
    ]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("tabguard:"));
    assert!(stderr.contains("characters_after_closing_quote.csv"));
}

#[test]
fn validate_passes_when_required_columns_are_present() {
    let output = run_tabguard(&[
        "validate",
        "tests/fixtures/customers.csv",
        "--contract",
        "tests/fixtures/contracts/customers.toml",
    ]);

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(output.status.code(), Some(0));
    assert!(stdout.contains("Validation: PASS"));
    assert!(!stdout.contains("ada@example.test"));
    assert!(output.stderr.is_empty());
}

#[test]
fn validate_fails_when_a_required_column_is_missing() {
    let output = run_tabguard(&[
        "validate",
        "tests/fixtures/customers.csv",
        "--contract",
        "tests/fixtures/contracts/customers_requires_phone.toml",
    ]);

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("Validation: FAIL"));
    assert!(stdout.contains("Required columns present: 2/3"));
    assert!(stdout.contains("Missing required columns:"));
    assert!(stdout.contains("phone"));

    assert!(
        !stdout.contains("ada@example.test"),
        "validation must not print individual cell values"
    );

    assert!(output.stderr.is_empty());
}

#[test]
fn validate_exits_with_code_two_when_contract_is_missing() {
    let output = run_tabguard(&[
        "validate",
        "tests/fixtures/customers.csv",
        "--contract",
        "tests/fixtures/contracts/missing.toml",
    ]);

    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "a validation report must not be printed when the contract cannot be loaded",
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("tabguard:"));
    assert!(stderr.contains("missing.toml"));
}

#[test]
fn validate_fails_when_non_empty_columns_contain_blank_values() {
    let output = run_tabguard(&[
        "validate",
        "tests/fixtures/completeness_empty.csv",
        "--contract",
        "tests/fixtures/contracts/customers_completeness.toml",
    ]);

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("Validation: FAIL"));
    assert!(stdout.contains("Non-empty value violations:"));
    assert!(stdout.contains("email: 2"));
    assert!(stdout.contains("Data records: 3"));

    assert!(
        !stdout.contains("ada@example.test"),
        "validation reports must not print cell values"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn validate_passes_when_non_empty_columns_are_populated() {
    let output = run_tabguard(&[
        "validate",
        "tests/fixtures/customers.csv",
        "--contract",
        "tests/fixtures/contracts/customers_completeness.toml",
    ]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Validation: PASS"));
    assert!(output.stderr.is_empty());
}

#[test]
fn validate_fails_when_csv_contains_duplicate_headers() {
    let output = run_tabguard(&[
        "validate",
        "tests/fixtures/duplicate_headers.csv",
        "--contract",
        "tests/fixtures/contracts/duplicate_header_completeness.toml",
    ]);

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("Validation: FAIL"));
    assert!(stdout.contains("Duplicate headers:"));
    assert!(stdout.contains("columns 1 and 3"));
    assert!(output.stderr.is_empty());
}
