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
pub fn parse_config<P: AsRef<Path>>(path: P) -> Result<Config> {
    let contents = fs::read_to_string(path)?;
    let config: Config = toml::from_str(&contents)?;
    validate_config(&config)?;
    Ok(config)
}

/// 验证配置的有效性
fn validate_config(config: &Config) -> Result<()> {
    for group in &config.groups {
        if group.anchor.is_empty() {
            anyhow::bail!("Anchor path cannot be empty in group");
        }
        if group.root.is_empty() {
            anyhow::bail!("Root path cannot be empty in group");
        }

        for file_rule in &group.files {
            if file_rule.url.is_empty() {
                anyhow::bail!("URL cannot be empty in file rule");
            }

            // 检查是否至少有一个匹配条件
            let has_match_condition = file_rule.name.is_some()
                || file_rule.mod_id.is_some()
                || file_rule.name_pattern.is_some();

            if !has_match_condition {
                anyhow::bail!("File rule must have at least one match condition (name, mod_id, or name_pattern)");
            }
        }
    }

    Ok(())
}

/// 获取配置中的元数据信息
pub fn get_metadata_info(config: &Config) -> String {
    match (&config.metadata_config.metadata, config.metadata_config.version) {
        (Some(metadata), Some(version)) => format!("{} v{}", metadata, version),
        (Some(metadata), None) => metadata.clone(),
        (None, Some(version)) => format!("Unknown v{}", version),
        (None, None) => "Unknown".to_string(),
    }
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

    #[test]
    fn test_validate_config() {
        // 测试配置验证功能
        let valid_config = Config {
            metadata_config: MetadataConfig {
                metadata: Some("Test Modpack".to_string()),
                version: Some(1),
            },
            groups: vec![GroupConfig {
                anchor: "mods".to_string(),
                root: "./mods".to_string(),
                recursive: false,
                mirror: false,
                delete: false,
                pattern: None,
                files: vec![FileRule {
                    name: Some("test.jar".to_string()),
                    mod_id: None,
                    mod_version: None,
                    name_pattern: None,
                    url: "https://example.com/test.jar".to_string(),
                }],
            }],
        };

        assert!(validate_config(&valid_config).is_ok());
    }
}