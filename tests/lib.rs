//! 项目集成测试入口点
//! 这个文件会被cargo test自动发现

pub static TEST_WORKSPACE: std::sync::LazyLock<std::path::PathBuf> =
    std::sync::LazyLock::new(|| {
        std::env::current_dir()
            .unwrap()
            .join("tests/test_workspace")
    });

// 导入测试模块
mod common;
mod unit;

// 重新导出以便测试使用
pub use common::*;
pub use unit::*;

// 重新导出宏
// 宏已通过 common/mod.rs 的 pub use setup::*; 导出
