use indicatif::ProgressBar;

use crate::config::Config;
use std::sync::{Arc, LazyLock, Mutex, RwLock};

// 全局配置存储
static GLOBAL_CONFIG: RwLock<Option<Arc<Config>>> = RwLock::new(None);

/// 初始化全局配置
///
/// # Arguments
///
/// * `config` - 配置对象
pub fn set_global_config(config: Config) {
    *GLOBAL_CONFIG.write().unwrap() = Some(Arc::new(config));
}

/// 获取全局配置的引用
///
/// # Returns
///
/// * `Arc<Config>` - 全局配置的原子引用计数指针
pub fn get_global_config() -> Arc<Config> {
    GLOBAL_CONFIG
        .read()
        .unwrap()
        .clone()
        .expect("在全局Config还没有进行初始化的时候就被访问了")
}

// 全局进度条句柄
static GLOBAL_PROGRESS: LazyLock<Mutex<Option<ProgressBar>>> = LazyLock::new(|| Mutex::new(None));

/// 设置全局进度条（在创建进度条时调用）
///
/// # Arguments
///
/// * `pb` - 进度条对象
pub fn set_global_progress(pb: ProgressBar) {
    *GLOBAL_PROGRESS.lock().unwrap() = Some(pb);
}

/// 清除全局进度条（结束后调用）
pub fn clear_global_progress() {
    *GLOBAL_PROGRESS.lock().unwrap() = None;
}

/// 获取全局进度条的引用（如果存在）
///
/// # Returns
///
/// * `Option<ProgressBar>` - 如果存在则返回进度条的克隆副本，否则返回 None
pub fn get_global_progress() -> Option<ProgressBar> {
    GLOBAL_PROGRESS.lock().unwrap().clone()
}
