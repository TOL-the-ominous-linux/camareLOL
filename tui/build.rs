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

/// Where the git directory is, if this is a git checkout at all.
///
/// A tarball has none, and that is fine: the identity fields fall back to
/// "unknown" rather than failing a build that has no git to consult.
fn git_dir(root: &std::path::Path) -> Option<std::path::PathBuf> {
    let dot = root.join(".git");
    if dot.is_file() {
        // A worktree or a submodule: .git holds "gitdir: <path>".
        let text = std::fs::read_to_string(&dot).ok()?;
        let rest = text.trim().strip_prefix("gitdir:")?.trim();
        let path = std::path::PathBuf::from(rest);
        return Some(if path.is_absolute() {
            path
        } else {
            root.join(path)
        });
    }
    dot.is_dir().then_some(dot)
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

    // The git state is read below and baked into the binary, so a change to it
    // has to rebuild. Without this, checking out another tag leaves cargo
    // thinking the crate is up to date and hands back the previous binary —
    // still carrying the previous tag and commit. That is the exact failure
    // this file exists to prevent: a release named 0.1.10 that reports itself
    // as 0.1.9, built from code that was never tagged.
    if let Some(git_dir) = git_dir(&root) {
        for watched in ["HEAD", "packed-refs", "refs/tags"] {
            let path = git_dir.join(watched);
            if path.exists() {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }

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
    // "yes" only when the tree actually had uncommitted changes to tracked files.
    //
    // Untracked files are excluded on purpose. A build leaves output behind,
    // and an older tag's .gitignore may not cover the directory this version
    // writes to — so building v0.1.9 reported a dirty tree purely because of
    // this release's out/. Nothing untracked reaches the binary: everything
    // compiled in is a named file, and every one of them is tracked and
    let dirty = git(&["status", "--porcelain", "--untracked-files=no"]).is_some();
    emit("CAMARELOL_GIT_DIRTY", if dirty { "yes" } else { "no" });

    // Reproducible builds prefer SOURCE_DATE_EPOCH (set by the CI tarball step).
    let built = std::env::var("SOURCE_DATE_EPOCH").ok();
    emit("CAMARELOL_BUILD_EPOCH", built.as_deref().unwrap_or("0"));
}
