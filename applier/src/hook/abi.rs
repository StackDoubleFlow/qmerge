// It doesn't like how the Il2CppTypeEnum values are named
#![allow(non_upper_case_globals)]

use crate::codegen_api::_ZN6il2cpp2vm13MetadataCache34GetTypeInfoFromTypeDefinitionIndexEi;
use crate::utils::get_fields;
use il2cpp_types::*;
use std::mem::size_of;

fn is_fp_ty(ty: Il2CppTypeEnum) -> bool {
    matches!(
        ty,
        Il2CppTypeEnum_IL2CPP_TYPE_R4 | Il2CppTypeEnum_IL2CPP_TYPE_R8
    )
}

fn is_composite_ty(ty: Il2CppTypeEnum) -> bool {
    matches!(ty, Il2CppTypeEnum_IL2CPP_TYPE_VALUETYPE)
}

fn is_integral_ty(ty: Il2CppTypeEnum) -> bool {
    matches!(
        ty,
        Il2CppTypeEnum_IL2CPP_TYPE_BOOLEAN
            | Il2CppTypeEnum_IL2CPP_TYPE_CHAR
            | Il2CppTypeEnum_IL2CPP_TYPE_I1
            | Il2CppTypeEnum_IL2CPP_TYPE_U1
            | Il2CppTypeEnum_IL2CPP_TYPE_I2
            | Il2CppTypeEnum_IL2CPP_TYPE_U2
            | Il2CppTypeEnum_IL2CPP_TYPE_I4
            | Il2CppTypeEnum_IL2CPP_TYPE_U4
            | Il2CppTypeEnum_IL2CPP_TYPE_I8
            | Il2CppTypeEnum_IL2CPP_TYPE_U8
            | Il2CppTypeEnum_IL2CPP_TYPE_I
            | Il2CppTypeEnum_IL2CPP_TYPE_U
            | Il2CppTypeEnum_IL2CPP_TYPE_ENUM
    )
}

fn is_pointer_ty(ty: Il2CppTypeEnum) -> bool {
    matches!(
        ty,
        Il2CppTypeEnum_IL2CPP_TYPE_PTR
            | Il2CppTypeEnum_IL2CPP_TYPE_FNPTR
            | Il2CppTypeEnum_IL2CPP_TYPE_STRING
            | Il2CppTypeEnum_IL2CPP_TYPE_SZARRAY
            | Il2CppTypeEnum_IL2CPP_TYPE_ARRAY
            | Il2CppTypeEnum_IL2CPP_TYPE_CLASS
            | Il2CppTypeEnum_IL2CPP_TYPE_OBJECT
            | Il2CppTypeEnum_IL2CPP_TYPE_VAR
            | Il2CppTypeEnum_IL2CPP_TYPE_MVAR
    )
}

fn get_class_from_idx(idx: TypeDefinitionIndex) -> &'static Il2CppClass {
    let ptr = _ZN6il2cpp2vm13MetadataCache34GetTypeInfoFromTypeDefinitionIndexEi(idx);
    if ptr.is_null() {
        panic!("tried to look up invalid type definition index");
    }
    unsafe { &*ptr }
}

fn get_ty_class(ty: &Il2CppType) -> &Il2CppClass {
    get_class_from_idx(unsafe { ty.data.klassIndex })
}

fn get_ty_size(ty: &Il2CppType) -> usize {
    match ty.type_() {
        Il2CppTypeEnum_IL2CPP_TYPE_I1
        | Il2CppTypeEnum_IL2CPP_TYPE_U1
        | Il2CppTypeEnum_IL2CPP_TYPE_BOOLEAN => 1,
        Il2CppTypeEnum_IL2CPP_TYPE_I2
        | Il2CppTypeEnum_IL2CPP_TYPE_U2
        | Il2CppTypeEnum_IL2CPP_TYPE_CHAR => 2,
        Il2CppTypeEnum_IL2CPP_TYPE_I4
        | Il2CppTypeEnum_IL2CPP_TYPE_U4
        | Il2CppTypeEnum_IL2CPP_TYPE_R4 => 4,
        Il2CppTypeEnum_IL2CPP_TYPE_I8
        | Il2CppTypeEnum_IL2CPP_TYPE_U8
        | Il2CppTypeEnum_IL2CPP_TYPE_R8 => 8,
        Il2CppTypeEnum_IL2CPP_TYPE_I | Il2CppTypeEnum_IL2CPP_TYPE_U => size_of::<usize>(),
        Il2CppTypeEnum_IL2CPP_TYPE_PTR
        | Il2CppTypeEnum_IL2CPP_TYPE_FNPTR
        | Il2CppTypeEnum_IL2CPP_TYPE_STRING
        | Il2CppTypeEnum_IL2CPP_TYPE_SZARRAY
        | Il2CppTypeEnum_IL2CPP_TYPE_ARRAY
        | Il2CppTypeEnum_IL2CPP_TYPE_CLASS
        | Il2CppTypeEnum_IL2CPP_TYPE_OBJECT
        | Il2CppTypeEnum_IL2CPP_TYPE_VAR
        | Il2CppTypeEnum_IL2CPP_TYPE_MVAR => size_of::<*const ()>(),
        Il2CppTypeEnum_IL2CPP_TYPE_VALUETYPE => {
            get_ty_class(ty).instance_size as usize - size_of::<Il2CppObject>()
        }
        type_ => unreachable!("size of type {}", type_),
    }
}

// Returns the number of registers the fields would consume
fn is_hfa(ty: &Il2CppType, ty_enum: Il2CppTypeEnum) -> Option<(u32, Il2CppTypeEnum)> {
    if !is_composite_ty(ty_enum) {
        return None;
    }

    let class = get_ty_class(ty);
    let fields = unsafe { get_fields(class as *const _ as *mut _) };
    let mut base_ty = None;
    let mut num = 0;
    for field in fields {
        if field.offset == -1 {
            // It's static
            continue;
        }
        let field_ty = unsafe { (*field.type_).type_() };
        match base_ty {
            None => {
                if is_fp_ty(field_ty) {
                    base_ty = Some(field_ty);
                }
            }
            Some(base_ty) => {
                if field_ty != base_ty {
                    return None;
                }
            }
        }
        num += 1;
    }

    if num <= 4 && base_ty.is_some() {
        Some((num, base_ty.unwrap()))
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ParameterStorage {
    Unallocated,
    VectorReg(u32),
    // Copy fields to consecutive vector registers starting at v[.0] with count .1 (one register per member).
    // .2 if double
    VectorRange(u32, u32, bool),
    // Copy to stack at offset .0
    Stack(u32),
    // Copy to general purpose registers starting at x[.0] using .1 registers as if the structure was loaded with consecutive ldrs
    GPRRange(u32, u32),
    GPReg(u32),
}

pub struct Arg {
    pub ty: &'static Il2CppType,
    // if the parameter was copied to memory and converted to a pointer
    pub ptr: bool,
    pub storage: ParameterStorage,
    pub size: usize,
}

pub struct ParamLayout {
    pub args: Vec<Arg>,
    pub num_gprs: u32,
    pub num_fprs: u32,
    pub stack_size: u32,
}

/// The instance parameter doesn't get returned, but is always in x0
pub fn layout_parameters(instance: bool, types: &[&'static Il2CppType]) -> ParamLayout {
    // A.1: next gpr num
    let mut ngrn = 0;
    // A.2: next vector reg num
    let mut nsrn = 0;
    // A.3: we don't use predicate registers
    // A.4: next stacked arg addr
    let mut nsaa = 0;

    let mut args: Vec<_> = types
        .iter()
        .map(|&ty| Arg {
            storage: ParameterStorage::Unallocated,
            ty,
            ptr: false,
            size: get_ty_size(ty),
        })
        .collect();

    // Stage B
    for arg in &mut args {
        // B.1: We don't have scalable types
        // B.2: We always know the size of composite types
        // B.3
        if is_hfa(arg.ty, arg.ty.type_()).is_some() {
            continue;
        }
        // B.4
        if is_composite_ty(arg.ty.type_()) && get_ty_class(arg.ty).actualSize > 16 {
            arg.ptr = true;
            arg.size = 8;
        }
        // B.5
        if is_composite_ty(arg.ty.type_()) {
            arg.size = (arg.size + 7) & !7;
        }
        // B.6: alignment adjusted types?
    }

    // Stage C
    if instance {
        // C.9
        ngrn += 1;
    }
    for arg in &mut args {
        let ty = arg.ty;
        let ty_enum = if arg.ptr {
            Il2CppTypeEnum_IL2CPP_TYPE_PTR
        } else {
            ty.type_()
        };

        // C.1
        if is_fp_ty(ty_enum) && nsrn < 8 {
            arg.storage = ParameterStorage::VectorReg(nsrn);
            nsrn += 1;
            continue;
        }
        let hfa = is_hfa(ty, ty_enum);
        if let Some((num, base_ty)) = hfa {
            if nsrn + num <= 8 {
                // C.2
                let is_double = base_ty == Il2CppTypeEnum_IL2CPP_TYPE_R8;
                arg.storage = ParameterStorage::VectorRange(nsrn, num, is_double);
                nsrn += num;
                continue;
            } else {
                nsrn = 8;
                arg.size = (arg.size + 7) & !7;
                // C.4
                let na = get_ty_class(ty).naturalAligment;
                if na <= 8 {
                    nsaa = (nsaa + 7) & !7;
                } else if na >= 16 {
                    nsaa = (nsaa + 15) & !15;
                }
            }
        }
        // C.5
        if is_fp_ty(ty_enum) {
            arg.size = 8;
        }
        // C.6
        if hfa.is_some() || is_fp_ty(ty_enum) {
            arg.storage = ParameterStorage::Stack(nsaa);
            nsaa += arg.size as u32;
            continue;
        }
        // C.7: we don't have pure scalable types
        // C.8: see above
        // C.9
        if (is_integral_ty(ty_enum) || is_pointer_ty(ty_enum)) && arg.size <= 8 && ngrn < 8 {
            arg.storage = ParameterStorage::GPReg(ngrn);
            ngrn += 1;
            continue;
        }
        // C.10
        if get_ty_class(ty).naturalAligment == 16 {
            ngrn = (ngrn + 1) & !1;
        }
        // C.11: We don't use 16 byte integral types
        // C.12
        let size_double_words = ((arg.size + 7) / 8) as u32;
        if is_composite_ty(ty_enum) && size_double_words <= 8 - ngrn {
            arg.storage = ParameterStorage::GPRRange(ngrn, size_double_words);
            ngrn += size_double_words;
            continue;
        }
        ngrn = 8;
        // C.14
        let stack_alignment = if is_composite_ty(ty_enum) {
            (get_ty_class(ty).naturalAligment as u32).max(8)
        } else {
            8
        };
        // this will only work if the alignment is a power of two, which is should be
        nsaa = (nsaa + (stack_alignment - 1)) & !(stack_alignment - 1);
        // C.15
        if is_composite_ty(ty_enum) {
            arg.storage = ParameterStorage::Stack(nsaa);
            nsaa += arg.size as u32;
        }
        // C.16
        if arg.size < 8 {
            arg.size = 8;
        }
        // C.17
        arg.storage = ParameterStorage::Stack(nsaa);
        nsaa += arg.size as u32;
    }

    ParamLayout {
        args,
        num_gprs: ngrn,
        num_fprs: nsrn,
        stack_size: nsaa,
    }
}
