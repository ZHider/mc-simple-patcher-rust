//! BSDIFF 补丁应用模块
//! 提供补丁文件的下载和应用功能

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use qbsdiff::Bspatch;
use tokio::fs;

pub use crate::config::FilePatch;

/// 备份文件路径
pub fn path_backup(file: &Path) -> PathBuf {
    file.with_added_extension("backup")
}

/// 生成备份文件路径（使用 .backup 扩展名）
pub fn make_backup_path(file: &Path) -> PathBuf {
    let base = file.file_stem().unwrap_or_default();
    let ext = file.extension().unwrap_or_default().to_string_lossy();
    let ext_str = if ext.is_empty() {
        "".to_string()
    } else {
        format!(".{}", ext)
    };
    file.with_file_name(format!("{}{}.backup", base.to_string_lossy(), ext_str))
}

/// 生成禁用文件路径（使用 .disabled 扩展名）
pub fn make_disabled_path(file: &Path) -> PathBuf {
    let base = file.file_stem().unwrap_or_default();
    let ext = file.extension().unwrap_or_default().to_string_lossy();
    let ext_str = if ext.is_empty() {
        "".to_string()
    } else {
        format!(".{}", ext)
    };
    file.with_file_name(format!("{}{}.disabled", base.to_string_lossy(), ext_str))
}

/// 应用 BSDIFF 补丁
///
/// # Arguments
///
/// * `patch_file` - 补丁文件路径
/// * `src_file` - 源文件路径
/// * `dst_file` - 目标文件路径
///
/// # Returns
///
/// * `Result<()>` - 成功时返回空值，失败时返回错误
pub async fn bspatch(patch_file: &Path, src_file: &Path, dst_file: &Path) -> Result<()> {
    log::info!(
        "开始进行文件 bspatch：'{}' + '{}' -> '{}'",
        src_file.display(),
        patch_file.display(),
        dst_file.display()
    );

    log::trace!("读取源文件：{:?}", src_file);
    let src = fs::read(src_file)
        .await
        .context("读取 src 文件到内存失败")?;
    log::trace!("源文件大小：{} bytes", src.len());

    log::trace!("读取补丁文件：{:?}", patch_file);
    let patch = fs::read(patch_file)
        .await
        .context("读取 patch file 到内存失败")?;
    log::trace!("补丁文件大小：{} bytes", patch.len());

    log::trace!("创建目标文件：{:?}", dst_file);
    let output_writer = std::fs::File::create(dst_file)?;

    log::trace!("创建 Bspatch 对象");
    let patched_bytes = Bspatch::new(&patch)
        .context("创建 bspatch 对象失败")?
        .buffer_size(128000)
        .apply(&src, output_writer)
        .context("进行文件 patch 时错误")?;
    log::debug!("文件 patch 完成，输出 {} bytes", patched_bytes);

    Ok(())
}

/// 查找补丁源文件
///
/// 在工作目录中搜索匹配 SHA256 的源文件，包括原始文件、.disabled 文件和 .backup 文件。
///
/// # Arguments
///
/// * `matched_file` - 匹配的文件路径
/// * `sha256_src` - 源文件的 SHA256 哈希值
///
/// # Returns
///
/// * `Result<Option<PathBuf>>` - 成功时返回找到的源文件路径，未找到返回 None
pub fn find_patch_source_file(matched_file: &Path, sha256_src: &str) -> Result<Option<PathBuf>> {
    log::debug!("查找补丁源文件：matched_file={:?}", matched_file);
    log::trace!("目标 sha256_src: {}", sha256_src);

    // 生成候选文件列表：原始文件、.disabled 文件、.backup 文件
    let candidates = vec![
        matched_file.to_path_buf(),
        make_disabled_path(matched_file),
        make_backup_path(matched_file),
    ];

    for candidate in &candidates {
        log::trace!("检查候选文件：{:?}", candidate);
        if !candidate.exists() {
            log::trace!("候选文件不存在，跳过");
            continue;
        }

        // 计算候选文件的 SHA256
        log::trace!("计算 SHA256: {:?}", candidate);
        let hash_bytes = crate::utils::calculate_file_sha256(candidate)?;
        let hash_hex = hex::encode(hash_bytes);
        log::trace!("计算结果：{}", hash_hex);

        if hash_hex == sha256_src {
            log::debug!("找到匹配的补丁源文件：{}", candidate.display());
            return Ok(Some(candidate.clone()));
        } else {
            log::trace!("SHA256 不匹配");
        }
    }

    log::debug!("未找到匹配的补丁源文件 (sha256_src={})", sha256_src);
    Ok(None)
}

/// 补丁下载任务（通用结构，可用于 FilePatch 和 SelfUpdatePatch）
pub struct PatchDownloadTask {
    /// 补丁 URL
    pub url: String,
    /// 补丁保存到临时目录的路径（如果为 None，则从 response 获取文件名）
    pub dest_path: Option<PathBuf>,
    /// 对应的补丁配置
    pub patch: FilePatch,
    /// 源文件路径
    pub src_file: PathBuf,
    /// 目标文件路径
    pub dst_file: PathBuf,
}

/// 收集补丁下载任务
///
/// 遍历所有文件规则的 patches，寻找 sha256_src 与当前匹配文件哈希值匹配的补丁，
/// 收集需要下载的补丁文件到任务列表中。
///
/// # Arguments
///
/// * `patches` - 补丁配置切片
/// * `work_dir` - 工作目录
/// * `matched_file` - 匹配的源文件
/// * `target_file_name` - 目标文件名
///
/// # Returns
///
/// * `Result<Option<PatchDownloadTask>>` - 成功时返回补丁任务，未找到源文件返回 None
pub fn match_patch_tasks(
    patches: &[FilePatch],
    work_dir: &Path,
    matched_file: &Path,
    target_file_name: &str,
) -> Result<Option<PatchDownloadTask>> {
    log::debug!(
        "检查补丁配置：patches_count={}, file={:?}",
        patches.len(),
        matched_file
    );

    if patches.is_empty() {
        log::trace!("补丁配置为空，跳过");
        return Ok(None);
    }

    // 计算当前匹配文件的 SHA256
    log::trace!("计算当前文件 SHA256: {:?}", matched_file);
    let current_hash_bytes = crate::utils::calculate_file_sha256(matched_file)?;
    let current_hash_hex = hex::encode(current_hash_bytes);

    log::debug!("当前文件 SHA256: {}", current_hash_hex);

    // 寻找 sha256_src 匹配的补丁
    let matching_patch = patches
        .iter()
        .find(|patch| patch.sha256_src == current_hash_hex);

    let patch = match matching_patch {
        Some(p) => {
            log::info!("找到匹配的补丁配置（sha256_src 匹配当前文件）");
            log::trace!("补丁 URL: {}", p.url_patch);
            p
        }
        None => {
            log::debug!("未找到 sha256_src 与当前文件匹配的补丁，跳过补丁流程");
            return Ok(None);
        }
    };

    // 验证源文件确实存在
    log::trace!("验证源文件存在");
    let src_file = match find_patch_source_file(matched_file, &patch.sha256_src)? {
        Some(f) => {
            log::debug!("源文件验证通过：{:?}", f);
            f
        }
        None => {
            log::warn!("未找到匹配 sha256_src 的源文件，跳过补丁流程");
            return Ok(None);
        }
    };

    let dst_file = work_dir.join(target_file_name);
    log::trace!("目标文件路径：{:?}", dst_file);

    // 创建补丁下载任务（dest_path 为 None，让下载模块从 response 获取文件名）
    let task = PatchDownloadTask {
        url: patch.url_patch.clone(),
        dest_path: None, // 从 response 获取文件名
        patch: patch.clone(),
        src_file,
        dst_file,
    };

    log::debug!("创建补丁下载任务：url={}", task.url);
    Ok(Some(task))
}

/// 处理源文件（当 src 和 dst 相同时）
///
/// 当源文件和目标文件路径相同时，将源文件重命名为 .backup。
///
/// # Arguments
///
/// * `src_file` - 源文件路径
///
/// # Returns
///
/// * `Result<PathBuf>` - 成功时返回备份文件路径，失败时返回错误
fn handle_src_dst_conflict(src_file: &Path) -> Result<PathBuf> {
    let backup_path = make_backup_path(src_file);
    log::info!(
        "src 和 dst 文件名相同，重命名源文件为 .backup: {}",
        backup_path.display()
    );
    std::fs::rename(src_file, &backup_path)
        .context(format!("无法重命名源文件为 .backup: {:?}", src_file))?;
    Ok(backup_path)
}

/// 处理源文件保留/删除
///
/// 根据 keep_src 参数决定是保留源文件（重命名为 .backup）还是删除源文件。
///
/// # Arguments
///
/// * `src_file` - 源文件路径
/// * `keep_src` - 是否保留源文件
///
/// # Returns
///
/// * `Result<()>` - 成功时返回空值，失败时返回错误
fn handle_src_after_patch(src_file: &Path, keep_src: bool) -> Result<()> {
    if keep_src {
        let backup_path = make_backup_path(src_file);
        log::info!("保留源文件为 .backup: {}", backup_path.display());
        std::fs::rename(src_file, &backup_path)
            .context(format!("无法重命名源文件为 .backup: {:?}", src_file))?;
    } else {
        log::info!("删除源文件：{}", src_file.display());
        std::fs::remove_file(src_file).context(format!("无法删除源文件：{:?}", src_file))?;
    }
    Ok(())
}

/// 应用已下载的补丁
///
/// 按顺序应用已下载的补丁文件，处理 src/dst 冲突和源文件保留。
///
/// # Arguments
///
/// * `tasks` - 已完成的补丁下载任务列表（可变引用）
///
/// # Returns
///
/// * `Result<Option<PathBuf>>` - 成功时返回最终补丁后的文件路径
pub async fn apply_downloaded_patches(tasks: &mut [PatchDownloadTask]) -> Result<Option<PathBuf>> {
    let tasks_len = tasks.len();
    log::debug!("开始应用补丁，任务数：{}", tasks_len);

    if tasks_len == 0 {
        log::trace!("补丁任务列表为空，跳过");
        return Ok(None);
    }

    let mut current_file: Option<PathBuf> = None;

    for (idx, task) in tasks.iter_mut().enumerate() {
        log::debug!("处理补丁任务 [{}/{}]: {}", idx + 1, tasks_len, task.url);

        let src_file = if current_file.is_none() {
            log::trace!("使用初始源文件：{:?}", task.src_file);
            task.src_file.clone()
        } else {
            log::trace!("使用上一次补丁结果作为源文件：{:?}", current_file);
            current_file.clone().unwrap()
        };

        log::info!("应用补丁：{}", task.patch.url_patch);

        // 确保 dest_path 已设置
        let patch_path = task
            .dest_path
            .as_ref()
            .context("补丁文件路径未设置，需要先下载补丁文件")?;
        log::trace!("补丁文件路径：{:?}", patch_path);

        // 检查 src 和 dst 是否相同
        let src_is_dst = src_file == task.dst_file;
        log::trace!("src 和 dst 是否相同：{}", src_is_dst);

        // 如果 src 和 dst 相同，先重命名源文件为 .backup
        let actual_src = if src_is_dst {
            log::debug!("src 和 dst 相同，处理冲突");
            handle_src_dst_conflict(&src_file)?
        } else {
            src_file
        };

        // 应用补丁
        log::info!("正在应用补丁到：{}", task.dst_file.display());
        bspatch(patch_path, &actual_src, &task.dst_file)
            .await
            .context("应用补丁失败")?;

        // 清理补丁文件
        log::trace!("清理补丁临时文件：{:?}", patch_path);
        let _ = std::fs::remove_file(patch_path);

        // 根据 keep_src 处理源文件（如果 src 和 dst 相同则已处理）
        if !src_is_dst {
            log::debug!("处理源文件，keep_src={}", task.patch.keep_src);
            handle_src_after_patch(&actual_src, task.patch.keep_src)?;
        }

        current_file = Some(task.dst_file.clone());
        log::info!("补丁应用成功：{}", current_file.as_ref().unwrap().display());
    }

    log::debug!("所有补丁应用完成，最终文件：{:?}", current_file);
    Ok(current_file)
}
