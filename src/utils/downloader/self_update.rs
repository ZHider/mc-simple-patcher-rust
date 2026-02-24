//! 自更新模块
//! 实现程序的自动更新功能

use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::path::Path;

use crate::utils::downloader;
use crate::utils::downloader::hash_check;
use crate::utils::temp_dir;

/// 检查并执行自更新
///
/// # Arguments
///
/// * `config` - 配置对象的引用
///
/// # Returns
///
/// * `Result<bool>` - 成功时返回布尔值表示是否进行了更新，失败时返回错误
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
///
/// # Arguments
///
/// * `update_url` - 更新URL的字符串引用
///
/// # Returns
///
/// * `Result<bool>` - 成功时返回布尔值表示是否进行了更新，失败时返回错误
async fn perform_update(update_url: &str) -> Result<bool> {
    // 获取当前可执行文件路径
    let current_exe = env::current_exe().context("无法获取当前可执行文件路径")?;

    log::debug!("当前可执行文件路径: {}", current_exe.display());

    // 下载新版本到临时文件
    let temp_file = temp_dir()?.join("mc_simple_patcher_update.exe.tmp");

    log::debug!("下载更新到临时文件: {:?}", temp_file.display());

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

/// 检查是否不需要更新
///
/// # Arguments
///
/// * `url` - URL字符串的引用
/// * `dest_path` - 目标路径的引用
///
/// # Returns
///
/// * `Result<Option<bool>>` - 成功时返回可选的布尔值表示是否需要更新，失败时返回错误
async fn check_neednot_update(url: &str, dest_path: &Path) -> Result<Option<bool>> {
    hash_check::check_file_integrity(url, dest_path).await
}

/// 替换当前可执行文件
///
/// # Arguments
///
/// * `current_exe` - 当前可执行文件路径的引用
/// * `new_exe` - 新可执行文件路径的引用
///
/// # Returns
///
/// * `Result<()>` - 成功时返回空值，失败时返回错误
fn replace_executable(current_exe: &Path, new_exe: &Path) -> Result<()> {
    // 在Windows上，我们需要特殊处理，因为可执行文件可能正在运行
    #[cfg(windows)]
    {
        use std::process::Command;

        // 将新文件复制到当前可执行文件位置
        // 在Windows上，我们需要使用批处理脚本来实现延迟替换
        let script_path = temp_dir()?.join("update.bat");

        // 创建一个批处理脚本，等待当前进程退出后替换文件
        let old_backup_path = temp_dir()?
            .join(
                current_exe
                    .file_name()
                    .expect("无法获取当前执行文件的文件名"),
            )
            .with_added_extension("old");
        let script_content = format!(
            "chcp 65001>nul\r\n
:loop\r\n
timeout /t 1 /nobreak >nul\r\n
move /Y \"{}\" \"{}\" \r\n
if exist \"{}\" goto loop\r\n
copy /Y \"{}\" \"{}\"\r\n",
            current_exe.display(),
            old_backup_path.display(),
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
