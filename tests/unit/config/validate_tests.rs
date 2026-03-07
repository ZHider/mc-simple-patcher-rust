//! 配置验证测试

use crate::common::*;
use mc_simple_patcher::config;
use toml;

#[test]
fn test_validate_empty_anchor() {
    let config = config::Config {
        groups: vec![config::GroupConfig {
            anchor: "".to_string(), // 空锚点
            root: "mods".to_string(),
            recursive: false,
            mirror: false,
            delete: false,
            pattern: None,
            files: vec![],
        }],
        ..Default::default()
    };

    // validate_config是私有函数，我们通过parse_config来测试
    // 创建临时配置文件来测试验证逻辑
    let temp_dir = tempfile::tempdir_in(crate::TEST_WORKSPACE.as_path()).expect("创建临时目录失败");
    let config_path = temp_dir.path().join("test_config.toml");
    let config_str = toml::to_string(&config).expect("序列化配置失败");
    crate::common::write_sync(&config_path, config_str).expect("写入配置文件失败");

    let result = config::parse_config(&config_path);
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("Anchor") || error_msg.contains("empty"));
    temp_dir.close().expect("关闭临时目录失败");
}

#[test]
fn test_validate_empty_root() {
    let config = config::Config {
        groups: vec![config::GroupConfig {
            anchor: ".minecraft".to_string(),
            root: "".to_string(), // 空根目录
            recursive: false,
            mirror: false,
            delete: false,
            pattern: None,
            files: vec![],
        }],
        ..Default::default()
    };

    // validate_config是私有函数，我们通过parse_config来测试
    // 创建临时配置文件来测试验证逻辑
    let temp_dir = tempfile::tempdir_in(crate::TEST_WORKSPACE.as_path()).expect("创建临时目录失败");
    let config_path = temp_dir.path().join("test_config.toml");
    let config_str = toml::to_string(&config).expect("序列化配置失败");
    crate::common::write_sync(&config_path, config_str).expect("写入配置文件失败");

    let result = config::parse_config(&config_path);
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("Root") || error_msg.contains("empty"));
    temp_dir.close().expect("关闭临时目录失败");
}

#[test]
fn test_validate_empty_url() {
    let config = config::Config {
        groups: vec![config::GroupConfig {
            anchor: ".minecraft".to_string(),
            root: "mods".to_string(),
            recursive: false,
            mirror: false,
            delete: false,
            pattern: None,
            files: vec![config::FileRule {
                name: Some("test.jar".to_string()),
                mod_id: None,
                mod_version: None,
                name_pattern: None,
                url: "".to_string(), // 空URL
                sha256: None,
                patches: vec![],
            }],
        }],
        ..Default::default()
    };

    // validate_config是私有函数，我们通过parse_config来测试
    // 创建临时配置文件来测试验证逻辑
    let temp_dir = tempfile::tempdir_in(crate::TEST_WORKSPACE.as_path()).expect("创建临时目录失败");
    let config_path = temp_dir.path().join("test_config.toml");
    let config_str = toml::to_string(&config).expect("序列化配置失败");
    crate::common::write_sync(&config_path, config_str).expect("写入配置文件失败");

    let result = config::parse_config(&config_path);
    assert_error_contains(result, "URL cannot be empty");
    temp_dir.close().expect("关闭临时目录失败");
}

#[test]
fn test_validate_no_match_condition() {
    let config = config::Config {
        groups: vec![config::GroupConfig {
            anchor: ".minecraft".to_string(),
            root: "mods".to_string(),
            recursive: false,
            mirror: false,
            delete: false,
            pattern: None,
            files: vec![config::FileRule {
                name: None,
                mod_id: None,
                mod_version: None,
                name_pattern: None,
                url: "https://example.com/test.jar".to_string(),
                sha256: None, // 没有任何匹配条件
                patches: vec![],
            }],
        }],
        ..Default::default()
    };

    // validate_config是私有函数，我们通过parse_config来测试
    // 创建临时配置文件来测试验证逻辑
    let temp_dir = tempfile::tempdir_in(crate::TEST_WORKSPACE.as_path()).expect("创建临时目录失败");
    let config_path = temp_dir.path().join("test_config.toml");
    let config_str = toml::to_string(&config).expect("序列化配置失败");
    crate::common::write_sync(&config_path, config_str).expect("写入配置文件失败");

    let result = config::parse_config(&config_path);
    assert_error_contains(result, "must have at least one match condition");
    temp_dir.close().expect("关闭临时目录失败");
}

#[test]
fn test_validate_invalid_regex_pattern() {
    let config = config::Config {
        groups: vec![config::GroupConfig {
            anchor: ".minecraft".to_string(),
            root: "mods".to_string(),
            recursive: false,
            mirror: false,
            delete: false,
            pattern: Some("[invalid regex".to_string()), // 无效正则
            files: vec![],
        }],
        ..Default::default()
    };

    // validate_config是私有函数，我们通过parse_config来测试
    // 创建临时配置文件来测试验证逻辑
    let temp_dir = tempfile::tempdir_in(crate::TEST_WORKSPACE.as_path()).expect("创建临时目录失败");
    let config_path = temp_dir.path().join("test_config.toml");
    let config_str = toml::to_string(&config).expect("序列化配置失败");
    crate::common::write_sync(&config_path, config_str).expect("写入配置文件失败");

    let result = config::parse_config(&config_path);
    assert_error_contains(result, "Invalid regex pattern");
    temp_dir.close().expect("关闭临时目录失败");
}

#[test]
fn test_validate_self_update_invalid_url() {
    let config = config::Config {
        self_update: config::SelfUpdateConfig {
            url: Some("not-a-url".to_string()), // 无效URL
            patches: vec![],
        },
        groups: vec![],
        ..Default::default()
    };

    // validate_config是私有函数，我们通过parse_config来测试
    // 创建临时配置文件来测试验证逻辑
    let temp_dir = tempfile::tempdir_in(crate::TEST_WORKSPACE.as_path()).expect("创建临时目录失败");
    let config_path = temp_dir.path().join("test_config.toml");
    let config_str = toml::to_string(&config).expect("序列化配置失败");
    crate::common::write_sync(&config_path, config_str).expect("写入配置文件失败");

    let result = config::parse_config(&config_path);
    assert_error_contains(result, "must be a valid HTTP or HTTPS URL");
    temp_dir.close().expect("关闭临时目录失败");
}

#[test]
fn test_validate_self_update_patch_empty_url() {
    let config = config::Config {
        self_update: config::SelfUpdateConfig {
            url: None,
            patches: vec![config::SelfUpdatePatch {
                url_patch: "".to_string(), // 空URL
                sha256_src: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
                    .to_string(),
                keep_src: true,
            }],
        },
        groups: vec![],
        ..Default::default()
    };

    // validate_config是私有函数，我们通过parse_config来测试
    // 创建临时配置文件来测试验证逻辑
    let temp_dir = tempfile::tempdir_in(crate::TEST_WORKSPACE.as_path()).expect("创建临时目录失败");
    let config_path = temp_dir.path().join("test_config.toml");
    let config_str = toml::to_string(&config).expect("序列化配置失败");
    crate::common::write_sync(&config_path, config_str).expect("写入配置文件失败");

    let result = config::parse_config(&config_path);
    assert_error_contains(result, "url_patch cannot be empty");
    temp_dir.close().expect("关闭临时目录失败");
}

#[test]
fn test_validate_self_update_patch_invalid_sha256_length() {
    let config = config::Config {
        self_update: config::SelfUpdateConfig {
            url: None,
            patches: vec![config::SelfUpdatePatch {
                url_patch: "https://example.com/patch.bspatch".to_string(),
                sha256_src: "short".to_string(), // 长度不足
                keep_src: true,
            }],
        },
        groups: vec![],
        ..Default::default()
    };

    // validate_config是私有函数，我们通过parse_config来测试
    // 创建临时配置文件来测试验证逻辑
    let temp_dir = tempfile::tempdir_in(crate::TEST_WORKSPACE.as_path()).expect("创建临时目录失败");
    let config_path = temp_dir.path().join("test_config.toml");
    let config_str = toml::to_string(&config).expect("序列化配置失败");
    crate::common::write_sync(&config_path, config_str).expect("写入配置文件失败");

    let result = config::parse_config(&config_path);
    assert_error_contains(result, "must be a 64-character hex string");
    temp_dir.close().expect("关闭临时目录失败");
}

#[test]
fn test_validate_file_patch_empty_url() {
    let config = config::Config {
        groups: vec![config::GroupConfig {
            anchor: ".minecraft".to_string(),
            root: "mods".to_string(),
            recursive: false,
            mirror: false,
            delete: false,
            pattern: None,
            files: vec![config::FileRule {
                name: Some("test.jar".to_string()),
                mod_id: None,
                mod_version: None,
                name_pattern: None,
                url: "https://example.com/test.jar".to_string(),
                sha256: None,
                patches: vec![config::FilePatch {
                    url_patch: "".to_string(), // 空URL
                    sha256_src: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
                        .to_string(),
                    keep_src: true,
                }],
            }],
        }],
        ..Default::default()
    };

    // validate_config是私有函数，我们通过parse_config来测试
    // 创建临时配置文件来测试验证逻辑
    let temp_dir = tempfile::tempdir_in(crate::TEST_WORKSPACE.as_path()).expect("创建临时目录失败");
    let config_path = temp_dir.path().join("test_config.toml");
    let config_str = toml::to_string(&config).expect("序列化配置失败");
    crate::common::write_sync(&config_path, config_str).expect("写入配置文件失败");

    let result = config::parse_config(&config_path);
    assert_error_contains(result, "url_patch cannot be empty");
    temp_dir.close().expect("关闭临时目录失败");
}

#[test]
fn test_validate_file_patch_invalid_url() {
    let config = config::Config {
        groups: vec![config::GroupConfig {
            anchor: ".minecraft".to_string(),
            root: "mods".to_string(),
            recursive: false,
            mirror: false,
            delete: false,
            pattern: None,
            files: vec![config::FileRule {
                name: Some("test.jar".to_string()),
                mod_id: None,
                mod_version: None,
                name_pattern: None,
                url: "https://example.com/test.jar".to_string(),
                sha256: None,
                patches: vec![config::FilePatch {
                    url_patch: "ftp://example.com/patch.bspatch".to_string(), // 非HTTP URL
                    sha256_src: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
                        .to_string(),
                    keep_src: true,
                }],
            }],
        }],
        ..Default::default()
    };

    // validate_config是私有函数，我们通过parse_config来测试
    // 创建临时配置文件来测试验证逻辑
    let temp_dir = tempfile::tempdir_in(crate::TEST_WORKSPACE.as_path()).expect("创建临时目录失败");
    let config_path = temp_dir.path().join("test_config.toml");
    let config_str = toml::to_string(&config).expect("序列化配置失败");
    crate::common::write_sync(&config_path, config_str).expect("写入配置文件失败");

    let result = config::parse_config(&config_path);
    assert_error_contains(result, "must be a valid HTTP or HTTPS URL");
    temp_dir.close().expect("关闭临时目录失败");
}

#[test]
fn test_validate_file_patch_empty_sha256() {
    let config = config::Config {
        groups: vec![config::GroupConfig {
            anchor: ".minecraft".to_string(),
            root: "mods".to_string(),
            recursive: false,
            mirror: false,
            delete: false,
            pattern: None,
            files: vec![config::FileRule {
                name: Some("test.jar".to_string()),
                mod_id: None,
                mod_version: None,
                name_pattern: None,
                url: "https://example.com/test.jar".to_string(),
                sha256: None,
                patches: vec![config::FilePatch {
                    url_patch: "https://example.com/patch.bspatch".to_string(),
                    sha256_src: "".to_string(), // 空SHA256
                    keep_src: true,
                }],
            }],
        }],
        ..Default::default()
    };

    // validate_config是私有函数，我们通过parse_config来测试
    // 创建临时配置文件来测试验证逻辑
    let temp_dir = tempfile::tempdir_in(crate::TEST_WORKSPACE.as_path()).expect("创建临时目录失败");
    let config_path = temp_dir.path().join("test_config.toml");
    let config_str = toml::to_string(&config).expect("序列化配置失败");
    crate::common::write_sync(&config_path, config_str).expect("写入配置文件失败");

    let result = config::parse_config(&config_path);
    assert_error_contains(result, "sha256_src cannot be empty");
    temp_dir.close().expect("关闭临时目录失败");
}

#[test]
fn test_validate_file_patch_invalid_sha256_format() {
    let config = config::Config {
        groups: vec![config::GroupConfig {
            anchor: ".minecraft".to_string(),
            root: "mods".to_string(),
            recursive: false,
            mirror: false,
            delete: false,
            pattern: None,
            files: vec![config::FileRule {
                name: Some("test.jar".to_string()),
                mod_id: None,
                mod_version: None,
                name_pattern: None,
                url: "https://example.com/test.jar".to_string(),
                sha256: None,
                patches: vec![config::FilePatch {
                    url_patch: "https://example.com/patch.bspatch".to_string(),
                    sha256_src: "not-hex".repeat(10), // 非十六进制
                    keep_src: true,
                }],
            }],
        }],
        ..Default::default()
    };

    // validate_config是私有函数，我们通过parse_config来测试
    // 创建临时配置文件来测试验证逻辑
    let temp_dir = tempfile::tempdir_in(crate::TEST_WORKSPACE.as_path()).expect("创建临时目录失败");
    let config_path = temp_dir.path().join("test_config.toml");
    let config_str = toml::to_string(&config).expect("序列化配置失败");
    crate::common::write_sync(&config_path, config_str).expect("写入配置文件失败");

    let result = config::parse_config(&config_path);
    assert_error_contains(result, "must be a valid 64-character hex string");
    temp_dir.close().expect("关闭临时目录失败");
}

#[test]
fn test_validate_valid_config_with_warnings() -> Result<(), anyhow::Error> {
    let config = config::Config {
        network: config::NetworkConfig {
            quic: true,
            ignore_invalid_cert: true,
            timeout: 15,
            retry: 3,
        },
        groups: vec![config::GroupConfig {
            anchor: ".minecraft".to_string(),
            root: "mods".to_string(),
            recursive: false,
            mirror: false,
            delete: false,
            pattern: None,
            files: vec![config::FileRule {
                name: Some("test.jar".to_string()),
                mod_id: None,
                mod_version: None,
                name_pattern: None,
                url: "https://example.com/test.jar".to_string(),
                sha256: None,
                patches: vec![],
            }],
        }],
        ..Default::default()
    };

    // 应该成功，但会有警告日志
    // validate_config是私有函数，我们通过parse_config来测试
    // 创建临时配置文件来测试验证逻辑
    let temp_dir = tempfile::tempdir_in(crate::TEST_WORKSPACE.as_path()).expect("创建临时目录失败");
    let config_path = temp_dir.path().join("test_config.toml");
    let config_str = toml::to_string(&config).expect("序列化配置失败");
    crate::common::write_sync(&config_path, config_str).expect("写入配置文件失败");

    let result = config::parse_config(&config_path);
    assert!(result.is_ok(), "有效配置应该验证通过");

    temp_dir.close()?;
    Ok(())
}

#[test]
fn test_validate_minimal_valid_config() -> Result<(), anyhow::Error> {
    let config = config::Config {
        groups: vec![config::GroupConfig {
            anchor: ".minecraft".to_string(),
            root: "mods".to_string(),
            recursive: false,
            mirror: false,
            delete: false,
            pattern: None,
            files: vec![config::FileRule {
                name: Some("test.jar".to_string()),
                mod_id: None,
                mod_version: None,
                name_pattern: None,
                url: "https://example.com/test.jar".to_string(),
                sha256: None,
                patches: vec![],
            }],
        }],
        ..Default::default()
    };

    // validate_config是私有函数，我们通过parse_config来测试
    // 创建临时配置文件来测试验证逻辑
    let temp_dir = tempfile::tempdir_in(crate::TEST_WORKSPACE.as_path()).expect("创建临时目录失败");
    let config_path = temp_dir.path().join("test_config.toml");
    let config_str = toml::to_string(&config).expect("序列化配置失败");
    crate::common::write_sync(&config_path, config_str).expect("写入配置文件失败");

    let result = config::parse_config(&config_path);
    assert!(result.is_ok(), "最小有效配置应该验证通过");

    temp_dir.close()?;
    Ok(())
}

#[test]
fn test_validate_config_with_all_match_conditions() -> Result<(), anyhow::Error> {
    let config = config::Config {
        groups: vec![config::GroupConfig {
            anchor: ".minecraft".to_string(),
            root: "mods".to_string(),
            recursive: false,
            mirror: false,
            delete: false,
            pattern: Some(r"^.*\.jar$".to_string()),
            files: vec![
                // 名称匹配
                config::FileRule {
                    name: Some("test1.jar".to_string()),
                    mod_id: None,
                    mod_version: None,
                    name_pattern: None,
                    url: "https://example.com/test1.jar".to_string(),
                    sha256: None,
                    patches: vec![],
                },
                // MOD ID匹配
                config::FileRule {
                    name: None,
                    mod_id: Some("testmod".to_string()),
                    mod_version: None,
                    name_pattern: None,
                    url: "https://example.com/testmod.jar".to_string(),
                    sha256: None,
                    patches: vec![],
                },
                // 名称模式匹配
                config::FileRule {
                    name: None,
                    mod_id: None,
                    mod_version: None,
                    name_pattern: Some(r"^prefix_.*\.jar$".to_string()),
                    url: "https://example.com/pattern.jar".to_string(),
                    sha256: None,
                    patches: vec![],
                },
                // SHA256匹配
                config::FileRule {
                    name: None,
                    mod_id: None,
                    mod_version: None,
                    name_pattern: None,
                    url: "https://example.com/sha256.jar".to_string(),
                    sha256: Some(
                        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
                            .to_string(),
                    ),
                    patches: vec![],
                },
            ],
        }],
        ..Default::default()
    };

    // validate_config是私有函数，我们通过parse_config来测试
    // 创建临时配置文件来测试验证逻辑
    let temp_dir = tempfile::tempdir_in(crate::TEST_WORKSPACE.as_path()).expect("创建临时目录失败");
    let config_path = temp_dir.path().join("test_config.toml");
    let config_str = toml::to_string(&config).expect("序列化配置失败");
    crate::common::write_sync(&config_path, config_str).expect("写入配置文件失败");

    let result = config::parse_config(&config_path);
    assert!(result.is_ok(), "包含所有匹配条件的配置应该验证通过");

    temp_dir.close()?;
    Ok(())
}
