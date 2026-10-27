# LOL kernel

**Status: planned. Nothing is built, packaged or shipped yet.**

This page exists so the name has a definition before it has code. If the project
never gets past this page, that is a better outcome than shipping a kernel with
a confident README and no tests.

## What it is

`LOL kernel` is the kernel line that camareLOL may install when the stock Arch
kernel is not enough for a machine. It is **Arch-based**: the package tracks
Arch's `linux` package, keeps Arch's configuration and patch set, and adds a
small, documented set of changes. It is delivered as normal Arch packages:

| Package | Provides |
|---|---|
| `linux-lol` | the kernel image, modules and firmware hooks |
| `linux-lol-headers` | headers for out-of-tree modules |

`uname -r` must show the suffix, so a support request can tell which kernel is
running: `6.x.y-arch1-1-lol`.

## What it is not

- **Not a distribution.** camareLOL installs Arch Linux. The kernel is a package
  choice inside that install, not a separate OS.
- **Not a fork with its own subsystem.** TOL does not maintain a Linux fork.
  Patches come from upstream, from Arch, or from a named source with a link.
- **Not the default.** Stock `linux` stays the default. `linux-lol` is opt-in,
  chosen explicitly, and the installer must say so on the Review screen.
- **Not a performance meme.** No "gaming" sysctl bundles, no scheduler swaps
  without a measured reason, no patch that only exists to make a benchmark
  number move.

## Why it might exist

Arch's kernel is deliberately close to upstream and deliberately general. That
is the right default, and it is why camareLOL does not need its own kernel to
install Arch. A separate line is justified only for hardware that is known to
be painful on a fresh Arch install, for example:

- recent laptops whose Wi-Fi or touchpad needs a patch that has not reached the
  stable Arch package yet;
- filesystems or storage controllers where Arch is behind the upstream fix;
- machines where a specific, documented module option is needed at boot and
  cannot be expressed cleanly through modprobe configuration.

Each entry in that list needs a machine, a symptom, a patch and a test. A
wishlist is not a reason to ship a kernel.

## Patch policy

1. Every patch has a source: upstream commit, mailing-list thread, or the
   hardware vendor's tree. No anonymous diffs.
2. Every patch has a test that fails without it. "It feels faster" is not a
   test.
3. A patch is dropped when it lands upstream and Arch picks it up. The set
   shrinks over time; it does not accumulate.
4. Configuration changes are listed in the package description and in the
   release notes. A kernel that silently changes a default is worse than the
   bug it fixed.
5. No patch touches the safety properties camareLOL relies on: block-device
   naming, partition-table reads, `fsync` behaviour.

## Versioning

`pkgver` follows Arch's kernel package, plus a TOL revision:

```
6.17.4.arch1-1        Arch
6.17.4.arch1-1.tol1   first TOL build of that Arch kernel
6.17.4.arch1-1.tol2   second TOL build: patch set or config changed
```

A rebuild that changes nothing does not get a new revision. A change to the
patch set does, even if the Arch `pkgver` is the same.

## What must be true before the first release

- [ ] A PKGBUILD in a TOL repository, building from Arch's source tarball.
- [ ] A written patch list with one line of justification per patch.
- [ ] The package builds in a clean chroot, not on the author's desktop.
- [ ] `linux-lol` and `linux-lol-headers` install and remove cleanly alongside
      stock `linux`.
- [ ] Boot tested in a VM: firmware → bootloader → kernel → userspace, with the
      `-lol` suffix visible in `uname -r`.
- [ ] Module loading checked for at least one out-of-tree module (e.g. a
      filesystem or a network driver) to prove the headers package works.
- [ ] Suspend/resume, Wi-Fi and GPU tested on at least one real machine, with
      the machine named in the notes.
- [ ] The camareLOL installer can select it, shows it on the Review screen, and
      still installs stock `linux` when the user does not choose it.
- [ ] The release notes say which Arch kernel it tracks and how to go back to
      stock `linux`.

Until every box is ticked, the honest description of `linux-lol` is "planned".

## Relationship to camareLOL

camareLOL does not need the LOL kernel to work. The installer's job is to put a
working Arch system on a disk, and stock `linux` does that. The kernel line is a
separate, optional package; if it is ever offered in the installer, it must be

- **off by default**,
- **named on the Review screen**, in the same list as every other package,
- **reversible**: the release notes must show the command that puts stock
  `linux` back,
- **tested by the same acceptance harness** as the rest of the install, not by
  a separate script that only the author runs.

If those four conditions cannot be met, the kernel line stays outside camareLOL
as a package people install themselves.

## Security

A kernel is the largest attack surface in the system. TOL is not a security
team, and this kernel must not be marketed as "hardened". If a hardened
configuration is ever offered, it is a documented config profile with its
trade-offs written down — not a slogan. Signing keys, build provenance and
reproducibility are requirements for a release, not stretch goals.
