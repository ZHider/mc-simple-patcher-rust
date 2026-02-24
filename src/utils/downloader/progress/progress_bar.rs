//! 进度条工具模块
//! 提供进度条创建和更新的工具函数

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::sync::LazyLock;

pub const PROGRESS_BAR_REFREST_RATE: u8 = 4;
pub const TICK_INTERVAL_MS: u64 = 250;

/// 进度条样式模板
static DEFAULT_PROGRESS_STYLE: LazyLock<ProgressStyle> = LazyLock::new(|| {
    ProgressStyle::default_bar()
        .template("{prefix:.bold.dim} {spinner:.green} [{bar:.cyan/blue}] {bytes}/{total_bytes} ({eta}) {msg}")
        .unwrap()
        .progress_chars("#>-")
});

static SINGLE_PROGRESS_STYLE: LazyLock<ProgressStyle> = LazyLock::new(|| {
    ProgressStyle::default_bar()
        .template("{spinner:.green} [{bar:.cyan/blue}] {bytes}/{total_bytes} ({eta}) {msg}")
        .unwrap()
        .progress_chars("#>-")
});

/// 设置多进度条容器
pub fn setup_multi_progress() -> MultiProgress {
    let multi_progress = MultiProgress::new();
    multi_progress.set_draw_target(indicatif::ProgressDrawTarget::stderr_with_hz(
        PROGRESS_BAR_REFREST_RATE,
    ));
    multi_progress
}

/// 创建单文件下载进度条
pub fn create_progress_bar_single() -> ProgressBar {
    let pb = ProgressBar::new(0);
    pb.set_style(SINGLE_PROGRESS_STYLE.clone());
    pb.enable_steady_tick(std::time::Duration::from_millis(250));
    pb
}

/// 创建多进度条中的一个
///
/// # Arguments
///
/// * `multi_progress` - 多进度条容器
/// * `prefix` - 进度条前缀（可选）
/// * `index` - 进度条索引
/// * `total_opt` - 总任务数（可选）
/// * `filename` - 文件名
///
/// # Returns
///
/// * `ProgressBar` - 创建的进度条
pub fn create_progress_bar(
    multi_progress: &MultiProgress,
    prefix: Option<&str>,
    index: usize,
    total_opt: Option<usize>,
    filename: &str,
) -> ProgressBar {
    let pb = multi_progress.add(ProgressBar::new(0));

    // 设置前缀（如 [1/10]）
    if let Some(total) = total_opt {
        let prefix_str = format!("[{}/{}]", index + 1, total);
        pb.set_prefix(prefix_str);
    } else if let Some(p) = prefix {
        pb.set_prefix(p.to_string());
    }

    pb.set_style(DEFAULT_PROGRESS_STYLE.clone());
    pb.set_message(filename.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(250));

    pb
}
