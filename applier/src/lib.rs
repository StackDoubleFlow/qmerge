#![feature(once_cell, backtrace)]

mod modloader;
mod setup;
mod xref;

use std::path::PathBuf;
use tracing::info;

fn get_mod_data_path() -> PathBuf {
    // TODO
    PathBuf::from("/sdcard/ModData/com.beatgames.beatsaber/Mods/QMerge")
}

#[no_mangle]
pub extern "C" fn setup() {
    setup::setup(env!("CARGO_PKG_NAME"));
    info!("merge applier is setting up");

    let xd = xref::get_symbol("_ZN6il2cpp2vm14MetadataLoader16LoadMetadataFileEPKc").unwrap();
}
