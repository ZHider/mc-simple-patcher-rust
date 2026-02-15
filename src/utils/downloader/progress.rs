use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::cmp::min;
use std::sync::LazyLock;
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

pub struct StringScroller {
    loop_content: String,
    display_width: usize,
    display_type: DisplayType,
    char_boundaries: Vec<usize>,
    offset_max: usize,
}

#[derive(Clone, Copy)]
enum DisplayType {
    NoLoop,
    Loop,
}

impl StringScroller {
    const SEPARATOR_SPACES: usize = 5;

    pub fn new(content: &str, display_width: usize) -> Self {
        let content_chars_num = content.chars().count();
        // NoLoop
        if content_chars_num <= display_width {
            let loop_content = format!("{:<width$}", content, width = display_width);
            Self {
                loop_content,
                display_width,
                display_type: DisplayType::NoLoop,
                char_boundaries: Vec::new(),
                offset_max: 0,
            }
        } else {
            // Loop
            // 构建循环字符串
            let mut loop_content =
                String::with_capacity(content.len() * 2 + Self::SEPARATOR_SPACES);
            loop_content.push_str(content);
            loop_content.push_str(&" ".repeat(Self::SEPARATOR_SPACES));
            loop_content.push_str(content);

            // 最大能够接受的offset，使其能够完全显示，又不会超过边界
            // 相当于在这里：{content}{SEPARATOR_SPACES}>|<{content}
            let offset_max = content_chars_num + Self::SEPARATOR_SPACES;
            // 收集全部有效的char边界->byte边界的序列
            let char_boundaries: Vec<usize> =
                loop_content.char_indices().map(|(i, _c)| i).collect();

            Self {
                loop_content,
                display_width,
                display_type: DisplayType::Loop,
                char_boundaries,
                offset_max,
            }
        }
    }

    pub fn display(&self, offset: usize) -> &str {
        match self.display_type {
            DisplayType::NoLoop => self.display_no_loop(),
            DisplayType::Loop => self.display_loop(offset),
        }
    }

    fn display_no_loop(&self) -> &str {
        &self.loop_content
    }

    fn display_loop(&self, offset: usize) -> &str {
        // offset语义是char的边界而不是byte的边界
        let char_start = offset % self.offset_max;
        let byte_start = self.char_boundaries[char_start];
        let byte_end = self.char_boundaries[min(
            char_start + self.display_width,
            self.char_boundaries.len() - 1,
        )];

        &self.loop_content[byte_start..byte_end]
    }
}

static PB_WAITING_STYLE: LazyLock<ProgressStyle> = std::sync::LazyLock::new(|| {
    ProgressStyle::default_bar()
        .template("{spinner} {msg} {total_bytes}")
        .unwrap()
        .progress_chars("  ")
});

static PB_FINISHED_STYLE: LazyLock<ProgressStyle> = std::sync::LazyLock::new(|| {
    ProgressStyle::default_bar()
        .template("{msg} [{elapsed_precise}] {total_bytes}")
        .unwrap()
        .progress_chars("  ")
});

static PB_DOWNLOADING_STYLE: LazyLock<ProgressStyle> = std::sync::LazyLock::new(|| {
    ProgressStyle::default_bar()
        .template("{msg} {bar:50.cyan/blue} {bytes}/{total_bytes} ({eta})")
        .unwrap()
        .progress_chars("=>-")
});

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
    pb.set_style(PB_WAITING_STYLE.clone());

    // 设置消息（带序号和格式化后的文件名），当 total 未知时显示 `?`
    let total_display = total
        .map(|t| t.to_string())
        .unwrap_or_else(|| "?".to_string());
    let file_info = format!(
        "[{:2}/{}] 开始下载 {}\t",
        index + 1,
        total_display,
        filename
    );
    pb.set_message(file_info);

    // 保存原始文件名作为状态（我们需要在下载时更新它）
    pb.set_position(0); // 初始位置

    // 启用稳定刷新（间隔由 refresh_rate_hz 计算得出）
    pb.enable_steady_tick(Duration::from_millis(TICK_INTERVAL_MS));

    pb
}

pub fn create_progress_bar_single() -> ProgressBar {
    let pb = ProgressBar::new(0);

    // 设置样式
    pb.set_style(PB_WAITING_STYLE.clone());

    // 设置消息（带序号和格式化后的文件名），当 total 未知时显示 `?`
    let file_info = "开始下载\t".to_string();
    pb.set_message(file_info);

    // 保存原始文件名作为状态（我们需要在下载时更新它）
    pb.set_position(0); // 初始位置

    // 启用稳定刷新（间隔由 refresh_rate_hz 计算得出）
    // pb.enable_steady_tick(Duration::from_millis(TICK_INTERVAL_MS));

    pb
}

/// 启动一个独立的进度更新器任务
///
/// `rx` 用于接收下载任务发送的已下载字节数；任务以固定频率刷新滚动文本并在下载完成后调用 finish
pub fn spawn_progress_updater(
    pb: ProgressBar,
    filename: String,
    prefix: String,
    total_size: u64,
    mut rx: mpsc::Receiver<u64>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let filename_scroller = StringScroller::new(&filename, 30);
        let mut downloaded: u64 = 0;
        let mut ticks: usize = 0;
        let mut interval = tokio::time::interval(Duration::from_millis(TICK_INTERVAL_MS));
        pb.set_style(PB_DOWNLOADING_STYLE.clone());

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
                    let scrolled_name = filename_scroller.display(ticks);
                    let msg = format!("{}{}", prefix, scrolled_name);
                    pb.set_message(msg);
                }
            }
        }

        pb.set_style(PB_FINISHED_STYLE.clone());
        pb.finish_with_message(format!("{}✓完成 {}", prefix, filename));
    })
}
