use crate::type_definitions::{Il2CppType, Il2CppTypeData};
use anyhow::{bail, Context, Result};
use il2cpp_metadata_raw::{
    Il2CppAssemblyDefinition, Il2CppMethodDefinition, Il2CppTypeDefinition,
    Metadata,
};
use merge_data::{
    AddedAssembly, AddedEvent, AddedField, AddedImage, AddedMethod, AddedParameter, AddedProperty,
    AddedTypeDefinition, EncodedMethodIndex, MergeModData, MethodDescription, TypeDefDescription,
    TypeDescription, TypeDescriptionData,
};
use std::collections::HashMap;
use std::str;

pub fn offset_len(offset: u32, len: u32) -> std::ops::Range<usize> {
    if (offset as i32) < 0 {
        return 0..0;
    }
    offset as usize..offset as usize + len as usize
}

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

fn find_assembly<'md>(
    metadata: &'md Metadata,
    name: &str,
) -> Result<&'md Il2CppAssemblyDefinition> {
    for assembly in &metadata.assemblies {
        let assembly_name = get_str(metadata.string, assembly.aname.name_index as usize)?;
        if assembly_name == name {
            return Ok(assembly);
        }
    }
    bail!("could not find mod assembly in il2cpp output");
}

fn method_params(metadata: &Metadata, method: &Il2CppMethodDefinition) -> Vec<u32> {
    let range = offset_len(method.parameter_start, method.parameter_count as u32);
    metadata.parameters[range]
        .iter()
        .map(|p| p.type_index as u32)
        .collect()
}

struct ModDefinitions {
    added_assembly: AddedAssembly,
    added_image: AddedImage,
    added_type_defintions: Vec<AddedTypeDefinition>,
}

pub struct ModDataBuilder<'md, 'ty> {
    pub metadata: &'md Metadata<'md>,
    types: &'ty [Il2CppType<'ty>],

    type_definitions: Vec<TypeDefDescription>,
    type_def_map: HashMap<u32, usize>,

    added_types: Vec<TypeDescription>,
    type_map: HashMap<u32, usize>,

    methods: Vec<MethodDescription>,
    method_def_map: HashMap<u32, usize>,

    mod_definitions: Option<ModDefinitions>,
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
            method_def_map: HashMap::new(),
            mod_definitions: None,
        }
    }

    fn get_str(&self, offset: u32) -> Result<&str> {
        get_str(self.metadata.string, offset as usize)
    }

    fn add_mod_method(&mut self, method_def: &Il2CppMethodDefinition) -> Result<AddedMethod> {
        let mut parameters = Vec::new();
        let params_range = offset_len(
            method_def.parameter_start,
            method_def.parameter_count as u32,
        );
        for param in &self.metadata.parameters[params_range] {
            parameters.push(AddedParameter {
                name: self.get_str(param.name_index as u32)?.to_string(),
                token: param.token,
                ty: self.add_type(param.type_index as u32)?,
            })
        }

        Ok(AddedMethod {
            name: self.get_str(method_def.name_index as u32)?.to_string(),
            declaring_type: self.add_type_def(method_def.declaring_type)?,
            return_ty: self.add_type(method_def.return_type)?,
            parameters,
            token: method_def.token,
            flags: method_def.flags,
            iflags: method_def.iflags,
            slot: method_def.slot,
        })
    }

    fn add_mod_ty_def(&mut self, ty_def: &Il2CppTypeDefinition) -> Result<AddedTypeDefinition> {
        let mut fields = Vec::new();
        let fields_range = offset_len(ty_def.field_start, ty_def.field_count as u32);
        for field in &self.metadata.fields[fields_range] {
            fields.push(AddedField {
                name: self.get_str(field.name_index as u32)?.to_string(),
                token: field.token,
                ty: self.add_type(field.type_index as u32)?,
            });
        }

        let mut methods = Vec::new();
        let methods_range = offset_len(ty_def.method_start, ty_def.method_count as u32);
        for method in &self.metadata.methods[methods_range] {
            methods.push(self.add_mod_method(method)?);
        }

        let mut events = Vec::new();
        let events_range = offset_len(ty_def.event_start, ty_def.event_count as u32);
        for event in &self.metadata.events[events_range] {
            events.push(AddedEvent {
                name: self.get_str(event.name_index as u32)?.to_string(),
                ty: self.add_type(event.type_index as u32)?,
                add: self.add_method(event.add)?,
                remove: self.add_method(event.remove)?,
                raise: self.add_method(event.raise)?,
                token: event.token,
            });
        }

        let mut properties = Vec::new();
        let properties_range = offset_len(ty_def.property_start, ty_def.property_count as u32);
        for property in &self.metadata.properties[properties_range] {
            properties.push(AddedProperty {
                name: self.get_str(property.name_index as u32)?.to_string(),
                get: self.add_method(property.get)?,
                set: self.add_method(property.set)?,
                attrs: property.attrs,
                token: property.token,
            });
        }

        let mut vtable = Vec::new();
        let vtable_range = offset_len(ty_def.vtable_start, ty_def.vtable_count as u32);
        for &encoded_idx in &self.metadata.vtable_methods[vtable_range] {
            let ty = (encoded_idx & 0xE0000000) >> 29;
            let idx = encoded_idx & 0x1FFFFFFF;
            vtable.push(match ty {
                1 => EncodedMethodIndex::Il2CppClass(self.add_type_def(idx)?),
                2 => EncodedMethodIndex::Il2CppType(self.add_type(idx)?),
                3 => EncodedMethodIndex::MethodInfo(self.add_method(idx)?),
                _ => bail!("Unsupposed encoded method index with type {}", ty),
            });
        }

        let mut interface_offsets = Vec::new();
        let interface_offsets_range = offset_len(
            ty_def.interface_offsets_start,
            ty_def.interface_offsets_count as u32,
        );
        for pair in &self.metadata.interface_offsets[interface_offsets_range] {
            interface_offsets.push((self.add_type(pair.interface_type_index)?, pair.offset));
        }

        Ok(AddedTypeDefinition {
            name: self.get_str(ty_def.name_index)?.to_string(),
            namespace: self.get_str(ty_def.namespace_index)?.to_string(),
            byval_type: self.add_type(ty_def.byval_type_index)?,
            byref_type: self.add_type(ty_def.byval_type_index)?,

            declaring_type: self.add_type(ty_def.byval_type_index)?,
            parent_type: self.add_type(ty_def.byval_type_index)?,
            element_type: self.add_type(ty_def.byval_type_index)?,

            flags: ty_def.flags,

            fields,
            methods,
            events,
            properties,
            nested_types: self.metadata.nested_types
                [offset_len(ty_def.nested_types_start, ty_def.nested_type_count as u32)]
            .iter()
            .map(|&idx| self.add_type_def(idx))
            .collect::<Result<Vec<usize>>>()?,
            interfaces: self.metadata.interfaces
                [offset_len(ty_def.interfaces_start, ty_def.interfaces_count as u32)]
            .iter()
            .map(|&idx| self.add_type(idx))
            .collect::<Result<Vec<usize>>>()?,
            vtable,
            interface_offsets,

            bitfield: ty_def.bitfield,
            token: ty_def.token,
        })
    }

    pub fn add_mod_definitions(&mut self, mod_id: &str) -> Result<()> {
        let assembly = find_assembly(self.metadata, mod_id)?;
        let added_assembly = AddedAssembly {
            name: mod_id.to_string(),
            culture: self.get_str(assembly.aname.culture_index)?.to_string(),
            public_key: self.get_str(assembly.aname.public_key_index)?.to_string(),
            hash_alg: assembly.aname.hash_alg,
            hash_len: assembly.aname.hash_len,
            flags: assembly.aname.flags,
            major: assembly.aname.major,
            minor: assembly.aname.minor,
            build: assembly.aname.build,
            revision: assembly.aname.revision,
            public_key_token: assembly.aname.public_key_token,

            token: assembly.token,
        };

        let image = &self.metadata.images[assembly.image_index as usize];
        let added_image = AddedImage {
            name: self.get_str(image.name_index)?.to_string(),
            token: image.token,
        };

        let mut type_defs = Vec::new();
        let type_range = offset_len(image.type_start, image.type_count);
        for type_def in &self.metadata.type_definitions[type_range] {
            type_defs.push(self.add_mod_ty_def(type_def)?);
        }

        self.mod_definitions = Some(ModDefinitions {
            added_assembly,
            added_image,
            added_type_defintions: type_defs,
        });

        Ok(())
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
            Il2CppTypeData::TypeDefIdx(idx) => TypeDescriptionData::TypeDefIdx(self.add_type_def(idx as u32)?),
            _ => panic!("unsupported type: {:?}", ty),
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

    pub fn add_method(&mut self, idx: u32) -> Result<usize> {
        if self.method_def_map.contains_key(&idx) {
            return Ok(self.method_def_map[&idx]);
        }

        let method = &self.metadata.methods[idx as usize];
        let params = method_params(self.metadata, method);

        let desc_idx = self.methods.len();
        let desc = MethodDescription {
            name: self.get_str(method.name_index)?.to_string(),
            defining_type: self.add_type_def(method.declaring_type)?,
            params: params
                .iter()
                .map(|idx| self.add_type(*idx))
                .collect::<Result<_>>()?,
            return_ty: self.add_type(method.return_type)?,
        };
        self.methods.push(desc);
        self.method_def_map.insert(idx, desc_idx);
        Ok(desc_idx)
    }

    pub fn build(self) -> Result<MergeModData> {
        let ModDefinitions {
            added_assembly,
            added_image,
            added_type_defintions,
        } = self
            .mod_definitions
            .context("tried to build mod data without mod defintions")?;
        Ok(MergeModData {
            type_def_descriptions: self.type_definitions,
            type_descriptions: self.added_types,
            method_descriptions: self.methods,

            added_assembly,
            added_image,
            added_type_defintions,
        })
    }
}
