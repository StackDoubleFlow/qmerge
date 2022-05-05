use deku::prelude::*;

#[derive(PartialEq, Eq, DekuRead)]
pub struct Il2CppType {
    pub data: usize,
    pub attrs: u16,
    pub ty: u8,
    // unused in practice
    #[deku(bits = 6)]
    pub num_mods: u8,
    #[deku(bits = 1)]
    pub byref: bool,
    #[deku(bits = 1)]
    pub pinned: bool,
}
