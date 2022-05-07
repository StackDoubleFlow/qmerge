use super::clang::CompileCommand;
use super::data::ModDataBuilder;
use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::fs;
use std::path::Path;

pub fn transform(
    compile_command: &mut CompileCommand,
    cpp_path: &Path,
    transformed_path: &Path,
    data_buider: &mut ModDataBuilder,
    mod_functions: HashSet<String>,
) -> Result<()> {
    let src = fs::read_to_string(cpp_path.join("Il2CppMetadataUsage.c"))?;

    let mut required_usage_ids = Vec::new();
    for line in src.lines() {
        if line.starts_with("const uint32_t") {
            let words: Vec<&str> = line.split_whitespace().collect();
            let name = words[2];
            let fn_name = &name[..name.len() - 16];
            if mod_functions.contains(fn_name) {
                required_usage_ids.push(words[4].trim_end_matches(';').parse::<u32>()?)
            }
        }
    }

    let mut usage_map = HashMap::new();
    // the indicies to add to the runtime metadataUsages table in order
    let mut usage_list = Vec::new();
    for idx in required_usage_ids {
        data_buider.add_metadata_usage_range(&mut usage_map, &mut usage_list, idx)?;
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
        } else if line.starts_with("extern const uint32_t") {
            lines.next().unwrap();
            let name = &line[22..line.len() - 17];
            if mod_functions.contains(name) {
                writeln!(new_src, "{}", line)?;
                // TODO: this is a hack and depends on HashSet iteration order being stable as long as it's not modified, plus its slow
                let idx = mod_functions.iter().position(|s| s == name).unwrap();
                writeln!(
                    new_src,
                    "const uint32_t {}_MetadataUsageId = {};",
                    name, idx
                )?;
            }
        } else {
            writeln!(new_src, "{}", line)?;
        }
    }

    let new_path = transformed_path.join("Il2CppMetadataUsage.c");
    fs::write(&new_path, new_src)?;
    compile_command.add_source(new_path);

    Ok(())
}
