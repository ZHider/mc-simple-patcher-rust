use anyhow::{Context, Result};
use reqwest::Response;

/// 解析响应头中的 Content-Length 并返回 u64
pub fn get_file_length(resp: &Response) -> Result<u64> {
    if let Some(file_length) = resp.headers().get("Content-Length") {
        let fl_str = file_length
            .to_str()
            .context(format!("HTTP头读取时解码失败: {:?}", file_length))?;
        let fl_u64: u64 = fl_str
            .parse()
            .context(format!("Content-Length: {} 似乎不是有效的整数？", fl_str))?;
        Ok(fl_u64)
    } else {
        anyhow::bail!(
            "未能找到 HTTP 头 Content-Length 来获得文件大小在 {}",
            resp.url()
        )
    }
}

/// 发送 HEAD 请求获取远程文件大小
pub async fn get_file_size(url: &str) -> Result<u64> {
    let client = reqwest::Client::new();
    let response = client
        .head(url)
        .send()
        .await
        .context(format!("无法发送HEAD请求到: {}", url))?;

    get_file_length(&response)
}

/// 确保响应成功
pub fn ensure_success_response(response: &reqwest::Response) -> Result<()> {
    if !response.status().is_success() {
        Err(anyhow::anyhow!("下载失败，状态码: {}", response.status()))
    } else {
        Ok(())
    }
}
