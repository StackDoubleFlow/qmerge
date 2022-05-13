use crate::build::FnDecl;
use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::fs;
use std::path::Path;

use super::clang::CompileCommand;
use super::data::ModDataBuilder;

// TODO: Figure out lifetimes for strings and use string slices for maps
#[derive(Default)]
pub struct ModFunctionUsages {
    // Mapping from name to method def metadata idx
    pub external_methods: HashMap<String, usize>,
    pub forward_decls: HashMap<String, String>,
    pub usages: HashSet<String>,
    pub using_gshared: HashSet<String>,
    pub generic_proxies: HashMap<String, String>,

    pub required_invokers: Vec<usize>,
    pub invokers_map: HashMap<usize, usize>,

    pub required_generic_funcs: Vec<usize>,
    pub generic_func_map: HashMap<usize, usize>,

    pub required_generic_adj_thunks: Vec<usize>,
    pub generic_adj_thunk_map: HashMap<usize, usize>,
}

impl ModFunctionUsages {
    pub fn process(
        &mut self,
        compile_command: &mut CompileCommand,
        mod_id: &str,
        data_builder: &mut ModDataBuilder,
        transformed_path: &Path,
    ) -> Result<()> {
        let mut external_src = String::new();
        writeln!(external_src, "#include \"codegen/il2cpp-codegen.h\"")?;
        writeln!(external_src, "#include \"merge/codegen.h\"")?;
        writeln!(external_src)?;

        for usage in &self.usages {
            if let Some(&orig_idx) = self.external_methods.get(usage) {
                let idx = data_builder.add_method(orig_idx as u32)?;
                let decl = &self.forward_decls[usage];
                let fn_def = FnDecl::try_parse(decl).unwrap();

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

                writeln!(external_src, "{}\n{{", decl)?;
                writeln!(
                    external_src,
                    "    return (({} (*){})(merge_codegen_resolve_method(\"{}\", {})))({});",
                    fn_def.return_ty, fn_def.params, mod_id, idx, params
                )?;
                writeln!(external_src, "}}\n")?;
            }
        }

        let external_path = transformed_path.join("External.cpp");
        fs::write(&external_path, external_src)?;
        compile_command.add_source(external_path);

        Ok(())
    }

    pub fn add_invoker(&mut self, idx: usize) -> usize {
        *self.invokers_map.entry(idx).or_insert_with(|| {
            let new_idx = self.required_invokers.len();
            self.required_invokers.push(idx);
            new_idx
        })
    }

    pub fn add_generic_func(&mut self, idx: usize) -> usize {
        *self.generic_func_map.entry(idx).or_insert_with(|| {
            let new_idx = self.required_generic_funcs.len();
            self.required_generic_funcs.push(idx);
            new_idx
        })
    }

    pub fn add_generic_adj_thunk(&mut self, idx: usize) -> usize {
        *self.generic_adj_thunk_map.entry(idx).or_insert_with(|| {
            let new_idx = self.required_generic_adj_thunks.len();
            self.required_generic_adj_thunks.push(idx);
            new_idx
        })
    }

    pub fn write_invokers(
        &self,
        compile_command: &mut CompileCommand,
        transformed_path: &Path,
        cpp_path: &Path,
    ) -> Result<()> {
        let src = fs::read_to_string(cpp_path.join("Il2CppInvokerTable.cpp"))?;

        let mut invokers = Vec::new();
        let arr_start = src
            .find("const InvokerMethod g_Il2CppInvokerPointers")
            .context("could not find g_Il2CppInvokerPointers")?;
        for line in src[arr_start..].lines().skip(3) {
            if line.starts_with('}') {
                break;
            }
            let name = line.trim().trim_end_matches(',');
            invokers.push(name);
        }

        let mut keep_invokers = HashSet::new();
        for &idx in &self.required_invokers {
            keep_invokers.insert(invokers[idx]);
        }

        let mut new_src = String::new();
        let mut lines = src.lines();
        while let Some(line) = lines.next() {
            if line.starts_with("void*") {
                let name = line.split_whitespace().nth(1).context("weird fn def")?;
                let keep = keep_invokers.contains(name);
                if keep {
                    writeln!(new_src, "{}", line)?;
                }
                for line in lines.by_ref() {
                    if keep {
                        writeln!(new_src, "{}", line)?;
                    }
                    if line.is_empty() {
                        break;
                    }
                }
            } else if line.starts_with("const InvokerMethod g_Il2CppInvokerPointers") {
                writeln!(
                    new_src,
                    "const InvokerMethod g_Il2CppInvokerPointers[{}] =",
                    self.required_invokers.len()
                )?;
                writeln!(new_src, "{{")?;
                for &idx in &self.required_invokers {
                    writeln!(new_src, "    {},", invokers[idx])?;
                }
                writeln!(new_src, "}};")?;
                break;
            } else {
                writeln!(new_src, "{}", line)?;
            }
        }

        let new_path = transformed_path.join("Il2CppInvokerTable.cpp");
        fs::write(&new_path, new_src)?;
        compile_command.add_source(new_path);

        Ok(())
    }

    pub fn write_generic_func_table(
        &self,
        compile_command: &mut CompileCommand,
        transformed_path: &Path,
        cpp_path: &Path,
    ) -> Result<()> {
        let src = fs::read_to_string(cpp_path.join("Il2CppGenericMethodPointerTable.cpp"))?;

        let mut methods = Vec::new();

        if let Some(arr_start) = src.find("const Il2CppMethodPointer g_Il2CppGenericMethodPointers")
        {
            for line in src[arr_start..].lines().skip(3) {
                if line.starts_with('}') {
                    break;
                }
                let name = line
                    .trim()
                    .trim_start_matches("(Il2CppMethodPointer)&")
                    .split('/')
                    .next()
                    .context("malformed generic method pointer table")?;
                methods.push(name);
            }
        };

        let mut new_src = String::new();
        writeln!(new_src, "#include \"codegen/il2cpp-codegen.h\"")?;
        writeln!(new_src, "#include \"merge/codegen.h\"")?;
        writeln!(new_src)?;

        for &idx in &self.required_generic_funcs {
            writeln!(new_src, "IL2CPP_EXTERN_C void {} ();", methods[idx])?;
        }
        writeln!(
            new_src,
            "extern const Il2CppMethodPointer g_Il2CppGenericMethodPointers[];"
        )?;
        writeln!(
            new_src,
            "const Il2CppMethodPointer g_Il2CppGenericMethodPointers[{}] = ",
            self.required_generic_funcs.len()
        )?;
        writeln!(new_src, "{{")?;
        for &idx in &self.required_generic_funcs {
            writeln!(new_src, "    (Il2CppMethodPointer)&{},", methods[idx])?;
        }
        writeln!(new_src, "}};")?;

        let new_path = transformed_path.join("Il2CppGenericMethodPointerTable.cpp");
        fs::write(&new_path, new_src)?;
        compile_command.add_source(new_path);

        Ok(())
    }

    pub fn write_generic_adj_thunk_table(
        &self,
        compile_command: &mut CompileCommand,
        transformed_path: &Path,
        cpp_path: &Path,
    ) -> Result<()> {
        let src = fs::read_to_string(cpp_path.join("Il2CppGenericAdjustorThunkTable.cpp"))?;

        let mut methods = Vec::new();

        if let Some(arr_start) = src.find("const Il2CppMethodPointer g_Il2CppGenericAdjustorThunks")
        {
            for line in src[arr_start..].lines().skip(3) {
                if line.starts_with('}') {
                    break;
                }
                let name = line.trim().trim_end_matches(',');
                methods.push(name);
            }
        };

        let mut new_src = String::new();
        writeln!(new_src, "#include \"codegen/il2cpp-codegen.h\"")?;
        writeln!(new_src, "#include \"merge/codegen.h\"")?;
        writeln!(new_src)?;

        for &idx in &self.required_generic_adj_thunks {
            writeln!(new_src, "IL2CPP_EXTERN_C void {} ();", methods[idx])?;
        }
        writeln!(
            new_src,
            "extern const Il2CppMethodPointer g_Il2CppGenericAdjustorThunks[];"
        )?;
        writeln!(
            new_src,
            "const Il2CppMethodPointer g_Il2CppGenericAdjustorThunks[{}] = ",
            self.required_generic_adj_thunks.len()
        )?;
        writeln!(new_src, "{{")?;
        for &idx in &self.required_generic_adj_thunks {
            writeln!(new_src, "    {},", methods[idx])?;
        }
        writeln!(new_src, "}};")?;

        let new_path = transformed_path.join("Il2CppGenericAdjustorThunkTable.cpp");
        fs::write(&new_path, new_src)?;
        compile_command.add_source(new_path);

        Ok(())
    }
}
