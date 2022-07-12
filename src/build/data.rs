use super::function_usages::ModFunctionUsages;
use super::runtime_metadata::{
    GenericClass, GenericMethodSpec, Il2CppType, Il2CppTypeData, Il2CppTypeEnum, SourceGenericInst,
    SrcGenericMethodFuncs,
};
use crate::config::Mod;
use anyhow::{bail, Context, Result};
use il2cpp_metadata_raw::{
    Il2CppAssemblyDefinition, Il2CppGenericContainer, Il2CppMethodDefinition, Il2CppTypeDefinition,
    Metadata,
};
use merge_data::{
    AddedAssembly, AddedEvent, AddedField, AddedGenericContainer, AddedGenericParameter,
    AddedImage, AddedMetadataUsagePair, AddedMethod, AddedParameter, AddedProperty,
    AddedTypeDefinition, CodeTableSizes, CustomAttributeTypeRange, EncodedMethodIndex,
    GenericClassInst, GenericContainerOwner, GenericContext, GenericInst, GenericMethodFunctions,
    GenericMethodInst, ImageDescription, MergeModData, MethodDescription, TypeDefDescription,
    TypeDescription, TypeDescriptionData,
};
use std::collections::{HashMap, HashSet};
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

pub struct RuntimeMetadata<'a> {
    pub types: &'a [Il2CppType<'a>],
    pub ty_name_map: HashMap<&'a str, usize>,

    pub generic_classes: &'a [GenericClass<'a>],
    pub gc_name_map: HashMap<&'a str, usize>,

    pub generic_insts: &'a [SourceGenericInst<'a>],
    pub gi_name_map: HashMap<&'a str, usize>,

    pub generic_methods: &'a [GenericMethodSpec],
    pub generic_method_funcs: &'a [SrcGenericMethodFuncs],
}

struct ModDefinitions {
    added_assembly: AddedAssembly,
    added_image: AddedImage,
    added_type_defintions: Vec<AddedTypeDefinition>,
    added_ca_ranges: Vec<CustomAttributeTypeRange>,
}

pub struct ModDataBuilder<'md, 'ty> {
    pub metadata: &'md Metadata<'md>,
    runtime_metadata: RuntimeMetadata<'ty>,

    images: Vec<ImageDescription>,
    image_map: HashMap<u32, usize>,

    type_definitions: Vec<TypeDefDescription>,
    type_def_map: HashMap<u32, usize>,

    added_types: Vec<TypeDescription>,
    type_map: HashMap<u32, usize>,

    methods: Vec<MethodDescription>,
    method_def_map: HashMap<u32, usize>,

    generic_methods: Vec<GenericMethodInst>,
    generic_method_map: HashMap<u32, usize>,
    generic_funcs: Option<Vec<GenericMethodFunctions>>,

    generic_classes: Vec<GenericClassInst>,
    generic_class_map: HashMap<u32, usize>,

    generic_insts: Vec<GenericInst>,
    generic_inst_map: HashMap<u32, usize>,

    mod_definitions: Option<ModDefinitions>,
    added_usage_lists: Vec<Vec<AddedMetadataUsagePair>>,
    added_string_literals: Vec<String>,
}

impl<'md, 'ty> ModDataBuilder<'md, 'ty> {
    pub fn new(metadata: &'md Metadata, runtime_metadata: RuntimeMetadata<'ty>) -> Self {
        ModDataBuilder {
            metadata,
            runtime_metadata,
            images: Vec::new(),
            image_map: HashMap::new(),
            type_definitions: Vec::new(),
            type_def_map: HashMap::new(),
            added_types: Vec::new(),
            type_map: HashMap::new(),
            methods: Vec::new(),
            method_def_map: HashMap::new(),
            generic_methods: Vec::new(),
            generic_method_map: HashMap::new(),
            generic_funcs: None,
            generic_classes: Vec::new(),
            generic_class_map: HashMap::new(),
            generic_insts: Vec::new(),
            generic_inst_map: HashMap::new(),
            mod_definitions: None,
            added_usage_lists: Vec::new(),
            added_string_literals: Vec::new(),
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
            });
        }

        Ok(AddedMethod {
            name: self.get_str(method_def.name_index as u32)?.to_string(),
            declaring_type: self.add_type_def(method_def.declaring_type)?,
            return_ty: self.add_type(method_def.return_type)?,
            parameters,
            generic_container: self.add_generic_container(method_def.generic_container_index)?,
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
            vtable.push(self.add_encoded(encoded_idx)?);
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
            byref_type: self.add_type(ty_def.byref_type_index)?,

            declaring_type_def: self.add_decl_ty(ty_def.declaring_type_index)?,
            declaring_type: self.add_type_optional(ty_def.declaring_type_index)?,
            parent_type: self.add_type_optional(ty_def.parent_index)?,
            element_type: self.add_type(ty_def.element_type_index)?,

            generic_container: self.add_generic_container(ty_def.generic_container_index)?,

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

        let mut ca_ranges = Vec::new();
        let attributes_range =
            offset_len(image.custom_attribute_start, image.custom_attribute_count);
        for attributes_info in &self.metadata.attributes_info[attributes_range] {
            let mut types = Vec::new();
            let types_range = offset_len(attributes_info.start, attributes_info.count);
            for &ty_idx in &self.metadata.attribute_types[types_range] {
                types.push(self.add_type(ty_idx)?)
            }
            ca_ranges.push(CustomAttributeTypeRange {
                token: attributes_info.token,
                types,
            });
        }

        self.mod_definitions = Some(ModDefinitions {
            added_assembly,
            added_image,
            added_type_defintions: type_defs,
            added_ca_ranges: ca_ranges,
        });

        Ok(())
    }

    pub fn add_image(&mut self, idx: u32) -> Result<usize> {
        if self.image_map.contains_key(&idx) {
            return Ok(self.image_map[&idx]);
        }

        let image = &self.metadata.images[idx as usize];
        let name = get_str(self.metadata.string, image.name_index as usize)?;

        let desc_idx = self.images.len();
        let desc = ImageDescription {
            name: name.to_owned(),
        };
        self.images.push(desc);
        self.image_map.insert(idx, desc_idx);
        Ok(desc_idx)
    }

    fn add_decl_ty(&mut self, idx: u32) -> Result<Option<usize>> {
        Ok(match idx as i32 {
            -1 => None,
            _ => Some({
                let ty = &self.runtime_metadata.types[idx as usize];
                match ty.data {
                    Il2CppTypeData::TypeDefIdx(idx) => {
                        self.add_type_def(idx as u32)?
                    }
                    // TODO: Is this possible?
                    _ => todo!("declaing type {:?}", ty.data)
                }
            })
        })
    }

    pub fn add_type_def(&mut self, idx: u32) -> Result<usize> {
        if self.type_def_map.contains_key(&idx) {
            return Ok(self.type_def_map[&idx]);
        }

        let type_def = &self.metadata.type_definitions[idx as usize];
        let image_idx = self
            .metadata
            .images
            .iter()
            .position(|image| image.type_start <= idx && image.type_start + image.type_count > idx)
            .context("could not find image type def belongs to")? as u32;
        let name = get_str(self.metadata.string, type_def.name_index as usize)?;
        let namespace = get_str(self.metadata.string, type_def.namespace_index as usize)?;

        let desc = TypeDefDescription {
            image: self.add_image(image_idx)?,
            decl_type: self.add_decl_ty(type_def.declaring_type_index)?,
            name: name.to_owned(),
            namespace: namespace.to_owned(),
        };
        self.type_definitions.push(desc);
        let desc_idx = self.type_definitions.len() - 1;
        self.type_def_map.insert(idx, desc_idx);
        Ok(desc_idx)
    }

    fn add_type_optional(&mut self, idx: u32) -> Result<Option<usize>> {
        Ok(if idx as i32 == -1 {
            None
        } else {
            Some(self.add_type(idx)?)
        })
    }

    pub fn add_type(&mut self, idx: u32) -> Result<usize> {
        if self.type_map.contains_key(&idx) {
            return Ok(self.type_map[&idx]);
        }
        let ty = &self.runtime_metadata.types[idx as usize];
        let data = match ty.data {
            Il2CppTypeData::TypeDefIdx(idx) => {
                TypeDescriptionData::TypeDefIdx(self.add_type_def(idx as u32)?)
            }
            Il2CppTypeData::GenericParamIdx(idx) => {
                let param = &self.metadata.generic_parameters[idx];
                let gc_idx = param.owner_index;
                let gc = &self.metadata.generic_containers[gc_idx as usize];

                let owner = match ty.ty {
                    Il2CppTypeEnum::Var => {
                        GenericContainerOwner::Class(self.add_type_def(gc.owner_index)?)
                    }
                    Il2CppTypeEnum::Mvar => {
                        // These get fixed up later in `fixup_types`
                        GenericContainerOwner::Method(gc.owner_index as usize)
                    }
                    _ => bail!(
                        "Il2CppType has data type GenericParamIdx but is not a generic parameter"
                    ),
                };
                TypeDescriptionData::GenericParam(owner, param.num)
            }
            Il2CppTypeData::Il2CppType(name) => {
                let idx = self.runtime_metadata.ty_name_map[name];
                TypeDescriptionData::TypeIdx(self.add_type(idx as u32)?)
            }
            Il2CppTypeData::Il2CppArrayType(_) => todo!("non-sz arrays"),
            Il2CppTypeData::Il2CppGenericClass(name) => {
                let idx = self.runtime_metadata.gc_name_map[name];
                TypeDescriptionData::GenericClass(self.add_generic_class(idx as u32)?)
            }
        };

        let desc_idx = self.added_types.len();
        let desc = TypeDescription {
            data,
            attrs: ty.attrs,
            ty: ty.ty.get_id(),
            by_ref: ty.byref,
            pinned: ty.pinned,
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
            num_gen_params: if method.generic_container_index != u32::MAX {
                self.metadata.generic_containers[method.generic_container_index as usize].type_argc
            } else {
                0
            },
        };
        self.methods.push(desc);
        self.method_def_map.insert(idx, desc_idx);
        Ok(desc_idx)
    }

    fn add_gc_owner(&mut self, gc: &Il2CppGenericContainer) -> Result<GenericContainerOwner> {
        Ok(if gc.is_method != 0 {
            GenericContainerOwner::Method(self.add_method(gc.owner_index)?)
        } else {
            GenericContainerOwner::Class(self.add_type_def(gc.owner_index)?)
        })
    }

    fn add_generic_container(&mut self, idx: u32) -> Result<Option<AddedGenericContainer>> {
        if idx as i32 == -1 {
            return Ok(None);
        }

        let gc = &self.metadata.generic_containers[idx as usize];
        let owner = self.add_gc_owner(gc)?;
        let mut params = Vec::new();
        let params_range = offset_len(gc.generic_parameter_start, gc.type_argc);
        for param in &self.metadata.generic_parameters[params_range] {
            let mut constraints = Vec::new();
            let constraints_range = offset_len(
                param.constraints_start as u32,
                param.constraints_count as u32,
            );
            for &constraint in &self.metadata.generic_parameter_constraints[constraints_range] {
                constraints.push(self.add_type(constraint)?);
            }
            params.push(AddedGenericParameter {
                name: self.get_str(param.name_index)?.to_string(),
                constraints,
                flags: param.flags,
            });
        }
        Ok(Some(AddedGenericContainer {
            owner,
            parameters: params,
        }))
    }

    fn fixup_types(&mut self) -> Result<()> {
        for idx in 0..self.added_types.len() {
            let (owner, num) = match self.added_types[idx].data {
                TypeDescriptionData::GenericParam(GenericContainerOwner::Method(owner), num) => {
                    (owner, num)
                }
                _ => continue,
            };
            self.added_types[idx].data = TypeDescriptionData::GenericParam(
                // We don't have to worry about this adding new types that we would have to fix up
                // because if this generic param type is in the list, it's from a method we already added
                GenericContainerOwner::Method(self.add_method(owner as u32)?),
                num,
            );
        }
        Ok(())
    }

    pub fn process_generic_funcs(&mut self, function_usages: &mut ModFunctionUsages) {
        let mut generic_funcs = Vec::new();
        for i in 0..self.generic_methods.len() {
            let orig_idx = self
                .generic_method_map
                .iter()
                .find_map(|(&k, &v)| if v == i { Some(k) } else { None })
                .unwrap() as usize;
            let funcs = self
                .runtime_metadata
                .generic_method_funcs
                .iter()
                .find(|fs| fs.generic_method_idx == orig_idx)
                .unwrap();

            generic_funcs.push(GenericMethodFunctions {
                generic_method: i,
                method_idx: function_usages.add_generic_func(funcs.method_idx),
                invoker_idx: function_usages.add_invoker(funcs.invoker_idx),
                adjustor_thunk_idx: funcs
                    .adjustor_thunk_idx
                    .map(|idx| function_usages.add_generic_adj_thunk(idx)),
            })
        }
        self.generic_funcs = Some(generic_funcs);
    }

    pub fn build(mut self, config: &Mod, code_table_sizes: CodeTableSizes) -> Result<MergeModData> {
        self.fixup_types()?;
        let ModDefinitions {
            added_assembly,
            added_image,
            added_type_defintions,
            added_ca_ranges,
        } = self
            .mod_definitions
            .context("tried to build mod data without mod defintions")?;
        Ok(MergeModData {
            code_table_sizes,

            image_descriptions: self.images,
            type_def_descriptions: self.type_definitions,
            type_descriptions: self.added_types,
            method_descriptions: self.methods,

            dependencies: config.dependencies.clone(),
            load_before: config.load_before.clone(),
            load_after: config.load_after.clone(),

            added_assembly,
            added_image,
            added_type_defintions,
            added_usage_lists: self.added_usage_lists,
            added_string_literals: self.added_string_literals,
            added_ca_ranges,

            generic_instances: self.generic_insts,
            generic_method_insts: self.generic_methods,
            generic_method_funcs: self
                .generic_funcs
                .context("tried to build mod data without processing generic funcs")?,
            generic_class_insts: self.generic_classes,
        })
    }

    fn add_string_literal(&mut self, idx: u32) -> Result<usize> {
        let literal = &self.metadata.string_literal[idx as usize];
        let data_range = offset_len(literal.data_index, literal.length);
        let data = &self.metadata.string_literal_data[data_range];
        let str = String::from_utf8(data.to_vec())
            .context("error reading string literal data as utf8")?;
        let new_idx = self.added_string_literals.len();
        self.added_string_literals.push(str);
        Ok(new_idx)
    }

    pub fn add_metadata_usage_range(
        &mut self,
        usage_map: &mut HashMap<u32, usize>,
        usage_list: &mut Vec<usize>,
        idx: u32,
    ) -> Result<usize> {
        let list = &self.metadata.metadata_usage_lists[idx as usize];

        let mut new_list = Vec::new();
        let usage_range = offset_len(list.start, list.count);
        for pair in &self.metadata.metadata_usage_pairs[usage_range] {
            let dest = *usage_map.entry(pair.destination_index).or_insert_with(|| {
                let idx = usage_list.len();
                usage_list.push(pair.destination_index as usize);
                idx
            });
            new_list.push(AddedMetadataUsagePair {
                dest,
                source: self.add_encoded(pair.encoded_source_index)?,
            })
        }
        let usage_list_idx = self.added_usage_lists.len();
        self.added_usage_lists.push(new_list);

        Ok(usage_list_idx)
    }

    fn add_generic_inst(&mut self, idx: u32) -> Result<usize> {
        if self.generic_inst_map.contains_key(&idx) {
            return Ok(self.generic_inst_map[&idx]);
        }

        let generic_inst = &self.runtime_metadata.generic_insts[idx as usize];

        let desc_idx = self.generic_insts.len();
        let desc = GenericInst {
            types: generic_inst
                .types
                .iter()
                .map(|ty_name| self.add_type(self.runtime_metadata.ty_name_map[ty_name] as u32))
                .collect::<Result<Vec<_>>>()?,
        };
        self.generic_insts.push(desc);
        self.generic_inst_map.insert(idx, desc_idx);
        Ok(desc_idx)
    }

    fn add_generic_class(&mut self, idx: u32) -> Result<usize> {
        if self.generic_class_map.contains_key(&idx) {
            return Ok(self.generic_class_map[&idx]);
        }

        let generic_class = &self.runtime_metadata.generic_classes[idx as usize];

        let desc_idx = self.generic_classes.len();
        let desc = GenericClassInst {
            class: if let Some(ty_def_idx) = generic_class.ty_def_idx {
                Some(self.add_type_def(ty_def_idx as u32)?)
            } else {
                None
            },
            context: GenericContext {
                class: if let Some(inst_name) = generic_class.class_inst {
                    let isnt_idx = self.runtime_metadata.gi_name_map[inst_name];
                    Some(self.add_generic_inst(isnt_idx as u32)?)
                } else {
                    None
                },
                method: None,
            },
        };

        self.generic_classes.push(desc);
        self.generic_class_map.insert(idx, desc_idx);
        Ok(desc_idx)
    }

    fn add_generic_method(&mut self, idx: u32) -> Result<usize> {
        if self.generic_funcs.is_some() {
            bail!("tried to add generic method after generic funcs were already processed");
        }

        if self.generic_method_map.contains_key(&idx) {
            return Ok(self.generic_method_map[&idx]);
        }

        let generic_method = &self.runtime_metadata.generic_methods[idx as usize];

        let desc_idx = self.generic_methods.len();
        let desc = GenericMethodInst {
            method: self.add_method(generic_method.method_def as u32)?,
            context: GenericContext {
                class: generic_method
                    .class_inst
                    .map(|idx| self.add_generic_inst(idx as u32))
                    .map_or(Ok(None), |r| r.map(Some))?,
                method: generic_method
                    .method_isnt
                    .map(|idx| self.add_generic_inst(idx as u32))
                    .map_or(Ok(None), |r| r.map(Some))?,
            },
        };
        self.generic_methods.push(desc);
        self.generic_method_map.insert(idx, desc_idx);
        Ok(desc_idx)
    }

    fn add_encoded(&mut self, encoded_idx: u32) -> Result<EncodedMethodIndex> {
        let ty = (encoded_idx & 0xE0000000) >> 29;
        let idx = encoded_idx & 0x1FFFFFFF;
        Ok(match ty {
            1 => EncodedMethodIndex::Il2CppClass(self.add_type(idx)?),
            2 => EncodedMethodIndex::Il2CppType(self.add_type(idx)?),
            3 => EncodedMethodIndex::MethodInfo(self.add_method(idx)?),
            5 => EncodedMethodIndex::StringLiteral(self.add_string_literal(idx)?),
            6 => EncodedMethodIndex::MethodRef(self.add_generic_method(idx)?),
            _ => bail!("Unsupported encoded method index with type {}", ty),
        })
    }

    pub fn check_for_shims<'a>(
        &self,
        funcs: HashSet<&'a str>,
        method_ptrs: &[&str],
        shims: &HashSet<String>,
    ) -> Result<HashMap<&'a str, bool>> {
        // TODO: There has to be a faster way to do this
        let mut method_map = vec![false; self.metadata.methods.len()];
        for image in &self.metadata.images {
            let name = self.get_str(image.name_index)?;
            let is_shim = shims.contains(name);
            let ty_def_range = offset_len(image.type_start, image.type_count);
            for ty_def in &self.metadata.type_definitions[ty_def_range] {
                let method_range = offset_len(ty_def.method_start, ty_def.method_count as u32);
                method_map[method_range]
                    .iter_mut()
                    .for_each(|is| *is = is_shim);
            }
        }

        let mut map = HashMap::new();
        for func in funcs {
            if func.ends_with("_inline") {
                map.insert(func, false);
                continue;
            }
            let func_idx = method_ptrs
                .iter()
                .position(|&s| s == func)
                .context("Could not find func in gen method ptr table")?;
            let gen_method_idx = self
                .runtime_metadata
                .generic_method_funcs
                .iter()
                .find_map(|funcs| {
                    (funcs.method_idx == func_idx).then_some(funcs.generic_method_idx)
                })
                .context("could not find use of generic func")?;
            let method_idx = self.runtime_metadata.generic_methods[gen_method_idx].method_def;
            map.insert(func, method_map[method_idx]);
        }
        Ok(map)
    }
}
