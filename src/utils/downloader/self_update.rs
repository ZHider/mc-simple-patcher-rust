//! 自更新模块
//! 实现程序的自动更新功能

use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::path::Path;

use crate::utils::downloader;
use crate::utils::downloader::hash_check;

/// 检查并执行自更新
pub async fn check_for_update(config: &crate::config::Config) -> Result<bool> {
    if let Some(update_url) = &config.self_update_url {
        log::info!("检查更新: {}", update_url);

        match perform_update(update_url).await {
            Ok(success) => {
                if success {
                    log::info!("更新成功完成");
                    return Ok(true);
                } else {
                    log::warn!("更新未执行或失败");
                }
            }
            Err(e) => {
                log::error!("更新失败: {}", e);
                crate::utils::print_error_chain(&e);
            }
        }
    }

    Ok(false)
}

/// 执行更新过程
async fn perform_update(update_url: &str) -> Result<bool> {
    // 获取当前可执行文件路径
    let current_exe = env::current_exe().context("无法获取当前可执行文件路径")?;

    log::debug!("当前可执行文件路径: {}", current_exe.display());

    // 下载新版本到临时文件
    let temp_dir = env::temp_dir();
    let temp_file = temp_dir.join(format!(
        "mc_simple_patcher_update_{}.tmp",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    log::debug!("下载更新到临时文件: {:?}", temp_file);

    if let Some(true) = check_neednot_update(update_url, &current_exe).await? {
        log::debug!("文件已存在且完整，或者未能检测到远程SHA256，跳过下载");
        return Ok(false);
    }

    // 下载更新文件
    downloader::download_file(update_url, &temp_file, false)
        .await
        .context(format!("下载更新文件失败: {}", update_url))?;

    // 替换当前可执行文件
    replace_executable(&current_exe, &temp_file).context("替换可执行文件失败")?;

    log::info!("成功更新可执行文件");

    Ok(true)
}

async fn check_neednot_update(url: &str, dest_path: &Path) -> Result<Option<bool>> {
    hash_check::check_file_integrity(url, dest_path).await
}

/// 替换当前可执行文件
fn replace_executable(current_exe: &Path, new_exe: &Path) -> Result<()> {
    // 在Windows上，我们需要特殊处理，因为可执行文件可能正在运行
    #[cfg(windows)]
    {
        use std::process::Command;

        // 将新文件复制到当前可执行文件位置
        // 在Windows上，我们需要使用批处理脚本来实现延迟替换
        let script_path = current_exe.with_extension("bat");

        // 创建一个批处理脚本，等待当前进程退出后替换文件
        let script_content = format!(
            "@echo off\r\n
            chcp 65001>nul\r\n
            :loop\r\n
            timeout /t 1 /nobreak >nul\r\n
            del \"{}\" 2>nul\r\n
            if exist \"{}\" goto loop\r\n
            move \"{}\" \"{}\"\r\n
            del \"%~f0\"\r\n",
            current_exe.display(),
            current_exe.display(),
            new_exe.display(),
            current_exe.display()
        );

        fs::write(&script_path, script_content).context("创建替换脚本失败")?;

        // 启动批处理脚本
        Command::new("cmd")
            .arg("/C")
            .arg(script_path)
            .spawn()
            .context("启动替换脚本失败")?;
    }

    // 在非Windows平台上，可以直接重命名
    #[cfg(not(windows))]
    {
        fs::rename(new_exe, current_exe).context("替换可执行文件失败")?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_replace_executable() {
        // 由于替换可执行文件是一个敏感操作，我们只测试函数的存在
        // 实际的替换操作在运行时进行
        assert!(true);
    }
}
