use super::clang::CompileCommand;
use super::data::{GenericCtx, ModDataBuilder};
use super::find_method_with_rid;
use anyhow::{bail, Context, Result};
use il2cpp_metadata_raw::Il2CppImageDefinition;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::fs;
use std::path::Path;

pub fn get_methods(src: &str) -> Result<Vec<Option<&str>>> {
    let mut methods = Vec::new();

    if let Some(arr_start) = src.find("static Il2CppMethodPointer s_methodPointers") {
        for line in src[arr_start..].lines().skip(2) {
            if line.starts_with('}') {
                break;
            }
            let name = line.trim().trim_end_matches(',');
            if name == "NULL" {
                methods.push(None);
            } else {
                methods.push(Some(name));
            }
        }
    };

    Ok(methods)
}

pub fn transform(
    compile_command: &mut CompileCommand,
    data_builder: &mut ModDataBuilder,
    image: &Il2CppImageDefinition,
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

    // map from token to s_rgctxValues range
    let mut rgctx_indices: Vec<(u32, (usize, usize))> = Vec::new();

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
                if num_str == "-1" {
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
        } else if line.starts_with("static const Il2CppTokenRangePair s_rgctxIndices") {
            lines
                .next()
                .context("file ended reading s_rgctxIndices (skip '{')")?;
            writeln!(new_src, "{{")?;
            loop {
                let line = lines.next().context("file ended reading s_rgctxIndices")?;
                writeln!(new_src, "{}", line)?;
                if line == "};" {
                    break;
                }
                let len_str = line
                    .split_whitespace()
                    .nth(4)
                    .context("value in s_rgctxValues has wrong word count")?;
                let len = str::parse(len_str)?;
                let start_str = line
                    .split_whitespace()
                    .nth(3)
                    .unwrap()
                    .trim_end_matches(',');
                let start = str::parse(start_str)?;
                let token_str = line
                    .split_whitespace()
                    .nth(1)
                    .unwrap()
                    .trim_end_matches(',')
                    .trim_start_matches("0x");
                let token = u32::from_str_radix(token_str, 16)?;
                rgctx_indices.push((token, (start, len)));
            }
        } else if line.starts_with("static const Il2CppRGCTXDefinition s_rgctxValues") {
            lines
                .next()
                .context("file ended reading s_rgctxValues (skip '{')")?;
            let mut values_idx = 0;
            writeln!(new_src, "{{")?;
            loop {
                let line = lines.next().context("file ended reading s_rgctxValues")?;
                if line == "};" {
                    break;
                }
                let idx_str = line
                    .split_whitespace()
                    .nth(2)
                    .context("value in s_rgctxValues has wrong word count")?;
                let idx = str::parse(idx_str)?;
                let data_ty_str = line
                    .split_whitespace()
                    .nth(1)
                    .unwrap()
                    .trim_start_matches("(Il2CppRGCTXDataType)")
                    .trim_end_matches(',');
                let data_ty = str::parse(data_ty_str)?;

                let new_idx = match data_ty {
                    1 => {
                        let token = rgctx_indices
                            .iter()
                            .find(|(_, (start, len))| {
                                values_idx >= *start && values_idx < *start + *len
                            })
                            .context("could not find token range for rgctx type value")?
                            .0;
                        let method_idx =
                            find_method_with_rid(data_builder.metadata, image, token & 0x00FFFFFF)?;
                        let ctx = GenericCtx::for_method(
                            data_builder.metadata,
                            &data_builder.metadata.methods[method_idx],
                        );
                        data_builder.add_type(idx, &ctx)?
                    }
                    2 => data_builder.add_type_def(idx)?,
                    3 => data_builder.add_method(idx)?,
                    _ => bail!("unsupported runtime generic context data type: {}", data_ty),
                };
                writeln!(
                    new_src,
                    "    {{ (Il2CppRGCTXDataType){}, {} }},",
                    data_ty, new_idx
                )?;
                values_idx += 1;
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
