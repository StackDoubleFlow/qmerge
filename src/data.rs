use crate::type_definitions::Il2CppType;
use il2cpp_metadata_raw::Metadata;

pub struct ModDataBuilder<'md> {
    metadata: &'md Metadata<'md>,
}

impl<'md> ModDataBuilder<'md> {
    fn new(metadata: &'md Metadata) -> Self {
        ModDataBuilder { metadata }
    }
}
