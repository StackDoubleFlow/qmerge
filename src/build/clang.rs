use crate::config::CONFIG;
use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(target_os = "linux")]
const NDK_HOST_TAG: &str = "linux-x86_64";
#[cfg(target_os = "macos")]
const NDK_HOST_TAG: &str = "darwin-x86_64";
#[cfg(all(target_os = "windows", target_arch = "x86"))]
const NDK_HOST_TAG: &str = "windows";
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
const NDK_HOST_TAG: &str = "windows-x86_64";

pub struct CompileCommand<'a> {
    source_files: Vec<PathBuf>,
    include_paths: Vec<PathBuf>,
    output_path: PathBuf,
    obj_path: &'a Path,
    target: &'a str,
}

impl<'a> CompileCommand<'a> {
    pub fn new(output_path: PathBuf, obj_path: &'a Path, target: &'a str) -> Self {
        Self {
            source_files: Vec::new(),
            include_paths: Vec::new(),
            output_path,
            obj_path,
            target,
        }
    }

    pub fn add_source(&mut self, path: PathBuf) {
        self.source_files.push(path);
    }

    pub fn add_include_path(&mut self, path: PathBuf) {
        self.include_paths.push(path);
    }

    fn base_command(&self, cpp: bool) -> Command {
        let mut clang_path = PathBuf::from(&CONFIG.ndk_path).join("toolchains/llvm/prebuilt");
        clang_path.push(NDK_HOST_TAG);
        if cpp {
            clang_path.push("bin/clang++");
        } else {
            clang_path.push("bin/clang");
        }
        #[cfg(target_os = "windows")]
        clang_path.set_extension(".exe");

        let mut command = Command::new(clang_path);
        command.args(&["-target", self.target]);
        command
    }

    fn compile_source(&self, path: &Path) -> Result<PathBuf> {
        let cpp = path.extension().unwrap() == ".cpp";
        let mut command = self.base_command(cpp);
        command
            .args(&[
                "-fpic",
                "-Wno-missing-declarations",
                "-Wno-invalid-offsetof",
                "-c",
            ])
            .arg(path);

        for path in &self.include_paths {
            command.arg("-I");
            command.arg(path);
        }

        let name = path.file_stem().unwrap();
        let output_path = self.obj_path.join(name).with_extension("o");
        command.arg("-o");
        command.arg(&output_path);
        // dbg!(&command);

        let status = command
            .status()
            .context("failed to execute compile command")?;
        if status.success() {
            Ok(output_path)
        } else {
            Err(anyhow!("compile command failed: {:?}", command))
        }
    }

    pub fn run(&self) -> Result<()> {
        let mut object_files = Vec::new();
        for source_file in &self.source_files {
            object_files.push(self.compile_source(source_file)?);
        }

        let mut command = self.base_command(true);
        command
            .args(&["-shared", "-static-libstdc++"])
            .arg("-o")
            .arg(&self.output_path)
            // TODO: Get path to applier binary
            .arg("../target/aarch64-linux-android/release/libmerge_applier.so")
            .args(&object_files);
        // dbg!(&command);
        let status = command.status().context("failed to execute link command")?;
        if status.success() {
            Ok(())
        } else {
            Err(anyhow!("link command failed: {:?}", command))
        }
    }
}
