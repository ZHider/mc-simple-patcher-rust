mod hash_check;

use anyhow::{Context, Result};
use futures::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::{self, Response};
use std::path::Path;
use tokio::io::AsyncWriteExt;

/// 下载任务结构
pub struct DownloadTask {
    pub url: String,
    pub dest_path: std::path::PathBuf,
    pub check_sha256: bool,
}

pub fn init_tokio_runtime() {
    // indicatif 不需要全局运行时，这个函数保留以保持兼容性
}

/// 简单的单文件下载
pub async fn download_file(url: &str, dest_path: &Path, check_sha256: bool) -> Result<()> {
    // 检查文件是否已存在且完整
    if check_sha256 && hash_check::check_file_integrity(url, dest_path).await? {
        log::info!("跳过下载，文件已是最新版本: {}", dest_path.display());
        return Ok(());
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
    Ok(())
}

/// 使用 indicatif 多进度条批量下载文件，接受迭代器
pub async fn download_files_with_progress<I>(tasks: I) -> Result<()>
where
    I: IntoIterator<Item = DownloadTask>,
{
    let tasks_vec: Vec<_> = tasks.into_iter().collect();
    let total = tasks_vec.len();

    log::info!("开始下载 {} 个文件", total);

    // 创建多进度条容器
    let multi_progress = MultiProgress::new();
    
    // 设置多进度条的样式
    multi_progress.set_draw_target(indicatif::ProgressDrawTarget::stderr());

    // 为每个任务创建进度条
    let mut handles = Vec::new();

    for (idx, task) in tasks_vec.into_iter().enumerate() {
        let filename = task
            .dest_path
            .file_name()
            .unwrap_or(std::ffi::OsStr::new("unknown"))
            .to_string_lossy()
            .to_string();

        // 创建进度条
        let pb = multi_progress.add(ProgressBar::new_spinner());
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("[{pos}/{len}] {spinner} {msg} {bytes}/{total_bytes} ({eta})")
                .unwrap()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
        );
        
        let desc = format!("[{}/{}] {}", idx + 1, total, filename);
        pb.set_message(desc);

        // 获取文件大小
        let file_size = get_file_size(&task.url).await.ok();
        if let Some(size) = file_size {
            pb.set_length(size);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("[{pos}/{len}] {bar:40.cyan/blue} {bytes}/{total_bytes} ({eta})")
                    .unwrap()
                    .progress_chars("=>-"),
            );
        }

        // 创建异步下载任务
        let download_task = task;
        let progress_bar = pb;

        let handle = tokio::spawn(async move {
            download_file_with_progress(&download_task.url, &download_task.dest_path, download_task.check_sha256, progress_bar).await
        });

        handles.push(handle);
    }

    // 等待所有下载完成
    for handle in handles {
        match handle.await {
            Ok(Ok(_)) => {},
            Ok(Err(e)) => {
                crate::utils::print_error_chain(&e);
            }
            Err(e) => {
                log::error!("下载任务被取消: {}", e);
            }
        }
    }

    log::info!("所有文件下载完成！");
    Ok(())
}


/// 带进度条的异步文件下载
async fn download_file_with_progress(
    url: &str,
    dest_path: &Path,
    check_sha256: bool,
    pb: ProgressBar,
) -> Result<()> {
    // 检查文件是否已存在且完整
    if check_sha256 && hash_check::check_file_integrity(url, dest_path).await? {
        log::debug!("跳过: 文件已是最新版本");
        pb.finish_with_message("已跳过，文件最新 ✓");
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

    let mut stream = response.bytes_stream();

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.context("无法读取数据块")?;
        dest_file.write_all(&chunk).await.context("无法写入文件")?;
        pb.inc(chunk.len() as u64);
    }

    pb.finish_with_message("完成 ✓");
    Ok(())
}

/// 获取远程文件大小
fn get_file_length(resp: &Response) -> Result<u64> {
    if let Some(file_length) = resp.headers().get("Content-Length") {
        let fl_str = file_length
            .to_str()
            .context(format!("HTTP头读取时解码失败: {:?}", file_length))?;
        let fl_u64: u64 = fl_str
            .parse()
            .context(format!("Content-Length: {} 似乎不是有效的整数？", fl_str))?;
        Ok(fl_u64)
    } else {
        anyhow::bail!(
            "未能找到 HTTP 头 Content-Length 来获得文件大小在 {}",
            resp.url()
        )
    }
}

/// 获取远程文件大小（不打开完整响应）
async fn get_file_size(url: &str) -> Result<u64> {
    let client = reqwest::Client::new();
    let response = client
        .head(url)
        .send()
        .await
        .context(format!("无法发送HEAD请求到: {}", url))?;

    get_file_length(&response)
}

/// 确保响应成功
fn ensure_success_response(response: &reqwest::Response) -> Result<()> {
    if !response.status().is_success() {
        Err(anyhow::anyhow!("下载失败，状态码: {}", response.status()))
    } else {
        Ok(())
    }
}
