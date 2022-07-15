use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use crate::config::Config;
use crate::{adb, build, package};

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
    /// Package the plugin into a qmod file
    Package,
    /// Upload your mod and start the game, and begin logging to `test.log`
    Run,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();

    let mut config = Config::load().context("error loading configuration")?;

    match cli.command {
        Commands::Build { regen_cpp } => build::build(regen_cpp, &mut config)?,
        Commands::Upload => adb::upload(&mut config)?,
        Commands::Package => package::build_package(&mut config)?,
        Commands::Run => {
            adb::upload(&mut config)?;
            adb::start_and_log(&mut config)?;
        }
    }

    Ok(())
}
