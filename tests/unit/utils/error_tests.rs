//! 错误处理工具测试

use anyhow::anyhow;
use mc_simple_patcher::utils::error::{format_error_chain, print_error_chain};

#[test]
fn test_format_error_chain_single_error() {
    let error = anyhow!("Simple error message");
    let formatted = format_error_chain(&error);

    assert!(formatted.contains("错误信息链"));
    assert!(formatted.contains("Simple error message"));
    assert!(formatted.contains("1."));
}

#[test]
fn test_format_error_chain_nested_errors() {
    let inner_error = anyhow!("Inner error");
    let middle_error = inner_error.context("Middle context");
    let outer_error = middle_error.context("Outer context");

    let formatted = format_error_chain(&outer_error);

    // 应该显示所有三个错误
    assert!(formatted.contains("错误信息链"));
    assert!(formatted.contains("1. Outer context"));
    assert!(formatted.contains("2. Middle context"));
    assert!(formatted.contains("3. Inner error"));
}

#[test]
fn test_format_error_chain_with_cause() {
    use std::io;

    let io_error = io::Error::new(io::ErrorKind::NotFound, "File not found");
    let wrapped_error = anyhow::Error::from(io_error).context("Failed to open file");

    let formatted = format_error_chain(&wrapped_error);

    assert!(formatted.contains("错误信息链"));
    assert!(formatted.contains("1. Failed to open file"));
    assert!(formatted.contains("2. File not found"));
}

#[test]
fn test_format_error_chain_empty_error() {
    // 创建一个"空"错误（虽然anyhow通常不会这样）
    let error = anyhow!("");
    let formatted = format_error_chain(&error);

    assert!(formatted.contains("错误信息链"));
    assert!(formatted.contains("1. ")); // 空消息
}

#[test]
fn test_format_error_chain_special_characters() {
    let error = anyhow!("Error with special chars: \n\t\"quotes\" & <tags>");
    let formatted = format_error_chain(&error);

    assert!(formatted.contains("错误信息链"));
    assert!(formatted.contains("Error with special chars:"));
    // 格式化应该正确处理特殊字符
    assert!(formatted.contains("quotes"));
    assert!(formatted.contains("tags"));
}

#[test]
fn test_format_error_chain_long_message() {
    let long_message = "A".repeat(1000);
    let error = anyhow!(long_message.clone());
    let formatted = format_error_chain(&error);

    assert!(formatted.contains("错误信息链"));
    // 长消息应该被完整包含
    assert!(formatted.contains(&long_message));
}

#[test]
fn test_print_error_chain_integration() {
    // 这个测试主要验证函数不会panic
    // 我们无法轻易捕获打印的输出，但可以确保它执行成功

    let error = anyhow!("Test error for printing");

    // 应该执行成功
    print_error_chain(&error);

    // 如果到达这里，测试通过
}

#[test]
fn test_error_chain_formatting_consistency() {
    let error1 = anyhow!("Error 1").context("Context 1");
    let error2 = anyhow!("Error 2").context("Context 2");

    let formatted1 = format_error_chain(&error1);
    let formatted2 = format_error_chain(&error2);

    // 格式应该一致
    assert!(formatted1.starts_with("\n === 错误信息链（Caused by）===\n"));
    assert!(formatted2.starts_with("\n === 错误信息链（Caused by）===\n"));

    assert!(formatted1.contains("1."));
    assert!(formatted2.contains("1."));

    // 结束分隔符应该一致
    assert!(formatted1.ends_with("\n--------------------------------------------------\n"));
    assert!(formatted2.ends_with("\n--------------------------------------------------\n"));
}

#[test]
fn test_error_with_multiple_contexts() {
    let error = anyhow!("Root error")
        .context("Third context")
        .context("Second context")
        .context("First context");

    let formatted = format_error_chain(&error);

    // 上下文应该按添加顺序的逆序显示
    assert!(formatted.contains("1. First context"));
    assert!(formatted.contains("2. Second context"));
    assert!(formatted.contains("3. Third context"));
    assert!(formatted.contains("4. Root error"));
}

#[test]
fn test_error_chain_with_io_error_details() {
    use std::io;

    // 创建详细的IO错误
    let io_error = io::Error::new(
        io::ErrorKind::PermissionDenied,
        "Permission denied: /root/file.txt",
    );

    let wrapped = anyhow::Error::from(io_error)
        .context("Failed to access file")
        .context("File operation failed");

    let formatted = format_error_chain(&wrapped);

    assert!(formatted.contains("1. File operation failed"));
    assert!(formatted.contains("2. Failed to access file"));
    assert!(formatted.contains("3. Permission denied"));
    assert!(formatted.contains("/root/file.txt"));
}
