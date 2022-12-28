use super::clang::CompileCommand;
use super::data::ModDataBuilder;
use super::StructDef;
use crate::build::{add_cpp_ty, FnDecl};
use color_eyre::eyre::{bail, ContextCompat, Result};
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::fs;
use std::path::Path;
use std::str::Lines;

#[derive(Default)]
pub struct ModFunctionUsages<'a> {
    // Mapping from name to method def metadata idx
    pub external_methods: HashMap<&'a str, usize>,
    pub mod_functions: HashSet<&'a str>,
    pub forward_decls: HashMap<&'a str, &'a str>,
    pub generic_proxies: HashMap<&'a str, &'a str>,

    /// Usages of the module sources, not from generic sources
    pub using_gshared: HashSet<&'a str>,
    /// from generic and module sources
    pub using_external: HashSet<&'a str>,

    /// These are from generic sources only and require fd
    pub generic_using_fns: HashSet<&'a str>,

    pub required_invokers: Vec<usize>,
    pub invokers_map: HashMap<usize, usize>,

    pub required_generic_funcs: Vec<usize>,
    pub generic_func_map: HashMap<usize, usize>,

    pub required_generic_adj_thunks: Vec<usize>,
    pub generic_adj_thunk_map: HashMap<usize, usize>,
}

impl<'a> ModFunctionUsages<'a> {
    pub fn process_function_usage(&mut self, usage: &'a str) -> Result<()> {
        if self.external_methods.contains_key(usage) {
            self.using_external.insert(usage);
        } else if let Some(gshared) = self.generic_proxies.get(usage) {
            if !self.using_gshared.contains(gshared) {
                self.using_gshared.insert(gshared);
            }
        } else if !self.mod_functions.contains(usage) {
            bail!("unable to handle function usage: {}", usage);
        }

        Ok(())
    }

    pub fn process_function_usages(&mut self, usages: HashSet<&'a str>) -> Result<()> {
        for usage in &usages {
            self.process_function_usage(usage)?;
        }

        Ok(())
    }

    pub fn parse_gshared_proxy_decl(&self, line: &'a str) -> &'a str {
        let words = line.split_whitespace().collect::<Vec<_>>();
        words[words.iter().position(|w| w.starts_with('(')).unwrap() - 1]
    }

    pub fn add_gshared_proxy(&mut self, line: &'a str, lines: &mut Lines<'a>) {
        let proxy_name = self.parse_gshared_proxy_decl(line);
        lines.next().unwrap();
        let line = lines.next().unwrap().trim();
        let name_start = line.find("))").unwrap() + 2;
        let name = line[name_start..].split(')').next().unwrap();
        self.generic_proxies.insert(proxy_name, name);
    }

    pub fn write_external(
        &mut self,
        compile_command: &mut CompileCommand,
        data_builder: &mut ModDataBuilder,
        transformed_path: &Path,
        struct_defs: &HashMap<&str, StructDef>,
    ) -> Result<()> {
        let mut external_src = String::new();

        writeln!(external_src, "#include \"codegen/il2cpp-codegen.h\"")?;
        writeln!(external_src, "#include \"merge/codegen.h\"")?;
        writeln!(external_src)?;

        writeln!(external_src, "extern const size_t g_ExternFuncCount;")?;
        writeln!(external_src, "extern void* g_MethodFixups[];")?;
        writeln!(external_src, "extern const func_lut_entry_t g_FuncLut[];")?;

        writeln!(external_src)?;

        let mut added_structs = HashSet::new();
        let mut fd_structs = HashSet::new();
        let mut added_fns = Vec::new();

        for (this_idx, external) in self.using_external.iter().enumerate() {
            let orig_idx = self.external_methods[external];
            let idx = data_builder.add_method(orig_idx as u32)?;
            let decl = &self.forward_decls[external];
            let fn_def = FnDecl::try_parse(decl).unwrap();
            add_cpp_ty(
                &mut external_src,
                fn_def.return_ty,
                struct_defs,
                &mut fd_structs,
                &mut added_structs,
            )?;

            let mut params = Vec::new();
            for param in fn_def
                .params
                .trim_start_matches('(')
                .trim_end_matches(')')
                .split(", ")
            {
                add_cpp_ty(
                    &mut external_src,
                    param,
                    struct_defs,
                    &mut fd_structs,
                    &mut added_structs,
                )?;
                let words = param.trim_start_matches("const ").split_whitespace();
                params.push(words.last().unwrap().to_string());
            }

            let params: Vec<String> = fn_def
                .params
                .trim_start_matches('(')
                .trim_end_matches(')')
                .split(',')
                .map(|param| param.split_whitespace().last())
                .collect::<Option<Vec<&str>>>()
                .unwrap()
                .into_iter()
                .map(String::from)
                .collect();
            let params = params.join(", ");

            writeln!(
                external_src,
                "__attribute__((noinline))\n{}\n__attribute__((disable_tail_calls))\n{{",
                decl
            )?;
            writeln!(
                external_src,
                "    return (({} (*){})(g_MethodFixups[{}]))({});",
                fn_def.return_ty, fn_def.params, this_idx, params
            )?;
            writeln!(external_src, "}}\n")?;

            added_fns.push((String::from(fn_def.name), idx));
        }

        // write the fixups table

        writeln!(
            external_src,
            "const size_t g_ExternFuncCount = {};",
            added_fns.len()
        )?;
        writeln!(external_src, "void* g_MethodFixups[{}] =", added_fns.len())?;
        writeln!(external_src, "{{")?;
        for _ in &added_fns {
            writeln!(external_src, "    (void*)&merge_prestub,")?;
        }
        writeln!(external_src, "}};")?;
        writeln!(external_src)?;

        // write the lookup table
        // the lookup table is const because it's going to be copied into a modloader datastructure to be sorted anyway
        writeln!(
            external_src,
            "const func_lut_entry_t g_FuncLut[{}] =",
            added_fns.len()
        )?;
        writeln!(external_src, "{{")?;
        for (fun, idx) in added_fns {
            writeln!(external_src, "    {{ (void*)&{}, {} }},", fun, idx)?;
        }
        writeln!(external_src, "}};")?;
        writeln!(external_src)?;

        let external_path = transformed_path.join("MergeExternal.cpp");
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
        funcs: &[&str],
    ) -> Result<()> {
        let mut new_src = String::new();
        writeln!(new_src, "#include \"codegen/il2cpp-codegen.h\"")?;
        writeln!(new_src, "#include \"merge/codegen.h\"")?;
        writeln!(new_src)?;

        for &idx in &self.required_generic_funcs {
            writeln!(new_src, "IL2CPP_EXTERN_C void {} ();", funcs[idx])?;
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
            writeln!(new_src, "    (Il2CppMethodPointer)&{},", funcs[idx])?;
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
    ) -> Result<HashSet<String>> {
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
        writeln!(new_src, "}};\n")?;

        let new_path = transformed_path.join("Il2CppGenericAdjustorThunkTable.cpp");
        fs::write(&new_path, new_src)?;
        compile_command.add_source(new_path);

        Ok(self
            .required_generic_adj_thunks
            .iter()
            .map(|&idx| methods[idx].to_string())
            .collect())
    }
}
