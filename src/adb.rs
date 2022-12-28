use crate::config::Config;
use crate::manifest::Manifest;
use color_eyre::eyre::{anyhow, Result};
use std::fs::File;
use std::path::PathBuf;
use std::process::{Command, ExitStatus};

struct Adb {
    path: String,
}

impl Adb {
    fn check_status(&self, status: ExitStatus) -> Result<()> {
        if status.success() {
            Ok(())
        } else {
            Err(anyhow!("adb command failed"))
        }
    }

    fn ensure_dir_exists(&self, dir: &str) -> Result<()> {
        let status = Command::new(&self.path)
            .args(["shell", "mkdir", "-p"])
            .arg(dir)
            .status()?;
        self.check_status(status)
    }

    fn push(&self, path: PathBuf, to: &str) -> Result<()> {
        let status = Command::new(&self.path)
            .arg("push")
            .arg(path)
            .arg(to)
            .status()?;
        self.check_status(status)
    }

    fn restart_app(&self, id: &str) -> Result<()> {
        let status = Command::new(&self.path)
            .args(["shell", "am", "start", "-S"])
            .arg(format!("{}/com.unity3d.player.UnityPlayerActivity", id))
            .status()?;
        self.check_status(status)
    }

    fn log_to_file(&self, file: File) -> Result<()> {
        // Clear old log buffer just in case
        let status = Command::new(&self.path).args(["logcat", "-c"]).status()?;
        self.check_status(status)?;
        let status = Command::new(&self.path)
            .stdout(file)
            .arg("logcat")
            .status()?;
        self.check_status(status)
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

pub fn start_and_log(config: &mut Config) -> Result<()> {
    let manifest = Manifest::load()?;
    let app = &manifest.plugin.app;

    let adb = Adb {
        path: config.get_adb_path()?,
    };

    adb.restart_app(app)?;

    let log_file = File::create("./test.log")?;
    adb.log_to_file(log_file)?;

    Ok(())
}
