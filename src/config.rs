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

/// 自更新补丁配置
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SelfUpdatePatch {
    /// 补丁文件下载 URL
    pub url_patch: String,
    /// 源文件的 SHA256 哈希值（64 字符十六进制字符串）
    pub sha256_src: String,
    /// 补丁成功后是否保留源文件（重命名为 .backup），默认为 true
    #[serde(default = "default_keep_src")]
    pub keep_src: bool,
}

/// 自更新配置
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SelfUpdateConfig {
    /// 自更新文件下载 URL
    pub url: Option<String>,
    /// 自更新补丁配置
    #[serde(default)]
    pub patches: Vec<SelfUpdatePatch>,
}

/// 文件补丁配置
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FilePatch {
    /// 补丁文件下载 URL
    pub url_patch: String,
    /// 源文件的 SHA256 哈希值（64 字符十六进制字符串）
    pub sha256_src: String,
    /// 补丁成功后是否保留源文件（重命名为 .backup），默认为 true
    #[serde(default = "default_keep_src")]
    pub keep_src: bool,
}

fn default_keep_src() -> bool {
    true
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
    #[serde(default)]
    pub patches: Vec<FilePatch>,
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
    #[serde(default = "metadata_config_retry_default")]
    pub retry: u32,
}

fn default_ignore_invalid_cert() -> bool {
    true
}

pub fn metadata_config_timeout_default() -> u64 {
    15
}

pub fn metadata_config_retry_default() -> u32 {
    3
}

/// 主配置结构
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Config {
    #[serde(flatten)]
    pub metadata_config: MetadataConfig,
    #[serde(default)]
    pub network: NetworkConfig,
    #[serde(default)]
    pub self_update: SelfUpdateConfig,
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

    // 验证 self_update 配置（如果存在）
    if let Some(update_url) = &config.self_update.url
        && (!update_url.starts_with("http://") && !update_url.starts_with("https://"))
    {
        anyhow::bail!("self_update.url must be a valid HTTP or HTTPS URL");
    }

    // 验证 self_update.patches
    for patch in &config.self_update.patches {
        if patch.url_patch.is_empty() {
            anyhow::bail!("url_patch cannot be empty in self_update patch");
        }
        if patch.sha256_src.len() != 64 {
            anyhow::bail!("sha256_src must be a 64-character hex string in self_update patch");
        }
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

            // 验证 patches 数组
            for patch in &file_rule.patches {
                if patch.url_patch.is_empty() {
                    anyhow::bail!("url_patch cannot be empty in patch rule");
                }
                if !patch.url_patch.starts_with("http://")
                    && !patch.url_patch.starts_with("https://")
                {
                    anyhow::bail!("url_patch must be a valid HTTP or HTTPS URL");
                }
                if patch.sha256_src.is_empty() {
                    anyhow::bail!("sha256_src cannot be empty in patch rule");
                }
                if patch.sha256_src.len() != 64
                    || !patch.sha256_src.chars().all(|c| c.is_ascii_hexdigit())
                {
                    anyhow::bail!("sha256_src must be a valid 64-character hex string");
                }
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
        if let Err(e) = &config {
            eprintln!("解析 example.toml 失败：{}", e);
        }
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
            self_update: SelfUpdateConfig::default(),
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
                    patches: Vec::new(),
                }],
            }],
        };

        assert!(validate_config(&valid_config).is_ok());
    }
}
