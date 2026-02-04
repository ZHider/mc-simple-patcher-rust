//! 网络下载模块
//! 实现文件下载功能

use std::path::Path;
use std::fs::File;
use reqwest;
use anyhow::{Context, Result};

/// 下载器
pub struct Downloader {}

impl Downloader {
    /// 创建新的下载器
    pub fn new() -> Self {
        Self {}
    }

    /// 下载文件到指定路径
    pub async fn download_file(&self, url: &str, dest_path: &Path) -> Result<()> {
        log::info!("开始下载: {} -> {:?}", url, dest_path);

        // 创建客户端
        let client = reqwest::Client::new();
        let response = client
            .get(url)
            .send()
            .await
            .context(format!("无法发送请求到: {}", url))?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("下载失败，状态码: {}", response.status()));
        }

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_downloader() -> Result<()> {
        let downloader = Downloader::new();
        let temp_dir = TempDir::new()?;
        let dest_path = temp_dir.path().join("test.txt");

        // 测试下载一个简单的网页
        downloader.download_file("https://httpbin.org/get", &dest_path).await?;
        
        assert!(dest_path.exists());
        Ok(())
    }
}