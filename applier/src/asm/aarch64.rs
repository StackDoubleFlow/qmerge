use crate::codegen_api;
use std::arch::asm;

#[no_mangle]
#[naked]
pub unsafe extern "C" fn merge_prestub() -> ! {
  asm!(
    "sub sp, sp, #80",
    "stp x0, x1, [sp, #(16*0)]",
    "stp x2, x3, [sp, #(16*1)]",
    "stp x4, x5, [sp, #(16*2)]",
    "stp x6, x7, [sp, #(16*3)]",
    "stp x8, lr, [sp, #(16*4)]",
    "mov x0, lr",
    "bl {}",
    "mov x10, x0",
    "ldp x0, x1, [sp, #(16*0)]",
    "ldp x2, x3, [sp, #(16*1)]",
    "ldp x4, x5, [sp, #(16*2)]",
    "ldp x6, x7, [sp, #(16*3)]",
    "ldp x8, lr, [sp, #(16*4)]",
    "add sp, sp, #80",
    "br x10",
    sym codegen_api::resolve_method_by_call_helper_addr,
    options(noreturn)
  );
}