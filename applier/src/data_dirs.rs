use std::{sync::LazyLock, path::PathBuf, fs};


pub static APPLICATION_ID: LazyLock<String> = LazyLock::new(|| {
    fs::read_to_string("/proc/self/cmdline").unwrap().trim_end_matches('\u{0}').to_string()
});

pub static MOD_DATA_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
    PathBuf::from(format!("/sdcard/ModData/{}/Mods/QMerge", *APPLICATION_ID))
});

pub static EXEC_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
    PathBuf::from(format!("/data/data/{}", *APPLICATION_ID))
});
