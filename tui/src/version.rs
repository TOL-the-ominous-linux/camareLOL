//! What this binary is, precisely.
//!
//! Every release used to print the crate version, which stayed at 0.1.0 — so a
//! user who downloaded a fresh binary and one running a months-old build were
//! indistinguishable. An installer that erases disks should be able to say
//! exactly which commit it is before anyone trusts it with a disk.

/// The version this build should be called.
///
/// The git tag wins when there is one: `Cargo.toml` fell behind the tag once,
/// and a build that calls itself 0.1.5 while shipping as 0.1.6 is precisely
/// the kind of ambiguity this module exists to remove.
///
/// `Cargo.toml` now tracks the tag again as of 0.1.11. It had sat at 0.1.5
/// while releases went to 0.1.10, so `cargo install --git` produced a binary
/// whose own metadata said 0.1.5. Keeping the tag override costs nothing and
/// still covers a packaged build that never saw a git checkout.
pub const VERSION: &str = match option_env!("CAMARELOL_RELEASE_VERSION") {
    Some(v) => v,
    None => env!("CARGO_PKG_VERSION"),
};

/// Short commit hash, or "unknown" when built outside a git checkout.
pub fn git_sha() -> &'static str {
    env!("CAMARELOL_GIT_SHA")
}

/// Most recent tag, or "unknown".
pub fn git_tag() -> &'static str {
    env!("CAMARELOL_GIT_TAG")
}

/// Whether the source tree had uncommitted changes when this was built.
pub fn built_from_dirty_tree() -> bool {
    env!("CAMARELOL_GIT_DIRTY") == "yes"
}

/// One line for `--version`, e.g.
/// `camareLOL 0.1.5 (tag v0.1.5, commit 33af02c)`.
pub fn banner() -> String {
    let mut s = format!("camareLOL {VERSION}");
    if git_tag() != "unknown" {
        s.push_str(&format!(" (tag {}", git_tag()));
        if git_sha() != "unknown" {
            s.push_str(&format!(", commit {}", git_sha()));
        }
        s.push(')');
    } else if git_sha() != "unknown" {
        s.push_str(&format!(" (commit {})", git_sha()));
    }
    if built_from_dirty_tree() {
        s.push_str(" [built from a dirty working tree]");
    }
    s
}
