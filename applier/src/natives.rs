use crate::hook;
use il2cpp_types::{Il2CppReflectionMethod, MethodInfo};

pub const NATIVE_MAP: &[((&str, &str, &str), *const ())] = &[(
    ("QMerge.Hooking", "HookManager", "CreateHookNative"),
    create_hook as _,
)];

unsafe extern "C" fn create_hook(
    original_obj: *const Il2CppReflectionMethod,
    prefix_obj: *const Il2CppReflectionMethod,
    postfix_obj: *const Il2CppReflectionMethod,
    _: *const MethodInfo,
) {
    hook::create_hook(original_obj, prefix_obj, postfix_obj).unwrap()
}
