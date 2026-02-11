# Minecraft 简易补丁工具

一个用 Rust 编写的 Minecraft 模组更新工具，可以根据配置文件自动下载 Minecraft 模组。

## 功能特性

- **配置文件解析**：支持 TOML 格式的配置文件，定义模组下载规则
- **锚点搜索**：智能搜索锚点文件/文件夹以定位工作目录
- **多种匹配策略**：支持多种文件匹配方式：文件名、mod_id/version、正则表达式
- **JAR 文件解析**：能读取 JAR 文件内部的 META-INF/mods.toml 以提取模组信息
- **镜像模式**：可选择性地删除或禁用不在配置列表中的文件
- **.disabled 文件**：自动检测并恢复被禁用的模组文件
- **配置文件生成**：根据规则自动生成配置文件
- **SHA256计算**：通过命令行参数调用计算文件SHA256功能
- **QUIC/HTTP/3 支持**：可选择使用 QUIC 协议进行更快的下载
- **自更新功能**：支持程序自动更新，通过配置文件中的 `self_update_url` 字段指定更新源

## Why is this？

目前拥有的更新方案，有着以下几个不便之处：

1. 更新选择不够灵活
    - 对于 `rsync`、`robocopy` 等同步方案，往往只能选中整个文件夹。开启镜像模式后，目标文件夹中不符合源文件夹的文件将被剔除。但有时，我们不需- 对整个文件夹进行操作。如 `config` 文件夹中，只需要对某个或某些特定的文件进行更新。
    - 对于 `HMCL` 提供的整合包更新功能，需要在整合包发布的时候就写好相关的数据，URL不可变，并非即插即用。

2. 对客户端有要求，可能依赖运行环境
    - 用户的运行环境上，可能缺少各种运行库，如 `git` `python` `powershell 5/7` 
    > 用 Java 对于 Minecraft 用户来说可能是更好的选择，可惜我不会 Java（哭

3. 对服务端有要求，可能需要额外架设服务

本程序：
- 使用 Rust 编写，静态链接依赖库，无需外置依赖
- 即插即用，只需将二进制文件和 toml 文件发布给用户
- 服务端仅需支持 HTTP 文件下载服务
- 支持在toml文件中预设的“文件组”中进行文件验证、文件禁用或删除
- 支持为每个文件设置下载 URL

## 使用方法

使用源码运行：

```bash
cargo run
# 如果你需要附加额外参数
cargo run -- <在这里附加参数>
```

使用二进制文件运行：

```bash
# 直接双击启动会加载同目录下的 mc_simple_patcher.toml
mc_simple_patcher.exe
# 使用debug参数输出更多信息
mc_simple_patcher.exe -d
```

### CLI Usage

```bash
Usage: mc_simple_patcher.exe [OPTIONS] [COMMAND]

Commands:
  generate  从TOML文件生成配置文件
  help      Print this message or the help of the given subcommand(s)

Options:
  -c, --config <CONFIG>  配置文件路径 [default: mc_simple_patcher.toml]
  -d, --debug            启用调试模式
      --sha256 <FILE>    SHA256模式：计算指定文件的SHA256哈希值
  -h, --help             Print help
  -V, --version          Print version
```

## 配置

### 程序基本逻辑

在配置文件中，需要更新的文件规则被分为若干组（表数组 `groups`）。对于每个组，程序进行逻辑：

1. 定位 `anchor` 文件，以此为基准。当玩家将本程序放置在 `folder/.minecraft/versions/folder/mods` 中的任意文件夹时，有优化搜索算法。因此，建议玩家将此程序放置在 `mods` 目录、其父目录，或和启动器放置在一起。
2. 相对于 `anchor` 目录，定位工作目录 `root`，在此目录下搜索文件。
3. 如果 `recursive = true`，文件搜索范围将扩大到子目录。
4. 开始搜索文件，将记录所有符合 `pattern` 的文件。
5. 对于每个 `groups.files`：
   1. 如果记录的文件中没有匹配到的文件，则该文件需要获取。
   2. 如果有 `<filename>.disabled`，且满足条件，就去掉扩展名，恢复该文件。
   3. 否则，通过 `url` 下载文件。
6. 检查完成后，对于记录中未能匹配到的文件，如果 `mirror = true`，则需要对该文件进行处理：
   1. 如果 `delete = false`，则在该文件名后增加 `.disabled`。
   2. 否则，删除该文件。

### 配置文件生成 - 子命令 generate

主要用于从现有的 mods 文件夹生成配置文件列表，批量生成 `name` `mod_id` `sha256` 等参数。

指定输入的TOML文件，根据其中的规则扫描目录并生成配置文件，生成的配置将保存为 `<input>-generated.toml`

```bash
mc_simple_patcher.exe generate <input.toml>
```

预计行为是 `[[generate]]` 之外的内容将被原样保留到生成出来的 `<input>-generated.toml` 中。

关于生成配置如何写，请查阅 [示例文件 generate.toml](./generate.toml)


### 配置文件格式

关于如何书写配置文件，请查阅 [示例文件 example.toml](./example.toml)

#### 网络配置

您可以使用 `[network]` 部分来配置网络相关选项：

```toml
[network]
# 是否强制切换到quic，只走UDP协议
quic = false
# 是否忽略证书错误（对于自签名证书等情况）
ignore_invalid_cert = true
```

- `quic`: 启用 HTTP/3 协议进行下载（默认为 false）
- `ignore_invalid_cert`: 忽略 SSL 证书错误（默认为 false）

## 源代码相关

### 文件结构设计

- `config.rs`：配置文件解析
- `anchor_finder.rs`：锚点搜索功能
- `file_manager.rs`：文件管理和匹配
- `downloader.rs`：网络下载功能
- `main_controller.rs`：主控制器协调各模块
- `main.rs`：程序入口点和日志系统

### 技术栈

- Rust 2024 edition
- Tokio 异步运行时
- Reqwest HTTP 客户端
- Serde 数据序列化
- ZIP 库用于处理 JAR 文件
- Env_logger 彩色日志系统
- Clap 命令行参数解析


## 后言

十分感谢您能看到这里！我是 Rust 初学者，写这个耗费了我很多的精力，好在没有碰到复杂所有权和生命周期的问题，仅仅是在写读取 `mod_id` 和 `mod_version` 的时候需要缓存提升性能，因此使用了 `Mutex<Hashmap>`。AI也帮了我很多，但我也坚持对代码进行检查、重构和优化，确保我的代码质量不过低。

如果各位觉得我的项目有用，希望能点个 Star，如果有任何问题、bug、反馈、希望新增的feature，或者是针对于代码风格、程序结构、流程优化的建议，也非常高兴如果您能提起issue！再次感谢！
