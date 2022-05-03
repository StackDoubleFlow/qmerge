#![feature(once_cell)]

mod build;
mod clang;
mod cli;
mod config;
mod data;
mod error;
mod modules;
mod type_definitions;

use crate::error::exit_on_err;

fn main() {
    exit_on_err(cli::run());
}
