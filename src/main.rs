use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

use clap::{Parser, Subcommand};

mod inspector;

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
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Commands::Inspect { path } => inspect(&path),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("tabguard: {error}");
            ExitCode::from(2)
        }
    }
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
