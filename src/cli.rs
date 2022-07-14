use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use crate::config::Config;
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
        regen_cpp: Option<String>,
    },
    /// Upload the generated mod files
    Upload,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();

    let mut config = Config::load().context("error loading configuration")?;

    match cli.command {
        Commands::Build { regen_cpp } => build::build(regen_cpp, &mut config)?,
        Commands::Upload => upload::upload(&mut config)?,
    }

    Ok(())
}
