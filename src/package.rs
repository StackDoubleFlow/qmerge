use crate::config::Config;
use crate::manifest::Manifest;
use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;
use zip::ZipWriter;

#[derive(Serialize)]
struct Dependency<'a> {
    id: &'a str,
    version: &'a str,
}

#[derive(Serialize)]
struct FileCopy<'a> {
    name: &'a str,
    destination: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct QModJson<'a> {
    #[serde(rename = "_QPVersion")]
    qp_ver: &'a str,
    name: &'a str,
    id: &'a str,
    author: &'a str,
    version: &'a str,
    package_id: &'a str,
    package_version: &'a str,
    description: &'a str,
    dependences: Vec<Dependency<'a>>,
    mod_files: Vec<&'a str>,
    library_files: Vec<&'a str>,
    file_copies: Vec<FileCopy<'a>>,
}

pub fn build_package(_config: &mut Config) -> Result<()> {
    let manifest = Manifest::load()?;
    let plugin = &manifest.plugin;

    let qmod_name = format!("{}.qmod", plugin.id);
    let qmod_file = OpenOptions::new()
        .create(true)
        .write(true)
        .open(qmod_name)?;
    let mut qmod = ZipWriter::new(qmod_file);

    let mut dependences = Vec::new();
    for (id, version) in &manifest.dependencies {
        dependences.push(Dependency { id, version })
    }

    let mut file_copies = Vec::new();
    for core_file in &plugin.core_files {
        let path = Path::new(core_file);
        let name = path
            .file_name()
            .context("could not get core file name")?
            .to_str()
            .unwrap();
        let destination = format!(
            "/sdcard/ModData/com.beatgames.beatsaber/Mods/QMerge/{}",
            name
        );
        file_copies.push(FileCopy { destination, name });
        qmod.start_file(name, Default::default())?;
        let file = fs::read(path)
            .with_context(|| format!("could not read core file at {}", path.display()))?;
        qmod.write_all(&file)?;
    }

    fn add_natives<'a>(qmod: &mut ZipWriter<File>, files: &'a [String]) -> Result<Vec<&'a str>> {
        let mut names = Vec::new();
        for native_file in files {
            let path = Path::new(native_file);
            let name = path
                .file_name()
                .context("could not get native mod/lib file name")?
                .to_str()
                .unwrap();
            names.push(name);
            qmod.start_file(name, Default::default())?;
            let file = fs::read(path)
                .with_context(|| format!("could not read native mod/lib at {}", path.display()))?;
            qmod.write_all(&file)?;
        }
        Ok(names)
    }
    let mod_files = add_natives(&mut qmod, &plugin.native_mods)?;
    let library_files = add_natives(&mut qmod, &plugin.native_libs)?;

    let out_dir = Path::new("./build/bin/out");
    let binaries = [format!("{}.mmd", plugin.id), format!("{}.so", plugin.id)];
    for binary in &binaries {
        let path = out_dir.join(binary);
        if !path.exists() {
            bail!("Output binaries could not be found! Make sure to build your mod first.");
        }
        qmod.start_file(binary, Default::default())?;
        let file = fs::read(&path)
            .with_context(|| format!("could not read binary at {}", path.display()))?;
        qmod.write_all(&file)?;

        let destination = format!(
            "/sdcard/ModData/com.beatgames.beatsaber/Mods/QMerge/Mods/{}/{}",
            plugin.id, binary
        );
        file_copies.push(FileCopy {
            destination,
            name: binary,
        });
    }

    let json = QModJson {
        qp_ver: "0.1.1",
        name: &plugin.name,
        id: &plugin.id,
        author: &plugin.author,
        version: &plugin.version,
        package_id: &plugin.app,
        package_version: &plugin.app_version,
        description: &plugin.description,
        dependences,
        mod_files,
        library_files,
        file_copies,
    };
    let json_str = serde_json::to_string_pretty(&json)?;
    qmod.start_file("mod.json", Default::default())?;
    qmod.write_all(json_str.as_bytes())?;

    qmod.finish()?;

    Ok(())
}
