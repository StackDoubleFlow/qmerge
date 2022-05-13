mod clang;
mod codegen;
mod data;
mod invokers;
mod metadata_usage;
mod runtime_metadata;
mod generics;

use crate::config::{Mod, APPS, CONFIG};
use anyhow::{bail, Context, Result};
use clang::CompileCommand;
use data::{get_str, offset_len, ModDataBuilder};
use il2cpp_metadata_raw::{Il2CppImageDefinition, Metadata};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::Lines;
use std::{fs, str};

use self::data::RuntimeMetadata;
use self::invokers::ModFunctionUsages;
use self::runtime_metadata::TypeDefinitionsFile;

const CODGEN_HEADER: &str = include_str!("../include/merge/codegen.h");

struct FnDecl<'a> {
    return_ty: &'a str,
    name: &'a str,
    params: &'a str,
    inline: bool
}

impl<'a> FnDecl<'a> {
    fn try_parse(line: &'a str) -> Option<Self> {
        let (line, inline) = if let Some(line) = line.strip_prefix("IL2CPP_EXTERN_C IL2CPP_METHOD_ATTR") {
            (line, false)
        } else if let Some(line) = line.strip_prefix("IL2CPP_EXTERN_C inline IL2CPP_METHOD_ATTR") {
            (line, true)
        } else {
            (line.strip_prefix("IL2CPP_EXTERN_C inline  IL2CPP_METHOD_ATTR")?, true)
        };

        let param_start = line.find('(')?;
        let params = &line[param_start..];
        let rest = line[..param_start].trim();

        let name = rest.split_whitespace().last()?;
        let return_ty = rest[..rest.len() - name.len()].trim();

        Some(FnDecl {
            return_ty,
            name,
            params,
            inline
        })
    }
}

pub fn try_parse_call(line: &str, include_inline: bool) -> Option<&str> {
    let possible_name = if let Some(pos) = line.find("= ") {
        &line[pos + 2..]
    } else {
        line.trim()
    };
    let possible_name = possible_name.split('(').next().unwrap();
    if possible_name.ends_with("_inline") && !include_inline {
        // Inlined functions will be defined in the same file anyways
        return None;
    }
    let len = possible_name.len();
    if possible_name.len() <= 42 {
        return None;
    }
    if &possible_name[len - 42..len - 40] == "_m" {
        let valid_id = possible_name[possible_name.len() - 40..]
            .chars()
            .all(|c| ('A'..='Z').contains(&c) || ('0'..='9').contains(&c));
        if valid_id {
            return Some(possible_name);
        }
    }

    None
}

fn get_function_usages(usages: &mut HashSet<String>, lines: &mut Lines) {
    loop {
        let line = lines.next().unwrap();
        if line == "}" {
            return;
        }
        if let Some(name) = try_parse_call(line, false) {
            if !usages.contains(name) {
                usages.insert(name.to_owned());
            }
        }
    }
}

fn find_method_with_rid(
    metadata: &Metadata,
    image: &Il2CppImageDefinition,
    rid: u32,
) -> Result<usize> {
    // dbg!(image, rid);
    for type_def in &metadata.type_definitions[offset_len(image.type_start, image.type_count)] {
        for i in offset_len(type_def.method_start, type_def.method_count as u32) {
            if metadata.methods[i].token & 0x00FFFFFF == rid {
                return Ok(i);
            }
        }
    }

    let mut method_count = 0;
    for type_def in &metadata.type_definitions[offset_len(image.type_start, image.type_count)] {
        method_count += type_def.method_count;
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
        mod_image,
        cpp_path,
        transformed_path,
        &mod_config.id,
        &mut function_usages,
    )
    .context("error transforming codegen")?;

    for assembly in &metadata.assemblies {
        let name = get_str(metadata.string, assembly.aname.name_index as usize)?;
        let code_gen_src = fs::read_to_string(cpp_path.join(format!("{}_CodeGen.c", name)))
            .with_context(|| format!("error opening CodeGen.c file for module {}", name))?;
        let method_pointers = codegen::get_methods(&code_gen_src)?;
        let image = &metadata.images[assembly.image_index as usize];

        for (idx, method_pointer) in method_pointers.iter().enumerate() {
            if let Some(method_pointer) = method_pointer {
                let method_idx =
                    find_method_with_rid(data_builder.metadata, image, idx as u32 + 1)?;
                function_usages
                    .external_methods
                    .insert(method_pointer.to_string(), method_idx);
            }
        }
    }

    let mut metadata_usage_names = HashSet::new();
    let mut mod_usages = HashSet::new();
    for i in 0.. {
        let file_name = if i > 0 {
            format!("{}{}", mod_config.id, i)
        } else {
            format!("{}", mod_config.id)
        };
        let path = Path::new(&file_name).with_extension("cpp");
        let src_path = cpp_path.join(&path);
        if src_path.exists() {
            let new_path = transformed_path.join(path);
            fs::copy(&src_path, &new_path)?;
            compile_command.add_source(new_path);
            let main_source = fs::read_to_string(src_path)?;
            let mut lines = main_source.lines();
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
                            metadata_usage_names.insert(fn_def.name.to_string() + &file_name);
                        } else {
                            metadata_usage_names.insert(fn_def.name.to_string());
                        }
                        get_function_usages(&mut mod_usages, &mut lines);
                    }
                } else if line.starts_with("inline") {
                    let words = line.split_whitespace().collect::<Vec<_>>();
                    let proxy_name =
                        words[words.iter().position(|w| w.starts_with('(')).unwrap() - 1];
                    lines.next().unwrap();
                    let line = lines.next().unwrap().trim();
                    let name_start = line.find("))").unwrap() + 2;
                    let name = line[name_start..].split(')').next().unwrap();
                    function_usages
                        .generic_proxies
                        .insert(proxy_name.to_string(), name.to_string());
                }
            }
        } else {
            break;
        }
    }
    function_usages.process_function_usages(mod_usages)?;
    generics::transform(cpp_path, &mut function_usages, &mut metadata_usage_names)?;

    metadata_usage::transform(
        &mut compile_command,
        cpp_path,
        transformed_path,
        &mut data_builder,
        metadata_usage_names,
    )?;

    function_usages.write_external(
        &mut compile_command,
        &mod_config.id,
        &mut data_builder,
        transformed_path,
    )?;
    let mod_data = data_builder.build(&mut function_usages)?;
    // dbg!(&mod_data);
    function_usages.write_invokers(&mut compile_command, transformed_path, cpp_path)?;
    function_usages.write_generic_func_table(&mut compile_command, transformed_path, cpp_path)?;
    function_usages.write_generic_adj_thunk_table(
        &mut compile_command,
        transformed_path,
        cpp_path,
    )?;

    fs::write(
        out_path.join(format!("{}.mmd", mod_config.id)),
        mod_data.serialize()?,
    )?;
    compile_command.run()?;

    Ok(())
}
