use crate::data_dirs::MOD_DATA_PATH;
use crate::loader::metadata_builder::{CodeRegistrationBuilder, Metadata};
use crate::utils::{get_str, offset_len};
use anyhow::{anyhow, bail, Context, Result};
use bad64::{Imm, Instruction, Op, Operand};
use dlopen::raw::Library;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::mem::transmute;
use std::sync::{LazyLock, OnceLock};
use tracing::debug;

static LIBIL2CPP: LazyLock<Library> = LazyLock::new(|| Library::open("libil2cpp.so").unwrap());

static XREF_DATA: LazyLock<XRefData> = LazyLock::new(|| {
    let path = MOD_DATA_PATH.join("xref_gen.json");
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
});
static XREF_ROOTS: OnceLock<HashMap<(String, String, String), Il2CppRoot>> = OnceLock::new();

#[derive(Debug)]
struct Il2CppRoot {
    method_pointer: Option<*const u32>,
    invoker: Option<*const u32>,
}

unsafe impl Send for Il2CppRoot {}
unsafe impl Sync for Il2CppRoot {}

impl Il2CppRoot {
    fn get(
        token: u32,
        image_name: &str,
        code_registration: &CodeRegistrationBuilder,
    ) -> Result<Self> {
        let rid = 0x00FFFFFF & token;
        let module = code_registration
            .find_module(image_name)
            .context("could not find module for xref trace")?;
        let method_pointer = unsafe {
            let ptr = module.methodPointers.add(rid as usize - 1);
            if ptr.is_null() {
                None
            } else {
                Some(transmute(ptr.read()))
            }
        };
        let invoker = unsafe {
            let invoker_idx = module.invokerIndices.add(rid as usize - 1).read();
            if invoker_idx != -1 {
                Some(transmute(
                    code_registration.invoker_pointers[invoker_idx as usize],
                ))
            } else {
                None
            }
        };

        Ok(Self {
            method_pointer,
            invoker,
        })
    }
}

pub fn initialize_roots(
    metadata: &Metadata,
    code_registration: &CodeRegistrationBuilder,
) -> Result<()> {
    let mut required_roots = HashSet::new();
    for trace in &XREF_DATA.traces {
        if trace.start.starts_with("il2cpp:") || trace.start.starts_with("invoker:") {
            let parts: Vec<&str> = trace.start.split(':').collect();
            let namespace = parts[1];
            let class = parts[2];
            let method_name = parts[3];
            required_roots.insert((namespace, class, method_name));
        }
    }

    let mut roots = HashMap::new();
    for image in &metadata.images {
        let image_name = get_str(&metadata.string, image.nameIndex as usize)?;
        let type_defs_range = offset_len(image.typeStart, image.typeCount as i32);
        for type_def in &metadata.type_definitions[type_defs_range] {
            let method_range = offset_len(type_def.methodStart, type_def.method_count as i32);
            let namespace = get_str(&metadata.string, type_def.namespaceIndex as usize)?;
            let class = get_str(&metadata.string, type_def.nameIndex as usize)?;
            for method in &metadata.methods[method_range] {
                let method_name = get_str(&metadata.string, method.nameIndex as usize)?;
                if required_roots
                    .take(&(namespace, class, method_name))
                    .is_some()
                {
                    let root = Il2CppRoot::get(method.token, image_name, code_registration)?;
                    roots.insert(
                        (
                            namespace.to_string(),
                            class.to_string(),
                            method_name.to_string(),
                        ),
                        root,
                    );
                }
            }
        }
    }

    XREF_ROOTS.set(roots).unwrap();

    Ok(())
}

#[derive(Deserialize, Debug)]
struct SymbolTrace {
    symbol: String,
    start: String,
    trace: String,
}

#[derive(Deserialize)]
pub struct XRefData {
    traces: Vec<SymbolTrace>,
}

unsafe fn load_ins(addr: *const u32) -> Result<Instruction> {
    let data = addr.read();
    bad64::decode(data, addr as u64)
        .map_err(|err| anyhow!("decode error during xref walk: {}", err))
}

pub fn get_data_symbol<T>(name: &str) -> Result<*mut T> {
    let symbol = get_symbol(name);
    unsafe { std::mem::transmute(symbol) }
}

fn get_root(namespace: &str, class: &str, method_name: &str) -> Result<&'static Il2CppRoot> {
    XREF_ROOTS
        .get()
        .context("il2cpp xref roots has not been initialized yet")?
        .get(&(
            namespace.to_string(),
            class.to_string(),
            method_name.to_string(),
        ))
        .context("could not find root")
}

pub fn get_symbol(name: &str) -> Result<*const ()> {
    let symbol_trace = XREF_DATA
        .traces
        .iter()
        .find(|st| st.symbol == name)
        .unwrap();

    let start: *const u32 = if symbol_trace.start.starts_with("il2cpp:") {
        let parts: Vec<&str> = symbol_trace.start.split(':').collect();
        let root = get_root(parts[1], parts[2], parts[3])?;
        root.method_pointer
            .context("root does not have method pointer")?
    } else if symbol_trace.start.starts_with("invoker:") {
        let parts: Vec<&str> = symbol_trace.start.split(':').collect();
        let root = get_root(parts[1], parts[2], parts[3])?;
        root.invoker.context("root does not have invoker pointer")?
    } else {
        unsafe { LIBIL2CPP.symbol(&symbol_trace.start)? }
    };

    let nums = symbol_trace
        .trace
        .split(|c| ('A'..='Z').contains(&c))
        .filter(|s| !s.is_empty())
        .map(|s| s.parse::<usize>());
    let ops = symbol_trace
        .trace
        .chars()
        .filter(|&c| char::is_alphabetic(c));

    let mut addr = start;
    for (op, num) in ops.zip(nums) {
        let num = num?;
        let mut count = 0;
        loop {
            let ins = unsafe { load_ins(addr)? };
            match ins.op() {
                Op::BL if op == 'L' => {
                    if count == num {
                        let to = match ins.operands()[0] {
                            Operand::Label(Imm::Unsigned(to)) => to,
                            _ => bail!("bl had wrong operand"),
                        };
                        addr = to as _;
                        break;
                    }
                    count += 1;
                }
                Op::B if op == 'B' => {
                    if count == num {
                        let to = match ins.operands()[0] {
                            Operand::Label(Imm::Unsigned(to)) => to,
                            _ => bail!("b had wrong operand"),
                        };
                        addr = to as _;
                        break;
                    }
                    count += 1;
                }
                Op::ADRP if op == 'P' => {
                    if count == num {
                        let (base, reg) = match ins.operands() {
                            [Operand::Reg { reg, .. }, Operand::Label(Imm::Unsigned(imm))] => {
                                (*imm, *reg)
                            }
                            _ => bail!("adrp had wrong operands"),
                        };
                        loop {
                            addr = unsafe { addr.offset(1) };
                            let ins = unsafe { load_ins(addr)? };
                            match (ins.op(), ins.operands()) {
                                (
                                    Op::LDR,
                                    [Operand::Reg { .. }, Operand::MemOffset {
                                        reg: a,
                                        offset: Imm::Signed(imm),
                                        ..
                                    }],
                                ) if reg == *a => {
                                    addr = ((base as i64) + imm) as _;
                                    break;
                                }
                                (
                                    Op::ADD,
                                    [Operand::Reg { .. }, Operand::Reg { reg: a, .. }, Operand::Imm64 {
                                        imm: Imm::Unsigned(imm),
                                        ..
                                    }],
                                ) if reg == *a => {
                                    addr = (base + imm) as _;
                                    break;
                                }
                                _ => {}
                            }
                        }
                        break;
                    }
                    count += 1;
                }
                _ => {}
            }
            addr = unsafe { addr.offset(1) };
        }
    }

    debug!("Found symbol {} at address {:?}", name, addr);

    Ok(addr as _)
}
