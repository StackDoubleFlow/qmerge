use crate::il2cpp_types::MethodInfo;
use crate::modloader::MODS;
use crate::xref;
use applier_proc_macro::proxy_codegen_api;
use std::ffi::CStr;
use std::lazy::{SyncLazy, SyncOnceCell};
use std::mem::transmute;
use std::os::raw::c_char;

fn get_method_info_from_idx(idx: usize) -> &'static MethodInfo {
    static FN_ADDR: SyncOnceCell<extern "C" fn(i32) -> *const MethodInfo> = SyncOnceCell::new();
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

type P = *const ();

#[proxy_codegen_api]
fn _Z25il2cpp_codegen_object_newP11Il2CppClass(_: P);

#[proxy_codegen_api]
fn _Z33il2cpp_codegen_runtime_class_initP11Il2CppClass(_: P);

#[proxy_codegen_api]
fn _Z32il2cpp_codegen_initialize_methodj(_: u32);

#[proxy_codegen_api]
fn _Z39il2cpp_codegen_class_from_type_internalPK10Il2CppType(_: P);

#[proxy_codegen_api]
fn _Z3BoxP11Il2CppClassPv(_: P, _: P);

#[proxy_codegen_api]
fn _ZN6il2cpp2vm12ClassInlines19InitFromCodegenSlowEP11Il2CppClass(_: P);
