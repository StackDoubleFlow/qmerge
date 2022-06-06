#![feature(once_cell, backtrace)]

mod codegen_api;
pub mod il2cpp_types;
mod metadata_builder;
mod modloader;
mod setup;
mod xref;

use anyhow::{anyhow, Context, Result};
use dlopen::raw::Library;
use inline_hook::Hook;
use merge_data::MergeModData;
use metadata_builder::{CodeRegistrationBuilder, Metadata, MetadataRegistrationBuilder};
use modloader::ModLoader;
use std::fs;
use std::lazy::SyncLazy;
use std::mem::transmute;
use std::path::PathBuf;
use tracing::{debug, info};

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

    let mut code_registration = unsafe { CodeRegistrationBuilder::from_raw(code_registration) };
    let mut metadata_registration =
        unsafe { MetadataRegistrationBuilder::from_raw(metadata_registration) };
    let mut metadata = unsafe { Metadata::from_raw(original_metadata) }.unwrap();

    load_mods(
        &mut metadata,
        &mut code_registration,
        &mut metadata_registration,
    )
    .unwrap();

    // TODO: Clean up original metadata
    code_registration.build();
    metadata_registration.build();
    metadata.build()
}

fn load_mods(
    metadata: &mut Metadata,
    code_registration: &mut CodeRegistrationBuilder,
    metadata_registration: &mut MetadataRegistrationBuilder,
) -> Result<()> {
    let mut modloader = ModLoader::new(metadata, code_registration, metadata_registration)?;

    for entry in fs::read_dir(get_mod_data_path().join("Mods"))? {
        let mod_dir = entry?;
        let id = mod_dir.file_name();
        info!("Loading mod {:?}", id);

        let mut file_path = mod_dir.path().join(&id);
        file_path.set_extension("mmd");
        let mmd = fs::read(&file_path).context("could not read mod data")?;
        let mmd = MergeModData::deserialize(&mmd).context("failed to deserialize mod data")?;
        debug!("{:?}", mmd);

        file_path.set_extension("so");
        let lib = Library::open(file_path).context("failed to open mod executable")?;

        // let so = fs::read(&file_path).context("could not read mod executable")?;
        modloader.load_mod(
            &id.into_string()
                .map_err(|str| anyhow!("{:?} is not a valid id", str))?,
            &mmd,
        )?;
    }

    Ok(())
}

#[no_mangle]
pub extern "C" fn setup() {
    setup::setup(env!("CARGO_PKG_NAME"));
    info!("merge applier is setting up");
    SyncLazy::force(&LOAD_METADATA_HOOK);
}
