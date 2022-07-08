use std::collections::{HashSet, HashMap};
use std::mem::transmute;
use tracing::debug;
use std::fmt::Write;
use crate::utils::get_fields;
use super::{CodegenMethod, ParamInjection, abi::ParameterStorage};

/// Fix up an ldr with an offset to data
struct DataFixup {
    ins_idx: usize,
    data: usize,
}

#[derive(Default)]
pub struct PostfixGenerator {
    stack_offset: usize,
    using_gprs: HashMap<u32, usize>,
    using_fprs: HashMap<u32, usize>,

    code: Vec<u32>,
    data: Vec<DataFixup>,
}

impl PostfixGenerator {
    fn use_gpr(&mut self, num: u32) -> usize {
        *self.using_gprs.entry(num).or_insert_with(|| {
            let offset = self.stack_offset;
            self.stack_offset += 8;
            offset
        })
    }

    fn use_fpr(&mut self, num: u32) -> usize {
        *self.using_gprs.entry(num).or_insert_with(|| {
            let offset = self.stack_offset;
            self.stack_offset += 8;
            offset
        })
    }

    fn load_gpr(&mut self, num: u32, to: u32) {
        let offset = self.use_gpr(num) as u32;
        let ins = 0xf94003e0 | (offset << 10) | to;
        self.code.push(ins); // ldr x<to>, [sp, #<offset>]
    }

    fn load_base_offset(&mut self, dest: u32, base: u32, offset: u32) {
        let offset = offset / 8;
        let ins = 0xf9400000 | (offset << 10) | (base << 5) | dest;
        self.code.push(ins); // ldr x<dest> [x<base>, #<offset>]
    }

    fn call_addr(&mut self, addr: usize) {
        self.data.push(DataFixup {
            data: addr,
            ins_idx: self.code.len()
        });
        self.code.push(0x58000009); // ldr x9, 0x0
        self.code.push(0xd63f0120); // blr x9
    }

    fn push_code_front(&mut self, code: Vec<u32>) {
        for fixup in &mut self.data {
            fixup.ins_idx += code.len();
        }
        self.code.splice(0..0, code);
    }

    fn write_prologue_epilogue(&mut self) {
        let mut prologue = Vec::new();

        let stack_offset = self.stack_offset as u32;
        prologue.push(0xd10003ff | (stack_offset << 10)); // sub sp, sp, #<stack_offset>

        for (&gpr, &offset) in &self.using_gprs {
            let offset = offset as u32 / 8;
            let ins = 0xf90003e0 | (offset << 10) | gpr;
            prologue.push(ins);
        }

        self.push_code_front(prologue);

        self.code.push(0x910003ff | (stack_offset << 10)); // add sp, sp, #<stack_offset>
        self.code.push(0xd65f03c0); // ret
    }

    pub(super) fn gen_postfix(&mut self, original: CodegenMethod, postfix: CodegenMethod, injections: Vec<ParamInjection>) {
        self.call_addr(original.method.methodPointer.unwrap() as usize);
        // let code = Vec::new();
        // subtract sp
        // write used params to stack
        // call original
        // load injections
        // call postfix
        // add sp
    
        for (injection, storage) in injections.iter().zip(postfix.layout.iter()) {
            match injection {
                ParamInjection::LoadField(idx) => {
                    let fields = unsafe { get_fields(original.method.klass) };
                    let field = &fields[*idx];
                    match storage {
                        ParameterStorage::GPReg(num) => {
                            // instance param
                            self.load_gpr(0, *num);
                            self.load_base_offset(*num, 0, field.offset as u32);
                            self.call_addr(postfix.method.methodPointer.unwrap() as usize);
                        }
                        _ => todo!(),
                    }
                }
                _ => todo!()
            }
        }
    }

    pub fn finish(mut self) -> Vec<u32> {
        self.write_prologue_epilogue();
        let code_len = self.code.len();

        for (i, fixup) in self.data.iter().enumerate() {
            let offset = ((code_len - fixup.ins_idx) + i * 2) as u32;
            self.code[fixup.ins_idx] |= offset << 5;
            let parts: [u32; 2] = unsafe { transmute(fixup.data) };
            self.code.push(parts[0]);
            self.code.push(parts[1]);
        }

        debug!("dumping generated code");
        for (i, &ins) in self.code[0..code_len].iter().enumerate() {
            debug!("{}", bad64::decode(ins, i as u64 * 4).unwrap());
        }
        let mut data_str = String::from("data: ");
        for &data in &self.code[code_len..] {
            let bytes: [u8; 4] = data.to_ne_bytes();
            for b in bytes {
                write!(data_str, "{:x}", b).unwrap();
            }
        }
        debug!("{}", data_str);

        self.code
    }
}


