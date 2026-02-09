mod hash_check;

use anyhow::{Context, Result};
use futures_util::StreamExt;
use reqwest;
use std::io::Write;
use std::path::Path;
use std::time::Instant;
use tokio::io::AsyncWriteExt;

/// 进度回调类型
pub type ProgressCallback = dyn Fn(u64, Option<u64>, Instant) -> Result<()> + Send + Sync;

/// 下载文件到指定路径
pub async fn download_file(url: &str, dest_path: &Path, check_sha256: bool) -> Result<()> {
    // 检查文件是否已存在且完整
    if check_sha256 && hash_check::check_file_integrity(url, dest_path).await? {
        log::info!("跳过下载，文件已是最新版本: {}", dest_path.display());
        return Ok(());
    }

    download_file_with_progress(url, dest_path, None).await
}

/// 使用进度回调下载文件到指定路径
pub async fn download_file_with_progress(
    url: &str,
    dest_path: &Path,
    progress_callback: Option<&ProgressCallback>,
) -> Result<()> {
    log::info!("开始下载: {} -> {}", url, dest_path.display());

    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .context(format!("无法发送请求到: {}", url))?;

    ensure_success_response(&response)?;

    let total_size = response.content_length();
    let start_time = Instant::now();

    let mut dest_file = tokio::fs::File::create(dest_path)
        .await
        .context(format!("无法创建目标文件: {:?}", dest_path))?;

    let downloaded = download_with_progress_tracking(
        response.bytes_stream(),
        &mut dest_file,
        total_size,
        start_time,
        progress_callback,
    )
    .await?;

    // 换行并显示完成信息
    println!();
    log::info!("下载完成: {} ({} bytes)", dest_path.display(), downloaded);
    Ok(())
}

/// 跟踪下载进度的主要函数
async fn download_with_progress_tracking(
    mut stream: impl futures_util::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Unpin,
    dest_file: &mut tokio::fs::File,
    total_size: Option<u64>,
    start_time: Instant,
    progress_callback: Option<&ProgressCallback>,
) -> Result<u64> {
    let mut downloaded: u64 = 0;

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.context("无法读取数据块")?;
        dest_file.write_all(&chunk).await.context("无法写入文件")?;

        downloaded += chunk.len() as u64;

        // 更新进度
        update_progress(downloaded, total_size, start_time, progress_callback)?;
    }

    Ok(downloaded)
}

/// 更新下载进度
fn update_progress(
    downloaded: u64,
    total_size: Option<u64>,
    start_time: Instant,
    progress_callback: Option<&ProgressCallback>,
) -> Result<()> {
    // 调用进度回调（如果提供）
    if let Some(callback) = progress_callback {
        callback(downloaded, total_size, start_time)?;
    } else {
        // 默认的进度显示
        display_progress(downloaded, total_size);
    }

    Ok(())
}

/// 显示下载进度
fn display_progress(downloaded: u64, total_size: Option<u64>) {
    if let Some(total) = total_size {
        let progress_percent = (downloaded as f64 / total as f64) * 100.0;
        print!(
            "\r下载进度: {:.1}% ({}/{})",
            progress_percent,
            human_readable_size(downloaded),
            human_readable_size(total)
        );
    } else {
        // 如果无法获取总大小，只显示已下载大小
        print!("\r下载进度: {}", human_readable_size(downloaded));
    }
    std::io::stdout().flush().unwrap();
}

/// 确保响应成功
fn ensure_success_response(response: &reqwest::Response) -> Result<()> {
    if !response.status().is_success() {
        Err(anyhow::anyhow!("下载失败，状态码: {}", response.status()))
    } else {
        Ok(())
    }
}

/// 将字节数转换为人类可读的大小格式
fn human_readable_size(size: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut size = size as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    format!("{:.2}{}", size, UNITS[unit_index])
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_downloader() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let dest_path = temp_dir.path().join("test.txt");

        // 测试下载一个简单的网页
        download_file("https://httpbin.org/get", &dest_path, false).await?;

        assert!(dest_path.exists());
        Ok(())
    }

    #[tokio::test]
    async fn test_download_with_progress() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let dest_path = temp_dir.path().join("test_with_progress.txt");

        // 测试带进度回调的下载
        let progress_callback =
            |downloaded: u64, total_size: Option<u64>, _: Instant| -> Result<()> {
                let progress_text = if let Some(total) = total_size {
                    let progress_percent = (downloaded as f64 / total as f64) * 100.0;
                    format!(
                        "下载进度: {:.1}% ({}/{})",
                        progress_percent,
                        human_readable_size(downloaded),
                        human_readable_size(total)
                    )
                } else {
                    format!("下载进度: {}", human_readable_size(downloaded))
                };
                println!("{}", progress_text);
                Ok(())
            };

        download_file_with_progress(
            "https://httpbin.org/get",
            &dest_path,
            Some(&progress_callback),
        )
        .await?;

        assert!(dest_path.exists());
        Ok(())
    }

    #[test]
    fn test_calculate_file_sha256() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let test_file = temp_dir.path().join("test_hash.txt");
        std::fs::write(&test_file, "hello world")?; // Just "hello world" without newline

        let hash = crate::utils::calculate_file_sha256(&test_file)?;
        // SHA256 of "hello world" is b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );

        Ok(())
    }
}
