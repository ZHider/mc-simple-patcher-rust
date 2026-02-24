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
    sync::Arc,
};

use rayon::prelude::*;

/// 默认最大递归深度
pub const DEFAULT_MAX_DEPTH: usize = 5;

/// 执行补丁操作
///
/// # Arguments
///
/// * `config` - 配置对象的原子引用计数指针
///
/// # Returns
///
/// * `Result<()>` - 成功时返回空值，失败时返回错误
pub async fn execute_patch(config: Arc<Config>) -> Result<()> {
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
///
/// # Arguments
///
/// * `group` - 组配置的引用
///
/// # Returns
///
/// * `Result<()>` - 成功时返回空值，失败时返回错误
async fn process_group(group: &GroupConfig) -> Result<()> {
    log::info!("处理组：anchor={}", group.anchor);
    log::debug!("组配置：{:?}", group);

    // 查找锚点
    let Some(ref anchor_dir) = anchor_finder::find_anchor_optimized(
        &group.anchor,
        &std::env::current_dir()?,
        DEFAULT_MAX_DEPTH,
    ) else {
        log::error!("未找到锚点 {}, 跳过此组", group.anchor);
        return Ok(());
    };

    log::debug!("找到锚点目录：{}", anchor_dir.display());

    // 计算工作目录
    let work_dir = anchor_dir.join(&group.root);
    log::info!("工作目录：{}", work_dir.display());

    // 获取符合这个组规则的所有现有文件
    let pattern = group
        .pattern
        .as_deref()
        .map(|pattern_str| {
            Regex::new(pattern_str).context(format!("无效的正则表达式：{}", pattern_str))
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
    .context("构建 MOD 信息缓存时出错……这个真的是实际存在的错误吗？")?;

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
            "镜像模式：需要处理 {} 个剩余文件",
            files_need_mirroring.len()
        );
        execute_mirror_mode(files_need_mirroring.into_iter(), group).await?;
    }

    Ok(())
}

/// 处理镜像模式
///
/// # Arguments
///
/// * `files` - 文件迭代器
/// * `group` - 组配置的引用
///
/// # Returns
///
/// * `Result<()>` - 成功时返回空值，失败时返回错误
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
            std::fs::remove_file(file).context(format!("无法删除文件：{:?}", file))?;
            log::info!("已删除文件：{}", file.display());
        } else {
            // 将文件重命名为 .jar.disabled
            let disabled_path = file.with_extension(format!(
                "{}.disabled",
                file.extension().unwrap_or_default().to_string_lossy()
            ));
            std::fs::rename(file, &disabled_path).context(format!("无法重命名文件：{:?}", file))?;
            log::info!("已禁用文件：{}", disabled_path.display());
        }
    }

    Ok(())
}

/// 同步文件结果
struct SyncResult {
    files_to_download: Vec<DownloadTask>,
    patch_tasks: Vec<crate::utils::downloader::bspatch::PatchDownloadTask>,
}

/// 处理单个文件规则
///
/// # Arguments
///
/// * `file_rule` - 文件规则
/// * `matched_file` - 匹配的文件路径
/// * `work_dir` - 工作目录
/// * `files_to_download` - 下载任务列表
/// * `patch_tasks` - 补丁任务列表
///
/// # Returns
///
/// * `Result<()>` - 成功时返回空值，失败时返回错误
fn process_matched_file(
    file_rule: &crate::config::FileRule,
    matched_file: &Path,
    work_dir: &Path,
    files_to_download: &mut Vec<DownloadTask>,
    patch_tasks: &mut Vec<crate::utils::downloader::bspatch::PatchDownloadTask>,
) -> Result<()> {
    log::info!("找到匹配的文件：{}", matched_file.display());

    // 检查是否需要应用补丁
    if !file_rule.patches.is_empty() {
        let target_file_name = Path::new(&file_rule.url)
            .file_name()
            .context("无法从 URL 提取文件名")?
            .to_string_lossy()
            .to_string();

        // 收集补丁下载任务
        if let Some(task) = crate::utils::downloader::bspatch::match_patch_tasks(
            &file_rule.patches,
            work_dir,
            matched_file,
            &target_file_name,
        )? {
            // 将补丁任务转换为 DownloadTask 并添加到下载列表
            files_to_download.push(DownloadTask {
                url: task.url.clone(),
                dest_path: task.dest_path.clone(),
                check_sha256: false,
            });
            // 保存补丁任务以便后续应用
            patch_tasks.push(task);
        }
    }

    Ok(())
}

/// 处理未匹配的文件规则
///
/// # Arguments
///
/// * `file_rule` - 文件规则
/// * `work_dir` - 工作目录
///
/// # Returns
///
/// * `Result<Option<DownloadTask>>` - 成功时返回下载任务选项
async fn process_unmatched_file(
    file_rule: &crate::config::FileRule,
    work_dir: &Path,
) -> Result<Option<DownloadTask>> {
    log::debug!("未找到匹配的文件……");
    build_file_downloadinfo(work_dir, file_rule).await
}

/// 应用补丁并处理结果
///
/// # Arguments
///
/// * `patch_tasks` - 补丁任务列表（可变引用，用于更新下载后的路径）
/// * `group` - 组配置
/// * `work_dir` - 工作目录
/// * `files_left_from_existing` - 剩余文件集合
///
/// # Returns
///
/// * `Result<()>` - 成功时返回空值，失败时返回错误
async fn apply_patches_and_handle_result(
    patch_tasks: &mut [crate::utils::downloader::bspatch::PatchDownloadTask],
    group: &GroupConfig,
    work_dir: &Path,
    files_left_from_existing: &mut Option<HashSet<PathBuf>>,
) -> Result<()> {
    if patch_tasks.is_empty() {
        return Ok(());
    }

    // 下载补丁文件（如果 dest_path 为 None）
    for task in patch_tasks.iter_mut() {
        if task.dest_path.is_none() {
            log::info!("正在下载补丁：{}", task.url);
            let patch_path = crate::utils::downloader::download_patch_file_auto(&task.url).await?;
            task.dest_path = Some(patch_path);
        }
    }

    match crate::utils::downloader::bspatch::apply_downloaded_patches(patch_tasks).await {
        Ok(Some(new_path)) => {
            log::info!("补丁应用成功：{}", new_path.display());
            if let Some(set) = files_left_from_existing {
                set.insert(new_path.clone());
            }
        }
        Ok(None) => {
            log::warn!("未找到补丁源文件，将下载完整文件");
            download_fallback_files(group, work_dir).await?;
        }
        Err(e) => {
            log::warn!("补丁应用失败：{}，将下载完整文件", e);
            download_fallback_files(group, work_dir).await?;
        }
    }

    Ok(())
}

/// 下载回退文件（补丁失败时）
///
/// # Arguments
///
/// * `group` - 组配置
/// * `work_dir` - 工作目录
///
/// # Returns
///
/// * `Result<()>` - 成功时返回空值，失败时返回错误
async fn download_fallback_files(group: &GroupConfig, work_dir: &Path) -> Result<()> {
    for file_rule in &group.files {
        if !file_rule.patches.is_empty()
            && let Some(task) = build_file_downloadinfo(work_dir, file_rule).await?
        {
            download_files_with_progress(vec![task]).await?;
        }
    }
    Ok(())
}

/// 同步文件
///
/// # Arguments
///
/// * `existing_files` - 现有文件路径切片的引用
/// * `group` - 组配置的引用
/// * `work_dir` - 工作目录路径的引用
/// * `modinfo_cache` - MOD 信息缓存
/// * `files_left_from_existing` - 剩余文件集合的可选可变引用
///
/// # Returns
///
/// * `Result<()>` - 成功时返回空值，失败时返回错误
async fn sync_files(
    existing_files: &[PathBuf],
    group: &GroupConfig,
    work_dir: &Path,
    modinfo_cache: ModInfoCache,
    files_left_from_existing: &mut Option<HashSet<PathBuf>>,
) -> Result<()> {
    log::info!("正准备同步 {} 个文件……", group.files.len());

    // 收集所有下载任务
    let SyncResult {
        files_to_download,
        mut patch_tasks,
    } = collect_download_tasks(
        existing_files,
        group,
        work_dir,
        &modinfo_cache,
        files_left_from_existing,
    )
    .await?;

    // 统一下载所有文件（包括补丁文件）
    if !files_to_download.is_empty() {
        download_files_with_progress(files_to_download.into_iter())
            .await
            .context("批量下载文件时出错……")?;
    }

    // 应用已下载的补丁
    apply_patches_and_handle_result(&mut patch_tasks, group, work_dir, files_left_from_existing).await?;

    Ok(())
}

/// 收集下载任务
///
/// # Arguments
///
/// * `existing_files` - 现有文件路径切片
/// * `group` - 组配置
/// * `work_dir` - 工作目录
/// * `modinfo_cache` - MOD 信息缓存
/// * `files_left_from_existing` - 剩余文件集合
///
/// # Returns
///
/// * `Result<SyncResult>` - 成功时返回同步结果
async fn collect_download_tasks(
    existing_files: &[PathBuf],
    group: &GroupConfig,
    work_dir: &Path,
    modinfo_cache: &ModInfoCache,
    files_left_from_existing: &mut Option<HashSet<PathBuf>>,
) -> Result<SyncResult> {
    let mut files_to_download = Vec::new();
    let mut patch_tasks = Vec::new();

    for (index, file_rule) in group.files.iter().enumerate() {
        log::debug!("处理第 {} 个文件规则", index + 1);

        // 检查是否有匹配的活动文件
        let matched_file = existing_files.iter().find(|file_path| {
            file_manager::matches_rule(file_path, file_rule, modinfo_cache).unwrap_or(false)
        });

        if let Some(file_path) = matched_file {
            // 处理匹配的文件
            process_matched_file(
                file_rule,
                file_path,
                work_dir,
                &mut files_to_download,
                &mut patch_tasks,
            )?;

            // 检查并恢复禁用的文件
            handle_disabled_file(file_path)?;

            // 如果启用镜像模式，记录已处理文件
            if let Some(set) = files_left_from_existing {
                set.insert(file_path.clone());
            }
        } else {
            // 处理未匹配的文件
            if let Some(task) = process_unmatched_file(file_rule, work_dir).await? {
                files_to_download.push(task);
            }
        }
    }

    Ok(SyncResult {
        files_to_download,
        patch_tasks,
    })
}

/// 构建文件下载信息
///
/// # Arguments
///
/// * `work_dir` - 工作目录路径的引用
/// * `file_rule` - 文件规则的引用
///
/// # Returns
///
/// * `Result<Option<DownloadTask>, anyhow::Error>` - 成功时返回下载任务选项，失败时返回错误
async fn build_file_downloadinfo(
    work_dir: &Path,
    file_rule: &crate::config::FileRule,
) -> Result<Option<DownloadTask>, anyhow::Error> {
    let file_name = Path::new(&file_rule.url)
        .file_name()
        .context(format!("无法从 URL 中提取文件名：{}", file_rule.url))?
        .to_string_lossy()
        .to_string();
    let dest_path = work_dir.join(&file_name);
    if handle_disabled_file(&dest_path)? {
        // 文件已从禁用状态恢复，无需下载
        Ok(None)
    } else {
        // log::info!("下载文件：{}", file_rule.url);
        // 下载文件
        Ok(Some(DownloadTask {
            url: file_rule.url.clone(),
            dest_path: Some(dest_path),
            check_sha256: file_rule.sha256.is_some(), // 如果配置中有 SHA256，则启用校验
        }))
    }
}

/// 检查并恢复禁用的文件
///
/// # Arguments
///
/// * `file_path` - 文件路径的引用
///
/// # Returns
///
/// * `Result<bool>` - 成功时返回布尔值表示是否恢复了文件，失败时返回错误
fn handle_disabled_file(file_path: &Path) -> Result<bool> {
    if let Some(disabled_path) = file_manager::find_disabled_file(file_path) {
        log::info!("恢复禁用文件：{}", disabled_path.display());
        file_manager::restore_disabled_file(&disabled_path)?;
        Ok(true) // 表示文件已恢复
    } else {
        Ok(false) // 表示没有禁用的文件
    }
}
