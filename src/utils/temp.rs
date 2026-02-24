//! 临时目录管理模块

use anyhow::Result;
use std::path::Path;
use std::sync::{Arc, OnceLock};

pub static TEMP_DIR: OnceLock<Arc<Path>> = OnceLock::new();

/// 获取临时目录路径
pub fn temp_dir() -> Result<&'static Path> {
    let temp_dir = TEMP_DIR.get_or_init(|| {
        log::debug!("初始化临时文件夹...");
        let mut temp_dir = std::env::current_dir().expect("无法获取当前文件夹");
        temp_dir.push(".mc_patcher.tmp");
        temp_dir.into()
    });

    if !temp_dir.is_dir()
        && let Err(e) = std::fs::create_dir_all(temp_dir.as_ref())
    {
        anyhow::bail!("未能创建临时文件夹 {}：{}", temp_dir.display(), e);
    }
    Ok(TEMP_DIR.get().unwrap())
}
