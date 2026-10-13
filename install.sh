#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# camareLOL installer helper.
#
#   ./install.sh              build from this checkout (default)
#   ./install.sh --binary     download the prebuilt binary from a release
#   ./install.sh --uninstall  remove an installed camarelol
#
# camareLOL is an installer: you normally run it from the live ISO, so the
# default target is /usr/local, not a package database.
# ---------------------------------------------------------------------------
set -euo pipefail

REPO="https://github.com/tamkogay9876-code/camareLOL"
PREFIX="${PREFIX:-/usr/local}"
BIN="$PREFIX/bin/camarelol"

info() { printf '\033[1;36m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m!!\033[0m %s\n' "$*" >&2; }
die()  { printf '\033[1;31mxx\033[0m %s\n' "$*" >&2; exit 1; }

need_root() {
    [ "$(id -u)" -eq 0 ] || die "needs root: run with sudo"
}

install_binary() {
    need_root
    local os arch url tmp
    os=$(uname -s | tr '[:upper:]' '[:lower:]')
    case "$(uname -m)" in
        x86_64|amd64) arch="x86_64" ;;
        *) die "no prebuilt binary for $(uname -m); build from source instead" ;;
    esac
    url="$REPO/releases/latest/download/camarelol-linux-$arch"
    tmp=$(mktemp)
    info "downloading $url"
    curl -sSfL "$url" -o "$tmp" || die "download failed"
    info "installing to $BIN"
    install -Dm755 "$tmp" "$BIN"
    rm -f "$tmp"
}

build_from_source() {
    need_root
    command -v cargo >/dev/null || die "cargo not found; install Rust from https://rustup.rs"
    local here; here=$(cd "$(dirname "$0")" && pwd)
    info "building camareLOL from source (this takes a few minutes)"
    (cd "$here" && cargo build --release --locked --manifest-path tui/Cargo.toml)
    info "installing to $BIN"
    install -Dm755 "$here/target/release/camarelol" "$BIN"
}

do_uninstall() {
    need_root
    info "removing $BIN"
    rm -f "$BIN"
}

verify() {
    command -v "$BIN" >/dev/null || die "$BIN is not on PATH"
    info "installed: $("$BIN" --version)"
    info
    info "Next steps:"
    info "  1. Run the non-destructive checks:  camarelol --self-test"
    info "  2. Boot the Arch live ISO and run:  sudo camarelol"
    info
    info "camareLOL never writes to a disk before you confirm it, and"
    info "re-checks the disk identity immediately before every write."
}

case "${1:-}" in
    --binary)    install_binary; verify ;;
    --uninstall) do_uninstall ;;
    --help|-h)   sed -n '2,12p' "$0" ;;
    *)           build_from_source; verify ;;
esac