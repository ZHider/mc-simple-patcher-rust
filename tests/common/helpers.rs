//! 测试辅助函数
//! 提供通用的测试辅助功能

use anyhow::Result;
use std::path::PathBuf;
use tempfile::TempDir;

/// 创建临时测试目录
///
/// # Returns
///
/// * `PathBuf` - 临时目录路径
pub fn create_temp_dir() -> TempDir {
    tempfile::tempdir_in(crate::TEST_WORKSPACE.as_path()).expect("创建临时目录失败")
}

/// 创建测试JAR文件
///
/// # Arguments
///
/// * `mod_id` - MOD ID
/// * `version` - MOD版本
/// * `mod_type` - MOD类型（"fabric" 或 "forge"）
///
/// # Returns
///
/// * `PathBuf` - JAR文件路径
pub fn create_test_jar(mod_id: &str, version: &str, mod_type: &str) -> Result<PathBuf> {
    let temp_dir = create_temp_dir();
    let jar_path = temp_dir.path().join(format!("{}-{}.jar", mod_id, version));

    // 创建临时目录结构
    let meta_dir = temp_dir.path().join("META-INF");
    std::fs::create_dir_all(&meta_dir)?;

    match mod_type {
        "fabric" => {
            // 创建fabric.mod.json
            let fabric_mod = serde_json::json!({
                "schemaVersion": 1,
                "id": mod_id,
                "version": version,
                "name": format!("Test {} Mod", mod_id),
            });
            crate::common::write_sync(
                temp_dir.path().join("fabric.mod.json"),
                serde_json::to_string_pretty(&fabric_mod)?,
            )?;
        }
        "forge" => {
            // 创建mods.toml
            let mods_toml = format!(
                r#"modLoader="javafml"
loaderVersion="[47,)"
license="All rights reserved"
[[mods]]
modId="{}"
version="{}"
displayName="Test {} Mod"
description="A test mod for unit testing"
"#,
                mod_id, version, mod_id
            );
            crate::common::write_sync(temp_dir.path().join("mods.toml"), mods_toml)?;
        }
        _ => anyhow::bail!("不支持的MOD类型: {}", mod_type),
    }

    // 使用zip库创建JAR文件
    use std::fs::File;
    use std::io::Write;
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    let file = File::create(&jar_path)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    // 添加文件到ZIP
    for entry in walkdir::WalkDir::new(&temp_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let relative_path = path.strip_prefix(&temp_dir)?;

        if let Some(name) = relative_path.to_str() {
            zip.start_file(name, options)?;
            let content = std::fs::read(path)?;
            zip.write_all(&content)?;
        }
    }

    zip.finish()?;
    Ok(jar_path)
}

/// 断言错误包含特定消息
///
/// # Arguments
///
/// * `result` - Result值
/// * `expected_message` - 期望的错误消息（部分匹配）
pub fn assert_error_contains<T, E>(result: Result<T, E>, expected_message: &str)
where
    E: std::fmt::Display,
{
    match result {
        Ok(_) => panic!("期望错误但得到了成功结果"),
        Err(e) => {
            let error_str = e.to_string();
            assert!(
                error_str.contains(expected_message),
                "错误消息不包含预期内容。错误: {}, 预期: {}",
                error_str,
                expected_message
            );
        }
    }
}

/// 断言错误包含多个可能的消息（用于跨平台测试）
///
/// # Arguments
///
/// * `result` - Result值
/// * `expected_messages` - 期望的错误消息列表（部分匹配）
pub fn assert_error_contains_any<T, E>(result: Result<T, E>, expected_messages: &[&str])
where
    E: std::fmt::Display,
{
    match result {
        Ok(_) => panic!("期望错误但得到了成功结果"),
        Err(e) => {
            let error_str = e.to_string();
            let found = expected_messages.iter().any(|msg| error_str.contains(msg));
            assert!(
                found,
                "错误消息不包含任何预期内容。错误: {}, 预期列表: {:?}",
                error_str, expected_messages
            );
        }
    }
}

/// 生成随机测试数据
///
/// # Arguments
///
/// * `size` - 数据大小（字节）
///
/// # Returns
///
/// * `Vec<u8>` - 随机数据
pub fn generate_random_data(size: usize) -> Vec<u8> {
    use rand::RngExt;
    let mut rng = rand::rng();
    (0..size).map(|_| rng.random::<u8>()).collect()
}

/// 同步确保文件被写入磁盘
pub fn write_sync<P: AsRef<std::path::Path>, C: AsRef<[u8]>>(
    path: P,
    contents: C,
) -> anyhow::Result<()> {
    use std::io::Write;

    let mut file = std::fs::File::create(path)?;
    file.write_all(contents.as_ref())?;
    file.sync_data()?;
    file.sync_all()?;
    Ok(())
}
