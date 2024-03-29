use super::abi::{Arg, ParameterStorage};
use super::alloc::HOOK_ALLOCATOR;
use super::{CodegenMethod, ParamInjection};
use crate::utils::{get_fields, get_method_pointer};
use il2cpp_types::FieldInfo;
use inline_hook::Hook;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::mem::transmute;
use std::slice;
use tracing::{debug, instrument};

enum DataFixupInfo {
    Addr(usize),
    Orig,
}

/// Fix up an ldr with an offset to data
struct DataFixup {
    ins_idx: usize,
    info: DataFixupInfo,
}

#[derive(Default)]
struct Code {
    code: Vec<u32>,
    data: Vec<DataFixup>,
    param_spill_fixups: Vec<usize>,
}

impl Code {
    /// stp x<src1>, x<src2>, [x<base>/sp, #<offset>]
    fn store_pair(&mut self, src1: u32, src2: u32, base: u32, offset: u32) {
        let offset = offset / 8;
        let ins = 0xa9000000 | (offset << 15) | (base << 5) | src1 | (src2 << 10);
        self.code.push(ins);
    }

    /// ldp x<dest1>, x<dest2>, [x<base>/sp, #<offset>]
    fn load_pair(&mut self, dest1: u32, dest2: u32, base: u32, offset: u32) {
        let offset = offset / 8;
        let ins = 0xa9400000 | (offset << 15) | (base << 5) | dest1 | (dest2 << 10);
        self.code.push(ins);
    }

    /// ldr x<dest>, [x<base>/sp, #<offset>]
    fn load_base_offset(&mut self, dest: u32, base: u32, offset: u32) {
        self.load_sized(dest, base, offset, 8);
    }

    /// ldr x<dest>, [x<base>/sp, #<offset>]
    fn load_sized(&mut self, dest: u32, base: u32, offset: u32, size: usize) {
        let offset = offset / size as u32;
        let ins = match size {
            8 => 0xf9400000,
            4 => 0xb9400000,
            2 => 0x79400000,
            1 => 0x39400000,
            _ => unreachable!("load sized {}", size),
        };
        let ins = ins | (offset << 10) | (base << 5) | dest;
        self.code.push(ins);
    }

    fn load_spill(&mut self, dest: u32, offset: u32) {
        self.param_spill_fixups.push(self.code.len());
        self.load_base_offset(dest, 31, offset);
    }

    /// str x<src>, [x<base>/sp, #<offset>]
    fn store_base_offset(&mut self, src: u32, base: u32, offset: u32) {
        let offset = offset / 8;
        let ins = 0xf9000000 | (offset << 10) | (base << 5) | src;
        self.code.push(ins);
    }

    /// ldr d<dest>, [x<base>/sp, #<offset>]
    fn load_base_offset_fp(&mut self, dest: u32, base: u32, offset: u32, size: u32) {
        let offset = offset / size;
        let ins = match size {
            8 => 0xfd400000,
            4 => 0xbd400000,
            _ => unreachable!(),
        };
        let ins = ins | (offset << 10) | (base << 5) | dest;
        self.code.push(ins);
    }

    /// str d<src>, [x<base>/sp, #<offset>]
    fn store_base_offset_fp(&mut self, src: u32, base: u32, offset: u32, size: u32) {
        let offset = offset / size;
        let ins = match size {
            8 => 0xfd000000,
            4 => 0xbd000000,
            _ => unreachable!(),
        };
        let ins = ins | (offset << 10) | (base << 5) | src;
        self.code.push(ins);
    }

    /// add x<dest>, x<reg>, #<imm>
    fn add_imm(&mut self, dest: u32, reg: u32, imm: u32) {
        self.code.push(0x91000000 | (imm << 10) | (reg << 5) | dest);
    }

    /// sub x<dest>, x<reg>, #<imm>
    fn sub_imm(&mut self, dest: u32, reg: u32, imm: u32) {
        self.code.push(0xd1000000 | (imm << 10) | (reg << 5) | dest);
    }

    fn mov_imm(&mut self, dest: u32, imm: u32) {
        self.code.push(0xd2800000 | (imm << 5) | dest);
    }

    fn mov_reg(&mut self, dest: u32, src: u32) {
        self.code.push(0xaa0003e0 | (src << 16) | dest);
    }

    /// offset param counts in instructions
    fn branch_zero(&mut self, reg: u32, offset: u32) {
        self.code.push(0xb4000000 | (offset << 5) | reg);
    }

    fn ret(&mut self) {
        self.code.push(0xd65f03c0);
    }

    fn push_front(&mut self, other: Code) {
        if !other.data.is_empty() {
            todo!();
        }
        for fixup in &mut self.data {
            fixup.ins_idx += other.code.len();
        }
        self.param_spill_fixups
            .iter_mut()
            .for_each(|idx| *idx += other.code.len());
        self.code.splice(0..0, other.code);
    }

    fn call_addr(&mut self, addr: Option<usize>) {
        self.data.push(DataFixup {
            ins_idx: self.code.len(),
            info: match addr {
                Some(addr) => DataFixupInfo::Addr(addr),
                None => DataFixupInfo::Orig,
            },
        });
        self.code.push(0x58000009); // ldr x9, 0x0
        self.code.push(0xd63f0120); // blr x9
    }

    fn size(&self) -> usize {
        self.code.len() + self.data.len() * 2
    }

    fn copy_to(&mut self, dest: *mut u32, orig_addr: usize, stack_size: u32) {
        let stack_size_offset = stack_size / 8;
        for &ins_idx in &self.param_spill_fixups {
            self.code[ins_idx] += stack_size_offset << 10;
        }

        let fixup_data = self
            .data
            .iter()
            .map(|fixup| {
                (
                    fixup.ins_idx,
                    match fixup.info {
                        DataFixupInfo::Addr(addr) => addr,
                        DataFixupInfo::Orig => orig_addr,
                    },
                )
            })
            .collect::<Vec<_>>();

        for (i, &(ins_idx, _)) in fixup_data.iter().enumerate() {
            let offset = ((self.code.len() - ins_idx) + i * 2) as u32;
            self.code[ins_idx] |= offset << 5;
        }

        let code_slice = unsafe { slice::from_raw_parts_mut(dest, self.code.len()) };
        code_slice.copy_from_slice(&self.code);

        let data_ptr = unsafe { dest.add(self.code.len()) } as *mut usize;
        for (i, &(_, data)) in fixup_data.iter().enumerate() {
            unsafe {
                data_ptr.add(i).write(data);
            }
        }

        debug!("dumping generated code");
        for ins in &code_slice[0..self.code.len()] {
            let ptr = ins as *const u32;
            debug!(
                "{:?}: {:08x}:  {}",
                ptr,
                *ins,
                bad64::decode(*ins, ptr as u64).unwrap()
            );
        }
        let mut data_str = String::from("data: ");
        for i in 0..self.data.len() {
            let data = unsafe { data_ptr.add(i).read() };
            let bytes: [u8; 8] = data.to_ne_bytes();
            for b in bytes {
                write!(data_str, "{:02x}", b).unwrap();
            }
        }
        debug!("{}", data_str);
    }
}

pub struct HookGenerator<'a> {
    original: &'a CodegenMethod,
    valuetype_instance: bool,
    orig_param_offsets: Vec<u32>,
    instance_param_offset: Option<u32>,
    result_offset: Option<u32>,
    stack_offset: u32,
    run_original_offset: u32,
    code: Code,
}

impl<'a> HookGenerator<'a> {
    pub(super) fn new(
        original: &CodegenMethod,
        is_instance: bool,
        reserve_call_stack: u32,
    ) -> HookGenerator {
        let max_param_spill = reserve_call_stack.max(original.layout.stack_size);

        let mut hook_gen = HookGenerator {
            original,
            valuetype_instance: unsafe { &*original.method.klass }.valuetype() != 0,
            orig_param_offsets: Vec::new(),
            instance_param_offset: None,
            result_offset: None,
            stack_offset: max_param_spill + 8,
            run_original_offset: max_param_spill,
            code: Default::default(),
        };

        if is_instance {
            hook_gen
                .code
                .store_base_offset(0, 31, hook_gen.stack_offset);
            let instance_offset = hook_gen.stack_offset;
            hook_gen.stack_offset += 8;
            hook_gen.instance_param_offset = Some(instance_offset);
        }

        for arg in &original.layout.args {
            let offset = hook_gen.alloc_arg_on_stack(arg);
            hook_gen.store_arg(arg, offset);
            hook_gen.orig_param_offsets.push(offset);
        }

        if let Some(ret_layout) = &original.ret_layout {
            let old_stack_size = hook_gen.stack_offset;
            let offset = hook_gen.alloc_arg_on_stack(ret_layout);
            hook_gen.result_offset = Some(offset);

            // TODO: if size is not a multiple of 8 this won't work properly
            let size = hook_gen.stack_offset - old_stack_size;
            for i in 0..size / 8 {
                hook_gen.code.store_base_offset(31, 31, offset + i * 8);
            }
        }

        // run original by default
        hook_gen.code.mov_imm(11, 1);

        hook_gen
    }

    fn alloc_arg_on_stack(&mut self, arg: &Arg) -> u32 {
        // We'll just align everything to 8 for now
        self.stack_offset = (self.stack_offset as u32 + 7) & !7;
        let stack_offset = self.stack_offset;
        match arg.storage {
            ParameterStorage::GPReg(_) => {
                if arg.ptr {
                    self.stack_offset += arg.ty_size as u32;
                } else {
                    self.stack_offset += 8;
                }
            }
            ParameterStorage::GPRRange(_, count) => {
                self.stack_offset += 8 * count;
            }
            ParameterStorage::VectorReg(_) => {
                self.stack_offset += 8;
            }
            ParameterStorage::VectorRange(_, count, is_double) => {
                let size = is_double.then_some(8).unwrap_or(4);
                self.stack_offset += size * count;
            }
            ParameterStorage::Stack(_) => {
                self.stack_offset += arg.size as u32;
            }
            ParameterStorage::Unallocated => unreachable!(),
            _ => todo!("stack alloc storage {:?}", arg.storage),
        }
        stack_offset
    }

    fn store_arg(&mut self, arg: &Arg, stack_offset: u32) {
        match arg.storage {
            ParameterStorage::GPReg(reg) => {
                if arg.ptr {
                    todo!("copy structure to stack")
                }
                self.code.store_base_offset(reg, 31, stack_offset);
            }
            ParameterStorage::GPRRange(start, count) => {
                for i in 0..count {
                    let offset = stack_offset + i * 8;
                    self.code.store_base_offset(start + i, 31, offset);
                }
            }
            ParameterStorage::VectorReg(reg) => {
                self.code.store_base_offset_fp(reg, 31, stack_offset, 8);
            }
            ParameterStorage::VectorRange(start, count, is_double) => {
                let size = is_double.then_some(8).unwrap_or(4);
                for i in 0..count {
                    let offset = stack_offset + i * size;
                    self.code.store_base_offset_fp(start + i, 31, offset, size);
                }
            }
            ParameterStorage::Stack(offset) => {
                let count = arg.size / 8;
                for i in 0..count as u32 {
                    self.code.load_spill(9, offset + i * 8);
                    self.code.store_base_offset(9, 31, stack_offset + i * 8);
                }
            }
            ParameterStorage::Unallocated => unreachable!(),
            _ => todo!("store parameter storage {:?}", arg.storage),
        }
    }

    fn load_arg(&mut self, stack_offset: u32, to: &Arg, byref: bool) {
        if byref {
            match to.storage {
                ParameterStorage::GPReg(reg) => {
                    self.code.add_imm(reg, 31, stack_offset);
                }
                ParameterStorage::Stack(to_offset) => {
                    self.code.add_imm(9, 31, stack_offset);
                    self.code.store_base_offset(9, 31, to_offset);
                }
                _ => unreachable!(),
            }
            return;
        }

        match to.storage {
            ParameterStorage::GPReg(reg) => {
                if to.ptr {
                    todo!("load ptr");
                } else {
                    self.code.load_base_offset(reg, 31, stack_offset);
                }
            }
            ParameterStorage::GPRRange(start, count) => {
                for i in 0..count {
                    let offset = stack_offset + i * 8;
                    self.code.load_base_offset(start + i, 31, offset);
                }
            }
            ParameterStorage::VectorReg(reg) => {
                self.code.load_base_offset_fp(reg, 31, stack_offset, 8);
            }
            ParameterStorage::VectorRange(start, count, is_double) => {
                let size = is_double.then_some(8).unwrap_or(4);
                for i in 0..count {
                    let offset = stack_offset + i * size;
                    self.code.load_base_offset_fp(start + i, 31, offset, size);
                }
            }
            ParameterStorage::Stack(to_offset) => {
                let count = to.size / 8;
                for i in 0..count as u32 {
                    self.code.load_base_offset(9, 31, stack_offset + i * 8);
                    self.code.store_base_offset(9, 31, to_offset + i * 8);
                }
            }
            ParameterStorage::Unallocated => unreachable!(),
            _ => todo!("load parameter storage {:?}", to.storage),
        }
    }

    fn load_orig_param(&mut self, num: usize, to: &Arg, byref: bool) {
        let offset = self.orig_param_offsets[num];
        self.load_arg(offset, to, byref);
    }

    pub fn call_orig(&mut self) {
        let branch_idx = self.code.code.len();
        self.code.branch_zero(11, 0);

        if let Some(instance_offset) = self.instance_param_offset {
            self.code.load_base_offset(0, 31, instance_offset);
        }
        for i in 0..self.original.params.len() {
            self.load_orig_param(i, &self.original.layout.args[i], false)
        }
        self.code.call_addr(None);
        if let Some(ret_layout) = &self.original.ret_layout {
            let offset = self.result_offset.unwrap();
            self.store_arg(ret_layout, offset);
        }

        let branch_offset = (self.code.code.len() - branch_idx) as u32;
        self.code.code[branch_idx] |= (branch_offset << 5);
    }

    fn inject_field(&mut self, field: &FieldInfo, arg: &Arg, byref: bool) {
        if byref {
            match arg.storage {
                ParameterStorage::GPReg(num) => {
                    // instance param
                    self.code
                        .load_base_offset(num, 31, self.instance_param_offset.unwrap());
                    self.code.add_imm(num, num, field.offset as u32)
                }
                _ => todo!("byref field {:?}", arg.storage),
            }
            return;
        }

        let mut field_offset = field.offset;
        if self.valuetype_instance {
            // we don't have to worry about boxing
            field_offset -= 0x10;
        }

        match arg.storage {
            ParameterStorage::GPReg(num) => {
                // instance param
                self.code
                    .load_base_offset(num, 31, self.instance_param_offset.unwrap());
                if arg.ptr {
                    self.code.add_imm(num, num, field_offset as u32)
                } else {
                    self.code
                        .load_sized(num, num, field_offset as u32, arg.size);
                }
            }
            _ => todo!(),
        }
    }

    fn inject_instance(&mut self, arg: &Arg) {
        let offset = self.instance_param_offset.unwrap();
        match arg.storage {
            ParameterStorage::GPReg(reg) => {
                self.code.load_base_offset(reg, 31, offset);
            }
            ParameterStorage::Stack(to_offset) => {
                self.code.load_base_offset(9, 31, offset);
                self.code.store_base_offset(9, 31, to_offset);
            }
            _ => unreachable!(),
        }
    }

    fn inject_run_original(&mut self, arg: &Arg) {
        let offset = self.run_original_offset;
        match arg.storage {
            ParameterStorage::GPReg(reg) => {
                self.code.load_base_offset(reg, 31, offset);
            }
            ParameterStorage::Stack(to_offset) => {
                self.code.load_base_offset(9, 31, offset);
                self.code.store_base_offset(9, 31, to_offset);
            }
            _ => unreachable!(),
        }
    }

    pub(super) fn gen_call_hook(&mut self, method: CodegenMethod, injections: Vec<ParamInjection>) {
        for (injection, arg) in injections.iter().zip(method.layout.args.iter()) {
            match injection {
                ParamInjection::LoadField(idx, byref) => {
                    let fields = unsafe { get_fields(self.original.method.klass) };
                    let field = &fields[*idx];
                    self.inject_field(field, arg, *byref);
                }
                ParamInjection::OriginalParam(idx, byref) => {
                    self.load_orig_param(*idx, arg, *byref);
                }
                ParamInjection::Instance => {
                    self.inject_instance(arg);
                }
                ParamInjection::Result(byref) => {
                    let offset = self.result_offset.unwrap();
                    self.load_arg(offset, arg, *byref);
                }
                ParamInjection::RunOriginal => {
                    self.inject_run_original(arg);
                }
            }
        }
        self.code
            .call_addr(Some(method.method.methodPointer.unwrap() as usize));

        if method.ret_layout.is_some() {
            // Assume it's the boolean runOriginal
            self.code.mov_reg(11, 0);
        } else {
            self.code.mov_imm(11, 1);
        }
    }

    fn write_prologue_epilogue(&mut self) {
        let mut prologue = Code::default();
        // save space for stack frame
        let frame_offset = self.stack_offset;
        self.stack_offset += 16;

        self.stack_offset = (self.stack_offset as u32 + 15) & !15;
        prologue.sub_imm(31, 31, self.stack_offset);
        prologue.store_pair(29, 30, 31, frame_offset);
        prologue.add_imm(29, 31, frame_offset);
        self.code.push_front(prologue);

        self.code.load_pair(29, 30, 31, frame_offset);
        self.code.add_imm(31, 31, self.stack_offset);
        self.code.ret();
    }

    pub fn finish_and_install(mut self) {
        self.write_prologue_epilogue();

        let dest = HOOK_ALLOCATOR.lock().unwrap().alloc(self.code.size());

        let hook = Hook::new();
        let orig_ptr = get_method_pointer(
            unsafe { &*self.original.method.klass }.image,
            self.original.method.token,
        )
        .unwrap();
        unsafe {
            hook.install(orig_ptr as _, dest as _);
        }
        self.code
            .copy_to(dest, hook.original().unwrap() as usize, self.stack_offset);
    }
}
