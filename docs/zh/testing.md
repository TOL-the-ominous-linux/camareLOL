<!-- 译自 docs/en/testing.md。请修改源文件后重新翻译；请勿直接修改本文件。 -->

# 测试

[← 文档索引](index.md)

四个层次，各自回答一个不同的问题。最重要的是最后一个：它在
一台真的必须能够启动的机器上是否管用。

## 1. 单元测试

```console
$ cargo test --workspace                        # Rust core + TUI
$ cd python && python3 -m unittest discover -s tests -v
$ cd java && ./build.sh                         # compiles, runs CatalogTest, builds the jar
$ python site/test_markdown.py                  # site renderer
$ python site/test_render_site.py               # site build, offline
$ python scripts/check_pkgbuild.py              # PKGBUILD matches the newest tag
```

快速，不碰磁盘，不联网。

## 2. 验收场景

不需要真实磁盘。规范中的验收标准会针对合成的设备探测运行：

```console
$ cd python && PYTHONPATH=. python3 -m camarelol_tools.testutil
PASS  empty-disk: self-test: OK
PASS  windows-disk: self-test: OK
PASS  partition-no-fs: self-test: OK
```

| 场景 | 它必须证明什么 |
| --- | --- |
| 空磁盘 | 能够规划并应用一个完整布局 |
| Windows 磁盘 | ESP、恢复分区和 Microsoft 保留分区被保留，而不是被重新格式化 |
| 没有文件系统的分区 | 被判定为未格式化，而不是被假定为可用 |
| 设备身份变化 | 中止运行，而不是写入另一块磁盘 |
| 无效的引导配置 | 在方案阶段就被拒绝，而不是到一半才发现 |

这些是针对构建出的二进制文件以 `--mock` 运行的，因此它们测试的是
真实的代码路径，而不是它的模拟物。

## 3. VirtualBox 中的全系统测试

真正的证明。在一台带**空**虚拟磁盘的虚拟机中启动 Arch ISO，
并以脚本模式运行安装器。

```console
$ ./scripts/vm-test.sh create      # VM + 20 GiB empty disk + Arch ISO + data disk
$ ./scripts/vm-test.sh boot        # headless boot, drives the VM with keystrokes
$ ./scripts/vm-test.sh check       # screenshot and verify
$ ./scripts/vm-test.sh destroy
```

在虚拟机内部：

```console
# mount /dev/sdb /mnt
# /mnt/camarelol --scan > /tmp/survey.json
# python3 /mnt/scripts/vm-helper.py /tmp/survey.json /tmp/test-profile.toml
# /mnt/camarelol --scripted /tmp/test-profile.toml --allow-destructive --execute
```

### 成功是什么样

```
preflight      completed  firmware=BIOS bootloader=GRUB fs=ext4 partitions=2
partition      completed  wrote GPT layout to /dev/sda
format         completed  filesystems created
mount          completed  target mounted
pacstrap       completed  installed 16 packages
fstab          completed  fstab generated
bootloader     completed  GRUB installed
system-config  completed  locale, keymap, timezone and hostname written
accounts       completed  0 user(s) configured
services       completed  services enabled
```

随后在虚拟机内部验证：

```console
# lsblk -f /dev/sda
sda1  vfat  LABEL=CAMARELOL                       (ESP)
sda2  ext4  LABEL=camarelol-root   mounted on /mnt
# cat /mnt/etc/fstab
/dev/sda2  /     ext4  defaults 0 2
# ls /mnt/boot
vmlinuz-linux  grub/  grub.cfg  i386-pc/
# ls /mnt/usr/bin/pacman
```

### 这证明了什么

- 二进制文件能在 Arch 实时环境中运行，没有任何缺失的库；
- 空磁盘被检测到，并规划出 GPT 布局；
- 分区表被写入，文件系统被创建；
- pacstrap 安装了一个基础系统；
- `genfstab` 的输出有效，GRUB 及其 BIOS 模块已安装；
- 目标系统拥有内核、引导加载程序和包管理器。

全绿的单元测试套件对这些一个也证明不了。

### 它抓到过的缺陷

以下所有问题都通过了每一项单元测试。

1. 生成的 `sfdisk` 脚本要求的扇区数超过了磁盘实际拥有的数量——
   没有把对齐间隙和 GPT 备份头算进去。
2. 1 MiB 的 BIOS boot 分区被悄悄丢掉了，因为生成器在遇到小于
   对齐步长的分区时就停止了。
3. 在 `mkfs` 之前，从未让内核重新读取分区表。
4. 挂载被归到 pacstrap 操作之下，却在另一个风险等级上被确认，
   于是运行总是停在 `mount`。
5. `grub` 不在 pacstrap 列表里，因此 `grub-install` 无法在目标
   系统内运行。
6. `zram-config` 不是一个真实的单元；一个缺失的单元会让整个
   服务阶段中止。

## 4. 打包

CI 构建发布压缩包并在 Arch 容器内对其运行 `makepkg`。一个无法构建的
`PKGBUILD` 会让构建失败，而不是等着某个人来报告。

之所以有这一层，是因为这份配方曾经写着 `pkgver=0.1.3` 而实际发布
版本是 `0.1.10`，并且 `source` 指向一个从未发布过的压缩包。

## 没有被测试的部分

| 未覆盖 | 原因 |
| --- | --- |
| 真实的 Windows 双系统 | 需要 Windows 许可证；模拟场景已覆盖检测逻辑 |
| NTFS 调整大小 | 明确推迟到以后的发布版本 |
| AUR 构建 | 需要网络和用户交互 |

## 自己动手运行

[自己动手运行](vm-testing.md) 中有那份实操指南，其中包括一个
什么都不写入的预览模式。

## 下一篇

- [架构](architecture.md)
- [自己动手运行](vm-testing.md)