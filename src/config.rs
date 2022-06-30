use crate::error::exit_on_err;
use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

pub static CONFIG: LazyLock<Config> = LazyLock::new(|| exit_on_err(load_config()));
pub static APPS: LazyLock<HashMap<String, AppConfig>> = LazyLock::new(|| exit_on_err(load_apps()));

fn load_toml<P, T>(path: P) -> Result<T>
where
    P: AsRef<Path>,
    T: DeserializeOwned,
{
    let str = fs::read_to_string(&path)
        .with_context(|| format!("error reading config at {}", path.as_ref().display()))?;
    let file = toml::from_str(&str)
        .with_context(|| format!("error parsing config at {}", path.as_ref().display()))?;
    Ok(file)
}

fn load_config() -> Result<Config> {
    let mut path = config_dir()?;
    path.push("Config.toml");
    load_toml(path)
}

fn load_apps() -> Result<HashMap<String, AppConfig>> {
    let mut path = config_dir()?;
    path.push("apps");

    let mut apps = HashMap::new();
    for entry in fs::read_dir(path).context("could not read apps directory")? {
        let entry = entry?;
        let id = entry.file_name().to_string_lossy().to_string();
        let mut path = entry.path();
        path.push("App.toml");
        apps.insert(id, load_toml(path)?);
    }
    Ok(apps)
}

fn config_dir() -> Result<PathBuf> {
    let mut dir = dirs::config_dir().context("unable to get user config directory")?;
    dir.push("qmerge");
    Ok(dir)
}

#[derive(Deserialize, Debug)]
pub struct AppConfig {
    pub unity_version: String,
    pub shims: HashSet<String>,
}

#[derive(Deserialize, Debug)]
pub struct Mod {
    pub app: String,
    pub name: String,
    pub version: String,
    pub id: String,
}

impl Mod {
    pub fn read_config() -> Result<Self> {
        load_toml("./Merge.toml")
    }
}

#[derive(Deserialize, Debug)]
pub struct Config {
    /// A mapping from untiy versions to their install paths
    pub unity_installs: HashMap<String, String>,
    pub ndk_path: String,
    pub adb_path: String,
}
