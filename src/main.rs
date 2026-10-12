use clap::Parser;

/// Validate CSV datasets against declarative data-quality contracts.
#[derive(Parser)]
#[command(version, about, long_about = None, arg_required_else_help = true)]
struct Cli;

fn main() {
    let _cli = Cli::parse();
}
