//! 批量下载模块
//! 实现多文件并发下载

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

    log::info!("开始下载 {} 个文件", total_for_log);

    let multi_progress = setup_multi_progress();

    // 并发数：若已知 total 则取 min(total,6)，否则默认使用 6
    let concurrency = match total_opt {
        Some(t) => t.min(6),
        None => 6,
    };

    let task_stream = futures::stream::iter(tasks_iter.enumerate().map(move |(idx, task)| {
        let total_copy = total_opt;

        let filename = task
            .dest_path
            .file_name()
            .unwrap_or(std::ffi::OsStr::new("unknown"))
            .to_string_lossy()
            .to_string();

        let progress_bar = create_progress_bar(&multi_progress, None, idx, total_copy, &filename);

        async move {
            download_file_internal(
                &task.url,
                &task.dest_path,
                task.check_sha256,
                Some(progress_bar),
                false,
            )
            .await?;
            Ok(())
        }
    }));

    task_stream
        .buffer_unordered(concurrency)
        .for_each(|res| async move {
            if let Err(e) = res {
                print_error_chain(&e);
            }
        })
        .await;

    log::info!("所有文件下载完成！");
    Ok(())
}
