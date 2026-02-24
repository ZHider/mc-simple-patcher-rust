//! 错误处理工具模块

use anyhow::Error;

/// 打印完整的错误信息和错误链
pub fn print_error_chain(err: &Error) {
    log::error!("{}", format_error_chain(err));
}

/// 格式化错误信息链
pub fn format_error_chain(err: &Error) -> String {
    let mut result = "\n === 错误信息链（Caused by）===\n".to_string();
    err.chain().enumerate().for_each(|(i, cause)| {
        result.push_str(format!("  {}. {}\n", i + 1, cause).as_str());
    });
    result.push_str(format!("\n{}\n", "-".repeat(50)).as_str());
    result
}
