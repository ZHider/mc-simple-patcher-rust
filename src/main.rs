use anyhow::{Ok, Result};
use clap::Parser;
use std::path::PathBuf;

pub mod anchor_finder;
pub mod config;
pub mod downloader;
pub mod file_manager;
pub mod generator;
pub mod logger;
pub mod main_controller;
pub mod utils;

/// 计算并打印文件的SHA256哈希值
fn calculate_and_print_file_sha256(file_path: &std::path::Path) -> Result<()> {
    if !file_path.exists() {
        anyhow::bail!("指定的文件不存在: {:?}", file_path);
    }

    let hash_string = utils::calculate_file_sha256(file_path)?;
    println!("{}", hash_string);
    Ok(())
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// 配置文件路径
    #[arg(short, long, default_value = "mc_simple_patcher.toml")]
    config: PathBuf,

    /// 启用调试模式
    #[arg(short, long)]
    debug: bool,

    /// 生成模式：指定要扫描的目录
    #[arg(short, long, value_name = "DIR")]
    generate: Option<PathBuf>,

    /// 生成模式：指定用于匹配文件名的正则表达式
    #[arg(long, value_name = "NAME-REGEX")]
    pattern: Option<String>,

    /// 生成模式：递归扫描子目录
    #[arg(short, long)]
    recursive: bool,

    /// 生成模式：基础 URL，用于生成下载链接
    #[arg(long)]
    base_url: Option<String>,

    /// 生成模式：尝试提取模组信息（mod ID 和 version）
    #[arg(long)]
    mod_info: bool,

    /// SHA256模式：计算指定文件的SHA256哈希值
    #[arg(long)]
    sha256: Option<std::path::PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // 初始化日志系统
    logger::init_logger(args.debug)?;

    log::info!("Minecraft 简易补丁工具启动");

    let result = if let Some(sha256_path) = args.sha256 {
        // 如果提供了 --sha256 参数，则计算文件的SHA256哈希值
        calculate_and_print_file_sha256(&sha256_path)
    } else if let Some(generate_dir) = args.generate {
        // 如果提供了 --generate 参数，则进入生成模式
        generator::generate_config(
            generate_dir,
            args.pattern,
            args.recursive,
            args.base_url,
            args.mod_info,
        )
    } else {
        // 否则执行原有的补丁逻辑
        execute_with_config(&args.config).await
    };

    if let Err(e) = &result {
        log::error!("程序执行出错: {}", e);
    } else {
        log::info!("程序执行完成");
    }

    // 无论成功还是失败，都等待用户按键退出
    pause_before_exit();

    result
}

/// 程序退出前暂停，等待用户按键
fn pause_before_exit() {
    use std::io::{Write, stdin, stdout};

    print!("\n请按任意键...");
    let _ = stdout().flush(); // 确保提示信息立即显示

    // 读取一个字符
    let mut input = String::new();
    let _ = stdin().read_line(&mut input);

    println!(); // 换行
}


/// 解析配置文件并执行补丁
async fn execute_with_config(config_path: &std::path::Path) -> Result<()> {
    log::info!("正在解析配置文件: {}", config_path.display());
    let config = update_metadata_if_needed(config_path).await?;
    pause_before_exit();
    main_controller::execute_patch(&config).await
}

async fn update_metadata_if_needed(
    config_path: &std::path::Path,
) -> Result<config::Config> {
    let config = config::parse_config(config_path)
        .map_err(|e| anyhow::anyhow!("解析配置文件失败: {}", e))?;

    let metadata_url = config.metadata_config.metadata.as_ref().unwrap();
    log::info!("尝试更新元数据: {}", metadata_url);

    if let Err(e) = downloader::download_file(metadata_url, config_path).await {
        log::error!("更新失败！{}", e);
        Ok(config)
    } else {
        log::info!("元数据文件已更新: {}", config_path.display());
        // 重新解析配置文件
        config::parse_config(config_path)
    }
}
