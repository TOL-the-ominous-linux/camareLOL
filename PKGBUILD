# Maintainer: camareLOL contributors
# A modern, safety-first TUI installer for Arch Linux.
#
# This PKGBUILD builds from the GitHub release tarball so that `makepkg` never
# needs network access to a VCS host.
pkgname=camarelol
# Bản phát hành sinh PKGBUILD này từ khuôn ở trong kho: pkgver, URL nguồn và
# sha256 đều lấy từ chính bản phát hành đang dựng. Sửa tay ở đây sẽ bị ghi đè,
# và nếu sửa sai thì `makepkg` tải một tarball khác — hoặc không tải được gì.
pkgver=0.1.17
pkgrel=1
pkgdesc="A modern, safety-first TUI installer for Arch Linux"
arch=('x86_64')
url="https://github.com/tamkogay9876-code/camareLOL"
license=('GPL3')
depends=(
    'util-linux'            # lsblk, findmnt, blkid
    'gptfdisk'              # sgdisk
    'arch-install-scripts'  # pacstrap, genfstab, arch-chroot
)
makedepends=(
    'rust'
    'cargo'
)
optdepends=(
    'grub: GRUB bootloader support'
    'iwd: Wi-Fi support in the live environment'
)
# The binary must keep its panic messages and debug info useful, and we ship a
# reproducible build via the release profile.
options=('!strip' '!debug')
# URL đầy đủ, không phải tên tệp trần: `source` cho một tệp tải về từ mạng phải
# là URL. Tên tệp trần khiến makepkg tìm trong thư mục hiện tại và hỏng.
source=("https://github.com/tamkogay9876-code/camareLOL/releases/download/v$pkgver/$pkgname-$pkgver.tar.gz")
# Không phải SKIP. Đây là dự án kiểm tra sha256 của chính nó trước khi cài bất
# cứ thứ gì; để ngoại trừ nó ở đây thì việc kiểm tra đó chỉ còn đúng khi người
# dùng tự làm, và im lặng sai khi không.
sha256sums=('0cb1b5de603111fe12714ce03f6a960b122b804c8a1cf5f6b84a62f6bd128181')

_pkgdir() {
    # The release tarball is created from the repository root, so the directory
    # name depends on the tag layout. Accept both and fail loudly otherwise.
    local d
    for d in "$srcdir/$pkgname-$pkgver" "$srcdir/$pkgname" "$srcdir/camarelOL"; do
        if [ -f "$d/Cargo.toml" ] && [ -f "$d/tui/Cargo.toml" ]; then
            printf '%s' "$d"
            return 0
        fi
    done
    error "cannot find the workspace root (Cargo.toml + tui/Cargo.toml) in $srcdir"
    return 1
}

build() {
    cd "$(_pkgdir)"
    # Build only the user-facing binary; the core is a library dependency.
    cargo build --release --locked --manifest-path tui/Cargo.toml
}

check() {
    cd "$(_pkgdir)"
    # CI runs the test suite in a dedicated job, so it sets this to avoid
    # running the same 92 tests twice. Packagers still get them here.
    [ -n "${CARAMELOL_SKIP_TESTS:-}" ] && { echo "skipping tests (CARAMELOL_SKIP_TESTS set)"; return 0; }
    cargo test --release --locked -p camarelol-core
}

package() {
    cd "$(_pkgdir)"
    install -Dm755 "target/release/camarelol" "$pkgdir/usr/bin/camarelol"

    install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
    # Ship whatever documentation the tarball happens to carry; a missing
    # doc file must not break the package.
    local doc
    for doc in README.md docs/*.md; do
        [ -f "$doc" ] || continue
        install -Dm644 "$doc" "$pkgdir/usr/share/doc/$pkgname/$(basename "$doc")"
    done
}