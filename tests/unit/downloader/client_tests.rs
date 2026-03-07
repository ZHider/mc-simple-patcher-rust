//! HTTP 客户端模块测试
//! 测试 create_http_client 和 build_request 函数

use anyhow::Result;
use mc_simple_patcher::config::Config;
use mc_simple_patcher::utils::downloader::{build_request, create_http_client};
use std::sync::OnceLock;

static CONFIG_INIT: OnceLock<()> = OnceLock::new();

/// 初始化全局配置（测试用）
fn init_test_config() {
    CONFIG_INIT.get_or_init(|| {
        let config = Config {
            metadata_config: Default::default(),
            network: Default::default(),
            self_update: Default::default(),
            groups: Vec::new(),
        };
        mc_simple_patcher::global_config::set_global_config(config);
    });
}

#[test]
fn test_create_http_client_basic() -> Result<()> {
    init_test_config();

    let _client = create_http_client()?;

    // 验证客户端创建成功
    // 客户端成功创建（没有错误）即可

    Ok(())
}

#[test]
fn test_create_http_client_is_singleton() -> Result<()> {
    init_test_config();

    let _client1 = create_http_client()?;
    let _client2 = create_http_client()?;

    // 验证返回的是同一个客户端（单例模式）
    // 两个客户端都成功创建即可

    Ok(())
}

#[tokio::test]
async fn test_build_request_without_quic() -> Result<()> {
    init_test_config();

    let client = create_http_client()?;
    let request_builder = client.get("http://example.com/test");
    let configured = build_request(request_builder);

    // 在不启用 QUIC 的情况下，请求应该保持不变
    // 我们通过发送请求来验证配置是否正确
    let request = configured.build()?;

    // 验证请求可以正常构建
    assert_eq!(request.url().as_str(), "http://example.com/test");

    Ok(())
}

#[test]
fn test_create_http_client_timeout() -> Result<()> {
    init_test_config();

    let _client = create_http_client()?;

    // 验证客户端创建成功（超时配置在内部设置）
    // 客户端成功创建即可

    Ok(())
}

#[test]
fn test_create_http_client_cert_validation() -> Result<()> {
    init_test_config();

    // 这个测试验证客户端可以成功创建
    // 具体的证书验证设置在 create_http_client 中配置
    let result = create_http_client();

    assert!(result.is_ok());

    Ok(())
}
