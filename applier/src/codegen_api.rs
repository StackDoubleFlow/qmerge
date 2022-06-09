use applier_proc_macro::proxy_codegen_api;
use std::lazy::SyncLazy;
use std::mem::transmute;
use std::os::raw::c_char;

#[no_mangle]
pub extern "C" fn merge_codegen_resolve_method(mod_id: *const c_char, method_ref_idx: usize) {
    todo!();
}

use crate::xref;

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
