use crate::il2cpp_types::*;
use crate::metadata_builder::{Metadata, MetadataBuilder};
use anyhow::{bail, Context, Result};
// use il2cpp_metadata_raw::{
//     Il2CppAssemblyDefinition, Il2CppAssemblyNameDefinition, Il2CppEventDefinition,
//     Il2CppFieldDefinition, Il2CppImageDefinition, Il2CppInterfaceOffsetPair,
//     Il2CppMethodDefinition, Il2CppParameterDefinition, Il2CppPropertyDefinition,
//     Il2CppStringLiteral, Il2CppTypeDefinition, Metadata,
// };
use merge_data::{EncodedMethodIndex, MergeModData, TypeDescriptionData};
use std::collections::HashMap;
use std::lazy::SyncLazy;
use std::str;
use std::sync::Mutex;

use crate::types::Il2CppType;

static MODS: SyncLazy<Mutex<HashMap<String, Mod>>> = SyncLazy::new(Default::default);

pub fn offset_len(offset: i32, len: i32) -> std::ops::Range<usize> {
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
    metadata: &'md mut Metadata,
    // TODO: use string data from original metadata
    type_def_map: HashMap<(String, String), usize>,
    types: Vec<Il2CppType>,
}

impl<'md> ModLoader<'md> {
    pub fn new(metadata: &'md mut Metadata, types: Vec<Il2CppType>) -> Result<Self> {
        let mut type_def_map = HashMap::with_capacity(metadata.type_definitions.len());
        for (i, type_def) in metadata.type_definitions.iter().enumerate() {
            let namespace = get_str(&metadata.string, type_def.namespaceIndex as usize)?;
            let name = get_str(&metadata.string, type_def.nameIndex as usize)?;
            type_def_map.insert((namespace.to_string(), name.to_string()), i);
        }
        let string = metadata.string.to_vec();
        let string_literal_data = metadata.string_literal_data.to_vec();
        Ok(Self {
            metadata,
            type_def_map,
            types,
        })
    }

    fn get_str(&self, offset: i32) -> Result<&str> {
        get_str(&self.metadata.string, offset as usize)
    }

    fn add_str(&mut self, str: &str) -> i32 {
        let idx = self.metadata.string.len() as i32;
        self.metadata.string.copy_from_slice(str.as_bytes());
        self.metadata.string.push(0);
        idx
    }

    fn add_str_literal(&mut self, str: &str) -> i32 {
        let idx = self.metadata.string_literal_data.len() as i32;
        self.metadata.string_literal_data.copy_from_slice(str.as_bytes());
        self.metadata.string_literal_data.push(0);
        idx
    }

    pub fn load_mod(&mut self, id: &str, mod_data: &MergeModData) -> Result<()> {
        // TODO: proper error handing, fix type_def_map if loading fails
        for type_def in &mod_data.added_type_defintions {
            self.type_def_map.insert(
                (type_def.namespace.to_string(), type_def.name.to_string()),
                self.type_def_map.len(),
            );
        }

        let type_def_refs = mod_data
            .type_def_descriptions
            .iter()
            .map(|desc| {
                self.type_def_map
                    .get(&(desc.namespace.to_string(), desc.name.to_string()))
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
                    TypeDescriptionData::TypeIdx(idx) => type_refs[idx],
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
                    nameIndex: name,
                    typeIndex: type_refs[field.ty] as i32,
                    token: field.token,
                });
            }
            let methods_start = self.metadata.methods.len();
            for method in &ty_def.methods {
                let params_start = self.metadata.parameters.len();
                for param in &method.parameters {
                    let name = self.add_str(&param.name);
                    self.metadata.parameters.push(Il2CppParameterDefinition {
                        nameIndex: name,
                        token: param.token,
                        typeIndex: type_refs[param.ty] as i32,
                    });
                }
                let name = self.add_str(&method.name);
                self.metadata.methods.push(Il2CppMethodDefinition {
                    nameIndex: name as i32,
                    declaringType: type_def_refs[method.declaring_type] as i32,
                    returnType: type_refs[method.return_ty] as i32,
                    parameterStart: params_start as i32,
                    // TODO: generics
                    genericContainerIndex: -1,
                    token: method.token,
                    flags: method.flags,
                    iflags: method.iflags,
                    slot: method.slot,
                    parameterCount: method.parameters.len() as u16,
                })
            }
            let nested_types_start = self.metadata.nested_types.len();
            for &nested_ty in &ty_def.nested_types {
                self.metadata
                    .nested_types
                    .push(type_def_refs[nested_ty] as i32);
            }
            let interfaces_start = self.metadata.interfaces.len();
            for &interface in &ty_def.interfaces {
                self.metadata.interfaces.push(type_refs[interface] as i32);
            }
            let interface_offsets_start = self.metadata.interface_offsets.len();
            for &(idx, offset) in &ty_def.interface_offsets {
                self.metadata
                    .interface_offsets
                    .push(Il2CppInterfaceOffsetPair {
                        interfaceTypeIndex: type_refs[idx] as i32,
                        offset: offset as i32,
                    });
            }

            let namespace = self.add_str(&ty_def.namespace);
            let name = self.add_str(&ty_def.name);
            self.metadata.type_definitions.push(Il2CppTypeDefinition {
                nameIndex: name as i32,
                namespaceIndex: namespace as i32,
                byvalTypeIndex: type_refs[ty_def.byval_type] as i32,
                byrefTypeIndex: type_refs[ty_def.byref_type] as i32,

                declaringTypeIndex: ty_def
                    .declaring_type
                    .map(|idx| type_refs[idx] as i32)
                    .unwrap_or(-1),
                parentIndex: ty_def
                    .parent_type
                    .map(|idx| type_refs[idx] as i32)
                    .unwrap_or(-1),
                elementTypeIndex: type_refs[ty_def.element_type] as i32,

                // TODO: generics
                genericContainerIndex: -1,

                flags: ty_def.flags,

                fieldStart: fields_start as i32,
                methodStart: methods_start as i32,
                eventStart: -1,
                propertyStart: -1,
                nestedTypesStart: nested_types_start as i32,
                interfacesStart: interfaces_start as i32,
                vtableStart: -1,
                interfaceOffsetsStart: interface_offsets_start as i32,

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
            let return_ty = type_refs[method.return_ty] as i32;

            let method_range = offset_len(ty_def.methodStart, ty_def.method_count as i32);
            for ty_method_idx in method_range {
                let ty_method = &self.metadata.methods[ty_method_idx];
                if self.get_str(ty_method.nameIndex)? != method.name
                    || ty_method.returnType != return_ty
                    || ty_method.parameterCount as usize != method.params.len()
                {
                    continue;
                }
                let params_range =
                    offset_len(ty_method.parameterStart, ty_method.parameterCount as i32);
                let params_match = ty_method.parameterCount == 0
                    || self.metadata.parameters[params_range]
                        .iter()
                        .map(|param| param.typeIndex as usize)
                        .eq(method.params.iter().map(|&r| type_refs[r]));
                if params_match {
                    method_refs.push(ty_method_idx);
                    continue 'mm;
                }
            }
            bail!(
                "unresolved method reference {}.{}::{}",
                self.get_str(ty_def.namespaceIndex)?,
                self.get_str(ty_def.nameIndex)?,
                method.name
            );
        }

        for (i, ty_def) in mod_data.added_type_defintions.iter().enumerate() {
            let events_start = self.metadata.events.len();
            for event in &ty_def.events {
                let name = self.add_str(&event.name);
                self.metadata.events.push(Il2CppEventDefinition {
                    nameIndex: name as i32,
                    typeIndex: type_refs[event.ty] as i32,
                    add: method_refs[event.add] as i32,
                    remove: method_refs[event.remove] as i32,
                    raise: method_refs[event.raise] as i32,
                    token: event.token,
                })
            }
            let properties_start = self.metadata.properties.len();
            for property in &ty_def.properties {
                let name = self.add_str(&property.name);
                self.metadata.properties.push(Il2CppPropertyDefinition {
                    nameIndex: name as i32,
                    get: method_refs[property.get] as i32,
                    set: method_refs[property.set] as i32,
                    attrs: property.attrs,
                    token: property.token,
                })
            }
            let vtable_start = self.metadata.vtable_methods.len();
            for &encoded_idx in &ty_def.vtable {
                let new_eidx = match encoded_idx {
                    EncodedMethodIndex::Il2CppClass(idx) => type_refs[idx] as u32 | 0x20000000,
                    EncodedMethodIndex::Il2CppType(idx) => type_refs[idx] as u32 | 0x40000000,
                    EncodedMethodIndex::MethodInfo(idx) => method_refs[idx] as u32 | 0x60000000,
                    EncodedMethodIndex::StringLiteral(idx) => {
                        let literal_idx = self.metadata.string_literal.len();
                        let literal_data_idx = self.metadata.string_literal_data.len();
                        let str = &mod_data.added_string_literals[idx];
                        self.metadata.string_literal.push(Il2CppStringLiteral {
                            length: str.len() as u32,
                            dataIndex: literal_data_idx as i32,
                        });
                        self.add_str_literal(str);
                        literal_idx as u32 | 0xA0000000
                    }
                    EncodedMethodIndex::MethodRef(idx) => todo!(),
                };
                self.metadata.vtable_methods.push(new_eidx);
            }

            let i = i + ty_defs_start;
            let metadata_def = &mut self.metadata.type_definitions[i];
            metadata_def.eventStart = events_start as i32;
            metadata_def.propertyStart = properties_start as i32;
            metadata_def.vtableStart = vtable_start as i32;
        }

        let aname = Il2CppAssemblyNameDefinition {
            nameIndex: self.add_str(&mod_data.added_assembly.name) as i32,
            cultureIndex: self.add_str(&mod_data.added_assembly.culture) as i32,
            publicKeyIndex: self.add_str(&mod_data.added_assembly.public_key) as i32,
            hash_alg: mod_data.added_assembly.hash_alg,
            hash_len: mod_data.added_assembly.hash_len as i32,
            flags: mod_data.added_assembly.flags,
            major: mod_data.added_assembly.major as i32,
            minor: mod_data.added_assembly.minor as i32,
            build: mod_data.added_assembly.build as i32,
            revision: mod_data.added_assembly.revision as i32,
            public_key_token: mod_data.added_assembly.public_key_token,
        };
        self.metadata.assemblies.push(Il2CppAssemblyDefinition {
            imageIndex: self.metadata.images.len() as i32,
            token: mod_data.added_assembly.token,
            referencedAssemblyStart: -1,
            referencedAssemblyCount: 0,
            aname,
        });

        let image_name = self.add_str(&mod_data.added_image.name) as i32;
        self.metadata.images.push(Il2CppImageDefinition {
            nameIndex: image_name,
            assemblyIndex: self.metadata.assemblies.len() as i32 - 1,

            typeStart: ty_defs_start as i32,
            typeCount: mod_data.added_type_defintions.len() as u32,

            exportedTypeStart: -1,
            exportedTypeCount: 0,

            // TODO: is this needed?
            entryPointIndex: -1,
            token: mod_data.added_image.token,

            // TODO: custom attributes
            customAttributeStart: -1,
            customAttributeCount: 0,
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
