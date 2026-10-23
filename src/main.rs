use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

use clap::{Parser, Subcommand};

mod contract;
mod inspector;
mod validator;

/// Validate CSV dataset against declarative data-quality contracts.
#[derive(Debug, Parser)]
#[command(version, about, long_about = None, arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Inspect the structure of a CSV dataset.
    Inspect {
        /// Path to the input CSV file.
        #[arg(value_name = "CSV_PATH")]
        path: PathBuf,
    },
    /// Validate the content of the CSV dataset.
    Validate {
        /// Path to the input CSV file.
        #[arg(value_name = "CSV_PATH")]
        path: PathBuf,

        /// Path to the contract file.
        #[arg(long, value_name = "CONTRACT_PATH")]
        contract: PathBuf,
    },
}

fn inspect(path: &Path) -> Result<(), inspector::InspectionError> {
    let report = inspector::inspect_csv(path)?;
    println!("File: {}", path.display());
    println!("Columns: {}", report.headers.len());

    if report.headers.is_empty() {
        println!("Headers: (none, input contains no header record)");
    } else {
        println!("Headers:");
        for (index, name) in report.headers.iter().enumerate() {
            println!("  {}. {}", index + 1, name);
        }
    }

    println!("Data records: {}", report.data_records);

    if !report.duplicate_headers.is_empty() {
        println!("Duplicate headers:");
        for duplicate in &report.duplicate_headers {
            println!(
                " {:?}: columns {} and {}",
                duplicate.name, duplicate.first_column, duplicate.duplicate_column
            );
        }
    }

    Ok(())
}

fn validate(path: &Path, contract_path: &Path) -> ExitCode {
    let report = match validator::validate_csv(path, contract_path) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("tabguard: {error}");
            return ExitCode::from(2);
        }
    };

    let passed = report.missing_columns.is_empty()
        && report.duplicate_headers.is_empty()
        && report.non_empty_violations.is_empty();

    println!("Validation: {}", if passed { "PASS" } else { "FAIL" });
    println!("Contract: {}", report.contract_name);
    println!(
        "Required columns present: {}/{}",
        report.present_required_columns, report.required_columns
    );
    println!("Data records: {}", report.data_records);

    if !report.duplicate_headers.is_empty() {
        println!("Duplicate headers:");

        for duplicate in &report.duplicate_headers {
            println!(
                "  {:?}: columns {} and {}",
                duplicate.name, duplicate.first_column, duplicate.duplicate_column
            );
        }
    }

    if !report.non_empty_violations.is_empty() {
        println!("Non-empty value violations:");

        for violation in &report.non_empty_violations {
            println!("  {}: {}", violation.column, violation.empty_values);
        }
    }

    if passed {
        return ExitCode::SUCCESS;
    }

    println!("Missing required columns:");

    for column in &report.missing_columns {
        println!("  - {column}");
    }

    ExitCode::from(1)
}

fn report_result<T, E: std::fmt::Display>(result: Result<T, E>) -> ExitCode {
    match result {
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("tabguard: {error}");
            ExitCode::from(2)
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Commands::Inspect { path } => report_result(inspect(&path)),
        Commands::Validate { path, contract } => validate(&path, &contract),
    }
}
