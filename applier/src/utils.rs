// Il2CppTypeEnum variants
#![allow(non_upper_case_globals)]

use crate::codegen_api::{
    _ZN6il2cpp2vm12ClassInlines19InitFromCodegenSlowEP11Il2CppClass,
    _ZN6il2cpp2vm13MetadataCache34GetTypeInfoFromTypeDefinitionIndexEi,
};
use anyhow::{Result, ensure, Context};
use il2cpp_types::{
    FieldInfo, Il2CppClass, Il2CppType, Il2CppTypeEnum_IL2CPP_TYPE_CLASS,
    Il2CppTypeEnum_IL2CPP_TYPE_VALUETYPE, TypeDefinitionIndex, Il2CppImage,
};
use std::{slice, str};

pub fn offset_len(offset: i32, len: i32) -> std::ops::Range<usize> {
    if (offset as i32) < 0 {
        return 0..0;
    }
    offset as usize..offset as usize + len as usize
}

fn strlen(data: &[u8], offset: usize) -> usize {
    let mut len = 0;
    while data[offset + len] != 0 {
        len += 1;
    }
    len
}

pub fn get_str(data: &[u8], offset: usize) -> Result<&str> {
    let len = strlen(data, offset);
    let str = str::from_utf8(&data[offset..offset + len])?;
    Ok(str)
}

pub unsafe fn ensure_class_init(class: *mut Il2CppClass) {
    if (*class).initialized_and_no_error() == 0 {
        _ZN6il2cpp2vm12ClassInlines19InitFromCodegenSlowEP11Il2CppClass(class);
    }
}

pub unsafe fn get_fields(class: *mut Il2CppClass) -> &'static [FieldInfo] {
    ensure_class_init(class);
    let class = &*class;
    slice::from_raw_parts(class.fields, class.field_count as usize)
}

fn get_class_from_idx(idx: TypeDefinitionIndex) -> &'static Il2CppClass {
    let ptr = _ZN6il2cpp2vm13MetadataCache34GetTypeInfoFromTypeDefinitionIndexEi(idx);
    if ptr.is_null() {
        panic!("tried to look up invalid type definition index");
    }
    unsafe { &*ptr }
}

/// type data must be klassIndex
pub fn get_ty_class(ty: &Il2CppType) -> &Il2CppClass {
    assert!(matches!(
        ty.type_(),
        Il2CppTypeEnum_IL2CPP_TYPE_CLASS | Il2CppTypeEnum_IL2CPP_TYPE_VALUETYPE
    ));
    get_class_from_idx(unsafe { ty.data.klassIndex })
}

pub fn get_method_pointer(image: *const Il2CppImage, token: u32) -> Result<unsafe extern "C" fn()> {
    let rid = token & 0x00FFFFFF;
    // let table = token & 0xFF000000;
    ensure!(rid != 0);

    let code_gen_module = unsafe { &*(*image).codeGenModule };
    ensure!(rid <= code_gen_module.methodPointerCount);

    unsafe { code_gen_module.methodPointers.add(rid as usize - 1).read() }
        .context("method pointer was null")
}
