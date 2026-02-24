//! BSDIFF 补丁应用模块
//! 提供补丁文件的下载和应用功能

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use qbsdiff::Bspatch;
use tokio::fs;

use crate::config::FilePatch;

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
    log::debug!(
        "开始进行文件 bspatch：'{}' + '{}' -> '{}'",
        src_file.display(),
        patch_file.display(),
        dst_file.display()
    );

    let src = fs::read(src_file).await.context("读取 src 文件到内存失败")?;
    let patch = fs::read(patch_file)
        .await
        .context("读取 patch file 到内存失败")?;
    let output_writer = std::fs::File::create(dst_file)?;

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
pub fn find_patch_source_file(
    matched_file: &Path,
    sha256_src: &str,
) -> Result<Option<PathBuf>> {
    // 生成候选文件列表：原始文件、.disabled 文件、.backup 文件
    let candidates = vec![
        matched_file.to_path_buf(),
        make_disabled_path(matched_file),
        make_backup_path(matched_file),
    ];

    for candidate in candidates {
        if !candidate.exists() {
            continue;
        }

        // 计算候选文件的 SHA256
        let hash_bytes = crate::utils::calculate_file_sha256(&candidate)?;
        let hash_hex = hex::encode(hash_bytes);

        if hash_hex == sha256_src {
            log::debug!("找到匹配的补丁源文件：{}", candidate.display());
            return Ok(Some(candidate));
        }
    }

    log::debug!("未找到匹配的补丁源文件");
    Ok(None)
}

/// 补丁下载任务
pub struct PatchDownloadTask {
    /// 补丁 URL
    pub url: String,
    /// 补丁保存到临时目录的路径
    pub dest_path: PathBuf,
    /// 对应的补丁配置
    pub patch: FilePatch,
    /// 源文件路径
    pub src_file: PathBuf,
    /// 目标文件路径
    pub dst_file: PathBuf,
}

/// 收集补丁下载任务
///
/// 遍历所有文件规则的 patches，收集需要下载的补丁文件到任务列表中。
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
/// * `Result<Option<Vec<PatchDownloadTask>>>` - 成功时返回补丁任务列表，未找到源文件返回 None
pub fn collect_patch_tasks(
    patches: &[FilePatch],
    work_dir: &Path,
    matched_file: &Path,
    target_file_name: &str,
) -> Result<Option<Vec<PatchDownloadTask>>> {
    if patches.is_empty() {
        return Ok(None);
    }

    log::info!("检测到 {} 个补丁，准备下载...", patches.len());

    let mut tasks = Vec::new();
    let current_src: Option<PathBuf> = None;

    for patch in patches {
        // 第一个补丁需要查找源文件
        let src_file = if current_src.is_none() {
            match find_patch_source_file(matched_file, &patch.sha256_src)? {
                Some(f) => f,
                None => {
                    log::warn!("未找到匹配 sha256_src 的源文件，跳过补丁流程");
                    return Ok(None);
                }
            }
        } else {
            current_src.clone().unwrap()
        };

        let dst_file = work_dir.join(target_file_name);

        // 生成补丁文件的临时路径
        let file_name = Path::new(&patch.url_patch)
            .file_name()
            .context(format!("无法从 URL 中提取文件名：{}", patch.url_patch))?
            .to_string_lossy()
            .to_string();
        let dest_path = crate::utils::temp_dir()?.join(&file_name);

        tasks.push(PatchDownloadTask {
            url: patch.url_patch.clone(),
            dest_path,
            patch: patch.clone(),
            src_file,
            dst_file,
        });
    }

    Ok(Some(tasks))
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
    std::fs::rename(src_file, &backup_path).context(format!(
        "无法重命名源文件为 .backup: {:?}",
        src_file
    ))?;
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
        std::fs::rename(src_file, &backup_path).context(format!(
            "无法重命名源文件为 .backup: {:?}",
            src_file
        ))?;
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
/// * `tasks` - 已完成的补丁下载任务列表
///
/// # Returns
///
/// * `Result<Option<PathBuf>>` - 成功时返回最终补丁后的文件路径
pub async fn apply_downloaded_patches(
    tasks: Vec<PatchDownloadTask>,
) -> Result<Option<PathBuf>> {
    if tasks.is_empty() {
        return Ok(None);
    }

    let mut current_file: Option<PathBuf> = None;

    for task in tasks {
        let src_file = if current_file.is_none() {
            task.src_file
        } else {
            current_file.clone().unwrap()
        };

        log::info!("应用补丁：{}", task.patch.url_patch);

        // 检查 src 和 dst 是否相同
        let src_is_dst = src_file == task.dst_file;

        // 如果 src 和 dst 相同，先重命名源文件为 .backup
        let actual_src = if src_is_dst {
            handle_src_dst_conflict(&src_file)?
        } else {
            src_file
        };

        // 应用补丁
        log::info!("正在应用补丁到：{}", task.dst_file.display());
        bspatch(&task.dest_path, &actual_src, &task.dst_file)
            .await
            .context("应用补丁失败")?;

        // 清理补丁文件
        let _ = std::fs::remove_file(&task.dest_path);

        // 根据 keep_src 处理源文件（如果 src 和 dst 相同则已处理）
        if !src_is_dst {
            handle_src_after_patch(&actual_src, task.patch.keep_src)?;
        }

        current_file = Some(task.dst_file);
        log::info!("补丁应用成功：{}", current_file.as_ref().unwrap().display());
    }

    Ok(current_file)
}
