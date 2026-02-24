use super::build_request;
use crate::utils;
use anyhow::Result;
use std::{path::Path, time::Duration};

/// 从 HTTP 响应头中尝试获取 SHA256 哈希值
///
/// # Arguments
///
/// * `response` - HTTP 响应的引用
///
/// # Returns
///
/// * `Option<String>` - 如果找到 SHA256 哈希值则返回，否则返回 None
fn get_sha256_from_headers(response: &reqwest::Response) -> Option<String> {
    log::trace!("尝试从 HTTP 响应头获取 SHA256");

    // 尝试从多种可能的头部获取哈希值
    // 常见的头部包括：Digest, X-Checksum-Sha256, X-SHA256 等
    if let Some(digest_header) = response.headers().get("digest") {
        let digest_str = digest_header.to_str().ok()?;
        // Digest 头部通常格式为 "sha-256=:xxx="，其中 xxx 是 base64 编码的哈希值
        log::debug!("从 Digest 头部获取到 SHA256 信息：{}", digest_str);
    }

    // 也可以尝试自定义头部
    if let Some(sha256_header) = response.headers().get("x-checksum-sha256") {
        let sha256 = sha256_header.to_str().ok().map(|s| s.to_string());
        if sha256.is_some() {
            log::trace!("从 x-checksum-sha256 头部获取到 SHA256");
        }
        return sha256;
    }

    if let Some(sha256_header) = response.headers().get("x-sha256") {
        let sha256 = sha256_header.to_str().ok().map(|s| s.to_string());
        if sha256.is_some() {
            log::trace!("从 x-sha256 头部获取到 SHA256");
        }
        return sha256;
    }

    log::trace!("未在响应头中找到 SHA256 信息");
    None
}

/// 从 URL 获取对应的.sha256 文件内容
///
/// # Arguments
///
/// * `client` - HTTP 客户端的引用
/// * `url` - URL 字符串的引用
///
/// # Returns
///
/// * `Option<String>` - 如果成功获取到 SHA256 哈希值则返回，否则返回 None
async fn get_sha256_from_file(client: &reqwest::Client, url: &str) -> Option<String> {
    // 构造.sha256 文件的 URL，直接在原 URL 后添加.sha256
    let sha256_url = format!("{}.sha256", url);

    log::debug!("尝试从 {} 获取 SHA256 哈希值", sha256_url);

    let request = client.get(&sha256_url);
    let configured_request = build_request(request);
    let response = configured_request.send().await.ok()?;
    if !response.status().is_success() {
        log::debug!(".sha256 文件请求失败，状态码：{}", response.status());
        return None;
    }

    let content = response.text().await.ok()?;
    // 通常.sha256 文件只包含哈希值，但也可能是"哈希值 文件名"格式
    let trimmed = content.trim();
    let sha256 = trimmed.split_whitespace().next().map(|s| s.to_string());
    if sha256.is_some() {
        log::trace!("从.sha256 文件获取到 SHA256");
    }
    sha256
}

/// 检查文件是否已存在且与远程文件哈希值匹配
///
/// # Arguments
///
/// * `url` - URL 字符串的引用
/// * `dest_path` - 目标文件路径的引用
///
/// # Returns
///
/// * `Result<Option<bool>>` - 成功时返回可选的布尔值表示文件完整性，失败时返回错误
pub async fn check_file_integrity(url: &str, dest_path: &Path) -> Result<Option<bool>> {
    log::trace!("检查文件完整性：url={}, dest_path={:?}", url, dest_path);

    if !dest_path.exists() {
        log::trace!("文件不存在：{:?}", dest_path);
        return Ok(Some(false));
    }

    log::info!("文件已存在，检查完整性：{}", dest_path.display());

    // 获取远程文件的 SHA256 哈希值
    let client = super::create_http_client()?;

    // 首先尝试从 HTTP 头部获取
    let mut remote_sha256 = None;

    log::trace!("发送 HEAD 请求获取响应头");
    let head_request = client.head(url);
    let configured_head_request = build_request(head_request)
        .timeout(Duration::from_secs(1))
        .send();
    if let Ok(head_response) = configured_head_request.await
        && head_response.status().is_success()
    {
        remote_sha256 = get_sha256_from_headers(&head_response);
        if remote_sha256.is_some() {
            log::debug!("从 HEAD 响应头获取到 SHA256");
        }
    } else {
        log::debug!("HEAD 请求失败。");
    }

    // 如果头部没有提供，则尝试从.sha256 文件获取
    if remote_sha256.is_none() {
        log::trace!("HEAD 请求未获取到 SHA256，尝试从.sha256 文件获取");
        remote_sha256 = get_sha256_from_file(&client, url).await;
    }

    if let Some(remote_sha256_val) = remote_sha256 {
        log::debug!("远程 SHA256: {}", remote_sha256_val);

        // 计算本地文件的 SHA256 哈希值
        log::trace!("计算本地文件 SHA256: {:?}", dest_path);
        let local_sha256 = utils::calculate_file_sha256(dest_path)?;
        let local_sha256 = hex::encode(local_sha256);
        log::trace!("本地 SHA256: {}", local_sha256);

        if local_sha256 == remote_sha256_val {
            log::info!("文件已是最新版本：{}", dest_path.display());
            Ok(Some(true))
        } else {
            log::info!(
                "文件已存在但内容不同，需要重新下载：{}",
                dest_path.display()
            );
            Ok(Some(false))
        }
    } else {
        log::info!(
            "服务器未提供 SHA256 哈希值，跳过完整性检查：{}",
            dest_path.display()
        );
        Ok(None)
    }
}
