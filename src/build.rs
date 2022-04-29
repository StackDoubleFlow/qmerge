use std::process::Command;
use std::path::PathBuf;
use anyhow::{Result, Context};
use crate::config::{CONFIG, APPS, Mod};

pub fn build() -> Result<()> {
    let mod_config = Mod::read_config()?;
    let app = APPS.get(&mod_config.app).with_context(|| format!("Application '{}' not configured", mod_config.app))?;
    let unity_install = CONFIG.unity_installs.get(&app.unity_version).with_context(|| format!("Unity version '{}' not configured", app.unity_version))?;

    let unity_path = PathBuf::from(unity_install);
    let mono_path = unity_path.join("Editor/Data/MonoBleedingEdge/bin/mono");
    let il2cpp_path = unity_path.join("Editor/Data/il2cpp/build/deploy/net471/il2cpp.exe");

    Command::new(mono_path)
        // Fix for System.ConsoleDriver type initializer
        .env("TERM", "xterm")
        .arg(il2cpp_path)
        .arg("--convert-to-cpp")
        .arg("--directory=./build/Managed")
        .arg("--generatedcppdir=./build/cpp")
        .status()
        .context("il2cpp command failed")?;

    Ok(())
}
