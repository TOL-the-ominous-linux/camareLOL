//! Records which source tree the binary was actually built from.
//!
//! Without this, `--version` prints a version string that says nothing about
//! the code: every release since the first has claimed to be 0.1.0, so a user
//! could not tell whether they were running a fixed build. The commit, the tag
//! and whether the tree was dirty go into the binary itself.
//!
//! Building from a release tarball has no `.git` directory. That is not an
//! error: the fields fall back to "unknown" rather than failing the build.

use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    // A tarball extracted under a different uid than the one that cloned it
    // makes git refuse with "dubious ownership"; the safe.directory override
    // keeps the build working.
    let out = Command::new("git")
        .arg("-c")
        .arg("safe.directory=*")
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8(out.stdout).ok()?;
    let trimmed = text.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn emit(key: &str, value: &str) {
    println!("cargo:rustc-env={key}={value}");
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    emit(
        "CAMARELOL_GIT_SHA",
        &git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".into()),
    );
    emit(
        "CAMARELOL_GIT_TAG",
        &git(&["describe", "--tags", "--abbrev=0"]).unwrap_or_else(|| "unknown".into()),
    );
    // "yes" only when the tree actually had uncommitted changes.
    let dirty = git(&["status", "--porcelain"]).is_some();
    emit("CAMARELOL_GIT_DIRTY", if dirty { "yes" } else { "no" });

    // Reproducible builds prefer SOURCE_DATE_EPOCH (set by the CI tarball step).
    let built = std::env::var("SOURCE_DATE_EPOCH").ok();
    emit("CAMARELOL_BUILD_EPOCH", built.as_deref().unwrap_or("0"));
}
