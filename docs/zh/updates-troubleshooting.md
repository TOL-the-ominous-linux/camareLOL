<!-- 译自 docs/en/updates-troubleshooting.md。请修改源文件后重新翻译；请勿直接修改本文件。 -->

# 更新：什么时候更新会被拒绝

[← 文档索引](index.md)

camareLOL 可能做出的每一种拒绝、它由什么引起，以及你该怎么办。

## 阅读错误信息

```console
error: [update_download_failed] could not download https://…
  (curl: (22) The requested URL returned error: 404)
  → try the next mirror, or pass --update-url to name one yourself
```

三个部分：一个稳定的、机器可读的代码；一句说明发生了什么的句子；
以及一个带箭头的下一步操作。下面的代码取自
`core/src/error.rs`；它们在各发布版本之间保持稳定，因此脚本可以
对它们做匹配。

## 索引与网络

### `update_download_failed`

所有镜像都无法访问，或者某个代理返回了一个错误页面。

```console
$ curl -sI https://tamkogay9876-code.github.io/camareLOL-mirror/latest.json
$ camarelol --check-update --update-url https://example.org/latest.json
```

镜像返回**内容为 HTML 页面的 200** 并不代表成功。有些静态托管
会对未知路径回退到 `index.html`，所以那里的 200 只是披着成功状态码
外衣的"未找到"。要检查内容类型，而不只是状态码。

### `update_manifest_invalid`

索引的形态不是客户端能据以行动的：`schema` 不对、缺少字段、
下载地址不是 HTTPS，或者 `sha256` 不是 64 个十六进制字符。

这是拒绝而不是尽力猜测。一个 camareLOL 无法完全理解的索引，
就是它不应该据以安装的索引。

### `update_url_refused`

```console
error: [update_url_refused] this build was packaged without an update channel
  → updates come from the package manager that built it, or pass --update-url
```

该二进制文件是用 `CAMARELOL_NO_UPDATE=1` 构建的。这是刻意为之，
正确的设计——见[构建你自己的软件包](updates-packaging.md)。

出于同样的原因，明文 HTTP 的 URL 也会被拒绝，除非它是回环地址：
一切不是 `127.0.0.1` 的目标都必须是 `https://`。

## 信任

### `update_signature_missing`

索引送达时没有签名。镜像在提供 `latest.json`，却没有提供
`latest.json.sig`。

两个都提供。如果你控制着镜像并且确实无法签名，请先读
[更新：签名](updates-signing.md)——这个覆盖开关意味着接受来自该
地址的任意内容体。

### `update_signature_invalid`

签名无法对照固定的密钥通过验证。

通常是上传被截断、文件搞错了，或者签名用了另一把密钥：

```console
$ gpg --verify latest.json.sig latest.json
```

签名所用的密钥必须是 `keys/camarelol.pub` 中的那把。在重新上传
任何东西之前先做验证。

### `update_expired`

索引已超过它的 `expires`。它签名正确，只是太旧了——
见[为什么拒绝过旧的索引](updates-rollback.md)。

要么是镜像没有刷新它，要么是这台机器已经离线超过一周。
如果你知道原因，`--allow-expired-index` 会继续执行。

## 内容体

### `update_checksum_mismatch`

下载到的字节与索引中的 `sha256` 不匹配。

下载被截断、代理改写了响应体，或者发布附件在索引签好名之后
被替换了。

**不要**盲目重试。把手上的东西和发布页面上列出的内容做个对比。

### `update_candidate_failed`

新的二进制文件已下载并通过校验，但无法运行 `--version`。

缺少某个库，或者架构不对。旧的二进制文件会被保留在原处——
正是这项检查保证了替换只在新副本证明自己能够启动之后才发生。

## 开关本身

### `update_not_privileged`

当前用户没有写入该二进制文件的权限。

```console
$ sudo camarelol --update
```

### 有安装正在进行

`an installation is running`

在安装进行到一半时替换二进制文件，是让机器停留在半安装状态
最快的办法，因此更新器会拒绝。即使索引签名正确也依然如此——
一个有效的签名绝不能成为绕过安全规则的门路。

### 更新无法替换正在运行的二进制文件

下载落到了正在运行的可执行文件之外的其他位置。请传入
`--update-url` 并显式复制，或者通过包管理器安装。

### `update_rollback_failed`

`--rollback-update` 没有找到可恢复的 `.prev` 二进制文件。
它只有在某次更新确实成功安装过的前提下才会存在。

## 不算错误的情况

| 消息 | 含义 |
|---|---|
| `up to date` | 索引与你的版本一致，或早于你的版本 |
| `version 0.1.11 is available` | 只是报告，没有下载任何东西 |
| 退出码 `10` | 存在更新的版本，但没有做任何事 |

退出码 10 的存在是为了让脚本可以提醒而不算失败：

```console
camarelol --check-update || [ $? = 10 ] || exit 1
```

## 当一切都不管用时

逐步手动检查：

```console
$ curl -sS https://tamkogay9876-code.github.io/camareLOL-mirror/latest.json | head -3
$ curl -sSI https://tamkogay9876-code.github.io/camareLOL-mirror/latest.json.sig | head -1
$ camarelol --check-update --update-url https://… --update-key ./keys/camarelol.pub
```

如果某个镜像有问题，就说出来——一个失效的镜像应该由运行它的人
来发现，而不是由下一个尝试更新的人。

- <https://tamkogay9876-code.github.io/camareLOL-mirror/status.html> — 实时镜像状态
- <https://github.com/tamkogay9876-code/camareLOL/issues> — 报告问题

## 下一篇

- [更新：签名](updates-signing.md)
- [为什么拒绝过旧的索引](updates-rollback.md)
- [构建你自己的软件包](updates-packaging.md)