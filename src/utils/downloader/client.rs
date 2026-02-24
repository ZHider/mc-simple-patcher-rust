//! HTTP 客户端管理模块
//! 负责创建和管理 HTTP 客户端

use std::sync::OnceLock;
use std::time::Duration;

use anyhow::Result;
use reqwest;

use crate::global_config::get_global_config;

static HTTP_CLIENT_TEMPLATE: OnceLock<reqwest::Client> = OnceLock::new();

/// 根据网络配置创建 HTTP 客户端
///
/// # Returns
///
/// * `Result<reqwest::Client>` - 成功时返回 HTTP 客户端，失败时返回错误
pub fn create_http_client() -> Result<reqwest::Client> {
    fn init_client_template() -> reqwest::Client {
        let config = get_global_config();
        let network_config = &config.network;

        let builder = reqwest::ClientBuilder::new()
            .tls_danger_accept_invalid_certs(network_config.ignore_invalid_cert)
            .timeout(Duration::from_secs(network_config.timeout));

        let builder = if network_config.quic {
            // 使用 HTTP/3 协议
            builder.http3_prior_knowledge()
        } else {
            builder
        };
        builder.build().expect("创建 client 客户端时错误")
    }

    Ok(HTTP_CLIENT_TEMPLATE
        .get_or_init(init_client_template)
        .clone())
}

/// 为请求添加版本信息（如果需要）
pub fn build_request(request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    let config = get_global_config();

    if config.network.quic {
        // 显式指定使用 HTTP/3 版本
        return request.version(reqwest::Version::HTTP_3);
    }
    request
}
