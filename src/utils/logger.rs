//! 日志系统模块
//! 实现应用程序的日志记录功能

use anyhow::{Context, Result};
use log::{Metadata, Record};
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;

use crate::global_config;

/// 双重日志记录器，同时输出到控制台和文件
pub struct DualLogger {
    file: Mutex<std::fs::File>,
    debug: bool,
    quiet: bool,
}

impl DualLogger {
    /// 创建新的双重日志记录器
    pub fn new(debug: bool, quiet: bool) -> Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open("mc_simple_patcher.log")?;

        Ok(DualLogger {
            file: Mutex::new(file),
            debug,
            quiet,
        })
    }
}

impl log::Log for DualLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        if self.quiet {
            false
        } else if self.debug {
            metadata.level() <= log::Level::Trace
        } else {
            metadata.level() <= log::Level::Info
        }
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            use console::Style;

            // 输出到控制台
            let level_tag = match record.level() {
                log::Level::Info => {
                    let style = Style::new().green();
                    format!("{}", style.apply_to("[INFO]"))
                }
                log::Level::Debug => {
                    let style = Style::new().yellow();
                    format!(
                        "{} [{}:{}]",
                        style.apply_to("[DEBUG]"),
                        record.module_path().unwrap_or("unknown"),
                        record.line().unwrap_or(0),
                    )
                }
                log::Level::Error => {
                    let style = Style::new().red();
                    format!("{}", style.apply_to("[ERROR]"))
                }
                log::Level::Warn => {
                    let style = Style::new().on_yellow();
                    format!("{}", style.apply_to("[WARN]"))
                }
                _ => format!("[{}]", record.level()),
            };

            if record.level() == log::Level::Error {
                eprintln!("{} {}", level_tag, record.args());
            } else if let Some(pb) = global_config::get_global_progress() {
                let msg = format!("{} {}", level_tag, record.args());
                pb.println(msg);
            } else {
                println!("{} {}", level_tag, record.args());
            }
        }
        // 写入日志文件
        if let Ok(mut file) = self.file.lock() {
            let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S");
            let _ = writeln!(
                file,
                "[{}] [{}] {}",
                timestamp,
                record.level(),
                record.args()
            );
        }
    }

    fn flush(&self) {
        if let Ok(mut file) = self.file.lock() {
            let _ = file.flush();
        }
    }
}

/// 初始化日志系统
///
/// # Arguments
///
/// * `debug` - 是否启用调试模式
///
/// # Returns
///
/// * `Result<()>` - 成功时返回空值，失败时返回错误
pub fn init_logger(debug: bool, quiet: bool) -> Result<()> {
    let logger = DualLogger::new(debug, quiet)?;

    log::set_boxed_logger(Box::new(logger))
        .map(|()| log::set_max_level(log::LevelFilter::Trace))
        .with_context(|| "Failed to initialize logger")?;

    log::info!("Logger initialized");
    Ok(())
}
