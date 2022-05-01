#![feature(once_cell, backtrace)]

mod xref;
mod setup;

use std::path::PathBuf;

fn get_mod_data_path() -> PathBuf {
    // TODO
    PathBuf::from("/sdcard/ModData/com.beatgames.beatsaber/Mods/QMerge")
}

pub extern "C" fn setup() {
    setup::setup(env!("CARGO_PKG_NAME"));
}
