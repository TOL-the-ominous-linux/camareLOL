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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_banner_never_claims_a_version_it_cannot_support() {
        let b = banner();
        assert!(b.starts_with("camareLOL "));
        assert!(
            VERSION.chars().next().unwrap().is_ascii_digit(),
            "the version must start with a digit"
        );
        // A build with no git metadata must not print empty parentheses.
        if git_tag() == "unknown" && git_sha() == "unknown" {
            assert!(!b.contains("()"), "banner was {b:?}");
        }
    }

    #[test]
    fn the_banner_matches_the_version_it_reports() {
        assert!(banner().contains(VERSION));
    }

    #[test]
    fn the_version_looks_like_a_version() {
        // Three dotted numbers, or something is wrong with how it was derived.
        let parts: Vec<&str> = VERSION.split('.').collect();
        assert_eq!(parts.len(), 3, "version was {VERSION:?}");
        for p in parts {
            assert!(p.parse::<u32>().is_ok(), "version was {VERSION:?}");
        }
    }
}
