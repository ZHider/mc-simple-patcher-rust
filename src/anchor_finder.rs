//! 锚点搜索模块
//! 实现锚点文件/文件夹的搜索和工作目录定位功能

use std::path::{Path, PathBuf};
use anyhow::Result;

/// 搜索锚点并返回工作目录
pub fn find_anchor(anchor_name: &str, start_dir: &Path, max_depth: usize) -> Result<Option<PathBuf>> {
    log::info!("开始搜索锚点: {}", anchor_name);

    // 检查当前目录是否包含名为 anchor_name 的文件或文件夹
    let current_path = start_dir.join(anchor_name);
    if current_path.exists() {
        let result = if current_path.is_file() {
            // 如果是文件，返回其所在目录
            log::info!("在当前目录找到锚点文件: {:?}", current_path);
            current_path.parent().map(|p| p.to_path_buf())
        } else if current_path.is_dir() {
            // 如果是目录，返回该目录
            log::info!("在当前目录找到锚点目录: {:?}", current_path);
            Some(current_path)
        } else {
            None
        };

        return Ok(result);
    }

    // 递归搜索父目录
    let result = search_parent_dirs(anchor_name, start_dir, max_depth)?;

    if result.is_none() {
        log::warn!("未找到锚点: {}", anchor_name);
    }

    Ok(result)
}

/// 在父目录中搜索锚点
fn search_parent_dirs(anchor_name: &str, start_dir: &Path, max_depth: usize) -> Result<Option<PathBuf>> {
    let mut current_dir = start_dir;
    let mut depth = 0;

    while let Some(parent) = current_dir.parent() {
        if depth >= max_depth {
            log::warn!("达到最大递归深度，停止搜索");
            break;
        }

        let anchor_path = parent.join(anchor_name);
        if anchor_path.exists() {
            let result = if anchor_path.is_file() {
                log::info!("在父目录找到锚点文件: {:?}", anchor_path);
                anchor_path.parent().map(|p| p.to_path_buf())
            } else if anchor_path.is_dir() {
                log::info!("在父目录找到锚点目录: {:?}", anchor_path);
                Some(anchor_path)
            } else {
                None
            };

            return Ok(result);
        }

        current_dir = parent;
        depth += 1;
    }

    Ok(None)
}

/// 应用锚点搜索优化策略
pub fn find_anchor_optimized(anchor_name: &str, start_dir: &Path, max_depth: usize) -> Result<Option<PathBuf>> {
    log::info!("使用优化策略搜索锚点: {}", anchor_name);

    // 检查特殊目录结构并提前返回
    if let Some(result) = check_special_structures(start_dir) {
        return Ok(result);
    }

    // 使用常规搜索方法
    find_anchor(anchor_name, start_dir, max_depth)
}

/// 检查特殊的目录结构
fn check_special_structures(start_dir: &Path) -> Option<Option<PathBuf>> {
    // 检查当前目录是否为mods目录
    if start_dir.file_name().map_or(false, |name| name == "mods") {
        log::info!("当前目录是mods目录，向上查找.minecraft");
        if let Some(parent) = start_dir.parent() {
            if parent.file_name().map_or(false, |name| name == ".minecraft") {
                log::info!("当前目录结构为 .minecraft/mods，返回当前目录");
                return Some(Some(start_dir.to_path_buf()));
            }
        }
    }

    // 检查当前目录是否包含mods文件夹
    let mods_path = start_dir.join("mods");
    if mods_path.is_dir() {
        log::info!("当前目录包含mods文件夹，返回mods目录");
        return Some(Some(mods_path));
    }

    // 检查是否为.minecraft/versions结构
    if let Some(file_name) = start_dir.file_name() {
        if start_dir.parent().map_or(false, |p| p.file_name().map_or(false, |name| name == ".minecraft")) {
            if start_dir.join("mods").is_dir() {
                log::info!("当前目录为.minecraft/versions/{}，返回mods目录", file_name.to_string_lossy());
                return Some(Some(start_dir.join("mods")));
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_anchor_finder() -> Result<()> {
        // 创建临时目录结构
        let temp_dir = TempDir::new()?;
        let anchor_file = temp_dir.path().join("anchor.txt");
        fs::write(&anchor_file, "test")?;

        let result = find_anchor("anchor.txt", temp_dir.path(), 5)?;

        assert_eq!(result, Some(temp_dir.path().to_path_buf()));
        Ok(())
    }
}