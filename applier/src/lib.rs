#![feature(once_cell, backtrace)]
#![feature(naked_functions)]
#![feature(asm_sym)]

mod asm;
mod codegen_api;
pub mod il2cpp_types;
mod loader;
mod setup;
mod utils;
mod xref;

use anyhow::Result;
use loader::MODS;
use tracing::{info, warn};

#[no_mangle]
pub extern "C" fn setup() {
    setup::setup(env!("CARGO_PKG_NAME"));
    info!("merge applier is setting up");
    loader::install_hooks();
}

fn call_plugin_loads() -> Result<()> {
    let ids: Vec<String> = MODS.lock().unwrap().keys().cloned().collect();
    for id in ids {
        info!("Initializing mod {}", id);
        let load_fn = MODS.lock().unwrap()[&id].load_fn;
        match load_fn {
            Some(load_fn) => unsafe { load_fn() },
            None => warn!("Mod {} is missing an init function!", id),
        }
    }
    Ok(())
}

#[no_mangle]
pub extern "C" fn load() {
    call_plugin_loads().unwrap();
}
