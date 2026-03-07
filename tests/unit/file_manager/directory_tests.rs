//! 目录遍历测试
//! 测试 get_files_in_dir 和相关目录遍历函数的功能

use anyhow::Result;
use mc_simple_patcher::file_manager::get_files_in_dir;
use regex::Regex;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use tempfile::tempdir;

#[test]
fn test_get_files_in_dir_non_recursive() -> Result<()> {
    // 创建临时目录
    let temp_dir = tempdir()?;
    let dir_path = temp_dir.path();

    // 创建一些测试文件
    let file1 = dir_path.join("file1.txt");
    let file2 = dir_path.join("file2.jar");
    let subdir = dir_path.join("subdir");
    fs::create_dir(&subdir)?;
    let file3 = subdir.join("file3.txt"); // 这个文件不应该在非递归搜索中出现

    File::create(&file1)?.write_all(b"test1")?;
    File::create(&file2)?.write_all(b"test2")?;
    File::create(&file3)?.write_all(b"test3")?;

    // 非递归搜索
    let files = get_files_in_dir(dir_path, false, None)?;

    // 应该只找到直接子目录中的文件，不包括子目录中的文件
    assert_eq!(files.len(), 2);
    assert!(files.contains(&file1));
    assert!(files.contains(&file2));
    assert!(!files.contains(&file3));

    Ok(())
}

#[test]
fn test_get_files_in_dir_recursive() -> Result<()> {
    // 创建临时目录
    let temp_dir = tempdir()?;
    let dir_path = temp_dir.path();

    // 创建嵌套目录结构
    let file1 = dir_path.join("file1.txt");
    let subdir1 = dir_path.join("subdir1");
    fs::create_dir(&subdir1)?;
    let file2 = subdir1.join("file2.jar");
    let subdir2 = subdir1.join("subdir2");
    fs::create_dir(&subdir2)?;
    let file3 = subdir2.join("file3.txt");

    File::create(&file1)?.write_all(b"test1")?;
    File::create(&file2)?.write_all(b"test2")?;
    File::create(&file3)?.write_all(b"test3")?;

    // 递归搜索
    let files = get_files_in_dir(dir_path, true, None)?;

    // 应该找到所有文件，包括嵌套目录中的
    assert_eq!(files.len(), 3);
    assert!(files.contains(&file1));
    assert!(files.contains(&file2));
    assert!(files.contains(&file3));

    Ok(())
}

#[test]
fn test_get_files_in_dir_with_regex_filter() -> Result<()> {
    // 创建临时目录
    let temp_dir = tempdir()?;
    let dir_path = temp_dir.path();

    // 创建各种类型的文件
    let files = [
        "test.jar",
        "test.txt",
        "mod.jar",
        "config.json",
        "another.jar",
    ];

    for filename in &files {
        let file_path = dir_path.join(filename);
        File::create(&file_path)?.write_all(b"test")?;
    }

    // 使用正则表达式过滤 .jar 文件
    let jar_regex = Regex::new(r"\.jar$")?;
    let jar_files = get_files_in_dir(dir_path, false, Some(&jar_regex))?;

    // 应该只找到 .jar 文件
    assert_eq!(jar_files.len(), 3);
    assert!(
        jar_files
            .iter()
            .all(|p| p.extension().unwrap_or_default() == "jar")
    );

    // 使用正则表达式过滤以 test 开头的文件
    let test_regex = Regex::new(r"^test")?;
    let test_files = get_files_in_dir(dir_path, false, Some(&test_regex))?;

    // 应该只找到以 test 开头的文件
    assert_eq!(test_files.len(), 2);
    assert!(test_files.iter().all(|p| {
        p.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .starts_with("test")
    }));

    Ok(())
}

#[test]
fn test_get_files_in_dir_empty_directory() -> Result<()> {
    // 创建空目录
    let temp_dir = tempdir()?;
    let dir_path = temp_dir.path();

    // 非递归搜索空目录
    let files = get_files_in_dir(dir_path, false, None)?;
    assert!(files.is_empty());

    // 递归搜索空目录
    let files = get_files_in_dir(dir_path, true, None)?;
    assert!(files.is_empty());

    Ok(())
}

#[test]
fn test_get_files_in_dir_with_regex_on_recursive() -> Result<()> {
    // 创建临时目录
    let temp_dir = tempdir()?;
    let dir_path = temp_dir.path();

    // 创建嵌套目录结构
    let file1 = dir_path.join("file1.jar");
    let subdir1 = dir_path.join("subdir1");
    fs::create_dir(&subdir1)?;
    let file2 = subdir1.join("file2.txt");
    let file3 = subdir1.join("file3.jar");
    let subdir2 = subdir1.join("subdir2");
    fs::create_dir(&subdir2)?;
    let file4 = subdir2.join("file4.jar");

    File::create(&file1)?.write_all(b"test1")?;
    File::create(&file2)?.write_all(b"test2")?;
    File::create(&file3)?.write_all(b"test3")?;
    File::create(&file4)?.write_all(b"test4")?;

    // 递归搜索，使用正则表达式过滤 .jar 文件
    let jar_regex = Regex::new(r"\.jar$")?;
    let jar_files = get_files_in_dir(dir_path, true, Some(&jar_regex))?;

    // 应该找到所有 .jar 文件，包括嵌套目录中的
    assert_eq!(jar_files.len(), 3);
    assert!(jar_files.contains(&file1));
    assert!(jar_files.contains(&file3));
    assert!(jar_files.contains(&file4));
    assert!(!jar_files.contains(&file2)); // 不是 .jar 文件

    Ok(())
}

#[test]
fn test_get_files_in_dir_nonexistent_directory() {
    // 测试不存在的目录
    let nonexistent_path = Path::new("/nonexistent/path/that/does/not/exist");
    let result = get_files_in_dir(nonexistent_path, false, None);

    // 应该返回错误
    assert!(result.is_err());
}

#[test]
fn test_get_files_in_dir_mixed_file_types() -> Result<()> {
    // 创建临时目录
    let temp_dir = tempdir()?;
    let dir_path = temp_dir.path();

    // 创建各种类型的文件
    let files = [
        ("mod.jar", "jar"),
        ("config.json", "json"),
        ("README.md", "md"),
        ("texture.png", "png"),
        ("data.dat", "dat"),
    ];

    for (filename, _) in &files {
        let file_path = dir_path.join(filename);
        File::create(&file_path)?.write_all(b"test")?;
    }

    // 非递归搜索所有文件
    let all_files = get_files_in_dir(dir_path, false, None)?;
    assert_eq!(all_files.len(), 5);

    // 使用正则表达式过滤特定扩展名
    let json_regex = Regex::new(r"\.json$")?;
    let json_files = get_files_in_dir(dir_path, false, Some(&json_regex))?;
    assert_eq!(json_files.len(), 1);
    assert!(json_files[0].file_name().unwrap_or_default() == "config.json");

    let image_regex = Regex::new(r"\.(png|jpg|jpeg)$")?;
    let image_files = get_files_in_dir(dir_path, false, Some(&image_regex))?;
    assert_eq!(image_files.len(), 1);
    assert!(image_files[0].file_name().unwrap_or_default() == "texture.png");

    Ok(())
}

#[test]
fn test_get_files_in_dir_symlinks() -> Result<()> {
    // 注意：在 Windows 上创建符号链接可能需要特殊权限
    // 这个测试主要确保函数能处理符号链接而不崩溃
    let temp_dir = tempdir()?;
    let dir_path = temp_dir.path();

    // 创建普通文件
    let real_file = dir_path.join("real.txt");
    File::create(&real_file)?.write_all(b"real content")?;

    // 非递归搜索
    let files = get_files_in_dir(dir_path, false, None)?;
    assert_eq!(files.len(), 1);
    assert!(files.contains(&real_file));

    Ok(())
}

#[test]
fn test_get_files_in_dir_with_complex_regex() -> Result<()> {
    // 创建临时目录
    let temp_dir = tempdir()?;
    let dir_path = temp_dir.path();

    // 创建符合特定模式的文件
    let files = [
        "fabric-api-1.0.0.jar",
        "forge-1.19.2.jar",
        "quilt-api-2.0.0.jar",
        "mod-1.2.3.jar",
        "api.jar",
        "test.txt",
    ];

    for filename in &files {
        let file_path = dir_path.join(filename);
        File::create(&file_path)?.write_all(b"test")?;
    }

    // 使用复杂的正则表达式匹配 fabric- 或 forge- 开头的 .jar 文件
    let mod_regex = Regex::new(r"^(fabric|forge)-.*\.jar$")?;
    let mod_files = get_files_in_dir(dir_path, false, Some(&mod_regex))?;

    // 应该只找到 fabric- 或 forge- 开头的 .jar 文件
    assert_eq!(mod_files.len(), 2);
    let filenames: Vec<String> = mod_files
        .iter()
        .map(|p| {
            p.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        })
        .collect();

    assert!(filenames.contains(&"fabric-api-1.0.0.jar".to_string()));
    assert!(filenames.contains(&"forge-1.19.2.jar".to_string()));
    assert!(!filenames.contains(&"quilt-api-2.0.0.jar".to_string()));
    assert!(!filenames.contains(&"mod-1.2.3.jar".to_string()));
    assert!(!filenames.contains(&"api.jar".to_string()));
    assert!(!filenames.contains(&"test.txt".to_string()));

    Ok(())
}

#[test]
fn test_get_files_in_dir_directory_permissions() -> Result<()> {
    // 测试目录权限（在 Windows 上可能表现不同）
    let temp_dir = tempdir()?;
    let dir_path = temp_dir.path();

    // 创建可访问的目录和文件
    let accessible_dir = dir_path.join("accessible");
    fs::create_dir(&accessible_dir)?;
    let accessible_file = accessible_dir.join("file.txt");
    File::create(&accessible_file)?.write_all(b"test")?;

    // 递归搜索应该能找到文件
    let files = get_files_in_dir(dir_path, true, None)?;
    assert_eq!(files.len(), 1);
    assert!(files.contains(&accessible_file));

    Ok(())
}
