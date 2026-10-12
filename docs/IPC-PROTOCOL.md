# camareLOL IPC protocol (version 1)

Newline-delimited JSON (NDJSON): one request per line, one response per line.
The Rust core is the only component allowed to perform system-changing
operations; Python and Java talk to it (or to the Java catalog's own server)
over this protocol.

## Wire format

```jsonc
// request
{"id": 1, "protocol_version": 1, "method": "scan_devices", "params": {}, "timeout_ms": 120000}

// response
{"id": 1, "protocol_version": 1, "status": "ok", "result": {...}, "duration_ms": 3}
```

| field | type | meaning |
| --- | --- | --- |
| `id` | number/string | echoed back; must be non-zero |
| `protocol_version` | number | must be `1` |
| `method` | string | see below |
| `params` | object | method arguments |
| `timeout_ms` | number? | per-request timeout (default 120 000) |
| `cancel` | number? | id of a previous request to cancel |

## Methods

| method | destructive? | description |
| --- | --- | --- |
| `hello` | no | protocol discovery |
| `scan_devices` | no | full storage survey (JSON array) |
| `create_plan` | no | build a partition plan for `params.disk` |
| `validate_plan` | no | re-validate a plan |
| `apply_plan` | **yes** | write the partition table (requires armed gate) |
| `format_partitions` | **yes** | create filesystems (requires armed gate) |
| `diagnostics` | no | diagnostics report |
| `get_config` / `set_config` | no | configuration read/write (validated) |
| `recommend_packages` | no | delegated to Python (informational) |

## Errors

```jsonc
{"id": 2, "protocol_version": 1, "status": "error",
 "error": {"code": "safety_gate_closed",
           "message": "refusing to write partition table: destructive mode is not enabled",
           "remediation": "restart camareLOL with --allow-destructive"},
 "duration_ms": 1}
```

Stable error codes: `missing_tool`, `spawn_failed`, `command_failed`,
`parse_failed`, `no_target_disk`, `device_gone`, `device_identity_changed`,
`device_busy`, `invalid_plan`, `invalid_config`, `unsupported_schema`,
`invalid_boot_config`, `invalid_account`, `network_unreachable`,
`safety_gate_closed`, `confirmation_required`, `not_supported`, `internal`.

## Rules

1. The core validates every request, regardless of the caller.
2. A destructive method is refused unless the gate is armed **and** the
   confirmation phrase for the operation's risk level has been recorded.
3. A response with `status: "error"` must never be treated as success.
4. Timeouts are per request; a cancelled request is answered, not dropped.
