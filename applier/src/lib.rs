#![feature(once_cell, backtrace)]
#![feature(naked_functions)]
#![feature(asm_sym)]

mod asm;
mod codegen_api;
mod data_dirs;
mod hook;
mod loader;
mod natives;
mod setup;
mod utils;
mod xref;

use anyhow::Result;
use loader::MOD_INIT_FNS;
use tracing::info;

#[no_mangle]
pub extern "C" fn setup() {
    setup::setup(env!("CARGO_PKG_NAME"));
    info!("merge applier is setting up");
    loader::install_hooks();
}

fn call_plugin_loads() -> Result<()> {
    info!("Calling mod initialization methods");
    for init_fn in MOD_INIT_FNS.get().unwrap() {
        unsafe { init_fn() }
    }
    info!("Initializaton complete");
    Ok(())
}

#[no_mangle]
pub extern "C" fn load() {
    call_plugin_loads().unwrap();
}
