use bincode::{Decode, Encode};

type TypeDescriptionIdx = usize;
type TypeDefDescriptionIdx = usize;

#[derive(Encode, Decode, Debug)]
pub struct TypeDefDescription {
    pub name: String,
    pub namespace: String,
}

#[derive(Encode, Decode, Debug)]
pub enum TypeDescriptionData {
    TypeDefIdx(TypeDefDescriptionIdx),
}

#[derive(Encode, Decode, Debug)]
pub struct TypeDescription {
    pub data: TypeDescriptionData,
    pub attrs: u16,
    pub by_ref: bool,
}

#[derive(Encode, Decode, Debug)]
pub struct MethodDescription {
    pub defining_type: TypeDefDescriptionIdx,
    pub name: String,
    pub params: Vec<TypeDescriptionIdx>,
    pub return_ty: TypeDescriptionIdx,
}

#[derive(Encode, Decode, Debug)]
pub struct MergeModData {
    pub type_def_descriptions: Vec<TypeDefDescription>,
    pub type_descriptions: Vec<TypeDescription>,
    pub method_descriptions: Vec<MethodDescription>,
}
