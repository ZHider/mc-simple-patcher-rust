# Minecraft 简易补丁工具

这是一个用 Rust 编写的 Minecraft 模组管理工具，可以根据配置文件自动下载和管理 Minecraft 模组。

## 功能特性

- **配置文件解析**：支持 TOML 格式的配置文件，定义模组下载规则
- **锚点搜索**：智能搜索锚点文件/文件夹以定位工作目录
- **多种匹配策略**：支持文件名、mod_id/version、正则表达式等多种文件匹配方式
- **JAR 文件解析**：能读取 JAR 文件内部的 META-INF/mods.toml 以提取模组信息
- **镜像模式**：可选择性地删除或禁用不在配置列表中的文件
- **.jar.disabled 文件恢复**：自动检测并恢复被禁用的模组文件
- **跨平台支持**：兼容 Unix 和 Windows 路径处理
- **彩色日志输出**：支持彩色日志输出，同时保存到文件
- **友好的用户界面**：提供清晰的操作提示

## 使用方法

```bash
# 使用默认配置文件 config.toml
cargo run

# 指定配置文件
cargo run -- -c my_config.toml

# 启用调试模式
cargo run -- -d

# 生成配置文件 - 扫描目录并生成配置
cargo run -- --generate <dir> --pattern "<re-pattern>" --base-url "<base-url>"

# 生成配置文件 - 递归扫描子目录
cargo run -- --generate <dir> --pattern "<re-pattern>" --recursive --base-url "<base-url>"

# 生成配置文件 - 尝试提取模组信息
cargo run -- --generate <dir> --pattern "<re-pattern>" --mod-info --base-url "<base-url>"
```

### 生成配置文件参数说明

- `--generate <dir>`: 指定要扫描的目录
- `--pattern <re-pattern>`: 指定用于匹配文件名的正则表达式
- `--recursive`: 递归扫描子目录
- `--base-url <base-url>`: 基础 URL，用于生成下载链接
- `--mod-info`: 尝试提取模组信息（mod ID 和 version）

## 配置文件格式

配置文件使用 TOML 格式，示例如下：

```toml
# 更新文件地址，每次启动都会检查更新
metadata = "http://example.com/metadata.toml"

# 版本号，每次更新后请增加此版本号以确保更新生效
version = 0

[[groups]]

# 锚点文件/文件夹，用于定位工作目录
anchor = "DeceasedCraft.jar"

# 工作目录，相对于锚点文件夹
root = "mods"

# 是否包含子文件夹中的文件
recursive = false

# 是否为镜像模式
mirror = true

# 是否删除不在列表内的文件
delete = false

# 组规则
pattern = '^.+\.jar$'

[[groups.files]]
name = "[矿石挖掘] OreExcavation-1.10.162.jar"
url = "https://example.com/files/OreExcavation-1.10.162.jar"

[[groups.files]]
mod_id = "immersiveengineering"
mod_version = "1.18.2-8.4.0-161"
url = "https://example.com/files/immersiveengineering-1.18.2-8.4.0-161.jar"

[[groups.files]]
name_pattern = '^lucky_hat-1.0.\d.jar$'
url = "https://example.com/files/JustEnoughItems-latest.jar"
```

## 架构设计

项目采用模块化设计，主要包括以下模块：

- `config.rs`：配置文件解析
- `anchor_finder.rs`：锚点搜索功能
- `file_manager.rs`：文件管理和匹配
- `downloader.rs`：网络下载功能
- `main_controller.rs`：主控制器协调各模块
- `main.rs`：程序入口点和日志系统

## 技术栈

- Rust 2024 edition
- Tokio 异步运行时
- Reqwest HTTP 客户端
- Serde 数据序列化
- ZIP 库用于处理 JAR 文件
- Env_logger 彩色日志系统
- Clap 命令行参数解析