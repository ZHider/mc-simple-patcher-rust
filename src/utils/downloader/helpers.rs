use anyhow::{Context, Ok, Result};
use reqwest::Response;

/// 解析响应头中的 Content-Length 并返回 u64
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

/// 确保响应成功
pub fn ensure_success_response(response: &reqwest::Response) -> Result<()> {
    if !response.status().is_success() {
        Err(anyhow::anyhow!("下载失败，状态码: {}", response.status()))
    } else {
        Ok(())
    }
}
