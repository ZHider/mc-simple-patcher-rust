//! 进度条更新器模块
//! 负责后台更新进度条显示

use indicatif::ProgressBar;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::progress_bar::TICK_INTERVAL_MS;
use super::string_scroller::StringScroller;

/// 启动进度更新任务
///
/// # Arguments
///
/// * `pb` - 进度条
/// * `full_name` - 完整文件名
/// * `prefix` - 进度条前缀
/// * `total_size` - 总大小
/// * `rx` - 接收已下载字节数的通道
///
/// # Returns
///
/// * `JoinHandle<()>` - 更新任务的句柄
pub fn spawn_progress_updater(
    pb: ProgressBar,
    full_name: &str,
    prefix: String,
    total_size: u64,
    rx: mpsc::Receiver<u64>,
) -> JoinHandle<()> {
    let name = full_name.to_string();
    tokio::spawn(async move {
        run_progress_updater(pb, &name, prefix, total_size, rx).await;
    })
}

/// 运行进度更新器
async fn run_progress_updater(
    pb: ProgressBar,
    name: &str,
    prefix: String,
    total_size: u64,
    mut rx: mpsc::Receiver<u64>,
) {
    let display_width = 50; // 假设终端宽度
    let scroller = StringScroller::new(name, display_width);
    let mut offset = 0;
    let last_downloaded: u64 = 0;

    loop {
        tokio::select! {
            Some(downloaded) = rx.recv() => {
                // 更新进度条
                pb.set_position(downloaded - last_downloaded);

                // 滚动文件名
                let display_name = scroller.display(offset);
                pb.set_message(format!("{}{}", prefix, display_name));

                // 更新滚动偏移
                if scroller.should_scroll() {
                    offset = (offset + 1) % scroller.max_offset();
                }

                // 下载完成
                if downloaded >= total_size {
                    let finish_prefix = prefix
                        .split_once("] ")
                        .map(|(p, _)| format!("{}]", p))
                        .unwrap_or_default();
                    pb.finish_with_message(format!("{} ✓ {}", finish_prefix, name));
                    break;
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(TICK_INTERVAL_MS)) => {
                // 定期更新，保持动画效果
                if scroller.should_scroll() {
                    offset = (offset + 1) % scroller.max_offset();
                    let display_name = scroller.display(offset);
                    pb.set_message(format!("{}{}", prefix, display_name));
                }
            }
        }
    }
}
