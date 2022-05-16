use super::clang::CompileCommand;
use super::invokers::ModFunctionUsages;
use super::{add_cpp_ty, find_struct_defs, FnDecl, StructDef};
use crate::build::try_parse_call;
use anyhow::{bail, Result};
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::fs;
use std::path::Path;

fn process_line<'a>(
    line: &'a str,
    src_idx: usize,
    function_usages: &mut ModFunctionUsages<'a>,
    visited: &HashSet<&str>,
    gshared_queue: &mut Vec<&'a str>,
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
        function_usages.using_external.insert(usage);
    } else if let Some(gshared) = function_usages.generic_proxies.get(usage) {
        gshared_queue.push(gshared);
    } else {
        bail!("unable to handle function usage: {}", usage);
    }

    Ok(())
}

fn process_fn<'a>(
    src: &'a str,
    name: &'a str,
    src_idx: usize,
    function_usages: &mut ModFunctionUsages<'a>,
    visited: &HashSet<&str>,
    gshared_queue: &mut Vec<&'a str>,
    inline_queue: &mut Vec<(&'a str, usize)>,
) -> Result<()> {
    let mut lines = src.lines();
    while let Some(line) = lines.next() {
        if line.ends_with(';') {
            continue;
        }
        if let Some(fn_def) = FnDecl::try_parse(line) {
            if fn_def.name == name {
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
                        visited,
                        gshared_queue,
                        inline_queue,
                    )?;
                }
                break;
            }
        }
    }
    Ok(())
}

fn get_md_usage_name(fn_name: &str, source_name: &str) -> String {
    // TODO: check metadata usage name for gshared inline
    if let Some(fn_name) = fn_name.strip_suffix("_inline") {
        fn_name.trim_end_matches("_gshared").to_string() + source_name
    } else {
        fn_name.trim_end_matches("_gshared").to_string()
    }
}

pub struct GenericTransformData<'a> {
    struct_fds: HashSet<&'a str>,
    struct_defs: HashMap<&'a str, StructDef<'a>>,
    generic_invoker_templates: HashMap<&'a str, String>,
    def_src_map: HashMap<&'a str, usize>,
    funcs: HashSet<&'a str>,
}

pub fn transform<'a>(
    function_usages: &mut ModFunctionUsages<'a>,
    metadata_usage_names: &mut HashSet<String>,
    source_names: &'a [String],
    sources: &'a [String],
) -> Result<GenericTransformData<'a>> {
    let mut inline_functions = HashSet::new();
    let mut def_src_map = HashMap::new();
    let mut visited = HashSet::new();

    for (src_idx, src) in sources.iter().enumerate() {
        let mut lines = src.lines();
        while let Some(line) = lines.next() {
            if let Some(fn_def) = FnDecl::try_parse(line) {
                if line.ends_with(';') {
                    function_usages
                        .forward_decls
                        .insert(fn_def.name, line.trim_end_matches(';'));
                } else {
                    lines.next().unwrap();
                    if fn_def.inline {
                        inline_functions.insert(fn_def.name);
                    }
                    def_src_map.insert(fn_def.name, src_idx);
                }
            } else if line.starts_with("inline") {
                function_usages.read_gshared_proxy(line, &mut lines);
            }
        }
    }

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
            visited.insert(gshared);
            // source name can be empty because this will never be inline
            metadata_usage_names.insert(get_md_usage_name(gshared, ""));
            let src_idx = def_src_map[gshared];
            let src = &sources[src_idx];
            process_fn(
                src,
                gshared,
                src_idx,
                function_usages,
                &visited,
                &mut gshared_queue,
                &mut inline_queue,
            )?;
        }
        while let Some((inline, src_idx)) = inline_queue.pop() {
            visited.insert(inline);
            let src_name = &source_names[src_idx];
            metadata_usage_names.insert(get_md_usage_name(inline, src_name));
            let src = &sources[src_idx];
            process_fn(
                src,
                inline,
                src_idx,
                function_usages,
                &visited,
                &mut gshared_queue,
                &mut inline_queue,
            )?;
        }
    }

    // Find all code for generic invokers
    let mut generic_invoker_templates = HashMap::new();
    for src in sources {
        let mut lines = src.lines();
        while let Some(line) = lines.next() {
            if line.starts_with("template") {
                let mut body = String::new();
                writeln!(body, "{}", line)?;
                let name = lines.next().unwrap();
                writeln!(body, "{}", name)?;
                for line in lines.by_ref() {
                    writeln!(body, "{}", line)?;
                    if line == "};" {
                        break;
                    }
                }
                generic_invoker_templates.insert(name.trim_start_matches("struct "), body);
            }
        }
    }

    let (struct_fds, struct_defs) = find_struct_defs(sources);

    Ok(GenericTransformData {
        struct_fds,
        struct_defs,
        generic_invoker_templates,
        funcs: visited,
        def_src_map,
    })
}

pub fn write(
    transform_data: GenericTransformData,
    transformed_path: &Path,
    compile_command: &mut CompileCommand,
    usage_fds: &[String],
    source_names: &[String],
    sources: &[String],
) -> Result<()> {
    let mut external_src = String::new();
    writeln!(external_src, "#include \"codegen/il2cpp-codegen.h\"")?;
    writeln!(external_src, "#include \"merge/codegen.h\"")?;
    writeln!(external_src)?;

    let GenericTransformData {
        struct_defs,
        mut struct_fds,
        generic_invoker_templates,
        funcs,
        def_src_map,
    } = transform_data;

    // TODO: don't just add all struct definitions
    // I need to find a way to cleanly find all struct usages of a function
    for &fd in struct_fds.iter() {
        writeln!(external_src, "struct {};", fd)?;
    }
    let mut added_structs = HashSet::new();
    for (name, body) in generic_invoker_templates {
        added_structs.insert(name);
        writeln!(external_src, "{}", body)?;
    }
    for (&name, _) in struct_defs.iter() {
        add_cpp_ty(
            &mut external_src,
            name,
            &struct_defs,
            &mut struct_fds,
            &mut added_structs,
        )?;
    }
    for usage_fd in usage_fds {
        writeln!(external_src, "{}", usage_fd)?;
    }
    writeln!(external_src)?;

    for src in sources {
        let mut lines = src.lines();
        while let Some(line) = lines.next() {
            if line.ends_with(';') {
                continue;
            }
            if let Some(fn_def) = FnDecl::try_parse(line) {
                if funcs.contains(fn_def.name) {
                    let source_name = &source_names[def_src_map[fn_def.name]];
                    writeln!(
                        external_src,
                        "IL2CPP_EXTERN_C const uint32_t {}_MetadataUsageId;",
                        get_md_usage_name(fn_def.name, source_name)
                    )?;
                    writeln!(external_src, "{}", line)?;
                    loop {
                        let line = lines.next().unwrap();
                        writeln!(external_src, "{}", line)?;
                        if line == "}" {
                            break;
                        }
                    }
                }
            }
        }
    }

    let new_path = transformed_path.join("MergeGeneric.cpp");
    fs::write(&new_path, external_src)?;
    compile_command.add_source(new_path);

    Ok(())
}
