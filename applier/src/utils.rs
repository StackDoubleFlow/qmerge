use anyhow::Result;
use std::path::PathBuf;
use std::str;

pub fn get_mod_data_path() -> PathBuf {
    // TODO
    PathBuf::from("/sdcard/ModData/com.beatgames.beatsaber/Mods/QMerge")
}

pub fn get_exec_path() -> PathBuf {
    // TODO
    PathBuf::from("/data/data/com.beatgames.beatsaber")
}

pub fn offset_len(offset: i32, len: i32) -> std::ops::Range<usize> {
    if (offset as i32) < 0 {
        return 0..0;
    }
    offset as usize..offset as usize + len as usize
}

fn strlen(data: &[u8], offset: usize) -> usize {
    let mut len = 0;
    while data[offset + len] != 0 {
        len += 1;
    }
    len
}

pub fn get_str(data: &[u8], offset: usize) -> Result<&str> {
    let len = strlen(data, offset);
    let str = str::from_utf8(&data[offset..offset + len])?;
    Ok(str)
}