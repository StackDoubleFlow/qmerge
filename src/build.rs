use crate::config::{Mod, APPS, CONFIG};
use crate::{modules, type_definitions};
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::iter::Peekable;
use std::path::{Path, PathBuf};
use std::str::Lines;
use std::{fs, str};

fn push_line(s: &mut String, line: &str) {
    s.push_str(line);
    s.push('\n');
}

// fn offset_len<L>(offset: i32, len: L) -> std::ops::Range<usize> where L: TryInto<usize> {
//     if offset < 0 {
//         return 0..0;
//     }
//     offset as usize..offset as usize + len as usize
// }

fn strlen(data: &[u8], offset: usize) -> usize {
    let mut len = 0;
    while data[offset + len] != 0 {
        len += 1;
    }
    len
}

fn get_str(data: &[u8], offset: usize) -> Result<&str> {
    let len = strlen(data, offset);
    let str = str::from_utf8(&data[offset..offset + len])?;
    Ok(str)
}

pub fn try_parse_fn_def(line: &str) -> Option<&str> {
    let name = if line.starts_with("IL2CPP_EXTERN_C IL2CPP_METHOD_ATTR") {
        let words: Vec<&str> = line.split_whitespace().collect();
        words[3]
    } else if line.starts_with("IL2CPP_EXTERN_C inline  IL2CPP_METHOD_ATTR") {
        let words: Vec<&str> = line.split_whitespace().collect();
        words[4]
    } else {
        return None;
    };
    Some(name)
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

fn process_other(usages: &HashSet<String>, src: String) -> String {
    let mut lines = src.lines().peekable();
    let mut new_src = String::new();

    while let Some(line) = lines.next() {
        // Copy over function definitions and replace body with merge stub
        if let Some(name) = try_parse_fn_def(line) {
            if *lines.peek().unwrap() == "{" && usages.contains(name) {
                push_line(&mut new_src, line);
                new_src.push_str("{\n    // TODO: merge stub\n}");
                // for line in &mut lines {
                //     push_line(&mut new_src, line);
                //     if line == "}" {
                //         break;
                //     }
                // }
            }
        } else if line.starts_with("#include") {
            push_line(&mut new_src, line);
        }
    }

    new_src
}

pub fn build() -> Result<()> {
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

    // Command::new(mono_path)
    //     // Fix for System.ConsoleDriver type initializer
    //     .env("TERM", "xterm")
    //     .arg(il2cpp_path)
    //     .arg("--convert-to-cpp")
    //     .arg("--directory=./build/Managed")
    //     .arg("--generatedcppdir=./build/cpp")
    //     .status()
    //     .context("il2cpp command failed")?;

    let metadata_data = fs::read("./build/cpp/Data/Metadata/global-metadata.dat")
        .context("failed to read generated metadata")?;
    let metadata = il2cpp_metadata_raw::deserialize(&metadata_data)
        .context("failed to deserialize generated metadata")?;

    // let using_types = HashSet::new();
    // let image = metadata.images.iter().position(|image| get_str(metadata.string, image.name_index as usize).unwrap() == "test");
    // for image in &metadata.images {
    //     for type_def in &metadata.type_definitions[offset_len(image.type_start, image.type_count)] {
    //         for method in &metadata.methods[offset_len(type_def.method_start, type_def.method_count)] {
    //             for param in &metadata.parameters[offset_len(method.parameter_start, method.parameter_count)] {
    //                 using_types.insert(param.type_index);
    //             }
    //             using_types.insert(method.return_type);
    //         }
    //     }
    // }

    let cpp_path = Path::new("./build/cpp");
    let transformed_path = Path::new("./build/transformed");

    let types_src = fs::read_to_string(cpp_path.join("Il2CppTypeDefinitions.c"))?;
    let types = type_definitions::parse(&types_src)?;

    let mut usages = HashSet::new();

    for i in 0.. {
        let path = if i > 0 {
            format!("{}{}.cpp", mod_config.id, i)
        } else {
            format!("{}.cpp", mod_config.id)
        };
        let path = cpp_path.join(path);
        if path.exists() {
            let main_source = fs::read_to_string(path)?;
            let mut lines = main_source.lines().peekable();
            while let Some(line) = lines.next() {
                if (line.starts_with("IL2CPP_EXTERN_C IL2CPP_METHOD_ATTR")
                    || line.starts_with("IL2CPP_EXTERN_C inline  IL2CPP_METHOD_ATTR"))
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
                    let new_src = process_other(&usages, src);
                    let new_path = transformed_path.join(path);
                    fs::write(&new_path, new_src).with_context(|| {
                        format!("error writing transformed source to {}", new_path.display())
                    })?;
                } else {
                    break;
                }
            }
        }
    }

    Ok(())
}
