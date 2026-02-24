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
    let headers = resp.headers();
    if let Some(file_length) = headers.get("Content-Length") {
        let fl_str = file_length
            .to_str()
            .context(format!("HTTP 头读取时解码失败：{:?}", file_length))?;
        let fl_u64: u64 = fl_str
            .parse()
            .context(format!("Content-Length: {} 似乎不是有效的整数？", fl_str))?;
        Ok(fl_u64)
    } else if let Some(file_range) = headers.get("Content-Range") {
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
        Ok(fl_u64)
    } else {
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
    let gz_file = File::open(gz_path)?;
    let reader = BufReader::new(gz_file);
    let mut decoder = GzDecoder::new(reader);
    let mut output_file = File::create(output_path)?;
    copy(&mut decoder, &mut output_file)?;
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
