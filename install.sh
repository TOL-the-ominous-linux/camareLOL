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

# rustup installs cargo into ~/.cargo/bin, which is often not on PATH yet.
ensure_cargo() {
    if ! command -v cargo >/dev/null 2>&1; then
        if [ -x "$HOME/.cargo/bin/cargo" ]; then
            export PATH="$HOME/.cargo/bin:$PATH"
        else
            die "cargo not found; install Rust first:
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        fi
    fi
}

install_binary() {
    need_root
    local arch tmp base version tag expected actual name
    case "$(uname -m)" in
        x86_64|amd64) arch="x86_64" ;;
        *) die "no prebuilt binary for $(uname -m); build from source instead" ;;
    esac
    version="${CAMARELOL_VERSION:-}"
    if [ -n "$version" ]; then
        tag="v${version#v}"
        base="$REPO/releases/download/$tag"
        info "downloading release $tag"
    else
        base="$REPO/releases/latest/download"
        info "downloading the newest stable release"
    fi
    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' RETURN

    # The checksum is not optional. Without it, this helper would install
    # whatever a proxy or a broken CDN returned under the right file name.
    curl -fsSL --retry 2 "$base/SHA256SUMS" -o "$tmp/SHA256SUMS" \
        || die "could not download $base/SHA256SUMS"
    expected="$(awk '$2 == "camarelol" || $2 == "*camarelol" { print $1 }' "$tmp/SHA256SUMS")"
    [ -n "$expected" ] || die "SHA256SUMS has no line for camarelol"

    for name in "camarelol-linux-$arch" "camarelol"; do
        info "downloading $name"
        if curl -fsSL --retry 2 "$base/$name" -o "$tmp/camarelol"; then
            break
        fi
    done
    [ -s "$tmp/camarelol" ] || die "no release binary found at $REPO/releases"

    actual="$(sha256sum "$tmp/camarelol" | awk '{print $1}')"
    [ "$actual" = "$expected" ] \
        || die "SHA-256 mismatch for camarelol:
  downloaded: $actual
  published:  $expected
Refusing to install an unverified binary."

    info "installing to $BIN"
    install -Dm755 "$tmp/camarelol" "$BIN"
    rm -rf "$tmp"
    trap - RETURN
}

build_from_source() {
    need_root
    ensure_cargo
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
    [ -x "$BIN" ] || die "$BIN was not created"
    info "installed: $("$BIN" --version)"
    info
    info "Next steps:"
    info "  1. Run the non-destructive checks:  camarelol --self-test"
    info "  2. From the Arch live ISO:  sudo camarelol --allow-destructive"
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