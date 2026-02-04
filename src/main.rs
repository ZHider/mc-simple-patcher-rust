use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

pub mod config;
pub mod anchor_finder;
pub mod file_manager;
pub mod downloader;
pub mod main_controller;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// 配置文件路径
    #[arg(short, long, default_value = "mc_simple_patcher.toml")]
    config: PathBuf,

    /// 启用调试模式
    #[arg(short, long)]
    debug: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // 初始化日志系统
    init_logger(args.debug)?;

    log::info!("Minecraft 简易补丁工具启动");

    let result = execute_with_config(&args.config).await;

    match &result {
        Ok(()) => {
            log::info!("程序执行完成");
        }
        Err(e) => {
            log::error!("程序执行出错: {}", e);
        }
    }

    // 无论成功还是失败，都等待用户按键退出
    pause_before_exit();

    result
}

/// 程序退出前暂停，等待用户按键
fn pause_before_exit() {
    use std::io::{stdin, stdout, Write};

    print!("\n请按任意键退出...");
    let _ = stdout().flush(); // 确保提示信息立即显示

    // 读取一个字符
    let mut input = String::new();
    let _ = stdin().read_line(&mut input);

    println!(); // 换行
}

/// 解析配置文件并执行补丁
async fn execute_with_config(config_path: &PathBuf) -> Result<()> {
    log::info!("正在解析配置文件: {:?}", config_path);
    let config = config::parse_config(config_path)
        .map_err(|e| anyhow::anyhow!("解析配置文件失败: {}", e))?;

    main_controller::execute_patch(&config).await
}

use log::{Metadata, Record};
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;

struct DualLogger {
    file: Mutex<std::fs::File>,
    debug: bool,
}

impl DualLogger {
    fn new(debug: bool) -> Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
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
                },
                log::Level::Debug => {
                    let style = Style::new().yellow();
                    format!("{}", style.apply_to("[DEBUG]"))
                },
                log::Level::Error => {
                    let style = Style::new().red();
                    format!("{}", style.apply_to("[ERROR]"))
                },
                _ => format!("[{}]", record.level()),
            };

            println!("{} {}", level_tag, record.args());

            // 同时写入日志文件
            if let Ok(mut file) = self.file.lock() {
                let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S");
                let _ = writeln!(file, "[{}] [{}] {}", timestamp, record.level(), record.args());
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
fn init_logger(debug: bool) -> Result<()> {
    let logger = DualLogger::new(debug)?;

    log::set_boxed_logger(Box::new(logger))
        .map(|()| log::set_max_level(log::LevelFilter::Debug))
        .map_err(|e| anyhow::anyhow!("Failed to initialize logger: {}", e))?;

    log::info!("Logger initialized");
    Ok(())
}
