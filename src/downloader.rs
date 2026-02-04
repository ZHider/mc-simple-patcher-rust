//! 网络下载模块
//! 实现文件下载功能

use std::path::Path;
use std::fs::File;
use reqwest;
use anyhow::{Context, Result};

/// 下载文件到指定路径
pub async fn download_file(url: &str, dest_path: &Path) -> Result<()> {
    log::info!("开始下载: {} -> {:?}", url, dest_path);

    // 创建客户端并执行下载
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .context(format!("无法发送请求到: {}", url))?;

    // 检查响应状态
    ensure_success_response(&response)?;

    // 创建目标文件
    let mut dest_file = File::create(dest_path)
        .context(format!("无法创建目标文件: {:?}", dest_path))?;

    // 将响应内容写入文件
    let content = response
        .bytes()
        .await
        .context(format!("无法读取响应内容: {}", url))?;

    let mut content_reader = std::io::Cursor::new(content);
    std::io::copy(&mut content_reader, &mut dest_file)
        .context(format!("无法写入文件: {:?}", dest_path))?;

    log::info!("下载完成: {:?}", dest_path);
    Ok(())
}

/// 确保响应成功
fn ensure_success_response(response: &reqwest::Response) -> Result<()> {
    if !response.status().is_success() {
        Err(anyhow::anyhow!("下载失败，状态码: {}", response.status()))
    } else {
        Ok(())
    }
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
}