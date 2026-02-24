use std::path::Path;

use anyhow::{Context, Ok, Result};
use reqwest::Response;

/// 解析响应头中的 Content-Length 并返回 u64
///
/// # Arguments
///
/// * `resp` - HTTP响应的引用
///
/// # Returns
///
/// * `Result<u64>` - 成功时返回文件长度，失败时返回错误
pub fn get_file_length(resp: &Response) -> Result<u64> {
    let headers = resp.headers();
    if let Some(file_length) = headers.get("Content-Length") {
        let fl_str = file_length
            .to_str()
            .context(format!("HTTP头读取时解码失败: {:?}", file_length))?;
        let fl_u64: u64 = fl_str
            .parse()
            .context(format!("Content-Length: {} 似乎不是有效的整数？", fl_str))?;
        Ok(fl_u64)
    } else if let Some(file_range) = headers.get("Content-Range") {
        let fl_str = file_range
            .to_str()
            .context(format!("HTTP头读取时解码失败: {:?}", file_range))?
            .split('/')
            .next_back()
            .with_context(|| {
                format!(
                    "尝试以/分割content-range时错误：{}",
                    file_range.to_str().unwrap()
                )
            })?
            .trim();
        let fl_u64: u64 = fl_str
            .parse()
            .context(format!("Content-Range: {} 似乎不是有效的整数？", fl_str))?;
        Ok(fl_u64)
    } else {
        anyhow::bail!(
            "未能找到 HTTP 头 Content-Length / Content-Range 来获得文件大小在 {}",
            resp.url()
        )
    }
}

/// 同步解压GZ文件
///
/// # Arguments
///
/// * `gz_path` - GZ压缩文件路径的引用
/// * `output_path` - 输出文件路径的引用
///
/// # Returns
///
/// * `Result<()>` - 成功时返回空值，失败时返回错误
pub fn decompress_gz_sync(gz_path: &Path, output_path: &Path) -> Result<()> {
    use flate2::read::GzDecoder;
    use std::fs::File;
    use std::io::{BufReader, copy};

    let gz_file = File::open(gz_path)?;
    let reader = BufReader::new(gz_file);
    let mut decoder = GzDecoder::new(reader);
    let mut output_file = File::create(output_path)?;
    copy(&mut decoder, &mut output_file)?;
    Ok(())
}

/// 异步获取URL响应
///
/// # Arguments
///
/// * `url` - URL字符串的引用
///
/// # Returns
///
/// * `Result<Response>` - 成功时返回HTTP响应，失败时返回错误
pub async fn url_get(url: &str) -> Result<Response> {
    use super::build_request;
    use super::create_http_client;

    let client = create_http_client()?;
    build_request(client.get(url))
        .send()
        .await
        .context("reqwest error in url get")
}

pub async fn url_get_range(url: &str, range_type: &str, start: u64, end: u64) -> Result<Response> {
    use super::build_request;
    use super::create_http_client;

    let client = create_http_client()?;
    build_request(client.get(url))
        .header("Range", format!("{range_type}={start}-{end}"))
        .send()
        .await
        .context("reqwest error in url get")
}

pub fn support_download_range(resp: &Response) -> Result<Option<String>> {
    if let Some(t) = resp.headers().get("accept-ranges") {
        let t = t.to_str().context("解码头部失败")?.trim().to_lowercase();
        if t == "none" {
            return Ok(None);
        }
        Ok(Some(t))
    } else {
        Ok(None)
    }
}

/// 从 HTTP 响应中获取文件名
///
/// 优先从 Content-Disposition 头部提取 filename，如果不存在则从 URL 提取。
///
/// # Arguments
///
/// * `resp` - HTTP 响应的引用
///
/// # Returns
///
/// * `Result<String>` - 成功时返回文件名，失败时返回错误
pub fn get_filename_from_response(resp: &Response) -> Result<String> {
    // 尝试从 Content-Disposition 头部获取文件名
    if let Some(content_disposition) = resp.headers().get("Content-Disposition") {
        let cd_str = content_disposition
            .to_str()
            .context(format!("HTTP 头读取时解码失败：{:?}", content_disposition))?;

        // 解析 filename="..." 或 filename=...
        if let Some(filename_start) = cd_str.find("filename=") {
            let filename_part = &cd_str[filename_start + 9..];
            let filename = if filename_part.starts_with('"') {
                // filename="..." 格式
                filename_part.split('"').nth(1).unwrap_or("").to_string()
            } else {
                // filename=... 格式（无引号）
                filename_part
                    .split(';')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string()
            };

            if !filename.is_empty() {
                return Ok(filename);
            }
        }
    }

    // 从 URL 提取文件名
    resp.url()
        .path_segments()
        .and_then(|segments| segments.last())
        .map(|s| s.to_string())
        .context(format!("无法从 URL 中提取文件名：{}", resp.url()))
}
