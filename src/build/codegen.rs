use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::fs;
use std::path::Path;

use super::clang::CompileCommand;

pub fn transform(
    compile_command: &mut CompileCommand,
    cpp_path: &Path,
    transformed_path: &Path,
    id: &str,
) -> Result<()> {
    let file_name = format!("{}_CodeGen.c", id);
    let src = fs::read_to_string(cpp_path.join(&file_name))?;
    let mut new_src = String::new();
    let mut lines = src.lines();

    let mut required_invokers = Vec::new();
    let mut invokers_map = HashMap::new();

    while let Some(line) = lines.next() {
        writeln!(new_src, "{}", line)?;
        if line.starts_with("static const int32_t s_InvokerIndices") {
            lines
                .next()
                .context("file ended reading s_InvokerIndices (skip '{')")?;
            writeln!(new_src, "{{")?;
            loop {
                let line = lines
                    .next()
                    .context("file ended reading s_InvokerIndices")?;
                if line == "};" {
                    break;
                }
                let num_str = line.trim().trim_end_matches(',');
                if num_str == "NULL" {
                    writeln!(new_src, "{}", line)?;
                    continue;
                }
                let num: usize = num_str.parse()?;
                let new_num = *invokers_map.entry(num).or_insert_with(|| {
                    let new_num = required_invokers.len();
                    required_invokers.push(num);
                    new_num
                });
                writeln!(new_src, "    {},", new_num)?;
            }
            writeln!(new_src, "}};")?;
        }
    }

    let new_path = transformed_path.join(&file_name);
    fs::write(&new_path, new_src)?;
    compile_command.add_source(new_path);
    transform_invoker_table(
        compile_command,
        cpp_path,
        transformed_path,
        required_invokers,
    )?;

    Ok(())
}

fn transform_invoker_table(
    compile_command: &mut CompileCommand,
    cpp_path: &Path,
    transformed_path: &Path,
    required_invokers: Vec<usize>,
) -> Result<()> {
    let src = fs::read_to_string(cpp_path.join("Il2CppInvokerTable.cpp"))?;

    let mut invokers = Vec::new();
    let arr_start = src
        .find("const InvokerMethod g_Il2CppInvokerPointers")
        .context("could not find g_Il2CppInvokerPointers")?;
    for line in src[arr_start..].lines().skip(3) {
        if line.starts_with('}') {
            break;
        }
        let name = line.trim().trim_end_matches(',');
        invokers.push(name);
    }

    let mut keep_invokers = HashSet::new();
    for &idx in &required_invokers {
        keep_invokers.insert(invokers[idx]);
    }

    let mut new_src = String::new();
    let mut lines = src.lines();
    while let Some(line) = lines.next() {
        if line.starts_with("void*") {
            let name = line.split_whitespace().nth(1).context("weird fn def")?;
            let keep = keep_invokers.contains(name);
            if keep {
                writeln!(new_src, "{}", line)?;
            }
            for line in lines.by_ref() {
                if keep {
                    writeln!(new_src, "{}", line)?;
                }
                if line.is_empty() {
                    break;
                }
            }
        } else if line.starts_with("const InvokerMethod g_Il2CppInvokerPointers") {
            writeln!(
                new_src,
                "const InvokerMethod g_Il2CppInvokerPointers[{}] =",
                required_invokers.len()
            )?;
            writeln!(new_src, "{{")?;
            for &idx in &required_invokers {
                writeln!(new_src, "    {},", invokers[idx])?;
            }
            writeln!(new_src, "}};")?;
            break;
        } else {
            writeln!(new_src, "{}", line)?;
        }
    }

    let new_path = transformed_path.join("Il2CppInvokerTable.cpp");
    fs::write(&new_path, new_src)?;
    compile_command.add_source(new_path);

    Ok(())
}
