use applier_proc_macro::proxy_codegen_api;
use std::lazy::SyncLazy;
use std::mem::transmute;

use crate::xref;

type P = *const ();

#[proxy_codegen_api("_ZN6il2cpp2vm6Object3BoxEP11Il2CppClassPv")]
fn _Z3BoxP11Il2CppClassPv(_: P, _: P);
