#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
// 128 bit ints are not ffi safe
#![allow(improper_ctypes)]

include!(concat!(env!("OUT_DIR"), "/bindings_24_5.rs"));

unsafe impl Sync for Il2CppCodeRegistration {}
unsafe impl Sync for Il2CppMetadataRegistration {}
