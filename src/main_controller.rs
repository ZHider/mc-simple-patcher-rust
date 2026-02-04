//! 主控制器模块
//! 协调各个模块的工作

use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use crate::{
    config::{Config, GroupConfig},
    anchor_finder,
    file_manager,
    downloader,
};

/// 默认最大递归深度
const DEFAULT_MAX_DEPTH: usize = 10;

/// 执行补丁操作
pub async fn execute_patch(config: &Config) -> Result<()> {
    log::info!("开始执行补丁操作");

    for group in &config.groups {
        process_group(group).await?;
    }

    log::info!("补丁操作完成");
    Ok(())
}

/// 处理单个组
async fn process_group(group: &GroupConfig) -> Result<()> {
    log::info!("处理组: anchor={}", group.anchor);

    // 查找锚点
    let anchor_dir = anchor_finder::find_anchor_optimized(
        &group.anchor,
        &std::env::current_dir()?,
        DEFAULT_MAX_DEPTH
    )
    .with_context(|| format!("无法找到锚点: {}", group.anchor))?;

    let anchor_dir = match anchor_dir {
        Some(dir) => dir,
        None => {
            log::warn!("未找到锚点 {}, 跳过此组", group.anchor);
            return Ok(());
        }
    };

    // 计算工作目录
    let work_dir = anchor_dir.join(&group.root);
    log::info!("工作目录: {:?}", work_dir);

    // 获取目录中的现有文件
    let existing_files = file_manager::get_files_in_dir(&work_dir, group.recursive)?;

    // 处理镜像模式
    if group.mirror {
        handle_mirror_mode(&existing_files, group).await?;
    }

    // 处理文件同步
    sync_files(&work_dir, group).await?;

    Ok(())
}

/// 处理镜像模式
async fn handle_mirror_mode(existing_files: &[PathBuf], group: &GroupConfig) -> Result<()> {
    log::info!("处理镜像模式");

    // 检查现有文件是否在配置中列出
    for file_path in existing_files {
        // 检查是否与任何规则匹配
        let matched = group.files.iter()
            .any(|file_rule| {
                file_manager::matches_rule(file_path, file_rule).unwrap_or(false)
            });

        if !matched {
            // 文件不在配置中，需要处理
            if group.delete {
                // 删除文件
                std::fs::remove_file(file_path)
                    .with_context(|| format!("无法删除文件: {:?}", file_path))?;
                log::info!("已删除文件: {:?}", file_path);
            } else {
                // 将文件重命名为 .jar.disabled
                let disabled_path = file_path.with_extension(format!("{}.disabled", file_path.extension().unwrap_or_default().to_string_lossy()));
                std::fs::rename(file_path, &disabled_path)
                    .with_context(|| format!("无法重命名文件: {:?}", file_path))?;
                log::info!("已禁用文件: {:?}", disabled_path);
            }
        }
    }

    Ok(())
}

/// 同步文件
async fn sync_files(work_dir: &Path, group: &GroupConfig) -> Result<()> {
    log::info!("同步文件");

    for file_rule in &group.files {
        // 搜索目录中的文件，看是否有匹配的
        let existing_files = file_manager::get_files_in_dir(work_dir, group.recursive)?;
        let matched_file = existing_files.iter()
            .find(|file_path| file_manager::matches_rule(file_path, file_rule).unwrap_or(false));

        if let Some(file_path) = matched_file {
            // 检查是否有对应的 .jar.disabled 文件
            if let Some(disabled_path) = file_manager::find_disabled_file(file_path) {
                // 恢复 .jar.disabled 文件
                file_manager::restore_disabled_file(&disabled_path)?;
            }
        } else {
            // 没有找到匹配的文件，需要下载
            let file_name = Path::new(&file_rule.url)
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("无法从URL中提取文件名: {}", file_rule.url))?
                .to_string_lossy()
                .to_string();

            let dest_path = work_dir.join(&file_name);

            // 检查是否有对应的 .jar.disabled 文件
            if let Some(disabled_path) = file_manager::find_disabled_file(&dest_path) {
                // 恢复 .jar.disabled 文件
                file_manager::restore_disabled_file(&disabled_path)?;
            } else {
                // 下载文件
                downloader::download_file(&file_rule.url, &dest_path).await?;
            }
        }
    }

    Ok(())
}