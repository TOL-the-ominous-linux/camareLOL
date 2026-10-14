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

    // A build script runs with the package directory as its working directory,
    // so files that belong to the workspace live one level up.
    let package = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let root = std::path::Path::new(&package)
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from(&package));
    println!(
        "cargo:rerun-if-changed={}",
        root.join("mirrors.toml").display()
    );

    // The tag is the version of record. Cargo.toml drifted from it once (a
    // release tagged 0.1.6 still printed 0.1.5), and a stale version is worse
    // than none: it is exactly what made two different builds look identical.
    let tag_version = git(&["describe", "--tags", "--abbrev=0"])
        .and_then(|t| t.strip_prefix('v').map(str::to_string))
        .filter(|v| {
            !v.is_empty()
                && v.split('.').count() == 3
                && v.split('.').all(|p| p.parse::<u32>().is_ok())
        });
    if let Some(v) = &tag_version {
        println!("cargo:rustc-env=CAMARELOL_RELEASE_VERSION={v}");
    }

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

    // Where the bundled updater looks. A distribution packaging camareLOL from
    // source sets this to its own mirror, or CAMARELOL_NO_UPDATE to switch the
    // updater off: its updates come from the package manager, and offering
    // binaries that manager never built would be wrong.
    // Without this, cargo reuses the cached build when only the environment
    // changed, and a distribution's setting silently has no effect.
    println!("cargo:rerun-if-env-changed=CAMARELOL_UPDATE_URL");
    println!("cargo:rerun-if-env-changed=CAMARELOL_NO_UPDATE");
    if let Ok(url) = std::env::var("CAMARELOL_UPDATE_URL") {
        if !url.trim().is_empty() {
            println!("cargo:rustc-env=CAMARELOL_UPDATE_URL={}", url.trim());
        }
    }
    if std::env::var("CAMARELOL_NO_UPDATE").is_ok() {
        println!("cargo:rustc-env=CAMARELOL_NO_UPDATE=1");
    }

    // The mirror list is embedded so adding one is a data change, not a code
    // change, and so a tampered copy on disk cannot redirect updates.
    let mirrors = std::fs::read_to_string(root.join("mirrors.toml")).unwrap_or_default();
    if !mirrors.trim().is_empty() {
        println!(
            "cargo:rustc-env=CAMARELOT_MIRRORS={}",
            mirrors.replace('\n', "\\n")
        );
    }

    // Reproducible builds prefer SOURCE_DATE_EPOCH (set by the CI tarball step).
    let built = std::env::var("SOURCE_DATE_EPOCH").ok();
    emit("CAMARELOL_BUILD_EPOCH", built.as_deref().unwrap_or("0"));
}
