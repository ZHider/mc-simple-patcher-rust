use crate::{
    config::Config,
    generator::write_sha256,
    global_config::get_global_config,
    utils::{downloader, logger},
};
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

pub mod config;
pub mod file_manager;
pub mod generator;
mod global_config;
pub mod main_controller;
pub mod utils;

/// 计算并打印文件的SHA256哈希值
///
/// # Arguments
///
/// * `file_path` - 文件路径的引用
///
/// # Returns
///
/// * `Result<()>` - 成功时返回空值，失败时返回错误
fn calculate_and_print_file_sha256(file_path: &std::path::Path) -> Result<()> {
    if !file_path.exists() {
        anyhow::bail!("指定的文件不存在: {:?}", file_path);
    }

    let result = write_sha256(file_path);

    if result.is_ok() {
        let mut hex_str_file = File::open(file_path.with_added_extension("sha256"))?;
        let mut hex_str = String::with_capacity(256);
        hex_str_file.read_to_string(&mut hex_str)?;
        println!("{}", &hex_str);
    }

    result
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// 从TOML文件生成配置文件
    Generate {
        /// 指定输入的TOML文件
        #[arg(value_name = "TOML_FILE")]
        toml_file: PathBuf,
    },
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,

    /// 配置文件路径
    #[arg(short, long, default_value = "mc_simple_patcher.toml")]
    config: String,

    /// 启用调试模式
    #[arg(short, long)]
    debug: bool,

    /// SHA256模式：计算指定文件的SHA256哈希值
    #[arg(short, long, value_name = "FILE")]
    sha256: Option<std::path::PathBuf>,
}

/// 程序入口点
///
/// # Returns
///
/// * `Result<()>` - 成功时返回空值，失败时返回错误
#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // 初始化日志系统
    logger::init_logger(args.debug)?;

    log::info!("Minecraft 简易补丁工具启动");

    let result = match args.command {
        Some(Commands::Generate { toml_file }) => {
            // 如果提供了 generate 子命令，则进入生成模式
            generator::generate_config_from_toml(toml_file).await
        }
        None => {
            // 如果没有提供子命令，则执行原有的补丁逻辑
            if let Some(sha256_path) = args.sha256 {
                // 如果提供了 --sha256 参数，则计算文件的SHA256哈希值
                calculate_and_print_file_sha256(&sha256_path)
            } else if is_https_scheme(&args.config) {
                log::info!("配置文件是网络路径，尝试下载到本地……");
                execute_with_config_2update(args.config).await
            } else {
                let config_path = PathBuf::from(&args.config);
                execute_with_config(&config_path).await
            }
        }
    };

    if let Err(e) = &result {
        // 打印完整的错误链
        utils::print_error_chain(e);
    } else {
        log::info!("程序执行完成");
    }

    // 无论成功还是失败，都等待用户按键退出
    pause_before_exit();

    result
}

/// 检查URL是否为HTTPS协议
///
/// # Arguments
///
/// * `url` - URL字符串的引用
///
/// # Returns
///
/// * `bool` - 如果是HTTP或HTTPS协议则返回true，否则返回false
fn is_https_scheme(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

/// 使用URL配置执行更新
///
/// # Arguments
///
/// * `url` - 配置文件的URL
///
/// # Returns
///
/// * `Result<()>` - 成功时返回空值，失败时返回错误
async fn execute_with_config_2update(url: String) -> Result<()> {
    // 构建只有metadata-url的config来调起下载行为
    let mut mock_config = Config::default();
    mock_config.metadata_config.metadata = Some(url);
    global_config::set_global_config(mock_config);

    // 更新metadata到本地
    let config = update_metadata(PathBuf::from("mc_simple_patcher.toml").as_path())
        .await
        .context("更新metadata失败！")?;
    global_config::set_global_config(config);

    // 根据新config继续执行之后的内容
    main_controller::execute_patch(get_global_config()).await
}

/// 检查并执行自更新
///
/// # Returns
///
/// 无返回值
async fn check_self_update() {
    // 解析配置文件以检查是否有更新URL
    let config = get_global_config();
    match utils::downloader::self_update::check_for_update(config.as_ref()).await {
        Ok(updated) => {
            if updated {
                log::info!("程序已更新，请重新运行程序");
                std::process::exit(0);
            }
        }
        Err(e) => {
            log::error!("自更新失败: {}", e);
            crate::utils::print_error_chain(&e);
        }
    }
}

/// 程序退出前暂停，等待用户按键
///
/// # Returns
///
/// 无返回值
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
///
/// # Arguments
///
/// * `config_path` - 配置文件路径的引用
///
/// # Returns
///
/// * `Result<()>` - 成功时返回空值，失败时返回错误
async fn execute_with_config(config_path: &std::path::Path) -> Result<()> {
    log::info!("正在解析配置文件: {}", config_path.display());
    let config = parse_metadata_with_update(config_path).await?;

    // 初始化全局配置
    global_config::set_global_config(config);

    // 检查自更新
    check_self_update().await;

    pause_before_exit();
    main_controller::execute_patch(get_global_config()).await
}

/// 更新元数据
///
/// # Arguments
///
/// * `dst` - 目标路径的引用
///
/// # Returns
///
/// * `Option<config::Config>` - 成功时返回配置选项，失败时返回 None
async fn update_metadata(dst: &Path) -> Option<config::Config> {
    match downloader::update_metadata(dst).await {
        Err(e) => {
            utils::print_error_chain(&e);
            None
        }
        // 元数据文件已是最新无需下载和重新解析
        Ok(false) => None,
        Ok(true) => {
            log::info!("元数据文件已更新: {}", dst.display());
            // 重新解析配置文件
            match config::parse_config(dst) {
                Ok(c) => Some(c),
                Err(e) => {
                    utils::print_error_chain(&e);
                    None
                }
            }
        }
    }
}

/// 解析元数据并更新
///
/// # Arguments
///
/// * `config_path` - 配置文件路径的引用
///
/// # Returns
///
/// * `Result<config::Config>` - 成功时返回配置对象，失败时返回错误
async fn parse_metadata_with_update(config_path: &std::path::Path) -> Result<config::Config> {
    let config = config::parse_config(config_path)
        .with_context(|| format!("解析配置文件失败: {}", config_path.display()))?;

    global_config::set_global_config(config.clone());
    let metadata_url = config.metadata_config.metadata.as_ref().unwrap();
    log::info!("尝试更新元数据: {}", metadata_url);

    Ok(update_metadata(config_path).await.unwrap_or_else(|| {
        log::warn!("元数据未更新！");
        config
    }))
}
