//! 文件管理模块
//! 实现文件匹配、同步和管理功能

use crate::config::FileRule;
use anyhow::{Context, Result};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use zip::ZipArchive;

// 缓存mod信息，避免重复解析JAR文件
type ModInfoCache = HashMap<PathBuf, Option<(String, String)>>;
lazy_static::lazy_static! {
    static ref MOD_INFO_CACHE: std::sync::Mutex<ModInfoCache> =
        std::sync::Mutex::new(HashMap::new());
}

/// 检查文件是否匹配规则
pub fn matches_rule(file_path: &Path, rule: &FileRule) -> Result<bool> {
    let file_name = file_path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("无法获取文件名: {:?}", file_path))?
        .to_string_lossy();

    // 检查名称是否匹配（包括禁用文件名）
    let name_matches = check_name_match_or_disabled(&file_name, rule);
    let pattern_matches = check_pattern_match(&file_name, rule);

    // 检查mod信息是否匹配
    let mod_info_matches = if rule.mod_id.is_some() || rule.mod_version.is_some() {
        check_mod_info_match_cached(file_path, rule)
    } else {
        true // 如果没有指定 mod_id 或 mod_version，则认为匹配
    };

    // 检查SHA256是否匹配
    let sha256_matches = if let Some(ref expected_sha256) = rule.sha256 {
        check_sha256_match(file_path, expected_sha256)?
    } else {
        true // 如果没有指定 sha256，则认为匹配
    };

    // 由于所有条件至少存在一个，因此即使是默认的true也不会导致误判
    Ok(name_matches && pattern_matches && mod_info_matches && sha256_matches)
}

/// 检查文件名是否匹配（包括禁用文件名）
fn check_name_match_or_disabled(file_name: &str, rule: &FileRule) -> bool {
    if let Some(ref name) = rule.name {
        return file_name == name || file_name == format!("{}.disabled", name);
    }
    true // 如果没有指定 name 字段，则认为匹配
}

/// 检查正则表达式是否匹配
pub fn check_pattern_match(file_name: &str, rule: &FileRule) -> bool {
    if let Some(ref pattern) = rule.name_pattern {
        // Assume regex is valid since it's validated during config parsing
        let re = Regex::new(pattern).unwrap();
        return re.is_match(file_name);
    }
    true // 如果没有指定 name_pattern 字段，则认为匹配
}

/// 检查mod信息是否匹配（使用缓存）
fn check_mod_info_match_cached(file_path: &Path, rule: &FileRule) -> bool {
    if file_path.extension().is_none_or(|ext| ext != "jar") {
        return false;
    }

    // 如果缓存中没有，则解析并存储到缓存
    fn extract_info_and_cache(
        file_path: &Path,
        cache: &mut ModInfoCache,
    ) -> Option<(String, String)> {
        log::debug!("Cache miss, 解析JAR文件以提取mod信息...");
        let result = extract_mod_info_from_jar(file_path);

        match result {
            Ok(mod_info) => {
                log::debug!("成功提取mod信息: {:?}", mod_info);
                cache.insert(file_path.to_path_buf(), Some(mod_info.clone()));
                Some(mod_info)
            }
            Err(e) => {
                log::warn!("解析JAR文件时出错: {:?}", e);
                cache.insert(file_path.to_path_buf(), None);
                None
            }
        }
    }

    // 检查缓存中是否已有该文件的mod信息
    let basename = file_path.file_name().unwrap_or(file_path.as_os_str());
    log::debug!("检查缓存中的mod信息，并对缓存加锁: {:?}", basename);
    let mut cache = MOD_INFO_CACHE.lock().unwrap();
    let mod_info_option = match cache.get(file_path) {
        Some(cached_option) => cached_option.clone(),
        None => extract_info_and_cache(file_path, &mut cache),
    };

    drop(cache); // 释放锁
    log::debug!("已释放缓存锁。");

    if let Some((mod_id, mod_version)) = mod_info_option {
        let id_matches = rule.mod_id.as_ref().is_none_or(|id| id == &mod_id);
        let version_matches = rule
            .mod_version
            .as_ref()
            .is_none_or(|ver| ver == &mod_version);
        id_matches && version_matches
    } else {
        false // 如果无法提取mod信息，则认为不匹配
    }
}

/// 检查文件的SHA256哈希值是否与期望值匹配
fn check_sha256_match(file_path: &Path, expected_sha256: &str) -> Result<bool> {
    let actual_sha256 = crate::utils::calculate_file_sha256(file_path)?;
    Ok(actual_sha256.eq_ignore_ascii_case(expected_sha256))
}

/// 从 JAR 文件中提取 mod 信息
pub fn extract_mod_info_from_jar(jar_path: &Path) -> Result<(String, String)> {
    use std::fs::File;
    use zip::ZipArchive;

    let file = File::open(jar_path)?;
    let mut archive = ZipArchive::new(file)?;

    // 查找 META-INF/mods.toml 文件
    let mods_toml_content = find_mods_toml_in_archive(&mut archive)?;

    // 解析 toml 内容
    let toml_value: toml::Value = toml::from_str(&mods_toml_content)?;
    // log::debug!("解析 mods.toml 内容: {:?}", toml_value);

    // 提取 mod_id 和 version
    let mod_base = toml_value
        .get("mods")
        .and_then(|mods| mods.get(0))
        .ok_or_else(|| anyhow::anyhow!("mods.toml 格式不正确，缺少 mods 部分"))?;

    let mod_id = mod_base
        .get("modId")
        .and_then(|id| id.as_str())
        .ok_or_else(|| anyhow::anyhow!("无法从 mods.toml 中提取 modId"))?
        .to_string();

    let mod_version = mod_base
        .get("version")
        .and_then(|ver| ver.as_str())
        .ok_or_else(|| anyhow::anyhow!("无法从 mods.toml 中提取 version"))?
        .to_string();

    Ok((mod_id, mod_version))
}

/// 在ZIP存档中查找mods.toml文件
fn find_mods_toml_in_archive(archive: &mut ZipArchive<std::fs::File>) -> Result<String> {
    let mut file = archive.by_name("META-INF/mods.toml")
        .map_err(|_| anyhow::anyhow!("JAR 文件中未找到 META-INF/mods.toml"))?;
    
    let mut content = String::new();
    std::io::Read::read_to_string(&mut file, &mut content)?;
    Ok(content)
}

/// 检查是否存在对应的 .jar.disabled 文件
pub fn find_disabled_file(file_path: &Path) -> Option<PathBuf> {
    let disabled_path = create_disabled_path(file_path);

    if disabled_path.exists() {
        log::info!("找到对应的 .jar.disabled 文件: {}", disabled_path.display());
        Some(disabled_path)
    } else {
        None
    }
}

/// 创建禁用文件的路径
fn create_disabled_path(file_path: &Path) -> PathBuf {
    if let Some(file_stem) = file_path.file_stem() {
        file_path.with_file_name(format!("{}.jar.disabled", file_stem.to_string_lossy()))
    } else {
        file_path.with_extension("jar.disabled")
    }
}

/// 恢复 .jar.disabled 文件
pub fn restore_disabled_file(disabled_path: &Path) -> Result<PathBuf> {
    let restored_path = disabled_path.with_extension("");
    fs::rename(disabled_path, &restored_path)
        .with_context(|| format!("无法恢复文件: {:?}", disabled_path))?;
    log::info!("已恢复文件: {}", restored_path.display());
    Ok(restored_path)
}

/// 获取目录中的所有文件
pub fn get_files_in_dir(dir_path: &Path, recursive: bool) -> Result<Vec<PathBuf>> {
    if recursive {
        get_recursive_files(dir_path)
    } else {
        get_non_recursive_files(dir_path)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_file_manager() -> Result<()> {
        // 创建临时文件进行测试
        let temp_dir = TempDir::new()?;
        let test_file = temp_dir.path().join("test.jar");
        fs::write(&test_file, "dummy content")?;

        // 创建一个简单的规则进行测试
        let rule = FileRule {
            name: Some("test.jar".to_string()),
            mod_id: None,
            mod_version: None,
            name_pattern: None,
            url: "http://example.com/test.jar".to_string(),
            sha256: None,
        };

        let matches = matches_rule(&test_file, &rule)?;
        assert!(matches);

        Ok(())
    }
}
