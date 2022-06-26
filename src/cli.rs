use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::{build, upload};

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
        input_dir: String,
    },
    /// Upload the generated mod files
    Upload,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Build { regen_cpp, input_dir } => build::build(regen_cpp, input_dir)?,
        Commands::Upload => upload::upload()?,
    }

    Ok(())
}
