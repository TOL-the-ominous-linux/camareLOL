<!-- 译自 docs/en/ipc-protocol.md。请修改源文件后重新翻译；请勿直接修改本文件。 -->

# IPC 协议，第 1 版

[← 文档索引](index.md)

换行分隔的 JSON：每行一个请求，每行一个响应。

只有 Rust 核心可以执行会更改系统的操作。Python 和 Java 通过这个
协议与它通信——或者与 Java 目录自身的服务器通信。

## 线路格式

```jsonc
// request
{"id": 1, "protocol_version": 1, "method": "scan_devices", "params": {}, "timeout_ms": 120000}

// response
{"id": 1, "protocol_version": 1, "status": "ok", "result": {}, "duration_ms": 3}
```

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `id` | 数字或字符串 | 原样回传；必须非零 |
| `protocol_version` | 数字 | 必须是 `1` |
| `method` | 字符串 | 见下文 |
| `params` | 对象 | 方法参数 |
| `timeout_ms` | 数字，可选 | 按请求的超时；默认 120 000 |
| `cancel` | 数字，可选 | 要取消的先前某个请求的 id |

## 方法

| method | 破坏性 | 说明 |
| --- | --- | --- |
| `hello` | 否 | 协议发现 |
| `scan_devices` | 否 | 完整的存储探测，返回 JSON 数组 |
| `create_plan` | 否 | 为 `params.disk` 构建分区方案 |
| `validate_plan` | 否 | 重新校验一个已有方案 |
| `apply_plan` | **是** | 写入分区表；需要已解除锁定的安全闸门 |
| `format_partitions` | **是** | 创建文件系统；需要已解除锁定的安全闸门 |
| `diagnostics` | 否 | 诊断报告 |
| `get_config` / `set_config` | 否 | 配置的读取与写入，经过校验 |
| `recommend_packages` | 否 | 委派给 Python；仅供参考 |

## 错误

```jsonc
{"id": 2, "protocol_version": 1, "status": "error",
 "error": {"code": "safety_gate_closed",
           "message": "refusing to write partition table: destructive mode is not enabled",
           "remediation": "restart camareLOL with --allow-destructive"},
 "duration_ms": 1}
```

错误就是一种响应。没有单独的错误通道；而一个只读取 `result`、
忽略 `status` 的调用方，会把失败读成成功。

### 稳定的错误码

| 分组 | 代码 |
| --- | --- |
| 工具 | `missing_tool`、`spawn_failed`、`command_failed` |
| 解析 | `parse_failed`、`unsupported_schema`、`invalid_config`、`invalid_plan` |
| 存储 | `no_target_disk`、`device_gone`、`device_identity_changed`、`device_busy` |
| 引导与账户 | `invalid_boot_config`、`invalid_account` |
| 网络 | `network_unreachable` |
| 安全 | `safety_gate_closed`、`confirmation_required` |
| 更新 | `update_download_failed`、`update_manifest_invalid`、`update_signature_missing`、`update_signature_invalid`、`update_expired`、`update_checksum_mismatch`、`update_candidate_failed`、`update_not_privileged`、`update_url_refused`、`update_rollback_failed` |
| 其他 | `not_supported`、`internal` |

## 规则

1. 核心校验每一个请求，无论它是谁发出的。调用方属于本项目，
   不是放松检查的理由。
2. 一个破坏性方法会被拒绝，除非安全闸门已解除锁定**并且**该操作
   相应风险等级的确认短语已经被记录。
3. 带 `status: "error"` 的响应绝不能被当作成功。
4. 超时是按请求计算的。被取消的请求是**被回应**的，而不是被丢弃——
   一个始终收不到回应的调用方无法区分"慢"和"死"。
5. `protocol_version` 在其他一切之前就被检查。未知版本会被拒绝，
   而不是靠猜测去解析。

## 为什么每条消息里都带版本号

因为现在检查很便宜，而事后再检查就不可能了。一个连上了较旧核心
并默默误读了某个字段的客户端，会比一个直接拒绝与它对话的客户端更糟。

## 目前谁实现了它

该协议已被定义和校验，核心在内部把它暴露给 TUI。它**没有**作为
网络服务或 stdin 服务暴露：`camarelol` 没有 `--serve` 标志，
也不存在监听的套接字。

目前这是刻意为之。一个通过套接字接受分区请求的守护进程，对于一个
会擦除磁盘的工具来说是新的攻击面，而目前并不需要它。加不加它是一个
需要刻意做出的决定，而不是一个可以随手打开的开关。

如果你现在要写一个工具，请改用有文档的 CLI——`--scan` 以 JSON 输出
一次设备探测，`--scripted` 根据配置档运行一次安装，两者都是稳定的。

## 下一篇

- [架构](architecture.md)
- [测试](testing.md)