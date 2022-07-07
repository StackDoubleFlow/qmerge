use anyhow::{Result, bail};
use il2cpp_types::{Il2CppReflectionMethod, Il2CppType, MethodInfo, Il2CppClass, FieldInfo, METHOD_ATTRIBUTE_STATIC};
use tracing::debug;
use std::slice;
use std::ffi::CStr;

struct Param {
    name: &'static str,
    ty: *const Il2CppType,
}

unsafe fn get_params(method: &MethodInfo) -> Result<Vec<Param>> {
    let params = slice::from_raw_parts(method.parameters, method.parameters_count as usize);
    params.iter().map(|param| Ok(Param {
        name: CStr::from_ptr(param.name).to_str()?,
        ty: param.parameter_type
    })).collect()
}

unsafe fn get_fields(class: *const Il2CppClass) -> &'static [FieldInfo] {
    let class = &*class;
    slice::from_raw_parts(class.fields, class.field_count as usize)
}

unsafe fn find_field(class: *const Il2CppClass, name: &str) -> Result<Option<usize>> {
    for (i, field) in get_fields(class).iter().enumerate() {
        let field_name = CStr::from_ptr(field.name).to_str()?;
        if name == field_name {
            return Ok(Some(i));
        }
    }
    Ok(None)
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
        if let Some(field_name) = param.name.strip_suffix("___") {
            let idx = find_field(original_method.klass, field_name)?;
            match idx {
                Some(idx) => {
                    if param.ty != get_fields(original_method.klass)[idx].type_ {
                        bail!("Field injection type mismatch on parameter \"{}\"", param.name);
                    }
                    injections.push(ParamInjection::LoadField(idx))
                },
                None => bail!("could not find field with name {}", field_name),
            }

        } else {
            let mut found = false;
            for (i, original_param) in original_params.iter().enumerate() {
                if original_param.name == param.name {
                    if original_param.ty != param.ty {
                        bail!("Parameter injection type mismatch on parameter \"{}\"", param.name);
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

    debug!("Injection: {:?}", injections);

    Ok(())
}
