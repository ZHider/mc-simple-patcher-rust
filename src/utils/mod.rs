//! 通用工具模块
//! 包含项目中多个模块共享的通用功能

pub mod downloader;
pub mod logger;

use anyhow::{Context, Error, Result};
use bytes::Bytes;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

/// 计算文件的SHA256哈希值
///
/// # Arguments
///
/// * `file_path` - 文件路径的引用
///
/// # Returns
///
/// * `Result<Bytes>` - 成功时返回文件的SHA256哈希值，失败时返回错误
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
///
/// # Arguments
///
/// * `err` - 错误对象的引用
///
/// # Returns
///
/// 无返回值
pub fn print_error_chain(err: &Error) {
    log::error!("{}", format_error_chain(err));
}

/// 格式化错误信息链
///
/// # Arguments
///
/// * `err` - 错误对象的引用
///
/// # Returns
///
/// * `String` - 格式化后的错误信息链字符串
pub fn format_error_chain(err: &Error) -> String {
    let mut result = "\n === 错误信息链（Caused by）===\n".to_string();
    err.chain().enumerate().for_each(|(i, cause)| {
        result.push_str(format!("  {}. {}\n", i + 1, cause).as_str());
    });
    result.push_str(format!("\n{}\n", "-".repeat(50)).as_str());
    result
}

/// 获取文件名
///
/// # Arguments
///
/// * `file_path` - 文件路径的引用
///
/// # Returns
///
/// * `Result<Arc<str>>` - 成功时返回文件名的原子引用计数字符串，失败时返回错误
pub fn get_filename(file_path: &Path) -> Result<Arc<str>> {
    Ok(Arc::from(
        file_path
            .file_name()
            .context(format!("无法获取文件名: {:?}", file_path))?
            .to_string_lossy(),
    ))
}
