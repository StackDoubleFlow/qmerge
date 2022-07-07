use il2cpp_types::Il2CppReflectionMethod;
use crate::hook;

pub const NATIVE_MAP: &[((&str, &str, &str), *const ())] = &[(
    ("QMerge.Hooking", "HookManager", "CreatePostfixHook"),
    create_postfix_hook as _,
)];

unsafe fn create_postfix_hook(
    original_obj: *const Il2CppReflectionMethod,
    postfix_obj: *const Il2CppReflectionMethod,
) {
    hook::create_postfix_hook(original_obj, postfix_obj).unwrap()
}
