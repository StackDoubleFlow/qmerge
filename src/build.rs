mod clang;
mod codegen;
mod data;
mod generics;
mod invokers;
mod metadata_usage;
mod parser;
mod runtime_metadata;
mod type_sizes;

use crate::config::{Mod, APPS, CONFIG};
use anyhow::{bail, Context, Result};
use clang::CompileCommand;
use data::{get_str, offset_len, ModDataBuilder, RuntimeMetadata};
use il2cpp_metadata_raw::{Il2CppImageDefinition, Metadata};
use invokers::ModFunctionUsages;
use merge_data::CodeTableSizes;
use parser::{is_included_ty, try_parse_call, FnDecl};
use runtime_metadata::TypeDefinitionsFile;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::Lines;
use std::{fs, str};

const CODGEN_HEADER: &str = include_str!("../include/merge/codegen.h");

fn get_function_usages<'a>(usages: &mut HashSet<&'a str>, lines: &mut Lines<'a>) {
    loop {
        let line = lines.next().unwrap();
        if line == "}" {
            return;
        }
        if let Some(name) = try_parse_call(line, false) {
            if !usages.contains(name) {
                usages.insert(name);
            }
        }
    }
}

fn find_method_with_rid(
    metadata: &Metadata,
    image: &Il2CppImageDefinition,
    rid: u32,
) -> Result<usize> {
    for type_def in &metadata.type_definitions[offset_len(image.type_start, image.type_count)] {
        for i in offset_len(type_def.method_start, type_def.method_count as u32) {
            if metadata.methods[i].token & 0x00FFFFFF == rid {
                return Ok(i);
            }
        }
    }

    bail!("could not find method with rid {}", rid);
}

fn find_image<'md>(metadata: &'md Metadata, find_name: &str) -> Result<&'md Il2CppImageDefinition> {
    for image in &metadata.images {
        let name = get_str(metadata.string, image.name_index as usize)?;
        if name == find_name {
            return Ok(image);
        }
    }
    bail!("could not find image: {}", find_name);
}

fn get_numbered_paths(source_paths: &mut Vec<String>, cpp_path: &Path, name: &str) {
    for i in 0.. {
        let name = if i > 0 {
            format!("{}{}", name, i)
        } else {
            name.to_string()
        };
        let path = cpp_path.join(format!("{}.cpp", name));
        if path.exists() {
            source_paths.push(name);
        } else {
            break;
        }
    }
}

pub struct StructDef<'src> {
    body: String,
    parent: Option<&'src str>,
}

fn find_struct_defs(sources: &[String]) -> (HashSet<&str>, HashMap<&str, StructDef>) {
    let mut struct_fds = HashSet::new();
    let mut struct_defs = HashMap::new();
    for src in sources {
        let mut lines = src.lines();
        while let Some(line) = lines.next() {
            let line = line.trim_end();

            let name = if let Some(name) = line.strip_prefix("struct ") {
                name
            } else {
                continue;
            };
            if let Some(name) = name.strip_suffix(';') {
                struct_fds.insert(name);
            } else {
                let without_generic = name
                    .trim_start_matches("Generic")
                    .trim_start_matches("Virt")
                    .trim_start_matches("Interface");
                if without_generic.starts_with("FuncInvoker")
                    || without_generic.starts_with("ActionInvoker")
                {
                    continue;
                }

                let parent = name.find(':').map(|pos| &name[pos + 9..]);
                let name = name.split(':').next().unwrap().trim();
                struct_defs.entry(name).or_insert_with(|| {
                    lines.next().unwrap();
                    let mut body = String::new();
                    for line in lines.by_ref() {
                        if line == "};" {
                            break;
                        }
                        body.push_str(line);
                        body.push('\n');
                    }

                    StructDef { body, parent }
                });
            }
        }
    }
    (struct_fds, struct_defs)
}

fn add_cpp_ty<'a>(
    writer: &mut String,
    src: &'a str,
    struct_defs: &'a HashMap<&str, StructDef>,
    fd_structs: &mut HashSet<&'a str>,
    added_structs: &mut HashSet<&'a str>,
) -> Result<()> {
    let mut words = src.trim_start_matches("const ").split_whitespace();
    let ty = words.next().unwrap().trim_end_matches('*');
    if ty == "typedef" {
        panic!()
    }
    if !is_included_ty(ty) && !added_structs.contains(ty) {
        if src.contains('*') {
            if fd_structs.contains(ty) {
                return Ok(());
            }
            writeln!(writer, "struct {};", ty)?;
            fd_structs.insert(ty);
        } else {
            let struct_def = struct_defs
                .get(ty)
                .with_context(|| format!("could not find cpp type: {}", ty))?;
            if let Some(parent) = struct_def.parent {
                if parent == "MethodInfo_t" {
                    dbg!(parent);
                }
                add_cpp_ty(writer, parent, struct_defs, fd_structs, added_structs)?;
            }
            for line in struct_def.body.lines() {
                let line = line.trim();
                if line.starts_with("union")
                    || line.starts_with("struct")
                    || line.starts_with("public:")
                    || line.starts_with("//")
                    || line.starts_with('#')
                    || line.starts_with('}')
                    || line.starts_with('{')
                {
                    continue;
                }
                if line.is_empty() {
                    break;
                }
                let line = if line.starts_with("ALIGN_FIELD") {
                    let end = line.find(')').with_context(|| {
                        format!("could not find end of ALIGN_FIELD in '{}'", line)
                    })?;
                    &line[end + 2..]
                } else {
                    line
                };
                add_cpp_ty(writer, line, struct_defs, fd_structs, added_structs)?;
            }
            if let Some(parent) = struct_def.parent {
                writeln!(
                    writer,
                    "struct {} : {}\n{{\n{}}};",
                    ty, parent, struct_def.body
                )?;
            } else {
                writeln!(writer, "struct {}\n{{\n{}}};", ty, struct_def.body)?;
            }
            added_structs.insert(ty);
        }
    }

    Ok(())
}

pub fn build(regen_cpp: bool) -> Result<()> {
    let mod_config = Mod::read_config()?;
    let app = APPS
        .get(&mod_config.app)
        .with_context(|| format!("Application '{}' not configured", mod_config.app))?;
    let unity_install = CONFIG
        .unity_installs
        .get(&app.unity_version)
        .with_context(|| format!("Unity version '{}' not configured", app.unity_version))?;

    let unity_path = PathBuf::from(unity_install);
    let mono_path = unity_path.join("Editor/Data/MonoBleedingEdge/bin/mono");
    let il2cpp_path = unity_path.join("Editor/Data/il2cpp/build/deploy/net471/il2cpp.exe");

    let cpp_path = Path::new("./build/cpp");
    let transformed_path = Path::new("./build/transformed");
    let out_path = Path::new("./build/out");
    fs::create_dir_all(transformed_path)?;
    fs::create_dir_all(out_path)?;

    let include_path = Path::new("./build/include");
    fs::create_dir_all(include_path.join("merge"))?;
    fs::write(include_path.join("merge/codegen.h"), CODGEN_HEADER)?;

    let mut compile_command = CompileCommand::new(out_path.join(format!("{}.so", mod_config.id)));
    compile_command.add_include_path(unity_path.join("Editor/Data/il2cpp/libil2cpp"));
    compile_command.add_include_path(include_path.into());

    if regen_cpp {
        if cpp_path.exists() {
            fs::remove_dir_all(&cpp_path)?;
        }
        fs::create_dir_all(&cpp_path)?;
        dbg!(&il2cpp_path);
        Command::new(mono_path)
            // Fix for System.ConsoleDriver type initializer
            .env("TERM", "xterm")
            .arg(il2cpp_path)
            .arg("--convert-to-cpp")
            .arg("--directory=./build/Managed")
            .arg("--generatedcppdir=./build/cpp")
            .status()
            .context("il2cpp command failed")?;
    }

    let metadata_data = fs::read("./build/cpp/Data/Metadata/global-metadata.dat")
        .context("failed to read generated metadata")?;
    let metadata = il2cpp_metadata_raw::deserialize(&metadata_data)
        .context("failed to deserialize generated metadata")?;
    let mod_image_name = format!("{}.dll", mod_config.id);
    let mod_image = find_image(&metadata, &mod_image_name)?;

    let types_src = fs::read_to_string(cpp_path.join("Il2CppTypeDefinitions.c"))?;
    let gct_src = fs::read_to_string(cpp_path.join("Il2CppGenericClassTable.c"))?;
    let TypeDefinitionsFile {
        types,
        ty_name_map,
        generic_classes,
        gc_name_map,
    } = runtime_metadata::parse(&types_src, &gct_src)?;
    let gid_src = fs::read_to_string(cpp_path.join("Il2CppGenericInstDefinitions.c"))?;
    let generic_insts = runtime_metadata::parse_inst_defs(&gid_src)?;
    let gmd_src = fs::read_to_string(cpp_path.join("Il2CppGenericMethodDefinitions.c"))?;
    let generic_method_defs = runtime_metadata::parse_generic_method_defs(&gmd_src)?;
    let gmt_src = fs::read_to_string(cpp_path.join("Il2CppGenericMethodTable.c"))?;
    let generic_method_table = runtime_metadata::parse_generic_method_table(&gmt_src)?;

    let runtime_metadata = RuntimeMetadata {
        types: &types,
        ty_name_map,
        generic_classes: &generic_classes,
        gc_name_map,
        generic_insts: &generic_insts,
        generic_methods: &generic_method_defs,
        generic_method_funcs: &generic_method_table,
    };
    let mut data_builder = ModDataBuilder::new(&metadata, runtime_metadata);
    data_builder.add_mod_definitions(&mod_config.id)?;

    let mut function_usages = ModFunctionUsages::default();
    codegen::transform(
        &mut compile_command,
        &mut data_builder,
        cpp_path,
        transformed_path,
        &mod_config.id,
        &mut function_usages,
    )
    .context("error transforming codegen")?;

    let mut codegen_source_names = Vec::new();
    get_numbered_paths(&mut codegen_source_names, cpp_path, &mod_config.id);
    let mut codegen_sources = Vec::new();
    let mut extern_code_source_names = Vec::new();
    for assembly in &metadata.assemblies {
        let name = get_str(metadata.string, assembly.aname.name_index as usize)?;
        if name == mod_config.id {
            continue;
        }
        get_numbered_paths(&mut extern_code_source_names, cpp_path, name);
        let code_gen_src = fs::read_to_string(cpp_path.join(format!("{}_CodeGen.c", name)))
            .with_context(|| format!("error opening CodeGen.c file for module {}", name))?;
        codegen_sources.push((assembly.image_index, code_gen_src))
    }

    // Populate list of all external method pointers by reading CodeGen.c for all modules except the mod's
    for (image_idx, src) in &codegen_sources {
        let image = &metadata.images[*image_idx as usize];
        let method_pointers = codegen::get_methods(src)?;

        for (idx, method_pointer) in method_pointers.iter().enumerate() {
            if let Some(method_pointer) = method_pointer {
                let method_idx =
                    find_method_with_rid(data_builder.metadata, image, idx as u32 + 1)?;
                function_usages
                    .external_methods
                    .insert(method_pointer, method_idx);
            }
        }
    }

    let mut mod_source_names = Vec::new();
    get_numbered_paths(&mut mod_source_names, cpp_path, &mod_config.id);
    let mut mod_sources = Vec::new();
    for name in &mod_source_names {
        let src_path = cpp_path.join(name).with_extension("cpp");
        let new_path = transformed_path.join(name).with_extension("cpp");
        fs::copy(&src_path, &new_path)?;
        compile_command.add_source(new_path);
        mod_sources.push(fs::read_to_string(src_path)?);
    }

    // TODO: populate mod_functions by using CodeGen.c
    for src in &mod_sources {
        for line in src.lines() {
            if let Some(fn_def) = FnDecl::try_parse(line) {
                if !line.ends_with(';') {
                    function_usages.mod_functions.insert(fn_def.name);
                }
            }
        }
    }
    let mut metadata_usage_names = HashSet::new();
    let mut mod_usages = HashSet::new();
    for (src_name, src) in mod_source_names.iter().zip(mod_sources.iter()) {
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
                        metadata_usage_names.insert(fn_def.name.to_string() + src_name);
                    } else {
                        metadata_usage_names.insert(fn_def.name.to_string());
                    }
                    get_function_usages(&mut mod_usages, &mut lines);
                }
            } else if line.starts_with("inline") {
                function_usages.read_gshared_proxy(line, &mut lines);
            }
        }
    }
    function_usages.process_function_usages(mod_usages)?;

    // Read generic sources
    let mut generic_source_names = Vec::new();
    get_numbered_paths(&mut generic_source_names, cpp_path, "GenericMethods");
    get_numbered_paths(&mut generic_source_names, cpp_path, "Generics");
    let mut generic_sources = Vec::new();
    for name in &generic_source_names {
        let path = cpp_path.join(name).with_extension("cpp");
        generic_sources.push(fs::read_to_string(path)?);
    }

    // Find all struct definitions and fds
    let mut extern_code_sources = Vec::new();
    for name in extern_code_source_names {
        let src_path = cpp_path.join(format!("{}.cpp", name));
        extern_code_sources.push(fs::read_to_string(src_path)?);
    }
    let (_, struct_defs) = find_struct_defs(&extern_code_sources);

    let generic_transform_data = generics::transform(
        &mut function_usages,
        &mut metadata_usage_names,
        &generic_source_names,
        &generic_sources,
    )?;

    let (usage_fds, usages_len) = metadata_usage::transform(
        &mut compile_command,
        cpp_path,
        transformed_path,
        &mut data_builder,
        metadata_usage_names,
    )?;

    generics::write(
        generic_transform_data,
        transformed_path,
        &mut compile_command,
        &usage_fds,
        &generic_source_names,
        &generic_sources,
        &function_usages,
    )?;

    function_usages.write_external(
        &mut compile_command,
        &mod_config.id,
        &mut data_builder,
        transformed_path,
        &struct_defs,
    )?;
    let code_table_sizes = CodeTableSizes {
        generic_adjustor_thunks: function_usages.required_generic_adj_thunks.len(),
        generic_method_pointers: function_usages.required_generic_funcs.len(),
        invoker_pointers: function_usages.required_invokers.len(),
        metadata_usages: usages_len,
    };
    let mod_data = data_builder.build(&mut function_usages, code_table_sizes)?;
    dbg!(&mod_data);
    function_usages.write_invokers(&mut compile_command, transformed_path, cpp_path)?;
    function_usages.write_generic_func_table(&mut compile_command, transformed_path, cpp_path)?;
    function_usages.write_generic_adj_thunk_table(
        &mut compile_command,
        transformed_path,
        cpp_path,
    )?;
    type_sizes::transform(&mut compile_command, mod_image, cpp_path, transformed_path)?;

    fs::write(
        out_path.join(format!("{}.mmd", mod_config.id)),
        mod_data.serialize()?,
    )?;
    compile_command.run()?;

    Ok(())
}
