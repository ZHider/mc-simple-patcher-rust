use anyhow::{Context, Result};
use futures_util::StreamExt;
use reqwest;
use sha2::{Sha256, Digest};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::Instant;
use tokio::io::AsyncWriteExt;

/// 进度回调类型
pub type ProgressCallback = dyn Fn(u64, Option<u64>, Instant) -> Result<()> + Send + Sync;

/// 计算文件的SHA256哈希值
fn calculate_file_sha256(file_path: &Path) -> Result<String> {
    let mut file = fs::File::open(file_path)?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)?;
    let hash_bytes = hasher.finalize();
    Ok(format!("{:x}", hash_bytes))
}

/// 从HTTP响应头中尝试获取SHA256哈希值
fn get_sha256_from_headers(response: &reqwest::Response) -> Option<String> {
    // 尝试从多种可能的头部获取哈希值
    // 常见的头部包括: Digest, X-Checksum-Sha256, X-SHA256 等
    if let Some(digest_header) = response.headers().get("digest") {
        let digest_str = digest_header.to_str().ok()?;
        // Digest头部通常格式为 "sha-256=:xxx="，其中xxx是base64编码的哈希值
        if digest_str.starts_with("sha-256=") {
            // 这里我们简化处理，实际应用中可能需要解析base64
            log::debug!("从Digest头部获取到SHA256信息: {}", digest_str);
        }
    }

    // 也可以尝试自定义头部
    if let Some(sha256_header) = response.headers().get("x-checksum-sha256") {
        return sha256_header.to_str().ok().map(|s| s.to_string());
    }

    if let Some(sha256_header) = response.headers().get("x-sha256") {
        return sha256_header.to_str().ok().map(|s| s.to_string());
    }

    None
}

/// 从URL获取对应的.sha256文件内容
async fn get_sha256_from_file(client: &reqwest::Client, url: &str) -> Option<String> {
    // 构造.sha256文件的URL，直接在原URL后添加.sha256
    let sha256_url = format!("{}.sha256", url);

    log::debug!("尝试从 {} 获取SHA256哈希值", sha256_url);

    let response = client.get(&sha256_url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }

    let content = response.text().await.ok()?;
    // 通常.sha256文件只包含哈希值，但也可能是"哈希值 文件名"格式
    let trimmed = content.trim();
    trimmed.split_whitespace().next().map(|s| s.to_string())
}

/// 检查文件是否已存在且与远程文件哈希值匹配
async fn check_file_integrity(url: &str, dest_path: &Path) -> Result<bool> {
    if !dest_path.exists() {
        return Ok(false);
    }

    log::info!("文件已存在，检查完整性: {:?}", dest_path);

    // 获取远程文件的SHA256哈希值
    let client = reqwest::Client::new();

    // 首先尝试从HTTP头部获取
    let mut remote_sha256 = None;
    let head_response = client
        .head(url)
        .send()
        .await
        .context(format!("无法发送HEAD请求到: {}", url))?;

    if head_response.status().is_success() {
        remote_sha256 = get_sha256_from_headers(&head_response);
    } else {
        log::warn!("HEAD请求失败: {}", head_response.status());
    }

    // 如果头部没有提供，则尝试从.sha256文件获取
    if remote_sha256.is_none() {
        remote_sha256 = get_sha256_from_file(&client, url).await;
    }

    if let Some(remote_sha256_val) = remote_sha256 {
        // 计算本地文件的SHA256哈希值
        let local_sha256 = calculate_file_sha256(dest_path)?;

        if local_sha256 == remote_sha256_val {
            log::info!("文件已是最新版本: {:?}", dest_path);
            return Ok(true);
        } else {
            log::info!("文件已存在但内容不同，需要重新下载: {:?}", dest_path);
            return Ok(false);
        }
    } else {
        log::info!("服务器未提供SHA256哈希值，跳过完整性检查: {:?}", dest_path);
        return Ok(false);
    }
}

/// 下载文件到指定路径
pub async fn download_file(url: &str, dest_path: &Path) -> Result<()> {
    // 检查文件是否已存在且完整
    if check_file_integrity(url, dest_path).await? {
        log::info!("跳过下载，文件已是最新版本: {:?}", dest_path);
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
    log::info!("开始下载: {} -> {:?}", url, dest_path);

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
    log::info!("下载完成: {:?} ({} bytes)", dest_path, downloaded);
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

    format!("{:.1}{}", size, UNITS[unit_index])
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
        download_file("https://httpbin.org/get", &dest_path).await?;

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
        std::fs::write(&test_file, "hello world")?;  // Just "hello world" without newline

        let hash = calculate_file_sha256(&test_file)?;
        // SHA256 of "hello world" is b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9
        assert_eq!(hash, "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9");

        Ok(())
    }
}
