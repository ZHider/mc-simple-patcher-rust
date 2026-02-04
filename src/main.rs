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
    #[arg(short, long, default_value = "config.toml")]
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

    // 解析配置文件
    log::info!("正在解析配置文件: {:?}", args.config);
    let config = config::parse_config(&args.config)
        .map_err(|e| anyhow::anyhow!("解析配置文件失败: {}", e))?;

    // 创建主控制器并执行补丁
    let controller = main_controller::MainController::new();
    controller.execute_patch(&config).await?;

    log::info!("程序执行完成");
    Ok(())
}

/// 初始化日志系统
fn init_logger(debug: bool) -> Result<()> {
    use std::fs::OpenOptions;
    use env_logger::Builder;
    use log::LevelFilter;
    use std::io::Write;

    // 设置日志级别
    let level = if debug {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };

    // 创建日志文件
    let mut log_file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(true) // 使用追加模式而不是截断
        .open("mc_simple_patcher.log")?;

    // 配置 env_logger
    let mut builder = Builder::new();
    builder
        .filter_level(level)
        .format(|buf, record| {
            use std::io::Write;
            use console::Style;

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

            writeln!(buf, "{} {}", level_tag, record.args())
        })
        .target(env_logger::Target::Stdout) // 输出到控制台
        .init();

    // 同时写入日志文件
    writeln!(log_file, "[{}] Logger initialized", chrono::Utc::now().format("%Y-%m-%d %H:%M:%S"))?;

    Ok(())
}
