//! 文件工具模块

use anyhow::{Context, Result};
use bytes::Bytes;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// 计算文件的 SHA256 哈希值
pub fn calculate_file_sha256(file_path: &Path) -> Result<Bytes> {
    let mut file = File::open(file_path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 256 * 1024];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let hash_bytes_array: [u8; 32] = hasher.finalize().into();
    Ok(Bytes::from(Into::<Box<[u8]>>::into(hash_bytes_array)))
}

/// 获取文件名
pub fn get_filename(file_path: &Path) -> Result<std::sync::Arc<str>> {
    Ok(std::sync::Arc::from(
        file_path
            .file_name()
            .context(format!("无法获取文件名：{:?}", file_path))?
            .to_string_lossy(),
    ))
}
