use crate::il2cpp_types::*;
use crate::metadata_builder::{CodeRegistrationBuilder, Metadata, MetadataRegistrationBuilder};
use anyhow::{bail, Context, Result};
use dlopen::raw::Library;
use merge_data::{
    AddedGenericContainer, EncodedMethodIndex, GenericContainerOwner, MergeModData,
    TypeDescriptionData,
};
use std::collections::HashMap;
use std::ffi::c_void;
use std::lazy::{SyncLazy, SyncOnceCell};
use std::sync::Mutex;
use std::{ptr, str};

pub static MODS: SyncLazy<Mutex<HashMap<String, Mod>>> = SyncLazy::new(Default::default);
pub static CODE_REGISTRATION: SyncOnceCell<&'static Il2CppCodeRegistration> = SyncOnceCell::new();
pub static METADATA_REGISTRATION: SyncOnceCell<&'static Il2CppMetadataRegistration> =
    SyncOnceCell::new();

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
    pub lib: Library,
    pub refs: ModRefs,
    pub load_fn: Option<unsafe extern "C" fn()>,
}

pub struct ModRefs {
    pub type_def_refs: Vec<usize>,
    pub type_refs: Vec<usize>,
    pub method_refs: Vec<usize>,
    pub usage_list_offset: usize,
}

pub struct ModLoader<'md> {
    metadata: &'md mut Metadata,
    code_registration: &'md mut CodeRegistrationBuilder,
    metadata_registration: &'md mut MetadataRegistrationBuilder,
    image_type_def_map: Vec<HashMap<(String, String), usize>>,
    method_spec_map: HashMap<Il2CppMethodSpec, usize>,
}

impl<'md> ModLoader<'md> {
    pub fn new(
        metadata: &'md mut Metadata,
        code_registration: &'md mut CodeRegistrationBuilder,
        metadata_registration: &'md mut MetadataRegistrationBuilder,
    ) -> Result<Self> {
        let mut image_type_def_map = Vec::new();
        for image in &metadata.images {
            let mut type_def_map = HashMap::with_capacity(image.typeCount as usize);
            let type_def_range = offset_len(image.typeStart, image.typeCount as i32);
            for (i, type_def) in metadata.type_definitions[type_def_range].iter().enumerate() {
                let namespace = get_str(&metadata.string, type_def.namespaceIndex as usize)?;
                let name = get_str(&metadata.string, type_def.nameIndex as usize)?;
                type_def_map.insert(
                    (namespace.to_string(), name.to_string()),
                    i + image.typeStart as usize,
                );
            }
            image_type_def_map.push(type_def_map);
        }
        let mut type_def_map = HashMap::with_capacity(metadata.type_definitions.len());
        for (i, type_def) in metadata.type_definitions.iter().enumerate() {
            let namespace = get_str(&metadata.string, type_def.namespaceIndex as usize)?;
            let name = get_str(&metadata.string, type_def.nameIndex as usize)?;
            type_def_map.insert((namespace.to_string(), name.to_string()), i);
        }
        let mut method_spec_map = HashMap::new();
        for (i, method_spec) in metadata_registration.method_specs.iter().enumerate() {
            method_spec_map.insert(*method_spec, i);
        }
        Ok(Self {
            metadata,
            code_registration,
            metadata_registration,
            image_type_def_map,
            method_spec_map,
        })
    }

    fn get_str(&self, offset: i32) -> Result<&str> {
        get_str(&self.metadata.string, offset as usize)
    }

    fn add_str(&mut self, str: &str) -> i32 {
        let idx = self.metadata.string.len() as i32;
        self.metadata.string.extend_from_slice(str.as_bytes());
        self.metadata.string.push(0);
        idx
    }

    fn add_str_literal(&mut self, str: &str) -> i32 {
        let idx = self.metadata.string_literal_data.len() as i32;
        self.metadata
            .string_literal_data
            .extend_from_slice(str.as_bytes());
        self.metadata.string_literal_data.push(0);
        idx
    }

    fn resolve_eidx(
        &mut self,
        eidx: EncodedMethodIndex,
        mod_data: &MergeModData,
        type_refs: &[usize],
        method_refs: &[usize],
        gen_methods: &[usize],
    ) -> u32 {
        match eidx {
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
            EncodedMethodIndex::MethodRef(idx) => gen_methods[idx] as u32 | 0xC0000000,
        }
    }

    fn resolve_generic_container(
        &mut self,
        generic_container: &Option<AddedGenericContainer>,
        owner_idx: usize,
        type_refs: &[usize],
    ) -> i32 {
        if let Some(container) = &generic_container {
            let idx = self.metadata.generic_containers.len();
            let generic_param_start = self.metadata.generic_parameters.len();
            for (num, param) in container.parameters.iter().enumerate() {
                let name = self.add_str(&param.name);
                let constraints_start = self.metadata.generic_parameter_constraints.len();
                let resolved_constraints =
                    param.constraints.iter().map(|&idx| type_refs[idx] as i32);
                self.metadata
                    .generic_parameter_constraints
                    .extend(resolved_constraints);

                self.metadata
                    .generic_parameters
                    .push(Il2CppGenericParameter {
                        ownerIndex: idx as i32,
                        nameIndex: name,
                        constraintsStart: constraints_start as i16,
                        constraintsCount: param.constraints.len() as i16,
                        num: num as u16,
                        flags: param.flags,
                    });
            }
            self.metadata
                .generic_containers
                .push(Il2CppGenericContainer {
                    ownerIndex: owner_idx as i32,
                    type_argc: container.parameters.len() as i32,
                    is_method: matches!(container.owner, GenericContainerOwner::Method(_)) as i32,
                    genericParameterStart: generic_param_start as i32,
                });
            idx as i32
        } else {
            -1
        }
    }

    pub fn load_mod(&mut self, id: &str, mod_data: &MergeModData, lib: Library) -> Result<()> {
        let image_name = self.add_str(&mod_data.added_image.name) as i32;
        self.metadata.images.push(Il2CppImageDefinition {
            nameIndex: image_name,
            assemblyIndex: self.metadata.assemblies.len() as i32,

            typeStart: self.metadata.type_definitions.len() as i32,
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
        let mut image_refs = Vec::new();
        'ii: for image_desc in &mod_data.image_descriptions {
            for (i, image) in self.metadata.images.iter().enumerate() {
                let name = self.get_str(image.nameIndex)?;
                if name == image_desc.name {
                    image_refs.push(i);
                    continue 'ii;
                }
            }

            bail!("could not resolve image reference: {}", image_desc.name);
        }

        self.image_type_def_map
            .push(HashMap::with_capacity(mod_data.added_type_defintions.len()));
        let type_def_map = self.image_type_def_map.last_mut().unwrap();
        for (i, type_def) in mod_data.added_type_defintions.iter().enumerate() {
            type_def_map.insert(
                (type_def.namespace.to_string(), type_def.name.to_string()),
                self.metadata.type_definitions.len() + i,
            );
        }

        let type_def_refs = mod_data
            .type_def_descriptions
            .iter()
            .map(|desc| {
                self.image_type_def_map[image_refs[desc.image]]
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

        // Save the allocated types to fix generic classes
        let mut added_types = Vec::new();
        let mut type_refs = Vec::with_capacity(mod_data.type_descriptions.len());
        for ty in &mod_data.type_descriptions {
            let data = match ty.data {
                TypeDescriptionData::TypeDefIdx(idx) => Il2CppType__bindgen_ty_1 {
                    klassIndex: type_def_refs[idx] as i32,
                },
                TypeDescriptionData::TypeIdx(idx) => Il2CppType__bindgen_ty_1 {
                    // Types used here should be earlier in the list and already resolved
                    type_: self.metadata_registration.types[type_refs[idx]],
                },
                TypeDescriptionData::GenericParam(owner, idx) => Il2CppType__bindgen_ty_1 {
                    // These definitely can't be resolved right now as they require type definitions and methods.
                    // Like generic classes, they will be fixed later
                    dummy: (match owner {
                        GenericContainerOwner::Class(idx) => idx << 16,
                        GenericContainerOwner::Method(idx) => idx << 16,
                    } | (idx as usize)) as _,
                },
                TypeDescriptionData::GenericClass(idx) => Il2CppType__bindgen_ty_1 {
                    // Generic classes cannot be resolved now because they require types to be resolved.
                    // They will be fixed later, but for now we just assign their index
                    generic_class: idx as _,
                },
            };
            let bitfield = Il2CppType::new_bitfield_1(
                ty.attrs as u32,
                ty.ty as u32,
                0,
                ty.by_ref as u32,
                false as u32,
            );
            let ty_idx = match ty.data {
                // Generic classes aren't resolved yet and contains dummy data, so we
                // can't search for a matching type
                TypeDescriptionData::GenericClass(_) => None,

                // TODO: this is probably hella slow, use stable hashset
                _ => self
                    .metadata_registration
                    .types
                    .iter()
                    .position(|&ty| unsafe {
                        (*ty).data.dummy == data.dummy && (*ty)._bitfield_1 == bitfield
                    }),
            };
            let idx = match ty_idx {
                Some(idx) => idx,
                None => {
                    let ty = Il2CppType {
                        data,
                        _bitfield_align_1: Default::default(),
                        _bitfield_1: bitfield,
                        __bindgen_padding_0: Default::default(),
                    };
                    let ptr = Box::leak(Box::new(ty)) as _;
                    let idx = self.metadata_registration.types.len();
                    added_types.push(ptr as *mut Il2CppType);
                    self.metadata_registration.types.push(ptr);
                    idx
                }
            };
            type_refs.push(idx);
        }

        let gen_inst_offset = self.metadata_registration.generic_insts.len();
        for gen_inst in &mod_data.generic_instances {
            let types: Box<[_]> = gen_inst
                .types
                .iter()
                .map(|&idx| self.metadata_registration.types[idx])
                .collect();

            self.metadata_registration
                .generic_insts
                .push(Box::leak(Box::new(Il2CppGenericInst {
                    type_argc: gen_inst.types.len() as u32,
                    type_argv: Box::leak(types).as_mut_ptr(),
                })));
        }

        let gen_class_offset = self.metadata_registration.generic_classes.len();
        for gen_class in &mod_data.generic_class_insts {
            self.metadata_registration
                .generic_classes
                .push(Box::leak(Box::new(Il2CppGenericClass {
                    typeDefinitionIndex: gen_class
                        .class
                        .map(|idx| type_def_refs[idx] as i32)
                        .unwrap_or(-1),
                    context: Il2CppGenericContext {
                        class_inst: gen_class.context.class.map_or(ptr::null(), |idx| {
                            self.metadata_registration.generic_insts[idx + gen_inst_offset]
                        }),
                        method_inst: gen_class.context.method.map_or(ptr::null(), |idx| {
                            self.metadata_registration.generic_insts[idx + gen_inst_offset]
                        }),
                    },
                    cached_class: ptr::null_mut(),
                })))
        }
        // Now that we've resolved our generic classes, we can fix our type refs
        for &type_ptr in &added_types {
            let ty = unsafe { &mut *type_ptr };
            if ty.type_() == Il2CppTypeEnum_IL2CPP_TYPE_GENERICINST {
                let idx = unsafe { ty.data.generic_class } as usize;
                ty.data.generic_class =
                    self.metadata_registration.generic_classes[idx + gen_class_offset];
            }
        }

        let code_gen_module: *const Il2CppCodeGenModule =
            unsafe { lib.symbol(&format!("g_{}CodeGenModule", id))? };
        self.code_registration
            .code_gen_modules
            .push(code_gen_module);

        let mut load_fn = None;

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
                if ty_def.name == "Plugin" && method.name == "Load" && method.parameters.is_empty()
                {
                    let rid = 0x00FFFFFF & method.token;
                    unsafe {
                        let code_gen_module = &*code_gen_module;
                        let f = code_gen_module.methodPointers.add(rid as usize - 1).read();
                        load_fn = f;
                    }
                }
                let method_idx = self.metadata.methods.len();
                let params_start = self.metadata.parameters.len();
                for param in &method.parameters {
                    let name = self.add_str(&param.name);
                    self.metadata.parameters.push(Il2CppParameterDefinition {
                        nameIndex: name,
                        token: param.token,
                        typeIndex: type_refs[param.ty] as i32,
                    });
                }

                let container_idx = self.resolve_generic_container(
                    &method.generic_container,
                    method_idx,
                    &type_refs,
                );
                let name = self.add_str(&method.name);
                self.metadata.methods.push(Il2CppMethodDefinition {
                    nameIndex: name as i32,
                    declaringType: type_def_refs[method.declaring_type] as i32,
                    returnType: type_refs[method.return_ty] as i32,
                    parameterStart: params_start as i32,
                    genericContainerIndex: container_idx,
                    token: method.token,
                    flags: method.flags,
                    iflags: method.iflags,
                    slot: method.slot,
                    parameterCount: method.parameters.len() as u16,
                })
            }
            let nested_types_start = self.metadata.nested_types.len();
            for &nested_ty in &ty_def.nested_types {
                tracing::debug!("adding nested type in {}", ty_def.name);
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

            let ty_def_idx = self.metadata.type_definitions.len();
            let container_idx =
                self.resolve_generic_container(&ty_def.generic_container, ty_def_idx, &type_refs);

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

                genericContainerIndex: container_idx,

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

            tracing::debug!("{} methods", ty_def.method_count);
            let method_range = offset_len(ty_def.methodStart, ty_def.method_count as i32);
            for ty_method_idx in method_range {
                let ty_method = &self.metadata.methods[ty_method_idx];
                tracing::debug!("{}", self.get_str(ty_method.nameIndex)?);
                if self.get_str(ty_method.nameIndex)? != method.name
                    || ty_method.returnType != return_ty
                    || ty_method.parameterCount as usize != method.params.len()
                {
                    tracing::debug!("returnType/parameterCount/name mismatch");
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
                tracing::debug!("parameter type mismatch");
            }
            bail!(
                "unresolved method reference {}.{}::{}",
                self.get_str(ty_def.namespaceIndex)?,
                self.get_str(ty_def.nameIndex)?,
                method.name
            );
        }

        let gen_method_offset = self.metadata_registration.method_specs.len();
        let mut gen_methods = Vec::with_capacity(mod_data.generic_method_insts.len());
        for gen_method in &mod_data.generic_method_insts {
            let method_spec = Il2CppMethodSpec {
                methodDefinitionIndex: method_refs[gen_method.method] as i32,
                classIndexIndex: gen_method
                    .context
                    .class
                    .map_or(-1, |idx| (idx + gen_inst_offset) as i32),
                methodIndexIndex: gen_method
                    .context
                    .method
                    .map_or(-1, |idx| (idx + gen_inst_offset) as i32),
            };
            if let Some(&idx) = self.method_spec_map.get(&method_spec) {
                gen_methods.push(idx);
            } else {
                let idx = self.metadata_registration.method_specs.len();
                self.metadata_registration.method_specs.push(method_spec);
                gen_methods.push(idx);
            }
        }
        let mod_adj_thunks: *const Il2CppMethodPointer =
            unsafe { lib.symbol("g_Il2CppGenericAdjustorThunks")? };
        let adj_thunks_offset = self.code_registration.generic_adjustor_thunks.len();
        for i in 0..mod_data.code_table_sizes.generic_adjustor_thunks {
            let method_pointer = unsafe { mod_adj_thunks.add(i).read() };
            self.code_registration
                .generic_adjustor_thunks
                .push(method_pointer);
        }
        let mod_gen_method_ptrs: *const Il2CppMethodPointer =
            unsafe { lib.symbol("g_Il2CppGenericMethodPointers")? };
        let gen_method_ptrs_offset = self.code_registration.generic_method_pointers.len();
        for i in 0..mod_data.code_table_sizes.generic_method_pointers {
            let method_pointer = unsafe { mod_gen_method_ptrs.add(i).read() };
            self.code_registration
                .generic_method_pointers
                .push(method_pointer);
        }
        let mod_invokers: *const InvokerMethod = unsafe { lib.symbol("g_Il2CppInvokerPointers")? };
        let invokers_offset = self.code_registration.invoker_pointers.len();
        for i in 0..mod_data.code_table_sizes.invoker_pointers {
            let method_pointer = unsafe { mod_invokers.add(i).read() };
            self.code_registration.invoker_pointers.push(method_pointer);
        }
        for gen_method_funcs in &mod_data.generic_method_funcs {
            let idx = gen_methods[gen_method_funcs.generic_method];
            if idx < gen_method_offset {
                // We're using a generic method instance that the game already has
                // no need to add our own function (it could possibly be a stub anyways)
                continue;
            }
            self.metadata_registration.generic_method_table.push(
                Il2CppGenericMethodFunctionsDefinitions {
                    genericMethodIndex: (gen_method_funcs.generic_method + gen_method_offset)
                        as i32,
                    indices: Il2CppGenericMethodIndices {
                        methodIndex: (gen_method_funcs.method_idx + gen_method_ptrs_offset) as i32,
                        invokerIndex: (gen_method_funcs.invoker_idx + invokers_offset) as i32,
                        adjustorThunkIndex: gen_method_funcs
                            .adjustor_thunk_idx
                            .map_or(-1, |idx| (idx + adj_thunks_offset) as i32),
                    },
                },
            );
        }

        // Now that method references have been resolved, we go back through the type definitions and add items that required method references.
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
                let new_eidx = self.resolve_eidx(
                    encoded_idx,
                    mod_data,
                    &type_refs,
                    &method_refs,
                    &gen_methods,
                );
                self.metadata.vtable_methods.push(new_eidx);
            }

            let i = i + ty_defs_start;
            let metadata_def = &mut self.metadata.type_definitions[i];
            metadata_def.eventStart = events_start as i32;
            metadata_def.propertyStart = properties_start as i32;
            metadata_def.vtableStart = vtable_start as i32;
        }
        // We can also fix our generic params in our type refs
        for &type_ptr in &added_types {
            let ty = unsafe { &mut *type_ptr };
            let encoded_data = unsafe { ty.data.dummy } as usize;
            let idx = encoded_data >> 16;
            let num = encoded_data & 0xFFFF;
            #[allow(non_upper_case_globals)]
            let gc_idx = match ty.type_() {
                Il2CppTypeEnum_IL2CPP_TYPE_VAR => {
                    self.metadata.type_definitions[type_def_refs[idx]].genericContainerIndex
                }
                Il2CppTypeEnum_IL2CPP_TYPE_MVAR => {
                    self.metadata.methods[method_refs[idx]].genericContainerIndex
                }
                _ => continue,
            } as usize;
            ty.data.genericParameterIndex =
                self.metadata.generic_containers[gc_idx].genericParameterStart + num as i32;

            if ty.type_() == Il2CppTypeEnum_IL2CPP_TYPE_GENERICINST {
                let idx = unsafe { ty.data.generic_class } as usize;
                ty.data.generic_class =
                    self.metadata_registration.generic_classes[idx + gen_class_offset];
            }
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
            imageIndex: self.metadata.images.len() as i32 - 1,
            token: mod_data.added_assembly.token,
            referencedAssemblyStart: -1,
            referencedAssemblyCount: 0,
            aname,
        });

        let field_offset_table: *const *const i32 = unsafe { lib.symbol("g_FieldOffsetTable")? };
        for i in 0..mod_data.added_type_defintions.len() {
            let offsets = unsafe { field_offset_table.add(i).read() };
            self.metadata_registration.field_offsets.push(offsets);
        }

        let type_def_sizes: *const *const Il2CppTypeDefinitionSizes =
            unsafe { lib.symbol("g_Il2CppTypeDefinitionSizesTable")? };
        for i in 0..mod_data.added_type_defintions.len() {
            let sizes = unsafe { type_def_sizes.add(i).read() };
            self.metadata_registration.type_definition_sizes.push(sizes);
        }

        let metadata_usages: *const *mut *mut c_void = unsafe { lib.symbol("g_MetadataUsages")? };
        let metadata_usage_offset = self.metadata_registration.metadata_usages.len();
        for i in 0..mod_data.code_table_sizes.metadata_usages {
            let usage = unsafe { metadata_usages.add(i).read() };
            self.metadata_registration.metadata_usages.push(usage);
        }
        let usage_list_offset = self.metadata.metadata_usage_lists.len();
        for usage_list in &mod_data.added_usage_lists {
            let pairs_idx = self.metadata.metadata_usage_pairs.len();
            self.metadata
                .metadata_usage_lists
                .push(Il2CppMetadataUsageList {
                    start: pairs_idx as u32,
                    count: usage_list.len() as u32,
                });
            for pair in usage_list {
                let source = self.resolve_eidx(
                    pair.source,
                    mod_data,
                    &type_refs,
                    &method_refs,
                    &gen_methods,
                );
                self.metadata
                    .metadata_usage_pairs
                    .push(Il2CppMetadataUsagePair {
                        encodedSourceIndex: source,
                        destinationIndex: (pair.dest + metadata_usage_offset) as u32,
                    })
            }
        }

        MODS.lock().unwrap().insert(
            id.to_string(),
            Mod {
                lib,
                refs: ModRefs {
                    type_def_refs,
                    type_refs,
                    method_refs,
                    usage_list_offset,
                },
                load_fn,
            },
        );

        Ok(())
    }
}
