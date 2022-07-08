use crate::hook;
use il2cpp_types::{Il2CppReflectionMethod, MethodInfo};

pub const NATIVE_MAP: &[((&str, &str, &str), *const ())] = &[(
    ("QMerge.Hooking", "HookManager", "CreatePostfixHook"),
    create_postfix_hook as _,
)];

unsafe extern "C" fn create_postfix_hook(
    original_obj: *const Il2CppReflectionMethod,
    postfix_obj: *const Il2CppReflectionMethod,
    _: *const MethodInfo,
) {
    hook::create_postfix_hook(original_obj, postfix_obj).unwrap()
}
