<!-- 译自 docs/en/roadmap.md。请修改源文件后重新翻译；请勿直接修改本文件。 -->

# 路线图

[← 文档索引](index.md)

哪些已经构建，哪些还没有，以及——在没有的地方——为什么。

## 第 1 阶段 —— MVP

已完成。

- [x] 带键盘导航和帮助的 TUI 框架
- [x] 环境检查：固件、CPU、RAM、网络
- [x] 通过 `lsblk` / `findmnt` / `blkid` 进行存储检测，并附带风险评估
- [x] 空磁盘的自动布局：GPT 和 MBR、ESP、BIOS boot、根分区、
      `/home`、swap
- [x] 文件系统选择：ext4、btrfs、xfs
- [x] locale、键盘布局、时区
- [x] 主机名、root、用户账户、带锁定防护的 sudo 策略
- [x] 最小化的 Arch 安装：pacstrap、fstab、引导加载程序、服务
- [x] 日志记录和确认界面
- [x] 安全闸门：锁定、演练（不写入）、输入短语后执行
- [x] 配置的保存与加载，带模式版本号，不含密码
- [x] ~~带镜像回退的签名自更新器~~ — 已移除，而且这正是预期状态。更新来自
      `pacman` 或手动下载。一个职责就是替换会擦除磁盘的二进制文件的组件，是
      从被攻破的服务器到数据被擦除之间最短的一条路；而它背后的镜像链条屡次
      失效，却始终报告成功。
- [x] Python 自动化层、Java 目录、带版本号的 IPC

## 一个勾只有在有证据时才成立

上面的每个 `[x]` 都是对照真实的 VirtualBox 安装核验过的，不是看"有这个
结构体"就打勾。四个包名通过了全部单元测试，却只在虚拟机里失败：
`greetd-tuigreeter`（Hyprland 和 Sway 装不上）、`p7zip`（已从 Arch
仓库移除）、以及 `systemd-boot`（根本不是一个包；每次 systemd-boot 安装都
死在 `pacstrap`）。对一个安装器来说，"代码里提到过"这种勾一文不值。

这项工作中有两次失败是**测试**错了，不是安装器错了：断言显示管理器会在
`multi-user.target.wants/` 里创建符号链接（它用的是
`Alias=display-manager.service`，什么都不创建），以及在那个绝对路径的
别名上 `test -e`——它按主机的根目录解析，而不是挂载点。

0.1.18 又以同样的方式找到两个：ESP 标签 `CAMARELOL-EFI` 超过 FAT 的
11 字符上限，于是 `mkfs.fat` 在 format 阶段拒绝所有 UEFI 安装；mount 阶段
按计划顺序执行，根分区挂载把 ESP 盖住了，`grub-install` 于是拒绝
`/boot`。两个都通过了全部测试套件，两个都只在虚拟机里死掉，如今各有测试
把守——按文件系统种类检查标签上限，以及根分区先于 ESP 的挂载顺序。

`[~]` 表示部分完成：条目的一部分已交付并验证过，另一部分还不存在。


## 第 2 阶段 —— 桌面环境

- [x] 桌面环境配置档：KDE、GNOME、Xfce、Cinnamon、LXQt、Hyprland、Sway —
      已在 VirtualBox 安装中验证（`pacstrap` 26 个包，`hyprland` 落盘）
- [x] Wayland 或 X11，通过兼容性检查来选择 — Sway 拒绝 X11；明确的选择
      只会拉取那一套栈，不多不少
- [x] 显示管理器：SDDM、GDM、LightDM、greetd — 安装、启用，并且 greetd
      写入自己的 `/etc/greetd/config.toml`
- [~] 驱动检测和固件建议 — **部分**：CPU 微码提示和基础集合（`mesa`、
      `linux-firmware`）根据主机选择。GPU 型号检测尚未实现。
- [x] PipeWire、PulseAudio，或者都不用
- [x] 蓝牙、zram、交换空间选项
- [~] NetworkManager 配置 — **部分**：包已安装，unit 已启用。
      没有写入 `/etc/NetworkManager` 文件。

## 第 4 阶段 —— 体验

- [x] 跨运行保存并恢复配置 — 保存的配置档可以在欢迎界面按 `L` 载回向导；
      损坏的文件或错误的 `schema_version` 会被拒绝，而不是被默认值悄悄
      覆盖，密码永远不会恢复（`#[serde(skip)]`）。
- [x] 包搜索与选择 — `/` 过滤目录，方向键移动，空格切换。AUR 包只在
      AUR 启用时提供，名字来自目录，绝不凭空捏造。
- [x] AUR 支持，以普通用户构建、绝不用 root — 手动 `makepkg` 加
      `yay`/`paru`/`pikaur`/`trizen` 检测。三道独立的 root 防线；不会自动
      安装任何 helper。
- [x] 诊断报告导出 — `--save-diagnostics <FILE>` 写出脱敏报告（主机、
      survey、计划、阶段、警告、日志路径）。
- [x] 更多界面语言 — 越南语、英语、简体中文。
- [x] 更好的恢复指引 — 安装中途停止时，TUI 列出已完成的阶段、失败的那个
      和哪些可以安全重试，并明说它不能承诺机器一定能启动。

## 第 5 阶段 —— 发布

- [x] 发布流水线：`scripts/make-release.sh` 从发布标签构建，拒绝脏的
      工作树，检查装好的 binary 自报的标签就是构建它的那个标签（它抓到过
      一次 0.1.10 式的漂移），并把发布挡在 VM acceptance 闸门之后 — 只有
      `SKIP_VM=1` 能跳过闸门，而且跳过是一个被记录下来的、深思熟虑的
      决定。
- [x] 自动化 VM acceptance harness — 客户机把每一步的结果通过 NAT 发回
      主机，所以不需要 Guest Additions，一步卡死也吞不掉它之前各步的
      结果。`camarelol-acceptance` 对 0.1.18 构建跑了 7/7。
- [x] 带校验和制品的 GitHub release — binary、源码 tarball、Java 目录
      jar、一个生成出来的并把 tarball 的 sha256 钉死的 PKGBUILD，以及覆盖
      全部制品的 `SHA256SUMS`。已随 0.1.18 发布。
- [x] npm 包 `@wenker-ai/camarelo` — 每次打包都从 GitHub release 下载
      恰好是 `package.json` 那个版本的 binary，对照 `SHA256SUMS` 校验，
      要求 `--version` 一致，跑包的测试，然后才打包；publish 是单独的、
      显式的一步。0.1.18 是已发布的 latest。

## Stage 6 — 与已有系统共存的双系统

- [x] 保留全部的规划器 (`plan_dual_boot`) — 从实时 survey 构建：一块带分区
      的 GPT+UEFI 磁盘把每一个既有分区都贡献成 `format=false` 行，它的 ESP
      在 `/boot/efi` 被复用，新布局以显式起始扇区放进最大对齐自由间隙；
      当请求放不下时规划器拒绝出计划。survey 必须先携带分区的 START
      扇区，这一项才可能存在。
- [x] 只追加的写入路径 — `sgdisk --new=5:<start>:<end>` 只添加新分区；
      不 zap、不重写分区表，format/mount 各阶段按真实分区号寻址保留的
      分区。
- [x] GRUB 装在被复用的 ESP 上并配 os-prober — 仅当计划是双系统时，
      `os-prober` 加入 pacstrap 列表，`/etc/default/grub` 写入
      `GRUB_DISABLE_OS_PROBER=false`。
- [x] VM 证据 — 安装在假 Windows 旁边并成功启动 —
      `scripts/vm-dualboot.sh` 造一块 Windows 形状的盘（ESP、MSR、6 GiB
      C:、1 GiB 恢复区——每个都有标记文件），在其旁边安装，并检查每个
      标记文件完好、root 落在新的分区 5、GRUB 在原 ESP 上、`os-prober`
      已安装、装完恰好 5 个分区（三次完整绿灯 run，2026-10-22）。第三次
      run 反而单独证明了拒绝闸门：固件设为 BIOS 时，规划器以
      "dual-boot only plans a UEFI + GPT disk" 拒绝。随后的从盘启动证明了
      启动链：NVRAM → 被复用 ESP 上的 `EFI/camareLOL/` → Arch 内核 →
      initramfs → 登录提示。
- [x] 启动菜单里的 Windows 条目 — 假 Windows 现在带有 os-prober 的
      `efi/20microsoft` 探测所需的成对文件（`bootmgfw.efi` 连同一个
      `BCD`），生成的 `grub.cfg` 里出现了
      `menuentry "Windows Boot Manager (on /dev/sda1)"` 并 chainload 到它
      —— harness 以 `windows_entry ≥ 1` 检查（run 7，17 项全绿，
      2026-10-22）。两个修复让这件事是真实的而非侥幸：
      `GRUB_DISABLE_OS_PROBER=false` 现在由 bootloader 阶段自己写
      （原先落在 system-config——那是 `grub-mkconfig` 之后的阶段——标志
      到得太晚，连真 Windows 都会被漏掉），而 BCD 的要求是在 VM 里
      跟踪 `os-prober` 时发现的。尚未观察到的：菜单条目真正启动一台
      真 Windows——这仍然推迟在下面。提醒照旧：**不要把 camareLOL 指向
      装着你唯一一份 Windows 的磁盘。** 真机的起步步骤在
      `docs/DUAL-BOOT-TEST.md`。

## 有意推迟

以下这些被单独点名，因为它们的缺失看起来像疏忽，其实不是。

**NTFS 缩小。** 验收标准存在，安全检查也已写好，但功能没有构建。缩小一个
装着某人唯一一份 Windows 的分区是不可撤销的单向操作，它需要的测试量超出
本项目目前敢于声称的水平。

**真实 Windows 旁边的双系统。** 保留全部的一侧和 Windows 菜单条目都已完成并
在上面 Stage 6 勾选 —— VM 用假 Windows 证明了它们。仍被推迟的是只有真 Windows
才能回答的：看到菜单条目真正启动 Windows（假 bootmgr 会被 chainload 但什么也
不做），以及从 Windows 内部缩容的全部流程。在那最后一程关闭之前，任何
camareLOL 发行版都不该靠近装着用户唯一一份 Windows 的磁盘。

**高级存储清单的其余部分** —— 手动分区编辑器、LUKS 加密、Btrfs 子卷
布局 —— 一行代码都还没有。它们曾以五个空勾的形式留在阶段清单里；那
说的比看起来少，所以它们搬到了这里。

这里的每一件事，诚实的行为都是直说，而不是给出一个比看起来能做到的
要少的勾。

## 下一步

- [架构](architecture.md)
- [测试](testing.md)
