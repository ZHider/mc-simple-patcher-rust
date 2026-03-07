//! 元数据信息测试

use mc_simple_patcher::config;

#[test]
fn test_get_metadata_info_both_present() {
    let config = config::Config {
        metadata_config: config::MetadataConfig {
            metadata: Some("Test Metadata".to_string()),
            version: Some(42),
        },
        ..Default::default()
    };

    let info = config::get_metadata_info(&config);
    assert_eq!(info, "Test Metadata v42");
}

#[test]
fn test_get_metadata_info_only_metadata() {
    let config = config::Config {
        metadata_config: config::MetadataConfig {
            metadata: Some("Test Metadata".to_string()),
            version: None,
        },
        ..Default::default()
    };

    let info = config::get_metadata_info(&config);
    assert_eq!(info, "Test Metadata");
}

#[test]
fn test_get_metadata_info_only_version() {
    let config = config::Config {
        metadata_config: config::MetadataConfig {
            metadata: None,
            version: Some(42),
        },
        ..Default::default()
    };

    let info = config::get_metadata_info(&config);
    assert_eq!(info, "Unknown v42");
}

#[test]
fn test_get_metadata_info_none() {
    let config = config::Config {
        metadata_config: config::MetadataConfig {
            metadata: None,
            version: None,
        },
        ..Default::default()
    };

    let info = config::get_metadata_info(&config);
    assert_eq!(info, "Unknown");
}

#[test]
fn test_get_metadata_info_empty_string() {
    let config = config::Config {
        metadata_config: config::MetadataConfig {
            metadata: Some("".to_string()),
            version: Some(0),
        },
        ..Default::default()
    };

    let info = config::get_metadata_info(&config);
    assert_eq!(info, " v0");
}

#[test]
fn test_get_metadata_info_special_characters() {
    let config = config::Config {
        metadata_config: config::MetadataConfig {
            metadata: Some("Test & Metadata © 2023".to_string()),
            version: Some(123),
        },
        ..Default::default()
    };

    let info = config::get_metadata_info(&config);
    assert_eq!(info, "Test & Metadata © 2023 v123");
}

#[test]
fn test_get_metadata_info_large_version() {
    let config = config::Config {
        metadata_config: config::MetadataConfig {
            metadata: Some("Test".to_string()),
            version: Some(u32::MAX),
        },
        ..Default::default()
    };

    let info = config::get_metadata_info(&config);
    assert_eq!(info, format!("Test v{}", u32::MAX));
}

#[test]
fn test_metadata_config_default() {
    let config = config::MetadataConfig::default();

    assert!(config.metadata.is_none());
    assert!(config.version.is_none());
}
