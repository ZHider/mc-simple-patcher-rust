//! 下载器模块
//! 负责文件的下载、进度管理和完整性校验

mod batch;
pub mod bspatch;
mod client;
mod download;
mod hash_check;
mod helpers;
mod metadata;
mod progress;
pub mod self_update;

pub use batch::download_files_with_progress;
pub use client::{build_request, create_http_client};
pub use download::{DownloadTask, download_file, download_file_internal, download_patch_file_auto};
pub use helpers::get_filename_from_response;
pub use metadata::update_metadata;
