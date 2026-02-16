use crate::utils::{calculate_file_sha256, get_filename};

use anyhow::{Context, Result};
use bytes::Bytes;
use indicatif::ProgressBar;
use rayon::iter::ParallelIterator;
use std::{
    collections::{HashMap, HashSet},
    io::Read,
    path::Path,
    sync::{Arc, Mutex},
};

pub struct ModInfoCache {
    pub sha256: HashSet<Bytes>,
    pub mod_id: HashMap<Arc<str>, Arc<str>>,
    pub mod_version: HashMap<Arc<str>, Arc<str>>,
}

struct ModInfo {
    pub sha256: Option<Bytes>,
    pub mod_id: Option<Arc<str>>,
    pub mod_version: Option<(Arc<str>, Arc<str>)>,
}

pub fn extract_modinfo<I>(files: I, capacity: Option<usize>) -> Result<ModInfoCache>
where
    I: ParallelIterator,
    I::Item: AsRef<Path>,
{
    let capacity = capacity.unwrap_or(0);

    // 进度跟踪器
    let progress = ExtractProgressTracker::new(capacity);

    let mod_infos: Vec<_> = files
        .map(|file| {
            let path = file.as_ref().to_owned();
            let mod_info = extract_file_info(&progress, &path);
            (path, mod_info)
        })
        .collect();
    progress.pb.lock().unwrap().finish();

    let mut mod_cache = ModInfoCache {
        sha256: HashSet::with_capacity(capacity),
        mod_id: HashMap::with_capacity(capacity),
        mod_version: HashMap::with_capacity(capacity),
    };

    println!();
    log::info!("文件检索完成，开始构建缓存……");

    for (file, mod_info) in mod_infos {
        if let Some(sha256) = mod_info.sha256 {
            mod_cache.sha256.insert(sha256);
        }
        if let Some(mod_id) = mod_info.mod_id {
            let file_name = get_filename(&file)?;
            mod_cache.mod_id.insert(file_name, mod_id.clone());

            if let Some((_, mod_version)) = mod_info.mod_version {
                mod_cache.mod_version.insert(mod_id, mod_version);
            }
        }
    }

    Ok(mod_cache)
}

fn extract_file_info(progress: &ExtractProgressTracker, file: &Path) -> ModInfo {
    log::debug!("正在缓存文件: {}", file.display());

    let mut mod_info = ModInfo {
        sha256: None,
        mod_id: None,
        mod_version: None,
    };
    // 解析mod信息
    match extract_mod_info_from_jar(file) {
        Ok((mod_id_str, mod_version_str)) => {
            let mod_id_arc: Arc<str> = Arc::from(mod_id_str);
            mod_info.mod_id = Some(mod_id_arc.clone());
            mod_info.mod_version = Some((mod_id_arc, Arc::from(mod_version_str)));
        }
        Err(e) => {
            log::debug!("未能从文件 {} 提取mod信息: {}", file.display(), e);
        }
    }

    // 计算并缓存SHA256
    match calculate_file_sha256(file) {
        Ok(sha256_bytes) => {
            mod_info.sha256 = Some(sha256_bytes);
        }
        Err(e) => {
            log::debug!("计算文件 {} 的SHA256失败: {}", file.display(), e);
        }
    }
    // 更新进度
    progress.update();

    mod_info
}

pub struct ExtractProgressTracker {
    pub pb: Mutex<ProgressBar>,
}
impl ExtractProgressTracker {
    pub fn new(total: usize) -> Self {
        let pb = ProgressBar::new(total as u64);
        pb.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("[{bar:40.cyan/blue}] {pos}/{len} ({percent}%) {msg}")
                .unwrap()
                .progress_chars("=>-"),
        );
        pb.set_message("文件信息提取中: ");
        Self { pb: Mutex::new(pb) }
    }

    pub fn update(&self) {
        let guard = self.pb.lock().unwrap();
        guard.inc(1);
    }
}

/// 从 JAR 文件中提取 mod 信息
///
/// # Arguments
///
/// * `jar_path` - JAR 文件路径的引用
///
/// # Returns
///
/// * `Result<(String, String)>` - 成功时返回 (mod_id, mod_version) 元组，失败时返回错误
pub fn extract_mod_info_from_jar(jar_path: &Path) -> Result<(String, String)> {
    use std::fs::File;
    use zip::ZipArchive;

    /// 在ZIP存档中查找 fabric.mod.json 文件
    fn extract_fabric_mod(archive: &mut ZipArchive<std::fs::File>) -> Result<(String, String)> {
        let mut file = archive
            .by_name("fabric.mod.json")
            .context("JAR 文件中未找到 fabric.mod.json")?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        let content: String = content.chars().filter(|c| !c.is_control()).collect();

        // 解析json内容
        let json_value: HashMap<String, serde_json::Value> = serde_json::from_str(&content)?;
        let mod_id = json_value
            .get("id")
            .context("无法从 JSON 中提取 id")?
            .as_str()
            .context("id 键的值类型不是 str")?
            .to_owned();
        let mod_version = json_value
            .get("version")
            .context("无法从 JSON 中提取 version")?
            .as_str()
            .context("version 键的值类型不是 str")?
            .to_owned();

        Ok((mod_id, mod_version))
    }

    /// 在ZIP存档中查找 mods.toml 文件
    fn extract_forge_mod(archive: &mut ZipArchive<std::fs::File>) -> Result<(String, String)> {
        let mut file = archive
            .by_name("META-INF/mods.toml")
            .context("JAR 文件中未找到 META-INF/mods.toml")?;

        let mut content = String::new();
        file.read_to_string(&mut content)?;

        // 解析 toml 内容
        let toml_value: toml::Value = toml::from_str(&content)?;
        // log::debug!("解析 mods.toml 内容: {:?}", toml_value);

        // 提取 mod_id 和 version
        let mod_base = toml_value
            .get("mods")
            .and_then(|mods| mods.get(0))
            .context("mods.toml 格式不正确，缺少 mods 部分")?;

        let mod_id = mod_base
            .get("modId")
            .and_then(|id| id.as_str())
            .context("无法从 mods.toml 中提取 modId")?
            .to_owned();

        let mod_version = mod_base
            .get("version")
            .and_then(|ver| ver.as_str())
            .context("无法从 mods.toml 中提取 version")?
            .to_owned();

        Ok((mod_id, mod_version))
    }

    let file = File::open(jar_path)?;
    let mut archive = ZipArchive::new(file)?;

    extract_forge_mod(&mut archive)
        .context("Forge模式MOD信息提取失败！")
        .or_else(|e| extract_fabric_mod(&mut archive).context(e))
        .context("Fabric模式MOD信息提取失败！")
}
