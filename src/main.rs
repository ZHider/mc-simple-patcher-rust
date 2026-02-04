use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

pub mod config;
pub mod anchor_finder;
pub mod file_manager;
pub mod downloader;
pub mod main_controller;
pub mod generator;
pub mod logger;

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
    #[arg(long)]
    generate: Option<PathBuf>,

    /// 生成模式：指定用于匹配文件名的正则表达式
    #[arg(long)]
    pattern: Option<String>,

    /// 生成模式：递归扫描子目录
    #[arg(long)]
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
        calculate_file_sha256(&sha256_path)
    } else if args.generate.is_some() {
        // 如果提供了 --generate 参数，则进入生成模式
        generator::generate_config(
            args.generate.unwrap(),
            args.pattern,
            args.recursive,
            args.base_url,
            args.mod_info,
        )
    } else {
        // 否则执行原有的补丁逻辑
        execute_with_config(&args.config).await
    };

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

/// 计算文件的SHA256哈希值
fn calculate_file_sha256(file_path: &std::path::Path) -> Result<()> {
    use sha2::{Sha256, Digest};
    use std::fs::File;
    use std::io::Read;

    if !file_path.exists() {
        anyhow::bail!("指定的文件不存在: {:?}", file_path);
    }

    let mut file = File::open(file_path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 8192]; // 8KB buffer

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break; // 文件读取完毕
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let hash_bytes = hasher.finalize();
    let hash_string = format!("{:x}", hash_bytes);

    println!("{}", hash_string);
    Ok(())
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
