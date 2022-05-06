mod clang;
mod codegen;
mod data;
mod modules;
mod type_definitions;

use crate::config::{Mod, APPS, CONFIG};
use anyhow::{bail, Context, Result};
use clang::CompileCommand;
use data::{get_str, offset_len, ModDataBuilder};
use il2cpp_metadata_raw::{Il2CppImageDefinition, Metadata};
use modules::CodeGenModule;
use std::collections::HashSet;
use std::fmt::Write;
use std::iter::Peekable;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::Lines;
use std::{fs, str};

const CODGEN_HEADER: &str = include_str!("../include/merge/codegen.h");

struct FnDef<'a> {
    return_ty: &'a str,
    name: &'a str,
    params: &'a str,
}

impl<'a> FnDef<'a> {
    fn try_parse(line: &'a str) -> Option<Self> {
        let line = if let Some(line) = line.strip_prefix("IL2CPP_EXTERN_C IL2CPP_METHOD_ATTR") {
            line
        } else if let Some(line) = line.strip_prefix("IL2CPP_EXTERN_C inline IL2CPP_METHOD_ATTR") {
            line
        } else {
            line.strip_prefix("IL2CPP_EXTERN_C inline  IL2CPP_METHOD_ATTR")?
        };

        let param_start = line.find('(')?;
        let params = &line[param_start..];
        let rest = line[..param_start].trim();

        let name = rest.split_whitespace().last()?;
        let return_ty = rest[..rest.len() - name.len()].trim();

        Some(FnDef {
            return_ty,
            name,
            params,
        })
    }
}

pub fn try_parse_call(line: &str) -> Option<&str> {
    let words: Vec<&str> = line.trim_start().split_whitespace().collect();
    if words.is_empty() {
        return None;
    }

    let possible_name = if words.len() > 3 && words[2] == "=" {
        // Store return into new variable
        words[3]
    } else if words.len() > 2 && words[1] == "=" {
        // Store return into existing variable
        words[2]
    } else {
        // Don't store return
        words[0]
    };
    let possible_name = possible_name.split('(').next().unwrap();
    let possible_name = possible_name.trim_end_matches("_inline");
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

type PeekableLines<'a> = Peekable<Lines<'a>>;

fn get_function_usages(usages: &mut HashSet<String>, lines: &mut PeekableLines) {
    loop {
        let line = lines.next().unwrap();
        if line == "}" {
            return;
        }
        if let Some(name) = try_parse_call(line) {
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
    for type_def in &metadata.type_definitions[offset_len(image.type_start, image.type_count)] {
        for i in offset_len(type_def.method_start, type_def.method_count as u32) {
            if metadata.methods[i].token & 0x00FFFFFF == rid {
                return Ok(i);
            }
        }
    }

    bail!("could not find method with rid {}", rid);
}

fn process_other(
    usages: &HashSet<String>,
    mod_id: &str,
    src: String,
    image: &Il2CppImageDefinition,
    module: &CodeGenModule,
    data_builder: &mut ModDataBuilder,
) -> Result<String> {
    let mut lines = src.lines().peekable();
    let mut new_src = String::new();

    let mut add_method = |cpp_name: &str| -> Result<usize> {
        let method_rid = module
            .methods
            .iter()
            .position(|n| n == &Some(cpp_name))
            .context("could not find method in module")? as u32;
        let method_idx = find_method_with_rid(data_builder.metadata, image, method_rid)?;
        data_builder.add_method(method_idx as u32)
    };

    while let Some(line) = lines.next() {
        // Copy over function definitions and replace body with merge stub
        if let Some(fn_def) = FnDef::try_parse(line) {
            if *lines.peek().unwrap() == "{" && usages.contains(fn_def.name) {
                writeln!(new_src, "\n{}", line)?;
                let idx = add_method(fn_def.name)?;
                let params = fn_def.params.trim_start_matches('(').trim_end_matches(')');
                let params: Vec<String> = params
                    .split(',')
                    .map(|param| param.split_whitespace().last())
                    .collect::<Option<Vec<&str>>>()
                    .unwrap()
                    .into_iter()
                    .map(String::from)
                    .collect();
                let params = params.join(", ");

                writeln!(new_src, "{{")?;
                writeln!(
                    new_src,
                    "    return (({} (*){})(merge_codegen_resolve_method(\"{}\", {})))({});",
                    fn_def.return_ty, fn_def.params, mod_id, idx, params
                )?;
                writeln!(new_src, "}}")?;
            }
        }
    }

    if !new_src.is_empty() {
        new_src.insert_str(0, "#include \"codegen/il2cpp-codegen.h\"\n");
        new_src.insert_str(0, "#include \"merge/codegen.h\"\n");
    }

    Ok(new_src)
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
    fs::create_dir_all(cpp_path)?;
    fs::create_dir_all(transformed_path)?;
    fs::create_dir_all(out_path)?;

    let include_path = Path::new("./build/include");
    fs::create_dir_all(include_path.join("merge"))?;
    fs::write(include_path.join("merge/codegen.h"), CODGEN_HEADER)?;

    let mut compile_command = CompileCommand::new(out_path.join(format!("{}.so", mod_config.id)));
    compile_command.add_include_path(unity_path.join("Editor/Data/il2cpp/libil2cpp"));
    compile_command.add_include_path(include_path.into());

    if regen_cpp {
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

    codegen::transform(
        &mut compile_command,
        cpp_path,
        transformed_path,
        &mod_config.id,
    )
    .context("error transforming codegen")?;

    let metadata_data = fs::read("./build/cpp/Data/Metadata/global-metadata.dat")
        .context("failed to read generated metadata")?;
    let metadata = il2cpp_metadata_raw::deserialize(&metadata_data)
        .context("failed to deserialize generated metadata")?;

    let types_src = fs::read_to_string(cpp_path.join("Il2CppTypeDefinitions.c"))?;
    let types = type_definitions::parse(&types_src)?;
    let mut data_builder = ModDataBuilder::new(&metadata, &types);
    data_builder.add_mod_definitions(&mod_config.id)?;

    let mut usages = HashSet::new();

    for i in 0.. {
        let path = if i > 0 {
            format!("{}{}.cpp", mod_config.id, i)
        } else {
            format!("{}.cpp", mod_config.id)
        };
        let src_path = cpp_path.join(&path);
        if src_path.exists() {
            let new_path = transformed_path.join(path);
            fs::copy(&src_path, &new_path)?;
            compile_command.add_source(new_path);
            let main_source = fs::read_to_string(src_path)?;
            let mut lines = main_source.lines().peekable();
            while let Some(line) = lines.next() {
                if (line.starts_with("IL2CPP_EXTERN_C IL2CPP_METHOD_ATTR")
                    || line.starts_with("IL2CPP_EXTERN_C inline  IL2CPP_METHOD_ATTR")
                    || line.starts_with("IL2CPP_EXTERN_C inline IL2CPP_METHOD_ATTR"))
                    && *lines.peek().unwrap() == "{"
                {
                    lines.next().unwrap();
                    get_function_usages(&mut usages, &mut lines);
                }
            }
        } else {
            break;
        }
    }

    for assembly in &metadata.assemblies {
        let name = get_str(metadata.string, assembly.aname.name_index as usize)?;
        let code_gen_src = fs::read_to_string(cpp_path.join(format!("{}_CodeGen.c", name)))
            .with_context(|| format!("error opening CodeGen.c file for module {}", name))?;
        let module = modules::parse(&code_gen_src)?;
        let image = &metadata.images[assembly.image_index as usize];
        if name != mod_config.id {
            for i in 0.. {
                let path = if i > 0 {
                    format!("{}{}.cpp", name, i)
                } else {
                    format!("{}.cpp", name)
                };
                let src_path = cpp_path.join(&path);
                if src_path.exists() {
                    let src = fs::read_to_string(src_path)?;
                    let new_src = process_other(
                        &usages,
                        &mod_config.id,
                        src,
                        image,
                        &module,
                        &mut data_builder,
                    )?;
                    if !new_src.is_empty() {
                        let new_path = transformed_path.join(path);
                        fs::write(&new_path, new_src).with_context(|| {
                            format!("error writing transformed source to {}", new_path.display())
                        })?;
                        compile_command.add_source(new_path);
                    }
                } else {
                    break;
                }
            }
        }
    }

    let mod_data = dbg!(data_builder.build()?);

    fs::write(
        out_path.join(format!("{}.mmd", mod_config.id)),
        mod_data.serialize()?,
    )?;
    compile_command.run()?;

    Ok(())
}
