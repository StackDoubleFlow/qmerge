mod abi;
mod alloc;
mod codegen;

use crate::hook::alloc::HOOK_ALLOCATOR;
use crate::hook::codegen::PostfixGenerator;
use crate::utils::get_fields;
use anyhow::{bail, Result};
use il2cpp_types::{
    FieldInfo, Il2CppClass, Il2CppReflectionMethod, Il2CppType, MethodInfo, METHOD_ATTRIBUTE_STATIC,
};
use inline_hook::Hook;
use std::ffi::CStr;
use std::fmt::Write;
use std::slice;
use tracing::{debug, instrument};

use self::abi::ParameterStorage;

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
}

pub unsafe fn create_postfix_hook(
    original_obj: *const Il2CppReflectionMethod,
    postfix_obj: *const Il2CppReflectionMethod,
) -> Result<()> {
    let original_method = &*(*original_obj).method;
    let original_params = get_params(original_method)?;

    let postfix_method = &*(*postfix_obj).method;
    let postfix_params = get_params(postfix_method)?;

    if postfix_method.flags & METHOD_ATTRIBUTE_STATIC as u16 == 0 {
        bail!("Postfix method must be static");
    }

    let mut injections = Vec::new();
    for param in &postfix_params {
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

    let original_codegen = CodegenMethod::new(original_method, original_params);
    let postfix_codegen = CodegenMethod::new(postfix_method, postfix_params);

    debug!("Injection: {:?}", injections);
    debug!("Original layout: {:?}", original_codegen.layout);
    debug!("Postfix layout: {:?}", postfix_codegen.layout);
    let mut gen = PostfixGenerator::default();
    gen.gen_postfix(original_codegen, postfix_codegen, injections);

    let (data, code_len) = gen.finish();
    let data_ptr = HOOK_ALLOCATOR.lock().unwrap().alloc(&data);

    let hook = Hook::new();
    hook.install(original_method.methodPointer.unwrap() as _, data_ptr as _);

    let new_orig = hook.original().unwrap();
    // This assumes the orig pointer is the first fixup entry
    let orig_fixup_ptr = data_ptr.add(code_len) as *mut usize;
    orig_fixup_ptr.write(new_orig as usize);

    debug!("dumping generated code");
    let code = std::slice::from_raw_parts(data_ptr, data.len());
    for (i, ins) in code[0..code_len].iter().enumerate() {
        let ptr = ins as *const u32;
        debug!("{:?}: {}", ptr, bad64::decode(*ins, ptr as u64).unwrap());
    }
    let mut data_str = String::from("data: ");
    for &data in &code[code_len..] {
        let bytes: [u8; 4] = data.to_ne_bytes();
        for b in bytes {
            write!(data_str, "{:02x}", b).unwrap();
        }
    }
    debug!("{}", data_str);

    Ok(())
}

struct CodegenMethod {
    method: &'static MethodInfo,
    params: Vec<Param>,
    layout: Vec<ParameterStorage>,
    is_instance: bool,
}

impl CodegenMethod {
    fn new(method: &'static MethodInfo, params: Vec<Param>) -> Self {
        let param_types: Vec<_> = params.iter().map(|param| unsafe { &*param.ty }).collect();
        let is_instance = (method.flags & METHOD_ATTRIBUTE_STATIC as u16) == 0;
        let layout = abi::layout_parameters(is_instance, &param_types);
        Self {
            method,
            params,
            layout,
            is_instance,
        }
    }
}
