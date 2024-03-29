use color_eyre::eyre::{bail, ContextCompat, Result};
use std::collections::HashMap;

use super::parser::SourceParser;

#[derive(Clone, Copy, Debug)]
pub enum Il2CppTypeEnum {
    Void,
    Boolean,
    Char,
    I1,
    U1,
    I2,
    U2,
    I4,
    U4,
    I8,
    U8,
    R4,
    R8,
    String,
    Ptr,
    Valuetype,
    Class,
    Var,
    Array,
    Genericinst,
    Typedbyref,
    I,
    U,
    Object,
    Szarray,
    Mvar,
}

impl Il2CppTypeEnum {
    pub fn get_id(self) -> u8 {
        match self {
            Il2CppTypeEnum::Void => 0x01,
            Il2CppTypeEnum::Boolean => 0x02,
            Il2CppTypeEnum::Char => 0x03,
            Il2CppTypeEnum::I1 => 0x04,
            Il2CppTypeEnum::U1 => 0x05,
            Il2CppTypeEnum::I2 => 0x06,
            Il2CppTypeEnum::U2 => 0x07,
            Il2CppTypeEnum::I4 => 0x08,
            Il2CppTypeEnum::U4 => 0x09,
            Il2CppTypeEnum::I8 => 0x0a,
            Il2CppTypeEnum::U8 => 0x0b,
            Il2CppTypeEnum::R4 => 0x0c,
            Il2CppTypeEnum::R8 => 0x0d,
            Il2CppTypeEnum::String => 0x0e,
            Il2CppTypeEnum::Ptr => 0x0f,
            Il2CppTypeEnum::Valuetype => 0x11,
            Il2CppTypeEnum::Class => 0x12,
            Il2CppTypeEnum::Var => 0x13,
            Il2CppTypeEnum::Array => 0x14,
            Il2CppTypeEnum::Genericinst => 0x15,
            Il2CppTypeEnum::Typedbyref => 0x16,
            Il2CppTypeEnum::I => 0x18,
            Il2CppTypeEnum::U => 0x19,
            Il2CppTypeEnum::Object => 0x1c,
            Il2CppTypeEnum::Szarray => 0x1d,
            Il2CppTypeEnum::Mvar => 0x1e,
        }
    }

    fn from_name(name: &str) -> Result<Il2CppTypeEnum> {
        let type_enum = match name {
            "IL2CPP_TYPE_VOID" => Il2CppTypeEnum::Void,
            "IL2CPP_TYPE_BOOLEAN" => Il2CppTypeEnum::Boolean,
            "IL2CPP_TYPE_CHAR" => Il2CppTypeEnum::Char,
            "IL2CPP_TYPE_I1" => Il2CppTypeEnum::I1,
            "IL2CPP_TYPE_U1" => Il2CppTypeEnum::U1,
            "IL2CPP_TYPE_I2" => Il2CppTypeEnum::I2,
            "IL2CPP_TYPE_U2" => Il2CppTypeEnum::U2,
            "IL2CPP_TYPE_I4" => Il2CppTypeEnum::I4,
            "IL2CPP_TYPE_U4" => Il2CppTypeEnum::U4,
            "IL2CPP_TYPE_I8" => Il2CppTypeEnum::I8,
            "IL2CPP_TYPE_U8" => Il2CppTypeEnum::U8,
            "IL2CPP_TYPE_R4" => Il2CppTypeEnum::R4,
            "IL2CPP_TYPE_R8" => Il2CppTypeEnum::R8,
            "IL2CPP_TYPE_STRING" => Il2CppTypeEnum::String,
            "IL2CPP_TYPE_PTR" => Il2CppTypeEnum::Ptr,
            "IL2CPP_TYPE_VALUETYPE" => Il2CppTypeEnum::Valuetype,
            "IL2CPP_TYPE_CLASS" => Il2CppTypeEnum::Class,
            "IL2CPP_TYPE_VAR" => Il2CppTypeEnum::Var,
            "IL2CPP_TYPE_ARRAY" => Il2CppTypeEnum::Array,
            "IL2CPP_TYPE_GENERICINST" => Il2CppTypeEnum::Genericinst,
            "IL2CPP_TYPE_TYPEDBYREF" => Il2CppTypeEnum::Typedbyref,
            "IL2CPP_TYPE_I" => Il2CppTypeEnum::I,
            "IL2CPP_TYPE_U" => Il2CppTypeEnum::U,
            "IL2CPP_TYPE_OBJECT" => Il2CppTypeEnum::Object,
            "IL2CPP_TYPE_SZARRAY" => Il2CppTypeEnum::Szarray,
            "IL2CPP_TYPE_MVAR" => Il2CppTypeEnum::Mvar,
            _ => bail!("invalid type enum name: {}", name),
        };
        Ok(type_enum)
    }
}

#[derive(Debug)]
pub enum Il2CppTypeData<'src> {
    /// for VALUETYPE and CLASS (and I guess everything else)
    TypeDefIdx(usize),
    /// for VAR and MVAR
    GenericParamIdx(usize),
    /// for PTR and SZARRAY
    Il2CppType(&'src str),
    /// for ARRAY
    Il2CppArrayType(&'src str),
    /// for GENERICINST
    Il2CppGenericClass(&'src str),
}

#[derive(Debug)]
pub struct Il2CppType<'src> {
    pub data: Il2CppTypeData<'src>,
    pub attrs: u16,
    pub ty: Il2CppTypeEnum,
    pub byref: bool,
    pub pinned: bool,
}

pub struct GenericClass<'src> {
    pub ty_def_idx: Option<usize>,
    pub class_inst: Option<&'src str>,
}

pub struct TypeDefinitionsFile<'src> {
    pub types: Vec<Il2CppType<'src>>,
    pub ty_name_map: HashMap<&'src str, usize>,
    pub generic_classes: Vec<GenericClass<'src>>,
    pub gc_name_map: HashMap<&'src str, usize>,
}

/// Parse Il2CppTypeDefinitions.c and Il2CppGenericClassTable.c
pub fn parse<'src>(src: &'src str, gct_src: &'src str) -> Result<TypeDefinitionsFile<'src>> {
    let mut types = HashMap::new();
    let mut generic_classes = HashMap::new();
    for line in src.lines() {
        if line.starts_with("const Il2CppType ") {
            let words: Vec<&str> = line.split_whitespace().collect();
            let name = words[2];
            let data = words[5].trim_end_matches(',').trim_start_matches("(void*)");
            let attrs: u16 = words[6].trim_end_matches(',').parse()?;
            let ty = Il2CppTypeEnum::from_name(words[7].trim_end_matches(','))?;
            let byref = words[9].trim_end_matches(',').parse::<u8>()? != 0;
            let pinned = words[10].parse::<u8>()? != 0;

            let data = match ty {
                Il2CppTypeEnum::Var | Il2CppTypeEnum::Mvar => {
                    Il2CppTypeData::GenericParamIdx(data.parse()?)
                }
                Il2CppTypeEnum::Ptr | Il2CppTypeEnum::Szarray => {
                    Il2CppTypeData::Il2CppType(data.trim_start_matches('&'))
                }
                Il2CppTypeEnum::Array => {
                    Il2CppTypeData::Il2CppArrayType(data.trim_start_matches('&'))
                }
                Il2CppTypeEnum::Genericinst => {
                    Il2CppTypeData::Il2CppGenericClass(data.trim_start_matches('&'))
                }
                _ => Il2CppTypeData::TypeDefIdx(data.parse()?),
            };

            let ty = Il2CppType {
                data,
                attrs,
                ty,
                byref,
                pinned,
            };
            types.insert(name, ty);
        } else if line.starts_with("Il2CppGenericClass ") {
            let words: Vec<&str> = line.split_whitespace().collect();
            let name = words[1];
            let ty_def_idx = words[4].trim_end_matches(',').parse::<isize>()?;
            let class_inst = words[6].trim_start_matches('&').trim_end_matches(',');
            generic_classes.insert(
                name,
                GenericClass {
                    ty_def_idx: if ty_def_idx != -1 {
                        Some(ty_def_idx as usize)
                    } else {
                        None
                    },
                    class_inst: if class_inst == "NULL" {
                        None
                    } else {
                        Some(class_inst)
                    },
                },
            );
        }
    }

    let mut ty_name_map = HashMap::new();
    let mut types_arr = Vec::new();
    let arr_start = src
        .find("const Il2CppType* const  g_Il2CppTypeTable")
        .context("could not find g_Il2CppTypeTable")?;
    for (i, line) in src[arr_start..].lines().skip(3).enumerate() {
        if line.starts_with('}') {
            break;
        }
        let name = line.trim().trim_start_matches('&').trim_end_matches(',');
        ty_name_map.insert(name, i);
        types_arr.push(
            types
                .remove(name)
                .context("type table contained non-existant type")?,
        );
    }

    let mut gc_name_map = HashMap::new();
    let mut gc_arr = Vec::new();
    let gc_arr_start = gct_src
        .find("Il2CppGenericClass* const s_Il2CppGenericTypes")
        .context("could not find s_Il2CppGenericTypes")?;
    for (i, line) in gct_src[gc_arr_start..].lines().skip(3).enumerate() {
        if line.starts_with('}') {
            break;
        }
        let name = line.trim().trim_start_matches('&').trim_end_matches(',');
        gc_name_map.insert(name, i);
        gc_arr.push(
            generic_classes
                .remove(name)
                .context("gc table contained non-existant generic class")?,
        );
    }

    Ok(TypeDefinitionsFile {
        types: types_arr,
        ty_name_map,

        generic_classes: gc_arr,
        gc_name_map,
    })
}

pub struct SourceGenericInst<'src> {
    pub types: Vec<&'src str>,
}

pub fn parse_inst_defs(src: &str) -> Result<(Vec<SourceGenericInst>, HashMap<&str, usize>)> {
    let parser = SourceParser::new(src);

    let mut insts = HashMap::new();
    let insts_start = match src.find("static const Il2CppType* ") {
        Some(insts_start) => insts_start,
        None => return Ok(Default::default()),
    };
    let lines = src[insts_start..].lines().step_by(3);
    for line in lines {
        if !line.starts_with("static const Il2CppType*") {
            break;
        }

        let words = line.split_whitespace().collect::<Vec<_>>();
        let name = words[3]
            .strip_suffix("_Types[]")
            .context("generic inst def has wrong name suffix")?;
        let types = words[6..words.len() - 1]
            .iter()
            .map(|item| {
                item.trim_end_matches(',')
                    .trim_end_matches(')')
                    .trim_start_matches("(&")
            })
            .collect();
        insts.insert(name, SourceGenericInst { types });
    }

    let mut name_map = HashMap::new();
    let mut arr = Vec::new();
    parser
        .parse_array("const Il2CppGenericInst* const", "g_Il2CppGenericInstTable")?
        .enumerate()
        .for_each(|(i, str)| {
            let name = str.trim_start_matches('&');
            name_map.insert(name, i);
            arr.push(
                insts
                    .remove(name)
                    .context("gc table contained non-existant generic class")
                    .unwrap(),
            );
        });

    Ok((arr, name_map))
}

pub struct GenericMethodSpec {
    pub method_def: usize,
    pub class_inst: Option<usize>,
    pub method_isnt: Option<usize>,
}

pub fn parse_generic_method_defs(src: &str) -> Result<Vec<GenericMethodSpec>> {
    let mut specs = Vec::new();
    let arr_start = src
        .find("const Il2CppMethodSpec g_Il2CppMethodSpecTable")
        .context("could not find g_Il2CppMethodSpecTable")?;
    for line in src[arr_start..].lines().skip(3) {
        if line.starts_with('}') {
            break;
        }
        let words = line.split_whitespace().collect::<Vec<_>>();
        let class_inst = words[2].trim_end_matches(',').parse::<isize>()?;
        let method_isnt = words[3].trim_end_matches(',').parse::<isize>()?;
        specs.push(GenericMethodSpec {
            method_def: words[1].trim_end_matches(',').parse()?,
            class_inst: class_inst.try_into().ok(),
            method_isnt: method_isnt.try_into().ok(),
        });
    }

    Ok(specs)
}

pub struct SrcGenericMethodFuncs {
    pub generic_method_idx: usize,

    pub method_idx: usize,
    pub invoker_idx: usize,
    pub adjustor_thunk_idx: Option<usize>,
}

pub fn parse_generic_method_table(src: &str) -> Result<Vec<SrcGenericMethodFuncs>> {
    let mut methods = Vec::new();
    let arr_start = src
        .find("const Il2CppGenericMethodFunctionsDefinitions s_Il2CppGenericMethodFunctions")
        .context("could not find s_Il2CppGenericMethodFunctions")?;
    for line in src[arr_start..].lines().skip(3) {
        if line.starts_with('}') {
            break;
        }
        let words = line.split_whitespace().collect::<Vec<_>>();
        let generic_method_idx = words[1].trim_end_matches(',').parse()?;
        let method_idx = words[2].split('/').next().unwrap().parse()?;
        let invoker_idx = words[3].split('/').next().unwrap().parse()?;
        let adjustor_thunk_idx = words[4].trim_end_matches("},").parse::<isize>()?;
        methods.push(SrcGenericMethodFuncs {
            generic_method_idx,
            method_idx,
            invoker_idx,
            adjustor_thunk_idx: adjustor_thunk_idx.try_into().ok(),
        });
    }

    Ok(methods)
}
