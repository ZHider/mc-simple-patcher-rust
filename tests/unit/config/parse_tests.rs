//! 配置解析测试

// use crate::common::*;
use anyhow::Result;
use mc_simple_patcher::config;

#[test]
fn test_parse_valid_config() -> Result<()> {
    // 创建最小有效配置
    let config_content = r#"
metadata = "http://example.com/metadata.toml"
version = 1

[[groups]]
anchor = ".minecraft"
root = "mods"
recursive = false
mirror = false
delete = false

[[groups.files]]
name = "test_mod.jar"
url = "https://example.com/test_mod.jar"
sha256 = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
"#;

    let temp_dir = tempfile::tempdir_in(crate::TEST_WORKSPACE.as_path()).expect("创建临时目录失败");
    let config_path = temp_dir.path().join("test_config.toml");
    crate::common::write_sync(&config_path, config_content).expect("写入配置文件失败");
    let config = config::parse_config(&config_path)?;

    assert_eq!(
        config.metadata_config.metadata,
        Some("http://example.com/metadata.toml".to_string())
    );
    assert_eq!(config.metadata_config.version, Some(1));
    assert_eq!(config.groups.len(), 1);
    assert_eq!(config.groups[0].files.len(), 1);
    assert_eq!(
        config.groups[0].files[0].name,
        Some("test_mod.jar".to_string())
    );

    temp_dir.close()?;
    Ok(())
}

#[test]
fn test_parse_config_with_network_settings() -> Result<()> {
    let config_content = r#"
metadata = "http://example.com/metadata.toml"
version = 1

[network]
quic = true
ignore_invalid_cert = false
timeout = 30
retry = 5

[[groups]]
anchor = ".minecraft"
root = "mods"

[[groups.files]]
name = "test_mod.jar"
url = "https://example.com/test_mod.jar"
"#;

    let temp_dir = tempfile::tempdir_in(crate::TEST_WORKSPACE.as_path()).expect("创建临时目录失败");
    let config_path = temp_dir.path().join("test_config.toml");
    crate::common::write_sync(&config_path, config_content).expect("写入配置文件失败");
    let config = config::parse_config(&config_path)?;

    assert!(config.network.quic);
    assert!(!config.network.ignore_invalid_cert);
    assert_eq!(config.network.timeout, 30);
    assert_eq!(config.network.retry, 5);

    temp_dir.close()?;
    Ok(())
}

#[test]
fn test_parse_config_with_self_update() -> Result<()> {
    let config_content = r#"
metadata = "http://example.com/metadata.toml"
version = 1

[self_update]
url = "https://example.com/updater.exe"

[[self_update.patches]]
url_patch = "https://example.com/patch.bspatch"
sha256_src = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
keep_src = false

[[groups]]
anchor = ".minecraft"
root = "mods"

[[groups.files]]
name = "test_mod.jar"
url = "https://example.com/test_mod.jar"
"#;

    let temp_dir = tempfile::tempdir_in(crate::TEST_WORKSPACE.as_path()).expect("创建临时目录失败");
    let config_path = temp_dir.path().join("test_config.toml");
    crate::common::write_sync(&config_path, config_content).expect("写入配置文件失败");
    let config = config::parse_config(&config_path)?;

    assert_eq!(
        config.self_update.url,
        Some("https://example.com/updater.exe".to_string())
    );
    assert_eq!(config.self_update.patches.len(), 1);
    assert_eq!(
        config.self_update.patches[0].url_patch,
        "https://example.com/patch.bspatch"
    );
    assert!(!config.self_update.patches[0].keep_src);

    temp_dir.close()?;
    Ok(())
}

#[test]
fn test_parse_config_with_file_patches() -> Result<()> {
    let config_content = r#"
metadata = "http://example.com/metadata.toml"
version = 1

[[groups]]
anchor = ".minecraft"
root = "mods"

[[groups.files]]
name = "test_mod.jar"
url = "https://example.com/test_mod.jar"
sha256 = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"

[[groups.files.patches]]
url_patch = "https://example.com/test_mod.patch"
sha256_src = "a3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b856"
keep_src = true
"#;

    let temp_dir = tempfile::tempdir_in(crate::TEST_WORKSPACE.as_path()).expect("创建临时目录失败");
    let config_path = temp_dir.path().join("test_config.toml");
    crate::common::write_sync(&config_path, config_content).expect("写入配置文件失败");
    let config = config::parse_config(&config_path)?;

    assert_eq!(config.groups[0].files[0].patches.len(), 1);
    let patch = &config.groups[0].files[0].patches[0];
    assert_eq!(patch.url_patch, "https://example.com/test_mod.patch");
    assert_eq!(
        patch.sha256_src,
        "a3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b856"
    );
    assert!(patch.keep_src);

    temp_dir.close()?;
    Ok(())
}

#[test]
fn test_parse_invalid_toml() {
    let config_content = r#"
this is not valid toml
{ invalid json style }
"#;

    let temp_dir = tempfile::tempdir_in(crate::TEST_WORKSPACE.as_path()).expect("创建临时目录失败");
    let config_path = temp_dir.path().join("test_config.toml");
    crate::common::write_sync(&config_path, config_content).expect("写入配置文件失败");
    let result = config::parse_config(&config_path);

    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("TOML") || error_msg.contains("parse"));
    temp_dir.close().expect("关闭临时目录失败");
}

#[test]
fn test_parse_missing_required_fields() {
    // 缺少groups字段
    let config_content = r#"
metadata = "http://example.com/metadata.toml"
version = 1
"#;

    let temp_dir = tempfile::tempdir_in(crate::TEST_WORKSPACE.as_path()).expect("创建临时目录失败");
    let config_path = temp_dir.path().join("test_config.toml");
    crate::common::write_sync(&config_path, config_content).expect("写入配置文件失败");
    let result = config::parse_config(&config_path);

    // TOML解析会成功，但groups为空向量
    let config = result.expect("应该解析成功");
    assert!(config.groups.is_empty());
    temp_dir.close().expect("关闭临时目录失败");
}

#[test]
fn test_parse_empty_config() {
    let config_content = "";
    let temp_dir = tempfile::tempdir_in(crate::TEST_WORKSPACE.as_path()).expect("创建临时目录失败");
    let config_path = temp_dir.path().join("test_config.toml");
    crate::common::write_sync(&config_path, config_content).expect("写入配置文件失败");
    let result = config::parse_config(&config_path);

    // 空配置应该解析为默认值
    let config = result.expect("应该解析成功");
    assert!(config.groups.is_empty());
    assert!(config.metadata_config.metadata.is_none());
    assert!(config.metadata_config.version.is_none());
    temp_dir.close().expect("关闭临时目录失败");
}

#[test]
fn test_parse_config_with_multiple_groups() -> Result<()> {
    let config_content = r#"
metadata = "http://example.com/metadata.toml"
version = 1

[[groups]]
anchor = ".minecraft"
root = "mods"
pattern = '^.*\.jar$'

[[groups.files]]
name = "mod1.jar"
url = "https://example.com/mod1.jar"

[[groups.files]]
name = "mod2.jar"
url = "https://example.com/mod2.jar"

[[groups]]
anchor = "configs"
root = "config"
recursive = true

[[groups.files]]
name = "config.json"
url = "https://example.com/config.json"
"#;

    let temp_dir = tempfile::tempdir_in(crate::TEST_WORKSPACE.as_path()).expect("创建临时目录失败");
    let config_path = temp_dir.path().join("test_config.toml");
    crate::common::write_sync(&config_path, config_content).expect("写入配置文件失败");
    let config = config::parse_config(&config_path)?;

    assert_eq!(config.groups.len(), 2);
    assert_eq!(config.groups[0].files.len(), 2);
    assert_eq!(config.groups[1].files.len(), 1);
    assert_eq!(config.groups[0].pattern, Some("^.*\\.jar$".to_string()));
    assert!(config.groups[1].recursive);

    temp_dir.close()?;
    Ok(())
}

#[test]
fn test_parse_config_with_various_file_match_conditions() -> Result<()> {
    let config_content = r#"
metadata = "http://example.com/metadata.toml"
version = 1

[[groups]]
anchor = ".minecraft"
root = "mods"

# 名称匹配
[[groups.files]]
name = "exact_name.jar"
url = "https://example.com/exact_name.jar"

# MOD ID匹配
[[groups.files]]
mod_id = "testmod"
url = "https://example.com/testmod.jar"

# 名称模式匹配
[[groups.files]]
name_pattern = '^prefix_.*\.jar$'
url = "https://example.com/pattern.jar"

# SHA256匹配
[[groups.files]]
sha256 = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
url = "https://example.com/sha256.jar"

# 组合匹配
[[groups.files]]
mod_id = "combined"
mod_version = "1.0.0"
url = "https://example.com/combined.jar"
"#;

    let temp_dir = tempfile::tempdir_in(crate::TEST_WORKSPACE.as_path()).expect("创建临时目录失败");
    let config_path = temp_dir.path().join("test_config.toml");
    crate::common::write_sync(&config_path, config_content).expect("写入配置文件失败");
    let config = config::parse_config(&config_path)?;

    assert_eq!(config.groups[0].files.len(), 5);

    // 检查各种匹配条件
    let files = &config.groups[0].files;
    assert_eq!(files[0].name, Some("exact_name.jar".to_string()));
    assert_eq!(files[1].mod_id, Some("testmod".to_string()));
    assert_eq!(files[2].name_pattern, Some("^prefix_.*\\.jar$".to_string()));
    assert_eq!(
        files[3].sha256,
        Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string())
    );
    assert_eq!(files[4].mod_id, Some("combined".to_string()));
    assert_eq!(files[4].mod_version, Some("1.0.0".to_string()));

    temp_dir.close()?;
    Ok(())
}
