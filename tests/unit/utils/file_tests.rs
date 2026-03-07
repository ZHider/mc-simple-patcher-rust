//! 文件工具模块测试

use crate::common::*;
use anyhow::Result;
use bytes::Bytes;
use mc_simple_patcher::utils::file;
use std::path::Path;

#[test]
fn test_calculate_file_sha256_empty_file() -> Result<()> {
    let temp_dir = tempfile::tempdir_in(crate::TEST_WORKSPACE.as_path()).expect("创建临时目录失败");

    // 创建空文件
    let empty_file = temp_dir.path().join("empty.txt");
    crate::common::write_sync(&empty_file, b"")?;

    let hash = file::calculate_file_sha256(&empty_file)?;

    // 空文件的SHA256
    let expected = Bytes::from(hex::decode(
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    )?);
    assert_eq!(hash, expected);

    temp_dir.close()?;
    Ok(())
}

#[test]
fn test_calculate_file_sha256_small_file() -> Result<()> {
    let temp_dir = tempfile::tempdir_in(crate::TEST_WORKSPACE.as_path()).expect("创建临时目录失败");

    let content = b"Hello, World!";
    let test_file = temp_dir.path().join("test.txt");
    crate::common::write_sync(&test_file, content)?;

    let hash = file::calculate_file_sha256(&test_file)?;

    // "Hello, World!"的SHA256
    let expected = Bytes::from(hex::decode(
        "dffd6021bb2bd5b0af676290809ec3a53191dd81c7f70a4b28688a362182986f",
    )?);
    assert_eq!(hash, expected);

    temp_dir.close()?;
    Ok(())
}

#[test]
fn test_calculate_file_sha256_large_file() -> Result<()> {
    let temp_dir = tempfile::tempdir_in(crate::TEST_WORKSPACE.as_path()).expect("创建临时目录失败");

    // 创建1MB的文件
    let large_content = vec![0xAAu8; 1024 * 1024];
    let large_file = temp_dir.path().join("large.bin");
    crate::common::write_sync(&large_file, &large_content)?;

    let hash = file::calculate_file_sha256(&large_file)?;

    // 验证哈希计算成功（不验证具体值，因为内容固定）
    assert_eq!(hash.len(), 32); // SHA256是32字节

    temp_dir.close()?;
    Ok(())
}

#[test]
fn test_calculate_file_sha256_nonexistent_file() {
    let non_existent = Path::new("/nonexistent/path/file.txt");
    let result = file::calculate_file_sha256(non_existent);

    assert!(result.is_err());
    assert_error_contains_any(
        result,
        &[
            "No such file or directory",
            "系统找不到指定的路径",
            "找不到",
        ],
    );
}

#[test]
fn test_calculate_file_sha256_directory() {
    let temp_dir = tempfile::tempdir_in(crate::TEST_WORKSPACE.as_path()).expect("创建临时目录失败");

    // 尝试计算目录的SHA256
    let result = file::calculate_file_sha256(temp_dir.path());

    assert!(result.is_err());
    // Windows和Unix的错误信息不同
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("permission denied")
            || error_msg.contains("Access is denied")
            || error_msg.contains("Is a directory")
            || error_msg.contains("拒绝访问")
            || error_msg.contains("目录")
    );
    temp_dir.close().expect("关闭临时目录失败");
}

#[test]
fn test_get_filename_valid_path() -> Result<()> {
    let path = Path::new("/path/to/file.txt");
    let filename = file::get_filename(path)?;

    assert_eq!(filename.as_ref(), "file.txt");
    Ok(())
}

#[test]
fn test_get_filename_with_extension() -> Result<()> {
    let path = Path::new("archive.tar.gz");
    let filename = file::get_filename(path)?;

    assert_eq!(filename.as_ref(), "archive.tar.gz");
    Ok(())
}

#[test]
fn test_get_filename_with_special_characters() -> Result<()> {
    let path = Path::new("file with spaces and (parentheses).txt");
    let filename = file::get_filename(path)?;

    assert_eq!(filename.as_ref(), "file with spaces and (parentheses).txt");
    Ok(())
}

#[test]
fn test_get_filename_unicode() -> Result<()> {
    let path = Path::new("测试文件.txt");
    let filename = file::get_filename(path)?;

    assert_eq!(filename.as_ref(), "测试文件.txt");
    Ok(())
}

#[test]
fn test_get_filename_directory_path() {
    let temp_dir = tempfile::tempdir_in(crate::TEST_WORKSPACE.as_path()).expect("创建临时目录失败");
    let dir_path = temp_dir.path();

    // 对目录路径调用 get_filename 应该返回错误
    let result = file::get_filename(dir_path);
    assert!(result.is_err());
    assert_error_contains(result, "无法获取文件名");
    temp_dir.close().expect("关闭临时目录失败");
}

#[test]
fn test_get_filename_root_path() {
    let path = Path::new("/");
    let result = file::get_filename(path);

    assert!(result.is_err());
    assert_error_contains(result, "无法获取文件名");
}

#[test]
fn test_get_filename_empty_path() {
    let path = Path::new("");
    let result = file::get_filename(path);

    assert!(result.is_err());
    assert_error_contains(result, "无法获取文件名");
}

#[test]
fn test_calculate_file_sha256_consistency() -> Result<()> {
    let temp_dir = tempfile::tempdir_in(crate::TEST_WORKSPACE.as_path()).expect("创建临时目录失败");

    // 创建测试文件
    let content = generate_random_data(8192); // 8KB随机数据
    let test_file = temp_dir.path().join("random.bin");
    crate::common::write_sync(&test_file, &content)?;

    // 多次计算应该得到相同结果
    let hash1 = file::calculate_file_sha256(&test_file)?;
    let hash2 = file::calculate_file_sha256(&test_file)?;
    let hash3 = file::calculate_file_sha256(&test_file)?;

    assert_eq!(hash1, hash2);
    assert_eq!(hash2, hash3);
    assert_eq!(hash1, hash3);

    temp_dir.close()?;
    Ok(())
}

#[test]
fn test_calculate_file_sha256_different_files() -> Result<()> {
    let temp_dir = tempfile::tempdir_in(crate::TEST_WORKSPACE.as_path()).expect("创建临时目录失败");

    // 创建两个不同文件
    let file1 = temp_dir.path().join("file1.txt");
    let file2 = temp_dir.path().join("file2.txt");

    crate::common::write_sync(&file1, b"Content 1")?;
    crate::common::write_sync(&file2, b"Content 2")?;

    let hash1 = file::calculate_file_sha256(&file1)?;
    let hash2 = file::calculate_file_sha256(&file2)?;

    // 不同内容应该有不同哈希
    assert_ne!(hash1, hash2);

    temp_dir.close()?;
    Ok(())
}

#[test]
fn test_calculate_file_sha256_file_modification() -> Result<()> {
    let temp_dir = tempfile::tempdir_in(crate::TEST_WORKSPACE.as_path()).expect("创建临时目录失败");

    let test_file = temp_dir.path().join("modifiable.txt");

    // 第一次写入
    crate::common::write_sync(&test_file, b"Version 1")?;
    let hash1 = file::calculate_file_sha256(&test_file)?;

    // 修改文件
    crate::common::write_sync(&test_file, b"Version 2")?;
    let hash2 = file::calculate_file_sha256(&test_file)?;

    // 哈希应该不同
    assert_ne!(hash1, hash2);

    temp_dir.close()?;
    Ok(())
}

#[test]
fn test_calculate_file_sha256_symlink() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let temp_dir =
            tempfile::tempdir_in(crate::TEST_WORKSPACE.as_path()).expect("创建临时目录失败");

        // 创建源文件
        let source_file = temp_dir.path().join("source.txt");
        crate::common::write_sync(&source_file, b"Source content").unwrap();

        // 创建符号链接
        let symlink_path = temp_dir.path().join("link.txt");
        symlink(&source_file, &symlink_path).unwrap();

        // 应该计算符号链接指向的文件内容
        let result = file::calculate_file_sha256(&symlink_path);
        assert!(result.is_ok());

        let hash = result.unwrap();
        let expected = file::calculate_file_sha256(&source_file).unwrap();
        assert_eq!(hash, expected);
        temp_dir.close().expect("关闭临时目录失败");
    }

    #[cfg(windows)]
    {
        // Windows符号链接测试（跳过，因为需要特殊权限）
        println!("跳过Windows符号链接测试");
    }
}

#[test]
fn test_get_filename_arc_str_properties() -> Result<()> {
    let path = Path::new("test.txt");
    let filename = file::get_filename(path)?;

    // 检查返回的是Arc<str>
    assert_eq!(filename.as_ref(), "test.txt");

    // 可以克隆而不复制数据
    let clone = filename.clone();
    assert_eq!(filename.as_ref(), clone.as_ref());

    // 使用Arc::ptr_eq检查是否是同一个引用
    use std::sync::Arc;
    let arc_ref1: Arc<str> = filename;
    let arc_ref2: Arc<str> = clone;
    assert!(Arc::ptr_eq(&arc_ref1, &arc_ref2));

    Ok(())
}
