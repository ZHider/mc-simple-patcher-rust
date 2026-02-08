//! 配置生成模块
//! 实现从目录扫描结果生成配置文件的功能

use anyhow::Result;
use hex::ToHex;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::file_manager;

/// 生成配置的规则定义
#[derive(Debug, Deserialize)]
struct GenerateRule {
    anchor: String,
    root: String,
    pattern: String,
    recursive: bool,
    url_base: Option<String>,
    name: bool,
    mod_id: bool,
    mod_version: bool,
    sha256: bool,
}

/// 从 TOML 文件生成配置
pub async fn generate_config_from_toml(toml_file: PathBuf) -> Result<()> {
    log::info!("开始从 {} 生成配置文件", toml_file.display());

    // 读取并解析生成规则
    let generate_config = load_generate_config(&toml_file)?;
    log::info!("成功解析生成规则文件");

    // 提取生成规则列表
    let generate_rules = extract_generate_rules(&generate_config)?;
    log::info!("共找到 {} 个生成规则", generate_rules.len());

    // 读取原始配置文件（如果存在）以保留元数据
    let mut base_config = load_base_config(&generate_config, &toml_file)?;
    log::info!("已加载基础配置");

    // 为每个生成规则扫描目录并添加到配置中
    for (idx, rule) in generate_rules.iter().enumerate() {
        log::info!(
            "处理第 {} 个生成规则: 锚点={}, 根目录={}",
            idx + 1,
            rule.anchor,
            rule.root
        );

        if let Some(new_group) = process_generate_rule(rule).await? {
            base_config.groups.push(new_group);
        }
    }

    // 生成输出文件
    write_generated_config(&base_config, &toml_file)?;

    log::info!("配置文件已成功生成");
    log::info!(
        "总共处理了 {} 个组和 {} 个文件规则",
        base_config.groups.len(),
        base_config
            .groups
            .iter()
            .map(|g| g.files.len())
            .sum::<usize>()
    );

    Ok(())
}

/// 加载生成配置
fn load_generate_config(toml_file: &Path) -> Result<HashMap<String, serde_json::Value>> {
    let generate_rules_str = fs::read_to_string(toml_file)
        .map_err(|e| anyhow::anyhow!("读取生成规则文件失败: {}", e))?;

    toml::from_str(&generate_rules_str).map_err(|e| anyhow::anyhow!("解析生成规则文件失败: {}", e))
}

/// 提取生成规则列表
fn extract_generate_rules(
    generate_config: &HashMap<String, serde_json::Value>,
) -> Result<Vec<GenerateRule>> {
    generate_config
        .get("generate")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("生成规则文件中缺少 generate 数组"))?
        .iter()
        .map(|v| {
            let rule_map = v
                .as_object()
                .ok_or_else(|| anyhow::anyhow!("生成规则必须是表格格式"))?;

            let rule_toml = toml::to_string(rule_map)
                .map_err(|e| anyhow::anyhow!("转换生成规则失败: {}", e))?;
            toml::from_str(&rule_toml).map_err(|e| anyhow::anyhow!("解析生成规则失败: {}", e))
        })
        .collect()
}

/// 加载基础配置
fn load_base_config(
    generate_config: &HashMap<String, serde_json::Value>,
    toml_file: &Path,
) -> Result<Config> {
    if generate_config.contains_key("metadata") || generate_config.contains_key("version") {
        log::info!("从生成规则文件中读取到元数据信息");
        Ok(Config {
            metadata_config: crate::config::MetadataConfig {
                metadata: generate_config
                    .get("metadata")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                version: generate_config
                    .get("version")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u32),
            },
            groups: vec![],
        })
    } else {
        log::info!("未在生成规则文件中找到元数据信息，尝试从默认配置文件加载");
        let default_config_path = toml_file
            .parent()
            .unwrap_or(Path::new("."))
            .join("mc_simple_patcher.toml");
        if default_config_path.exists() {
            match crate::config::parse_config(&default_config_path) {
                Ok(config) => {
                    log::info!("成功从默认配置文件加载元数据");
                    Ok(config)
                }
                Err(_) => {
                    log::warn!("无法解析默认配置文件，使用默认元数据");
                    Ok(Config {
                        metadata_config: crate::config::MetadataConfig {
                            metadata: None,
                            version: Some(0),
                        },
                        groups: vec![],
                    })
                }
            }
        } else {
            log::warn!("未找到默认配置文件，使用默认元数据");
            Ok(Config {
                metadata_config: crate::config::MetadataConfig {
                    metadata: None,
                    version: Some(0),
                },
                groups: vec![],
            })
        }
    }
}

/// 处理单个生成规则
async fn process_generate_rule(rule: &GenerateRule) -> Result<Option<crate::config::GroupConfig>> {
    // 查找锚点目录
    let anchor_dir = crate::file_manager::anchor_finder::find_anchor_optimized(
        &rule.anchor,
        &std::env::current_dir()?,
        10,
    )? // 使用默认最大深度
    .ok_or_else(|| anyhow::anyhow!("未找到锚点: {}", rule.anchor))?;

    // 构建工作目录路径
    let work_dir = anchor_dir.join(&rule.root);
    if !work_dir.exists() {
        log::warn!("工作目录不存在: {}", work_dir.display());
        return Ok(None);
    }

    log::info!(
        "在目录 {} 中搜索匹配 '{}' 的文件",
        work_dir.display(),
        rule.pattern
    );

    // 使用 file_manager 中的函数扫描目录
    let regex_pattern =
        regex::Regex::new(&rule.pattern).map_err(|e| anyhow::anyhow!("无效的正则表达式: {}", e))?;

    let files = file_manager::get_files_in_dir(&work_dir, rule.recursive, Some(&regex_pattern))?;
    log::info!("找到 {} 个匹配的文件", files.len());

    // 创建新的组
    let new_group = create_group_config(rule, files)?;
    log::info!(
        "为组 '{}' 添加了 {} 个文件规则",
        rule.anchor,
        new_group.files.len()
    );

    Ok(Some(new_group))
}

/// 创建组配置
fn create_group_config(
    rule: &GenerateRule,
    files: Vec<PathBuf>,
) -> Result<crate::config::GroupConfig> {
    let mut new_group = crate::config::GroupConfig {
        anchor: rule.anchor.clone(),
        root: rule.root.clone(),
        recursive: rule.recursive,
        mirror: false, // 默认不启用镜像模式
        delete: false, // 默认不删除文件
        pattern: Some(rule.pattern.clone()),
        files: Vec::new(),
    };

    // 为每个找到的文件创建配置条目
    for file_path in files {
        let file_rule = create_file_rule(rule, &file_path)?;
        new_group.files.push(file_rule);
    }

    Ok(new_group)
}

/// 创建文件规则
fn create_file_rule(rule: &GenerateRule, file_path: &Path) -> Result<crate::config::FileRule> {
    log::debug!("处理文件: {}", file_path.display());

    let file_name = file_path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("无法获取文件名: {}", file_path.display()))?
        .to_string_lossy()
        .to_string();

    // 构建 URL
    let url = if let Some(ref url_base) = rule.url_base {
        format!("{}{}", url_base, file_name)
    } else {
        "".to_string() // URL 需要用户手动填写
    };

    // 计算 SHA256
    let sha256 = if rule.sha256 {
        match crate::utils::calculate_file_sha256(file_path) {
            Ok(hash) => {
                let hash_str = hash.encode_hex();
                log::debug!("计算文件 {} 的 SHA256: {}", file_path.display(), hash_str);
                Some(hash_str)
            }
            Err(e) => {
                log::warn!("无法计算文件 {} 的 SHA256: {}", file_path.display(), e);
                None
            }
        }
    } else {
        None
    };

    let mut file_rule = crate::config::FileRule {
        name: if rule.name {
            Some(file_name.clone())
        } else {
            None
        },
        mod_id: None,
        mod_version: None,
        name_pattern: None,
        url,
        sha256,
    };

    // 如果需要，提取 mod_id 和 mod_version
    if (rule.mod_id || rule.mod_version) && file_path.extension().is_some_and(|ext| ext == "jar") {
        log::debug!("尝试从 JAR 文件中提取 mod 信息: {}", file_path.display());
        if let Ok((mod_id, mod_version)) = file_manager::extract_mod_info_from_jar(file_path) {
            log::debug!(
                "成功提取 mod 信息: mod_id={}, mod_version={}",
                mod_id,
                mod_version
            );

            if rule.mod_id {
                file_rule.mod_id = Some(mod_id.clone());
            }
            if rule.mod_version {
                file_rule.mod_version = Some(mod_version.clone());
            }
        } else {
            log::warn!("无法从 JAR 文件中提取 mod 信息: {}", file_path.display());
        }
    }

    Ok(file_rule)
}

/// 写入生成的配置文件
fn write_generated_config(base_config: &Config, toml_file: &Path) -> Result<()> {
    // 生成输出文件名
    let output_path = toml_file.with_file_name(format!(
        "{}-generated.toml",
        toml_file
            .file_stem()
            .ok_or_else(|| anyhow::anyhow!("无效的文件名: {}", toml_file.display()))?
            .to_string_lossy()
    ));

    // 将配置写入文件
    log::info!("正在序列化配置并写入文件: {}", output_path.display());
    let config_content = toml::to_string_pretty(base_config)
        .map_err(|e| anyhow::anyhow!("序列化配置失败: {}", e))?;

    fs::write(&output_path, config_content)
        .map_err(|e| anyhow::anyhow!("写入配置文件失败: {}", e))?;

    log::info!("配置文件已生成: {}", output_path.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_generate_config_from_toml_basic() -> Result<()> {
        // 创建临时目录
        let temp_dir = TempDir::new()?;
        let temp_path = temp_dir.path();

        // 创建一个模拟的 generate.toml 文件
        let generate_toml_path = temp_path.join("test_generate.toml");
        let generate_content = r#"metadata = "http://example.com/config.toml"
version = 1

[[generate]]
anchor = "test_anchor.txt"
root = "."
pattern = '.*\.txt$'
recursive = false
url_base = "http://example.com/mods/"
name = true
mod_id = false
mod_version = false
sha256 = true
"#;
        fs::write(&generate_toml_path, generate_content)?;

        // 创建锚点文件
        let anchor_file = temp_path.join("test_anchor.txt");
        fs::write(&anchor_file, "anchor content")?;

        // 创建一些测试文件
        let test_file1 = temp_path.join("test1.txt");
        fs::write(&test_file1, "test content 1")?;

        let test_file2 = temp_path.join("test2.txt");
        fs::write(&test_file2, "test content 2")?;

        // 切换到临时目录以确保锚点查找正常工作
        let original_dir = std::env::current_dir()?;
        std::env::set_current_dir(temp_path)?;

        // 运行生成函数
        let result = generate_config_from_toml(generate_toml_path).await;

        // 恢复原始目录
        std::env::set_current_dir(original_dir)?;

        // 检查是否成功
        assert!(result.is_ok());

        // 检查是否生成了输出文件
        let output_path = temp_path.join("test_generate-generated.toml");
        assert!(output_path.exists());

        // 读取并验证生成的配置
        let generated_content = fs::read_to_string(output_path)?;
        assert!(generated_content.contains("metadata"));
        assert!(generated_content.contains("version"));

        Ok(())
    }

    #[tokio::test]
    async fn test_generate_config_from_toml_invalid_file() -> Result<()> {
        // 创建一个不存在的文件路径
        let invalid_path = PathBuf::from("/non/existent/file.toml");

        // 运行生成函数，应该返回错误
        let result = generate_config_from_toml(invalid_path).await;

        // 检查是否返回错误
        assert!(result.is_err());

        Ok(())
    }
}
