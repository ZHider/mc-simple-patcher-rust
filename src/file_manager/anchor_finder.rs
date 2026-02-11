//! 锚点搜索模块
//! 实现锚点文件/文件夹的搜索和工作目录定位功能

use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// 搜索锚点并返回工作目录
pub fn find_anchor(anchor_name: &str, start_dir: &Path, max_depth: usize) -> Option<PathBuf> {
    log::info!("开始搜索锚点: {}", anchor_name);

    // 检查当前目录是否包含名为 anchor_name 的文件或文件夹
    let current_path = start_dir.join(anchor_name);
    if current_path.exists() {
        return if current_path.is_file() {
            // 如果是文件，返回其所在目录
            log::info!("在当前目录找到锚点文件: {}", current_path.display());
            current_path.parent().map(|p| p.to_path_buf())
        } else if current_path.is_dir() {
            // 如果是目录，返回该目录
            log::info!("在当前目录找到锚点目录: {}", current_path.display());
            Some(current_path)
        } else {
            None
        };
    }

    // 递归搜索子目录
    search_sub_dirs(anchor_name, start_dir, max_depth)
}

/// 在子目录中搜索锚点
fn search_sub_dirs(anchor_name: &str, start_dir: &Path, max_depth: usize) -> Option<PathBuf> {
    let walker = WalkDir::new(start_dir)
        .max_depth(max_depth)
        .contents_first(true);
    for file in walker {
        if file.is_err() {
            log::error!("文件搜索时遇到错误：{}", file.unwrap_err());
            continue;
        }
        let file = file.unwrap();
        if file.file_name() == anchor_name {
            return Some(file.into_path());
        }
    }
    None
}

/// 应用锚点搜索优化策略
pub fn find_anchor_optimized(
    anchor_name: &str,
    start_dir: &Path,
    max_depth: usize,
) -> Option<PathBuf> {
    log::info!("使用优化策略搜索锚点: {}", anchor_name);

    // 检查特殊目录结构并提前返回
    if let Some(result) = check_special_structures(anchor_name, start_dir) {
        return Some(result);
    }

    // 使用常规搜索方法
    find_anchor(anchor_name, start_dir, max_depth)
}

/// 检查特殊的目录结构
fn check_special_structures(anchor_name: &str, start_dir: &Path) -> Option<PathBuf> {
    /// 进行最大深度为2的广度优先搜索，查看anchor文件是否在`.minecraft\versions\{Mod Pack Name}`目录下。
    fn has_mc_vers_dir(anchor_name: &str, start_dir: &Path) -> Option<PathBuf> {
        let versions_path = start_dir.join(".minecraft").join("versions");
        if versions_path.is_dir() {
            log::info!(
                "发现.minecraft/versions文件夹，进行广度优先搜索anchor文件: {}",
                anchor_name
            );
            if let Some(result) = breadth_first_search_for_anchor(&versions_path, anchor_name, 2) {
                log::info!("在.minecraft/versions结构中找到anchor文件");
                return Some(result);
            }
        }
        None
    }

    log::debug!("当前搜索路径：{}", start_dir.display());

    // 策略1: 判断当前目录是否为`mods`。如果有，查看父目录下是否有anchor文件。
    if start_dir.file_name().is_some_and(|name| name == "mods") {
        log::info!(
            "当前目录是mods目录，检查父目录下是否有anchor文件: {}",
            anchor_name
        );
        if let Some(parent) = start_dir.parent() {
            let anchor_path = parent.join(anchor_name);
            if anchor_path.exists() && anchor_path.is_file() {
                log::info!("在父目录找到anchor文件: {}", anchor_path.display());
                return Some(parent.to_path_buf());
            }
        }
    }

    // 策略2: 判断当前目录下是否有`mods`文件夹。如果有，查看当前目录下是否有anchor文件。
    let mods_path = start_dir.join("mods");
    if mods_path.is_dir() {
        log::info!(
            "当前目录包含mods文件夹，检查当前目录下是否有anchor文件: {}",
            anchor_name
        );
        let anchor_path = start_dir.join(anchor_name);
        if anchor_path.exists() && anchor_path.is_file() {
            log::info!("在当前目录找到anchor文件: {}", anchor_path.display());
            return Some(start_dir.to_path_buf());
        }
    }

    // 策略3: 判断当前文件夹下是否有`.minecraft`、`versions`文件夹。如果有，导航到`versions`文件夹下，
    // 策略4：搜索官方路径（Appdata）下是否有versions文件夹
    has_mc_vers_dir(anchor_name, start_dir).or_else(|| {
        let appdata = std::env::var_os("APPDATA")?;
        log::debug!("找到APPDATA路径: {}", appdata.display());
        has_mc_vers_dir(anchor_name, Path::new(&appdata))
    })
}

/// 在指定目录下进行广度优先搜索，寻找anchor文件
fn breadth_first_search_for_anchor(
    versions_dir: &Path,
    anchor_name: &str,
    max_depth: usize,
) -> Option<PathBuf> {
    use std::collections::VecDeque;

    let mut queue = VecDeque::new();
    queue.push_back((versions_dir.to_path_buf(), 0)); // (目录路径, 当前深度)

    while let Some((current_dir, depth)) = queue.pop_front() {
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

            for subdir in subdirs {
                queue.push_back(subdir);
            }
        }
    }

    None
}
