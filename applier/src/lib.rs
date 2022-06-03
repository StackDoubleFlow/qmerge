#![feature(once_cell, backtrace)]

mod codegen_api;
pub mod il2cpp_types;
// mod modloader;
mod metadata_builder;
mod setup;
mod types;
mod xref;

use anyhow::Result;
use il2cpp_metadata_raw::Metadata;
use inline_hook::Hook;
// use modloader::ModLoader;
use std::fs;
use std::lazy::SyncLazy;
use std::mem::transmute;
use std::path::PathBuf;
use tracing::info;
use crate::metadata_builder::MetadataBuilder;

fn get_mod_data_path() -> PathBuf {
    // TODO
    PathBuf::from("/sdcard/ModData/com.beatgames.beatsaber/Mods/QMerge")
}

static LOAD_METADATA_HOOK: SyncLazy<Hook> = SyncLazy::new(|| {
    let addr = xref::get_symbol("_ZN6il2cpp2vm14MetadataLoader16LoadMetadataFileEPKc").unwrap();
    let hook = Hook::new();
    unsafe {
        hook.install(addr as _, load_metadata as _);
    }
    hook
});
pub extern "C" fn load_metadata(file_name: *const u8) -> *const u8 {
    let original_ptr = LOAD_METADATA_HOOK.original().unwrap();
    let original_fn = unsafe { transmute::<_, fn(*const u8) -> *const u8>(original_ptr) };

    let original_metadata = original_fn(file_name);
    let code_registration = xref::get_data_symbol("_ZL24s_Il2CppCodeRegistration").unwrap();
    let metadata_registration = xref::get_data_symbol("_ZL28s_Il2CppMetadataRegistration").unwrap();

    let metadata_builder =
        MetadataBuilder::new(code_registration, metadata_registration, original_metadata).unwrap();

    // TODO: Clean up original metadata
    metadata_builder.finish()
}

#[no_mangle]
pub extern "C" fn setup() {
    setup::setup(env!("CARGO_PKG_NAME"));
    info!("merge applier is setting up");
    SyncLazy::force(&LOAD_METADATA_HOOK);
}
