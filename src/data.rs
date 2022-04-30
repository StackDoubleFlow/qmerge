use crate::type_definitions::{Il2CppType, Il2CppTypeData};
use anyhow::{bail, Result};
use il2cpp_metadata_raw::Metadata;
use merge_data::{
    MergeModData, MethodDescription, TypeDefDescription, TypeDescription, TypeDescriptionData,
};
use std::collections::HashMap;
use std::str;

fn strlen(data: &[u8], offset: usize) -> usize {
    let mut len = 0;
    while data[offset + len] != 0 {
        len += 1;
    }
    len
}

pub fn get_str(data: &[u8], offset: usize) -> Result<&str> {
    let len = strlen(data, offset);
    let str = str::from_utf8(&data[offset..offset + len])?;
    Ok(str)
}

pub struct ModDataBuilder<'md, 'ty> {
    pub metadata: &'md Metadata<'md>,
    types: &'ty [Il2CppType<'ty>],

    type_definitions: Vec<TypeDefDescription>,
    type_def_map: HashMap<u32, usize>,

    added_types: Vec<TypeDescription>,
    type_map: HashMap<u32, usize>,

    methods: Vec<MethodDescription>,
    // I'm not sure why methods are duplicated sometimes
    // method_map: HashMap<u32, usize>,
}

impl<'md, 'ty> ModDataBuilder<'md, 'ty> {
    pub fn new(metadata: &'md Metadata, types: &'ty [Il2CppType]) -> Self {
        ModDataBuilder {
            metadata,
            types,
            type_definitions: Vec::new(),
            type_def_map: HashMap::new(),
            added_types: Vec::new(),
            type_map: HashMap::new(),
            methods: Vec::new(),
        }
    }

    fn add_type_def(&mut self, idx: u32) -> Result<usize> {
        if self.type_def_map.contains_key(&idx) {
            return Ok(self.type_def_map[&idx]);
        }

        let type_def = &self.metadata.type_definitions[idx as usize];
        let name = get_str(self.metadata.string, type_def.name_index as usize)?;
        let namespace = get_str(self.metadata.string, type_def.namespace_index as usize)?;

        let desc_idx = self.type_definitions.len();
        let desc = TypeDefDescription {
            name: name.to_owned(),
            namespace: namespace.to_owned(),
        };
        self.type_definitions.push(desc);
        self.type_def_map.insert(idx, desc_idx);
        Ok(desc_idx)
    }

    fn add_type(&mut self, idx: u32) -> Result<usize> {
        if self.type_map.contains_key(&idx) {
            return Ok(self.type_map[&idx]);
        }

        let ty = &self.types[idx as usize];
        let data = match ty.data {
            Il2CppTypeData::Idx(idx) => TypeDescriptionData::TypeDefIdx(idx),
            _ => bail!("unsupported type: {:?}", ty),
        };

        let desc_idx = self.added_types.len();
        let desc = TypeDescription {
            data,
            attrs: ty.attrs,
            by_ref: ty.byref,
        };
        self.added_types.push(desc);
        self.type_map.insert(idx, desc_idx);
        Ok(desc_idx)
    }

    pub fn add_method(
        &mut self,
        name: &str,
        decl_ty_def: u32,
        params: &[u32],
        return_ty: u32,
    ) -> Result<usize> {
        let desc_idx = self.methods.len();
        let desc = MethodDescription {
            name: name.to_owned(),
            defining_type: self.add_type_def(decl_ty_def)?,
            params: params
                .iter()
                .map(|idx| self.add_type(*idx))
                .collect::<Result<_>>()?,
            return_ty: self.add_type(return_ty)?,
        };
        self.methods.push(desc);
        Ok(desc_idx)
    }

    pub fn build(self) -> MergeModData {
        MergeModData {
            type_def_descriptions: self.type_definitions,
            type_descriptions: self.added_types,
            method_descriptions: self.methods,
        }
    }
}
