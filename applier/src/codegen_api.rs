use crate::loader::{MODS, MOD_IMPORT_LUT};
use crate::xref;
use anyhow::{ensure, Context, Result};
use applier_proc_macro::proxy_codegen_api;
use il2cpp_types::{Il2CppImage, TypeDefinitionIndex, MethodInfo, Il2CppClass, Il2CppAssembly};
use tracing::info;
use std::ffi::{CStr, CString};
use std::mem::transmute;
use std::os::raw::c_char;
use std::sync::{LazyLock, OnceLock};

pub fn get_method_info_from_idx(idx: usize) -> &'static MethodInfo {
    static FN_ADDR: OnceLock<extern "C" fn(i32) -> *const MethodInfo> = OnceLock::new();
    let fn_addr = FN_ADDR.get_or_init(|| {
        let addr = xref::get_symbol(
            "_ZN6il2cpp2vm13MetadataCache38GetMethodInfoFromMethodDefinitionIndexEi",
        )
        .unwrap();
        unsafe { transmute(addr) }
    });
    unsafe { &*fn_addr(idx as i32) }
}

#[no_mangle]
pub extern "C" fn merge_codegen_resolve_method(
    mod_id: *const c_char,
    method_ref_idx: usize,
) -> Option<unsafe extern "C" fn()> {
    let mod_id = unsafe { CStr::from_ptr(mod_id) }.to_str().unwrap();
    let method_idx = MODS.lock().unwrap()[mod_id].refs.method_refs[method_ref_idx];
    let method_info = get_method_info_from_idx(method_idx);
    method_info.methodPointer
}
#[no_mangle]
pub extern "C" fn merge_codegen_initialize_method(
    mod_id: *const c_char,
    metadata_usage_idx: usize,
) {
    let mod_id = unsafe { CStr::from_ptr(mod_id) }.to_str().unwrap();
    let usage_offset = MODS.lock().unwrap()[mod_id].refs.usage_list_offset;

    _Z32il2cpp_codegen_initialize_methodj((metadata_usage_idx + usage_offset) as u32);
}

fn get_method_pointer(image: *const Il2CppImage, token: u32) -> Result<unsafe extern "C" fn()> {
    let rid = token & 0x00FFFFFF;
    // let table = token & 0xFF000000;
    ensure!(rid != 0);

    let code_gen_module = unsafe { &*(*image).codeGenModule };
    ensure!(rid <= code_gen_module.methodPointerCount);

    unsafe { code_gen_module.methodPointers.add(rid as usize - 1).read() }
        .context("method pointer was null")
}

pub(crate) extern "C" fn resolve_method_by_call_helper_addr(fn_addr: P) -> unsafe extern "C" fn() {
    let addr = fn_addr as usize;

    // look up mod info/ref index using a sorted list of mod function import helper addresses
    let info = {
        let lut = MOD_IMPORT_LUT.get().unwrap();
        let index = match lut.ptrs.as_slice().binary_search(&addr) {
            Ok(s) => s,
            Err(s) => s - 1,
        };

        lut.data[index]
    };

    // look up the method for that reference for that mod
    let mod_info = unsafe { &*info.mod_info };
    let real_idx = mod_info.refs.method_refs[info.ref_index];
    let method_info = get_method_info_from_idx(real_idx);

    let class = unsafe { &*method_info.klass };
    let ptr = if class.valuetype() != 0 {
        get_method_pointer(class.image, method_info.token).unwrap()
    } else {
        method_info.methodPointer.unwrap()
    };

    // update the mod's fixup table entry to point to the relevant function pointer
    let fixup_idx = info.fixup_index;
    unsafe { (*mod_info.fixups.add(fixup_idx)).value = ptr };

    // return that function pointer
    ptr
}

type P = *const ();

#[proxy_codegen_api]
fn _Z25il2cpp_codegen_object_newP11Il2CppClass(_: P) -> P;

#[proxy_codegen_api]
fn _Z33il2cpp_codegen_runtime_class_initP11Il2CppClass(_: P);

#[proxy_codegen_api]
fn _Z32il2cpp_codegen_initialize_methodj(_: u32);

#[proxy_codegen_api]
fn _Z39il2cpp_codegen_class_from_type_internalPK10Il2CppType(_: P) -> P;

#[proxy_codegen_api]
fn _Z3BoxP11Il2CppClassPv(_: P, _: P) -> P;

#[proxy_codegen_api]
fn _ZN6il2cpp2vm12ClassInlines19InitFromCodegenSlowEP11Il2CppClass(_: P);

#[proxy_codegen_api]
fn _Z30il2cpp_codegen_raise_exceptionP11Exception_tP10MethodInfo(_: P, _: P);

#[proxy_codegen_api("_ZN6il2cpp2vm10Reflection13GetTypeObjectEPK10Il2CppType")]
fn _Z30il2cpp_codegen_type_get_objectPK10Il2CppType(_: P) -> P;

#[proxy_codegen_api("_ZN6il2cpp2vm6String10NewWrapperEPKc")]
fn _Z33il2cpp_codegen_string_new_wrapperPKc(_: P) -> P;

#[proxy_codegen_api]
fn _Z37il2cpp_codegen_get_executing_assemblyPK10MethodInfo(_: P) -> P;

#[proxy_codegen_api]
fn _Z48il2cpp_codegen_get_array_type_mismatch_exceptionv() -> P;

#[proxy_codegen_api]
fn _Z10SZArrayNewP11Il2CppClassj(_: P, _: P) -> P;

#[proxy_codegen_api]
fn _Z6IsInstP12Il2CppObjectP11Il2CppClass(_: P, _: u32) -> P;

// TODO: Maybe log and panic here instead of proxying call?
#[proxy_codegen_api("_ZN6il2cpp2vm9Exception25GetMissingMethodExceptionEPKc")]
fn _Z43il2cpp_codegen_get_missing_method_exceptionPKc(_: P) -> P;

// These methods aren't used in generated code, but are useful to have nevertheless

#[proxy_codegen_api]
fn _ZN6il2cpp2vm13MetadataCache34GetTypeInfoFromTypeDefinitionIndexEi(index: TypeDefinitionIndex) -> *const Il2CppClass;
