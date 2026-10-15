<!-- 译自 docs/en/architecture.md。请修改源文件后重新翻译；请勿直接修改本文件。 -->

# 架构

[← 文档索引](index.md)

camareLOL 由三个组件构成，它们之间只有一条规则：**只有 Rust 核心
可以更改系统。** 另外两个可以提出请求。

```
┌──────────────────────────────────────────────────────────────────┐
│                        camareLOL                                 │
│                                                                    │
│   ┌────────────┐   ┌──────────────┐   ┌───────────────────────┐  │
│   │  TUI       │   │  Python      │   │  Java catalog         │  │
│   │  (ratatui) │   │  tools       │   │  (package catalog,    │  │
│   │            │   │              │   │   desktop profiles,   │  │
│   │  renders,  │   │  validates,  │   │   plugin interface,   │  │
│   │  navigates │   │  recommends, │   │   JSON-RPC server)    │  │
│   └─────┬──────┘   │  diagnoses   │   └──────────┬────────────┘  │
│         │          └──────┬───────┘              │               │
│         └────────┬────────┴──────────────────────┘               │
│                  │ versioned JSON protocol (NDJSON)              │
│                  ▼                                                │
│   ┌──────────────────────────────────────────────────────────┐  │
│   │              Rust core (authoritative)                    │  │
│   │  hardware · storage · layout · boot · accounts · packages │  │
│   │  config · logging · recovery · install orchestration      │  │
│   │  safety gate — every system-changing request passes here  │  │
│   └──────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────┘
```

## 职责

| 组件 | 必须做 | 绝不能做 |
| --- | --- | --- |
| **Rust 核心** | 硬件与存储发现、分区规划、文件系统与引导配置、安装编排、安全校验、特权操作 | 把密码存到磁盘上、在安全闸门未经确认时运行破坏性命令、不经重新验证就信任某个设备路径 |
| **Python** | 自动化、配置校验、软件包推荐、依赖分析、诊断、测试工具 | 执行破坏性操作、绕过安全层、悄悄禁用某项功能——缺少运行时必须被**报告出来** |
| **Java** | 可扩展的软件包目录、桌面环境配置档、插件接口、基于既定 API 的配置模块 | 执行破坏性操作、绕过安全层 |

## 为什么是核心，而不是脚本

另一种设计是一个调用 `sgdisk` 和 `mkfs` 的 shell 脚本。它写起来
更简单，却在最关键的时刻根本无法推理：那正是磁盘即将被擦除的时刻。

把所有会更改系统的调用都放到同一个组件之后，调用方就无法把检查
忘掉。Python 和 Java 加功能更快，因此也更可能长出新代码——所以
核心会以与校验来自 TUI 的请求完全相同的方式，校验来自它们的每一个请求。

## 构建顺序

1. `core/` —— Rust 库：`cargo build -p camarelol-core`
2. `tui/` —— `camarelol` 二进制文件，它依赖核心
3. `python/` —— 无构建步骤：`python3 -m unittest discover -s tests`
4. `java/` —— `cd java && ./build.sh`（javac、jar、测试）

仓库根目录下的 `./build.sh` 会完成以上四步。

## IPC 协议

一句话概括：带请求 id、带协议版本、带结构化类型错误、带按请求超时
和取消机制的换行分隔 JSON。见 [IPC 协议](ipc-protocol.md)。

真正要紧的细节：核心会校验**每一个**请求。没有核心的同意，
Python 工具无法触及任何写入路径。

## 失败处理

- 外部工具失败时，会连同它的退出码和一段经过净化的 stderr 摘录
  一起报告。在写入任何东西之前，机密信息都已被脱敏。
- 安装会在**第一个**失败的阶段停止。它绝不会继续进入不安全的步骤，
  绝不会宣称它没有达到的成功，也绝不会自动重跑某个破坏性步骤。
  `recovery::resume_plan` 会列出哪些阶段可以安全重试。
- 自方案创建以来身份已发生变化的磁盘会中止整个运行。
- 缺少运行时会明确报告：安装器会说"Python tool not available"，
  而不是假装该功能不存在。

## 安全模型

1. **不做 shell 字符串插值。** 用户输入永远不进入 shell。每个
   外部工具都用独立的 argv 项调用，`sfdisk` 脚本通过 stdin 管道传入，
   而不是作为参数传入。
2. **安全闸门。** 一个破坏性操作需要 `--allow-destructive` **并且**
   手动输入与其风险等级相符的确认短语（`ERASE`、`ERASE AND WIPE`）。
3. **设备身份。** 方案中存储的是 WWN 和序列号，而不是 `/dev/sda`。
   在每次写入之前会立即重新扫描磁盘，并与方案中的内容作比较。
4. **静态无机密。** 密码只存在于内存中。它们通过 stdin 管道传给
   `chpasswd`，绝不会出现在 argv、日志或配置文件中。
5. **默认演练（不写入）。** 任何写入都需要 `--execute`。

## 测试策略

| 层次 | 方式 |
| --- | --- |
| 单元 | `cargo test`、Python `unittest`、Java `CatalogTest` |
| 验收 | 通过 `--mock` 针对构建出的二进制文件运行合成设备探测 |
| 全系统 | 一台 VirtualBox 虚拟机，配一块空磁盘，从 Arch ISO 启动，以脚本模式运行安装器 |
| 打包 | CI 构建发布压缩包并在 Arch 容器中对其运行 `makepkg`，因此损坏的 `PKGBUILD` 会让构建失败 |

细节见 [testing](testing.md)。

## 下一篇

- [IPC 协议](ipc-protocol.md)
- [测试](testing.md)
- [尚未构建的内容](roadmap.md)