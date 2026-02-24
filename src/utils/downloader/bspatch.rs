use std::path::{Path, PathBuf};

use anyhow::{Context, Ok, Result};
use qbsdiff::Bspatch;
use tokio::fs;

pub async fn bspatch(patch_file: &Path, src_file: &Path, dst_file: &Path) -> Result<()> {
    log::debug!(
        "开始进行文件 bspatch：'{}' + '{}' -> '{}'",
        src_file.display(),
        patch_file.display(),
        dst_file.display()
    );

    let src = fs::read(src_file).await.context("读取src文件到内存失败")?;
    let patch = fs::read(patch_file)
        .await
        .context("读取 patch file 到内存失败")?;
    let output_writer = std::fs::File::create(dst_file)?;

    let patched_bytes = Bspatch::new(&patch)
        .context("创建bspatch对象失败")?
        .buffer_size(128000)
        .apply(&src, output_writer)
        .context("进行文件patch时错误")?;
    log::debug!("文件patch完成，输出 {} bytes", patched_bytes);

    Ok(())
}

pub fn path_backup(file: &Path) -> PathBuf {
    file.with_added_extension("backup")
}