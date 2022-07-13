mod abi;
mod alloc;
mod codegen;

use self::abi::{Arg, ParamLayout, ParameterStorage};
use crate::hook::alloc::HOOK_ALLOCATOR;
use crate::hook::codegen::HookGenerator;
use crate::utils::get_fields;
use anyhow::{bail, Result};
use il2cpp_types::{
    FieldInfo, Il2CppClass, Il2CppReflectionMethod, Il2CppType, Il2CppTypeEnum_IL2CPP_TYPE_CLASS,
    Il2CppTypeEnum_IL2CPP_TYPE_VALUETYPE, MethodInfo, METHOD_ATTRIBUTE_STATIC, Il2CppTypeEnum_IL2CPP_TYPE_VOID,
};
use inline_hook::Hook;
use std::ffi::CStr;
use std::slice;
use tracing::{debug, instrument};

struct Param {
    name: &'static str,
    ty: *const Il2CppType,
}

unsafe fn get_params(method: &MethodInfo) -> Result<Vec<Param>> {
    let params = slice::from_raw_parts(method.parameters, method.parameters_count as usize);
    params
        .iter()
        .map(|param| {
            Ok(Param {
                name: CStr::from_ptr(param.name).to_str()?,
                ty: param.parameter_type,
            })
        })
        .collect()
}

unsafe fn find_field(class: *mut Il2CppClass, name: &str) -> Result<Option<usize>> {
    for (i, field) in get_fields(class).iter().enumerate() {
        let field_name = CStr::from_ptr(field.name).to_str()?;
        if name == field_name {
            return Ok(Some(i));
        }
    }
    Ok(None)
}

unsafe fn field_ty_matches(field_ty: *const Il2CppType, injection_ty: *const Il2CppType) -> bool {
    let field_ty = &*field_ty;
    let injection_ty = &*injection_ty;
    field_ty.data.dummy == injection_ty.data.dummy
        && field_ty.type_() == injection_ty.type_()
        && field_ty.byref() == injection_ty.byref()
}

#[derive(Debug)]
enum ParamInjection {
    OriginalParam(usize),
    LoadField(usize),
    Result,
    Instance,
}

unsafe fn get_injections(
    method_obj: *const Il2CppReflectionMethod,
    original_method: &MethodInfo,
    original_params: &[Param],
    is_instance: bool,
) -> Result<Option<(CodegenMethod, Vec<ParamInjection>)>> {
    if method_obj.is_null() {
        return Ok(None);
    }
    let method = &*(*method_obj).method;
    let params = get_params(method)?;

    if method.flags & METHOD_ATTRIBUTE_STATIC as u16 == 0 {
        bail!("Hook method must be static");
    }

    let mut injections = Vec::new();
    for param in &params {
        if let Some(field_name) = param.name.strip_prefix("___") {
            let idx = find_field(original_method.klass, field_name)?;
            match idx {
                Some(idx) => {
                    let field_ty = get_fields(original_method.klass)[idx].type_;
                    if !field_ty_matches(field_ty, param.ty) {
                        bail!(
                            "Field injection type mismatch on parameter \"{}\"",
                            param.name
                        );
                    }
                    injections.push(ParamInjection::LoadField(idx))
                }
                None => bail!("could not find field with name {}", field_name),
            }
        } else if param.name == "__instance" {
            if !is_instance {
                bail!("cannot inject __instance parameter on non-instance method");
            }
            let ty = &*param.ty;

            if ty.type_() == Il2CppTypeEnum_IL2CPP_TYPE_CLASS
                || (ty.type_() == Il2CppTypeEnum_IL2CPP_TYPE_VALUETYPE && ty.byref() != 0)
            {
                // TODO: Verify type data
                injections.push(ParamInjection::Instance);
            } else {
                bail!("type mismatch for instance parameter injection");
            }
        } else if param.name == "__result" {
            if original_method.return_type.is_null() || (*original_method.return_type).type_() == Il2CppTypeEnum_IL2CPP_TYPE_VOID {
                bail!("cannot inject __result for method with void return type")
            }
            if param.ty != original_method.return_type {
                bail!("__result type mismatch");
            }
            injections.push(ParamInjection::Result);
        } else {
            let mut found = false;
            for (i, original_param) in original_params.iter().enumerate() {
                if original_param.name == param.name {
                    if original_param.ty != param.ty {
                        bail!(
                            "Parameter injection type mismatch on parameter \"{}\"",
                            param.name
                        );
                    }
                    injections.push(ParamInjection::OriginalParam(i));
                    found = true;
                    break;
                }
            }
            if !found {
                bail!("could not find param with name {}", param.name);
            }
        }
    }

    let codegen = CodegenMethod::new(method, params, false);
    Ok(Some((codegen, injections)))
}

pub unsafe fn create_hook(
    original_obj: *const Il2CppReflectionMethod,
    prefix_obj: *const Il2CppReflectionMethod,
    postfix_obj: *const Il2CppReflectionMethod,
) -> Result<()> {
    let original_method = &*(*original_obj).method;
    let original_params = get_params(original_method)?;

    let is_instance = (original_method.flags & METHOD_ATTRIBUTE_STATIC as u16) == 0;
    let prefix_injections =
        get_injections(prefix_obj, original_method, &original_params, is_instance)?;
    let postfix_injections =
        get_injections(postfix_obj, original_method, &original_params, is_instance)?;
    let original_codegen = CodegenMethod::new(original_method, original_params, is_instance);

    let mut reserve_call_stack = 0;
    if let Some((codegen, _)) = &prefix_injections {
        reserve_call_stack = reserve_call_stack.max(codegen.layout.stack_size);
    }
    if let Some((codegen, _)) = &postfix_injections {
        reserve_call_stack = reserve_call_stack.max(codegen.layout.stack_size);
    }
    let mut gen = HookGenerator::new(&original_codegen, is_instance, reserve_call_stack);
    if let Some((codegen, injections)) = prefix_injections {
        gen.gen_call_hook(codegen, injections);
    }
    gen.call_orig();
    if let Some((codegen, injections)) = postfix_injections {
        gen.gen_call_hook(codegen, injections);
    }
    gen.finish_and_install();

    Ok(())
}

struct CodegenMethod {
    method: &'static MethodInfo,
    params: Vec<Param>,
    layout: ParamLayout,
    ret_layout: Option<Arg>,
}

impl CodegenMethod {
    fn new(method: &'static MethodInfo, params: Vec<Param>, is_instance: bool) -> Self {
        let param_types: Vec<_> = params.iter().map(|param| unsafe { &*param.ty }).collect();
        let layout = abi::layout_parameters(is_instance, &param_types);
        let ret_layout = if method.return_type.is_null() {
            None
        } else {
            Some(
                abi::layout_parameters(false, &[unsafe { &*method.return_type }])
                    .args
                    .remove(0),
            )
        };
        Self {
            method,
            params,
            layout,
            ret_layout,
        }
    }
}
