mod applier;
pub mod metadata_builder;

use crate::il2cpp_types::{Il2CppCodeRegistration, Il2CppMetadataRegistration};
use crate::utils::{get_exec_path, get_mod_data_path};
use crate::xref;
use anyhow::{anyhow, Context, Result};
use applier::ModLoader;
use dlopen::raw::Library;
use inline_hook::Hook;
use merge_data::MergeModData;
use metadata_builder::{CodeRegistrationBuilder, Metadata, MetadataRegistrationBuilder};
use std::collections::HashMap;
use std::fs;
use std::mem::transmute;
use std::sync::{LazyLock, Mutex, OnceLock};
use tracing::info;

#[derive(Default, Debug)]
pub struct ImportLut {
    pub ptrs: Vec<usize>,
    pub data: Vec<ImportLutEntry>,
}

#[derive(Copy, Clone, Debug)]
pub struct ImportLutEntry {
    pub mod_info: *const Mod,
    pub fixup_index: usize,
    pub ref_index: usize,
}

unsafe impl Sync for ImportLutEntry {}
unsafe impl Send for ImportLutEntry {}

#[repr(transparent)]
pub struct FixupEntry {
    pub value: unsafe extern "C" fn(),
}

pub struct ModRefs {
    pub type_def_refs: Vec<usize>,
    pub method_refs: Vec<usize>,
    pub usage_list_offset: usize,
}

pub struct Mod {
    pub lib: Library,
    pub refs: ModRefs,
    pub load_fn: Option<unsafe extern "C" fn()>,

    pub extern_len: usize,
    pub fixups: *mut FixupEntry,
}

unsafe impl Sync for Mod {}
unsafe impl Send for Mod {}

pub static MODS: LazyLock<Mutex<HashMap<String, Box<Mod>>>> = LazyLock::new(Default::default);
pub static MOD_IMPORT_LUT: OnceLock<ImportLut> = OnceLock::new();
pub static CODE_REGISTRATION: OnceLock<&'static Il2CppCodeRegistration> = OnceLock::new();
pub static METADATA_REGISTRATION: OnceLock<&'static Il2CppMetadataRegistration> = OnceLock::new();

pub fn install_hooks() {
    LazyLock::force(&LOAD_METADATA_HOOK);
}

static LOAD_METADATA_HOOK: LazyLock<Hook> = LazyLock::new(|| {
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

    modloader.finish();
    Ok(())
}
