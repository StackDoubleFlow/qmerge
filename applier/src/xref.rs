use anyhow::{anyhow, bail, Result};
use bad64::{Imm, Instruction, Op, Operand};
use dlopen::raw::Library;
use serde::Deserialize;
use std::fs;
use std::lazy::SyncLazy;
use tracing::debug;

use crate::get_mod_data_path;

static LIBIL2CPP: SyncLazy<Library> = SyncLazy::new(|| Library::open("libil2cpp.so").unwrap());

static XREF_DATA: SyncLazy<XRefData> = SyncLazy::new(|| {
    let path = get_mod_data_path().join("xref_gen.json");
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
});

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

pub fn get_symbol(name: &str) -> Result<*const ()> {
    let symbol_trace = XREF_DATA
        .traces
        .iter()
        .find(|st| st.symbol == name)
        .unwrap();

    let start: *const u32 = unsafe { LIBIL2CPP.symbol(&symbol_trace.start)? };

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
