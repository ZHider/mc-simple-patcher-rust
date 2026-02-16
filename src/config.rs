//! 配置文件解析模块
//! 实现对 TOML 配置文件的解析和验证

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// 元数据配置
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
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
    pub sha256: Option<String>,
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

/// 网络配置
#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default)]
pub struct NetworkConfig {
    #[serde(default)]
    pub quic: bool,
    #[serde(default = "default_ignore_invalid_cert")]
    pub ignore_invalid_cert: bool,
    #[serde(default = "metadata_config_timeout_default")]
    pub timeout: u64,
}

fn default_ignore_invalid_cert() -> bool {
    true
}

pub fn metadata_config_timeout_default() -> u64 {
    15
}

/// 主配置结构
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Config {
    #[serde(flatten)]
    pub metadata_config: MetadataConfig,
    #[serde(default)]
    pub network: NetworkConfig,
    pub self_update_url: Option<String>,
    pub groups: Vec<GroupConfig>,
}

/// 解析配置文件
/// 
/// # Arguments
/// 
/// * `path` - 配置文件的路径
/// 
/// # Returns
/// 
/// * `Result<Config>` - 成功时返回解析后的配置对象，失败时返回错误
pub fn parse_config<P: AsRef<Path>>(path: P) -> Result<Config> {
    // log::trace!("{:?}", path.as_ref());
    let contents = fs::read_to_string(path)?;
    let config: Config = toml::from_str(&contents)?;
    validate_config(&config)?;
    Ok(config)
}

/// 验证配置的有效性
/// 
/// # Arguments
/// 
/// * `config` - 待验证的配置对象
/// 
/// # Returns
/// 
/// * `Result<()>` - 验证通过时返回空值，验证失败时返回错误
fn validate_config(config: &Config) -> Result<()> {
    if config.network.quic {
        log::warn!("已开启 HTTP3/QUIC/UDP 协议！");
    }
    if config.network.ignore_invalid_cert {
        log::warn!("已经关闭证书验证，您的通讯可能更容易遭受 MITM 攻击！");
    }

    // 验证 self_update_url（如果存在）
    if let Some(update_url) = &config.self_update_url
        && (!update_url.starts_with("http://") && !update_url.starts_with("https://"))
    {
        anyhow::bail!("self_update_url must be a valid HTTP or HTTPS URL");
    }

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
                || file_rule.name_pattern.is_some()
                || file_rule.sha256.is_some();

            if !has_match_condition {
                anyhow::bail!(
                    "File rule must have at least one match condition (name, mod_id, name_pattern, or sha256)"
                );
            }
        }

        // 如果定义了 pattern 字段，检查其是否为有效的正则表达式
        if let Some(ref pattern) = group.pattern
            && let Err(e) = regex::Regex::new(pattern)
        {
            anyhow::bail!("Invalid regex pattern in group: {}. Error: {}", pattern, e);
        }
    }

    Ok(())
}

/// 获取配置中的元数据信息
/// 
/// # Arguments
/// 
/// * `config` - 配置对象的引用
/// 
/// # Returns
/// 
/// * `String` - 返回格式化的元数据信息字符串
pub fn get_metadata_info(config: &Config) -> String {
    match (
        &config.metadata_config.metadata,
        config.metadata_config.version,
    ) {
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
            network: NetworkConfig::default(),
            self_update_url: None,
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
                    sha256: None,
                }],
            }],
        };

        assert!(validate_config(&valid_config).is_ok());
    }
}
