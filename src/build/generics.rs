use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use anyhow::{bail, Result};
use crate::build::try_parse_call;
use super::invokers::ModFunctionUsages;
use super::FnDecl;

fn get_numbered_paths(source_paths: &mut Vec<String>, cpp_path: &Path, name: &str) {
    for i in 0.. {
        let name = if i > 0 {
            format!("{}{}", name, i)
        } else {
            name.to_string()
        };
        let path = cpp_path.join(&name).with_extension("cpp");
        if path.exists() {
            source_paths.push(name);
        } else {
            break;
        }
    }
}

fn process_line<'a>(
    line: &'a str,
    src_idx: usize,
    function_usages: &mut ModFunctionUsages,
    visited: &HashSet<String>,
    gshared_queue: &mut Vec<String>,
    inline_queue: &mut Vec<(&'a str, usize)>,
) -> Result<()> {
    let usage = if let Some(name) = try_parse_call(line, true) {
        name
    } else {
        return Ok(());
    };
    if visited.contains(usage) {
        return Ok(());
    }
    
    if usage.ends_with("_inline") {
        inline_queue.push((usage, src_idx));   
    } else if function_usages.external_methods.contains_key(usage) {
        function_usages.using_external.insert(usage.to_string());
    } else if let Some(gshared) = function_usages.generic_proxies.get(usage) {
        gshared_queue.push(gshared.clone());
    } else {
        bail!("unable to handle function usage: {}", usage);
    }

    Ok(())
}

pub fn transform(
    cpp_path: &Path,
    function_usages: &mut ModFunctionUsages,
    metadata_usage_names: &mut HashSet<String>,
) -> Result<()> {
    let mut source_names = Vec::new();
    get_numbered_paths(&mut source_names, cpp_path, "GenericMethods");
    get_numbered_paths(&mut source_names, cpp_path, "Generics");

    let mut sources = Vec::new();
    for name in &source_names {
        let path = cpp_path.join(name).with_extension("cpp");
        sources.push(fs::read_to_string(path)?);
    }

    let mut inline_functions = HashSet::new();
    let mut def_src_map = HashMap::new();
    let mut visited = HashSet::new();

    for (src_idx, src) in sources.iter().enumerate() {
        let mut lines = src.lines();
        while let Some(line) = lines.next() {
            if let Some(fn_def) = FnDecl::try_parse(line) {
                if line.ends_with(';') {
                    function_usages.forward_decls.insert(
                        fn_def.name.to_string(),
                        line.trim_end_matches(';').to_string(),
                    );
                } else {
                    lines.next().unwrap();
                    if fn_def.inline {
                        inline_functions.insert(fn_def.name);
                    }
                    def_src_map.insert(fn_def.name, src_idx);
                }
            } else if line.starts_with("inline") {
                let words = line.split_whitespace().collect::<Vec<_>>();
                let proxy_name = words[words.iter().position(|w| w.starts_with('(')).unwrap() - 1];
                lines.next().unwrap();
                let line = lines.next().unwrap().trim();
                let name_start = line.find("))").unwrap() + 2;
                let name = line[name_start..].split(')').next().unwrap();
                function_usages
                    .generic_proxies
                    .insert(proxy_name.to_string(), name.to_string());
            }
        }
    }

    // let mut gshared_queue = function_usages.using_gshared.iter().map(|s| s.as_str()).collect::<Vec<_>>();
    let mut gshared_queue = function_usages
        .using_gshared
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    let mut inline_queue = Vec::new();
    loop {
        if gshared_queue.is_empty() && inline_queue.is_empty() {
            break;
        }
        while let Some(gshared) = gshared_queue.pop() {
            visited.insert(gshared.clone());
            metadata_usage_names.insert(gshared.trim_end_matches("_gshared").to_string());
            let src_idx = def_src_map[gshared.as_str()];
            let src = &sources[src_idx];
            let mut lines = src.lines();
            while let Some(line) = lines.next() {
                if line.ends_with(';') {
                    continue;
                }
                if let Some(fn_def) = FnDecl::try_parse(line) {
                    if fn_def.name == gshared {
                        lines.next().unwrap();
                        loop {
                            let line = lines.next().unwrap();
                            if line == "}" {
                                break;
                            }
                            process_line(
                                line,
                                src_idx,
                                function_usages,
                                &visited,
                                &mut gshared_queue,
                                &mut inline_queue,
                            )?;
                        }
                        break;
                    }
                }
            }
        }
        while let Some((inline, src_idx)) = inline_queue.pop() {
            visited.insert(inline.to_string());
            let src_name = &source_names[src_idx];
            let md_usage_name = inline.trim_end_matches("_inline").to_string() + src_name;
            metadata_usage_names.insert(md_usage_name.to_string());
            let src = &sources[src_idx];
            let mut lines = src.lines();
            while let Some(line) = lines.next() {
                if let Some(fn_def) = FnDecl::try_parse(line) {
                    if fn_def.name == inline {
                        lines.next().unwrap();
                        loop {
                            let line = lines.next().unwrap();
                            if line == "}" {
                                break;
                            }
                            process_line(
                                line,
                                src_idx,
                                function_usages,
                                &visited,
                                &mut gshared_queue,
                                &mut inline_queue,
                            )?;
                        }
                        break;
                    }
                }
            }
        }
    }

    dbg!(&function_usages.using_external);
    dbg!(&metadata_usage_names);

    // dbg!(&function_usages.forward_decls);

    Ok(())
}
