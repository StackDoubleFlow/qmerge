mod applier;
pub mod metadata_builder;

use crate::natives::NATIVE_MAP;
use crate::utils::{get_exec_path, get_mod_data_path};
use crate::xref;
use anyhow::{anyhow, Context, Result};
use applier::ModLoader;
use dlopen::raw::Library;
use il2cpp_types::{Il2CppCodeRegistration, Il2CppMetadataRegistration};
use inline_hook::Hook;
use merge_data::MergeModData;
use metadata_builder::{CodeRegistrationBuilder, Metadata, MetadataRegistrationBuilder};
use std::collections::HashMap;
use std::fs;
use std::mem::transmute;
use std::sync::{Arc, LazyLock, Mutex, OnceLock};
use topological_sort::TopologicalSort;
use tracing::{error, info};

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
    pub lib: Arc<Library>,
    pub refs: ModRefs,
    pub load_fn: Option<unsafe extern "C" fn()>,

    pub extern_len: usize,
    pub fixups: *mut FixupEntry,
}

unsafe impl Sync for Mod {}
unsafe impl Send for Mod {}

pub static MODS: LazyLock<Mutex<HashMap<String, Box<Mod>>>> = LazyLock::new(Default::default);
pub static MOD_INIT_FNS: OnceLock<Vec<unsafe extern "C" fn()>> = OnceLock::new();
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

fn find_load_ordering(mods: &[(String, MergeModData, Arc<Library>)]) -> Vec<usize> {
    let name_map: HashMap<&String, usize> = mods
        .iter()
        .enumerate()
        .map(|(i, (id, _, _))| (id, i))
        .collect();

    let mut sort = TopologicalSort::new();

    for (idx, (id, mmd, _)) in mods.iter().enumerate() {
        for dep_id in &mmd.dependencies {
            match name_map.get(dep_id) {
                Some(&dep_idx) => sort.add_dependency(dep_idx, idx),
                None => error!("Could not resolve dependency {} for mod {}", dep_id, id),
            }
        }
        for before_id in &mmd.load_before {
            if let Some(&before_idx) = name_map.get(before_id) {
                sort.add_dependency(before_idx, idx);
            }
        }
        for after_id in &mmd.load_after {
            if let Some(&after_idx) = name_map.get(after_id) {
                sort.add_dependency(idx, after_idx);
            }
        }
    }

    let mut ordering = Vec::new();
    while let Some(next) = sort.pop() {
        ordering.push(next);
    }

    if !sort.is_empty() {
        for (i, (id, _, _)) in mods.iter().enumerate() {
            if !ordering.contains(&i) {
                error!("Could not load mod {} due to a dependency cycle", id);
            }
        }
    }
    ordering
}

fn load_mods(
    metadata: &mut Metadata,
    code_registration: &mut CodeRegistrationBuilder,
    metadata_registration: &mut MetadataRegistrationBuilder,
) -> Result<()> {
    let mut mods = Vec::new();

    for entry in fs::read_dir(get_mod_data_path().join("Mods"))? {
        let mod_dir = entry?;
        let id = mod_dir
            .file_name()
            .into_string()
            .map_err(|str| anyhow!("{:?} is not a valid id", str))?;

        let mut file_path = mod_dir.path().join(&id);
        file_path.set_extension("mmd");
        let mmd = fs::read(&file_path).context("could not read mod data")?;
        let mmd = MergeModData::deserialize(&mmd).context("failed to deserialize mod data")?;

        file_path.set_extension("so");
        let so_path = get_exec_path().join(file_path.file_name().unwrap());
        fs::copy(file_path, &so_path)?;
        let lib = Library::open(so_path).context("failed to open mod executable")?;
        let lib = Arc::new(lib);

        mods.push((id, mmd, lib));
    }

    let load_ordering = find_load_ordering(&mods);
    let mut init_fns = Vec::new();
    let mut modloader = ModLoader::new(metadata, code_registration, metadata_registration)?;
    for i in load_ordering {
        let (id, mmd, lib) = &mods[i];
        info!("Loading mod {}", id);
        modloader.load_mod(id, mmd, lib.clone())?;
        if let Some(load_fn) = MODS.lock().unwrap()[id].load_fn {
            init_fns.push(load_fn);
        }
    }
    MOD_INIT_FNS.set(init_fns).unwrap();

    if let Some(core_image) = modloader.find_image("QMergeCore.dll")? {
        info!("Hooking core natives");
        let code_gen_module = modloader
            .code_registration
            .find_module("QMergeCore.dll")
            .context("could not find core module")?;
        for (name, fn_ptr) in NATIVE_MAP {
            let token = modloader
                .find_method_token_by_name(core_image, name.0, name.1, name.2)?
                .context("Could not resolve a core native")?;
            let rid = token & 0x00FFFFFF;
            let original_ptr = unsafe { code_gen_module.methodPointers.add(rid as usize - 1).read() };

            unsafe {
                Hook::new().install(transmute::<_, _>(original_ptr), *fn_ptr);
            }
        }
    }

    modloader.finish();
    Ok(())
}
