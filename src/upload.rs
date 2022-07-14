use crate::config::Config;
use crate::manifest::Manifest;
use anyhow::{anyhow, Result};
use std::path::PathBuf;
use std::process::Command;

struct Adb {
    path: String,
}

impl Adb {
    fn ensure_dir_exists(&self, dir: &str) -> Result<()> {
        let status = Command::new(&self.path)
            .args(["shell", "mkdir", "-p"])
            .arg(dir)
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(anyhow!("adb mkdir failed"))
        }
    }

    fn push(&self, path: PathBuf, to: &str) -> Result<()> {
        let status = Command::new(&self.path)
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
}

pub fn upload(config: &mut Config) -> Result<()> {
    let manifest = Manifest::load()?;
    let mod_id = &manifest.plugin.id;
    let app = &manifest.plugin.app;

    let adb = Adb {
        path: config.get_adb_path()?,
    };

    let mod_dir = format!("/sdcard/ModData/{}/Mods/QMerge/Mods/{}/", app, mod_id);
    adb.ensure_dir_exists(&mod_dir)?;
    let out_path = PathBuf::from("./build/bin/out");

    let mmd_name = format!("{}.mmd", mod_id);
    let so_name = format!("{}.so", mod_id);
    adb.push(out_path.join(&mmd_name), &(mod_dir.clone() + &mmd_name))?;
    adb.push(out_path.join(&so_name), &(mod_dir + &so_name))?;

    Ok(())
}
