//! 配置生成模块
//! 实现从目录扫描结果生成配置文件的功能

use anyhow::{Context, Result};
use hex::ToHex;
use indicatif::ProgressBar;
use rayon::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use toml::Value;

use crate::config::GroupConfig;
use crate::file_manager;
use crate::main_controller::DEFAULT_MAX_DEPTH;
use crate::utils::format_error_chain;

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
///
/// # Arguments
///
/// * `toml_file` - 输入的 TOML 配置文件路径
///
/// # Returns
///
/// * `Result<()>` - 成功时返回空值，失败时返回错误
pub async fn generate_config_from_toml(toml_file: PathBuf) -> Result<()> {
    log::info!("开始从 {} 生成配置文件", toml_file.display());

    // 读取并解析生成规则
    let mut generator_config = load_generate_config(&toml_file)?;
    log::info!("成功解析生成规则文件");

    // 提取生成规则列表
    let generator_rules = extract_generator_rules(&generator_config)?;
    let generator_rules_len = generator_rules.len();
    log::info!("共找到 {} 个生成规则", generator_rules_len);

    // 为每个生成规则扫描目录并添加到配置中
    let generated_file_groups: Vec<_> = generator_rules
        .into_par_iter()
        .zip(1..generator_rules_len + 1)
        .map(|rule_idx| {
            let (rule, idx) = rule_idx;
            log::info!(
                "处理第 {} 个生成规则: 锚点={}, 根目录={}",
                idx,
                rule.anchor,
                rule.root
            );
            let new_group = process_generate_rule(&rule);
            if let Err(e) = new_group {
                crate::utils::print_error_chain(&e);
                return None;
            }
            new_group.unwrap()
        })
        .filter(|c| c.is_some())
        .map(|o| o.unwrap())
        .collect();

    if generated_file_groups.is_empty() {
        log::error!("没有处理任何文件组！");
        return Ok(());
    }

    let generated_file_groups_files_len: usize =
        generated_file_groups.iter().map(|g| g.files.len()).sum();

    log::info!("配置文件已成功生成");
    log::info!(
        "总共处理了 {} 个组和 {} 个文件规则",
        generated_file_groups.len(),
        generated_file_groups_files_len
    );

    inject_generator_config(&mut generator_config, generated_file_groups);

    // 生成输出文件
    log::info!("正在写入文件到 {}", toml_file.display());
    write_generated_config(generator_config, &toml_file)?;

    Ok(())
}

// 将 generated_groups 转换为 toml groups table array，更新到generator_config中
/// 将生成的文件组注入到生成器配置中
///
/// # Arguments
///
/// * `generator_config` - 生成器配置的可变引用
/// * `generated_file_groups` - 生成的文件组向量
fn inject_generator_config(
    generator_config: &mut HashMap<String, Value>,
    generated_file_groups: Vec<GroupConfig>,
) {
    generator_config.remove("generate");
    generator_config.insert(
        "groups".to_string(),
        Value::try_from(generated_file_groups).unwrap(),
    );
}

/// 加载生成配置
///
/// # Arguments
///
/// * `toml_file` - TOML 配置文件的路径引用
///
/// # Returns
///
/// * `Result<HashMap<String, Value>>` - 成功时返回配置映射，失败时返回错误
fn load_generate_config(toml_file: &Path) -> Result<HashMap<String, Value>> {
    let generate_rules_str = fs::read_to_string(toml_file)
        .context(format!("读取生成规则文件失败: {}", toml_file.display()))?;

    toml::from_str(&generate_rules_str)
        .context(format!("解析生成规则文件失败: {}", toml_file.display()))
}

/// 提取生成规则列表
///
/// # Arguments
///
/// * `generate_config` - 生成配置的引用
///
/// # Returns
///
/// * `Result<Vec<GenerateRule>>` - 成功时返回生成规则向量，失败时返回错误
fn extract_generator_rules(generate_config: &HashMap<String, Value>) -> Result<Vec<GenerateRule>> {
    generate_config
        .get("generate")
        .and_then(|v| v.as_array())
        .context("生成规则文件中缺少 generate 数组")?
        .iter()
        .map(|v| {
            let rule_map = v.as_table().context("生成规则必须是 Table")?;

            let rule_toml = toml::to_string(rule_map).context("转换生成规则失败")?;
            toml::from_str(&rule_toml).context("解析生成规则失败")
        })
        .collect()
}

/// 处理单个生成规则
///
/// # Arguments
///
/// * `rule` - 生成规则的引用
///
/// # Returns
///
/// * `Result<Option<crate::config::GroupConfig>>` - 成功时返回组配置选项，失败时返回错误
fn process_generate_rule(rule: &GenerateRule) -> Result<Option<crate::config::GroupConfig>> {
    // 查找锚点目录
    let Some(anchor_dir) = crate::file_manager::anchor_finder::find_anchor_optimized(
        &rule.anchor,
        &std::env::current_dir()?,
        DEFAULT_MAX_DEPTH,
    ) else {
        anyhow::bail!("未找到锚点: {}", rule.anchor);
    };

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
        regex::Regex::new(&rule.pattern).context(format!("无效的正则表达式: {}", rule.pattern))?;

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
///
/// # Arguments
///
/// * `rule` - 生成规则的引用
/// * `files` - 文件路径向量
///
/// # Returns
///
/// * `Result<crate::config::GroupConfig>` - 成功时返回组配置，失败时返回错误
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

    // 创建进度跟踪器
    let progress = FileProcessingProgressTracker::new(files.len());

    // 使用 rayon 并行处理文件
    let file_rules: Vec<Result<crate::config::FileRule>> = files
        .into_par_iter()
        .map(|file_path| {
            let result = create_file_rule(rule, &file_path);
            progress.update();
            result
        })
        .collect();

    progress.finish();
    println!();

    // 处理结果
    for file_rule in file_rules {
        match file_rule {
            Ok(rule) => new_group.files.push(rule),
            Err(e) => log::warn!("生成文件规则失败: {}", e),
        }
    }

    new_group.files.sort_by(|a, b| {
        let key_a = a
            .name
            .as_deref()
            .or(a.mod_id.as_deref())
            .or(a.sha256.as_deref())
            .unwrap_or("unknown");
        let key_b = b
            .name
            .as_deref()
            .or(b.mod_id.as_deref())
            .or(b.sha256.as_deref())
            .unwrap_or("unknown");
        key_a.cmp(key_b)
    });

    Ok(new_group)
}

/// 文件处理进度跟踪器
struct FileProcessingProgressTracker {
    pb: Mutex<ProgressBar>,
}

impl FileProcessingProgressTracker {
    pub fn new(total: usize) -> Arc<Self> {
        let pb = ProgressBar::new(total as u64);
        pb.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("[{bar:40.cyan/blue}] {pos}/{len} ({percent}%) {msg}")
                .unwrap()
                .progress_chars("=>-"),
        );
        pb.set_message("文件规则生成中: ");
        Arc::new(Self { pb: Mutex::new(pb) })
    }

    pub fn update(&self) {
        let guard = self.pb.lock().unwrap();
        guard.inc(1);
    }

    pub fn finish(&self) {
        let guard = self.pb.lock().unwrap();
        guard.finish();
    }
}

/// 创建文件规则
///
/// # Arguments
///
/// * `rule` - 生成规则的引用
/// * `file_path` - 文件路径的引用
///
/// # Returns
///
/// * `Result<crate::config::FileRule>` - 成功时返回文件规则，失败时返回错误
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
        patches: Vec::new(),
    };

    // 如果需要，提取 mod_id 和 mod_version
    if (rule.mod_id || rule.mod_version) && file_path.extension().is_some_and(|ext| ext == "jar") {
        log::debug!("尝试从 JAR 文件中提取 mod 信息: {}", file_path.display());

        match file_manager::modinfo_cache::extract_mod_info_from_jar(file_path) {
            Ok((mod_id, mod_version)) => {
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
            }
            Err(e) => {
                log::warn!("无法从 JAR 文件中提取 mod 信息: {}", file_path.display());
                log::debug!("{}", format_error_chain(&e));
            }
        };
    }
    Ok(file_rule)
}

/// 写入生成的配置文件
///
/// # Arguments
///
/// * `generated_config` - 生成的配置映射
/// * `dst_path` - 目标路径的引用
///
/// # Returns
///
/// * `Result<()>` - 成功时返回空值，失败时返回错误
fn write_generated_config(generated_config: HashMap<String, Value>, dst_path: &Path) -> Result<()> {
    // 生成输出文件名
    let output_path = dst_path.with_file_name(format!(
        "{}-generated.toml",
        dst_path
            .file_stem()
            .ok_or_else(|| anyhow::anyhow!("无效的文件名: {}", dst_path.display()))?
            .to_string_lossy()
    ));

    // 将配置写入文件
    log::info!("正在序列化配置并写入文件: {}", output_path.display());
    let config_content = toml::to_string_pretty(&generated_config)?;

    fs::write(&output_path, config_content)
        .context(format!("写入配置文件失败: {}", output_path.display()))?;

    log::info!("配置文件已生成: {}", output_path.display());

    // 生成.sha256文件
    write_sha256(&output_path)?;
    write_sha256(std::env::current_exe()?.as_path())?;

    Ok(())
}

/// 生成sha256文件
///
/// # Arguments
///
/// * `file_path` - 文件路径的引用
///
/// # Returns
///
/// * `Result<()>` - 成功时返回空值，失败时返回错误
pub fn write_sha256(file_path: &Path) -> Result<()> {
    let sha256 =
        crate::utils::calculate_file_sha256(file_path).context("计算配置文件SHA256失败")?;
    let sha256_str = hex::encode(sha256);
    let sha256_path = file_path.with_added_extension("sha256");
    fs::write(&sha256_path, &sha256_str)
        .context(format!("写入配置文件SHA256失败: {}", sha256_path.display()))?;
    log::info!(
        "成功生成配置校验文件 {}\n\tSHA256: {}",
        sha256_path.canonicalize()?.display(),
        sha256_str,
    );
    Ok(())
}
