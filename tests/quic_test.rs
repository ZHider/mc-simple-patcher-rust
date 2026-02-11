use mc_simple_patcher::utils::downloader::create_http_client;
use mc_simple_patcher::config::NetworkConfig;

#[tokio::test]
async fn test_quic_client_creation() {
    // 测试创建启用了QUIC的HTTP客户端
    let quic_config = Some(NetworkConfig {
        quic: true,
        ignore_invalid_cert: false,
    });
    
    let client = create_http_client(quic_config);
    assert!(client.is_ok());
    
    // 测试创建普通的HTTP客户端
    let normal_config = Some(NetworkConfig {
        quic: false,
        ignore_invalid_cert: true,
    });
    
    let client = create_http_client(normal_config);
    assert!(client.is_ok());
    
    // 测试默认配置
    let default_config = None;
    let client = create_http_client(default_config);
    assert!(client.is_ok());
}