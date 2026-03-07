//! HTTP 辅助函数模块
//! 提供 HTTP 请求和响应处理的底层函数

use std::path::Path;

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use reqwest::Response;
use std::fs::File;
use std::io::{BufReader, copy};

use super::client::{build_request, create_http_client};

/// 解析响应头中的 Content-Length
///
/// # Arguments
///
/// * `resp` - HTTP 响应的引用
///
/// # Returns
///
/// * `Result<u64>` - 成功时返回文件长度，失败时返回错误
pub fn get_file_length(resp: &Response) -> Result<u64> {
    log::trace!("解析响应头获取文件大小");
    let headers = resp.headers();
    if let Some(file_length) = headers.get("Content-Length") {
        let fl_str = file_length
            .to_str()
            .context(format!("HTTP 头读取时解码失败：{:?}", file_length))?;
        let fl_u64: u64 = fl_str
            .parse()
            .context(format!("Content-Length: {} 似乎不是有效的整数？", fl_str))?;
        log::trace!("从 Content-Length 获取文件大小：{} bytes", fl_u64);
        Ok(fl_u64)
    } else if let Some(file_range) = headers.get("Content-Range") {
        log::trace!("Content-Length 不存在，尝试从 Content-Range 获取");
        let fl_str = file_range
            .to_str()
            .context(format!("HTTP 头读取时解码失败：{:?}", file_range))?
            .split('/')
            .next_back()
            .with_context(|| {
                format!(
                    "尝试以/分割 content-range 时错误：{}",
                    file_range.to_str().unwrap()
                )
            })?
            .trim();
        let fl_u64: u64 = fl_str
            .parse()
            .context(format!("Content-Range: {} 似乎不是有效的整数？", fl_str))?;
        log::trace!("从 Content-Range 获取文件大小：{} bytes", fl_u64);
        Ok(fl_u64)
    } else {
        log::debug!("未能找到 Content-Length 或 Content-Range 头部");
        anyhow::bail!(
            "未能找到 HTTP 头 Content-Length / Content-Range 来获得文件大小在 {}",
            resp.url()
        )
    }
}

/// 同步解压 GZ 文件
///
/// # Arguments
///
/// * `gz_path` - GZ 压缩文件路径的引用
/// * `output_path` - 输出文件路径的引用
///
/// # Returns
///
/// * `Result<()>` - 成功时返回空值，失败时返回错误
pub fn decompress_gz_sync(gz_path: &Path, output_path: &Path) -> Result<()> {
    log::trace!("开始解压 GZ 文件：{:?} -> {:?}", gz_path, output_path);
    let gz_file = File::open(gz_path)?;
    let reader = BufReader::new(gz_file);
    let mut decoder = GzDecoder::new(reader);
    let mut output_file = File::create(output_path)?;
    let bytes = copy(&mut decoder, &mut output_file)?;
    log::debug!("GZ 解压完成：{} bytes", bytes);
    Ok(())
}

/// 异步获取 URL 响应（带 Range 头）
///
/// # Arguments
///
/// * `url` - URL 字符串的引用
/// * `range_type` - Range 类型（如 bytes）
/// * `start` - 起始位置
/// * `end` - 结束位置
///
/// # Returns
///
/// * `Result<Response>` - 成功时返回 HTTP 响应，失败时返回错误
pub async fn url_get_range(url: &str, range_type: &str, start: u64, end: u64) -> Result<Response> {
    log::trace!("发送 Range 请求：{} {}={}-{}", url, range_type, start, end);
    let client = create_http_client()?;
    build_request(client.get(url))
        .header("Range", format!("{range_type}={start}-{end}"))
        .send()
        .await
        .context("reqwest error in url get")
}

/// 检查服务器是否支持 Range 请求
///
/// # Arguments
///
/// * `resp` - HTTP 响应的引用
///
/// # Returns
///
/// * `Result<Option<String>>` - 成功时返回 Range 类型，不支持返回 None
pub fn support_download_range(resp: &Response) -> Result<Option<String>> {
    log::trace!("检查服务器 Range 支持");
    if let Some(t) = resp.headers().get("accept-ranges") {
        let t = t.to_str().context("解码头部失败")?.trim().to_lowercase();
        if t == "none" {
            log::trace!("服务器明确不支持 Range (accept-ranges: none)");
            return Ok(None);
        }
        log::trace!("服务器支持 Range 类型：{}", t);
        Ok(Some(t))
    } else {
        log::trace!("服务器未返回 accept-ranges 头部");
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
    log::trace!("从 HTTP 响应获取文件名");

    // 尝试从 Content-Disposition 头部获取文件名
    if let Some(content_disposition) = resp.headers().get("Content-Disposition") {
        log::trace!("找到 Content-Disposition 头部");
        let cd_str = content_disposition
            .to_str()
            .context(format!("HTTP 头读取时解码失败：{:?}", content_disposition))?;

        // 解析 filename="..." 或 filename=...
        if let Some(filename_start) = cd_str.find("filename=") {
            let filename_part = &cd_str[filename_start + 9..];
            let filename = if filename_part.starts_with('"') {
                // filename="..." 格式
                let name = filename_part.split('"').nth(1).unwrap_or("").to_string();
                log::trace!("从 Content-Disposition (quoted) 获取文件名：{}", name);
                name
            } else {
                // filename=... 格式（无引号）
                let name = filename_part
                    .split(';')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                log::trace!("从 Content-Disposition (unquoted) 获取文件名：{}", name);
                name
            };

            if !filename.is_empty() {
                return Ok(filename);
            }
        }
        log::trace!("Content-Disposition 解析失败，尝试从 URL 获取");
    } else {
        log::trace!("未找到 Content-Disposition 头部，从 URL 获取文件名");
    }

    // 从 URL 提取文件名
    let filename = resp
        .url()
        .path_segments()
        .context(format!("无法从 URL 中提取文件名：{}", resp.url()))?
        .next_back()
        .context("无法读取最后一个URL中的文本")?
        .to_string();

    log::trace!("从 URL 获取文件名：{}", filename);
    Ok(filename)
}
