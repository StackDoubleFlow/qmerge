use crate::il2cpp_types::*;
use anyhow::{ensure, Result};
use std::mem::{size_of, transmute};
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
            unsafe fn from_raw(original: *const u8, header: &Il2CppGlobalMetadataHeader) -> Self {
                $(
                    let $name = table_from_raw(original.offset(header.$offset_name as isize), header.$count_name);
                )*

                Self {
                    $(
                        $name,
                    )*
                }
            }

            fn build(self) -> *const u8 {
                let size = size_of::<Il2CppGlobalMetadataHeader>() $(+ table_size(&self.$name))*;
                let data: *mut u8 = Box::into_raw(vec![0u8; size].into_boxed_slice()).cast();

                let mut cur: *mut u8 = unsafe { data.offset(size_of::<Il2CppGlobalMetadataHeader>() as isize) };

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

struct CodeRegistrationBuilder {
    generic_method_pointers: Vec<Il2CppMethodPointer>,
    generic_adjuster_thunks: Vec<Il2CppMethodPointer>,
    invoker_pointers: Vec<InvokerMethod>,
    custom_attribute_generators: Vec<CustomAttributesCacheGenerator>,
    code_gen_modules: Vec<*const Il2CppCodeGenModule>,
}

struct MetadataRegistrationBuilder {
    generic_classes: Vec<*const Il2CppGenericClass>,
    generic_insts: Vec<*const Il2CppGenericInst>,
    generic_method_table: Vec<Il2CppGenericMethodFunctionsDefinitions>,
    types: Vec<*const Il2CppType>,
    method_specs: Vec<Il2CppMethodSpec>,
    field_offsets: Vec<*const i32>,
    type_definition_sizes: Vec<*const Il2CppTypeDefinitionSizes>,
    metadata_usages: Vec<*const ()>,
}

pub struct MetadataBuilder {
    code_registration_raw: *mut *const Il2CppCodeRegistration,
    metadata_registration_raw: *mut *const Il2CppMetadataRegistration,

    metadata: Metadata,
}

impl MetadataBuilder {
    pub fn new(
        code_registration_raw: *mut *const Il2CppCodeRegistration,
        metadata_registration_raw: *mut *const Il2CppMetadataRegistration,
        global_metadata: *const u8,
    ) -> Result<Self> {
        let header: &Il2CppGlobalMetadataHeader = unsafe { transmute(global_metadata) };
        ensure!(header.sanity == SANITY);
        ensure!(header.version == 24);

        let metadata = unsafe { Metadata::from_raw(global_metadata, header) };

        Ok(Self {
            code_registration_raw,
            metadata_registration_raw,

            metadata,
        })
    }

    pub fn finish(self) -> *const u8 {
        self.metadata.build()
    }
}
