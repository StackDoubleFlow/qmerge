use super::clang::CompileCommand;
use anyhow::{Context, Result};
use il2cpp_metadata_raw::Il2CppImageDefinition;
use std::fmt::Write;
use std::fs;
use std::path::Path;

pub fn transform(
    compile_command: &mut CompileCommand,
    image: &Il2CppImageDefinition,
    cpp_path: &Path,
    transformed_path: &Path,
) -> Result<()> {
    let src = fs::read_to_string(cpp_path.join("Il2CppCompilerCalculateTypeValues.cpp"))?;
    let mut lines = src.lines();
    let mut new_src = String::new();
    while let Some(line) = lines.next() {
        if line.starts_with("extern const Il2CppTypeDefinitionSizes") {
            let name = line.split_whitespace().nth(3).unwrap();
            let num = name
                .trim_start_matches("g_typeDefinitionSize")
                .trim_end_matches(';')
                .parse::<u32>()?;
            let def = lines.next().unwrap();
            if num >= image.type_start && num < image.type_start + image.type_count {
                writeln!(new_src, "{}", line)?;
                writeln!(new_src, "{}", def)?;
            }
        } else if line.starts_with("IL2CPP_EXTERN_C const int32_t") {
            let name = line.split_whitespace().nth(3).unwrap();
            let num_str = name
                .trim_start_matches("g_FieldOffsetTable")
                .split('[')
                .next()
                .unwrap();
            let num = num_str.parse::<u32>()?;
            if num >= image.type_start && num < image.type_start + image.type_count {
                writeln!(new_src, "{}", line)?;
                for line in lines.by_ref() {
                    writeln!(new_src, "{}", line)?;
                    if line == "};" {
                        break;
                    }
                }
            } else {
                for line in lines.by_ref() {
                    if line == "};" {
                        break;
                    }
                }
            }
        } else {
            writeln!(new_src, "{}", line)?;
        }
    }
    let new_path = transformed_path.join("Il2CppCompilerCalculateTypeValues.cpp");
    fs::write(&new_path, new_src)?;
    compile_command.add_source(new_path);

    let src = fs::read_to_string(cpp_path.join("Il2CppCompilerCalculateTypeValuesTable.cpp"))?;
    let mut new_src = String::new();
    writeln!(new_src, "#include \"codegen/il2cpp-codegen.h\"")?;
    writeln!(new_src, "#include \"merge/codegen.h\"")?;
    writeln!(new_src)?;
    for line in src.lines() {
        if line.starts_with("extern const Il2CppTypeDefinitionSizes") {
            let name = line.split_whitespace().nth(3).unwrap();
            let num = name
                .trim_start_matches("g_typeDefinitionSize")
                .trim_end_matches(';')
                .parse::<u32>()?;
            if num >= image.type_start && num < image.type_start + image.type_count {
                writeln!(new_src, "{}", line)?;
            }
        } else if let Some(name) =
            line.strip_prefix("IL2CPP_EXTERN_C_CONST int32_t g_FieldOffsetTable")
        {
            let num_str = name.split('[').next().unwrap();
            let num = num_str.parse::<u32>()?;
            if num >= image.type_start && num < image.type_start + image.type_count {
                writeln!(new_src, "{}", line)?;
            }
        }
    }

    writeln!(
        new_src,
        "IL2CPP_EXTERN_C_CONST int32_t* g_FieldOffsetTable[{}] =\n{{",
        image.type_count
    )?;
    let arr_start = src
        .find("IL2CPP_EXTERN_C_CONST int32_t* g_FieldOffsetTable")
        .context("could not find g_FieldOffsetTable")?;
    for line in src[arr_start..]
        .lines()
        .skip(2 + image.type_start as usize)
        .take(image.type_count as usize)
    {
        writeln!(new_src, "{}", line)?;
    }
    writeln!(new_src, "}};")?;

    writeln!(new_src, "IL2CPP_EXTERN_C_CONST Il2CppTypeDefinitionSizes* g_Il2CppTypeDefinitionSizesTable[{}] =\n{{", image.type_count)?;
    let arr_start = src
        .find("IL2CPP_EXTERN_C_CONST Il2CppTypeDefinitionSizes* g_Il2CppTypeDefinitionSizesTable")
        .context("could not find g_Il2CppTypeDefinitionSizesTable")?;
    for line in src[arr_start..]
        .lines()
        .skip(2 + image.type_start as usize)
        .take(image.type_count as usize)
    {
        writeln!(new_src, "{}", line)?;
    }
    writeln!(new_src, "}};")?;

    let new_path = transformed_path.join("Il2CppCompilerCalculateTypeValuesTable.cpp");
    fs::write(&new_path, new_src)?;
    compile_command.add_source(new_path);

    // (1) OverrideInterfaceMethods:
    // for each method override:
    // 1. get slot of original
    // 2. set slot in vtable to new method
    // 3. set slot of new method to slot of original

    // (2) SetupInterfaceMethods
    // for each original method in interfaces
    // 1. get slot of original (and offset with interface offset)
    // 2. if original method hasn't already been explicitly overriden
    // 3.

    Ok(())
}