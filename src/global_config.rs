use crate::config::Config;
use std::sync::OnceLock;

// 全局配置存储
static GLOBAL_CONFIG: OnceLock<Config> = OnceLock::new();

/// 初始化全局配置
pub fn init_global_config(config: &Config) -> Result<(), Config> {
    GLOBAL_CONFIG.set(config.to_owned())
}

/// 获取全局配置的引用
pub fn get_global_config() -> &'static Config {
    GLOBAL_CONFIG.get().expect("Global config not initialized")
}
