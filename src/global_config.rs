use crate::config::Config;
use std::sync::{Arc, RwLock};

// 全局配置存储
static GLOBAL_CONFIG: RwLock<Option<Arc<Config>>> = RwLock::new(None);

/// 初始化全局配置
pub fn set_global_config(config: Config) {
    *GLOBAL_CONFIG.write().unwrap() = Some(Arc::new(config));
}

/// 获取全局配置的引用
pub fn get_global_config() -> Arc<Config> {
    GLOBAL_CONFIG
        .read()
        .unwrap()
        .clone()
        .expect("在全局Config还没有进行初始化的时候就被访问了")
}
