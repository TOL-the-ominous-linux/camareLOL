# 更新 camareLOL

[← 文档首页](index.md)

camareLOL 不会自我更新。

---

## 没有更新器

没有 `--update` 选项。没有索引地址。代码里也没有任何替换自身的路径。

这是一个决定，不是一个没做完的功能。camareLOL 会擦除磁盘。一个职责就是替换
自身的组件，是从被攻破的服务器到数据被擦除之间最短的一条路——签名校验能挡住
被篡改的*下载文件*，但挡不住别的。而信任锚点本身就是一个按定义可被攻击者
访问的服务器。

它底下的机制同样是负担。在移除之前，每次发布都要附带一个签名密钥、一个更新
索引、一份镜像列表和两台镜像主机。这条链条一天之内断了四次。故障形态每次都
一样：一台主机停止响应，另一台继续提供冻结的索引副本，于是外面每一个
camareLOL 都报告"你已经是最新版本"。

**谎报新鲜度的镜像，比没有镜像更糟。**

---

## 改用这些方式更新

### 从软件仓库或 AUR

常规做法，和其他 Arch 软件包一样：

```console
$ sudo pacman -S camarelol
```

### 手动下载

从 [Releases](https://github.com/tamkogay9876-code/camareLOL/releases)
下载想要的版本，替换二进制文件：

```console
$ sudo pacman -S wget                 # 如果还没有的话
$ sudo wget -O /usr/local/bin/camarelol \
    https://github.com/tamkogay9876-code/camareLOL/releases/latest/download/camarelol
$ sudo chmod +x /usr/local/bin/camarelol
$ camarelol --version                  # 看看现在装的是哪一版
```

没有签名需要校验，因为也没有东西可校验。`--version` 告诉你正在运行哪个发布
版本，这就是信任问题的全部。

---

## 如果你在 0.1.10 或更早的版本上

那些版本内置了更新器。在欢迎界面按 `u`，它会去请求一个已经失效的地址，或者
拿一份冻结的索引悄悄报告成功。

不会有风险：它装不了任何东西，因为它要问的那台服务器已经没了。下载当前版本，
按上面的方式替换二进制文件即可。之后的版本不再有 `u` 键。

---

## 查看你装的是什么

```console
$ camarelol --version
camareLOL 0.1.11 (tag v0.1.11, commit abc1234)
```

三部分，彼此应该一致：

| 部分 | 含义 |
|---|---|
| `0.1.11` | 这个二进制文件是从哪个发布版本构建的 |
| `tag v0.1.11` | git 标签，也就是发布版本的名称 |
| `commit abc1234` | 具体是哪个提交 |

如果它显示 `[built from a dirty working tree]`，说明这个二进制文件是从未提交
的改动编译来的。不管它自称什么版本，它都不是发布版本。

---

## 其他一切不变

移除更新器只删掉了替换二进制文件的那部分代码。安装器、安全闸门、IPC 协议和
文档都没有受影响。项目往哪走，见[尚未构建的内容](roadmap.md)。
