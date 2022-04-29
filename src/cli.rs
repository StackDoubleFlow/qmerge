use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::build;

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
    /// Upload the generated mod files
    Upload,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Build => build::build(),
        Commands::Upload => todo!(),
    }

    Ok(())
}
