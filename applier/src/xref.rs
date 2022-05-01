use std::lazy::SyncLazy;
use serde::Deserialize;
use std::fs;

use crate::get_mod_data_path;

static XREF_DATA: SyncLazy<XRefData> = SyncLazy::new(|| {
    let path = get_mod_data_path().join("xref_gen.json");
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
});

#[derive(Deserialize)]
struct SymbolTrace {
    symbol: String,
    start: String,
    trace: String,
}

#[derive(Deserialize)]
pub struct XRefData {
    traces: Vec<SymbolTrace>,
}

pub fn get_symbol(name: String) -> u64 {
    let symbol_trace = XREF_DATA.traces.iter().find(|st| st.symbol == name).unwrap();

    todo!();
}
