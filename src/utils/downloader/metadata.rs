//! Metadata 更新模块

use std::path::Path;

use anyhow::Result;

use super::download::download_file;
use crate::global_config::get_global_config;

/// 更新 metadata
///
/// # Arguments
///
/// * `dest_path` - 目标路径的引用
///
/// # Returns
///
/// * `Result<bool>` - 成功时返回布尔值表示是否更新了元数据，失败时返回错误
pub async fn update_metadata(dest_path: &Path) -> Result<bool> {
    let config = get_global_config();
    let metadata = config.metadata_config.metadata.as_deref().unwrap();
    download_file(metadata, dest_path, true).await
}
