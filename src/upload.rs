use crate::config::{Mod, CONFIG};
use anyhow::{anyhow, Result};
use std::path::PathBuf;
use std::process::Command;

fn ensure_dir_exists(dir: &str) -> Result<()> {
    let status = Command::new(&CONFIG.adb_path)
        .args(["shell", "mkdir", "-p"])
        .arg(dir)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("adb mkdir failed"))
    }
}

fn push(path: PathBuf, to: &str) -> Result<()> {
    let status = Command::new(&CONFIG.adb_path)
        .arg("push")
        .arg(path)
        .arg(to)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("adb upload failed"))
    }
}

pub fn upload() -> Result<()> {
    let mod_config = Mod::read_config()?;

    let mod_dir = format!(
        "/sdcard/ModData/{}/Mods/QMerge/Mods/{}/",
        mod_config.app, mod_config.id
    );
    ensure_dir_exists(&mod_dir)?;
    let out_path = PathBuf::from("./build/out");

    let mmd_name = format!("{}.mmd", mod_config.id);
    let so_name = format!("{}.so", mod_config.id);
    push(out_path.join(&mmd_name), &(mod_dir.clone() + &mmd_name))?;
    push(out_path.join(&so_name), &(mod_dir + &so_name))?;

    Ok(())
}
