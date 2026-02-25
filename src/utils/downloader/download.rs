//! 下载核心逻辑模块
//! 实现单文件下载的完整流程

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use futures::{AsyncReadExt, StreamExt, TryStreamExt};
use indicatif::ProgressBar;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};

use super::client::build_request;
use super::client::create_http_client;
use super::hash_check;
use super::helpers::decompress_gz_sync;
use super::helpers::get_file_length;
use super::helpers::{support_download_range, url_get_range};
use super::progress::create_progress_bar_single;
use crate::utils::temp_dir;

/// 下载任务结构
///
/// `dest_path` 可以是：
/// - `Some(path)`: 直接下载到指定路径
/// - `None`: 从 HTTP 响应的 Content-Disposition 或 URL 中推断文件名，下载到临时目录
pub struct DownloadTask {
    pub url: String,
    pub dest_path: Option<PathBuf>,
    pub check_sha256: bool,
}

/// 内部下载函数，实现核心下载逻辑
///
/// # Arguments
///
/// * `url` - 下载链接的字符串引用
/// * `dest_path` - 目标路径的引用（如果为 None，则从 response 获取文件名并保存到临时目录）
/// * `check_sha256` - 是否检查 SHA256 校验和
/// * `progress_bar` - 可选的进度条
/// * `stop_if_cannot_check_integrity` - 如果无法检查完整性是否停止
///
/// # Returns
///
/// * `Result<(bool, PathBuf)>` - 成功时返回（是否进行了下载，实际文件路径），失败时返回错误
pub async fn download_file_internal(
    url: &str,
    dest_path: Option<&Path>,
    check_sha256: bool,
    progress_bar: Option<ProgressBar>,
    stop_if_cannot_check_integrity: bool,
) -> Result<(bool, PathBuf)> {
    log::debug!("开始下载：url={}, dest_path={:?}", url, dest_path);

    // 检查文件是否已存在且完整
    if check_sha256 {
        let dest_path = dest_path.context("检查完整性时需要指定目标路径")?;
        log::trace!("检查文件完整性：{:?}", dest_path);
        match hash_check::check_file_integrity(url, dest_path).await? {
            // 检查成功且文件完整
            Some(true) => {
                if let Some(pb) = progress_bar {
                    let prefix = extract_prefix_from_pb(&pb);
                    log::debug!("跳过：文件已是最新版本");
                    pb.finish_with_message(format!("{}✓ 已跳过", prefix));
                } else {
                    log::info!("跳过下载，文件已是最新版本：{}", dest_path.display());
                }
                return Ok((false, dest_path.to_path_buf()));
            }
            // 检查失败，不知道文件是否完整
            None if stop_if_cannot_check_integrity => {
                log::debug!("无法检查完整性，根据参数停止下载");
                return Ok((false, dest_path.to_path_buf()));
            }
            // 其他情况直接继续
            _ => {
                log::debug!("文件不完整或不存在，继续下载");
            }
        }
    }

    log::trace!("开始确定下载源和目标路径");
    let (response, dest_path_to_use, gz_compressed) = determine_gz_support(url, dest_path).await?;

    let result = if let Some(pb) = progress_bar {
        // 带进度条的下载
        log::trace!("使用进度条下载");
        download_with_progress(response, &dest_path_to_use, pb).await?
    } else {
        // 不带进度条的简单下载
        log::trace!("无进度条下载");
        download_without_progress(response, &dest_path_to_use).await?
    };

    // 如果下载的是压缩文件，则进行解压缩
    let final_path = if gz_compressed {
        log::info!("正在解压文件：{}", dest_path_to_use.display());
        let final_path = dest_path.unwrap_or(&dest_path_to_use).to_path_buf();
        decompress_gz_sync(&dest_path_to_use, &final_path)?;
        log::info!("已经解压到 {}", final_path.display());
        let _ = std::fs::remove_file(&dest_path_to_use); // 清理 gz 临时文件
        log::trace!("已清理临时 gz 文件");
        final_path
    } else {
        dest_path_to_use
    };

    log::debug!("下载完成：final_path={:?}", final_path);
    Ok((result, final_path))
}

/// 确定下载源和目标路径，处理 GZ 压缩文件的情况
///
/// 如果 `dest_path` 为 None，则从 response 中获取文件名并保存到临时目录。
async fn determine_gz_support(
    url: &str,
    dest_path: Option<&Path>,
) -> Result<(reqwest::Response, std::path::PathBuf, bool)> {
    let gz_url = format!("{}.gz", url);
    log::trace!("检查 GZ 版本：{}", gz_url);

    let client = create_http_client()?;
    let gz_response = build_request(client.get(&gz_url)).send().await?;

    match gz_response.error_for_status_ref() {
        Ok(_) => {
            // GZ 文件存在，使用压缩版本
            log::info!("检测到服务器有{}，正在下载……", gz_url);

            // 决定目标路径
            let dest_path_gz = if let Some(dp) = dest_path {
                // 有指定路径，保存到临时目录的 gz 文件
                let path = temp_dir()?
                    .join(dp.file_name().context("无法获取文件名！")?)
                    .with_added_extension("gz");
                log::trace!("使用指定路径的 gz 临时文件：{:?}", path);
                path
            } else {
                // 无指定路径，从 response 获取文件名
                let filename = super::helpers::get_filename_from_response(&gz_response)?;
                let path = temp_dir()?.join(&filename).with_added_extension("gz");
                log::trace!("从 response 获取文件名的 gz 临时文件：{:?}", path);
                path
            };

            Ok((gz_response, dest_path_gz, true))
        }
        Err(e) => {
            // GZ 文件不存在，使用原始 URL
            log::debug!("未找到 gz 压缩包：{}", gz_url);
            log::trace!("GZ 检查错误：{}", e);

            let client = create_http_client()?;
            log::trace!("发送请求到：{}", url);
            let response = build_request(client.get(url))
                .send()
                .await
                .context(format!("无法发送请求到：{}", url))?;

            // 决定目标路径
            let dest_path_buf = if let Some(dp) = dest_path {
                log::trace!("使用指定路径：{:?}", dp);
                dp.to_path_buf()
            } else {
                // 从 response 获取文件名
                let filename = super::helpers::get_filename_from_response(&response)?;
                let path = temp_dir()?.join(&filename);
                log::trace!("从 response 获取文件名：{:?}", path);
                path
            };

            Ok((response, dest_path_buf, false))
        }
    }
}

/// 单文件下载（带进度条）
pub async fn download_file(url: &str, dest_path: &Path, check_sha256: bool) -> Result<bool> {
    let pb = create_progress_bar_single();
    let (downloaded, _) =
        download_file_internal(url, Some(dest_path), check_sha256, Some(pb), false).await?;
    Ok(downloaded)
}

/// 下载补丁文件（从 response 获取文件名）
/// 返回实际下载的补丁文件路径
pub async fn download_patch_file_auto(url: &str) -> Result<PathBuf> {
    let (_, path) = download_file_internal(url, None, false, None, false).await?;
    Ok(path)
}

/// 带进度条的下载逻辑
async fn download_with_progress(
    response: reqwest::Response,
    dest_path: &Path,
    pb: ProgressBar,
) -> Result<bool> {
    let total_size = get_file_length(&response)?;
    log::trace!("下载总大小：{} bytes", total_size);
    pb.set_length(total_size);

    log::trace!("创建目标文件：{:?}", dest_path);
    let mut dest_file = create_destination_file(dest_path).await?;
    let (tx, updater_handle) = setup_progress_update_task(&pb, dest_path, &response).await?;
    log::trace!("开始带重试的下载");
    download_with_retries(response, total_size, &mut dest_file, &tx).await?;
    log::trace!("下载完成，清理资源");
    finalize_download(&mut dest_file, tx, updater_handle).await?;

    Ok(true)
}

/// 不带进度条的下载逻辑
async fn download_without_progress(response: reqwest::Response, dest_path: &Path) -> Result<bool> {
    log::trace!("创建目标文件（无进度条）：{:?}", dest_path);
    let mut dest_file = tokio::fs::File::create(dest_path)
        .await
        .with_context(|| format!("无法创建目标文件：{:?}", dest_path))?;

    let mut stream = response.bytes_stream();
    let mut total_bytes: u64 = 0;

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.context("无法读取数据块")?;
        total_bytes += chunk.len() as u64;
        dest_file.write_all(&chunk).await.context("无法写入文件")?;
    }

    log::debug!(
        "下载完成：{}, 总字节数：{}",
        dest_path.display(),
        total_bytes
    );
    Ok(true)
}

/// 创建目标文件
async fn create_destination_file(dest_path: &Path) -> Result<tokio::fs::File> {
    log::trace!("创建文件：{:?}", dest_path);
    tokio::fs::File::create(dest_path)
        .await
        .with_context(|| format!("无法创建目标文件：{:?}", dest_path))
}

/// 设置进度更新任务
async fn setup_progress_update_task(
    pb: &ProgressBar,
    dest_path: &Path,
    response: &reqwest::Response,
) -> Result<(tokio::sync::mpsc::Sender<u64>, tokio::task::JoinHandle<()>)> {
    use super::progress::spawn_progress_updater;
    use crate::utils::get_filename;

    let total_size = get_file_length(response)?;
    log::trace!("设置进度更新：total_size={} bytes", total_size);
    let (tx, rx) = tokio::sync::mpsc::channel::<u64>(128);

    let pb_updater = pb.clone();
    let full_name_updater = get_filename(dest_path)?;
    let prefix_updater = extract_prefix_from_pb(pb);

    log::trace!("启动进度更新任务：file={}", full_name_updater);
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
async fn download_with_progress_updates(
    dest_file: &mut tokio::fs::File,
    response: reqwest::Response,
    tx: &tokio::sync::mpsc::Sender<u64>,
) -> Result<()> {
    let mut response_body = response
        .bytes_stream()
        .map_err(|e| std::io::Error::other(format!("reqwest error: {}", e)))
        .into_async_read();

    let mut buffer = [0u8; 1024];
    let mut downloaded: u64 = dest_file.stream_position().await?;

    loop {
        match response_body.read(&mut buffer).await {
            Ok(n) if n > 0 => {
                dest_file
                    .write_all(&buffer[..n])
                    .await
                    .context("写入文件失败")?;

                downloaded += n as u64;
                if tx.send(downloaded).await.is_err() {
                    break;
                }
            }
            Ok(0) => break,
            Err(e) if e.kind() != std::io::ErrorKind::Interrupted => {
                return Err(e).context("读取响应体失败");
            }
            _ => continue,
        }
    }

    Ok(())
}

/// 带重试的下载
async fn download_with_retries(
    response: reqwest::Response,
    total_size: u64,
    dest_file: &mut tokio::fs::File,
    tx: &tokio::sync::mpsc::Sender<u64>,
) -> Result<(), anyhow::Error> {
    use crate::global_config::get_global_config;

    let retry_max = get_global_config().network.retry;
    let url = response.url().to_string();
    let range_type = support_download_range(&response)?;

    log::trace!(
        "开始下载：url={}, total_size={}, retry_max={}",
        url,
        total_size,
        retry_max
    );
    let mut value = download_with_progress_updates(dest_file, response, tx).await;
    let mut i = 0;

    let _: () = while let Err(e) = value {
        log::error!("{e}");
        if i >= retry_max {
            log::error!("用尽重试次数 ({} 次)，下载失败", retry_max);
            anyhow::bail!("用尽重试次数，下载失败！");
        }

        i += 1;
        log::warn!("下载错误，第 {i} 次重试");

        dest_file.flush().await?;
        let cur_length = dest_file.stream_position().await?;
        log::debug!("当前已下载 {} bytes，准备断点续传", cur_length);

        let client = create_http_client()?;
        let response = if let Some(ref rt) = range_type {
            log::trace!("使用 Range 请求：{}={}-{}", rt, cur_length, total_size);
            url_get_range(&url, rt, cur_length, total_size).await?
        } else {
            log::trace!("服务器不支持 Range，重新从头下载");
            build_request(client.get(&url)).send().await?
        };

        value = download_with_progress_updates(dest_file, response, tx).await;
    };
    log::trace!("下载成功完成");
    Ok(())
}

/// 完成下载后清理资源
async fn finalize_download(
    dest_file: &mut tokio::fs::File,
    tx: tokio::sync::mpsc::Sender<u64>,
    updater_handle: tokio::task::JoinHandle<()>,
) -> Result<()> {
    dest_file.flush().await.context("刷新文件缓冲区失败")?;
    dest_file.sync_data().await?;
    drop(tx);
    let _ = updater_handle.await;
    Ok(())
}

/// 从进度条提取前缀
fn extract_prefix_from_pb(pb: &ProgressBar) -> String {
    let original_msg = pb.message().to_string();
    original_msg
        .split_once("] ")
        .map(|(prefix, _)| format!("{}] ", prefix))
        .unwrap_or_default()
}
