# The IPC protocol, version 1

[← Documentation index](index.md)

Newline-delimited JSON: one request per line, one response per line.

Only the Rust core may perform system-changing operations. Python and Java talk
to it — or to the Java catalog's own server — over this protocol.

## Wire format

```jsonc
// request
{"id": 1, "protocol_version": 1, "method": "scan_devices", "params": {}, "timeout_ms": 120000}

// response
{"id": 1, "protocol_version": 1, "status": "ok", "result": {}, "duration_ms": 3}
```

| field | type | meaning |
| --- | --- | --- |
| `id` | number or string | echoed back; must be non-zero |
| `protocol_version` | number | must be `1` |
| `method` | string | see below |
| `params` | object | method arguments |
| `timeout_ms` | number, optional | per-request timeout; default 120 000 |
| `cancel` | number, optional | id of a previous request to cancel |

## Methods

| method | destructive | description |
| --- | --- | --- |
| `hello` | no | protocol discovery; the result carries `host`, e.g. `Arch Linux / systemd` |
| `scan_devices` | no | full storage survey, as a JSON array |
| `create_plan` | no | build a partition plan for `params.disk` |
| `validate_plan` | no | re-validate an existing plan |
| `apply_plan` | **yes** | write the partition table; requires an armed gate |
| `format_partitions` | **yes** | create filesystems; requires an armed gate |
| `diagnostics` | no | diagnostics report |
| `get_config` / `set_config` | no | configuration read and write, validated |
| `recommend_packages` | no | delegated to Python; informational only |

## Errors

```jsonc
{"id": 2, "protocol_version": 1, "status": "error",
 "error": {"code": "safety_gate_closed",
           "message": "refusing to write partition table: destructive mode is not enabled",
           "remediation": "restart camareLOL with --allow-destructive"},
 "duration_ms": 1}
```

An error is a response. There is no separate error channel, and a caller that
reads only `result` and ignores `status` will read a failure as a success.

### Stable error codes

| group | codes |
| --- | --- |
| tools | `missing_tool`, `spawn_failed`, `command_failed` |
| parsing | `parse_failed`, `unsupported_schema`, `invalid_config`, `invalid_plan` |
| storage | `no_target_disk`, `device_gone`, `device_identity_changed`, `device_busy` |
| boot and accounts | `invalid_boot_config`, `invalid_account` |
| network | `network_unreachable` |
| safety | `safety_gate_closed`, `confirmation_required`, `unsupported_host`, `aur_build_refused` |
| updates | `update_download_failed`, `update_manifest_invalid`, `update_signature_missing`, `update_signature_invalid`, `update_expired`, `update_checksum_mismatch`, `update_candidate_failed`, `update_not_privileged`, `update_url_refused`, `update_rollback_failed` |
| other | `not_supported`, `internal` |

## Rules

1. The core validates every request, regardless of who sent it. A caller being
   part of this project is not a reason to relax a check.
2. A destructive method is refused unless the gate is armed **and** the
   confirmation phrase for that operation's risk level has been recorded.
3. A destructive method is also refused with `unsupported_host` when the
   distribution or init system is not Arch + systemd. The check runs *before*
   the gate, so the user is told the real reason rather than being told to pass
   a flag that would not make the install safe. It lives in `apply_plan` and not
   only in the install orchestrator, because the IPC surface writes a partition
   table on its own.
4. A response with `status: "error"` must never be treated as success.
5. Timeouts are per request. A cancelled request is **answered**, not dropped —
   a caller that never hears back cannot tell a slow operation from a dead one.
6. `protocol_version` is checked before anything else. An unknown version is
   refused rather than parsed on a guess.

## Why the version is in every message

Because it is cheap to check and impossible to check later. A client that
connected to an older core and silently misread a field would be worse than one
that refuses to talk to it.

## Who implements it today

The protocol is defined and validated, and the core exposes it internally to the
TUI. It is **not** exposed as a network or stdin service: `camarelol` has no
`--serve` flag, and there is no listening socket.

That is deliberate for now. A daemon that accepts partition requests over a
socket is a new attack surface on a tool that erases disks, and it has not been
needed. Adding one is a decision to make deliberately, not a flag to turn on.

If you are writing a tool today, use the documented CLI instead — `--scan`
emits a device survey as JSON, `--scripted` runs an install from a profile, and
both are stable.

## Next

- [Architecture](architecture.md)
- [Testing](testing.md)