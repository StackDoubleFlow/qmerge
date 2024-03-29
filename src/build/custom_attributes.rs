use crate::build::parser::try_parse_call;

use super::clang::CompileCommand;
use super::convert_codegen_init_method;
use super::function_usages::ModFunctionUsages;
use super::parser::{FnDecl, SourceParser};
use color_eyre::eyre::Result;
use il2cpp_metadata_raw::Il2CppImageDefinition;
use std::collections::HashSet;
use std::fmt::Write;
use std::fs;
use std::path::Path;

pub fn transform<'src>(
    src: &'src str,
    mod_id: &str,
    compile_command: &mut CompileCommand,
    image: &Il2CppImageDefinition,
    transformed_path: &Path,
    metadata_usage_names: &mut HashSet<String>,
    function_usages: &mut ModFunctionUsages<'src>,
) -> Result<()> {
    let parser = SourceParser::new(src);

    let mut generator_names = Vec::new();
    let mut names_set = HashSet::new();

    parser
        .parse_array(
            "const CustomAttributesCacheGenerator",
            "g_AttributeGenerators",
        )?
        .for_each(|str| {
            let name = str.trim_start_matches('&');
            generator_names.push(name);
            names_set.insert(name);
            metadata_usage_names.insert(name.to_string());
        });

    let mut new_src = String::new();
    let mut lines = src.lines();
    while let Some(line) = lines.next() {
        if line.starts_with("static void") {
            let name = line
                .trim_start_matches("static void ")
                .split('(')
                .next()
                .unwrap();
            let copy = names_set.contains(name);
            if copy {
                writeln!(new_src, "{}", line)?;
            }
            loop {
                let line = lines.next().unwrap();
                if copy {
                    if let Some(name) = try_parse_call(line, false) {
                        function_usages.process_function_usage(name)?;
                    }
                    writeln!(new_src, "{}", line)?;
                }
                if line == "}" {
                    break;
                }
            }
        } else if line
            .starts_with("extern const CustomAttributesCacheGenerator g_AttributeGenerators")
        {
            writeln!(new_src, "{}", line)?;
            writeln!(
                new_src,
                "const CustomAttributesCacheGenerator g_AttributeGenerators[{}] =",
                generator_names.len()
            )?;

            writeln!(new_src, "{{")?;
            for name in generator_names {
                writeln!(new_src, "    &{},", name)?;
            }
            writeln!(new_src, "}};")?;
            break;
        } else {
            if let Some(fn_def) = FnDecl::try_parse(line) {
                if line.ends_with(';') {
                    function_usages
                        .forward_decls
                        .insert(fn_def.name, line.trim_end_matches(';'));
                }
            }
            writeln!(new_src, "{}", line)?;
        }
    }

    let new_path = transformed_path.join("Il2CppAttributes.cpp");
    fs::write(
        &new_path,
        convert_codegen_init_method(&new_src, mod_id, true)?,
    )?;
    compile_command.add_source(new_path);

    Ok(())
}
