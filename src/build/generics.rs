use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

fn get_method_pointer_table(src: &str) -> Result<Vec<Option<&str>>> {
    let mut methods = Vec::new();

    if let Some(arr_start) = src.find("const Il2CppMethodPointer g_Il2CppGenericMethodPointers") {
        for line in src[arr_start..].lines().skip(3) {
            if line.starts_with('}') {
                break;
            }
            let name = line
                .trim()
                .trim_start_matches("(Il2CppMethodPointer)&")
                .split('/')
                .next()
                .context("malformed generic method pointer table")?;
            if name == "NULL" {
                methods.push(None);
            } else {
                methods.push(Some(name));
            }
        }
    };

    Ok(methods)
}

fn transform(cpp_path: &Path) -> Result<()> {
    let needed_generic_funs = vec![""];
    let mpt_src = fs::read_to_string(cpp_path.join("Il2CppGenericMethodPointerTable.cpp"))?;
    let mpt = get_method_pointer_table(&mpt_src)?;

    Ok(())
}
