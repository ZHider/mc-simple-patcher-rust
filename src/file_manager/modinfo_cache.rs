use crate::utils::calculate_file_sha256;

use anyhow::{Context, Result};
use bytes::Bytes;
use std::{
    collections::{HashMap, HashSet}, io::Write, path::Path, sync::{Arc, Mutex}
};
use zip::ZipArchive;

pub struct ModInfoCache {
    pub sha256: HashSet<Bytes>,
    pub mod_id: HashSet<Arc<str>>,
    pub mod_version: HashMap<Arc<str>, String>,
}

pub fn extract_modinfo<I>(files: I, capacity: Option<usize>) -> Result<ModInfoCache>
where
    I: Iterator,
    I::Item: AsRef<Path>,
{
    let capacity = capacity.unwrap_or(0);
    let mut cache = ModInfoCache {
        sha256: HashSet::with_capacity(capacity),
        mod_id: HashSet::with_capacity(capacity),
        mod_version: HashMap::with_capacity(capacity),
    };
    // 进度跟踪器
    let progress = ExtractProgressTracker::new(capacity);

    for file in files {
        let file = file.as_ref();
        log::debug!("正在缓存文件: {}", file.display());
        // 解析mod信息
        match extract_mod_info_from_jar(file) {
            Ok((mod_id_str, mod_version_str)) => {
                let mod_id_arc: Arc<str> = Arc::from(mod_id_str);
                cache.mod_id.insert(mod_id_arc.clone());
                cache.mod_version.insert(mod_id_arc, mod_version_str);
            }
            Err(e) => {
                log::debug!("未能从文件 {} 提取mod信息: {}", file.display(), e);
            }
        }

        // 计算并缓存SHA256
        match calculate_file_sha256(file) {
            Ok(sha256_bytes) => {
                cache.sha256.insert(sha256_bytes);
            }
            Err(e) => {
                log::debug!("计算文件 {} 的SHA256失败: {}", file.display(), e);
            }
        }
        // 更新进度
        progress.update();
    }

    Ok(cache)
}


pub struct ExtractProgressTracker {
    mutex: Mutex<(usize, usize)>,
}
impl ExtractProgressTracker {
    pub fn new(total: usize) -> Self {
        Self {
            mutex: Mutex::new((0, total)),
        }
    }

    pub fn update(&self) {
        let mut guard = self.mutex.lock().unwrap();
        *guard = (guard.0 + 1, guard.1);
        print!("\r已提取 {} / {} 个文件", guard.0, guard.1);
        drop(guard);
        std::io::stdout().flush().unwrap();
    }
}

/// 从 JAR 文件中提取 mod 信息
pub fn extract_mod_info_from_jar(jar_path: &Path) -> Result<(String, String)> {
    use std::fs::File;
    use zip::ZipArchive;

    let file = File::open(jar_path)?;
    let mut archive = ZipArchive::new(file)?;

    // 查找 META-INF/mods.toml 文件
    let mods_toml_content = find_mods_toml_in_archive(&mut archive)?;

    // 解析 toml 内容
    let toml_value: toml::Value = toml::from_str(&mods_toml_content)?;
    // log::debug!("解析 mods.toml 内容: {:?}", toml_value);

    // 提取 mod_id 和 version
    let mod_base = toml_value
        .get("mods")
        .and_then(|mods| mods.get(0))
        .context("mods.toml 格式不正确，缺少 mods 部分")?;

    let mod_id = mod_base
        .get("modId")
        .and_then(|id| id.as_str())
        .context("无法从 mods.toml 中提取 modId")?;

    let mod_version = mod_base
        .get("version")
        .and_then(|ver| ver.as_str())
        .context("无法从 mods.toml 中提取 version")?;

    Ok((mod_id.to_string(), mod_version.to_string()))
}

/// 在ZIP存档中查找mods.toml文件
fn find_mods_toml_in_archive(archive: &mut ZipArchive<std::fs::File>) -> Result<String> {
    let mut file = archive
        .by_name("META-INF/mods.toml")
        .map_err(|_| anyhow::anyhow!("JAR 文件中未找到 META-INF/mods.toml"))?;

    let mut content = String::new();
    std::io::Read::read_to_string(&mut file, &mut content)?;
    Ok(content)
}
