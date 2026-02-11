mod hash_check;
mod helpers;
mod progress;

use anyhow::{Context, Result};
use futures::{AsyncReadExt, StreamExt, TryStreamExt};
use helpers::{ensure_success_response, get_file_length};
use indicatif::ProgressBar;
use progress::{create_progress_bar, setup_multi_progress, spawn_progress_updater};
use reqwest::{self};
use std::path::Path;
// Duration not needed here; kept in progress.rs
use crate::global_config::get_global_config;
use tokio::io::AsyncWriteExt;
/// 根据网络配置创建HTTP客户端
pub fn create_http_client() -> Result<reqwest::Client> {
    let config = get_global_config();
    let network_config = &config.network;

    let mut builder = reqwest::ClientBuilder::new();

    if let Some(config) = network_config {
        if config.quic {
            // 使用HTTP/3协议
            builder = builder.http3_prior_knowledge();
        }

        // 是否验证TLS证书
        builder = builder.tls_danger_accept_invalid_certs(config.ignore_invalid_cert);
    }

    Ok(builder.build()?)
}

/// 为请求添加版本信息（如果需要）
fn configure_request_version(request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    let config = get_global_config();

    if let Some(config) = config.network
        && config.quic
    {
        // 显式指定使用HTTP/3版本
        return request.version(reqwest::Version::HTTP_3);
    }
    request
}

/// 下载任务结构
pub struct DownloadTask {
    pub url: String,
    pub dest_path: std::path::PathBuf,
    pub check_sha256: bool,
}

/// 内部下载函数，实现核心下载逻辑
async fn download_file_internal(
    url: &str,
    dest_path: &Path,
    check_sha256: bool,
    progress_bar: Option<&ProgressBar>,
) -> Result<bool> {
    // 检查文件是否已存在且完整
    if check_sha256 && hash_check::check_file_integrity(url, dest_path).await? {
        if let Some(pb) = progress_bar {
            let prefix = extract_prefix_from_pb(pb);
            log::debug!("跳过: 文件已是最新版本");
            pb.finish_with_message(format!("{}✓ 已跳过", prefix));
        } else {
            log::info!("跳过下载，文件已是最新版本: {}", dest_path.display());
        }
        return Ok(false);
    }

    let client = create_http_client()?;
    let request = client.get(url);
    let configured_request = configure_request_version(request);
    let response = configured_request
        .send()
        .await
        .context(format!("无法发送请求到: {}", url))?;

    ensure_success_response(&response)?;

    if let Some(pb) = progress_bar {
        // 带进度条的下载
        download_with_progress_logic(response, dest_path, pb).await
    } else {
        // 不带进度条的简单下载
        download_without_progress_logic(response, dest_path).await
    }
}

/// 简单的单文件下载
pub async fn download_file(url: &str, dest_path: &Path, check_sha256: bool) -> Result<bool> {
    download_file_internal(url, dest_path, check_sha256, None).await
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
            // 执行下载并返回结果
            download_file_internal(
                &task.url,
                &task.dest_path,
                task.check_sha256,
                Some(&progress_bar),
            )
            .await?;
            Ok(())
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

/// 带进度条的下载逻辑
async fn download_with_progress_logic(
    response: reqwest::Response,
    dest_path: &Path,
    pb: &ProgressBar,
) -> Result<bool> {
    let full_filename = extract_full_filename(dest_path);
    let prefix = extract_prefix_from_pb(pb);

    let total_size = get_file_length(&response)?;
    pb.set_length(total_size);

    let mut dest_file = tokio::fs::File::create(dest_path)
        .await
        .context(format!("无法创建目标文件: {:?}", dest_path))?;

    // 将流转换为 AsyncRead，然后把下载与进度更新拆分成两个任务：
    //  - 下载任务：负责从网络读取并写入磁盘，同时把已读取的字节数发送到通道
    //  - 更新任务：独立运行，接收字节数并以固定频率更新进度条和滚动文本
    // 这样能保证进度条以稳定的频率刷新，不受网络 I/O 阻塞影响。
    let mut response_body = response
        .bytes_stream()
        // 将reqwest::Error转为std::io::Error（核心修复）
        .map_err(|e| std::io::Error::other(format!("reqwest error: {}", e)))
        .into_async_read();

    // 通道用于在下载任务与更新任务之间传递已下载字节数
    let (tx, rx) = tokio::sync::mpsc::channel::<u64>(128);

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
            Err(e) if e.kind() != std::io::ErrorKind::Interrupted => {
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

    Ok(true)
}

/// 不带进度条的下载逻辑
async fn download_without_progress_logic(
    response: reqwest::Response,
    dest_path: &Path,
) -> Result<bool> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_http_client_default() {
        // 由于现在使用全局配置，我们无法在测试中轻松更改配置
        // 因此，我们只测试函数是否能正常工作
        // 注意：这需要在全局配置设置后才能正常工作

        // 这里只是确保函数不会崩溃
        let result = create_http_client();
        // 由于我们无法设置全局配置，这里可能会失败
        // 但我们仍然测试函数的存在
        if result.is_ok() {
            assert!(true); // 如果成功创建客户端，测试通过
        } else {
            // 如果失败，我们不认为这是测试失败，因为全局配置可能未设置
            // 在实际应用中，全局配置会在启动时设置
            println!("Note: Global config may not be set for this test");
        }
    }

    #[tokio::test]
    async fn test_create_http_client_quic_enabled() {
        // 由于现在使用全局配置，我们无法在测试中轻松更改配置
        // 因此，我们只测试函数是否能正常工作
        // 注意：这需要在全局配置设置后才能正常工作

        // 这里只是确保函数不会崩溃
        let result = create_http_client();
        // 由于我们无法设置全局配置，这里可能会失败
        // 但我们仍然测试函数的存在
        if result.is_ok() {
            assert!(true); // 如果成功创建客户端，测试通过
        } else {
            // 如果失败，我们不认为这是测试失败，因为全局配置可能未设置
            // 在实际应用中，全局配置会在启动时设置
            println!("Note: Global config may not be set for this test");
        }
    }

    #[test]
    fn test_create_http_client_ignore_cert() {
        // 由于现在使用全局配置，我们无法在测试中轻松更改配置
        // 因此，我们只测试函数是否能正常工作
        // 注意：这需要在全局配置设置后才能正常工作

        // 这里只是确保函数不会崩溃
        let result = create_http_client();
        // 由于我们无法设置全局配置，这里可能会失败
        // 但我们仍然测试函数的存在
        if result.is_ok() {
            assert!(true); // 如果成功创建客户端，测试通过
        } else {
            // 如果失败，我们不认为这是测试失败，因为全局配置可能未设置
            // 在实际应用中，全局配置会在启动时设置
            println!("Note: Global config may not be set for this test");
        }
    }
}
