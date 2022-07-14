#![feature(once_cell)]

mod build;
mod cli;
mod config;
mod error;
mod manifest;
mod package;
mod upload;
mod utils;

use crate::error::exit_on_err;

fn main() {
    exit_on_err(cli::run());
}
