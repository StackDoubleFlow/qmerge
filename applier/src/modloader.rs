use anyhow::{bail, Context, Result};
use il2cpp_metadata_raw::{
    Il2CppAssemblyDefinition, Il2CppAssemblyNameDefinition, Il2CppEventDefinition,
    Il2CppFieldDefinition, Il2CppImageDefinition, Il2CppInterfaceOffsetPair,
    Il2CppMethodDefinition, Il2CppParameterDefinition, Il2CppPropertyDefinition,
    Il2CppTypeDefinition, Metadata,
};
use merge_data::{EncodedMethodIndex, MergeModData, TypeDescriptionData};
use std::collections::HashMap;
use std::lazy::SyncLazy;
use std::str;
use std::sync::Mutex;

use crate::types::Il2CppType;

static MODS: SyncLazy<Mutex<HashMap<String, Mod>>> = SyncLazy::new(Default::default);

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

pub struct Mod {
    refs: ModRefs,
}

pub struct ModRefs {
    type_def_refs: Vec<usize>,
    type_refs: Vec<usize>,
    method_refs: Vec<usize>,
}

pub struct ModLoader<'md> {
    metadata: Metadata<'md>,
    type_def_map: HashMap<(&'md str, &'md str), usize>,
    types: Vec<Il2CppType>,

    // metadata replacements
    string: Vec<u8>,
}

impl<'md> ModLoader<'md> {
    pub fn new(metadata: Metadata<'md>, types: Vec<Il2CppType>) -> Result<Self> {
        let mut type_def_map = HashMap::with_capacity(metadata.type_definitions.len());
        for (i, type_def) in metadata.type_definitions.iter().enumerate() {
            let namespace = get_str(metadata.string, type_def.namespace_index as usize)?;
            let name = get_str(metadata.string, type_def.name_index as usize)?;
            type_def_map.insert((namespace, name), i);
        }
        let string = metadata.string.to_vec();
        Ok(Self {
            metadata,
            type_def_map,
            types,

            string,
        })
    }

    fn get_str(&self, offset: u32) -> Result<&str> {
        get_str(&self.string, offset as usize)
    }

    fn add_str(&mut self, str: &str) -> i32 {
        let idx = self.string.len() as i32;
        self.string.copy_from_slice(str.as_bytes());
        self.string.push(0);
        idx
    }

    pub fn load_mod(
        &mut self,
        metadata: &mut Metadata,
        id: &str,
        mod_data: &'md MergeModData,
    ) -> Result<()> {
        // TODO: proper error handing, fix type_def_map if loading fails
        for type_def in &mod_data.added_type_defintions {
            self.type_def_map.insert(
                (&type_def.namespace, &type_def.name),
                self.type_def_map.len(),
            );
        }

        let type_def_refs = mod_data
            .type_def_descriptions
            .iter()
            .map(|desc| {
                self.type_def_map
                    .get(&(&desc.namespace, &desc.name))
                    .cloned()
                    .with_context(|| {
                        format!(
                            "unresolved type reference: {}.{}",
                            &desc.namespace, &desc.name
                        )
                    })
            })
            .collect::<Result<Vec<usize>>>()?;

        let mut type_refs = Vec::with_capacity(mod_data.type_descriptions.len());
        for ty in &mod_data.type_descriptions {
            let ty = Il2CppType {
                data: match ty.data {
                    TypeDescriptionData::TypeDefIdx(idx) => type_def_refs[idx],
                },
                attrs: ty.attrs,
                ty: ty.ty,
                num_mods: 0,
                byref: ty.by_ref,
                // TODO: pinned types
                pinned: false,
            };
            // TODO: this is probably hella slow, use stable hashset
            let idx = match self.types.iter().position(|t| t == &ty) {
                Some(idx) => idx,
                None => {
                    self.types.push(ty);
                    self.types.len() - 1
                }
            };
            type_refs.push(idx);
        }

        // Fill in everything that doesn't requre method references now
        let ty_defs_start = self.metadata.type_definitions.len();
        for ty_def in &mod_data.added_type_defintions {
            let fields_start = self.metadata.fields.len();
            for field in &ty_def.fields {
                let name = self.add_str(&field.name);
                self.metadata.fields.push(Il2CppFieldDefinition {
                    name_index: name,
                    type_index: type_refs[field.ty] as i32,
                    token: field.token,
                });
            }
            let methods_start = self.metadata.methods.len();
            for method in &ty_def.methods {
                let params_start = self.metadata.parameters.len();
                for param in &method.parameters {
                    let name = self.add_str(&param.name);
                    self.metadata.parameters.push(Il2CppParameterDefinition {
                        name_index: name,
                        token: param.token,
                        type_index: type_refs[param.ty] as i32,
                    });
                }
                let name = self.add_str(&method.name);
                self.metadata.methods.push(Il2CppMethodDefinition {
                    name_index: name as u32,
                    declaring_type: type_def_refs[method.declaring_type] as u32,
                    return_type: type_refs[method.return_ty] as u32,
                    parameter_start: params_start as u32,
                    // TODO: generics
                    generic_container_index: u32::MAX,
                    token: method.token,
                    flags: method.flags,
                    iflags: method.iflags,
                    slot: method.slot,
                    parameter_count: method.parameters.len() as u16,
                })
            }
            let nested_types_start = self.metadata.nested_types.len();
            for &nested_ty in &ty_def.nested_types {
                self.metadata
                    .nested_types
                    .push(type_def_refs[nested_ty] as u32);
            }
            let interfaces_start = self.metadata.interfaces.len();
            for &interface in &ty_def.interfaces {
                self.metadata.interfaces.push(type_refs[interface] as u32);
            }
            let interface_offsets_start = self.metadata.interface_offsets.len();
            for &(idx, offset) in &ty_def.interface_offsets {
                self.metadata
                    .interface_offsets
                    .push(Il2CppInterfaceOffsetPair {
                        interface_type_index: type_refs[idx] as u32,
                        offset,
                    });
            }

            let namespace = self.add_str(&ty_def.namespace);
            let name = self.add_str(&ty_def.name);
            self.metadata.type_definitions.push(Il2CppTypeDefinition {
                name_index: name as u32,
                namespace_index: namespace as u32,
                byval_type_index: type_refs[ty_def.byval_type] as u32,
                byref_type_index: type_refs[ty_def.byref_type] as u32,

                declaring_type_index: ty_def
                    .declaring_type
                    .map(|idx| type_refs[idx] as u32)
                    .unwrap_or(u32::MAX),
                parent_index: ty_def
                    .parent_type
                    .map(|idx| type_refs[idx] as u32)
                    .unwrap_or(u32::MAX),
                element_type_index: type_refs[ty_def.element_type] as u32,

                // TODO: generics
                generic_container_index: u32::MAX,

                flags: ty_def.flags,

                field_start: fields_start as u32,
                method_start: methods_start as u32,
                event_start: u32::MAX,
                property_start: u32::MAX,
                nested_types_start: nested_types_start as u32,
                interfaces_start: interfaces_start as u32,
                vtable_start: u32::MAX,
                interface_offsets_start: interface_offsets_start as u32,

                method_count: ty_def.methods.len() as u16,
                property_count: ty_def.properties.len() as u16,
                field_count: ty_def.fields.len() as u16,
                event_count: ty_def.events.len() as u16,
                nested_type_count: ty_def.nested_types.len() as u16,
                vtable_count: ty_def.vtable.len() as u16,
                interfaces_count: ty_def.interfaces.len() as u16,
                interface_offsets_count: ty_def.interface_offsets.len() as u16,

                bitfield: ty_def.bitfield,
                token: ty_def.token,
            })
        }

        let mut method_refs = Vec::with_capacity(mod_data.method_descriptions.len());
        'mm: for method in &mod_data.method_descriptions {
            let decl_ty_idx = type_def_refs[method.defining_type];
            let ty_def = &self.metadata.type_definitions[decl_ty_idx];
            let return_ty = type_refs[method.return_ty] as u32;

            let method_range = offset_len(ty_def.method_start, ty_def.method_count as u32);
            for ty_method_idx in method_range {
                let ty_method = &self.metadata.methods[ty_method_idx];
                if self.get_str(ty_method.name_index)? != method.name
                    || ty_method.return_type != return_ty
                    || ty_method.parameter_count as usize != method.params.len()
                {
                    continue;
                }
                let params_range =
                    offset_len(ty_method.parameter_start, ty_method.parameter_count as u32);
                let params_match = ty_method.parameter_count == 0
                    || self.metadata.parameters[params_range]
                        .iter()
                        .map(|param| param.type_index as usize)
                        .eq(method.params.iter().map(|&r| type_refs[r]));
                if params_match {
                    method_refs.push(ty_method_idx);
                    continue 'mm;
                }
            }
            bail!(
                "unresolved method reference {}.{}::{}",
                self.get_str(ty_def.namespace_index)?,
                self.get_str(ty_def.name_index)?,
                method.name
            );
        }

        for (i, ty_def) in mod_data.added_type_defintions.iter().enumerate() {
            let events_start = self.metadata.events.len();
            for event in &ty_def.events {
                let name = self.add_str(&event.name);
                self.metadata.events.push(Il2CppEventDefinition {
                    name_index: name as u32,
                    type_index: type_refs[event.ty] as u32,
                    add: method_refs[event.add] as u32,
                    remove: method_refs[event.remove] as u32,
                    raise: method_refs[event.raise] as u32,
                    token: event.token,
                })
            }
            let properties_start = self.metadata.properties.len();
            for property in &ty_def.properties {
                let name = self.add_str(&property.name);
                self.metadata.properties.push(Il2CppPropertyDefinition {
                    name_index: name as u32,
                    get: method_refs[property.get] as u32,
                    set: method_refs[property.set] as u32,
                    attrs: property.attrs,
                    token: property.token,
                })
            }
            let vtable_start = self.metadata.vtable_methods.len();
            for &encoded_idx in &ty_def.vtable {
                self.metadata.vtable_methods.push(match encoded_idx {
                    EncodedMethodIndex::Il2CppClass(idx) => type_def_refs[idx] as u32 | 0x20000000,
                    EncodedMethodIndex::Il2CppType(idx) => type_refs[idx] as u32 | 0x40000000,
                    EncodedMethodIndex::MethodInfo(idx) => method_refs[idx] as u32 | 0x60000000,
                });
            }

            let i = i + ty_defs_start;
            let metadata_def = &mut self.metadata.type_definitions[i];
            metadata_def.event_start = events_start as u32;
            metadata_def.property_start = properties_start as u32;
            metadata_def.vtable_start = vtable_start as u32;
        }

        metadata.assemblies.push(Il2CppAssemblyDefinition {
            image_index: metadata.images.len() as u32,
            token: mod_data.added_assembly.token,
            referenced_assembly_start: u32::MAX,
            referenced_assembly_count: 0,
            aname: Il2CppAssemblyNameDefinition {
                name_index: self.add_str(&mod_data.added_assembly.name) as u32,
                culture_index: self.add_str(&mod_data.added_assembly.culture) as u32,
                public_key_index: self.add_str(&mod_data.added_assembly.public_key) as u32,
                hash_alg: mod_data.added_assembly.hash_alg,
                hash_len: mod_data.added_assembly.hash_len,
                flags: mod_data.added_assembly.flags,
                major: mod_data.added_assembly.major,
                minor: mod_data.added_assembly.minor,
                build: mod_data.added_assembly.build,
                revision: mod_data.added_assembly.revision,
                public_key_token: mod_data.added_assembly.public_key_token,
            },
        });

        metadata.images.push(Il2CppImageDefinition {
            name_index: self.add_str(&mod_data.added_image.name) as u32,
            assembly_index: self.metadata.assemblies.len() as u32 - 1,

            type_start: ty_defs_start as u32,
            type_count: mod_data.added_type_defintions.len() as u32,

            exported_type_start: u32::MAX,
            exported_type_count: u32::MAX,

            // TODO: is this needed?
            entry_point_index: u32::MAX,
            token: mod_data.added_image.token,

            // TODO: custom attributes
            custom_attribute_start: u32::MAX,
            custom_attribute_count: u32::MAX,
        });

        MODS.lock().unwrap().insert(
            id.to_string(),
            Mod {
                refs: ModRefs {
                    type_def_refs,
                    type_refs,
                    method_refs,
                },
            },
        );

        Ok(())
    }
}
