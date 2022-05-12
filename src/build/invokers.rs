use crate::build::FnDecl;
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::fs;
use std::path::Path;

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
}

impl ModFunctionUsages {
    pub fn process(
        self,
        mod_id: &str,
        data_builder: &mut ModDataBuilder,
        transformed_path: &Path,
    ) -> Result<()> {
        let mut external_src = String::new();
        writeln!(external_src, "#include \"codegen/il2cpp-codegen.h\"")?;
        writeln!(external_src, "#include \"merge/codegen.h\"")?;
        writeln!(external_src)?;

        for usage in self.usages {
            if let Some(&orig_idx) = self.external_methods.get(&usage) {
                let idx = data_builder.add_method(orig_idx as u32)?;
                let decl = &self.forward_decls[&usage];
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

                writeln!(external_src, "{}\n{{\n", decl)?;
                writeln!(
                    external_src,
                    "    return (({} (*){})(merge_codegen_resolve_method(\"{}\", {})))({});",
                    fn_def.return_ty, fn_def.params, mod_id, idx, params
                )?;
                writeln!(external_src, "}}\n")?;
            }
        }

        fs::write(transformed_path.join("External.cpp"), external_src)?;

        Ok(())
    }
}
