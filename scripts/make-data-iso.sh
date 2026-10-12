# Build the camareLOL data ISO used by the VirtualBox acceptance test.
#
# The ISO contains the built binary plus a sample profile, so the VM can run
# the installer without network access.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BINARY="${1:-$ROOT/target/release/camarelol}"
OUT="${2:-$ROOT/.test/camarelol-data.iso}"

if [ ! -x "$BINARY" ]; then
    echo "error: binary not found: $BINARY" >&2
    exit 1
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

mkdir -p "$WORK/iso"
cp "$BINARY" "$WORK/iso/camarelol"
cat > "$WORK/iso/test-profile.toml" <<'EOF'
# Sample profile for the VirtualBox acceptance test.
# The installer re-verifies the target disk before writing anything.
schema_version = 1
language = "en"
system_locale = "en_US.UTF-8"
keymap = "us"
timezone = "UTC"
firmware = "auto"
filesystem = "ext4"
separate_boot = false
separate_home = false
swap_bytes = null
bootloader = "grub"
desktop = "console"
display_server = "auto"
hostname = "camarelol-vm"
root_login_allowed = true
users = []
admin_users = []
EOF

command -v xorriso >/dev/null || { echo "error: xorriso is required" >&2; exit 1; }
xorriso -as mkisofs -o "$OUT" -V "CAMARELOL" "$WORK/iso" >/dev/null
echo "wrote $OUT"
