//! 文件管理模块
//! 实现文件匹配、同步和管理功能

pub mod anchor_finder;
pub mod modinfo_cache;

use crate::config::FileRule;
use crate::file_manager::modinfo_cache::ModInfoCache;
use anyhow::{Context, Result};
use bytes::Bytes;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};

/// 检查文件是否匹配规则
///
/// # Arguments
///
/// * `file_path` - 文件路径的引用
/// * `rule` - 文件规则的引用
/// * `cache` - MOD信息缓存的引用
///
/// # Returns
///
/// * `Result<bool>` - 成功时返回布尔值表示是否匹配，失败时返回错误
pub fn matches_rule(file_path: &Path, rule: &FileRule, cache: &ModInfoCache) -> Result<bool> {
    // log::trace!("matches_rule: {:?}", file_path);
    let file_name = crate::utils::get_filename(file_path)?;

    // 检查名称是否匹配
    let name_matches = check_name_match(&file_name, rule);
    let pattern_matches = check_pattern_match(&file_name, rule);

    // 检查mod信息是否匹配
    let mod_info_matches = check_mod_info_match(&file_name, rule, cache);

    // 检查SHA256是否匹配
    let sha256_matches = match check_sha256_match(rule, cache) {
        Ok(matches) => matches,
        Err(e) => {
            log::error!("检查文件 {} 的SHA256匹配时出错: {}", file_path.display(), e);
            // 总之先返回个true不处理这个文件
            true
        }
    };

    // 由于所有条件至少存在一个，因此即使是默认的true也不会导致误判
    Ok(name_matches && pattern_matches && mod_info_matches && sha256_matches)
}

/// 检查文件名是否匹配（包括禁用文件名）
fn check_name_match(file_name: &str, rule: &FileRule) -> bool {
    if let Some(ref name) = rule.name {
        return file_name == name;
    }
    true // 如果没有指定 name 字段，则认为匹配
}

/// 检查正则表达式是否匹配
///
/// # Arguments
///
/// * `file_name` - 文件名字符串的引用
/// * `rule` - 文件规则的引用
///
/// # Returns
///
/// * `bool` - 如果匹配则返回true，否则返回false
pub fn check_pattern_match(file_name: &str, rule: &FileRule) -> bool {
    if let Some(ref pattern) = rule.name_pattern {
        // Assume regex is valid since it's validated during config parsing
        let re = Regex::new(pattern).unwrap();
        return re.is_match(file_name);
    }
    true // 如果没有指定 name_pattern 字段，则认为匹配
}

/// 检查mod信息是否匹配（使用缓存）
fn check_mod_info_match(file_name: &str, rule: &FileRule, cache: &ModInfoCache) -> bool {
    if rule.mod_id.is_none() {
        return true; // 如果没有指定 mod_id 或 mod_version，则认为匹配
    }
    let mod_id_rule = rule.mod_id.as_deref().unwrap();

    // 检查缓存中是否已有该文件的mod信息
    let mod_id_actual = cache.mod_id.get(file_name);
    if mod_id_actual.is_none() {
        // 不在缓存中说明没有这个mod，直接返回不匹配
        return false;
    }

    let mod_id_cached = mod_id_actual.unwrap().as_ref();
    if mod_id_rule != mod_id_cached {
        // 如果同一个文件的mod id都不同，返回不匹配
        return false;
    } else if rule.mod_version.is_none() {
        return true; // 如果没有指定 mod_version，则认为匹配
    }

    // 检查mod_version是否匹配
    let mod_version_rule = rule.mod_version.as_deref().unwrap();
    if let Some(actual_version) = cache.mod_version.get(mod_id_rule) {
        actual_version.as_ref() == mod_version_rule
    } else {
        false // 如果缓存中没有找到对应的版本信息，则认为不匹配
    }
}

/// 检查文件的SHA256哈希值是否与在缓存中
fn check_sha256_match(rule: &FileRule, cache: &ModInfoCache) -> Result<bool> {
    if rule.sha256.is_none() {
        return Ok(true); // 如果没有指定 sha256，则认为匹配
    }

    let sha256_hex = rule.sha256.as_ref().unwrap();
    let sha256_bytes = Bytes::from(hex::decode(sha256_hex).context("SHA256 Hex 文本解析失败")?);
    Ok(cache.sha256.contains(&sha256_bytes))
}

/// 检查是否存在对应的 .jar.disabled 文件
///
/// # Arguments
///
/// * `file_path` - 文件路径的引用
///
/// # Returns
///
/// * `Option<PathBuf>` - 如果存在则返回禁用文件路径，否则返回 None
pub fn find_disabled_file(file_path: &Path) -> Option<PathBuf> {
    let disabled_path = create_disabled_path(file_path);

    if disabled_path.exists() {
        log::info!("找到对应的 .disabled 文件: {}", disabled_path.display());
        Some(disabled_path)
    } else {
        None
    }
}

/// 创建禁用文件的路径
fn create_disabled_path(file_path: &Path) -> PathBuf {
    if let Some(file_name) = file_path.file_name() {
        file_path.with_file_name(format!("{}.disabled", file_name.to_string_lossy()))
    } else {
        file_path.with_extension("disabled")
    }
}

/// 恢复 .jar.disabled 文件
///
/// # Arguments
///
/// * `disabled_path` - 禁用文件路径的引用
///
/// # Returns
///
/// * `Result<PathBuf>` - 成功时返回恢复后的路径，失败时返回错误
pub fn restore_disabled_file(disabled_path: &Path) -> Result<PathBuf> {
    let restored_path = disabled_path.with_extension("");
    fs::rename(disabled_path, &restored_path)
        .context(format!("无法恢复文件: {:?}", disabled_path))?;
    log::info!("已恢复文件: {}", restored_path.display());
    Ok(restored_path)
}

/// 获取目录中的所有文件
///
/// # Arguments
///
/// * `dir_path` - 目录路径的引用
/// * `recursive` - 是否递归搜索
/// * `rule` - 可选的正则表达式规则引用
///
/// # Returns
///
/// * `Result<Vec<PathBuf>>` - 成功时返回文件路径向量，失败时返回错误
pub fn get_files_in_dir(
    dir_path: &Path,
    recursive: bool,
    rule: Option<&Regex>,
) -> Result<Vec<PathBuf>> {
    let files = if recursive {
        get_recursive_files(dir_path)
    } else {
        get_non_recursive_files(dir_path)
    };

    if let Some(reg) = rule {
        let files: Vec<PathBuf> = files?
            .into_iter()
            .filter(|file| {
                reg.is_match(
                    file.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .as_ref(),
                )
            })
            .collect();
        Ok(files)
    } else {
        files
    }
}

/// 递归获取目录中的所有文件
fn get_recursive_files(dir_path: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(dir_path) {
        let entry = entry?;
        if entry.file_type().is_file() {
            files.push(entry.into_path());
        }
    }
    Ok(files)
}

/// 非递归获取目录中的所有文件
fn get_non_recursive_files(dir_path: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir_path)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            files.push(entry.path());
        }
    }
    Ok(files)
}
