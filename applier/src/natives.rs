use std::ffi::CString;

use crate::hook;
use il2cpp_types::{Il2CppReflectionMethod, Il2CppString, MethodInfo};
use ndk_sys::{__android_log_buf_write, log_id_LOG_ID_MAIN};

pub const NATIVE_MAP: &[((&str, &str, &str), *const ())] = &[
    (
        ("QMerge.Hooking", "HookManager", "CreateHookNative"),
        create_hook as _,
    ),
    (
        ("QMerge.Logging", "Logger", "LogMessageNative"),
        log_message as _,
    ),
];

unsafe extern "C" fn create_hook(
    original_obj: *const Il2CppReflectionMethod,
    prefix_obj: *const Il2CppReflectionMethod,
    postfix_obj: *const Il2CppReflectionMethod,
    _: *const MethodInfo,
) {
    hook::create_hook(original_obj, prefix_obj, postfix_obj).unwrap()
}

unsafe fn read_string(str_obj: *const Il2CppString) -> String {
    let str_obj = &*str_obj;
    let utf16_chars = str_obj.chars.as_slice(str_obj.length as usize);
    String::from_utf16_lossy(utf16_chars)
}

unsafe extern "C" fn log_message(
    priority: i32,
    tag: *const Il2CppString,
    message: *const Il2CppString,
    _: *const MethodInfo,
) {
    let tag = CString::new(read_string(tag)).unwrap();
    let message = CString::new(read_string(message)).unwrap();
    __android_log_buf_write(
        log_id_LOG_ID_MAIN as i32,
        priority,
        tag.as_ptr(),
        message.as_ptr(),
    );
}
