use crate::il2cpp_types::*;
use crate::metadata_builder::{CodeRegistrationBuilder, Metadata, MetadataRegistrationBuilder};
use anyhow::{bail, Context, Result};
use dlopen::raw::Library;
use merge_data::{
    AddedGenericContainer, EncodedMethodIndex, GenericClassInst, GenericContainerOwner,
    GenericInst, MergeModData, TypeDescription, TypeDescriptionData,
};
use std::collections::HashMap;
use std::ffi::c_void;
use std::lazy::{SyncLazy, SyncOnceCell};
use std::sync::Mutex;
use std::{ptr, slice, str};

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

#[derive(Default, Clone, Copy)]
struct TypeResolveContext {
    class_container: Option<usize>,
    method_container: Option<usize>,
}

impl TypeResolveContext {
    fn new(class: i32) -> Self {
        Self {
            class_container: if class != -1 {
                Some(class as usize)
            } else {
                None
            },
            method_container: None,
        }
    }

    fn with_method(self, method: i32) -> Self {
        Self {
            class_container: self.class_container,
            method_container: if method != -1 {
                Some(method as usize)
            } else {
                None
            },
        }
    }
}

struct TypeResolver<'a> {
    descs: &'a [TypeDescription],
    type_def_refs: &'a [usize],
    refs: Vec<Option<i32>>,
    generic_inst_descs: &'a [GenericInst],
    generic_insts: Vec<Option<i32>>,
    generic_class_descs: &'a [GenericClassInst],
    generic_classes: Vec<Option<*mut Il2CppGenericClass>>,
}

impl<'a> TypeResolver<'a> {
    fn new(mod_data: &'a MergeModData, type_def_refs: &'a [usize]) -> Self {
        Self {
            descs: &mod_data.type_descriptions,
            type_def_refs,
            refs: vec![None; mod_data.type_descriptions.len()],
            generic_inst_descs: &mod_data.generic_instances,
            generic_insts: vec![None; mod_data.generic_instances.len()],
            generic_class_descs: &mod_data.generic_class_insts,
            generic_classes: vec![None; mod_data.generic_class_insts.len()],
        }
    }

    fn resolve(
        &mut self,
        idx: usize,
        loader: &mut ModLoader,
        ctx: &TypeResolveContext,
    ) -> Result<i32> {
        if let Some(idx) = self.refs[idx] {
            return Ok(idx);
        }

        let desc = &self.descs[idx];
        let data = match desc.data {
            TypeDescriptionData::TypeDefIdx(idx) => Il2CppType__bindgen_ty_1 {
                klassIndex: self.type_def_refs[idx] as i32,
            },
            TypeDescriptionData::TypeIdx(idx) => Il2CppType__bindgen_ty_1 {
                type_: {
                    let ty_idx = self.resolve(idx, loader, ctx)?;
                    loader.metadata_registration.types[ty_idx as usize]
                },
            },
            TypeDescriptionData::GenericParam(owner, idx) => {
                let container_idx = match owner {
                    // TODO: verify that owner is the one in context
                    GenericContainerOwner::Class(_) => ctx.class_container,
                    GenericContainerOwner::Method(_) => ctx.method_container,
                };
                let container_idx =
                    container_idx.context("failed to resolve generic param in this context")?;
                let container = &loader.metadata.generic_containers[container_idx];
                Il2CppType__bindgen_ty_1 {
                    genericParameterIndex: container.genericParameterStart + idx as i32,
                }
            }
            TypeDescriptionData::GenericClass(idx) => Il2CppType__bindgen_ty_1 {
                generic_class: self.resolve_generic_class(idx, loader, ctx)?,
            },
        };
        let bitfield = Il2CppType::new_bitfield_1(
            desc.attrs as u32,
            desc.ty as u32,
            0,
            desc.by_ref as u32,
            false as u32,
        );
        // TODO: this is probably hella slow, use stable hashset
        let ty_idx = loader
            .metadata_registration
            .types
            .iter()
            .position(|&ty| unsafe {
                (*ty).data.dummy == data.dummy && (*ty)._bitfield_1 == bitfield
            });
        let ty_idx = match ty_idx {
            Some(idx) => idx,
            None => {
                let ty = Il2CppType {
                    data,
                    _bitfield_align_1: Default::default(),
                    _bitfield_1: bitfield,
                    __bindgen_padding_0: Default::default(),
                };
                let ptr = Box::leak(Box::new(ty)) as _;
                let idx = loader.metadata_registration.types.len();
                // added_types.push(ptr as *mut Il2CppType);
                loader.metadata_registration.types.push(ptr);
                idx
            }
        };
        self.refs[idx] = Some(ty_idx as i32);
        Ok(ty_idx as i32)
    }

    fn resolve_generic_class(
        &mut self,
        idx: usize,
        loader: &mut ModLoader,
        ctx: &TypeResolveContext,
    ) -> Result<*mut Il2CppGenericClass> {
        if let Some(ptr) = self.generic_classes[idx] {
            return Ok(ptr);
        }

        let gen_class = &self.generic_class_descs[idx];

        let class_inst = self.resolve_generic_inst_ptr(gen_class.context.class, loader, ctx)?;
        let method_inst = self.resolve_generic_inst_ptr(gen_class.context.method, loader, ctx)?;
        let ptr = Box::leak(Box::new(Il2CppGenericClass {
            typeDefinitionIndex: gen_class
                .class
                .map(|idx| self.type_def_refs[idx] as i32)
                .unwrap_or(-1),
            context: Il2CppGenericContext {
                class_inst,
                method_inst,
            },
            cached_class: ptr::null_mut(),
        }));
        loader.metadata_registration.generic_classes.push(ptr);

        Ok(ptr)
    }

    fn resolve_generic_inst(
        &mut self,
        idx: Option<usize>,
        loader: &mut ModLoader,
        ctx: &TypeResolveContext,
    ) -> Result<i32> {
        let idx = match idx {
            Some(idx) => idx,
            None => return Ok(-1),
        };
        if let Some(idx) = self.generic_insts[idx] {
            return Ok(idx);
        }
        let gen_inst = &self.generic_inst_descs[idx];

        let types = gen_inst
            .types
            .iter()
            .map(|&idx| {
                self.resolve(idx, loader, ctx)
                    .map(|idx| loader.metadata_registration.types[idx as usize])
            })
            .collect::<Result<Box<_>>>()?;
        let base_idx = loader
            .metadata_registration
            .generic_insts
            .iter()
            .position(|&base_inst| {
                let base_types = unsafe {
                    let base_inst = &*base_inst;
                    slice::from_raw_parts(base_inst.type_argv, base_inst.type_argc as usize)
                };
                &*types == base_types
            });

        let resolved_idx = match base_idx {
            Some(idx) => idx,
            None => {
                let idx = loader.metadata_registration.generic_insts.len();
                loader
                    .metadata_registration
                    .generic_insts
                    .push(Box::leak(Box::new(Il2CppGenericInst {
                        type_argc: gen_inst.types.len() as u32,
                        type_argv: Box::leak(types).as_mut_ptr(),
                    })));
                idx
            }
        };

        self.generic_insts[idx] = Some(resolved_idx as i32);
        Ok(resolved_idx as i32)
    }

    fn resolve_generic_inst_ptr(
        &mut self,
        idx: Option<usize>,
        loader: &mut ModLoader,
        ctx: &TypeResolveContext,
    ) -> Result<*const Il2CppGenericInst> {
        let idx = self.resolve_generic_inst(idx, loader, ctx)?;
        let ptr = if idx == -1 {
            ptr::null()
        } else {
            loader.metadata_registration.generic_insts[idx as usize]
        };
        Ok(ptr)
    }
}

pub struct Mod {
    pub lib: Library,
    pub refs: ModRefs,
    pub load_fn: Option<unsafe extern "C" fn()>,
}

pub struct ModRefs {
    pub type_def_refs: Vec<usize>,
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
        ty_resolver: &mut TypeResolver,
        ctx: &TypeResolveContext,
        method_refs: &[usize],
        gen_methods: &[usize],
    ) -> Result<u32> {
        Ok(match eidx {
            EncodedMethodIndex::Il2CppClass(idx) => {
                ty_resolver.resolve(idx, self, ctx)? as u32 | 0x20000000
            }
            EncodedMethodIndex::Il2CppType(idx) => {
                ty_resolver.resolve(idx, self, ctx)? as u32 | 0x40000000
            }
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
        })
    }

    fn resolve_generic_container(
        &mut self,
        generic_container: &Option<AddedGenericContainer>,
        owner_idx: usize,
        type_resolver: &mut TypeResolver,
        ctx: &TypeResolveContext,
    ) -> Result<i32> {
        if let Some(container) = &generic_container {
            let idx = self.metadata.generic_containers.len();
            let generic_param_start = self.metadata.generic_parameters.len();
            for (num, param) in container.parameters.iter().enumerate() {
                let name = self.add_str(&param.name);
                let constraints_start = self.metadata.generic_parameter_constraints.len();
                for &constraint in &param.constraints {
                    let idx = type_resolver.resolve(constraint, self, ctx)?;
                    self.metadata.generic_parameter_constraints.push(idx)
                }

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
            Ok(idx as i32)
        } else {
            Ok(-1)
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

        let mut ty_resolver = TypeResolver::new(mod_data, &type_def_refs);

        let code_gen_module: *const Il2CppCodeGenModule =
            unsafe { lib.symbol(&format!("g_{}CodeGenModule", id))? };
        self.code_registration
            .code_gen_modules
            .push(code_gen_module);

        let mut load_fn = None;

        tracing::debug!("Adding mod type defs");
        // Fill in everything that doesn't requre method references now
        let ty_defs_start = self.metadata.type_definitions.len();
        for ty_def in &mod_data.added_type_defintions {
            let ty_def_idx = self.metadata.type_definitions.len();
            let container_idx = self.resolve_generic_container(
                &ty_def.generic_container,
                ty_def_idx,
                &mut ty_resolver,
                &Default::default(),
            )?;
            let ctx = TypeResolveContext::new(container_idx);

            let fields_start = self.metadata.fields.len();
            for field in &ty_def.fields {
                let name = self.add_str(&field.name);
                let ty = ty_resolver.resolve(field.ty, self, &ctx)?;
                self.metadata.fields.push(Il2CppFieldDefinition {
                    nameIndex: name,
                    typeIndex: ty,
                    token: field.token,
                });
            }
            let methods_start = self.metadata.methods.len();
            for method in &ty_def.methods {
                let method_idx = self.metadata.methods.len();
                let container_idx = self.resolve_generic_container(
                    &method.generic_container,
                    method_idx,
                    &mut ty_resolver,
                    &ctx,
                )?;

                if ty_def.name == "Plugin" && method.name == "Load" && method.parameters.is_empty()
                {
                    let rid = 0x00FFFFFF & method.token;
                    unsafe {
                        let code_gen_module = &*code_gen_module;
                        let f = code_gen_module.methodPointers.add(rid as usize - 1).read();
                        load_fn = f;
                    }
                }
                let params_start = self.metadata.parameters.len();
                for param in &method.parameters {
                    let name = self.add_str(&param.name);
                    let ty = ty_resolver.resolve(param.ty, self, &ctx)?;
                    self.metadata.parameters.push(Il2CppParameterDefinition {
                        nameIndex: name,
                        token: param.token,
                        typeIndex: ty,
                    });
                }
                let name = self.add_str(&method.name);
                let return_ty = ty_resolver.resolve(method.return_ty, self, &ctx)?;
                self.metadata.methods.push(Il2CppMethodDefinition {
                    nameIndex: name as i32,
                    declaringType: type_def_refs[method.declaring_type] as i32,
                    returnType: return_ty,
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
                let ty = ty_resolver.resolve(interface, self, &ctx)?;
                self.metadata.interfaces.push(ty);
            }
            let interface_offsets_start = self.metadata.interface_offsets.len();
            for &(idx, offset) in &ty_def.interface_offsets {
                let ty = ty_resolver.resolve(idx, self, &ctx)?;
                self.metadata
                    .interface_offsets
                    .push(Il2CppInterfaceOffsetPair {
                        interfaceTypeIndex: ty,
                        offset: offset as i32,
                    });
            }

            let namespace = self.add_str(&ty_def.namespace);
            let name = self.add_str(&ty_def.name);
            let byval_ty = ty_resolver.resolve(ty_def.byval_type, self, &ctx)?;
            let byref_ty = ty_resolver.resolve(ty_def.byref_type, self, &ctx)?;
            let declaring_ty = match ty_def.declaring_type {
                Some(idx) => ty_resolver.resolve(idx, self, &ctx)?,
                None => -1,
            };
            let parent_ty = match ty_def.parent_type {
                Some(idx) => ty_resolver.resolve(idx, self, &ctx)?,
                None => -1,
            };
            let elem_ty = ty_resolver.resolve(ty_def.element_type, self, &ctx)?;
            self.metadata.type_definitions.push(Il2CppTypeDefinition {
                nameIndex: name as i32,
                namespaceIndex: namespace as i32,
                byvalTypeIndex: byval_ty,
                byrefTypeIndex: byref_ty,

                declaringTypeIndex: declaring_ty,
                parentIndex: parent_ty,
                elementTypeIndex: elem_ty,

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

        tracing::debug!("Resolving methods");
        let mut method_refs = Vec::with_capacity(mod_data.method_descriptions.len());
        'mm: for method in &mod_data.method_descriptions {
            let decl_ty_idx = type_def_refs[method.defining_type];
            let ty_def = &self.metadata.type_definitions[decl_ty_idx];
            let ctx = TypeResolveContext::new(ty_def.genericContainerIndex);

            let method_range = offset_len(ty_def.methodStart, ty_def.method_count as i32);
            for ty_method_idx in method_range {
                let ty_method = &self.metadata.methods[ty_method_idx];
                let tm_return_ty = ty_method.returnType;
                let params_count = ty_method.parameterCount;
                let params_range = offset_len(ty_method.parameterStart, params_count as i32);
                let name_idx = ty_method.nameIndex;

                let ctx = if method.num_gen_params > 0 {
                    if ty_method.genericContainerIndex == -1 {
                        continue;
                    }
                    let gc =
                        &self.metadata.generic_containers[ty_method.genericContainerIndex as usize];
                    if gc.type_argc as u32 != method.num_gen_params {
                        continue;
                    }
                    ctx.with_method(ty_method.genericContainerIndex)
                } else {
                    ctx
                };
                let return_ty = ty_resolver.resolve(method.return_ty, self, &ctx)?;

                if self.get_str(name_idx)? != method.name
                    || tm_return_ty != return_ty
                    || params_count as usize != method.params.len()
                {
                    continue;
                }

                let mut params_match = true;
                for (&param, tm_param_idx) in method.params.iter().zip(params_range) {
                    let param = ty_resolver.resolve(param, self, &ctx)?;
                    if param != self.metadata.parameters[tm_param_idx].typeIndex {
                        params_match = false;
                        break;
                    }
                }
                if params_match {
                    method_refs.push(ty_method_idx);
                    continue 'mm;
                }
            }

            let ty_def = &self.metadata.type_definitions[decl_ty_idx];
            bail!(
                "unresolved method reference {}.{}::{}",
                self.get_str(ty_def.namespaceIndex)?,
                self.get_str(ty_def.nameIndex)?,
                method.name
            );
        }

        tracing::debug!("Resolving generic methods");
        let gen_method_offset = self.metadata_registration.method_specs.len();
        let mut gen_methods = Vec::with_capacity(mod_data.generic_method_insts.len());
        for gen_method in &mod_data.generic_method_insts {
            let method = &self.metadata.methods[method_refs[gen_method.method]];
            let ctx = TypeResolveContext::new(
                self.metadata.type_definitions[method.declaringType as usize].genericContainerIndex,
            )
            .with_method(method.genericContainerIndex);

            let method_spec = Il2CppMethodSpec {
                methodDefinitionIndex: method_refs[gen_method.method] as i32,
                classIndexIndex: ty_resolver.resolve_generic_inst(
                    gen_method.context.class,
                    self,
                    &ctx,
                )?,
                methodIndexIndex: ty_resolver.resolve_generic_inst(
                    gen_method.context.method,
                    self,
                    &ctx,
                )?,
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

        tracing::debug!("Resolving rest of added type defs");
        // Now that method references have been resolved, we go back through the type definitions and add items that required method references.
        for (i, ty_def) in mod_data.added_type_defintions.iter().enumerate() {
            let i = i + ty_defs_start;
            let metadata_def = &mut self.metadata.type_definitions[i];
            let ctx = TypeResolveContext::new(metadata_def.genericContainerIndex);

            let events_start = self.metadata.events.len();
            for event in &ty_def.events {
                let name = self.add_str(&event.name);
                let ty = ty_resolver.resolve(event.ty, self, &ctx)?;
                self.metadata.events.push(Il2CppEventDefinition {
                    nameIndex: name as i32,
                    typeIndex: ty,
                    add: method_refs[event.add] as i32,
                    remove: method_refs[event.remove] as i32,
                    raise: method_refs[event.raise] as i32,
                    token: event.token,
                })
            }
            self.metadata.type_definitions[i].eventStart = events_start as i32;

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
            self.metadata.type_definitions[i].propertyStart = properties_start as i32;

            let vtable_start = self.metadata.vtable_methods.len();
            for &encoded_idx in &ty_def.vtable {
                let new_eidx = self.resolve_eidx(
                    encoded_idx,
                    mod_data,
                    &mut ty_resolver,
                    &ctx,
                    &method_refs,
                    &gen_methods,
                )?;
                self.metadata.vtable_methods.push(new_eidx);
            }
            self.metadata.type_definitions[i].vtableStart = vtable_start as i32;
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

        tracing::debug!("Resolving metadata usages");
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
                    &mut ty_resolver,
                    &Default::default(),
                    &method_refs,
                    &gen_methods,
                )?;
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
                    method_refs,
                    usage_list_offset,
                },
                load_fn,
            },
        );

        Ok(())
    }
}
