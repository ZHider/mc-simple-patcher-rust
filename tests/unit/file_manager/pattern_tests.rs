//! 正则表达式模式匹配测试
//! 测试 check_pattern_match 函数的功能

use anyhow::Result;
use mc_simple_patcher::config::FileRule;
use mc_simple_patcher::file_manager::check_pattern_match;

#[test]
fn test_check_pattern_match_simple_regex() {
    // 创建包含简单正则表达式的规则
    let rule = FileRule {
        name: None,
        mod_id: None,
        mod_version: None,
        name_pattern: Some(r"^test_.*\.jar$".to_string()),
        url: "https://example.com/test.jar".to_string(),
        sha256: None,
        patches: Vec::new(),
    };

    // 匹配以 test_ 开头，以 .jar 结尾的文件
    assert!(check_pattern_match("test_mod.jar", &rule));
    assert!(check_pattern_match("test_123.jar", &rule));
    assert!(check_pattern_match("test_.jar", &rule));

    // 不匹配的情况
    assert!(!check_pattern_match("other_mod.jar", &rule));
    assert!(!check_pattern_match("test_mod.txt", &rule));
    assert!(!check_pattern_match("atest_mod.jar", &rule)); // 不是以 test_ 开头
}

#[test]
fn test_check_pattern_match_no_pattern_field() {
    // 没有指定 name_pattern 字段时，应该始终匹配
    let rule = FileRule {
        name: None,
        mod_id: None,
        mod_version: None,
        name_pattern: None,
        url: "https://example.com/test.jar".to_string(),
        sha256: None,
        patches: Vec::new(),
    };

    // 任何文件名都应该匹配
    assert!(check_pattern_match("any_file.jar", &rule));
    assert!(check_pattern_match("another_file.txt", &rule));
    assert!(check_pattern_match("test-mod-1.0.0.jar", &rule));
}

#[test]
fn test_check_pattern_match_complex_regex() {
    // 测试更复杂的正则表达式
    let rule = FileRule {
        name: None,
        mod_id: None,
        mod_version: None,
        name_pattern: Some(r"^[a-z]+-\d+\.\d+\.\d+\.jar$".to_string()),
        url: "https://example.com/test.jar".to_string(),
        sha256: None,
        patches: Vec::new(),
    };

    // 匹配小写字母开头，后跟版本号的模式
    assert!(check_pattern_match("mod-1.2.3.jar", &rule));
    assert!(check_pattern_match("test-0.0.1.jar", &rule));
    assert!(check_pattern_match("abc-10.20.30.jar", &rule));

    // 不匹配的情况
    assert!(!check_pattern_match("MOD-1.2.3.jar", &rule)); // 大写字母
    assert!(!check_pattern_match("mod-1.2.jar", &rule)); // 缺少一个版本号
    assert!(!check_pattern_match("mod-1.2.3.4.jar", &rule)); // 版本号太多
    assert!(!check_pattern_match("123-mod.jar", &rule)); // 数字开头
}

#[test]
fn test_check_pattern_match_with_character_classes() {
    // 测试字符类
    let rule = FileRule {
        name: None,
        mod_id: None,
        mod_version: None,
        name_pattern: Some(r"^[A-Za-z0-9_]+\.jar$".to_string()),
        url: "https://example.com/test.jar".to_string(),
        sha256: None,
        patches: Vec::new(),
    };

    // 匹配字母、数字、下划线组成的文件名
    assert!(check_pattern_match("test.jar", &rule));
    assert!(check_pattern_match("Test123.jar", &rule));
    assert!(check_pattern_match("test_mod_1.jar", &rule));
    assert!(check_pattern_match("123.jar", &rule));

    // 不匹配的情况
    assert!(!check_pattern_match("test-mod.jar", &rule)); // 包含连字符
    assert!(!check_pattern_match("test.mod.jar", &rule)); // 包含点号（除了扩展名）
    assert!(!check_pattern_match("test jar.jar", &rule)); // 包含空格
}

#[test]
fn test_check_pattern_match_case_insensitive() {
    // 测试大小写不敏感的正则表达式
    let rule = FileRule {
        name: None,
        mod_id: None,
        mod_version: None,
        name_pattern: Some(r"(?i)^test.*\.jar$".to_string()),
        url: "https://example.com/test.jar".to_string(),
        sha256: None,
        patches: Vec::new(),
    };

    // 大小写不敏感匹配
    assert!(check_pattern_match("test.jar", &rule));
    assert!(check_pattern_match("Test.jar", &rule));
    assert!(check_pattern_match("TEST.jar", &rule));
    assert!(check_pattern_match("TestMod.jar", &rule));
    assert!(check_pattern_match("TEST_MOD.jar", &rule));

    // 不匹配的情况
    assert!(!check_pattern_match("atest.jar", &rule)); // 不是以 test 开头
    assert!(!check_pattern_match("test.txt", &rule)); // 不是 .jar 扩展名
}

#[test]
fn test_check_pattern_match_quantifiers() {
    // 测试量词
    let rule = FileRule {
        name: None,
        mod_id: None,
        mod_version: None,
        name_pattern: Some(r"^mod-\d{1,3}\.\d{1,3}\.\d{1,3}\.jar$".to_string()),
        url: "https://example.com/test.jar".to_string(),
        sha256: None,
        patches: Vec::new(),
    };

    // 匹配 1-3 位数字的版本号
    assert!(check_pattern_match("mod-1.2.3.jar", &rule));
    assert!(check_pattern_match("mod-12.34.56.jar", &rule));
    assert!(check_pattern_match("mod-123.456.789.jar", &rule));

    // 不匹配的情况
    assert!(!check_pattern_match("mod-1234.5.6.jar", &rule)); // 第一位数字超过3位
    assert!(!check_pattern_match("mod-1.2345.6.jar", &rule)); // 第二位数字超过3位
    assert!(!check_pattern_match("mod-1.2.3456.jar", &rule)); // 第三位数字超过3位
    assert!(!check_pattern_match("mod-1.2.jar", &rule)); // 缺少一位版本号
}

#[test]
fn test_check_pattern_match_alternation() {
    // 测试选择结构
    let rule = FileRule {
        name: None,
        mod_id: None,
        mod_version: None,
        name_pattern: Some(r"^(fabric|forge)-.*\.jar$".to_string()),
        url: "https://example.com/test.jar".to_string(),
        sha256: None,
        patches: Vec::new(),
    };

    // 匹配 fabric- 或 forge- 开头的文件
    assert!(check_pattern_match("fabric-mod.jar", &rule));
    assert!(check_pattern_match("forge-mod.jar", &rule));
    assert!(check_pattern_match("fabric-api-1.0.0.jar", &rule));
    assert!(check_pattern_match("forge-1.19.2.jar", &rule));

    // 不匹配的情况
    assert!(!check_pattern_match("mod.jar", &rule)); // 没有前缀
    assert!(!check_pattern_match("quilt-mod.jar", &rule)); // 不是 fabric 或 forge
    assert!(!check_pattern_match("fabricforge.jar", &rule)); // 没有连字符
}

#[test]
fn test_check_pattern_match_combined_with_name() -> Result<()> {
    // 测试在 matches_rule 中 pattern 与 name 的组合
    use mc_simple_patcher::file_manager::matches_rule;
    use mc_simple_patcher::file_manager::modinfo_cache::ModInfoCache;
    use std::collections::{HashMap, HashSet};
    use std::path::Path;

    // 创建测试缓存
    let cache = ModInfoCache {
        sha256: HashSet::new(),
        mod_id: HashMap::new(),
        mod_version: HashMap::new(),
    };

    // 同时指定 name 和 name_pattern（这种情况应该很少见，但需要测试）
    let rule = FileRule {
        name: Some("specific.jar".to_string()),
        mod_id: None,
        mod_version: None,
        name_pattern: Some(r"^.*\.jar$".to_string()), // 匹配所有 .jar 文件
        url: "https://example.com/test.jar".to_string(),
        sha256: None,
        patches: Vec::new(),
    };

    // name 必须精确匹配，pattern 也必须匹配
    let file_path = Path::new("mods/specific.jar");
    assert!(matches_rule(file_path, &rule, &cache)?);

    // 即使 pattern 匹配，name 不匹配也不行
    let file_path = Path::new("mods/other.jar");
    assert!(!matches_rule(file_path, &rule, &cache)?);

    Ok(())
}

#[test]
fn test_check_pattern_match_edge_cases() {
    // 测试边界情况
    let rule = FileRule {
        name: None,
        mod_id: None,
        mod_version: None,
        name_pattern: Some(r"^\.hidden$".to_string()), // 匹配隐藏文件
        url: "https://example.com/test.jar".to_string(),
        sha256: None,
        patches: Vec::new(),
    };

    assert!(check_pattern_match(".hidden", &rule));
    assert!(!check_pattern_match("hidden", &rule));

    // 测试空字符串匹配
    let rule = FileRule {
        name: None,
        mod_id: None,
        mod_version: None,
        name_pattern: Some(r"^$".to_string()), // 匹配空字符串
        url: "https://example.com/test.jar".to_string(),
        sha256: None,
        patches: Vec::new(),
    };

    assert!(check_pattern_match("", &rule));
    assert!(!check_pattern_match("a", &rule));
}

#[test]
#[should_panic(expected = "regex parse error")]
fn test_check_pattern_match_invalid_regex() {
    // 测试无效的正则表达式（应该 panic，因为代码中使用了 unwrap）
    // 注意：在实际代码中，正则表达式应该在配置解析时验证
    let rule = FileRule {
        name: None,
        mod_id: None,
        mod_version: None,
        name_pattern: Some(r"[".to_string()), // 无效的正则表达式
        url: "https://example.com/test.jar".to_string(),
        sha256: None,
        patches: Vec::new(),
    };

    // 这应该 panic，因为 Regex::new 会失败
    check_pattern_match("test.jar", &rule);
}
