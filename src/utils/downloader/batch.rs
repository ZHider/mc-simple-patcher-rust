//! 批量下载模块
//! 实现多文件并发下载

use std::path::PathBuf;

use anyhow::Result;
use futures::stream::StreamExt;

use super::download::{DownloadTask, download_file_internal};
use super::progress::{create_progress_bar, setup_multi_progress};
use crate::utils::print_error_chain;

/// 使用 indicatif 多进度条批量下载文件
///
/// # Arguments
///
/// * `tasks` - 下载任务的迭代器
///
/// # Returns
///
/// * `Result<()>` - 成功时返回空值，失败时返回错误
pub async fn download_files_with_progress<I>(tasks: I) -> Result<()>
where
    I: IntoIterator<Item = DownloadTask>,
{
    let tasks_iter = tasks.into_iter();
    let (lower, upper) = tasks_iter.size_hint();
    let total_opt: Option<usize> = upper.or(Some(lower));
    let total_for_log = total_opt.unwrap_or(0);

    log::info!(
        "开始下载 {} 个文件，并发数：{}",
        total_for_log,
        total_opt.unwrap_or(6).min(6)
    );

    let multi_progress = setup_multi_progress();

    // 并发数：若已知 total 则取 min(total,6)，否则默认使用 6
    let concurrency = match total_opt {
        Some(t) => t.min(6),
        None => 6,
    };

    log::debug!("创建下载任务流，concurrency={}", concurrency);
    let task_stream = futures::stream::iter(tasks_iter.enumerate().map(move |(idx, task)| {
        let total_copy = total_opt;
        let multi_progress = multi_progress.clone();
        let url = task.url.clone();

        async move {
            // 先创建进度条（使用 URL 或 dest_path 的文件名作为占位符）
            let filename = if let Some(ref dp) = task.dest_path {
                dp.file_name()
                    .unwrap_or(std::ffi::OsStr::new("unknown"))
                    .to_string_lossy()
                    .to_string()
            } else {
                // 从 URL 提取临时文件名
                PathBuf::from(&task.url)
                    .file_name()
                    .unwrap_or(std::ffi::OsStr::new("unknown"))
                    .to_string_lossy()
                    .to_string()
            };

            log::trace!(
                "开始下载任务 [{}/{}]: {}",
                idx + 1,
                total_copy.unwrap_or(0),
                filename
            );
            let progress_bar =
                create_progress_bar(&multi_progress, None, idx, total_copy, &filename);

            // 执行下载
            let (_, actual_path) = download_file_internal(
                &task.url,
                task.dest_path.as_deref(),
                task.check_sha256,
                Some(progress_bar),
                false,
            )
            .await?;

            log::debug!("下载完成 [{}]: {}", url, actual_path.display());
            Ok(())
        }
    }));

    log::trace!("开始执行并发下载流");
    task_stream
        .buffer_unordered(concurrency)
        .for_each(|res| async move {
            if let Err(e) = res {
                log::debug!("下载任务出错：{}", e);
                print_error_chain(&e);
            }
        })
        .await;

    log::info!("所有文件下载完成！");
    Ok(())
}
