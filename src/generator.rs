//! 配置生成模块
//! 实现从目录扫描结果生成配置文件的功能

use crate::config::FileRule;
use crate::file_manager;
use anyhow::Result;
use std::path::PathBuf;

/// 生成配置文件
pub fn generate_config(
    dir: PathBuf,
    pattern: Option<String>,
    recursive: bool,
    base_url: Option<String>,
    mod_info: bool,
) -> Result<()> {
    log::info!("开始生成配置，目录: {:?}", dir);

    // 检查目录是否存在
    if !dir.exists() {
        anyhow::bail!("指定的目录不存在: {:?}", dir);
    }

    if !dir.is_dir() {
        anyhow::bail!("指定的路径不是目录: {:?}", dir);
    }

    // 保存原始 pattern 用于后续输出
    let pattern_str = pattern.clone();

    // 获取目录中的文件
    let files = file_manager::get_files_in_dir(&dir, recursive)?;

    // 创建一个临时的 FileRule 用于模式匹配
    let temp_rule = FileRule {
        name: None,
        mod_id: None,
        mod_version: None,
        name_pattern: pattern,
        url: String::new(), // 不需要 URL
        sha256: None,
    };

    // 过滤匹配的文件
    let matching_files: Vec<&PathBuf> = files
        .iter()
        .filter(|file_path| {
            // 如果没有提供正则表达式，则匹配所有文件
            if temp_rule.name_pattern.is_none() {
                return true;
            }

            let file_name = file_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("")
                .to_string();

            match file_manager::check_pattern_match(&file_name, &temp_rule) {
                Ok(matches) => matches,
                Err(_) => false, // 如果出现错误，视为不匹配
            }
        })
        .collect();

    // 输出 TOML 格式的配置
    println!("# 由 generate 功能自动生成的配置");
    println!("# 目录: {:?}", dir);
    println!("# 递归: {}", recursive);
    if let Some(ref pat) = pattern_str {
        println!("# 模式: {}", pat);
    }
    if let Some(ref url) = base_url {
        println!("# 基础 URL: {}", url);
    }
    println!("# 提取模组信息: {}", mod_info);
    println!();

    // 输出组配置
    println!("[[groups]]");
    println!("anchor = \"Example.jar\"  # 请根据实际情况修改");
    println!("root = \"mods\"");
    println!("recursive = {}", recursive);
    println!("mirror = true");
    println!("delete = false");
    if let Some(ref pat) = pattern_str {
        println!("pattern = '{}'", pat);
    }
    println!();

    // 为每个匹配的文件生成配置条目
    matching_files.iter().for_each(|file_path| {
        let file_name = file_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("")
            .to_string();

        println!("[[groups.files]]");
        println!("name = \"{}\"", file_name);

        // 如果提供了基础 URL，则生成完整的 URL
        if let Some(ref base) = base_url {
            println!("url = \"{}/{}\"", base, file_name);
        } else {
            println!("url = \"{}\"", file_name); // 占位符 URL
        }

        // 如果启用了模组信息提取，则尝试提取
        if mod_info && file_path.extension().map_or(false, |ext| ext == "jar") {
            match file_manager::extract_mod_info_from_jar(file_path) {
                Ok((mod_id, mod_version)) => {
                    println!("mod_id = \"{}\"", mod_id);
                    println!("mod_version = \"{}\"", mod_version);
                }
                Err(e) => {
                    log::warn!("无法从 {:?} 提取模组信息: {}", file_path, e);
                    // 即使提取失败，也继续处理下一个文件
                }
            }
        }

        // 计算并添加SHA256哈希值
        match calculate_file_sha256(file_path) {
            Ok(sha256) => {
                println!("sha256 = \"{}\"", sha256);
            }
            Err(e) => {
                log::warn!("无法计算 {:?} 的SHA256: {}", file_path, e);
            }
        }

        println!(); // 空行分隔
    });

    log::info!("配置生成完成，共处理 {} 个文件", matching_files.len());
    Ok(())
}

/// 计算文件的SHA256哈希值
fn calculate_file_sha256(file_path: &std::path::Path) -> Result<String> {
    use sha2::{Sha256, Digest};
    use std::fs::File;
    use std::io::Read;

    let mut file = File::open(file_path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 8192]; // 8KB buffer

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break; // 文件读取完毕
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let hash_bytes = hasher.finalize();
    let hash_string = format!("{:x}", hash_bytes);

    Ok(hash_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_generate_config_basic() -> Result<()> {
        // 创建临时目录和文件
        let temp_dir = TempDir::new()?;
        let test_file = temp_dir.path().join("test.jar");
        fs::write(&test_file, "dummy content")?;

        // 测试基本功能
        let result = generate_config(
            temp_dir.path().to_path_buf(),
            Some(r".*\.jar$".to_string()),
            false,
            Some("https://example.com/mods".to_string()),
            false,
        );

        assert!(result.is_ok());

        Ok(())
    }
}
