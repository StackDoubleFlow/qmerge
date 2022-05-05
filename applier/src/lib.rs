#![feature(once_cell, backtrace)]

mod modloader;
mod setup;
mod xref;
mod codegen_api;
mod types;

use inline_hook::Hook;
use std::lazy::SyncLazy;
use std::mem::transmute;
use std::path::PathBuf;
use tracing::info;

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
pub extern "C" fn load_metadata(file_name: *const u8) -> *const () {
    let original_ptr = LOAD_METADATA_HOOK.original().unwrap();
    let original_fn = unsafe { transmute::<_, fn(*const u8) -> *const ()>(original_ptr) };

    let original_metadata = original_fn(file_name);
    info!("hook was called");
    original_metadata
}

#[no_mangle]
pub extern "C" fn setup() {
    setup::setup(env!("CARGO_PKG_NAME"));
    info!("merge applier is setting up");
    SyncLazy::force(&LOAD_METADATA_HOOK);
}
