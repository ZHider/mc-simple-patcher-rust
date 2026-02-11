//! 通用工具模块
//! 包含项目中多个模块共享的通用功能

pub mod downloader;
pub mod logger;

use anyhow::{Error, Result};
use bytes::Bytes;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// 计算文件的SHA256哈希值
pub fn calculate_file_sha256(file_path: &Path) -> Result<Bytes> {
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

    let hash_bytes_array: [u8; 32] = hasher.finalize().into();
    Ok(Bytes::from(Into::<Box<[u8]>>::into(hash_bytes_array)))
}

/// 打印完整的错误信息和错误链
pub fn print_error_chain(err: &Error) {
    log::error!("\n=== 错误信息链（Caused by）===");
    err.chain().enumerate().for_each(|(i, cause)| {
        log::error!("  {}. {}", i + 1, cause);
    });
    log::error!("\n{}\n", "-".repeat(50));
}
