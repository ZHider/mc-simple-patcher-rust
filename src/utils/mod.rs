//! 通用工具模块
//! 包含项目中多个模块共享的通用功能

pub mod downloader;
pub mod logger;

use anyhow::Result;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// 计算文件的SHA256哈希值
pub fn calculate_file_sha256(file_path: &Path) -> Result<String> {
    let mut file = File::open(file_path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 256 * 1024]; // 256KB buffer

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break; // 文件读取完毕
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let hash_bytes = hasher.finalize();
    Ok(format!("{:x}", hash_bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_calculate_file_sha256() -> Result<()> {
        // 创建临时文件进行测试
        let temp_dir = TempDir::new()?;
        let test_file = temp_dir.path().join("test.txt");

        // 写入测试内容 "hello world" (不含换行符)
        let mut file = File::create(&test_file)?;
        file.write_all(b"hello world")?;

        let expected_hash = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        let actual_hash = calculate_file_sha256(&test_file)?;

        assert_eq!(actual_hash, expected_hash);
        Ok(())
    }
}
