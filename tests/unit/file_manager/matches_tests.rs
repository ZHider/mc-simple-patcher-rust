//! 文件匹配规则测试
//! 测试 matches_rule 和 check_name_match 函数的功能

use anyhow::Result;
use bytes::Bytes;
use mc_simple_patcher::config::FileRule;
use mc_simple_patcher::file_manager::matches_rule;
use mc_simple_patcher::file_manager::modinfo_cache::ModInfoCache;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

#[test]
fn test_matches_rule_name_match() -> Result<()> {
    // 创建测试缓存
    let cache = ModInfoCache {
        sha256: HashSet::new(),
        mod_id: HashMap::new(),
        mod_version: HashMap::new(),
    };

    // 创建精确名称匹配的规则
    let rule = FileRule {
        name: Some("test_mod.jar".to_string()),
        mod_id: None,
        mod_version: None,
        name_pattern: None,
        url: "https://example.com/test_mod.jar".to_string(),
        sha256: None,
        patches: Vec::new(),
    };

    // 测试匹配的文件名
    let file_path = Path::new("mods/test_mod.jar");
    assert!(matches_rule(file_path, &rule, &cache)?);

    // 测试不匹配的文件名
    let file_path = Path::new("mods/other_mod.jar");
    assert!(!matches_rule(file_path, &rule, &cache)?);

    Ok(())
}

#[test]
fn test_matches_rule_no_name_field() -> Result<()> {
    // 没有指定 name 字段时，名称匹配应该返回 true
    let cache = ModInfoCache {
        sha256: HashSet::new(),
        mod_id: HashMap::new(),
        mod_version: HashMap::new(),
    };

    let rule = FileRule {
        name: None,
        mod_id: None,
        mod_version: None,
        name_pattern: None,
        url: "https://example.com/test_mod.jar".to_string(),
        sha256: None,
        patches: Vec::new(),
    };

    let file_path = Path::new("mods/any_file.jar");
    assert!(matches_rule(file_path, &rule, &cache)?);

    Ok(())
}

#[test]
fn test_matches_rule_sha256_match() -> Result<()> {
    // 创建包含 SHA256 的缓存
    let sha256_hex = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    let sha256_bytes = Bytes::from(hex::decode(sha256_hex)?);
    let mut sha256_set = HashSet::new();
    sha256_set.insert(sha256_bytes);

    let cache = ModInfoCache {
        sha256: sha256_set,
        mod_id: HashMap::new(),
        mod_version: HashMap::new(),
    };

    let rule = FileRule {
        name: None,
        mod_id: None,
        mod_version: None,
        name_pattern: None,
        url: "https://example.com/test_mod.jar".to_string(),
        sha256: Some(sha256_hex.to_string()),
        patches: Vec::new(),
    };

    // SHA256 匹配应该返回 true
    let file_path = Path::new("mods/test_mod.jar");
    assert!(matches_rule(file_path, &rule, &cache)?);

    Ok(())
}

#[test]
fn test_matches_rule_sha256_no_match() -> Result<()> {
    // 创建包含不同 SHA256 的缓存
    let sha256_hex = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    let sha256_bytes = Bytes::from(hex::decode(sha256_hex)?);
    let mut sha256_set = HashSet::new();
    sha256_set.insert(sha256_bytes);

    let cache = ModInfoCache {
        sha256: sha256_set,
        mod_id: HashMap::new(),
        mod_version: HashMap::new(),
    };

    // 使用不同的 SHA256
    let rule = FileRule {
        name: None,
        mod_id: None,
        mod_version: None,
        name_pattern: None,
        url: "https://example.com/test_mod.jar".to_string(),
        sha256: Some(
            "a3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b856".to_string(),
        ),
        patches: Vec::new(),
    };

    // SHA256 不匹配应该返回 false
    let file_path = Path::new("mods/test_mod.jar");
    assert!(!matches_rule(file_path, &rule, &cache)?);

    Ok(())
}

#[test]
fn test_matches_rule_invalid_sha256() -> Result<()> {
    let cache = ModInfoCache {
        sha256: HashSet::new(),
        mod_id: HashMap::new(),
        mod_version: HashMap::new(),
    };

    // 使用无效的 SHA256 格式
    let rule = FileRule {
        name: Some("test.jar".to_string()),
        mod_id: None,
        mod_version: None,
        name_pattern: None,
        url: "https://example.com/test.jar".to_string(),
        sha256: Some("invalid_sha256".to_string()),
        patches: Vec::new(),
    };

    let file_path = Path::new("mods/test.jar");
    // 无效的 SHA256 应该记录错误但返回 true（不处理该文件）
    let result = matches_rule(file_path, &rule, &cache);
    assert!(result.is_ok());
    assert!(result?);

    Ok(())
}

#[test]
fn test_matches_rule_combined_conditions() -> Result<()> {
    // 创建包含 mod 信息的缓存
    let mut mod_id_map = HashMap::new();
    mod_id_map.insert(Arc::from("test_mod.jar"), Arc::from("testmod"));
    let mut mod_version_map = HashMap::new();
    mod_version_map.insert(Arc::from("testmod"), Arc::from("1.0.0"));

    let cache = ModInfoCache {
        sha256: HashSet::new(),
        mod_id: mod_id_map,
        mod_version: mod_version_map,
    };

    // 组合条件：name + mod_id + mod_version
    let rule = FileRule {
        name: Some("test_mod.jar".to_string()),
        mod_id: Some("testmod".to_string()),
        mod_version: Some("1.0.0".to_string()),
        name_pattern: None,
        url: "https://example.com/test_mod.jar".to_string(),
        sha256: None,
        patches: Vec::new(),
    };

    let file_path = Path::new("mods/test_mod.jar");
    assert!(matches_rule(file_path, &rule, &cache)?);

    // 版本不匹配
    let rule_wrong_version = FileRule {
        name: Some("test_mod.jar".to_string()),
        mod_id: Some("testmod".to_string()),
        mod_version: Some("2.0.0".to_string()),
        name_pattern: None,
        url: "https://example.com/test_mod.jar".to_string(),
        sha256: None,
        patches: Vec::new(),
    };
    assert!(!matches_rule(file_path, &rule_wrong_version, &cache)?);

    Ok(())
}

#[test]
fn test_check_name_match_exact_name() {
    use mc_simple_patcher::file_manager::check_name_match;

    let rule = FileRule {
        name: Some("exact_match.jar".to_string()),
        mod_id: None,
        mod_version: None,
        name_pattern: None,
        url: "https://example.com/test.jar".to_string(),
        sha256: None,
        patches: Vec::new(),
    };

    // 精确匹配
    assert!(check_name_match("exact_match.jar", &rule));

    // 不匹配
    assert!(!check_name_match("other.jar", &rule));
}

#[test]
fn test_check_name_match_no_name_field() {
    use mc_simple_patcher::file_manager::check_name_match;

    let rule = FileRule {
        name: None,
        mod_id: None,
        mod_version: None,
        name_pattern: None,
        url: "https://example.com/test.jar".to_string(),
        sha256: None,
        patches: Vec::new(),
    };

    // 没有 name 字段时应该始终匹配
    assert!(check_name_match("any_file.jar", &rule));
    assert!(check_name_match("another_file.txt", &rule));
}
