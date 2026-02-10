//! 主控制器模块
//! 协调各个模块的工作

use crate::{
    config::{Config, GroupConfig},
    file_manager::{self, anchor_finder, modinfo_cache::ModInfoCache},
    utils::downloader::{DownloadTask, download_files_with_progress},
};
use anyhow::{Context, Result};
use regex::Regex;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use rayon::prelude::*;

/// 默认最大递归深度
const DEFAULT_MAX_DEPTH: usize = 5;

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
        .as_deref()
        .map(|pattern_str| {
            Regex::new(pattern_str).context(format!("无效的正则表达式: {}", pattern_str))
        })
        .transpose()?;
    let existing_files =
        file_manager::get_files_in_dir(&work_dir, group.recursive, pattern.as_ref())?;
    log::debug!("找到 {} 个现有文件", existing_files.len());

    log::info!("开始提取现有文件信息……");
    let modinfo_cache = file_manager::modinfo_cache::extract_modinfo(
        existing_files.par_iter(),
        Some(existing_files.len()),
    )
    .context("构建MOD信息缓存时出错……这个真的是实际存在的错误吗？")?;

    // 处理镜像模式
    let mut processd_file = if group.mirror {
        log::debug!("启用镜像模式，将构建剩余文件列表缓存……");
        // execute_mirror_mode(&existing_files, group, &modinfo_cache).await?;
        Some(HashSet::<PathBuf>::new())
    } else {
        None
    };
    // 处理文件同步
    sync_files(
        &existing_files,
        group,
        &work_dir,
        modinfo_cache,
        &mut processd_file,
    )
    .await?;

    // 处理镜像模式剩余文件
    if let Some(processd_file_set) = processd_file {
        // 这里直接消耗掉所有东西！
        let files_existing_set: HashSet<PathBuf> = existing_files.into_iter().collect();
        let files_need_mirroring: Vec<PathBuf> = files_existing_set
            .difference(&processd_file_set)
            .cloned()
            .collect();
        log::info!(
            "镜像模式: 需要处理 {} 个剩余文件",
            files_need_mirroring.len()
        );
        execute_mirror_mode(files_need_mirroring.into_iter(), group).await?;
    }

    Ok(())
}

/// 处理镜像模式
async fn execute_mirror_mode<I>(files: I, group: &GroupConfig) -> Result<()>
where
    I: Iterator,
    I::Item: AsRef<Path>,
{
    log::info!("处理镜像模式……");

    for file in files {
        let file = file.as_ref();
        // 文件不在配置中，需要处理
        if group.delete {
            // 删除文件
            std::fs::remove_file(file).context(format!("无法删除文件: {:?}", file))?;
            log::info!("已删除文件: {}", file.display());
        } else {
            // 将文件重命名为 .jar.disabled
            let disabled_path = file.with_extension(format!(
                "{}.disabled",
                file.extension().unwrap_or_default().to_string_lossy()
            ));
            std::fs::rename(file, &disabled_path).context(format!("无法重命名文件: {:?}", file))?;
            log::info!("已禁用文件: {}", disabled_path.display());
        }
    }

    Ok(())
}

/// 同步文件
async fn sync_files(
    existing_files: &[PathBuf],
    group: &GroupConfig,
    work_dir: &Path,
    modinfo_cache: ModInfoCache,
    files_left_from_existing: &mut Option<HashSet<PathBuf>>,
) -> Result<()> {
    log::info!("正准备同步 {} 个文件……", group.files.len());

    let mut files_needs_download: Vec<DownloadTask> = Vec::new();

    for (index, file_rule) in group.files.iter().enumerate() {
        log::debug!("处理第 {} 个文件规则", index + 1);

        // 检查是否有匹配的活动文件
        let matched_file = existing_files.iter().find(|file_path| {
            file_manager::matches_rule(file_path, file_rule, &modinfo_cache).unwrap_or(false)
        });

        if let Some(file_path) = matched_file {
            log::info!("找到匹配的文件: {}", file_path.display());

            // 检查并恢复禁用的文件
            handle_disabled_file(file_path)?;

            // 如果启用镜像模式，就记录已经处理过的文件
            if let Some(set) = files_left_from_existing {
                set.insert(file_path.clone());
            }
        } else {
            log::debug!("未找到匹配的文件……");
            // 没有找到匹配的文件，需要下载
            if let Some(task) = build_file_downloadinfo(work_dir, file_rule).await? {
                files_needs_download.push(task);
            }
        }
    }

    if !files_needs_download.is_empty() {
        download_files_with_progress(files_needs_download.into_iter())
            .await
            .context("批量下载文件时出错……")?;
    }

    Ok(())
}

async fn build_file_downloadinfo(
    work_dir: &Path,
    file_rule: &crate::config::FileRule,
) -> Result<Option<DownloadTask>, anyhow::Error> {
    let file_name = Path::new(&file_rule.url)
        .file_name()
        .context(format!("无法从URL中提取文件名: {}", file_rule.url))?
        .to_string_lossy()
        .to_string();
    let dest_path = work_dir.join(&file_name);
    if handle_disabled_file(&dest_path)? {
        // 文件已从禁用状态恢复，无需下载
        Ok(None)
    } else {
        // log::info!("下载文件: {}", file_rule.url);
        // 下载文件
        Ok(Some(DownloadTask {
            url: file_rule.url.clone(),
            dest_path,
            check_sha256: false,
        }))
    }
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
