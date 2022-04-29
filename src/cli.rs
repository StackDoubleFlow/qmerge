use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[clap(version)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile mod in working directory
    Build,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();

    Ok(())
}
