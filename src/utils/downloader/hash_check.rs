use crate::utils;
use anyhow::{Context, Result};
use std::path::Path;

/// 从HTTP响应头中尝试获取SHA256哈希值
fn get_sha256_from_headers(response: &reqwest::Response) -> Option<String> {
    // 尝试从多种可能的头部获取哈希值
    // 常见的头部包括: Digest, X-Checksum-Sha256, X-SHA256 等
    if let Some(digest_header) = response.headers().get("digest") {
        let digest_str = digest_header.to_str().ok()?;
        // Digest头部通常格式为 "sha-256=:xxx="，其中xxx是base64编码的哈希值
        if digest_str.starts_with("sha-256=") {
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
pub async fn check_file_integrity(url: &str, dest_path: &Path) -> Result<bool> {
    if !dest_path.exists() {
        return Ok(false);
    }

    log::info!("文件已存在，检查完整性: {}", dest_path.display());

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
        log::debug!("HEAD请求失败: {}", head_response.status());
    }

    // 如果头部没有提供，则尝试从.sha256文件获取
    if remote_sha256.is_none() {
        remote_sha256 = get_sha256_from_file(&client, url).await;
    }

    if let Some(remote_sha256_val) = remote_sha256 {
        // 计算本地文件的SHA256哈希值
        let local_sha256 = utils::calculate_file_sha256(dest_path)?;
        let local_sha256 = hex::encode(local_sha256);

        if local_sha256 == remote_sha256_val {
            log::info!("文件已是最新版本: {}", dest_path.display());
            Ok(true)
        } else {
            log::info!(
                "文件已存在但内容不同，需要重新下载: {}",
                dest_path.display()
            );
            Ok(false)
        }
    } else {
        log::info!(
            "服务器未提供SHA256哈希值，跳过完整性检查: {}",
            dest_path.display()
        );
        Ok(false)
    }
}
