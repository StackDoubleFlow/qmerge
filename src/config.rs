use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::{fs, io};
use toml_edit::{Document, Item, Table};

fn ask_and_verify<F>(ask: &str, verify: F) -> Result<String>
where
    F: Fn(&str) -> bool,
{
    loop {
        print!("{}: ", ask);
        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .context("unable to read user input")?;
        if verify(&input) {
            return Ok(input);
        }
    }
}

fn verify_unity_install(path: &str) -> bool {
    let path = Path::new(path);
    if !path.exists() {
        println!("path does not exist.");
        return false;
    }

    // TODO: Verify valid unity installation and that il2cpp module is installed

    true
}

fn verify_unity_ver(ver: &str) -> bool {
    // TODO: more unity versions
    matches!(ver, "2019.4.28f1")
}

fn verify_ndk_install(path: &str) -> bool {
    let path = Path::new(path);
    if !path.exists() {
        println!("path does not exist.");
        return false;
    }

    // TODO: Verity valid NDK installation

    true
}

fn verify_adb_executable(path: &str) -> bool {
    let path = Path::new(path);
    if !path.exists() {
        println!("path does not exist.");
        return false;
    }

    // TODO: Verity valid NDK installation

    true
}

#[derive(Deserialize, Debug)]
struct ConfigTOML {
    /// A mapping from untiy versions to their install paths
    unity_installs: Option<HashMap<String, String>>,
    ndk_path: Option<String>,
    adb_path: Option<String>,
}

pub struct Config {
    toml: ConfigTOML,
    doc: Document,
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_dir = config_dir()?;

        let config_path = config_dir.join("Config.toml");
        let config_str = if config_path.exists() {
            fs::read_to_string(&config_path)
                .with_context(|| format!("error reading file at {}", config_path.display()))?
        } else {
            println!("Welcome to QMerge!");
            println!("Since this is your first time starting this tool, you may be asked questions to complete your configuration.");
            String::new()
        };

        Ok(Self {
            toml: toml::from_str(&config_str)
                .with_context(|| format!("error parsing TOML at {}", config_path.display()))?,
            doc: config_str.parse()?,
        })
    }

    fn save(&self) -> Result<()> {
        let config_dir = config_dir()?;
        fs::create_dir_all(&config_dir)?;
        let str = self.doc.to_string();
        fs::write(config_dir.join("Config.toml"), str)?;
        Ok(())
    }

    pub fn get_unity_install(&mut self, version: &str) -> Result<String> {
        let mut path = None;
        if let Some(installs) = &self.toml.unity_installs {
            if let Some(install) = installs.get(version) {
                path = Some(install.clone());
            }
        }

        let path = match path {
            Some(path) => path,
            None => {
                println!("Unity installation has not yet been configured.");
                let ask = format!(
                    "Please enter the path to your unity installation (version {})",
                    version
                );
                let path = ask_and_verify(&ask, verify_unity_install)?;
                let installs = self.doc["unity_installs"].or_insert(Item::Table(Table::new()));
                installs[version] = toml_edit::value(&path);
                self.save()?;
                path
            }
        };
        Ok(path)
    }

    pub fn get_app(&self, id: &str) -> Result<AppConfig> {
        let config_dir = config_dir()?;

        let app_dir = config_dir.join(format!("apps/{}", id));
        let config_path = app_dir.join("App.toml");
        let app = if !config_path.exists() {
            println!("Application {} has not yet been configured.", id);
            let unity_ver = ask_and_verify(
                "Please enter this application's unity version",
                verify_unity_ver,
            )?;

            let app = AppConfig {
                unity_version: unity_ver,
                shims: HashSet::new(),
            };
            fs::create_dir_all(app_dir)?;
            let str = toml::to_string(&app)?;
            fs::write(config_path, str).context("failed to write app config")?;

            app
        } else {
            let str = fs::read_to_string(config_path).context("failed to read app config")?;
            toml::from_str(&str).context("failed to deserialize app config")?
        };
        Ok(app)
    }

    pub fn get_ndk_path(&mut self) -> Result<String> {
        let path = match &self.toml.ndk_path {
            Some(path) => path.clone(),
            None => {
                println!("NDK installation has not yet been configured.");
                println!("It is recommended to use NDK r22b for QMerge development.");
                #[cfg(target_os = "linux")]
                println!("You may download it from here: https://dl.google.com/android/repository/android-ndk-r22b-linux-x86_64.zip");
                #[cfg(target_os = "macos")]
                println!("You may download it from here: https://dl.google.com/android/repository/android-ndk-r22b-darwin-x86_64.zip");
                #[cfg(target_os = "windows")]
                println!("You may download it from here: https://dl.google.com/android/repository/android-ndk-r22b-windows-x86_64.zip");

                let path = ask_and_verify(
                    "Please enter your NDK installation directory",
                    verify_ndk_install,
                )?;
                self.save()?;
                path
            }
        };

        Ok(path)
    }

    pub fn get_adb_path(&mut self) -> Result<String> {
        let path = match &self.toml.adb_path {
            Some(path) => path.clone(),
            None => {
                println!("ADB executable has not yet been configured.");
                println!("You may find this in a SideQuest installation or in the official Android SDK Platform-Tools");
                #[cfg(target_os = "linux")]
                println!("You may download SDK Platform-Tools here: https://dl.google.com/android/repository/platform-tools-latest-linux.zip");
                #[cfg(target_os = "macos")]
                println!("You may download SDK Platform-Tools here: https://dl.google.com/android/repository/platform-tools-latest-darwin.zip");
                #[cfg(target_os = "windows")]
                println!("You may download SDK Platform-Tools here: https://dl.google.com/android/repository/platform-tools-latest-windows.zip");

                let path = ask_and_verify(
                    "Please enter your ADB executable path",
                    verify_adb_executable,
                )?;
                self.save()?;
                path
            }
        };

        Ok(path)
    }
}

fn config_dir() -> Result<PathBuf> {
    let mut dir = dirs::config_dir().context("unable to get user config directory")?;
    dir.push("qmerge");
    Ok(dir)
}

#[derive(Deserialize, Serialize, Debug)]
pub struct AppConfig {
    pub unity_version: String,
    pub shims: HashSet<String>,
}
