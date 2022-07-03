use crate::il2cpp_types::Il2CppReflectionMethod;
use std::mem::transmute;

pub const NATIVE_MAP: &[((&str, &str, &str), *const ())] = &[(
    ("QMerge.Hooking", "HookManager", "CreatePostfixHook"),
    hook_all as *const (),
)];

unsafe fn hook_all(
    original_obj: *const Il2CppReflectionMethod,
    target_obj: *const Il2CppReflectionMethod,
) {
    // let original_method = (*original_obj).method;
    // original_method.
}
