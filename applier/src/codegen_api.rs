macro_rules! api_todo {
    ( $( $name:ident, )* ) => {
        $(
            #[no_mangle]
            extern "C" fn $name() {
                panic!("TODO: codegen_api::{}", stringify!($name))
            }
        )*
    };
}

api_todo! {
    il2cpp_codegen_marshal_store_last_error,
    il2cpp_codegen_delegate_begin_invoke,
    il2cpp_codegen_delegate_end_invoke,
    il2cpp_codegen_resolve_icall,
    il2cpp_codegen_type_get_object,
    il2cpp_codegen_get_method_object_internal,
    il2cpp_codegen_get_executing_assembly,
    il2cpp_codegen_register,
    il2cpp_codegen_initialize_method,
    il2cpp_codegen_get_generic_method_definition,
    il2cpp_codegen_get_thread_static_data,
    il2cpp_codegen_memory_barrier,
    SZArrayNew,
    GenArrayNew,
    il2cpp_codegen_method_is_generic_instance,
    il2cpp_codegen_method_get_declaring_type,
    MethodIsStatic,
    MethodHasParameters,
    il2cpp_codegen_raise_profile_exception,
    il2cpp_codegen_get_generic_virtual_method_internal,
    il2cpp_codegen_runtime_class_init,
    il2cpp_codegen_raise_execution_engine_exception,
    il2cpp_codegen_raise_execution_engine_exception_if_method_is_not_found,
    IsInst,
    Box,
    Unbox_internal,
    UnBoxNullable_internal,
    il2cpp_codegen_object_new,
    il2cpp_codegen_marshal_allocate,

}
