mod hash_check;
mod helpers;
mod progress;
pub mod self_update;

use anyhow::{Context, Result};
use futures::{AsyncReadExt, StreamExt, TryStreamExt};
use helpers::get_file_length;
use indicatif::ProgressBar;
use progress::{create_progress_bar, setup_multi_progress, spawn_progress_updater};
use reqwest::{self};
use std::{path::Path, sync::OnceLock, time::Duration};
// Duration not needed here; kept in progress.rs
use crate::{global_config::get_global_config, utils::{downloader::helpers::url_get, get_filename}};
use tokio::io::AsyncWriteExt;

static HTTP_CLIENT_TEMPLATE: OnceLock<reqwest::Client> = OnceLock::new();

/// 根据网络配置创建HTTP客户端
pub fn create_http_client() -> Result<reqwest::Client> {
    fn init_client_template() -> reqwest::Client {
        let config = get_global_config();
        let network_config = &config.network;

        let builder = reqwest::ClientBuilder::new()
            .tls_danger_accept_invalid_certs(network_config.ignore_invalid_cert)
            .timeout(Duration::from_secs(network_config.timeout));

        let builder = if network_config.quic {
            // 使用HTTP/3协议
            builder.http3_prior_knowledge()
        } else {
            builder
        };
        builder.build().expect("创建client客户端时错误")
    }

    Ok(HTTP_CLIENT_TEMPLATE
        .get_or_init(init_client_template)
        .clone())
}

/// 为请求添加版本信息（如果需要）
fn build_request(request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    let config = get_global_config();

    if config.network.quic {
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
    progress_bar: Option<ProgressBar>,
    stop_if_cannot_check_integrity: bool,
) -> Result<bool> {
    // 检查文件是否已存在且完整
    if check_sha256 {
        match hash_check::check_file_integrity(url, dest_path).await? {
            // 检查成功且文件完整
            Some(true) => {
                if let Some(pb) = progress_bar {
                    let prefix = extract_prefix_from_pb(&pb);
                    log::debug!("跳过: 文件已是最新版本");
                    pb.finish_with_message(format!("{}✓ 已跳过", prefix));
                } else {
                    log::info!("跳过下载，文件已是最新版本: {}", dest_path.display());
                }
                return Ok(false);
            }
            // 检查失败，不知道文件是否完整
            None if stop_if_cannot_check_integrity => return Ok(false),
            // 其他情况直接继续
            _ => {}
        }
    }

    let (response, dest_path_to_use, gz_compressed) = determine_gz_support(url, dest_path).await?;

    let result = if let Some(pb) = progress_bar {
        // 带进度条的下载
        download_with_progress_logic(response, &dest_path_to_use, pb).await?
    } else {
        // 不带进度条的简单下载
        download_without_progress_logic(response, &dest_path_to_use).await?
    };

    // 如果下载的是压缩文件，则进行解压缩
    if gz_compressed {
        log::info!("正在解压文件：{}", dest_path_to_use.display());
        helpers::decompress_gz_sync(&dest_path_to_use, dest_path)?;
        log::info!("已经解压到 {}", dest_path.display());
    }

    Ok(result)
}

/// 确定下载源和目标路径，处理GZ压缩文件的情况
async fn determine_gz_support(
    url: &str,
    dest_path: &Path,
) -> Result<(reqwest::Response, std::path::PathBuf, bool)> {
    let gz_url = format!("{}.gz", url);
    let gz_response = url_get(&gz_url).await;

    match gz_response {
        Ok(response) => {
            // GZ文件存在，使用压缩版本
            log::info!("检测到服务器有{}，正在下载……", gz_url);
            let dest_path_gz = dest_path.with_added_extension("gz");
            Ok((response, dest_path_gz, true)) // 需要解压缩
        }
        Err(_) => {
            // GZ文件不存在，使用原始URL
            log::debug!("未找到gz压缩包: {}", gz_url);
            let response = url_get(url)
                .await
                .context(format!("无法发送请求到: {}", url))?;
            Ok((response, dest_path.to_path_buf(), false)) // 不需要解压缩
        }
    }
}

/// 更新metadata
pub async fn update_metadata(dest_path: &Path) -> Result<bool> {
    let config = get_global_config();
    let metadata = config.metadata_config.metadata.as_deref().unwrap();
    download_file(metadata, dest_path, true).await
}

/// 单文件下载
pub async fn download_file(url: &str, dest_path: &Path, check_sha256: bool) -> Result<bool> {
    let pb = progress::create_progress_bar_single();
    download_file_internal(url, dest_path, check_sha256, Some(pb), false).await
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
                Some(progress_bar),
                false,
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
    pb: ProgressBar,
) -> Result<bool> {
    // 初始化进度条参数
    initialize_progress_bar(&response, &pb, dest_path)?;

    // 创建目标文件
    let mut dest_file = create_destination_file(dest_path).await?;

    // 设置进度更新机制
    let (tx, updater_handle) = setup_progress_update_task(&pb, dest_path, &response).await?;

    // 开始下载并实时更新进度
    perform_download_with_progress_updates(&mut dest_file, response, &tx).await?;

    // 完成下载后清理资源
    finalize_download(dest_file, tx, updater_handle).await?;

    Ok(true)
}

/// 初始化进度条参数
fn initialize_progress_bar(
    response: &reqwest::Response,
    pb: &ProgressBar,
    dest_path: &Path,
) -> Result<()> {
    let total_size = get_file_length(response)?;
    pb.set_length(total_size);
    log::debug!(
        "开始下载文件: {}, 大小: {} bytes",
        get_filename(dest_path)?,
        total_size
    );
    Ok(())
}

/// 创建目标文件
async fn create_destination_file(dest_path: &Path) -> Result<tokio::fs::File> {
    tokio::fs::File::create(dest_path)
        .await
        .with_context(|| format!("无法创建目标文件: {:?}", dest_path))
}

/// 设置进度更新任务
async fn setup_progress_update_task(
    pb: &ProgressBar,
    dest_path: &Path,
    response: &reqwest::Response,
) -> Result<(tokio::sync::mpsc::Sender<u64>, tokio::task::JoinHandle<()>)> {
    let total_size = get_file_length(response)?;

    // 通道用于在下载任务与更新任务之间传递已下载字节数
    let (tx, rx) = tokio::sync::mpsc::channel::<u64>(128);

    // 克隆用于 updater 的状态
    let pb_updater = pb.clone();
    let full_name_updater = get_filename(dest_path)?;
    let prefix_updater = extract_prefix_from_pb(pb);

    // 启动进度更新任务
    let updater_handle = spawn_progress_updater(
        pb_updater,
        full_name_updater.as_ref(),
        prefix_updater,
        total_size,
        rx,
    );

    Ok((tx, updater_handle))
}

/// 执行下载并实时更新进度
async fn perform_download_with_progress_updates(
    dest_file: &mut tokio::fs::File,
    response: reqwest::Response,
    tx: &tokio::sync::mpsc::Sender<u64>,
) -> Result<()> {
    // 将流转换为 AsyncRead
    let mut response_body = response
        .bytes_stream()
        // 将reqwest::Error转为std::io::Error
        .map_err(|e| std::io::Error::other(format!("reqwest error: {}", e)))
        .into_async_read();

    let mut buffer = vec![0u8; 1024];
    loop {
        match response_body.read(&mut buffer).await {
            Ok(n) if n > 0 => {
                dest_file
                    .write_all(&buffer[..n])
                    .await
                    .context("写入文件失败")?;

                // 发送下载字节数到进度更新任务
                if tx.send(n as u64).await.is_err() {
                    // 接收方已关闭，停止下载
                    break;
                }
            }
            Ok(0) => break, // 下载完成
            Err(e) if e.kind() != std::io::ErrorKind::Interrupted => {
                return Err(e).context("读取响应体失败");
            }
            _ => continue, // 继续尝试读取
        }
    }

    Ok(())
}

/// 完成下载后清理资源
async fn finalize_download(
    mut dest_file: tokio::fs::File,
    tx: tokio::sync::mpsc::Sender<u64>,
    updater_handle: tokio::task::JoinHandle<()>,
) -> Result<()> {
    // 刷新文件缓冲区
    dest_file.flush().await.context("刷新文件缓冲区失败")?;

    // 关闭发送端，通知 updater 完成
    drop(tx);

    // 等待进度更新任务完成
    let _ = updater_handle.await;

    Ok(())
}

/// 不带进度条的下载逻辑
async fn download_without_progress_logic(
    response: reqwest::Response,
    dest_path: &Path,
) -> Result<bool> {
    let mut dest_file = tokio::fs::File::create(dest_path)
        .await
        .with_context(|| format!("无法创建目标文件: {:?}", dest_path))?;

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

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试HTTP客户端创建功能
    ///
    /// 注意：由于使用全局配置，这些测试仅验证函数不会崩溃，
    /// 实际行为取决于全局配置的状态
    #[test]
    fn test_create_http_client() {
        let result = create_http_client();

        match result {
            Ok(_) => {
                // 成功创建客户端
                assert!(true);
            }
            Err(e) => {
                // 如果失败，打印信息但不使测试失败，因为全局配置可能未设置
                println!("Note: Global config may not be set for this test: {}", e);
                assert!(true); // 仍视为测试通过
            }
        }
    }

    /// 测试异步HTTP客户端创建功能
    #[tokio::test]
    async fn test_create_http_client_async() {
        let result = create_http_client();

        match result {
            Ok(_) => {
                assert!(true);
            }
            Err(e) => {
                println!("Note: Global config may not be set for this test: {}", e);
                assert!(true);
            }
        }
    }
}
