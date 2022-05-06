use crate::config::CONFIG;
use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;
use std::process::Command;

#[cfg(target_os = "linux")]
const NDK_HOST_TAG: &str = "linux-x86_64";
#[cfg(target_os = "macos")]
const NDK_HOST_TAG: &str = "darwin-x86_64";
#[cfg(all(target_os = "windows", target_arch = "x86"))]
const NDK_HOST_TAG: &str = "windows";
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
const NDK_HOST_TAG: &str = "windows-x86_64";

pub struct CompileCommand {
    source_files: Vec<PathBuf>,
    include_paths: Vec<PathBuf>,
    output_path: PathBuf,
}

impl CompileCommand {
    pub fn new(output_path: PathBuf) -> Self {
        Self {
            source_files: Vec::new(),
            include_paths: Vec::new(),
            output_path,
        }
    }

    pub fn add_source(&mut self, path: PathBuf) {
        self.source_files.push(path);
    }

    pub fn add_include_path(&mut self, path: PathBuf) {
        self.include_paths.push(path);
    }

    pub fn run(&self) -> Result<()> {
        let mut clang_path = PathBuf::from(&CONFIG.ndk_path).join("toolchains/llvm/prebuilt");
        clang_path.push(NDK_HOST_TAG);
        clang_path.push("bin/clang-12");
        #[cfg(target_os = "windows")]
        clang_path.set_extension(".exe");

        let target = "aarch64-linux-android21";

        let mut command = Command::new(clang_path);
        for path in &self.include_paths {
            command.arg("-I");
            command.arg(path);
        }
        command
            .args(&["-target", target])
            .args(&["-shared"])
            .arg("-o")
            .arg(&self.output_path)
            .args(&self.source_files);
        dbg!(&command);
        let status = command
            .status()
            .context("failed to execute compile command")?;
        if status.success() {
            Ok(())
        } else {
            Err(anyhow!("compile command failed: {:?}", command))
        }
    }
}
