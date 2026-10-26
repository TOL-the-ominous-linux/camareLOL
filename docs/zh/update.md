# 更新 camareLOL

[← 文档首页](index.md)

camareLOL 不会自我更新。

---

## 没有更新器

没有 `--update` 选项。没有索引地址。代码里也没有任何替换自身的路径。

这是一个决定，不是一个没做完的功能。camareLOL 会擦除磁盘。一个职责就是替换
自身的组件，是从被攻破的服务器到数据被擦除之间最短的一条路——SHA-256
校验能挡住损坏或被替换的*下载文件*，但挡不住别的。而信任锚点本身就是一个
按定义可被攻击者访问的服务器。

它底下的机制同样是负担。在移除之前，每次发布都要附带一个签名密钥、一个更新
索引、一份镜像列表和两台镜像主机。这条链条一天之内断了四次。故障形态每次都
一样：一台主机停止响应，另一台继续提供冻结的索引副本，于是外面每一个
camareLOL 都报告"你已经是最新版本"。

**谎报新鲜度的镜像，比没有镜像更糟。**

---

## 改用这些方式更新

camareLOL 没有 `pacman` 软件包，也没有 AUR 软件包。重新运行上面那一行命令即可，
它会解析最新的**稳定版**并校验：

```console
$ curl -fsSL https://raw.githubusercontent.com/tamkogay9876-code/camareLOL/main/scripts/camarelol.sh | bash
```

1.0.1 是当前 beta；1.0.0 是首个 beta。它同样以 GitHub pre-release 发布。
固定 tag 或选择 beta 通道：

```console
$ curl -fsSL https://raw.githubusercontent.com/tamkogay9876-code/camareLOL/main/scripts/camarelol.sh \
    | CAMARELOL_VERSION=v1.0.1 bash
```

```console
$ curl -fsSL https://raw.githubusercontent.com/tamkogay9876-code/camareLOL/main/scripts/camarelol.sh \
    | CAMARELOL_CHANNEL=beta bash
```

### 手动下载

从 [Releases](https://github.com/tamkogay9876-code/camareLOL/releases)
下载想要的版本，替换二进制文件：

```console
$ sudo pacman -S wget                 # 如果还没有的话
$ sudo wget -O /usr/local/bin/camarelol \
    https://github.com/tamkogay9876-code/camareLOL/releases/download/v1.0.1/camarelol
$ sudo wget -O /tmp/SHA256SUMS \
    https://github.com/tamkogay9876-code/camareLOL/releases/download/v1.0.1/SHA256SUMS
$ cd /usr/local/bin && grep ' camarelol$' /tmp/SHA256SUMS | sha256sum -c -
$ sudo chmod +x /usr/local/bin/camarelol
$ camarelol --version                  # 看看现在装的是哪一版
```

校验和可以发现下载过程中损坏或被替换的文件。它不是签名：`SHA256SUMS`
与二进制文件放在同一位置，被攻破的服务器可以同时替换两者。
`--version` 告诉你正在运行哪个发布版本。

### 如果机器只有 Wi-Fi

camareLOL 需要网络来下载 Arch 软件包。如果你用 `pacman` 或手动安装，需要先用
`iwctl` 连接 Wi-Fi —— Arch live ISO 自带这个工具：

```console
$ iwctl
[iwd]# device list
[iwd]# station wlan0 scan
[iwd]# station wlan0 get-networks
[iwd]# station wlan0 connect "网络名"
[iwd]# station wlan0 enable
```

加密网络会让 `connect` 询问密码，在那里输入即可。接口名不一定是 `wlan0` ——
用 `device list` 看真实名称。

如果没有 `iwctl`：

```console
$ sudo pacman -S iwd
```

有两个迹象说明 camareLOL 已经能看到足够的网络来装包：欢迎界面的状态栏写着
`internet connection OK`，并且服务列表里的 `zram` 没有被划掉。

有一种情况 camareLOL 现在会主动说明：机器**只有 Wi-Fi、尚未连接，而且既没有
`iwctl` 也没有 `nmtui`** —— 也就是没有任何工具可以用来连接。这时欢迎界面会显示
一行黄色警告，而不是只写一句 "no network connection detected" 然后沉默。如果
`iwctl` 还在（live ISO 一定有），你可以自己连接，所以不会出现警告。

### 通过 npm

需要先装好 `node` 和 `npm`：

```console
$ sudo pacman -S --needed nodejs npm curl
$ npm install -g @wenker-ai/camarelo
```

在 **archiso**（live ISO）上要多一步：pacman 的软件包数据库还是空的，直接用
`pacman -S` 会报 `database file for 'core' does not exist`。先连好网络
再把数据库与软件一起装——pacman 需要网络来下载数据库，Wi-Fi 则取决于你的安装方式：

- **通过 npm 安装**（`npm install -g @wenker-ai/camarelo`）：npm 包装器会先用
  `iwctl` 询问是否连接 Wi-Fi，加密网络同样支持。
- **通过 `pacman` 或手动安装**：请自行先连接 Wi-Fi，见
  [如果机器只有 Wi-Fi](#如果机器只有-wi-fi)。

```console
$ sudo pacman -Sy --needed nodejs npm curl
```

npm 外壳不属于二进制文件的一部分：它是在二进制文件启动**之前**运行的 Node
代码，而二进制文件仍然没有任何替换自身的路径。当包内的版本比 GitHub 最新发布
旧时，外壳会问一句——回答 `y` 就去下载该版本、对照 `SHA256SUMS` 校验、替换
包内的二进制文件再继续；回答 `n` 就直接进入安装器。

信任故事与手动下载相同：校验文件和二进制文件在同一个服务器上，它能拦住下载
途中损坏的文件，拦不住被攻破的服务器。

---

## 如果你在 0.1.10 或更早的版本上

那些版本内置了更新器。在欢迎界面按 `u`，它会去请求一个已经失效的地址，或者
拿一份冻结的索引悄悄报告成功。

不会有风险：它装不了任何东西，因为它要问的那台服务器已经没了。下载当前版本，
按上面的方式替换二进制文件即可。之后的版本不再有 `u` 键。

---

## 如果系统是用 0.1.12 安装且选了 zram

0.1.12 正确启用了 unit（`systemd-zram-setup@zram0.service`）并装上
`zram-generator` —— 但这仍然不是一个能用的 zram 交换空间。`zram-generator`
要读取 `/etc/systemd/zram-generator.conf` 才知道创建哪些设备；缺这个文件时
它只是运行一下就结束，系统启动后并没有任何 zram。0.1.13 修掉了这个缺口：
安装时会按照 `zram_size_mb` 的选择自动写入配置文件。

0.1.12 安装的系统已经有启用中的 unit 和软件包，只缺配置文件。最省事的办法
是手动补上这个文件，不必重装：

```console
$ sudo tee /etc/systemd/zram-generator.conf >/dev/null <<'EOF'
[zram0]
zram-size = min(ram / 2, 4096)
EOF
$ sudo systemctl start systemd-zram-setup@zram0
$ zramctl
```

`min(ram / 2, 4096)` 正是安装器的默认值：一半内存，最多 4 GiB。想要不同
大小就写普通 MiB 数字，例如 `zram-size = 2048`。执行之后 `zramctl` 应该能
看到 `zram0`；如果没看到，重启后再检查一次。

---

## 检查你自己的安装：`--postcheck`

别猜，直接问安装器：

```console
$ sudo camarelol --postcheck
  PASS zram_unit
  PASS zram_wants
  PASS zram_bin
  PASS zram_conf
  PASS sshd_unit
  PASS sshd_wants
  PASS openssh
  all checks passed
```

这七行是七件**必须真的在磁盘上**的东西，不是菜单里的七个选项。如果你的
系统挂在别处，指向它的根目录：

```console
$ sudo camarelol --postcheck /mnt
```

出现 `FAIL` 说明有东西被选中了却从未真正落地 —— 这个命令什么都不修，只说
实话。按上面的方法手动修，或者用更新的版本重装。

### 0.1.13 有一个 bug：zram 从未被启用

0.1.13 正确写了配置文件，但**没有创建启用 symlink**（在安装环境里对模板
unit 执行 `systemctl enable` 不会产生 symlink）。结果是 `zram_conf` 有了而
`zram_wants` 没有，所以即便安装报告 `completed`，zram 仍然不会运行。手动
修复：

```console
$ sudo ln -s ../../../../usr/lib/systemd/system/systemd-zram-setup@.service \
    /etc/systemd/system/multi-user.target.wants/systemd-zram-setup@zram0.service
$ sudo systemctl start systemd-zram-setup@zram0
$ zramctl
```

修复已在仓库里，VirtualBox 验收测试现在正好盯住这一点：最后一步会挂载装
好的磁盘，要求七个标记全部存在，所以以后的发布不会再漏掉它。

---

## 查看你装的是什么

```console
$ camarelol --version
camareLOL 1.0.1 (tag v1.0.1, commit abc1234)
```

三部分，彼此应该一致：

| 部分 | 含义 |
|---|---|
| `1.0.1` | 这个二进制文件是从哪个发布版本构建的 |
| `tag v1.0.1` | git 标签，也就是发布版本的名称 |
| `commit abc1234` | 具体是哪个提交 |

如果它显示 `[built from a dirty working tree]`，说明这个二进制文件是从未提交
的改动编译来的。不管它自称什么版本，它都不是发布版本。

---

## 其他一切不变

移除更新器只删掉了替换二进制文件的那部分代码。安装器、安全闸门、IPC 协议和
文档都没有受影响。项目往哪走，见[尚未构建的内容](roadmap.md)。
