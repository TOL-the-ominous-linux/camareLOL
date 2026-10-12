# camareLOL — Python tools

Automation layer around the Rust core:

| module | responsibility |
| --- | --- |
| `config_validate` | strict, opinionated validation of installation profiles (incl. rejecting secrets) |
| `package_recommend` | package/dependency suggestions, groups, AUR policy |
| `diagnostics` | secret-free diagnostics reports for bug reports |
| `ipc_client` | JSON IPC client for the Rust core (`docs/IPC-PROTOCOL.md`) |
| `hw_info` | CPU/RAM/GPU/firmware processing, zram sizing |
| `testutil` | device surveys for the acceptance scenarios |

Run it directly from a checkout:

```console
$ python3 -m camarelol_tools.config_validate profiles/*.toml
$ python3 -m camarelol_tools.package_recommend --desktop plasma --server wayland
$ python3 -m camarelol_tools.diagnostics -o report.json
$ python3 -m camarelol_tools.scenarios          # needs the built binary
```

Tests: `python3 -m unittest discover -s tests -v`

Nothing in this package can modify a disk: every destructive request is sent to
the Rust core, which revalidates it and passes it through its safety gate.
