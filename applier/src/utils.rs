use crate::codegen_api::_ZN6il2cpp2vm12ClassInlines19InitFromCodegenSlowEP11Il2CppClass;
use anyhow::Result;
use il2cpp_types::{FieldInfo, Il2CppClass};
use std::path::PathBuf;
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
