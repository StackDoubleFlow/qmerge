use std::ffi::CStr;
use std::lazy::SyncLazy;
use std::os::raw::c_char;
use std::mem::transmute;

use crate::xref;

type P = *const ();

#[no_mangle]
pub unsafe extern "C" fn merge_api_todo(name: *const c_char) {
    let name = CStr::from_ptr(name).to_str().unwrap();
    panic!("merge_api_todo: {}", name);
}

// #[link(name = "codegen_api", kind = "static", modifiers = "+whole-archive")]
// extern "C" {
//     pub fn _Z3BoxP11Il2CppClassPv();
// }

pub extern "C" fn _Z3BoxP11Il2CppClassPv(param1: P, param2: P) {
    static GAME_FN: SyncLazy<fn (P, P)> = SyncLazy::new(|| {
        let ptr = xref::get_symbol("_Z3BoxP11Il2CppClassPv").unwrap();
        unsafe { transmute(ptr) }
    });
    GAME_FN(param1, param2)
}