<!-- 译自 docs/en/updates-signing.md。请修改源文件后重新翻译；请勿直接修改本文件。 -->

# 更新：签名

[← 文档索引](index.md)

校验和能证明一个文件完整送达。它不能证明这个文件来自你信任的人——
如果服务器同时给你内容本体和匹配的哈希，两者当然吻合，而这种吻合
说明不了任何事情。

因此更新索引是单独签名的，并且密钥被**编译进二进制文件**里。

## 索引

`latest.json` 是客户端所请求的内容：

```json
{
  "schema": 1,
  "version": "0.1.10",
  "url": "https://github.com/tamkogay9876-code/camareLOL/releases/download/v0.1.10/camarelol",
  "sha256": "fbfe9d8e84fd62b733270932a35113afeb14fa27b6749433d5ddfd0481cc6897"
}
```

| 字段 | 含义 |
|---|---|
| `schema` | 必须是 `1`；无法识别的客户端会拒绝而不是猜测 |
| `version` | 按数值比较而非按字符串比较，所以 `0.1.10` 比 `0.1.9` 新 |
| `url` | 必须是 `https://`；明文 HTTP 的下载地址会被拒绝 |
| `sha256` | 64 个十六进制字符，与下载到的字节逐一校验 |

还可能存在一个 `expires` 字段。详见
[为什么拒绝过旧的索引](updates-rollback.md)。

## 分离式签名

`latest.json.sig` 是针对 `latest.json` **原始字节**的 GPG 分离签名——
在解析这些字节之前就完成校验，因此一个狡猾的索引无法靠格式畸形
来藏匿任何东西。

```
latest.json       the index
latest.json.sig   its signature
```

每一个镜像都同时提供这两者。只提供其中之一的镜像是有故障的，
而只找到其中一个的客户端会明确告知，而不是继续执行。

## 被固定的密钥

`keys/camarelol.pub` 在构建时通过 `include_str!` 嵌入到二进制文件中，
因此它随每一份 camareLOL 副本一同分发。不存在配置步骤，
也不存在可以被篡改的密钥环。

检查标准是：**签名所用的**密钥**等于**被固定的密钥。
不是"GPG 愿意信任的某个密钥"——一个已经含有攻击者添加的密钥的
密钥环，会满足任何更宽松的检验。

| | |
|---|---|
| 密钥存放位置 | `keys/camarelol.pub`，在构建时嵌入 |
| 用于校验的密钥环 | 一个临时目录；你自己的 GPG 密钥环绝不会被触碰 |
| `EXPKEYSIG`、`REVKEYSIG`、`BADSIG` | 拒绝 |
| 签名对应的字节不同 | 拒绝 |

## 没有签名时会发生什么

未签名的索引会被拒绝，而且这个拒绝不是一条警告：

```console
$ camarelol --update
error: [update_signature_missing] refusing to install an update index that is not signed
  (no public key is configured, so the index cannot be authenticated)
  → publish latest.json.sig and configure a pinned key, or re-run with
    --allow-unsigned-update if you accept the risk
```

之所以存在 `--allow-unsigned-update`，是因为处在企业代理之后的打包者
可能根本没有办法提供签名。它必须手动输入。默认行为是什么都不做。

对于一个会擦除磁盘的工具来说，一个无法验证身份的更新不值得安装。

## 轮换密钥

当密钥泄露时，仅仅更换它本身并不够——现场已经分发的二进制文件里
仍带着旧密钥。步骤是：

1. 生成替换密钥，并把它加入 `keys/camarelol.pub`。
2. 用新密钥构建并签署一个发布版本。
3. 发布。旧密钥从此不再签署任何东西，因此镜像无法再以它的名义
   提供任何新内容。

这件事已经发生过一次。泄露的密钥被删除，新密钥签署了 v0.1.10
及以后的版本，而这次吊销正是旧镜像无法把由已泄露密钥签署的东西
交给当前客户端的原因。

## 面向维护者

```console
$ ./scripts/make-signing-key.sh          # once, on your own machine
$ ./scripts/make-release.sh 0.1.10       # builds from the tag, refuses a dirty tree
$ ./scripts/sign-release.sh 0.1.10       # signs, then verifies with the pinned key
```

`sign-release.sh` 在报告成功之前，会用自己的输出对照
`keys/camarelol.pub` 进行验证，因此用错误密钥生成的签名不会被
误上传。

## 下一篇

- [为什么拒绝过旧的索引](updates-rollback.md)
- [构建你自己的软件包](updates-packaging.md)
- [什么时候更新会被拒绝](updates-troubleshooting.md)