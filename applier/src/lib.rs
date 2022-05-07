#![feature(once_cell, backtrace)]

mod codegen_api;
mod modloader;
mod setup;
mod types;
mod xref;

use anyhow::Result;
use il2cpp_metadata_raw::Metadata;
use inline_hook::Hook;
use modloader::ModLoader;
use std::fs;
use std::lazy::SyncLazy;
use std::mem::transmute;
use std::path::PathBuf;
use tracing::info;

fn get_mod_data_path() -> PathBuf {
    // TODO
    PathBuf::from("/sdcard/ModData/com.beatgames.beatsaber/Mods/QMerge")
}

fn get_global_metadata_path() -> PathBuf {
    // TODO
    PathBuf::from(
        "/sdcard/Android/data/com.beatgames.beatsaber/files/il2cpp/Metadata/global-metadata.dat",
    )
}

static LOAD_METADATA_HOOK: SyncLazy<Hook> = SyncLazy::new(|| {
    let addr = xref::get_symbol("_ZN6il2cpp2vm14MetadataLoader16LoadMetadataFileEPKc").unwrap();
    let hook = Hook::new();
    unsafe {
        hook.install(addr as _, load_metadata as _);
    }
    hook
});
pub extern "C" fn load_metadata(file_name: *const u8) -> *const () {
    let original_ptr = LOAD_METADATA_HOOK.original().unwrap();
    let original_fn = unsafe { transmute::<_, fn(*const u8) -> *const ()>(original_ptr) };

    let original_metadata = original_fn(file_name);
    info!("hook was called");
    original_metadata
}

fn load_mods() -> Result<()> {
    let global_metadata_data = fs::read(get_global_metadata_path())?;
    let global_metadata = il2cpp_metadata_raw::deserialize(&global_metadata_data)?;
    let mod_loader = ModLoader::new(global_metadata, todo!())?;

    for entry in fs::read_dir(get_mod_data_path().join("Mods"))? {
        let mod_dir = entry?;
    }

    Ok(())
}

#[no_mangle]
pub extern "C" fn setup() {
    setup::setup(env!("CARGO_PKG_NAME"));
    info!("merge applier is setting up");
    SyncLazy::force(&LOAD_METADATA_HOOK);
}
