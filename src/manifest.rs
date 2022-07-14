use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;

#[derive(Deserialize, Debug)]
pub struct Manifest {
    pub plugin: Mod,
}

impl Manifest {
    pub fn load() -> Result<Manifest> {
        let str = fs::read_to_string("./QMerge.toml").context("failed to read plugin manifest (`QMerge.toml`)")?;
        let manifest = toml::from_str(&str).context("failed to deserialize plugin manifest")?;
        Ok(manifest)
    }
}

#[derive(Deserialize, Debug)]
pub struct Mod {
    pub app: String,
    pub name: String,
    pub version: String,
    pub id: String,
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub load_before: Vec<String>,
    #[serde(default)]
    pub load_after: Vec<String>,
}
