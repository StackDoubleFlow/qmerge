use std::path::PathBuf;

pub fn platform_executable(path: &mut PathBuf) {
    #[cfg(target_os = "windows")]
    path.set_extension("exe");
    #[cfg(not(target_os = "windows"))]
    path.set_extension("");
}
