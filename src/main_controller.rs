//! 主控制器模块
//! 协调各个模块的工作

use crate::{
    config::{Config, GroupConfig},
    file_manager::{self, anchor_finder},
    utils::downloader,
};
use anyhow::{Context, Result};
use regex::Regex;
use std::path::{Path, PathBuf};

/// 默认最大递归深度
const DEFAULT_MAX_DEPTH: usize = 10;

/// 执行补丁操作
pub async fn execute_patch(config: &Config) -> Result<()> {
    log::info!("开始执行补丁操作");
    log::debug!("当前有 {} 个组需要处理", config.groups.len());

    for (index, group) in config.groups.iter().enumerate() {
        log::debug!("处理第 {} 个组，anchor: {}", index, group.anchor);
        process_group(group).await?;
    }

    log::info!("补丁操作完成");
    Ok(())
}

/// 处理单个组
async fn process_group(group: &GroupConfig) -> Result<()> {
    log::info!("处理组: anchor={}", group.anchor);
    log::debug!("组配置: {:?}", group);

    // 查找锚点
    let anchor_dir = anchor_finder::find_anchor_optimized(
        &group.anchor,
        &std::env::current_dir()?,
        DEFAULT_MAX_DEPTH,
    )
    .context(format!("无法找到锚点: {}", group.anchor))?;

    let anchor_dir = match anchor_dir {
        Some(dir) => {
            log::debug!("找到锚点目录: {}", dir.display());
            dir
        }
        None => {
            log::error!("未找到锚点 {}, 跳过此组", group.anchor);
            return Ok(());
        }
    };

    // 计算工作目录
    let work_dir = anchor_dir.join(&group.root);
    log::info!("工作目录: {}", work_dir.display());

    // 获取符合这个组规则的所有现有文件
    let pattern = group
        .pattern
        .as_ref()
        .map(|pattern_str| {
            Regex::new(pattern_str).context(format!("无效的正则表达式: {}", pattern_str))
        })
        .transpose()?;
    let existing_files =
        file_manager::get_files_in_dir(&work_dir, group.recursive, pattern.as_ref())?;
    log::debug!("找到 {} 个现有文件", existing_files.len());

    // 处理镜像模式
    if group.mirror {
        log::debug!("启用镜像模式。");
        handle_mirror_mode(&existing_files, group).await?;
    }

    // 处理文件同步
    sync_files(&existing_files, group, &work_dir).await?;

    Ok(())
}

/// 处理镜像模式
async fn handle_mirror_mode(existing_files: &[PathBuf], group: &GroupConfig) -> Result<()> {
    log::info!("处理镜像模式……");

    // 检查现有文件是否在配置中列出
    for file_path in existing_files {
        // 检查是否与任何规则匹配
        let matched = group
            .files
            .iter()
            .any(|file_rule| file_manager::matches_rule(file_path, file_rule).unwrap_or(false));

        if !matched {
            // 文件不在配置中，需要处理
            if group.delete {
                // 删除文件
                std::fs::remove_file(file_path)
                    .context(format!("无法删除文件: {:?}", file_path))?;
                log::info!("已删除文件: {}", file_path.display());
            } else {
                // 将文件重命名为 .jar.disabled
                let disabled_path = file_path.with_extension(format!(
                    "{}.disabled",
                    file_path.extension().unwrap_or_default().to_string_lossy()
                ));
                std::fs::rename(file_path, &disabled_path)
                    .context(format!("无法重命名文件: {:?}", file_path))?;
                log::info!("已禁用文件: {}", disabled_path.display());
            }
        }
    }

    Ok(())
}

/// 同步文件
async fn sync_files(existing_files: &[PathBuf], group: &GroupConfig, work_dir: &Path) -> Result<()> {
    log::info!("正准备同步 {} 个文件……", group.files.len());

    for (index, file_rule) in group.files.iter().enumerate() {
        log::debug!("处理第 {} 个文件规则", index + 1);

        // 检查是否有匹配的活动文件
        let matched_file = existing_files
            .iter()
            .find(|file_path| file_manager::matches_rule(file_path, file_rule).unwrap_or(false));

        if let Some(file_path) = matched_file {
            log::info!("找到匹配的文件: {}", file_path.display());
            // 检查并恢复禁用的文件
            handle_disabled_file(file_path)?;
        } else {
            log::debug!("未找到匹配的文件……");
            // 没有找到匹配的文件，需要下载
            let file_name = Path::new(&file_rule.url)
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("无法从URL中提取文件名: {}", file_rule.url))?
                .to_string_lossy()
                .to_string();

            let dest_path = work_dir.join(&file_name);

            // 检查并恢复禁用的文件，如果存在则不需要下载
            if handle_disabled_file(&dest_path)? {
                // 文件已从禁用状态恢复，无需下载
            } else {
                log::info!("下载文件: {}", file_rule.url);
                // 下载文件
                downloader::download_file(&file_rule.url, &dest_path).await?;
            }
        }
    }

    Ok(())
}

/// 检查并恢复禁用的文件
fn handle_disabled_file(file_path: &Path) -> Result<bool> {
    if let Some(disabled_path) = file_manager::find_disabled_file(file_path) {
        log::info!("恢复禁用文件: {}", disabled_path.display());
        file_manager::restore_disabled_file(&disabled_path)?;
        Ok(true) // 表示文件已恢复
    } else {
        Ok(false) // 表示没有禁用的文件
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, GroupConfig, MetadataConfig};
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_execute_patch_with_mock_config() -> Result<()> {
        // 创建临时目录
        let temp_dir = TempDir::new()?;
        let anchor_file = temp_dir.path().join("test_anchor.txt");
        fs::write(&anchor_file, "test content")?;

        // 创建一个测试配置
        let config = Config {
            metadata_config: MetadataConfig {
                metadata: Some("Test Modpack".to_string()),
                version: Some(1),
            },
            groups: vec![GroupConfig {
                anchor: "test_anchor.txt".to_string(),
                root: "mods".to_string(),
                recursive: false,
                mirror: false,
                delete: false,
                pattern: None,
                files: vec![],
            }],
        };

        // 执行补丁操作
        let result = execute_patch(&config).await;

        // 验证结果
        assert!(result.is_ok());

        Ok(())
    }
}
