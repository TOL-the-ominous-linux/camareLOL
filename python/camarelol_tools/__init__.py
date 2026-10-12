"""camareLOL Python tools — automation layer around the Rust core.

Responsibilities (see docs/ARCHITECTURE.md):
  * configuration validation (belt and braces on top of the Rust core)
  * package recommendations and dependency analysis
  * hardware information processing
  * diagnostics collection
  * test utilities for the acceptance criteria

The Python side NEVER performs destructive operations: every system-changing
request goes to the Rust core over the versioned JSON protocol
(docs/IPC-PROTOCOL.md).
"""

__version__ = "0.1.0"
PROTOCOL_VERSION = 1

__all__ = [
    "config_validate",
    "package_recommend",
    "diagnostics",
    "ipc_client",
    "hw_info",
    "testutil",
]
