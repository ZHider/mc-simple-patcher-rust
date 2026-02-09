//! 日志系统模块
//! 实现应用程序的日志记录功能

use anyhow::{Context, Result};
use log::{Metadata, Record};
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;

/// 双重日志记录器，同时输出到控制台和文件
pub struct DualLogger {
    file: Mutex<std::fs::File>,
    debug: bool,
}

impl DualLogger {
    /// 创建新的双重日志记录器
    pub fn new(debug: bool) -> Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open("mc_simple_patcher.log")?;

        Ok(DualLogger {
            file: Mutex::new(file),
            debug,
        })
    }
}

impl log::Log for DualLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        if self.debug {
            metadata.level() <= log::Level::Debug
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
                    format!("{}", style.apply_to("[DEBUG]"))
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

            println!("{} {}", level_tag, record.args());

            // 同时写入日志文件
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
    }

    fn flush(&self) {
        if let Ok(mut file) = self.file.lock() {
            let _ = file.flush();
        }
    }
}

/// 初始化日志系统
pub fn init_logger(debug: bool) -> Result<()> {
    let logger = DualLogger::new(debug)?;

    log::set_boxed_logger(Box::new(logger))
        .map(|()| log::set_max_level(log::LevelFilter::Debug))
        .with_context(|| "Failed to initialize logger")?;

    log::info!("Logger initialized");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_logger_initialization() -> Result<()> {
        // 测试日志初始化功能
        let result = init_logger(false);

        // 验证初始化成功
        assert!(result.is_ok());

        // 验证日志文件被创建
        assert!(fs::metadata("mc_simple_patcher.log").is_ok());

        Ok(())
    }

    #[test]
    fn test_dual_logger_creation() -> Result<()> {
        // 测试 DualLogger 创建功能
        let logger = DualLogger::new(false);

        // 验证创建成功
        assert!(logger.is_ok());

        Ok(())
    }
}
