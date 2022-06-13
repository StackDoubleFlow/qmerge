#![feature(once_cell, backtrace)]
#![feature(native_link_modifiers_bundle)]
#![feature(naked_functions)]
#![feature(asm_sym)]


mod asm;
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
use modloader::{ModLoader, MODS};
use std::fs;
use std::lazy::SyncLazy;
use std::mem::transmute;
use std::path::PathBuf;
use tracing::{info, warn};

fn get_mod_data_path() -> PathBuf {
    // TODO
    PathBuf::from("/sdcard/ModData/com.beatgames.beatsaber/Mods/QMerge")
}

fn get_exec_path() -> PathBuf {
    // TODO
    PathBuf::from("/data/data/com.beatgames.beatsaber")
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

    let mut metadata = unsafe { Metadata::from_raw(original_metadata) }.unwrap();
    let mut code_registration = unsafe { CodeRegistrationBuilder::from_raw(code_registration) };
    xref::initialize_roots(&metadata, &code_registration).unwrap();

    let metadata_registration = xref::get_data_symbol("_ZL28s_Il2CppMetadataRegistration").unwrap();
    // The count saved in the Il2CppMetadataRegistration struct is seemingly incorrect, so we calculate our own
    let metadata_usages_count = metadata
        .metadata_usage_pairs
        .iter()
        .map(|pair| pair.destinationIndex)
        .max()
        .unwrap_or(0)
        + 1;
    let mut metadata_registration = unsafe {
        MetadataRegistrationBuilder::from_raw(metadata_registration, metadata_usages_count)
    };

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

        file_path.set_extension("so");
        let so_path = get_exec_path().join(file_path.file_name().unwrap());
        fs::copy(file_path, &so_path)?;
        let lib = Library::open(so_path).context("failed to open mod executable")?;

        // let so = fs::read(&file_path).context("could not read mod executable")?;
        modloader.load_mod(
            &id.into_string()
                .map_err(|str| anyhow!("{:?} is not a valid id", str))?,
            &mmd,
            lib,
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

fn call_plugin_loads() -> Result<()> {
    let ids: Vec<String> = MODS.lock().unwrap().keys().cloned().collect();
    for id in ids {
        info!("Initializing mod {}", id);
        let load_fn = MODS.lock().unwrap()[&id].load_fn;
        match load_fn {
            Some(load_fn) => unsafe { load_fn() },
            None => warn!("Mod {} is missing a load function!", id),
        }
    }
    Ok(())
}

#[no_mangle]
pub extern "C" fn load() {
    call_plugin_loads().unwrap();
}
