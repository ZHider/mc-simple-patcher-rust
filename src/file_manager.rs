//! 文件管理模块
//! 实现文件匹配、同步和管理功能

use std::path::{Path, PathBuf};
use std::fs;
use regex::Regex;
use anyhow::{Context, Result};
use crate::config::FileRule;

/// 文件管理器
pub struct FileManager {}

impl FileManager {
    /// 创建新的文件管理器
    pub fn new() -> Self {
        Self {}
    }

    /// 检查文件是否匹配规则
    pub fn matches_rule(&self, file_path: &Path, rule: &FileRule) -> Result<bool> {
        let file_name = file_path
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("无法获取文件名: {:?}", file_path))?
            .to_string_lossy();

        // 优先使用 name 进行完全匹配
        if let Some(ref name) = rule.name {
            if &file_name == name {
                return Ok(true);
            }
        }

        // 如果有 name_pattern，则使用正则表达式匹配
        if let Some(ref pattern) = rule.name_pattern {
            let re = Regex::new(pattern)?;
            if re.is_match(&file_name) {
                return Ok(true);
            }
        }

        // 如果有 mod_id 和 mod_version，则解析 JAR 文件进行匹配
        if rule.mod_id.is_some() && rule.mod_version.is_some() {
            if file_path.extension().map_or(false, |ext| ext == "jar") {
                if let Ok((mod_id, mod_version)) = self.extract_mod_info_from_jar(file_path) {
                    if Some(&mod_id) == rule.mod_id.as_ref() && Some(&mod_version) == rule.mod_version.as_ref() {
                        return Ok(true);
                    }
                }
            }
        }

        Ok(false)
    }

    /// 从 JAR 文件中提取 mod 信息
    pub fn extract_mod_info_from_jar(&self, jar_path: &Path) -> Result<(String, String)> {
        use std::io::Read;
        use zip::ZipArchive;
        use std::fs::File;

        let file = File::open(jar_path)?;
        let mut archive = ZipArchive::new(file)?;

        // 查找 META-INF/mods.toml 文件
        let mut mods_toml_content = String::new();
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            if file.name() == "META-INF/mods.toml" {
                file.read_to_string(&mut mods_toml_content)?;
                break;
            }
        }

        if mods_toml_content.is_empty() {
            return Err(anyhow::anyhow!("JAR 文件中未找到 META-INF/mods.toml"));
        }

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

    /// 检查是否存在对应的 .jar.disabled 文件
    pub fn find_disabled_file(&self, file_path: &Path) -> Option<PathBuf> {
        if let Some(file_stem) = file_path.file_stem() {
            let disabled_path = file_path.with_file_name(format!("{}.jar.disabled", file_stem.to_string_lossy()));
            if disabled_path.exists() {
                log::info!("找到对应的 .jar.disabled 文件: {:?}", disabled_path);
                return Some(disabled_path);
            }
        }
        None
    }

    /// 恢复 .jar.disabled 文件
    pub fn restore_disabled_file(&self, disabled_path: &Path) -> Result<PathBuf> {
        let restored_path = disabled_path.with_extension("");
        fs::rename(disabled_path, &restored_path)
            .with_context(|| format!("无法恢复文件: {:?}", disabled_path))?;
        log::info!("已恢复文件: {:?}", restored_path);
        Ok(restored_path)
    }

    /// 获取目录中的所有文件
    pub fn get_files_in_dir(&self, dir_path: &Path, recursive: bool) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        
        if recursive {
            for entry in walkdir::WalkDir::new(dir_path) {
                let entry = entry?;
                if entry.file_type().is_file() {
                    files.push(entry.into_path());
                }
            }
        } else {
            for entry in fs::read_dir(dir_path)? {
                let entry = entry?;
                if entry.file_type()?.is_file() {
                    files.push(entry.path());
                }
            }
        }
        
        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_file_manager() -> Result<()> {
        let fm = FileManager::new();
        
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
        
        let matches = fm.matches_rule(&test_file, &rule)?;
        assert!(matches);
        
        Ok(())
    }
}