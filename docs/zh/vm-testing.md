<!-- 译自 docs/en/vm-testing.md。请修改源文件后重新翻译；请勿直接修改本文件。 -->

# 自己动手运行

[← 文档索引](index.md)

两种方式：**预览**，什么都不写入；以及**真实安装**，写入一块虚拟磁盘。
两者都需要 VirtualBox。

---

## 方式一 —— 预览，什么都不写入

先从这个开始。它在一台真实机器上检验界面和磁盘检测，而不触碰任何东西。

```console
$ cd /path/to/camareLOL
$ bash scripts/vm-release-test.sh create    # 虚拟机：Arch ISO + 两块空磁盘
$ bash scripts/vm-release-test.sh boot      # 无头模式；下载二进制文件并运行自检
$ bash scripts/vm-release-test.sh verify    # 截图保存结果
```

在虚拟机内部：

```console
$ camarelol --version
$ camarelol --scan          # JSON：型号、序列号、分区、风险等级
$ camarelol --self-test     # 非破坏性检查
$ camarelol                 # 打开 TUI；不输入任何内容就不会写入
```

`--self-test` 在真实机器上运行是安全的。`--scan` 也是。只有当你
输入确认短语时，TUI 才会写入。

截图保存在 `/tmp/opencode/vm-shots/`。

---

## 方式二 —— 真实安装到虚拟磁盘

> **此脚本会擦除虚拟磁盘上的数据。** 它只适用于虚拟机。
> 绝不要把它指向真实机器。

```console
$ cd /path/to/camareLOL
$ bash scripts/vm-test.sh create    # 虚拟机 + 20 GiB 空磁盘 + 存放二进制文件的 FAT 磁盘
$ bash scripts/vm-test.sh boot      # 启动，自动输入安装命令
$ bash scripts/vm-test.sh check     # 截图保存结果
$ bash scripts/vm-test.sh destroy   # 删除虚拟机
```

预期结果：所有阶段完成，存在有效的 `/mnt/etc/fstab`，并且有
`vmlinuz-linux` 和 `grub.cfg`。

---

## 手动创建虚拟机

如果你想看到真实的屏幕。

```console
$ VBoxManage createvm --name lol --ostype ArchLinux_64 --register
$ VBoxManage modifyvm lol --memory 4096 --cpus 4 --boot1 dvd --boot2 disk
$ VBoxManage storagectl lol --name SATA --add sata --controller IntelAhci
$ VBoxManage createmedium disk --filename ~/lol.vdi --size 20480 --format VDI
$ VBoxManage storageattach lol --storagectl SATA --port 0 --device 0 \
    --type hdd --medium ~/lol.vdi
$ VBoxManage storagectl lol --name IDE --add ide
$ VBoxManage storageattach lol --storagectl IDE --port 0 --device 0 \
    --type dvddrive --medium "$PWD/.test/archlinux-x86_64.iso"
$ VBoxManage startvm lol            # 不要使用无头模式——你需要看到画面
```

在实时 ISO 内部：

```console
# mount -o remount,rw /          # a live ISO usually mounts read-only
# pacman -Sy --needed curl
# curl -fLO https://github.com/tamkogay9876-code/camareLOL/releases/download/v1.0.1/camarelol
# curl -fLO https://github.com/tamkogay9876-code/camareLOL/releases/download/v1.0.1/SHA256SUMS
# sha256sum -c SHA256SUMS
# chmod +x camarelol
# ./camarelol --self-test
# sudo ./camarelol --allow-destructive    # add --execute to actually write
```

camareLOL 需要的一切都已经在实时 ISO 里：`sfdisk`、`sgdisk`、
`mkfs.*`、`pacstrap`、`genfstab`、`arch-chroot`。无需额外安装任何东西。

## 没有终端时

通过管道、从 `cron` 或在 CI 中运行它，会得到一条清晰的提示信息，
而不是 panic——TUI 没有地方可以绘制界面：

```
camareLOL needs a terminal to draw its interface, and there is none here.

For unattended runs there is no interface to draw, so use the scripted path:
    camarelol --scripted <config.toml>
and add --allow-destructive --execute to actually write to a disk.
```

---

## 需要检查什么

| 做什么 | 预期 |
| --- | --- |
| 打开 TUI，用 `↑↓` 移动 | 每块磁盘都显示型号、序列号和风险等级 |
| 选中一块磁盘，按 `F10` | 列出破坏性操作并要求输入 `ERASE` |
| 把短语输错 | 被拒绝，并且**什么都没有被写入** |
| 连按两次 `F1` | 帮助浮层打开又关闭 |
| 执行一次真实安装 | 所有阶段完成，`fstab` 有效，GRUB 已安装 |
| 拔掉网线，然后继续 | 报告网络错误，绝不谎报成功 |

第三行才是关键。其余的只是便利功能。

---

## 清理

```console
$ bash scripts/vm-test.sh destroy
$ bash scripts/vm-release-test.sh destroy
$ rm -rf ~/VirtualBox\ VMs/camarelol-*
```

虚拟磁盘和 ISO 位于项目的 `.test/` 目录下。删除虚拟机
不会影响宿主机。

## 下一篇

- [测试](testing.md)
- [尚未构建的内容](roadmap.md)