# Maintainer: camareLOL contributors
# A modern, safety-first TUI installer for Arch Linux.
pkgname=camarelol
pkgver=0.1.0
pkgrel=1
pkgdesc="A modern, safety-first TUI installer for Arch Linux"
arch=('x86_64')
url="https://github.com/tamkogay9876-code/camareLOL"
license=('GPL3')
depends=(
    'util-linux'      # lsblk, findmnt, blkid
    'parted'
    'gptfdisk'        # sgdisk
    'arch-install-scripts'  # pacstrap, genfstab, arch-chroot
    'python'
)
makedepends=(
    'rust'
    'cargo'
)
optdepends=(
    'iwd: Wi-Fi support'
    'grub: bootloader'
    'systemd-boot: bootloader'
    'squashfs-tools: for rebuilding the ISO'
)
conflicts=()
provides=()
options=(!strip)

build() {
    cd "$srcdir/$pkgname"
    cargo build --release --locked --manifest-path tui/Cargo.toml
}

package() {
    cd "$srcdir/$pkgname"
    install -Dm755 "target/release/camarelol" "$pkgdir/usr/bin/camarelol"
    install -Dm644 "README.md" "$pkgdir/usr/share/doc/camarelol/README.md"
    install -Dm644 "docs/ARCHITECTURE.md" "$pkgdir/usr/share/doc/camarelol/ARCHITECTURE.md"
    install -Dm644 "docs/ROADMAP.md" "$pkgdir/usr/share/doc/camarelol/ROADMAP.md"
    install -Dm644 "LICENSE" "$pkgdir/usr/share/licenses/camarelol/LICENSE"
}
