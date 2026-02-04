//! 文件管理模块
//! 实现文件匹配、同步和管理功能

use std::path::{Path, PathBuf};
use std::fs;
use regex::Regex;
use anyhow::{Context, Result};
use crate::config::FileRule;
use zip::ZipArchive;

/// 检查文件是否匹配规则
pub fn matches_rule(file_path: &Path, rule: &FileRule) -> Result<bool> {
    let file_name = file_path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("无法获取文件名: {:?}", file_path))?
        .to_string_lossy();

    // 尝试不同的匹配方式
    let name_matches = check_name_match(&file_name, rule);
    let pattern_matches = check_pattern_match(&file_name, rule)?;
    let mod_info_matches = check_mod_info_match(file_path, rule)?;

    Ok(name_matches || pattern_matches || mod_info_matches)
}

/// 检查文件名是否匹配
fn check_name_match(file_name: &str, rule: &FileRule) -> bool {
    if let Some(ref name) = rule.name {
        return &file_name == name;
    }
    false
}

/// 检查正则表达式是否匹配
fn check_pattern_match(file_name: &str, rule: &FileRule) -> Result<bool> {
    if let Some(ref pattern) = rule.name_pattern {
        let re = Regex::new(pattern)?;
        return Ok(re.is_match(file_name));
    }
    Ok(false)
}

/// 检查mod信息是否匹配
fn check_mod_info_match(file_path: &Path, rule: &FileRule) -> Result<bool> {
    if rule.mod_id.is_none() || rule.mod_version.is_none() {
        return Ok(false);
    }

    if !file_path.extension().map_or(false, |ext| ext == "jar") {
        return Ok(false);
    }

    match extract_mod_info_from_jar(file_path) {
        Ok((mod_id, mod_version)) => {
            let id_matches = Some(&mod_id) == rule.mod_id.as_ref();
            let version_matches = Some(&mod_version) == rule.mod_version.as_ref();
            Ok(id_matches && version_matches)
        }
        Err(_) => Ok(false), // 如果无法提取mod信息，则认为不匹配
    }
}

/// 从 JAR 文件中提取 mod 信息
pub fn extract_mod_info_from_jar(jar_path: &Path) -> Result<(String, String)> {
    use zip::ZipArchive;
    use std::fs::File;

    let file = File::open(jar_path)?;
    let mut archive = ZipArchive::new(file)?;

    // 查找 META-INF/mods.toml 文件
    let mods_toml_content = find_mods_toml_in_archive(&mut archive)?;

    // 解析 toml 内容
    let toml_value: toml::Value = toml::from_str(&mods_toml_content)?;

    // 提取 mod_id 和 version
    let mod_id = toml_value
        .get("modLoader")
        .and_then(|loader| loader.get("modId"))
        .and_then(|id| id.as_str())
        .ok_or_else(|| anyhow::anyhow!("无法从 mods.toml 中提取 modId"))?
        .to_string();

    let mod_version = toml_value
        .get("modLoader")
        .and_then(|loader| loader.get("version"))
        .and_then(|ver| ver.as_str())
        .ok_or_else(|| anyhow::anyhow!("无法从 mods.toml 中提取 version"))?
        .to_string();

    Ok((mod_id, mod_version))
}

/// 在ZIP存档中查找mods.toml文件
fn find_mods_toml_in_archive(archive: &mut ZipArchive<std::fs::File>) -> Result<String> {
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        if file.name() == "META-INF/mods.toml" {
            let mut content = String::new();
            std::io::Read::read_to_string(&mut file, &mut content)?;
            return Ok(content);
        }
    }

    Err(anyhow::anyhow!("JAR 文件中未找到 META-INF/mods.toml"))
}

/// 检查是否存在对应的 .jar.disabled 文件
pub fn find_disabled_file(file_path: &Path) -> Option<PathBuf> {
    let disabled_path = create_disabled_path(file_path);

    if disabled_path.exists() {
        log::info!("找到对应的 .jar.disabled 文件: {:?}", disabled_path);
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
    log::info!("已恢复文件: {:?}", restored_path);
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
        };

        let matches = matches_rule(&test_file, &rule)?;
        assert!(matches);

        Ok(())
    }
}