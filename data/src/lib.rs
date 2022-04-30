use bincode::{Decode, Encode};

type TypeDescriptionIdx = usize;
type TypeDefDescriptionIdx = usize;

#[derive(Encode, Decode)]
pub struct TypeDefDescription {
    pub name: String,
    pub namespace: String,
}

#[derive(Encode, Decode)]
pub enum TypeDescriptionData {
    TypeDefIdx(TypeDefDescriptionIdx),
}

#[derive(Encode, Decode)]
pub struct TypeDescription {
    data: TypeDescriptionData,
    attrs: u16,
    by_ref: bool,
}

#[derive(Encode, Decode)]
pub struct MethodDescription {
    defining_type: TypeDefDescriptionIdx,
    name: String,
    params: Vec<TypeDescriptionIdx>,
    return_ty: TypeDescriptionIdx,
}

#[derive(Encode, Decode)]
pub struct MergeModData {
    type_def_descriptions: Vec<TypeDefDescription>,
    type_descriptions: Vec<TypeDescription>,
    method_descriptions: Vec<MethodDescription>,
}
