<!-- 译自 docs/en/updates-packaging.md。请修改源文件后重新翻译；请勿直接修改本文件。 -->

# 更新：构建你自己的软件包

[← 文档索引](index.md)

如果你为 camareLOL 打包——一个 PKGBUILD、一个 ebuild、一个 AppImage、
你自己的镜像——那么有一个决定比其余所有决定都重要。

**你发布的二进制文件不应该向用户提供上游项目的二进制文件。**

## 为什么

上游更新器会提供一个你的包管理器从未构建、从未测试、也从未用你所在
发行版的流程签过名的二进制文件。那么一个从你的仓库安装的用户，
运行的将是别人对别人软件包的构建结果。对于一个会划分磁盘分区的
安装器来说，这不是一件小事。

另一种做法是把你的镜像变成发布渠道，这样签名检查、校验和、过期、
回滚和镜像回退全都保留，只改变地址。

## 在构建时设定渠道

两个变量，在**构建二进制文件时**读取：

```console
$ CAMARELOL_UPDATE_URL="https://mirror.example.org/camarelol/latest.json" cargo build --release
$ CAMARELOL_NO_UPDATE=1 cargo build --release      # or switch the updater off
```

| 变量 | 作用 |
|---|---|
| `CAMARELOL_UPDATE_URL` | 你的二进制文件所请求的索引 |
| `CAMARELOL_NO_UPDATE` | 完全不嵌入渠道；更新器拒绝运行 |

这些值是烘焙进二进制文件的，而不是在启动时从环境读取。
来自环境的取值可能被安装这个软件包的任何人设定，
而那正是错误的信任来源方向。

## 拼写

变量名是 `CAMARELOL_UPDATE_URL` 和 `CAMARELOL_NO_UPDATE`。在文档中
这两个名字都曾被写反过——`CARAMELOL_` 和 `CAMAMELOL_`——
而拼错的变量会被**默默地**忽略：构建成功，打包者以为自己的镜像
已经配置好了，于是他发布的每一个二进制文件都悄悄地
从上游更新。

现在构建在发现近似拼写时会指出它实际读取的变量名：

```
warning: CARAMELOL_UPDATE_URL is set but this build reads CAMARELOL_UPDATE_URL.
The misspelled one is ignored, and the binary keeps the upstream update channel.
```

## 验证是否生效

不要想当然。二进制文件知道自己是用什么构建的：

```console
$ camarelol --check-update
camareLOL 0.1.10 (tag v0.1.10, commit 8b765de)
index: https://mirror.example.org/camarelol/latest.json
binary: /usr/bin/camarelol
indexes to try: 2
answered by: https://mirror.example.org/camarelol/latest.json
trust: the index is signed by a pinned key
```

三个方向都要检查，而不是只查一个：

| 构建方式 | `--check-update` 应显示 |
|---|---|
| 不设任何变量 | 项目镜像 |
| 设置了 `CAMARELOL_UPDATE_URL` | 你的镜像 |
| `CAMARELOL_NO_UPDATE=1` | 完全没有地址 |

第三种情况值得双向测试——先设置它，再取消设置并重新构建。
去掉变量必须回到默认值，而不是把上一次的取值留在里面。

## 重新构建

当这些变量变化时，Cargo 会重新运行构建脚本，因此在重新构建之前
设置变量确实会在那一次重新构建中生效。

如果你不确定，请在干净的工作树中构建，而不是手动删除
`target/` 的部分内容：

```console
$ cargo clean && cargo build --release
```

手动删除 `target/release/build/<pkg>` 会破坏指纹链接，之后构建
可能在其实并未完成时把自己报告为最新。

## 你的镜像必须提供什么

```console
$ ./scripts/sign-release.sh 0.1.10
$ ls /var/www/camarelol/
latest.json  latest.json.sig
```

两个文件都要有，否则那就不是一个镜像。`latest.json.sig` 是在
发布版本创建之后才上传的——在发布版本存在之前索引无法被签名，
因为签名要指向它——所以任何监听新发布版本的东西都应该等待签名，
而不是在第一个 404 上就失败。

## 下一篇

- [更新：签名](updates-signing.md)
- [为什么拒绝过旧的索引](updates-rollback.md)
- [什么时候更新会被拒绝](updates-troubleshooting.md)