use il2cpp_types::Il2CppReflectionMethod;
use std::mem::transmute;

pub const NATIVE_MAP: &[((&str, &str, &str), *const ())] = &[(
    ("QMerge.Hooking", "HookManager", "CreatePostfixHook"),
    create_postfix_hook as _,
)];

unsafe fn create_postfix_hook(
    original_obj: *const Il2CppReflectionMethod,
    target_obj: *const Il2CppReflectionMethod,
) {
    let original_method = &*(*original_obj).method;
}
