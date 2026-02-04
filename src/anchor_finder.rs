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
    if let Some(result) = check_special_structures(anchor_name, start_dir) {
        return Ok(result);
    }

    // 使用常规搜索方法
    find_anchor(anchor_name, start_dir, max_depth)
}

/// 检查特殊的目录结构
fn check_special_structures(anchor_name: &str, start_dir: &Path) -> Option<Option<PathBuf>> {
    // 策略1: 判断当前目录是否为`mods`。如果有，查看父目录下是否有anchor文件。
    if start_dir.file_name().map_or(false, |name| name == "mods") {
        log::info!("当前目录是mods目录，检查父目录下是否有anchor文件: {}", anchor_name);
        if let Some(parent) = start_dir.parent() {
            let anchor_path = parent.join(anchor_name);
            if anchor_path.exists() && anchor_path.is_file() {
                log::info!("在父目录找到anchor文件: {:?}", anchor_path);
                return Some(Some(parent.to_path_buf()));
            }
        }
    }

    // 策略2: 判断当前目录下是否有`mods`文件夹。如果有，查看当前目录下是否有anchor文件。
    let mods_path = start_dir.join("mods");
    if mods_path.is_dir() {
        log::info!("当前目录包含mods文件夹，检查当前目录下是否有anchor文件: {}", anchor_name);
        let anchor_path = start_dir.join(anchor_name);
        if anchor_path.exists() && anchor_path.is_file() {
            log::info!("在当前目录找到anchor文件: {:?}", anchor_path);
            return Some(Some(start_dir.to_path_buf()));
        }
    }

    // 策略3: 判断当前文件夹下是否有`.minecraft`、`versions`文件夹。如果有，导航到`versions`文件夹下，
    // 进行最大深度为2的广度优先搜索，查看anchor文件是否在`.minecraft\versions\{Mod Pack Name}`目录下。
    let minecraft_path = start_dir.join(".minecraft");
    let versions_path = start_dir.join("versions");
    if minecraft_path.is_dir() && versions_path.is_dir() {
        log::info!("发现.minecraft和versions文件夹，进行广度优先搜索anchor文件: {}", anchor_name);
        if let Some(result) = breadth_first_search_for_anchor(&versions_path, anchor_name, 2) {
            log::info!("在.minecraft/versions结构中找到anchor文件");
            return Some(Some(result));
        }
    }

    None
}

/// 在指定目录下进行广度优先搜索，寻找anchor文件
fn breadth_first_search_for_anchor(versions_dir: &Path, anchor_name: &str, max_depth: usize) -> Option<PathBuf> {
    let mut queue = vec![(versions_dir.to_path_buf(), 0)]; // (目录路径, 当前深度)

    while let Some((current_dir, depth)) = queue.pop() {
        if depth > max_depth {
            continue;
        }

        // 检查当前目录是否包含anchor文件
        let anchor_path = current_dir.join(anchor_name);
        if anchor_path.exists() && anchor_path.is_file() {
            return Some(current_dir);
        }

        // 如果还没达到最大深度，将子目录加入队列
        if depth < max_depth {
            let subdirs: Vec<_> = std::fs::read_dir(&current_dir)
                .ok()?
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.is_dir())
                .map(|path| (path, depth + 1))
                .collect();

            queue.extend(subdirs);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// 测试用的最大递归深度
    const TEST_MAX_DEPTH: usize = 5;

    #[test]
    fn test_anchor_finder() -> Result<()> {
        // 创建临时目录结构
        let temp_dir = TempDir::new()?;
        let anchor_file = temp_dir.path().join("anchor.txt");
        fs::write(&anchor_file, "test")?;

        let result = find_anchor("anchor.txt", temp_dir.path(), TEST_MAX_DEPTH)?;

        assert_eq!(result, Some(temp_dir.path().to_path_buf()));
        Ok(())
    }

    #[test]
    fn test_check_special_structures_mods_dir() -> Result<()> {
        // 创建临时目录结构: temp/mods/ + temp/anchor.txt
        let temp_dir = TempDir::new()?;
        let mods_dir = temp_dir.path().join("mods");
        std::fs::create_dir(&mods_dir)?;

        let anchor_file = temp_dir.path().join("anchor.txt");
        fs::write(&anchor_file, "test")?;

        let result = check_special_structures("anchor.txt", temp_dir.path());

        assert_eq!(result, Some(Some(temp_dir.path().to_path_buf())));
        Ok(())
    }
}