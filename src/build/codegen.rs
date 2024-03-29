use super::clang::CompileCommand;
use super::data::ModDataBuilder;
use super::function_usages::ModFunctionUsages;
use color_eyre::eyre::{bail, ContextCompat, Result};
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
    cpp_path: &Path,
    transformed_path: &Path,
    id: &str,
    function_usages: &mut ModFunctionUsages,
) -> Result<()> {
    let file_name = format!("{}_CodeGen.c", id);
    let src = fs::read_to_string(cpp_path.join(&file_name))?;
    let mut new_src = String::new();
    let mut lines = src.lines();

    // map from token to s_rgctxValues range
    let mut rgctx_indices: Vec<(u32, (usize, usize))> = Vec::new();

    while let Some(line) = lines.next() {
        if let Some(rest) = line.strip_prefix("static const int32_t s_InvokerIndices[") {
            let len: usize = rest.trim_end_matches("] = ").parse().unwrap();
            writeln!(new_src, "static int32_t s_InvokerIndices[{}] = ", len)?;
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
                let new_num = function_usages.add_invoker(num);
                writeln!(new_src, "    {},", new_num)?;
            }
            writeln!(new_src, "}};")?;
        } else if line.starts_with("static const Il2CppTokenRangePair s_rgctxIndices") {
            writeln!(new_src, "{}", line)?;
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
            writeln!(new_src, "{}", line)?;
            lines
                .next()
                .context("file ended reading s_rgctxValues (skip '{')")?;
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
                    1 => data_builder.add_type(idx)?,
                    2 => data_builder.add_type_def(idx)?,
                    3 => data_builder.add_method(idx)?,
                    _ => bail!("unsupported runtime generic context data type: {}", data_ty),
                };
                writeln!(
                    new_src,
                    "    {{ (Il2CppRGCTXDataType){}, {} }},",
                    data_ty, new_idx
                )?;
            }
            writeln!(new_src, "}};")?;
        } else {
            writeln!(new_src, "{}", line)?;
        }
    }

    let new_path = transformed_path.join(&file_name);
    fs::write(&new_path, new_src)?;
    compile_command.add_source(new_path);

    Ok(())
}
