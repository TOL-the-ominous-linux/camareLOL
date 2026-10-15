<!-- 译自 docs/en/index.md。请修改源文件后重新翻译；请勿直接修改本文件。 -->

# camareLOL

> 一个安全优先的 Arch Linux TUI 安装器，提供三种语言。

camareLOL 从终端安装 Arch Linux。它会检测你的硬件、提出一个布局方案、
准确展示它即将做什么，并且只有在你输入确认短语之后才会写入磁盘。

**它不是一层 shell 包装。** 安装器有自己的流水线 —— 发现 →
方案 → 复核 → 确认 → 执行 → 验证 —— 并且在每一个破坏性操作之前
都设有安全闸门。

| | |
|---|---|
| 网站 | **<https://camarelol.netlify.app>** |
| 发布版本 | <https://github.com/tamkogay9876-code/camareLOL/releases> |
| 讨论 | <https://github.com/tamkogay9876-code/camareLOL/discussions> |
| 问题 | <https://github.com/tamkogay9876-code/camareLOL/issues> |

## 语言

| 语言 | 文档 |
|---|---|
| English | <https://camarelol.netlify.app/en/docs/> |
| Tiếng Việt | <https://camarelol.netlify.app/vi/docs/> |
| 中文 | <https://camarelol.netlify.app/zh/docs/> |

安装器本身支持越南语和英语（`--lang vi|en`）。

## 安装

在 Arch 实时 ISO 中运行它——安装器就该在那里运行：

```console
$ curl -sSLO https://github.com/tamkogay9876-code/camareLOL/releases/latest/download/camarelol
$ chmod +x camarelol
$ ./camarelol --self-test      # 非破坏性检查；在真实机器上运行也安全
$ sudo ./camarelol             # 打开 TUI
```

`--self-test` 绝不会写入磁盘。请先运行它。

保持最新——更新器默认只做报告，只有在被要求时才安装：

```console
$ camarelol --check-update     # 只报告，不下载任何内容；有新版本时退出码为 10
$ camarelol --update           # 下载、验证签名与 SHA-256，然后安装
$ camarelol --rollback-update  # 恢复上一个二进制文件
```

它绝不会自行替换自身，绝不在安装进行中替换，也会拒绝任何校验和或签名
不匹配的下载。

## 文档

| 主题 | English | Tiếng Việt | 中文 |
|---|---|---|---|
| 更新是如何签名的 | [updates-signing](updates-signing.md) | [VI](https://camarelol.netlify.app/vi/docs/updates-signing.html) | [ZH](updates-signing.md) |
| 为什么拒绝过旧的索引 | [updates-rollback](updates-rollback.md) | [VI](https://camarelol.netlify.app/vi/docs/updates-rollback.html) | [ZH](updates-rollback.md) |
| 构建你自己的软件包 | [updates-packaging](updates-packaging.md) | [VI](https://camarelol.netlify.app/vi/docs/updates-packaging.html) | [ZH](updates-packaging.md) |
| 什么时候更新会被拒绝 | [updates-troubleshooting](updates-troubleshooting.md) | [VI](https://camarelol.netlify.app/vi/docs/updates-troubleshooting.html) | [ZH](updates-troubleshooting.md) |
| 三个组件 | [architecture](architecture.md) | [VI](https://camarelol.netlify.app/vi/docs/architecture.html) | [ZH](architecture.md) |
| IPC 协议 | [ipc-protocol](ipc-protocol.md) | [VI](https://camarelol.netlify.app/vi/docs/ipc-protocol.html) | [ZH](ipc-protocol.md) |
| 测试了什么 | [testing](testing.md) | [VI](https://camarelol.netlify.app/vi/docs/testing.html) | [ZH](testing.md) |
| 自己动手运行 | [vm-testing](vm-testing.md) | [VI](https://camarelol.netlify.app/vi/docs/vm-testing.html) | [ZH](vm-testing.md) |
| 尚未构建的内容 | [roadmap](roadmap.md) | [VI](https://camarelol.netlify.app/vi/docs/roadmap.html) | [ZH](roadmap.md) |

## 目前它能做什么

- 检测磁盘、分区、文件系统、EFI 分区、Windows、恢复分区和
  Microsoft 保留分区，以及实时系统挂载点。
- 规划 GPT 和 MBR 布局：ESP、BIOS boot、`/`、`/home`、swap，使用
  ext4、btrfs 或 xfs，并针对固件模式进行校验。
- 检测到 Windows 就拒绝触碰它。双系统布局只会被规划，绝不会盲目执行。
- 安装一个最小化的 Arch 系统：pacstrap、fstab、引导加载程序、locale、
  键盘布局、时区、主机名、用户、sudo 策略、服务、initramfs。
- 安全闸门：默认锁定。`--allow-destructive` 加上手动输入的确认短语
  （`ERASE`、`ERASE AND WIPE`）将其解除锁定；在写入任何内容之前还
  必须提供 `--execute`。
- 配置档：可保存和加载，带模式版本号，**绝不包含密码**。
- 对每个阶段做日志记录，并对机密信息做脱敏。

## 它有意不做的事

- 因为你在菜单里选中了某块磁盘就格式化它。
- 未经明确选择加入并以普通用户构建，就安装 AUR 软件包。
- 调整 NTFS 大小——验收标准已写入文档，该功能被推迟。
- 为双系统复用现有 ESP。检测与保留规则已经实现；布局界面将在第 3 阶段
  到来。

## 安装器流程

```
Welcome → Language → Keyboard → Timezone → Storage → Disk mode →
Layout → Filesystem → Accounts → Bootloader → Desktop → Services →
Packages → Review (type the confirmation phrase) → Progress → Result
```

仅用键盘操作。`↑↓` 移动，`Enter` 选择，`Space` 切换，`Tab` 跳到下一个
字段，`Esc` 返回，`F1` 帮助，`F5` 重新扫描，`F10` 复核，`Ctrl-C` 退出，
`u` 检查更新。

## 警告

安装操作系统和划分磁盘分区**可能销毁数据**。
camareLOL 在每个不可逆步骤之前都会询问，写入之前会重新识别磁盘，
并且绝不宣称它没有验证过的成功。

即便如此：在你安装任何东西之前，先备份你在乎的任何内容；
并且从实时 ISO 运行它，而不是在你正想修复的那台机器上运行它。