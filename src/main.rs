#![feature(once_cell)]

mod build;
mod cli;
mod config;
mod error;
mod upload;

use crate::error::exit_on_err;

fn main() {
    exit_on_err(cli::run());
}
