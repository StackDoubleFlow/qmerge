use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;

#[derive(Deserialize, Debug)]
pub struct Manifest {
    pub plugin: Mod,
    pub dependencies: HashMap<String, String>,
}

impl Manifest {
    pub fn load() -> Result<Manifest> {
        let str = fs::read_to_string("./QMerge.toml")
            .context("failed to read plugin manifest (`QMerge.toml`)")?;
        let manifest = toml::from_str(&str).context("failed to deserialize plugin manifest")?;
        Ok(manifest)
    }
}

#[derive(Deserialize, Debug)]
pub struct Mod {
    pub app: String,
    pub app_version: String,
    pub id: String,
    pub name: String,
    pub author: String,
    pub version: String,
    pub description: String,
    #[serde(default)]
    pub load_before: Vec<String>,
    #[serde(default)]
    pub load_after: Vec<String>,
    /// These files are copied to /sdcard/Android/data/com.beatgames.beatsaber/files/mods/
    #[serde(default)]
    pub native_mods: Vec<String>,
    /// These files are copied to /sdcard/Android/data/com.beatgames.beatsaber/files/libs/
    #[serde(default)]
    pub native_libs: Vec<String>,
    /// These files are copied to /sdcard/ModData/com.beatgames.beatsaber/Mods/QMerge/
    #[serde(default)]
    pub core_files: Vec<String>,
}
