<!-- 译自 docs/en/index.md。请修改源文件后重新翻译；请勿直接修改本文件。 -->

# camareLOL

> 一个安全优先的 Arch Linux TUI 安装器，提供三种语言。

**当前版本：** `1.0.0` —— 首个 beta（GitHub pre-release）。npm 的
`latest` 标签仍指向 `0.1.22`；beta 位于 `@wenker-ai/camarelo@beta`。
请阅读 [1.0.0 发布说明](../releases/1.0.0.md)。

camareLOL 从终端安装 Arch Linux。它会检测你的硬件、提出一个布局方案、
准确展示它即将做什么，并且只有在你输入确认短语之后才会写入磁盘。

**它不是一层 shell 包装。** 安装器有自己的流水线 —— 发现 →
方案 → 复核 → 确认 → 执行 → 验证 —— 并且在每一个破坏性操作之前
都设有安全闸门。

| | |
|---|---|
| 发布版本 | <https://github.com/tamkogay9876-code/camareLOL/releases> |
| 讨论 | <https://github.com/tamkogay9876-code/camareLOL/discussions> |
| 问题 | <https://github.com/tamkogay9876-code/camareLOL/issues> |

## 语言

安装器本身支持越南语、英语和简体中文（`--lang vi|en|zh`）。

## 安装

在 Arch 实时 ISO 中运行它——安装器就该在那里运行。默认的一行命令解析最新的
**稳定版**（0.1.22）；1.0.0 beta 需要显式选择：

```console
$ curl -fsSL https://raw.githubusercontent.com/tamkogay9876-code/camareLOL/main/scripts/camarelol.sh \
    | CAMARELOL_VERSION=v1.0.0 bash
$ camarelol --self-test      # 非破坏性检查；在真实机器上运行也安全
$ sudo camarelol             # 打开 TUI
```

用 `CAMARELOL_CHANNEL=beta` 代替 `CAMARELOL_VERSION` 可跟随最新的预发布版。
两者都省略则使用稳定通道。

`--self-test` 绝不会写入磁盘。请先运行它。

如果你想自己下载二进制文件，请保留 `-f` 和 `-L`：

```console
$ curl -fLO https://github.com/tamkogay9876-code/camareLOL/releases/download/v1.0.0/camarelol
$ curl -fLO https://github.com/tamkogay9876-code/camareLOL/releases/download/v1.0.0/SHA256SUMS
$ sha256sum -c SHA256SUMS
```

少了 `-f`，代理返回的 404 页面会被保存成名为 `camarelol` 的文件，而 curl
仍然返回 0；`chmod +x` 也会在一个文本文件上成功，于是第一次运行会报
`line 1: Not: command not found`。

camareLOL 不会自我更新。没有更新器，没有索引地址，代码里也没有任何
替换自身二进制文件的路径。

这是一个决定，不是一个没做完的功能。本程序会擦除磁盘；一个职责就是
替换自身的组件，是从被攻破的服务器到数据被擦除之间最短的一条路。
没有 `pacman` 软件包，也没有 AUR 软件包。重新运行上面那一行命令即可，
它会解析最新的**稳定版**并校验：

```console
$ curl -fsSL https://raw.githubusercontent.com/tamkogay9876-code/camareLOL/main/scripts/camarelol.sh | bash
```

或者从 <https://github.com/tamkogay9876-code/camareLOL/releases>
下载想要的版本，手动替换二进制文件。

## 文档

| 主题 | English | Tiếng Việt | 中文 |
|---|---|---|---|
| 三个组件 | [EN](../en/architecture.md) | [VI](../vi/architecture.md) | [ZH](architecture.md) |
| IPC 协议 | [EN](../en/ipc-protocol.md) | [VI](../vi/ipc-protocol.md) | [ZH](ipc-protocol.md) |
| 测试了什么 | [EN](../en/testing.md) | [VI](../vi/testing.md) | [ZH](testing.md) |
| 自己动手运行 | [EN](../en/vm-testing.md) | [VI](../vi/vm-testing.md) | [ZH](vm-testing.md) |
| 更新 | [EN](../en/update.md) | [VI](../vi/update.md) | [ZH](update.md) |
| 尚未构建的内容 | [EN](../en/roadmap.md) | [VI](../vi/roadmap.md) | [ZH](roadmap.md) |

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

- 在 Arch Linux 与 systemd 之外的任何系统上运行。发行版和 init 系统来自
  `/etc/os-release` 与 `/proc/1/comm`，其他任何组合都会在规划阶段被拒绝，
  此时还没有写入任何一个字节。**Parabola 和 Hyperbola 是 GNU/Linux-libre
  发行版，不是 Arch** —— 它们使用 `pacman`，但会删除或替换非自由软件包，
  因此一份在 Arch 上完全正确的软件包清单在那里可能根本不存在或换了名字。
  Artix、Nyarch 以及 EndeavourOS 这类 `ID_LIKE=arch` 的衍生版同样被拒绝：
  它们是未经验证的主机，而不是 Arch。
- 因为你在菜单里选中了某块磁盘就格式化它。
- 未经明确选择加入并以普通用户构建，就安装 AUR 软件包。
- 调整 NTFS 大小——验收标准已写入文档，该功能被推迟。
- 从生成的菜单项启动真实 Windows，或在 Windows 内部缩小 Windows。保留全部分区
  的规划器、只追加的写入路径和 Windows 菜单项已在虚拟机中验证；真实 Windows
  这一部分仍被推迟。
- 根据解析出的型号推荐 GPU 驱动。`GpuInfo.model` 已读取并显示；按型号选择驱动
  仍是部分实现。

## 安装器流程

```
Welcome → Language → Keyboard → Timezone → Storage → Disk mode →
Layout → Filesystem → Accounts → Bootloader → Desktop → Services →
Packages → Review (type the confirmation phrase) → Progress → Result
```

仅用键盘操作。`↑↓` 移动，`Enter` 选择，`Space` 切换，`Tab` 跳到下一个
字段，`Esc` 返回，`F1` 帮助，`F5` 重新扫描，`F10` 复核，`Ctrl-C` 退出。

## 警告

安装操作系统和划分磁盘分区**可能销毁数据**。
camareLOL 在每个不可逆步骤之前都会询问，写入之前会重新识别磁盘，
并且绝不宣称它没有验证过的成功。

即便如此：在你安装任何东西之前，先备份你在乎的任何内容；
并且从实时 ISO 运行它，而不是在你正想修复的那台机器上运行它。
