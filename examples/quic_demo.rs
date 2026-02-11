//! 演示如何使用新的QUIC/HTTP/3功能

use mc_simple_patcher::utils::downloader::create_http_client;
use mc_simple_patcher::config::NetworkConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== QUIC/HTTP/3 功能演示 ===\n");

    // 演示1: 创建普通HTTP客户端
    println!("1. 创建普通HTTP客户端:");
    let normal_config = Some(NetworkConfig {
        quic: false,
        ignore_invalid_cert: false,
    });
    let client = create_http_client(normal_config)?;
    println!("   ✓ 普通HTTP客户端创建成功\n");

    // 演示2: 创建启用了证书忽略的客户端
    println!("2. 创建忽略证书错误的客户端:");
    let insecure_config = Some(NetworkConfig {
        quic: false,
        ignore_invalid_cert: true,
    });
    let client = create_http_client(insecure_config)?;
    println!("   ✓ 忽略证书错误的客户端创建成功\n");

    // 演示3: 创建启用了QUIC的客户端
    println!("3. 创建启用了QUIC/HTTP/3的客户端:");
    let quic_config = Some(NetworkConfig {
        quic: true,
        ignore_invalid_cert: false,
    });
    let client = create_http_client(quic_config)?;
    println!("   ✓ QUIC/HTTP/3客户端创建成功\n");

    // 演示4: 使用默认配置
    println!("4. 创建使用默认配置的客户端:");
    let default_config = None;
    let client = create_http_client(default_config)?;
    println!("   ✓ 默认配置客户端创建成功\n");

    println!("=== 所有客户端创建演示完成 ===");
    println!("\n要启用QUIC功能，请在配置文件中设置:");
    println!("[network]");
    println!("quic = true");
    println!("ignore_invalid_cert = false  # 或根据需要设置");

    Ok(())
}