#![feature(once_cell)]

mod cli;
mod config;
mod error;
mod build;

use crate::error::exit_on_err;

fn main() {
    exit_on_err(cli::run());

    dbg!(&*config::CONFIG);
    dbg!(&*config::APPS);
}
