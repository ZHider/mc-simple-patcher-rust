//! 自更新模块
//! 实现程序的自动更新功能，支持完整下载和补丁方式

use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::path::Path;

use crate::config::SelfUpdateConfig;
use crate::utils::downloader;
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
    let self_update_config = &config.self_update;

    // 如果没有自更新配置，直接返回
    if self_update_config.url.is_none() && self_update_config.patches.is_empty() {
        log::trace!("自更新配置为空，跳过自更新检查");
        return Ok(false);
    }

    log::debug!("开始检查自更新");

    // 获取当前可执行文件路径
    let current_exe = env::current_exe().context("无法获取当前可执行文件路径")?;
    log::debug!("当前可执行文件路径：{}", current_exe.display());

    // 尝试使用补丁方式更新
    if !self_update_config.patches.is_empty() {
        log::info!(
            "检测到 {} 个自更新补丁配置，尝试补丁更新",
            self_update_config.patches.len()
        );
        match update_with_patches(self_update_config, &current_exe).await {
            Ok(true) => {
                log::info!("补丁更新成功完成");
                return Ok(true);
            }
            Ok(false) => {
                log::debug!("补丁更新未执行，将尝试完整下载");
            }
            Err(e) => {
                log::warn!("补丁更新失败：{}，将尝试完整下载", e);
            }
        }
    } else {
        log::trace!("无补丁配置，跳过补丁更新");
    }

    // 补丁更新失败或不适用，使用完整下载方式
    if let Some(update_url) = &self_update_config.url {
        log::info!("使用完整下载方式更新：{}", update_url);
        match update_with_full_download(update_url, &current_exe).await {
            Ok(success) => {
                if success {
                    log::info!("完整下载更新成功完成");
                    return Ok(true);
                } else {
                    log::debug!("完整下载未执行（文件已是最新）");
                }
            }
            Err(e) => {
                log::error!("完整下载更新失败：{}", e);
                crate::utils::print_error_chain(&e);
            }
        }
    } else {
        log::trace!("无完整下载 URL，跳过完整下载更新");
    }

    log::debug!("自更新检查完成，未进行更新");
    Ok(false)
}

/// 使用补丁方式更新
async fn update_with_patches(config: &SelfUpdateConfig, current_exe: &Path) -> Result<bool> {
    log::debug!("开始补丁方式自更新");

    // 计算当前文件的 SHA256
    log::trace!("计算当前可执行文件 SHA256: {:?}", current_exe);
    let current_hash_bytes = crate::utils::calculate_file_sha256(current_exe)?;
    let current_hash_hex = hex::encode(current_hash_bytes);
    log::debug!("当前文件 SHA256: {}", current_hash_hex);

    // 寻找匹配的补丁
    log::trace!("在 {} 个补丁配置中查找匹配项", config.patches.len());
    let matching_patch = config
        .patches
        .iter()
        .find(|patch| patch.sha256_src.eq_ignore_ascii_case(&current_hash_hex));

    let patch = match matching_patch {
        Some(p) => {
            log::info!("找到匹配的补丁配置（sha256_src 匹配当前文件）");
            log::trace!("补丁 URL: {}", p.url_patch);
            p
        }
        None => {
            log::debug!("未找到 sha256_src 与当前文件匹配的补丁");
            return Ok(false);
        }
    };

    // 验证源文件
    log::trace!("验证源文件存在");
    let src_file =
        match downloader::bspatch::find_patch_source_file(current_exe, &patch.sha256_src)? {
            Some(f) => {
                log::debug!("源文件验证通过：{:?}", f);
                f
            }
            None => {
                log::warn!("未找到匹配 sha256_src 的源文件");
                return Ok(false);
            }
        };

    // 创建临时目录作为目标
    let dst_file = temp_dir()?.join("mc_simple_patcher_update.exe");
    log::trace!("目标文件路径：{:?}", dst_file);

    // 下载补丁文件（从 response 获取文件名）
    log::info!("正在下载补丁：{}", patch.url_patch);
    let patch_path = downloader::download_patch_file_auto(&patch.url_patch).await?;
    log::debug!("补丁文件下载到：{:?}", patch_path);

    // 创建通用的 PatchDownloadTask
    log::trace!("创建补丁下载任务");
    let mut patch_tasks = vec![downloader::bspatch::PatchDownloadTask {
        url: patch.url_patch.clone(),
        dest_path: Some(patch_path),
        patch: downloader::bspatch::FilePatch {
            url_patch: patch.url_patch.clone(),
            sha256_src: patch.sha256_src.clone(),
            keep_src: patch.keep_src,
        },
        src_file,
        dst_file,
    }];

    // 应用补丁
    log::debug!("开始应用补丁");
    match downloader::bspatch::apply_downloaded_patches(&mut patch_tasks).await? {
        Some(new_path) => {
            log::info!("补丁应用成功：{}", new_path.display());

            // 替换当前可执行文件
            log::debug!("开始替换可执行文件：{:?} -> {:?}", current_exe, new_path);
            replace_executable(current_exe, &new_path).context("替换可执行文件失败")?;
            log::info!("成功更新可执行文件");

            Ok(true)
        }
        None => {
            log::warn!("补丁应用未返回目标文件");
            Ok(false)
        }
    }
}

/// 使用完整下载方式更新
async fn update_with_full_download(update_url: &str, current_exe: &Path) -> Result<bool> {
    log::debug!("开始完整下载方式自更新：{}", update_url);

    // 下载新版本到临时文件
    let temp_file = temp_dir()?.join("mc_simple_patcher_update.exe.tmp");
    log::trace!("临时文件路径：{:?}", temp_file);

    log::debug!("下载新版本到临时文件：{:?}", temp_file.display());

    // 检查是否需要更新
    log::trace!("检查文件完整性");
    if let Some(true) = check_neednot_update(update_url, current_exe).await? {
        log::debug!("文件已存在且完整，或者未能检测到远程 SHA256，跳过下载");
        return Ok(false);
    }

    // 下载更新文件
    log::info!("正在下载更新文件：{}", update_url);
    downloader::download_file(update_url, &temp_file, false)
        .await
        .context(format!("下载更新文件失败：{}", update_url))?;
    log::debug!("更新文件下载完成：{:?}", temp_file);

    // 替换当前可执行文件
    log::debug!("开始替换可执行文件：{:?} -> {:?}", current_exe, temp_file);
    replace_executable(current_exe, &temp_file).context("替换可执行文件失败")?;

    log::info!("成功更新可执行文件");

    Ok(true)
}

/// 检查是否不需要更新
async fn check_neednot_update(url: &str, dest_path: &Path) -> Result<Option<bool>> {
    downloader::hash_check::check_file_integrity(url, dest_path).await
}

/// 替换当前可执行文件
fn replace_executable(current_exe: &Path, new_exe: &Path) -> Result<()> {
    log::debug!("替换可执行文件：{:?} -> {:?}", current_exe, new_exe);

    // 在 Windows 上，我们需要特殊处理，因为可执行文件可能正在运行
    #[cfg(windows)]
    {
        use std::process::Command;

        log::trace!("Windows 平台，创建批处理脚本进行延迟替换");

        // 将新文件复制到当前可执行文件位置
        // 在 Windows 上，我们需要使用批处理脚本来实现延迟替换
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

        log::trace!("创建替换脚本：{:?}", script_path);
        fs::write(&script_path, script_content).context("创建替换脚本失败")?;

        // 启动批处理脚本
        log::debug!("启动替换脚本：{:?}", script_path);
        Command::new("cmd")
            .arg("/C")
            .arg(script_path)
            .spawn()
            .context("启动替换脚本失败")?;
        log::debug!("替换脚本已启动，将在当前进程退出后执行替换");
    }

    // 在非 Windows 平台上，可以直接重命名
    #[cfg(not(windows))]
    {
        log::trace!("非 Windows 平台，直接重命名");
        fs::rename(new_exe, current_exe).context("替换可执行文件失败")?;
        log::debug!("可执行文件替换完成");
    }

    Ok(())
}
