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
    Build {
        #[clap(long)]
        regen_cpp: bool,
    },
    /// Upload the generated mod files
    Upload,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Build { regen_cpp } => build::build(regen_cpp)?,
        Commands::Upload => todo!(),
    }

    Ok(())
}
