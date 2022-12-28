#![feature(once_cell)]

mod adb;
mod build;
mod cli;
mod config;
mod manifest;
mod package;
mod utils;

use color_eyre::Result;

fn main() -> Result<()> {
    cli::run()
}
