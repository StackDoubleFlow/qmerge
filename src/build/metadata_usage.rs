use super::clang::CompileCommand;
use super::data::ModDataBuilder;
use color_eyre::eyre::{ContextCompat, Result};
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::fs;
use std::path::Path;

/// Returns a vec containing forward declarations for all usages
pub fn transform(
    compile_command: &mut CompileCommand,
    cpp_path: &Path,
    transformed_path: &Path,
    data_buider: &mut ModDataBuilder,
    mod_functions: &HashSet<String>,
) -> Result<(Vec<String>, usize)> {
    let src = fs::read_to_string(cpp_path.join("Il2CppMetadataUsage.c"))?;

    // The names and indices of the metadata usage ranges
    let mut required_usage_ids = Vec::new();
    for line in src.lines() {
        if line.starts_with("const uint32_t") {
            let words: Vec<&str> = line.split_whitespace().collect();
            let name = words[2];
            let fn_name = &name[..name.len() - 16];
            if mod_functions.contains(fn_name) {
                let id = words[4].trim_end_matches(';').parse::<u32>()?;
                required_usage_ids.push((name, id));
            }
        }
    }

    let mut usage_map = HashMap::new();
    // the indicies to add to the runtime metadataUsages table in order
    let mut usage_list = Vec::new();
    let mut new_usage_ids = String::new();
    for (name, idx) in required_usage_ids {
        let new_idx = data_buider.add_metadata_usage_range(&mut usage_map, &mut usage_list, idx)?;
        writeln!(new_usage_ids, "extern const uint32_t {};", name)?;
        writeln!(new_usage_ids, "const uint32_t {} = {};", name, new_idx)?;
    }

    let mut using_names = HashSet::new();
    let mut new_list = Vec::new();

    let arr_start = src
        .find("const g_MetadataUsages")
        .context("could not find g_MetadataUsages")?;
    for usage in usage_list {
        let line = src[arr_start..]
            .lines()
            .nth(3 + usage)
            .context("metadata usage out of range")?;
        let name = line
            .trim()
            .trim_start_matches("(void**)(&")
            .trim_end_matches("),");
        new_list.push(name);
        using_names.insert(name);
    }

    let mut usage_fds = Vec::new();
    let mut new_src = String::new();
    let mut lines = src.lines();
    while let Some(line) = lines.next() {
        if line.starts_with("const RuntimeType*")
            || line.starts_with("RuntimeClass*")
            || line.starts_with("const RuntimeMethod*")
            || line.starts_with("RuntimeField*")
            || line.starts_with("String_t*")
        {
            let name = line
                .trim_start_matches("const ")
                .trim_end_matches(';')
                .split_whitespace()
                .nth(1)
                .unwrap();
            if using_names.contains(&name) {
                writeln!(new_src, "{}", line)?;
                usage_fds.push(line.to_string());
            }
        } else if line.starts_with("void** const g_MetadataUsages") {
            writeln!(
                new_src,
                "void** const g_MetadataUsages[{}] = ",
                new_list.len()
            )?;
            writeln!(new_src, "{{")?;
            for name in &new_list {
                writeln!(new_src, "    (void**)(&{}),", name)?;
            }
            writeln!(new_src, "}};")?;

            for line in lines.by_ref() {
                if line == "};" {
                    break;
                }
            }
        } else if !line
            .trim_start_matches("extern ")
            .starts_with("const uint32_t")
        {
            writeln!(new_src, "{}", line)?;
        }
    }
    new_src.push_str(&new_usage_ids);

    let new_path = transformed_path.join("Il2CppMetadataUsage.c");
    fs::write(&new_path, new_src)?;
    compile_command.add_source(new_path);

    Ok((usage_fds, new_list.len()))
}
