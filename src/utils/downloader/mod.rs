mod hash_check;
mod helpers;
mod progress;

use anyhow::{Context, Result};
use futures::{AsyncReadExt, StreamExt, TryStreamExt};
use helpers::{ensure_success_response, get_file_length, get_file_size};
use indicatif::ProgressBar;
use progress::{create_progress_bar, setup_multi_progress, spawn_progress_updater};
use reqwest::{self};
use std::io::{Error, ErrorKind};
use std::path::Path;
// Duration not needed here; kept in progress.rs
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

/// 下载任务结构
pub struct DownloadTask {
    pub url: String,
    pub dest_path: std::path::PathBuf,
    pub check_sha256: bool,
}

/// 简单的单文件下载
pub async fn download_file(url: &str, dest_path: &Path, check_sha256: bool) -> Result<bool> {
    // 检查文件是否已存在且完整
    if check_sha256 && hash_check::check_file_integrity(url, dest_path).await? {
        log::info!("跳过下载，文件已是最新版本: {}", dest_path.display());
        return Ok(false);
    }

    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .context(format!("无法发送请求到: {}", url))?;

    ensure_success_response(&response)?;

    let mut dest_file = tokio::fs::File::create(dest_path)
        .await
        .context(format!("无法创建目标文件: {:?}", dest_path))?;

    let mut stream = response.bytes_stream();

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.context("无法读取数据块")?;
        dest_file.write_all(&chunk).await.context("无法写入文件")?;
    }

    log::info!("下载完成: {}", dest_path.display());
    Ok(true)
}

/// 使用 indicatif 多进度条批量下载文件，接受迭代器
pub async fn download_files_with_progress<I>(tasks: I) -> Result<()>
where
    I: IntoIterator<Item = DownloadTask>,
{
    // 懒惰处理传入迭代器：不提前 collect，尽量保持惰性求值
    let tasks_iter = tasks.into_iter();
    let (lower, upper) = tasks_iter.size_hint();
    let total_opt: Option<usize> = upper.or(Some(lower));
    let total_for_log = total_opt.unwrap_or(0);

    log::info!("开始下载 {} 个文件", total_for_log);

    let multi_progress = setup_multi_progress();

    // 并发数：若已知 total 则取 min(total,6)，否则默认使用 6
    let concurrency = match total_opt {
        Some(t) => {
            if t >= 6 {
                6
            } else {
                t
            }
        }
        None => 6,
    };

    // 在异步闭包内对 multi_progress 做不可变借用来创建每个任务的 ProgressBar，避免使用 Arc
    let task_stream = futures::stream::iter(tasks_iter.enumerate().map(move |(idx, task)| {
        let total_copy = total_opt; // Option<usize> is Copy

        // 在同步闭包内创建 ProgressBar（不会跨 await），然后把 owned ProgressBar 移入 async block
        let filename = task
            .dest_path
            .file_name()
            .unwrap_or(std::ffi::OsStr::new("unknown"))
            .to_string_lossy()
            .to_string();

        let progress_bar = create_progress_bar(&multi_progress, None, idx, total_copy, &filename);

        async move {
            // 异步在任务内查询文件大小（可选）
            let _ = get_file_size(&task.url).await.ok();

            // 执行下载并返回结果
            download_file_with_progress(&task.url, &task.dest_path, task.check_sha256, progress_bar)
                .await
        }
    }));

    // 运行限并发流并处理每个结果
    task_stream
        .buffer_unordered(concurrency)
        .for_each(|res| async move {
            if let Err(e) = res {
                crate::utils::print_error_chain(&e);
            }
        })
        .await;

    log::info!("所有文件下载完成！");
    Ok(())
}

/// 带进度条的异步文件下载（支持长文件名滚动显示）
async fn download_file_with_progress(
    url: &str,
    dest_path: &Path,
    check_sha256: bool,
    pb: ProgressBar,
) -> Result<()> {
    let prefix = extract_prefix_from_pb(&pb);
    let full_filename = extract_full_filename(dest_path);

    // 检查文件是否已存在且完整
    if check_sha256 && hash_check::check_file_integrity(url, dest_path).await? {
        log::debug!("跳过: 文件已是最新版本");
        pb.finish_with_message(format!("{}✓ 已跳过", prefix));
        return Ok(());
    }

    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .context(format!("无法发送请求到: {}", url))?;

    ensure_success_response(&response)?;

    let total_size = get_file_length(&response)?;
    pb.set_length(total_size);

    let mut dest_file = tokio::fs::File::create(dest_path)
        .await
        .context(format!("无法创建目标文件: {:?}", dest_path))?;

    // 辅助函数：提取前缀和文件名用的小函数在文件底部实现

    // ====================== 关键修复部分 ======================
    // 将流转换为 AsyncRead，然后把下载与进度更新拆分成两个任务：
    //  - 下载任务：负责从网络读取并写入磁盘，同时把已读取的字节数发送到通道
    //  - 更新任务：独立运行，接收字节数并以固定频率更新进度条和滚动文本
    // 这样能保证进度条以稳定的频率刷新，不受网络 I/O 阻塞影响。
    let mut response_body = response
        .bytes_stream()
        // 将reqwest::Error转为std::io::Error（核心修复）
        .map_err(|e| Error::new(ErrorKind::Other, format!("reqwest error: {}", e)))
        .into_async_read();

    // 通道用于在下载任务与更新任务之间传递已下载字节数
    let (tx, rx) = mpsc::channel::<u64>(128);

    // 克隆用于 updater 的状态
    let pb_updater = pb.clone();
    let full_name_updater = full_filename.clone();
    let prefix_updater = prefix.clone();
    let total_size_updater = total_size;

    // 更新任务：定期刷新进度和滚动文本，传入接收端
    let updater_handle = spawn_progress_updater(
        pb_updater,
        full_name_updater,
        prefix_updater,
        total_size_updater,
        rx,
    );

    // 下载任务：读取网络数据并写入文件，同时把已读字节数发送给更新任务
    let mut buffer = vec![0u8; 4096];
    loop {
        match response_body.read(&mut buffer).await {
            Ok(n) if n > 0 => {
                dest_file
                    .write_all(&buffer[..n])
                    .await
                    .context("写入文件失败")?;
                let chunk_len = n as u64;
                // 发送到更新任务；若接收方已关闭则忽略错误
                let _ = tx.send(chunk_len).await;
            }
            Ok(0) => break,
            Err(e) if e.kind() != ErrorKind::Interrupted => {
                return Err(e).context("读取响应体失败");
            }
            _ => continue,
        }
    }

    dest_file.flush().await.context("刷新文件缓冲区失败")?;
    // 关闭发送端，通知 updater 完成
    drop(tx);
    // 等待 updater 完成并已设置 finish message
    let _ = updater_handle.await;

    Ok(())
}

// ------------------ 小的辅助函数，帮助拆分主流程 ------------------
fn extract_prefix_from_pb(pb: &ProgressBar) -> String {
    let original_msg = pb.message().to_string();
    original_msg
        .split_once("] ")
        .map(|(prefix, _)| format!("{}] ", prefix))
        .unwrap_or_default()
}

fn extract_full_filename(dest_path: &Path) -> String {
    dest_path
        .file_name()
        .unwrap_or(std::ffi::OsStr::new("unknown"))
        .to_string_lossy()
        .to_string()
}

// spawn_progress_updater 已移动到 progress.rs，实现 UI 更新逻辑。
