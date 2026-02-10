use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

pub const PROGRESS_BAR_REFREST_RATE: u8 = 4;
pub const TICK_INTERVAL_MS: u64 = 250;

/// 设置多进度条容器
pub fn setup_multi_progress() -> MultiProgress {
    let multi_progress = MultiProgress::new();
    multi_progress.set_draw_target(indicatif::ProgressDrawTarget::stderr_with_hz(
        PROGRESS_BAR_REFREST_RATE,
    ));
    multi_progress
}

/// 创建进度条样式
pub fn create_progress_bar_style() -> ProgressStyle {
    ProgressStyle::default_bar()
        .template("{msg} {bar:30.cyan/blue} {bytes}/{total_bytes} ({eta})")
        .unwrap()
        .progress_chars("=>-")
}

/// 格式化文件名，实现跑马灯式滚动
/// 
/// # 参数
/// - `filename`: 原始文件名
/// - `display_width`: 显示窗口宽度（字符数）
/// - `character_offset`: 移动的字符数（每次递增1表示移动一个字符）
pub fn format_filename_with_scroll(filename: &str, display_width: usize, character_offset: usize) -> String {
    if filename.len() <= display_width {
        // 文件名较短，用空格填充到指定宽度
        format!("{:<width$}", filename, width = display_width)
    } else {
        // 文件名过长，实现跑马灯式滚动
        // 创建循环字符串：filename + 5个空格 + filename（循环）
        const SEPARATOR_SPACES: usize = 5;
        let loop_content = format!("{}{}", filename, " ".repeat(SEPARATOR_SPACES));
        let loop_len = loop_content.len();

        // 根据字符offset计算当前窗口位置（使用模运算实现循环）
        let offset = (character_offset as usize) % loop_len;

        // 提取显示窗口
        let mut result = String::with_capacity(display_width);
        for i in 0..display_width {
            let idx = (offset + i) % loop_len;
            result.push_str(&loop_content[idx..=idx]);
        }
        result
    }
}

/// 创建和配置进度条
pub fn create_progress_bar(
    multi_progress: &MultiProgress,
    file_size: Option<u64>,
    index: usize,
    total: Option<usize>,
    filename: &str,
) -> ProgressBar {
    let initial_length = file_size.unwrap_or(0);
    let pb = multi_progress.add(ProgressBar::new(initial_length));

    // 设置样式
    pb.set_style(create_progress_bar_style());

    // 初始文件名显示（字符偏移从0开始）
    let formatted_name = format_filename_with_scroll(filename, 30, 0);

    // 设置消息（带序号和格式化后的文件名），当 total 未知时显示 `?`
    let total_display = total.map(|t| t.to_string()).unwrap_or_else(|| "?".to_string());
    let file_info = format!("[{:2}/{}] {}", index + 1, total_display, formatted_name);
    pb.set_message(file_info);

    // 保存原始文件名作为状态（我们需要在下载时更新它）
    pb.set_position(0); // 初始位置

    // 启用稳定刷新（间隔由 refresh_rate_hz 计算得出）
    pb.enable_steady_tick(Duration::from_millis(TICK_INTERVAL_MS));

    pb
}

/// 启动一个独立的进度更新器任务
///
/// `rx` 用于接收下载任务发送的已下载字节数；任务以固定频率刷新滚动文本并在下载完成后调用 finish
pub fn spawn_progress_updater(
    pb: ProgressBar,
    full_filename: String,
    prefix: String,
    total_size: u64,
    mut rx: mpsc::Receiver<u64>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut downloaded: u64 = 0;
        let mut ticks: usize = 0;
        let mut interval = tokio::time::interval(Duration::from_millis(50)); // 20Hz 刷新

        loop {
            tokio::select! {
                biased;
                Some(chunk) = rx.recv() => {
                    downloaded = downloaded.saturating_add(chunk);
                    pb.set_position(downloaded);
                    if downloaded >= total_size {
                        break;
                    }
                }
                _ = interval.tick() => {
                    ticks = ticks.wrapping_add(1);
                    let character_offset = ticks as usize;
                    let scrolled_name = format_filename_with_scroll(&full_filename, 30, character_offset);
                    let msg = format!("{}{}", prefix, scrolled_name);
                    pb.set_message(msg);
                }
            }
        }

        pb.finish_with_message(format!("{}✓", prefix));
    })
}
