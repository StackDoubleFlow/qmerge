use bincode::error::{DecodeError, EncodeError};
use bincode::{Decode, Encode};

type TypeDescriptionIdx = usize;
type TypeDefDescriptionIdx = usize;
type MethodDescriptionIdx = usize;
type StringLiteralIdx = usize;

type GenericInstIdx = usize;
type GenericMethodInstIdx = usize;
type GenericClassInstIdx = usize;

#[derive(Encode, Decode, Debug)]
pub struct TypeDefDescription {
    pub name: String,
    pub namespace: String,
}

#[derive(Encode, Decode, Debug)]
pub enum TypeDescriptionData {
    /// for VALUETYPE and CLASS
    TypeDefIdx(TypeDefDescriptionIdx),
    /// for PTR and SZARRAY
    TypeIdx(TypeDescriptionIdx),
    // TODO: Arrays
    // Adding the owner here could make this self referencial during link
    /// for VAR and MVAR
    GenericParam(u16),
    /// for GENERICINST
    GenericClass(GenericClassInstIdx),
}

#[derive(Encode, Decode, Debug)]
pub struct TypeDescription {
    pub data: TypeDescriptionData,
    pub attrs: u16,
    pub ty: u8,
    pub by_ref: bool,
    pub pinned: bool,
}

#[derive(Encode, Decode, Debug)]
pub struct MethodDescription {
    pub defining_type: TypeDefDescriptionIdx,
    pub name: String,
    pub params: Vec<TypeDescriptionIdx>,
    pub return_ty: TypeDescriptionIdx,
}

#[derive(Encode, Decode, Debug)]
pub struct AddedAssembly {
    // Il2CppAssemblyNameDefinition
    pub name: String,
    pub culture: String,
    pub public_key: String,
    pub hash_alg: u32,
    pub hash_len: u32,
    pub flags: u32,
    pub major: u32,
    pub minor: u32,
    pub build: u32,
    pub revision: u32,
    pub public_key_token: [u8; 8],

    pub token: u32,
}

#[derive(Encode, Decode, Debug)]
pub struct AddedImage {
    pub name: String,
    pub token: u32,
}

#[derive(Encode, Decode, Debug)]
pub struct AddedField {
    pub name: String,
    pub ty: TypeDescriptionIdx,
    pub token: u32,
}

#[derive(Encode, Decode, Debug)]
pub struct AddedParameter {
    pub name: String,
    pub token: u32,
    pub ty: TypeDescriptionIdx,
}

#[derive(Encode, Decode, Debug)]
pub struct AddedMethod {
    pub name: String,
    pub declaring_type: TypeDefDescriptionIdx,
    pub return_ty: TypeDescriptionIdx,
    pub parameters: Vec<AddedParameter>,
    pub generic_container: Option<AddedGenericContainer>,
    pub token: u32,
    pub flags: u16,
    pub iflags: u16,
    pub slot: u16,
}

#[derive(Encode, Decode, Debug)]
pub struct AddedEvent {
    pub name: String,
    pub ty: TypeDescriptionIdx,
    pub add: MethodDescriptionIdx,
    pub remove: MethodDescriptionIdx,
    pub raise: MethodDescriptionIdx,
    pub token: u32,
}

#[derive(Encode, Decode, Debug)]
pub struct AddedProperty {
    pub name: String,
    pub get: MethodDescriptionIdx,
    pub set: MethodDescriptionIdx,
    pub attrs: u32,
    pub token: u32,
}

#[derive(Encode, Decode, Debug, Clone, Copy)]
pub enum EncodedMethodIndex {
    Il2CppClass(TypeDescriptionIdx),
    Il2CppType(TypeDescriptionIdx),
    MethodInfo(MethodDescriptionIdx),
    StringLiteral(StringLiteralIdx),
    MethodRef(GenericMethodInstIdx),
    // TODO:
    // FieldInfo
}

#[derive(Encode, Decode, Debug)]
pub struct AddedTypeDefinition {
    pub name: String,
    pub namespace: String,
    pub byval_type: TypeDescriptionIdx,
    pub byref_type: TypeDescriptionIdx,

    pub declaring_type: Option<TypeDescriptionIdx>,
    pub parent_type: Option<TypeDescriptionIdx>,
    pub element_type: TypeDescriptionIdx,

    pub generic_container: Option<AddedGenericContainer>,

    pub flags: u32,

    pub fields: Vec<AddedField>,
    pub methods: Vec<AddedMethod>,
    pub events: Vec<AddedEvent>,
    pub properties: Vec<AddedProperty>,
    pub nested_types: Vec<TypeDefDescriptionIdx>,
    pub interfaces: Vec<TypeDescriptionIdx>,
    pub vtable: Vec<EncodedMethodIndex>,
    pub interface_offsets: Vec<(TypeDescriptionIdx, u32)>,

    pub bitfield: u32,
    pub token: u32,
}

#[derive(Encode, Decode, Debug)]
pub struct AddedMetadataUsagePair {
    pub source: EncodedMethodIndex,
    pub dest: usize,
}

#[derive(Encode, Decode, Debug)]
pub enum GenericContainerOwner {
    Class(TypeDefDescriptionIdx),
    Method(MethodDescriptionIdx),
}

#[derive(Encode, Decode, Debug)]
pub struct AddedGenericParameter {
    pub name: String,
    pub constraints: Vec<TypeDescriptionIdx>,
    pub flags: u16,
}

#[derive(Encode, Decode, Debug)]
pub struct AddedGenericContainer {
    pub owner: GenericContainerOwner,
    pub parameters: Vec<AddedGenericParameter>,
}

#[derive(Encode, Decode, Debug)]
pub struct GenericInst {
    pub types: Vec<TypeDescriptionIdx>,
}

#[derive(Encode, Decode, Debug)]
pub struct GenericContext {
    pub class: Option<GenericInstIdx>,
    pub method: Option<GenericInstIdx>,
}

#[derive(Encode, Decode, Debug)]
pub struct GenericMethodInst {
    pub method: MethodDescriptionIdx,
    pub context: GenericContext,
}

#[derive(Encode, Decode, Debug)]
pub struct GenericMethodFunctions {
    pub generic_method: GenericMethodInstIdx,

    pub method_idx: usize,
    pub invoker_idx: usize,
    pub adjuster_thunk_idx: Option<usize>,
}

#[derive(Encode, Decode, Debug)]
pub struct GenericClassInst {
    pub class: TypeDefDescriptionIdx,
    pub context: GenericContext,
}

#[derive(Encode, Decode, Debug)]
pub struct MergeModData {
    // Linkage information
    pub type_def_descriptions: Vec<TypeDefDescription>,
    pub type_descriptions: Vec<TypeDescription>,
    pub method_descriptions: Vec<MethodDescription>,

    // Added types
    pub added_assembly: AddedAssembly,
    pub added_image: AddedImage,
    pub added_type_defintions: Vec<AddedTypeDefinition>,
    pub added_usage_lists: Vec<Vec<AddedMetadataUsagePair>>,
    pub added_string_literals: Vec<String>,

    // Generics
    pub generic_instances: Vec<GenericInst>,
    pub generic_method_insts: Vec<GenericMethodInst>,
    pub generic_method_funcs: Vec<GenericMethodFunctions>,
    pub generic_class_insts: Vec<GenericClassInst>,
}

impl MergeModData {
    pub fn serialize(self) -> Result<Vec<u8>, EncodeError> {
        // TODO: use encode_into_std_write
        bincode::encode_to_vec(self, bincode::config::standard())
    }

    pub fn deserialize(data: &[u8]) -> Result<MergeModData, DecodeError> {
        bincode::decode_from_slice(data, bincode::config::standard()).map(|(data, _)| data)
    }
}
