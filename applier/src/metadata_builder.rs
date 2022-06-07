use crate::il2cpp_types::*;
use anyhow::{ensure, Result};
use std::mem::size_of;
use std::slice;

const SANITY: i32 = 0xFAB11BAFu32 as i32;

unsafe fn table_from_raw<T: Clone>(offset_ptr: *const u8, count: i32) -> Vec<T> {
    slice::from_raw_parts(offset_ptr.cast(), count as usize / size_of::<T>()).to_vec()
}

fn table_size<T>(table: &Vec<T>) -> usize {
    table.len() * size_of::<T>()
}

unsafe fn write_to_table<T>(ptr: *mut u8, data: T) -> *mut u8 {
    let ptr = ptr.cast::<T>();
    ptr.write(data);
    ptr.offset(1).cast()
}

macro_rules! metadata {
    ($(
        #[metadata($offset_name:ident, $count_name:ident)]
        $name:ident: $ty:ty,
    )*) => {
        pub struct Metadata {
            $(
                pub $name: $ty,
            )*
        }

        impl Metadata {
            pub unsafe fn from_raw(original: *const u8) -> Result<Self> {
                let header: &Il2CppGlobalMetadataHeader = &*original.cast();
                ensure!(header.sanity == SANITY);
                ensure!(header.version == 24);

                $(
                    let $name = table_from_raw(original.offset(header.$offset_name as isize), header.$count_name);
                )*

                Ok(Self {
                    $(
                        $name,
                    )*
                })
            }

            pub fn build(self) -> *const u8 {
                let size = size_of::<Il2CppGlobalMetadataHeader>() $(+ table_size(&self.$name))*;
                let data: *mut u8 = Box::leak(vec![0u8; size].into_boxed_slice()).as_mut_ptr();

                let mut cur: *mut u8 = unsafe { data.add(size_of::<Il2CppGlobalMetadataHeader>()) };

                $(
                    #[allow(non_snake_case)]
                    let $offset_name = unsafe { cur.offset_from(data) } as i32;
                    #[allow(non_snake_case)]
                    let $count_name = table_size(&self.$name) as i32;
                    for item in self.$name {
                        cur = unsafe { write_to_table(cur, item) };
                    }
                )*

                let header = Il2CppGlobalMetadataHeader {
                    sanity: SANITY,
                    version: 24,
                    $(
                        $offset_name,
                        $count_name,
                    )*
                };

                unsafe { data.cast::<Il2CppGlobalMetadataHeader>().write(header); }

                data
            }
        }
    }
}

metadata! {
    #[metadata(stringLiteralOffset, stringLiteralCount)]
    string_literal: Vec<Il2CppStringLiteral>,
    #[metadata(stringLiteralDataOffset, stringLiteralDataCount)]
    string_literal_data: Vec<u8>,
    #[metadata(stringOffset, stringCount)]
    string: Vec<u8>,
    #[metadata(eventsOffset, eventsCount)]
    events: Vec<Il2CppEventDefinition>,
    #[metadata(propertiesOffset, propertiesCount)]
    properties: Vec<Il2CppPropertyDefinition>,
    #[metadata(methodsOffset, methodsCount)]
    methods: Vec<Il2CppMethodDefinition>,
    #[metadata(parameterDefaultValuesOffset, parameterDefaultValuesCount)]
    parameter_default_values: Vec<Il2CppParameterDefaultValue>,
    #[metadata(fieldDefaultValuesOffset, fieldDefaultValuesCount)]
    field_default_values: Vec<Il2CppFieldDefaultValue>,
    #[metadata(fieldAndParameterDefaultValueDataOffset, fieldAndParameterDefaultValueDataCount)]
    field_and_parameter_default_value_data: Vec<u8>,
    #[metadata(fieldMarshaledSizesOffset, fieldMarshaledSizesCount)]
    field_marshaled_sizes: Vec<Il2CppFieldMarshaledSize>,
    #[metadata(parametersOffset, parametersCount)]
    parameters: Vec<Il2CppParameterDefinition>,
    #[metadata(fieldsOffset, fieldsCount)]
    fields: Vec<Il2CppFieldDefinition>,
    #[metadata(genericParametersOffset, genericParametersCount)]
    generic_parameters: Vec<Il2CppGenericParameter>,
    #[metadata(genericParameterConstraintsOffset, genericParameterConstraintsCount)]
    generic_parameter_constraints: Vec<TypeIndex>,
    #[metadata(genericContainersOffset, genericContainersCount)]
    generic_containers: Vec<Il2CppGenericContainer>,
    #[metadata(nestedTypesOffset, nestedTypesCount)]
    nested_types: Vec<TypeDefinitionIndex>,
    #[metadata(interfacesOffset, interfacesCount)]
    interfaces: Vec<TypeIndex>,
    #[metadata(vtableMethodsOffset, vtableMethodsCount)]
    vtable_methods: Vec<EncodedMethodIndex>,
    #[metadata(interfaceOffsetsOffset, interfaceOffsetsCount)]
    interface_offsets: Vec<Il2CppInterfaceOffsetPair>,
    #[metadata(typeDefinitionsOffset, typeDefinitionsCount)]
    type_definitions: Vec<Il2CppTypeDefinition>,
    #[metadata(imagesOffset, imagesCount)]
    images: Vec<Il2CppImageDefinition>,
    #[metadata(assembliesOffset, assembliesCount)]
    assemblies: Vec<Il2CppAssemblyDefinition>,
    #[metadata(metadataUsageListsOffset, metadataUsageListsCount)]
    metadata_usage_lists: Vec<Il2CppMetadataUsageList>,
    #[metadata(metadataUsagePairsOffset, metadataUsagePairsCount)]
    metadata_usage_pairs: Vec<Il2CppMetadataUsagePair>,
    #[metadata(fieldRefsOffset, fieldRefsCount)]
    field_refs: Vec<Il2CppFieldRef>,
    #[metadata(referencedAssembliesOffset, referencedAssembliesCount)]
    referenced_assemblies: Vec<u32>,
    #[metadata(attributesInfoOffset, attributesInfoCount)]
    attributes_info: Vec<Il2CppCustomAttributeTypeRange>,
    #[metadata(attributeTypesOffset, attributeTypesCount)]
    attribute_types: Vec<TypeIndex>,
    #[metadata(unresolvedVirtualCallParameterTypesOffset, unresolvedVirtualCallParameterTypesCount)]
    unresolved_virtual_call_parameter_types: Vec<TypeIndex>,
    #[metadata(unresolvedVirtualCallParameterRangesOffset, unresolvedVirtualCallParameterRangesCount)]
    unresolved_virtual_call_parameter_ranges: Vec<Il2CppRange>,
    #[metadata(windowsRuntimeTypeNamesOffset, windowsRuntimeTypeNamesSize)]
    windows_runtime_type_names: Vec<Il2CppWindowsRuntimeTypeNamePair>,
    #[metadata(exportedTypeDefinitionsOffset, exportedTypeDefinitionsCount)]
    exported_type_definitions: Vec<TypeDefinitionIndex>,
}

pub struct CodeRegistrationBuilder {
    raw: *mut *const Il2CppCodeRegistration,

    pub generic_method_pointers: Vec<Il2CppMethodPointer>, // TODO
    pub generic_adjuster_thunks: Vec<Il2CppMethodPointer>, // TODO
    pub invoker_pointers: Vec<InvokerMethod>,              // TODO
    pub custom_attribute_generators: Vec<CustomAttributesCacheGenerator>, // TODO
    pub code_gen_modules: Vec<*const Il2CppCodeGenModule>, // TODO
}

impl CodeRegistrationBuilder {
    pub unsafe fn from_raw(raw: *mut *const Il2CppCodeRegistration) -> Self {
        let cr = &**raw;

        Self {
            raw,

            generic_method_pointers: slice::from_raw_parts(
                cr.genericMethodPointers,
                cr.genericMethodPointersCount as usize,
            )
            .to_vec(),
            generic_adjuster_thunks: slice::from_raw_parts(
                cr.genericAdjustorThunks,
                cr.genericMethodPointersCount as usize,
            )
            .to_vec(),
            invoker_pointers: slice::from_raw_parts(
                cr.invokerPointers,
                cr.invokerPointersCount as usize,
            )
            .to_vec(),
            custom_attribute_generators: slice::from_raw_parts(
                cr.customAttributeGenerators,
                cr.customAttributeCount as usize,
            )
            .to_vec(),
            code_gen_modules: slice::from_raw_parts(
                cr.codeGenModules,
                cr.codeGenModulesCount as usize,
            )
            .to_vec(),
        }
    }

    pub fn build(self) {
        fn to_raw<T>(data: Vec<T>) -> (*const T, u32) {
            let data = Box::leak(data.into_boxed_slice());
            (data.as_ptr(), data.len() as u32)
        }

        let mut cr = Box::new(unsafe { **self.raw });
        (cr.genericMethodPointers, cr.genericMethodPointersCount) =
            to_raw(self.generic_method_pointers);
        (cr.genericAdjustorThunks, _) = to_raw(self.generic_adjuster_thunks);
        (cr.invokerPointers, cr.invokerPointersCount) = to_raw(self.invoker_pointers);
        let ca = to_raw(self.custom_attribute_generators);
        (cr.customAttributeGenerators, cr.customAttributeCount) = (ca.0, ca.1 as i32);
        let cg = to_raw(self.code_gen_modules);
        (cr.codeGenModules, cr.codeGenModulesCount) = (cg.0 as _, cg.1);
    }
}

pub struct MetadataRegistrationBuilder {
    raw: *mut *const Il2CppMetadataRegistration,

    pub generic_classes: Vec<*mut Il2CppGenericClass>,
    pub generic_insts: Vec<*const Il2CppGenericInst>,
    pub generic_method_table: Vec<Il2CppGenericMethodFunctionsDefinitions>, // TODO
    pub types: Vec<*const Il2CppType>,
    pub method_specs: Vec<Il2CppMethodSpec>,
    pub field_offsets: Vec<*const i32>, // TODO
    pub type_definition_sizes: Vec<*const Il2CppTypeDefinitionSizes>, // TODO
    pub metadata_usages: Vec<*mut *mut std::ffi::c_void>, // TODO
}

impl MetadataRegistrationBuilder {
    pub unsafe fn from_raw(raw: *mut *const Il2CppMetadataRegistration) -> Self {
        let mr = &**raw;

        Self {
            raw,

            generic_classes: slice::from_raw_parts(
                mr.genericClasses,
                mr.genericClassesCount as usize,
            )
            .to_vec(),
            generic_insts: slice::from_raw_parts(mr.genericInsts, mr.genericInstsCount as usize)
                .to_vec(),
            generic_method_table: slice::from_raw_parts(
                mr.genericMethodTable,
                mr.genericMethodTableCount as usize,
            )
            .to_vec(),
            types: slice::from_raw_parts(mr.types, mr.typesCount as usize).to_vec(),
            method_specs: slice::from_raw_parts(mr.methodSpecs, mr.methodSpecsCount as usize)
                .to_vec(),
            field_offsets: slice::from_raw_parts(mr.fieldOffsets, mr.fieldOffsetsCount as usize)
                .to_vec(),
            type_definition_sizes: slice::from_raw_parts(
                mr.typeDefinitionsSizes,
                mr.typeDefinitionsSizesCount as usize,
            )
            .to_vec(),
            metadata_usages: slice::from_raw_parts(
                mr.metadataUsages,
                mr.metadataUsagesCount as usize,
            )
            .to_vec(),
        }
    }

    pub fn build(self) {
        fn to_raw<T>(data: Vec<T>) -> (*const T, i32) {
            let data = Box::leak(data.into_boxed_slice());
            (data.as_ptr(), data.len() as i32)
        }

        let mut mr = Box::new(unsafe { **self.raw });
        (mr.genericClasses, mr.genericClassesCount) = to_raw(self.generic_classes);
        (mr.genericInsts, mr.genericInstsCount) = to_raw(self.generic_insts);
        (mr.genericMethodTable, mr.genericMethodTableCount) = to_raw(self.generic_method_table);
        (mr.types, mr.typesCount) = to_raw(self.types);
        (mr.methodSpecs, mr.methodSpecsCount) = to_raw(self.method_specs);
        let fo = to_raw(self.field_offsets);
        (mr.fieldOffsets, mr.fieldOffsetsCount) = (fo.0 as _, fo.1);
        let tds = to_raw(self.type_definition_sizes);
        (mr.typeDefinitionsSizes, mr.typeDefinitionsSizesCount) = (tds.0 as _, tds.1);
        assert!(self.metadata_usages.len() <= u32::MAX as usize);
        let mu = to_raw(self.metadata_usages);
        (mr.metadataUsages, mr.metadataUsagesCount) = (mu.0, mu.1 as _);
    }
}
