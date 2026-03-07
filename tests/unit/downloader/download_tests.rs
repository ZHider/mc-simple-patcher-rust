//! 下载功能测试
//! 测试 download_file_internal 和相关下载逻辑

use anyhow::Result;
use mc_simple_patcher::config::Config;
use mc_simple_patcher::global_config::{GLOBAL_CONFIG, set_global_config};
use mc_simple_patcher::utils::downloader::download_file_internal;
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[allow(unused_imports)]
use mc_simple_patcher::utils::logger;

/// 初始化全局配置（测试用）
fn init_test_config() {
    if GLOBAL_CONFIG.read().unwrap().is_none() {
        let config = Config {
            metadata_config: Default::default(),
            network: Default::default(),
            self_update: Default::default(),
            groups: Vec::new(),
        };
        set_global_config(config);
    }
    // logger::init_logger(true, false).unwrap();
}

#[tokio::test]
async fn test_download_file_success() -> Result<()> {
    init_test_config();

    // 启动模拟服务器
    let mock_server = MockServer::start().await;

    let test_content = b"Hello, World! This is test content for download.";
    let filename = "test_file.txt";

    // 配置模拟响应
    Mock::given(method("GET"))
        .and(path(format!("/{}", filename)))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(std::str::from_utf8(test_content).unwrap())
                .insert_header("Content-Length", test_content.len().to_string())
                .insert_header(
                    "Content-Disposition",
                    format!("attachment; filename=\"{}\"", filename),
                ),
        )
        .mount(&mock_server)
        .await;

    let url = format!("{}/{}", mock_server.uri(), filename);
    let temp_dir = TempDir::new()?;
    let dest_path = temp_dir.path().join(filename);

    // 执行下载
    let (downloaded, path) =
        download_file_internal(&url, Some(&dest_path), false, None, false).await?;

    // 验证下载成功
    assert!(downloaded);
    assert_eq!(path, dest_path);
    assert!(dest_path.exists());

    // 验证文件内容
    let content = std::fs::read(&dest_path)?;
    assert_eq!(content, test_content);

    temp_dir.close()?;
    Ok(())
}

#[tokio::test]
async fn test_download_file_with_sha256_check_skip() -> Result<()> {
    init_test_config();

    let mock_server = MockServer::start().await;

    let test_content = b"Test content for SHA256 check";
    let filename = "test_sha256.txt";
    let sha256_hex = "5a04b8eca290a51e54ac0ccf7ca26c53a3fbb338256a75fccb8862ae328ba07d"; // SHA256 of test_content

    // 配置.sha256 文件响应
    Mock::given(method("GET"))
        .and(path(format!("/{}.sha256", filename)))
        .respond_with(ResponseTemplate::new(200).set_body_string(sha256_hex))
        .mount(&mock_server)
        .await;

    // 配置文件下载响应（但应该不会触发下载）
    Mock::given(method("GET"))
        .and(path(format!("/{}", filename)))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(std::str::from_utf8(test_content).unwrap()),
        )
        .mount(&mock_server)
        .await;

    let url = format!("{}/{}", mock_server.uri(), filename);
    let temp_dir = TempDir::new()?;
    let dest_path = temp_dir.path().join(filename);

    // 先写入相同内容的文件
    crate::common::write_sync(&dest_path, test_content)?;

    // 执行下载（应该跳过）
    let (downloaded, path) =
        download_file_internal(&url, Some(&dest_path), true, None, false).await?;

    // 验证跳过下载
    assert!(!downloaded);
    assert_eq!(path, dest_path);

    temp_dir.close()?;
    Ok(())
}

#[tokio::test]
async fn test_download_file_with_sha256_check_download() -> Result<()> {
    init_test_config();

    let mock_server = MockServer::start().await;

    let test_content = b"Test content for SHA256 check - different content";
    let existing_content = b"Existing content that should be replaced";
    let filename = "test_sha256_diff.txt";
    let sha256_hex = "invalid_sha256_for_existing"; // 与现有文件不匹配

    // 配置.sha256 文件响应
    Mock::given(method("GET"))
        .and(path(format!("/{}.sha256", filename)))
        .respond_with(ResponseTemplate::new(200).set_body_string(sha256_hex))
        .mount(&mock_server)
        .await;

    // 配置文件下载响应
    Mock::given(method("GET"))
        .and(path(format!("/{}", filename)))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(std::str::from_utf8(test_content).unwrap()),
        )
        .mount(&mock_server)
        .await;

    let url = format!("{}/{}", mock_server.uri(), filename);
    let temp_dir = TempDir::new()?;
    let dest_path = temp_dir.path().join(filename);

    // 先写入不同内容的文件
    crate::common::write_sync(&dest_path, existing_content)?;

    // 执行下载（应该重新下载）
    let (downloaded, path) =
        download_file_internal(&url, Some(&dest_path), true, None, false).await?;

    // 验证进行了下载
    assert!(downloaded);
    assert_eq!(path, dest_path);

    // 验证文件内容已更新
    let content = std::fs::read(&dest_path)?;
    assert_eq!(content, test_content);

    temp_dir.close()?;
    Ok(())
}

#[tokio::test]
async fn test_download_file_not_found() -> Result<()> {
    init_test_config();

    let mock_server = MockServer::start().await;

    let filename = "nonexistent.txt";
    let url = format!("{}/{}", mock_server.uri(), filename);
    let temp_dir = TempDir::new()?;
    let dest_path = temp_dir.path().join(filename);

    // 执行下载（应该失败）
    let result = download_file_internal(&url, Some(&dest_path), false, None, false).await;

    // 验证下载失败
    assert!(result.is_err());

    temp_dir.close()?;
    Ok(())
}

#[tokio::test]
async fn test_download_file_infer_filename() -> Result<()> {
    init_test_config();

    let mock_server = MockServer::start().await;

    let test_content = b"Test content with inferred filename";
    let filename = "inferred_file.bin";

    // 配置模拟响应（从 URL 推断文件名）
    Mock::given(method("GET"))
        .and(path(format!("/download/{}", filename)))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(std::str::from_utf8(test_content).unwrap())
                .insert_header("Content-Length", test_content.len().to_string())
                .append_header(
                    "Content-Disposition",
                    format!("Content-Disposition: attachment; filename={filename}"),
                ),
        )
        .mount(&mock_server)
        .await;

    let url = format!("{}/download/{}", mock_server.uri(), filename);

    // 不指定目标路径，让下载器从 URL 推断
    let (downloaded, path) = download_file_internal(&url, None, false, None, false).await?;

    // 验证下载成功
    assert!(downloaded);
    assert!(path.exists());

    // 验证文件名正确推断
    assert_eq!(path.file_name().unwrap(), filename);

    // 验证文件内容
    let content = std::fs::read(&path)?;
    assert_eq!(content, test_content);

    Ok(())
}

#[tokio::test]
async fn test_download_file_server_error() -> Result<()> {
    init_test_config();

    let mock_server = MockServer::start().await;

    let filename = "server_error.txt";

    // 配置 500 服务器错误响应
    Mock::given(method("GET"))
        .and(path(format!("/{}", filename)))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock_server)
        .await;

    let url = format!("{}/{}", mock_server.uri(), filename);
    let temp_dir = TempDir::new()?;
    let dest_path = temp_dir.path().join(filename);

    // 执行下载（应该失败）
    let result = download_file_internal(&url, Some(&dest_path), false, None, false).await;

    // 验证下载失败
    assert!(result.is_err());

    temp_dir.close()?;
    Ok(())
}

#[tokio::test]
async fn test_download_file_empty_content() -> Result<()> {
    init_test_config();

    let mock_server = MockServer::start().await;

    let filename = "empty_file.txt";

    // 配置空内容响应
    Mock::given(method("GET"))
        .and(path(format!("/{}", filename)))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("")
                .insert_header("Content-Length", "0"),
        )
        .mount(&mock_server)
        .await;

    let url = format!("{}/{}", mock_server.uri(), filename);
    let temp_dir = TempDir::new()?;
    let dest_path = temp_dir.path().join(filename);

    // 执行下载
    let (downloaded, path) =
        download_file_internal(&url, Some(&dest_path), false, None, false).await?;

    // 验证下载成功
    assert!(downloaded);
    assert_eq!(path, dest_path);

    // 验证文件为空
    let content = std::fs::read(&dest_path)?;
    assert!(content.is_empty());

    temp_dir.close()?;
    Ok(())
}
