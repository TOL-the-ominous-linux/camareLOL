#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# camareLOL — build & test script for the whole project.
#
#   ./build.sh build   compile everything (Rust, Python, Java)
#   ./build.sh test    unit tests + acceptance scenarios
#   ./build.sh fmt     format the Rust sources
#   ./build.sh lint    clippy + rustfmt check
#   ./build.sh clean   remove build artifacts
#
# Needs: cargo (rustup), python3, javac (JDK 17+).
# ---------------------------------------------------------------------------
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT"

# Rust installed by rustup into the user's home when missing.
if ! command -v cargo >/dev/null 2>&1; then
    if [ -x "$HOME/.cargo/bin/cargo" ]; then
        export PATH="$HOME/.cargo/bin:$PATH"
    else
        echo "error: cargo not found; install it with:" >&2
        echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y" >&2
        exit 1
    fi
fi

# JDK: use JAVA_HOME, then the usual PATH lookup.
if [ -n "${JAVA_HOME:-}" ] && [ -x "$JAVA_HOME/bin/javac" ]; then
    export PATH="$JAVA_HOME/bin:$PATH"
fi
if [ -z "${JAVA_HOME:-}" ] && ! command -v javac >/dev/null 2>&1; then
    for candidate in "$HOME"/tools/jdk*/bin/javac /usr/lib/jvm/*/bin/javac; do
        [ -x "$candidate" ] && export JAVA_HOME="$(dirname "$(dirname "$candidate")")" && break
    done
    [ -n "${JAVA_HOME:-}" ] && export PATH="$JAVA_HOME/bin:$PATH"
fi

# Keep parallel codegen modest: the workspace is small and big -j values
# exhaust memory on modest machines.
CARGO_JOBS="${CARGO_JOBS:-2}"

target="${1:-build}"

case "$target" in
    build)
        echo "==> cargo build"
        cargo build -j "$CARGO_JOBS" --workspace
        echo "==> python tests"
        (cd python && python3 -m unittest discover -s tests -q)
        echo "==> java catalog"
        (cd java && ./build.sh)
        ;;

    test)
        echo "==> cargo test"
        cargo test -j "$CARGO_JOBS" --workspace
        echo "==> python tests"
        (cd python && python3 -m unittest discover -s tests -v)
        echo "==> java catalog"
        (cd java && ./build.sh)
        echo "==> acceptance scenarios (binary vs. synthetic surveys)"
        (cd python && CAMARELOL_BIN="$ROOT/target/debug/camarelol" \
            python3 -m camarelol_tools.testutil)
        ;;

    fmt)
        cargo fmt --all
        echo "formatted"
        ;;

    lint)
        cargo fmt --all -- --check
        cargo clippy --workspace --all-targets -- -D warnings
        ;;

    clean)
        cargo clean
        rm -rf java/out
        find . -name __pycache__ -type d -prune -exec rm -rf {} + 2>/dev/null || true
        echo "cleaned"
        ;;

    *)
        echo "usage: $0 {build|test|fmt|lint|clean}"
        exit 2
        ;;
esac