//! Host detection: which distribution and which init system camareLOL is
//! running on.
//!
//! camareLOL was written against one host: **Arch Linux with systemd**. Every
//! package name, every generated script (`/etc/systemd/zram-generator.conf`,
//! `systemctl enable`, `mkinitcpio`, `%wheel` sudoers, `bootctl`) and the whole
//! VirtualBox acceptance run assume that pair. Running the same code on Artix,
//! on a Hyperbola laptop, or on Arch with OpenRC produces an install that
//! *looks* like it worked and is broken in ways no unit test can see.
//!
//! So the host is **detected, never assumed**, and a host that cannot be proven
//! compatible is refused at plan time — before a single byte is written.
//! Guessing is what produced the wrong-package-name class of bug this project
//! already suffered: everything was green and only a real machine found out.
//!
//! # Reading `/etc/os-release` without spawning anything
//!
//! The file is read with [`crate::sys::read_file`], not with `cat` through
//! [`crate::sys::inspect`]. That is deliberate: `read_file` starts **no process
//! at all**, so it cannot be caught by — or trip — the `--preview` spawn latch
//! in [`crate::sys`]. The acceptance criterion for preview is a spawn count of
//! exactly zero, and an `inspect("cat", …)` inside detection would break it.
//! Process-spawning read-only probes (`yay --version`, `id -u <user>`) do use
//! [`crate::sys::inspect`], because they genuinely need a program.
//!
//! # Decision table
//!
//! | Detected | Verdict | Why |
//! |---|---|---|
//! | `ID=arch` + systemd | **proceed** | the one combination this installer is built, documented and VM-accepted for |
//! | `ID=arch` + OpenRC / runit / dinit / unknown init | **refuse** | every service, unit and script camareLOL writes is systemd-specific; `systemctl enable` in a non-systemd target is a no-op that still reports success |
//! | Artix Linux | **refuse** | ships OpenRC or runit by default and its own install scripts and mirrors; not the tested host |
//! | Nyarch | **refuse** | Arch-based but not Arch: its own repositories, not the tested host |
//! | Hyperbola / Parabola (GNU/Linux-libre) | **refuse** | **not Arch**: non-free packages are dropped or replaced, so a package list that is correct on Arch can be missing or renamed here. This is exactly the bug class that passes every unit test and fails in a VM |
//! | any other `ID`, including `ID_LIKE=arch` derivatives (EndeavourOS, Manjaro, Arco…) | **refuse** | an unrecognised host is refused rather than assumed; nothing in this table has been validated on them |
//! | `/etc/os-release` absent, unreadable, or without `ID` | **refuse** | the host cannot be proven compatible, and refusing is the only honest answer |
//!
//! Parabola and Hyperbola deserve the emphasis: they use `pacman` and they are
//! frequently *called* Arch, which is exactly why a naive `ID_LIKE=arch` check
//! would wave them through. They are GNU/Linux-libre distributions with a
//! different binary set, and camareLOL has never been run on either.

use crate::error::{CamarelolError, ErrorCode, Result};
use crate::sys;
use serde::{Deserialize, Serialize};

/// Where `/etc/os-release` lives. The freedesktop fallback is checked too: some
/// systems only ship `/usr/lib/os-release`.
pub const OS_RELEASE_PATHS: &[&str] = &["/etc/os-release", "/usr/lib/os-release"];

// ---------------------------------------------------------------------------
// Distribution
// ---------------------------------------------------------------------------

/// Distribution identities camareLOL reasons about by name.
///
/// Everything that is not [`Distro::Arch`] is refused; the variants exist so the
/// refusal message can name the actual host instead of saying "unknown".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Distro {
    Arch,
    Artix,
    Nyarch,
    Hyperbola,
    Parabola,
    /// Recognised only in the sense that `/etc/os-release` named *something*.
    Unknown,
}

impl Distro {
    pub fn as_str(&self) -> &'static str {
        match self {
            Distro::Arch => "Arch Linux",
            Distro::Artix => "Artix Linux",
            Distro::Nyarch => "Nyarch",
            Distro::Hyperbola => "Hyperbola GNU/Linux-libre",
            Distro::Parabola => "Parabola GNU/Linux-libre",
            Distro::Unknown => "unknown distribution",
        }
    }

    /// The `ID` values this variant is recognised by, in lowercase.
    fn known_ids(&self) -> &'static [&'static str] {
        match self {
            Distro::Arch => &["arch"],
            Distro::Artix => &["artix"],
            Distro::Nyarch => &["nyarch"],
            Distro::Hyperbola => &["hyperbola"],
            Distro::Parabola => &["parabola"],
            Distro::Unknown => &[],
        }
    }
}

/// The five named distributions, in the order the decision table lists them.
const NAMED_DISTROS: [Distro; 5] = [
    Distro::Arch,
    Distro::Artix,
    Distro::Nyarch,
    Distro::Hyperbola,
    Distro::Parabola,
];

// ---------------------------------------------------------------------------
// Init system
// ---------------------------------------------------------------------------

/// Init system camareLOL can observe. camareLOL only ever *writes* systemd
/// configuration, so only [`InitSystem::Systemd`] is supported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InitSystem {
    Systemd,
    OpenRc,
    Runit,
    Dinit,
    /// Nothing recognisable: `/proc/1/comm` was unreadable and no marker was
    /// present. Refused, never treated as systemd.
    Unknown,
}

impl InitSystem {
    pub fn as_str(&self) -> &'static str {
        match self {
            InitSystem::Systemd => "systemd",
            InitSystem::OpenRc => "OpenRC",
            InitSystem::Runit => "runit",
            InitSystem::Dinit => "dinit",
            InitSystem::Unknown => "unknown init system",
        }
    }

    /// Map a `/proc/1/comm` value onto an init system.
    fn from_pid1_comm(comm: &str) -> Option<Self> {
        match comm.trim().to_ascii_lowercase().as_str() {
            "systemd" => Some(InitSystem::Systemd),
            // Artix, Gentoo, Alpine all use `openrc-init`; `/sbin/init` on
            // OpenRC systems is frequently a busybox init of the same name.
            "openrc-init" | "openrc" => Some(InitSystem::OpenRc),
            // Void's init is runit; it is not a separate variant because the
            // installer cannot do anything with it either way.
            "runit" => Some(InitSystem::Runit),
            "dinit" => Some(InitSystem::Dinit),
            _ => None,
        }
    }
}

/// Filesystem markers that identify a running init when `/proc/1/comm` is not
/// readable (a permission-restricted `/proc` is common in containers).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InitMarkers {
    pub systemd: bool,
    pub openrc: bool,
    pub runit: bool,
    pub dinit: bool,
}

// ---------------------------------------------------------------------------
// os-release parsing (pure)
// ---------------------------------------------------------------------------

/// The fields of `/etc/os-release` that camareLOL looks at.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OsRelease {
    /// `ID=` — empty when the key is missing or blank.
    pub id: String,
    /// `ID_LIKE=` split into individual distribution ids.
    pub id_like: Vec<String>,
    /// `NAME=` or `PRETTY_NAME=`, for the message the user reads.
    pub name: String,
}

/// Parse `/etc/os-release` text.
///
/// Pure and total: any input produces a value, and an unparseable or empty
/// file produces an [`OsRelease`] with empty fields, which later classifies as
/// "unknown" and is refused. It never panics and never guesses.
///
/// Rules, per the freedesktop specification:
/// - blank lines and lines whose first non-blank character is `#` are ignored;
/// - `export ` before the name is tolerated (harmless, seen in the wild);
/// - the value is the text after the **first** `=`, so a value containing `=`
///   survives;
/// - a single pair of matching surrounding quotes is removed;
/// - the backslash escapes the specification defines are unescaped inside
///   double quotes;
/// - whitespace around the name and the value is trimmed;
/// - a later assignment of the same key overrides an earlier one, as the
///   specification requires;
/// - names are case-sensitive (`ID`), values are compared case-insensitively.
pub fn parse_os_release(contents: &str) -> OsRelease {
    let mut id = String::new();
    let mut id_like_raw = String::new();
    let mut name = String::new();
    let mut pretty_name = String::new();

    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line).trim_start();
        let Some((key, raw)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = unquote_os_release(raw.trim());
        match key {
            "ID" => id = value,
            "ID_LIKE" => id_like_raw = value,
            "NAME" => name = value,
            "PRETTY_NAME" => pretty_name = value,
            _ => {}
        }
    }

    let id_like = id_like_raw
        .split_whitespace()
        .map(|s| s.to_ascii_lowercase())
        .collect();

    OsRelease {
        id: id.to_ascii_lowercase(),
        id_like,
        // PRETTY_NAME is the friendlier of the two and is what a user
        // recognises; NAME is the fallback.
        name: if pretty_name.is_empty() {
            name
        } else {
            pretty_name
        },
    }
}

/// Remove one pair of matching surrounding quotes and unescape the sequences
/// the os-release specification defines.
fn unquote_os_release(value: &str) -> String {
    let bytes = value.as_bytes();
    let quoted = bytes.len() >= 2
        && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\''));
    if !quoted {
        return value.to_string();
    }
    let inner = &value[1..value.len() - 1];
    // Single quotes are literal in the spec; double quotes allow escapes.
    if bytes[0] == b'\'' {
        return inner.to_string();
    }
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some('$') => out.push('$'),
            // An unknown escape is kept as written rather than silently
            // dropping the backslash.
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Decide which distribution an [`OsRelease`] describes.
///
/// `ID` is authoritative. `ID_LIKE` is consulted **only** to name a derivative
/// of a known non-Arch distribution (an Artix spin still says `artix`), never
/// to promote an unknown id to Arch: `ID_LIKE=arch` is printed by EndeavourOS,
/// Manjaro, ArcoLinux and others, and treating that as Arch is the mistake this
/// whole module exists to prevent.
pub fn classify_distro(release: &OsRelease) -> Distro {
    let id = release.id.to_ascii_lowercase();
    for distro in NAMED_DISTROS {
        if distro.known_ids().contains(&id.as_str()) {
            return distro;
        }
    }
    for distro in NAMED_DISTROS {
        if distro == Distro::Arch {
            // Never: an unknown `ID` is not Arch just because it is Arch-like.
            continue;
        }
        if release
            .id_like
            .iter()
            .any(|like| distro.known_ids().contains(&like.as_str()))
        {
            return distro;
        }
    }
    Distro::Unknown
}

/// Decide which init system is running.
///
/// `/proc/1/comm` wins: it is PID 1's own name, which is the ground truth. The
/// marker directories are only consulted when `/proc` was unreadable, and in
/// that fallback the *specific* markers are checked before systemd's, because
/// `/run/systemd/system` can be present on a host that is not booted with
/// systemd. Guessing is never an option: an unrecognised host is
/// [`InitSystem::Unknown`], which is refused.
pub fn classify_init(pid1_comm: Option<&str>, markers: InitMarkers) -> InitSystem {
    if let Some(comm) = pid1_comm {
        if let Some(init) = InitSystem::from_pid1_comm(comm) {
            return init;
        }
    }
    if markers.openrc {
        return InitSystem::OpenRc;
    }
    if markers.runit {
        return InitSystem::Runit;
    }
    if markers.dinit {
        return InitSystem::Dinit;
    }
    if markers.systemd {
        return InitSystem::Systemd;
    }
    InitSystem::Unknown
}

// ---------------------------------------------------------------------------
// The profile
// ---------------------------------------------------------------------------

/// Everything camareLOL decided about the machine it is running on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostProfile {
    pub distro: Distro,
    pub init: InitSystem,
    /// The raw `ID=` value, kept verbatim so the refusal message can name what
    /// the host actually calls itself.
    pub id: String,
    /// `NAME=` / `PRETTY_NAME=`, for display.
    pub name: String,
    /// True only for Arch + systemd.
    pub supported: bool,
    /// Why the host is (not) supported. Empty only when `supported` is true.
    pub reason: String,
}

impl HostProfile {
    /// The profile camareLOL is built for. Used by tests and by anything that
    /// has already validated the host by other means.
    pub fn arch_systemd() -> Self {
        HostProfile {
            distro: Distro::Arch,
            init: InitSystem::Systemd,
            id: "arch".into(),
            name: "Arch Linux".into(),
            supported: true,
            reason: "Arch Linux with systemd — the host camareLOL is built for".into(),
        }
    }

    /// Build a profile from parsed `os-release` text and an observed init
    /// system. Pure, so the whole decision table is testable without a
    /// filesystem.
    pub fn from_parts(release: &OsRelease, init: InitSystem) -> Self {
        let distro = classify_distro(release);
        let supported = distro == Distro::Arch && init == InitSystem::Systemd;
        let reason = if supported {
            "Arch Linux with systemd — the host camareLOL is built for".to_string()
        } else {
            refusal_reason(distro, init, &release.id, &release.name)
        };
        HostProfile {
            distro,
            init,
            id: release.id.clone(),
            name: release.name.clone(),
            supported,
            reason,
        }
    }

    /// One line for the welcome screen.
    pub fn summary(&self) -> String {
        let what = if self.name.is_empty() {
            self.distro.as_str().to_string()
        } else {
            self.name.clone()
        };
        format!("{what} / {}", self.init.as_str())
    }

    /// Refuse the host, with the decision spelled out.
    ///
    /// This is the message a user on Artix or Hyperbola will actually read, so
    /// it says *why* the host is refused instead of leaving them to guess
    /// whether it is a bug.
    pub fn require_supported(&self) -> Result<()> {
        if self.supported {
            return Ok(());
        }
        let host = if self.name.is_empty() {
            self.distro.as_str().to_string()
        } else {
            self.name.clone()
        };
        Err(CamarelolError::new(
            ErrorCode::UnsupportedHost,
            format!(
                "{host} with {} is not a host camareLOL supports: {}",
                self.init.as_str(),
                self.reason
            ),
        )
        .with_remediation(
            "boot the official Arch Linux live ISO and run camareLOL from there; \
             no disk has been touched",
        )
        .with_context(format!(
            "ID={} detected from os-release",
            quote_id(&self.id)
        )))
    }
}

/// `ID="..."` for the error context, or `ID=<missing>` when there was none.
fn quote_id(id: &str) -> String {
    if id.is_empty() {
        "<missing>".to_string()
    } else {
        format!("\"{id}\"")
    }
}

/// The refusal text, per the decision table at the top of this module.
fn refusal_reason(distro: Distro, init: InitSystem, id: &str, name: &str) -> String {
    if distro == Distro::Arch {
        // The distribution is right and the init is not.
        return format!(
            "camareLOL writes systemd configuration only (systemctl enable, \
             /etc/systemd/zram-generator.conf, mkinitcpio, bootctl), and this host \
             boots {}. On {} those commands silently do nothing while the install \
             still reports success",
            init.as_str(),
            init.as_str()
        );
    }
    let label = if name.is_empty() {
        distro.as_str().to_string()
    } else {
        name.to_string()
    };
    match distro {
        Distro::Arch => unreachable!("handled above"),
        Distro::Artix => format!(
            "{label} is not the host camareLOL was built for: it ships OpenRC or \
             runit by default rather than systemd, and carries its own install \
             scripts and mirrors. No camareLOL release has ever been accepted on \
             it, so an Artix machine is turned away instead of assumed"
        ),
        Distro::Nyarch => format!(
            "{label} is Arch-based but not Arch: it has its own repositories and \
             package set, and no camareLOL release has ever been run or accepted on it"
        ),
        Distro::Hyperbola | Distro::Parabola => format!(
            "{label} is a GNU/Linux-libre distribution, **not** Arch. It uses pacman, \
             but non-free packages are dropped or replaced, so a package list that is \
             correct on Arch can be missing or renamed here — and that kind of wrong \
             package name passes every unit test and only fails on a real machine"
        ),
        Distro::Unknown => {
            if id.is_empty() {
                "/etc/os-release has no ID field, so the distribution cannot be \
                 identified. camareLOL refuses to guess which packages are correct"
                    .to_string()
            } else {
                format!(
                    "distribution \"{id}\" is not one camareLOL has been validated on. \
                     An unrecognised host is refused rather than assumed, because the \
                     package list, the init layout and the bootloader steps are all \
                     Arch-specific"
                )
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Live detection
// ---------------------------------------------------------------------------

/// Detect the live host.
///
/// Read-only throughout: file reads and `sys::path_exists`, no process is
/// started, so it works identically in a `--preview` run. Nothing here can
/// change the machine, and nothing here needs the safety gate.
pub fn detect() -> HostProfile {
    let release = read_os_release().unwrap_or_default();
    let init = detect_init();
    HostProfile::from_parts(&release, init)
}

/// Read `os-release` from the first location that exists.
pub fn read_os_release() -> Option<OsRelease> {
    for path in OS_RELEASE_PATHS {
        if let Ok(text) = sys::read_file(path) {
            return Some(parse_os_release(&text));
        }
    }
    None
}

/// Detect the running init system from `/proc/1/comm` and the standard
/// runtime markers.
pub fn detect_init() -> InitSystem {
    let pid1 = sys::read_file("/proc/1/comm").ok();
    let markers = InitMarkers {
        systemd: sys::path_exists("/run/systemd/system"),
        openrc: sys::path_exists("/run/openrc"),
        // Void (runit) and runit-parallels create this directory.
        runit: sys::path_exists("/run/runit") || sys::path_exists("/run/runit.container"),
        dinit: sys::path_exists("/run/dinit") || sys::path_exists("/run/dinit.pids"),
    };
    classify_init(pid1.as_deref(), markers)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------- parsing

    #[test]
    fn parses_a_real_arch_os_release() {
        let release = parse_os_release(
            r#"
NAME="Arch Linux"
PRETTY_NAME="Arch Linux"
ID=arch
BUILD_ID=rolling
ANSI_COLOR="38;2;23;147;209"
LOGO=archlinux-logo
"#,
        );
        assert_eq!(release.id, "arch");
        assert_eq!(release.name, "Arch Linux");
        assert!(release.id_like.is_empty());
    }

    #[test]
    fn parses_a_real_artix_os_release() {
        // Artix really does ship `ID_LIKE=arch`; a naive check would call this
        // Arch and wave a non-systemd host through.
        let release = parse_os_release(
            r#"NAME="Artix Linux"
ID=artix
PRETTY_NAME="Artix Linux"
ID_LIKE=arch
BUILD_ID=rolling
"#,
        );
        assert_eq!(release.id, "artix");
        assert_eq!(release.id_like, vec!["arch".to_string()]);
    }

    #[test]
    fn id_like_accepts_several_values_and_lower_cases_them() {
        let release = parse_os_release("ID=hyperbola\nID_LIKE=\"Parabola Arch\"\n");
        assert_eq!(release.id, "hyperbola");
        assert_eq!(release.id_like, vec!["parabola", "arch"]);
    }

    #[test]
    fn an_inline_hash_is_part_of_the_value_not_a_comment() {
        // Only a `#` in the first column starts a comment, per the
        // specification. Treating an inline `#` as a comment would let a
        // mangled file read as `arch` when it says something else.
        let release = parse_os_release("ID=arch   # this is still the value\n");
        assert_eq!(release.id, "arch   # this is still the value");
        assert_eq!(classify_distro(&release), Distro::Unknown);
    }

    #[test]
    fn ignores_comments_and_blank_lines() {
        let release = parse_os_release("# a comment\n\n   \n#ID=nixos\nID=arch\n");
        assert_eq!(release.id, "arch");
    }

    #[test]
    fn tolerates_the_export_prefix() {
        let release = parse_os_release("export ID=arch\nexport NAME=\"Arch\"\n");
        assert_eq!(release.id, "arch");
        assert_eq!(release.name, "Arch");
    }

    #[test]
    fn single_quotes_are_literal_and_double_quotes_unescape() {
        assert_eq!(unquote_os_release(r#""Arch Linux""#), "Arch Linux");
        assert_eq!(unquote_os_release(r#"'Arch Linux'"#), "Arch Linux");
        assert_eq!(unquote_os_release(r#""a \"b\" c""#), "a \"b\" c");
        assert_eq!(unquote_os_release(r#""line\nbreak""#), "line\nbreak");
        // An unknown escape keeps its backslash rather than eating a character.
        assert_eq!(unquote_os_release(r#""a\qb""#), "a\\qb");
        // Unbalanced quotes are left exactly as they are.
        assert_eq!(unquote_os_release(r#""Arch"#), r#""Arch"#);
        assert_eq!(unquote_os_release("Arch"), "Arch");
    }

    #[test]
    fn a_value_may_contain_an_equals_sign() {
        let release = parse_os_release("ID=arch\nPRETTY_NAME=\"Arch = Linux\"\n");
        assert_eq!(release.name, "Arch = Linux");
    }

    #[test]
    fn the_last_assignment_of_a_key_wins() {
        let release = parse_os_release("ID=artix\nID=arch\n");
        assert_eq!(release.id, "arch");
    }

    #[test]
    fn key_names_are_case_sensitive_like_the_spec() {
        // `id=arch` is not `ID=arch`. Treating it as Arch would let a malformed
        // file authorise an install on an unidentified host.
        let release = parse_os_release("id=arch\n");
        assert_eq!(release.id, "");
        assert_eq!(classify_distro(&release), Distro::Unknown);
    }

    #[test]
    fn values_are_compared_case_insensitively() {
        assert_eq!(
            classify_distro(&parse_os_release("ID=ARCH\n")),
            Distro::Arch
        );
        assert_eq!(
            classify_distro(&parse_os_release("ID=Artix\n")),
            Distro::Artix
        );
    }

    #[test]
    fn an_empty_or_garbage_file_parses_to_nothing_rather_than_panicking() {
        for input in [
            "",
            "\n\n\n",
            "not a key value file",
            "=",
            "===",
            "ID=",
            "=\"x\"",
        ] {
            let release = parse_os_release(input);
            assert!(
                release.id.is_empty(),
                "input {input:?} produced ID {:?}",
                release.id
            );
            assert_eq!(classify_distro(&release), Distro::Unknown);
        }
    }

    #[test]
    fn pretty_name_wins_over_name_for_display() {
        let release = parse_os_release("NAME=\"Arch\"\nPRETTY_NAME=\"Arch Linux\"\nID=arch\n");
        assert_eq!(release.name, "Arch Linux");
        let release = parse_os_release("NAME=\"Arch\"\nID=arch\n");
        assert_eq!(release.name, "Arch");
    }

    // ------------------------------------------------------------ classifying

    #[test]
    fn each_named_distribution_is_classified_from_its_own_id() {
        for (id, expected) in [
            ("arch", Distro::Arch),
            ("artix", Distro::Artix),
            ("nyarch", Distro::Nyarch),
            ("hyperbola", Distro::Hyperbola),
            ("parabola", Distro::Parabola),
        ] {
            let release = parse_os_release(&format!("ID={id}\n"));
            assert_eq!(classify_distro(&release), expected, "ID={id}");
        }
    }

    #[test]
    fn arch_like_derivatives_are_never_promoted_to_arch() {
        // The trap this module exists for: EndeavourOS, Manjaro and
        // ArcoLinux all print ID_LIKE=arch, and two of them ship packages that
        // do not exist under their upstream names.
        for id in ["endeavouros", "manjaro", "arcolinux", "garuda", "cachyos"] {
            let release = parse_os_release(&format!("ID={id}\nID_LIKE=arch\n"));
            assert_eq!(
                classify_distro(&release),
                Distro::Unknown,
                "ID={id} must not be treated as Arch"
            );
        }
    }

    #[test]
    fn a_derivative_of_a_known_non_arch_distro_is_still_named() {
        // An Artix spin is refused *as Artix*, which gives a better message
        // than "unknown distribution".
        let release = parse_os_release("ID=artix-lite\nID_LIKE=artix\n");
        assert_eq!(classify_distro(&release), Distro::Artix);
    }

    #[test]
    fn init_is_classified_from_pid1_first() {
        assert_eq!(
            classify_init(Some("systemd\n"), InitMarkers::default()),
            InitSystem::Systemd
        );
        assert_eq!(
            classify_init(Some("openrc-init\n"), InitMarkers::default()),
            InitSystem::OpenRc
        );
        assert_eq!(
            classify_init(Some("runit\n"), InitMarkers::default()),
            InitSystem::Runit
        );
        assert_eq!(
            classify_init(Some("dinit\n"), InitMarkers::default()),
            InitSystem::Dinit
        );
        // A busybox `init` in a container is not systemd and must not be
        // guessed into it.
        assert_eq!(
            classify_init(Some("init\n"), InitMarkers::default()),
            InitSystem::Unknown
        );
    }

    #[test]
    fn markers_only_decide_when_proc_is_unreadable() {
        let markers = InitMarkers {
            systemd: true,
            ..Default::default()
        };
        assert_eq!(classify_init(None, markers), InitSystem::Systemd);
        // An unrecognised PID 1 falls through to the markers rather than
        // claiming systemd outright.
        assert_eq!(
            classify_init(Some("weird-init"), markers),
            InitSystem::Systemd
        );
    }

    #[test]
    fn specific_markers_win_over_the_systemd_one() {
        // `/run/systemd/system` can exist on a host that was not booted with
        // systemd; `/run/openrc` is the more specific statement.
        let markers = InitMarkers {
            systemd: true,
            openrc: true,
            runit: true,
            dinit: true,
        };
        assert_eq!(classify_init(None, markers), InitSystem::OpenRc);
        let markers = InitMarkers {
            systemd: true,
            runit: true,
            ..Default::default()
        };
        assert_eq!(classify_init(None, markers), InitSystem::Runit);
    }

    #[test]
    fn nothing_recognisable_is_unknown_never_systemd() {
        assert_eq!(
            classify_init(None, InitMarkers::default()),
            InitSystem::Unknown
        );
    }

    // ------------------------------------------------------------ the decision

    fn profile(os_release: &str, init: InitSystem) -> HostProfile {
        HostProfile::from_parts(&parse_os_release(os_release), init)
    }

    #[test]
    fn arch_on_systemd_is_the_only_supported_host() {
        let p = profile("ID=arch\n", InitSystem::Systemd);
        assert!(p.supported, "{}", p.reason);
        assert_eq!(p.distro, Distro::Arch);
        assert_eq!(p.init, InitSystem::Systemd);
        assert!(p.require_supported().is_ok());
    }

    #[test]
    fn arch_on_any_other_init_is_refused_and_says_why() {
        for init in [
            InitSystem::OpenRc,
            InitSystem::Runit,
            InitSystem::Dinit,
            InitSystem::Unknown,
        ] {
            let p = profile("ID=arch\n", init);
            assert!(!p.supported, "Arch on {} must be refused", init.as_str());
            let err = p.require_supported().unwrap_err();
            assert_eq!(err.code, ErrorCode::UnsupportedHost);
            let text = err.to_string();
            assert!(text.contains("systemd"), "{}", text);
            assert!(
                err.remediation.as_deref().unwrap().contains("live ISO"),
                "the remediation must say what to do: {text}"
            );
        }
    }

    #[test]
    fn artix_nyarch_hyperbola_and_parabola_are_refused() {
        for (id, expected) in [
            ("artix", Distro::Artix),
            ("nyarch", Distro::Nyarch),
            ("hyperbola", Distro::Hyperbola),
            ("parabola", Distro::Parabola),
        ] {
            let p = profile(&format!("ID={id}\nID_LIKE=arch\n"), InitSystem::Systemd);
            assert!(!p.supported, "{id} must be refused even on systemd");
            assert_eq!(p.distro, expected);
            let err = p.require_supported().unwrap_err();
            assert_eq!(err.code, ErrorCode::UnsupportedHost);
        }
    }

    #[test]
    fn parabola_and_hyperbola_are_named_as_libre_not_arch() {
        for id in ["parabola", "hyperbola"] {
            let p = profile(&format!("ID={id}\nID_LIKE=arch\n"), InitSystem::Systemd);
            let text = p.reason.clone();
            assert!(text.contains("GNU/Linux-libre"), "{id}: {text}");
            assert!(
                text.contains("not** Arch") || text.contains("not Arch"),
                "{id}: the refusal must state it is not Arch: {text}"
            );
            assert!(
                text.contains("pacman"),
                "{id}: it should explain the pacman-but-different trap: {text}"
            );
        }
    }

    #[test]
    fn artix_and_nyarch_refusals_name_their_own_reason() {
        let artix = profile("ID=artix\n", InitSystem::Systemd);
        assert!(artix.reason.contains("OpenRC"), "{}", artix.reason);
        let nyarch = profile("ID=nyarch\n", InitSystem::Systemd);
        assert!(
            nyarch.reason.contains("Arch-based but not Arch"),
            "{}",
            nyarch.reason
        );
    }

    #[test]
    fn an_unknown_distribution_is_refused_even_when_it_claims_to_be_arch_like() {
        let p = profile("ID=somethingelse\nID_LIKE=arch\n", InitSystem::Systemd);
        assert!(!p.supported);
        assert_eq!(p.distro, Distro::Unknown);
        let err = p.require_supported().unwrap_err();
        assert!(err.to_string().contains("somethingelse"), "{err}");
        assert_eq!(
            err.context.as_deref(),
            Some("ID=\"somethingelse\" detected from os-release"),
            "the raw ID must survive into the error"
        );
    }

    #[test]
    fn a_missing_id_says_so_instead_of_naming_a_distribution() {
        let p = profile("PRETTY_NAME=\"Some Live Image\"\n", InitSystem::Systemd);
        assert!(!p.supported);
        let err = p.require_supported().unwrap_err();
        assert!(err.to_string().contains("no ID field"), "{err}");
        assert!(err.context.as_deref().unwrap().contains("ID=<missing>"));
    }

    #[test]
    fn an_empty_os_release_is_refused() {
        let p = profile("", InitSystem::Systemd);
        assert!(!p.supported);
        assert_eq!(p.distro, Distro::Unknown);
        assert!(p.require_supported().is_err());
    }

    #[test]
    fn the_supported_profile_is_stable() {
        let p = HostProfile::arch_systemd();
        assert!(p.supported);
        assert_eq!(p.summary(), "Arch Linux / systemd");
        assert!(p.require_supported().is_ok());
    }

    #[test]
    fn summary_prefers_the_reported_name() {
        let p = profile("ID=artix\nNAME=\"Artix Linux\"\n", InitSystem::OpenRc);
        assert_eq!(p.summary(), "Artix Linux / OpenRC");
        let p = profile("ID=artix\n", InitSystem::OpenRc);
        assert_eq!(p.summary(), "Artix Linux / OpenRC");
    }

    #[test]
    fn the_profile_serialises_for_logs_and_ipc() {
        let json = serde_json::to_string(&HostProfile::arch_systemd()).unwrap();
        assert!(json.contains("\"supported\":true"), "{json}");
        let back: HostProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(back, HostProfile::arch_systemd());
    }

    // ------------------------------------------------------------ live probe

    #[test]
    fn detection_never_panics_and_answers_the_same_twice() {
        let a = detect();
        let b = detect();
        assert_eq!(a, b, "host detection must be deterministic within a run");
        // Whatever this machine is, the answer must be self-consistent.
        assert_eq!(
            a.supported,
            a.distro == Distro::Arch && a.init == InitSystem::Systemd
        );
    }

    #[test]
    fn detection_starts_no_process_so_preview_stays_at_zero_spawns() {
        // The acceptance criterion for `--preview` is a spawn count of exactly
        // zero. Detection is on the preflight path of every run, so if it used
        // `inspect("cat", …)` this number would no longer be zero.
        let scope = crate::sys::PreviewScope::on();
        let _p = detect();
        assert_eq!(crate::sys::spawn_count(), 0);
        drop(scope);
    }
}
