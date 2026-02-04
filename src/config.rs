//! 配置文件解析模块
//! 实现对 TOML 配置文件的解析和验证

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use anyhow::Result;

/// 元数据配置
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MetadataConfig {
    pub metadata: Option<String>,
    pub version: Option<u32>,
}

/// 文件匹配规则
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileRule {
    pub name: Option<String>,
    pub mod_id: Option<String>,
    pub mod_version: Option<String>,
    pub name_pattern: Option<String>,
    pub url: String,
}

/// 组配置
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GroupConfig {
    pub anchor: String,
    pub root: String,
    #[serde(default)]
    pub recursive: bool,
    #[serde(default)]
    pub mirror: bool,
    #[serde(default)]
    pub delete: bool,
    #[serde(default)]
    pub pattern: Option<String>,
    #[serde(default)]
    pub files: Vec<FileRule>,
}

/// 主配置结构
#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    #[serde(flatten)]
    pub metadata_config: MetadataConfig,
    pub groups: Vec<GroupConfig>,
}

/// 解析配置文件
pub fn parse_config<P: AsRef<Path>>(path: P) -> Result<Config, Box<dyn std::error::Error>> {
    let contents = fs::read_to_string(path)?;
    let config: Config = toml::from_str(&contents)?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_example_config() {
        // 测试解析示例配置文件
        let config = parse_config("example.toml");
        assert!(config.is_ok());
    }
}