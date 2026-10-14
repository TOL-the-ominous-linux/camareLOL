//! Keeping the installer itself up to date.
//!
//! # Why this is deliberately timid
//!
//! camareLOL erases disks. An updater that silently replaces this binary is a
//! remote-code-execution path straight to data destruction: whoever controls
//! the response controls what runs with your disk and your `root`. So every
//! decision here is biased towards *not* acting:
//!
//! - Checking is the default. Installing requires an explicit opt-in flag.
//! - Nothing is downloaded or written while an install is running.
//! - The candidate's SHA-256 must match the manifest before it is allowed near
//!   the real binary, and the candidate must actually start (`--version`) or it
//!   is thrown away.
//! - The previous binary is kept as `<name>.prev`, so a bad update is undone
//!   with [`rollback`].
//!
//! A checksum detects corruption and a truncated download. It does **not**
//! detect a malicious server or a hijacked DNS: both sides of the comparison
//! would come from the attacker. Signing the manifest (minisign/GPG) is the
//! fix for that and is not implemented yet — [`plan`] says so out loud rather
//! than implying a guarantee it cannot make.

use crate::error::{CamarelolError, ErrorCode, Result};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::process::Stdio;

/// Where the release index lives. Overridable so a mirror, a local test server
/// or an air-gapped site can be used instead.
pub const DEFAULT_MANIFEST_URL: &str = "https://tamkogay9876-code.github.io/camareLOL/latest.json";

/// Bumped when the manifest layout changes incompatibly.
pub const MANIFEST_SCHEMA: u32 = 1;

/// A published release, as described by the index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Release {
    pub schema: u32,
    pub version: String,
    pub url: String,
    /// Lowercase hex SHA-256 of the file at `url`.
    pub sha256: String,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// Parse and sanity-check the index document.
///
/// Every field is attacker-controlled, so each one is checked here rather than
/// at the point of use: a manifest must not be able to talk us into fetching
/// `file:///etc/shadow`, writing outside the target directory, or skipping the
/// integrity check.
pub fn parse_manifest(raw: &str) -> Result<Release> {
    let rel: Release = serde_json::from_str(raw).map_err(|e| {
        CamarelolError::new(
            ErrorCode::UpdateManifestInvalid,
            "the update index is not valid JSON",
        )
        .with_context(e.to_string())
        .with_remediation("check the manifest URL, or turn updates off")
    })?;

    if rel.schema != MANIFEST_SCHEMA {
        return Err(CamarelolError::new(
            ErrorCode::UpdateManifestInvalid,
            format!(
                "update index uses schema {} but this build understands {MANIFEST_SCHEMA}",
                rel.schema
            ),
        )
        .with_remediation("update camareLOL, or publish an index with the current schema"));
    }

    if rel.version.trim().is_empty() {
        return Err(CamarelolError::new(
            ErrorCode::UpdateManifestInvalid,
            "the update index has an empty version",
        ));
    }

    if !url_is_acceptable(&rel.url) {
        // Plain HTTP would let anyone on the path swap the payload. A checksum
        // fetched over the same channel does not save us, so refuse outright.
        return Err(CamarelolError::new(
            ErrorCode::UpdateUrlRefused,
            format!(
                "refusing to download an update over a non-HTTPS URL: {}",
                rel.url
            ),
        )
        .with_remediation("publish the release over https:// and update the index"));
    }

    if rel.sha256.len() != 64 || !rel.sha256.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(CamarelolError::new(
            ErrorCode::UpdateManifestInvalid,
            "the update index has no usable SHA-256 to check the download against",
        )
        .with_remediation(
            "an update that cannot be verified will not be installed; publish a sha256 field",
        ));
    }

    Ok(rel)
}

/// Whether a URL may be fetched.
///
/// HTTPS everywhere except the loopback interface: a local mirror is how the
/// updater gets tested end to end, and plaintext to `127.0.0.1` does not cross
/// a network an attacker could sit on. Anything that leaves the machine must be
/// HTTPS.
pub fn url_is_acceptable(url: &str) -> bool {
    if url.starts_with("https://") {
        return true;
    }
    for host in ["http://127.0.0.1", "http://localhost", "http://[::1]"] {
        if let Some(rest) = url.strip_prefix(host) {
            // Must be host[:port]/... — not `http://localhost.evil.example`.
            return rest.is_empty()
                || rest.starts_with('/')
                || rest.starts_with(':')
                || rest.starts_with('?');
        }
    }
    false
}

/// Compare two `MAJOR.MINOR.PATCH` versions.
///
/// Suffixes after a `-` are ignored: `1.2.0-rc1` sorts as `1.2.0` rather than
/// being rejected, because refusing to understand a version string should not
/// mean refusing to tell the user a newer one exists.
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let parts = |v: &str| -> Vec<u64> {
        let core = v.split(['-', '+']).next().unwrap_or("");
        core.split('.')
            .map(|p| p.parse::<u64>().unwrap_or(0))
            .collect()
    };
    let (pa, pb) = (parts(a), parts(b));
    for i in 0..pa.len().max(pb.len()) {
        match pa.get(i).unwrap_or(&0).cmp(pb.get(i).unwrap_or(&0)) {
            Ordering::Equal => continue,
            other => return other,
        }
    }
    Ordering::Equal
}

/// Why we are not replacing the binary, even though a newer one exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Block {
    /// An install is running. Swapping the binary mid-install could leave a
    /// half-written system that the new code no longer understands.
    InstallRunning,
    /// Replacing `/usr/bin/camarelol` needs privileges we do not have.
    NotPrivileged,
    /// The update would replace a binary that is not the one we are running.
    TargetMismatch,
}

impl Block {
    pub fn as_str(self) -> &'static str {
        match self {
            Block::InstallRunning => "an installation is running",
            Block::NotPrivileged => "the current binary is not writable by this user",
            Block::TargetMismatch => "the update would not replace the running binary",
        }
    }
}

/// What to do about the release in the index.
#[derive(Debug, Clone, PartialEq)]
pub enum Plan {
    /// Already running the newest published version.
    UpToDate,
    /// Newer version available. Nothing has been downloaded or changed.
    Available(Release),
    /// Newer version available and the user asked us to install it.
    Install(Release),
    /// Newer version available, but replacing the binary right now is unsafe.
    Blocked { release: Release, reason: Block },
}

/// The situation the updater finds itself in.
pub struct Request<'a> {
    pub current_version: &'a str,
    pub release: &'a Release,
    /// The user passed the flag that allows replacing the binary.
    pub opted_in: bool,
    /// An install is in progress right now.
    pub install_running: bool,
    /// We can write to the binary we are running.
    pub can_write_target: bool,
}

/// Decide what to do. Pure: no network, no filesystem.
pub fn plan(req: &Request<'_>) -> Plan {
    if compare_versions(&req.release.version, req.current_version) != Ordering::Greater {
        return Plan::UpToDate;
    }
    let release = req.release.clone();
    if req.install_running {
        return Plan::Blocked {
            release,
            reason: Block::InstallRunning,
        };
    }
    if !req.can_write_target {
        return Plan::Blocked {
            release,
            reason: Block::NotPrivileged,
        };
    }
    if req.opted_in {
        Plan::Install(release)
    } else {
        Plan::Available(release)
    }
}

/// Check downloaded bytes against the manifest's checksum.
pub fn verify(bytes: &[u8], expected: &str) -> Result<()> {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    let actual = digest
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    if !actual.eq_ignore_ascii_case(expected.trim()) {
        return Err(CamarelolError::new(
            ErrorCode::UpdateChecksumMismatch,
            "the downloaded update does not match its checksum",
        )
        .with_context(format!("expected {expected}, got {actual}"))
        .with_remediation("the download was corrupted or altered; nothing was installed"));
    }
    Ok(())
}

/// Fetch the index and decide what to do.
pub fn check(
    manifest_url: &str,
    current_version: &str,
    opted_in: bool,
    install_running: bool,
    can_write_target: bool,
) -> Result<Plan> {
    let raw = fetch_text(manifest_url)?;
    let release = parse_manifest(&raw)?;
    Ok(plan(&Request {
        current_version,
        release: &release,
        opted_in,
        install_running,
        can_write_target,
    }))
}

/// GET a small text document with a hard size limit and a short timeout.
fn fetch_text(url: &str) -> Result<String> {
    let out = sys::download(url, None, 256 * 1024)?;
    Ok(out)
}

mod sys {
    use crate::error::{CamarelolError, ErrorCode, Result};
    use std::path::Path;
    use std::process::Stdio;
    use std::time::Duration;

    /// Download `url` to a temporary file and return its contents.
    ///
    /// `max_bytes` is enforced by curl so a hostile server cannot make us fill
    /// the disk. Only HTTPS is permitted, and only to the host named in the
    /// URL — a redirect elsewhere is refused rather than followed, because the
    /// manifest's checksum arrives over the same trust boundary as the payload.
    pub fn download(url: &str, dest: Option<&Path>, max_bytes: u64) -> Result<String> {
        if !super::url_is_acceptable(url) {
            return Err(CamarelolError::new(
                ErrorCode::UpdateUrlRefused,
                format!("refusing to download from a non-HTTPS URL: {url}"),
            ));
        }

        let path = match dest {
            Some(p) => p.to_path_buf(),
            None => std::env::temp_dir().join(format!("camarelol-dl-{}", std::process::id())),
        };

        let size_limit = format!("{max_bytes}");
        // curl gets its own allowlist, derived from the URL we already vetted.
        // Anything off-loopback is https only; loopback may use http so the
        // updater can be tested against a local mirror. Redirects stay https
        // so a loopback index cannot bounce us onto the open network.
        let allowed_proto = if url.starts_with("https://") {
            "=https"
        } else {
            "=http,https"
        };
        let mut cmd = std::process::Command::new("curl");
        cmd.args([
            "--silent",
            "--show-error",
            "--fail",
            "--proto",
            allowed_proto,
            "--proto-redir",
            "=https",
            "--max-filesize",
            &size_limit,
            "--max-time",
            "30",
            "--retry",
            "2",
            "-o",
            path.to_str().unwrap_or_default(),
            url,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
        crate::sys::run_command(cmd, Duration::from_secs(90)).map_err(|e| {
            CamarelolError::new(
                ErrorCode::UpdateDownloadFailed,
                "could not download the update",
            )
            .with_context(e.to_string())
            .with_remediation("check the network, then try again")
        })?;

        let body = std::fs::read_to_string(&path).unwrap_or_default();
        if dest.is_none() {
            let _ = std::fs::remove_file(&path);
        }
        Ok(body)
    }
}

/// Download a release and verify it without installing anything.
pub fn fetch_candidate(release: &Release, dest: &Path) -> Result<Vec<u8>> {
    sys::download(&release.url, Some(dest), 256 * 1024 * 1024)?;
    let bytes = std::fs::read(dest).map_err(|e| {
        CamarelolError::new(ErrorCode::UpdateDownloadFailed, "the update did not arrive")
            .with_context(e.to_string())
    })?;
    verify(&bytes, &release.sha256)?;
    // curl writes 0644. The candidate has to be executable before it can be
    // run as a smoke test, and running it before installing it is the whole
    // point of the test.
    set_mode(dest, 0o755)?;
    Ok(bytes)
}

/// Replace `target` with `candidate`, keeping the old binary for [`rollback`].
///
/// The candidate is executed first. A binary that cannot even print its own
/// version is not installed, whatever its checksum says.
pub fn install_candidate(target: &Path, candidate: &Path) -> Result<()> {
    if !target.exists() {
        return Err(CamarelolError::new(
            ErrorCode::UpdateNotPrivileged,
            "the running binary is not where the update expects it",
        ));
    }

    let probe = std::process::Command::new(candidate)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| {
            CamarelolError::new(
                ErrorCode::UpdateCandidateFailed,
                "the downloaded update would not run",
            )
            .with_context(e.to_string())
            .with_remediation("nothing was installed; the old binary is untouched")
        })?;
    if !probe.success() {
        return Err(CamarelolError::new(
            ErrorCode::UpdateCandidateFailed,
            "the downloaded update failed its own --version check",
        )
        .with_remediation("nothing was installed; the old binary is untouched"));
    }

    // Keep the old binary: a working rollback beats a clean directory.
    let backup = backup_path(target);
    let _ = std::fs::remove_file(&backup);
    std::fs::copy(target, &backup)?;

    replace_binary(target, candidate, ErrorCode::UpdateCandidateFailed)
}

/// Put `source` in place of `target`, keeping `target`'s permissions.
///
/// Copying straight over the target fails with ETXTBSY: this very binary is
/// the one running. Writing a sibling and renaming over it sidesteps that —
/// rename swaps the directory entry, and the running process keeps its own
/// inode until it exits. The switch is therefore atomic: there is no instant
/// where `target` is half-written or missing.
fn replace_binary(target: &Path, source: &Path, code: ErrorCode) -> Result<()> {
    let staged = staged_path(target);
    let mode = file_mode(target);
    std::fs::copy(source, &staged).map_err(|e| {
        CamarelolError::new(
            code,
            format!(
                "could not stage the replacement next to {}",
                target.display()
            ),
        )
        .with_context(e.to_string())
    })?;
    set_mode(&staged, mode)?;
    std::fs::rename(&staged, target).map_err(|e| {
        let _ = std::fs::remove_file(&staged);
        CamarelolError::new(code, "could not replace the running binary")
            .with_context(e.to_string())
            .with_remediation(format!(
                "the directory holding {} may not be writable; nothing was changed",
                target.display()
            ))
    })?;
    Ok(())
}

fn staged_path(target: &Path) -> PathBuf {
    let mut name = target.file_name().unwrap_or_default().to_os_string();
    name.push(".new");
    target.with_file_name(name)
}

/// The permission bits of `path`, so a replaced binary keeps its old mode.
#[cfg(unix)]
fn file_mode(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.permissions().mode())
        .unwrap_or(0o755)
}

#[cfg(not(unix))]
fn file_mode(_path: &Path) -> u32 {
    0o755
}

fn backup_path(target: &Path) -> PathBuf {
    let mut name = target.file_name().unwrap_or_default().to_os_string();
    name.push(".prev");
    target.with_file_name(name)
}

/// Put the previous binary back.
pub fn rollback(target: &Path) -> Result<()> {
    let backup = backup_path(target);
    if !backup.exists() {
        return Err(CamarelolError::new(
            ErrorCode::UpdateRollbackFailed,
            "there is no previous binary to restore",
        ));
    }
    let probe = std::process::Command::new(&backup)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .status();
    if !matches!(probe, Ok(st) if st.success()) {
        return Err(CamarelolError::new(
            ErrorCode::UpdateRollbackFailed,
            "the saved binary does not run either",
        )
        .with_remediation("the current binary was left in place"));
    }
    replace_binary(target, &backup, ErrorCode::UpdateRollbackFailed)
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(version: &str) -> Release {
        Release {
            schema: MANIFEST_SCHEMA,
            version: version.to_string(),
            url: "https://example.invalid/camarelol".into(),
            sha256: "a".repeat(64),
            size: Some(10),
            notes: None,
        }
    }

    fn request<'a>(rel: &'a Release, current: &'a str, opted_in: bool) -> Request<'a> {
        Request {
            current_version: current,
            release: rel,
            opted_in,
            install_running: false,
            can_write_target: true,
        }
    }

    // ---------------------------------------------------------- versioning

    #[test]
    fn versions_sort_numerically_not_alphabetically() {
        assert_eq!(compare_versions("0.2.0", "0.1.9"), Ordering::Greater);
        assert_eq!(compare_versions("0.1.10", "0.1.9"), Ordering::Greater);
        assert_eq!(compare_versions("0.1.0", "0.1.0"), Ordering::Equal);
        assert_eq!(compare_versions("0.1.0", "0.2.0"), Ordering::Less);
    }

    #[test]
    fn pre_release_suffixes_do_not_break_comparison() {
        assert_eq!(compare_versions("1.2.0-rc1", "1.2.0"), Ordering::Equal);
        assert_eq!(compare_versions("1.2.1", "1.2.0-rc1"), Ordering::Greater);
    }

    #[test]
    fn different_lengths_of_version_are_comparable() {
        assert_eq!(compare_versions("1.2", "1.2.0"), Ordering::Equal);
        assert_eq!(compare_versions("1.2.1", "1.2"), Ordering::Greater);
    }

    // ------------------------------------------------------------- manifest

    #[test]
    fn a_well_formed_manifest_parses() {
        let raw = format!(
            r#"{{"schema":{},"version":"0.2.0","url":"https://example.invalid/camarelol","sha256":"{}"}}"#,
            MANIFEST_SCHEMA,
            "b".repeat(64)
        );
        let r = parse_manifest(&raw).unwrap();
        assert_eq!(r.version, "0.2.0");
    }

    #[test]
    fn only_loopback_may_be_plaintext() {
        assert!(url_is_acceptable("https://example.invalid/x"));
        assert!(url_is_acceptable("http://127.0.0.1:8765/latest.json"));
        assert!(url_is_acceptable("http://localhost:8765/latest.json"));
        assert!(!url_is_acceptable("http://example.invalid/x"));
        // A host that merely starts with "localhost" is not loopback.
        assert!(!url_is_acceptable("http://localhost.evil.example/x"));
        assert!(!url_is_acceptable("ftp://example.invalid/x"));
        assert!(!url_is_acceptable("file:///etc/shadow"));
    }

    #[test]
    fn a_plaintext_url_is_refused() {
        let raw = format!(
            r#"{{"schema":{},"version":"9.9.9","url":"http://evil.invalid/camarelol","sha256":"{}"}}"#,
            MANIFEST_SCHEMA,
            "b".repeat(64)
        );
        let err = parse_manifest(&raw).unwrap_err();
        assert_eq!(err.code, ErrorCode::UpdateUrlRefused);
    }

    #[test]
    fn a_manifest_without_a_usable_checksum_is_refused() {
        let raw = format!(
            r#"{{"schema":{},"version":"9.9.9","url":"https://example.invalid/x","sha256":"nope"}}"#,
            MANIFEST_SCHEMA
        );
        let err = parse_manifest(&raw).unwrap_err();
        assert_eq!(err.code, ErrorCode::UpdateManifestInvalid);
    }

    #[test]
    fn a_future_schema_is_refused_rather_than_guessed() {
        let raw = format!(
            r#"{{"schema":99,"version":"9.9.9","url":"https://example.invalid/x","sha256":"{}"}}"#,
            "b".repeat(64)
        );
        assert_eq!(
            parse_manifest(&raw).unwrap_err().code,
            ErrorCode::UpdateManifestInvalid
        );
    }

    #[test]
    fn garbage_is_rejected_with_a_readable_error() {
        assert_eq!(
            parse_manifest("not json at all").unwrap_err().code,
            ErrorCode::UpdateManifestInvalid
        );
    }

    // ----------------------------------------------------------------- plan

    #[test]
    fn nothing_happens_when_the_build_is_current() {
        let r = release("0.1.5");
        assert_eq!(plan(&request(&r, "0.1.5", true)), Plan::UpToDate);
        assert_eq!(plan(&request(&r, "0.2.0", true)), Plan::UpToDate);
    }

    #[test]
    fn a_newer_release_is_only_reported_unless_the_user_opted_in() {
        let r = release("0.2.0");
        match plan(&request(&r, "0.1.5", false)) {
            Plan::Available(got) => assert_eq!(got.version, "0.2.0"),
            other => panic!("expected Available, got {other:?}"),
        }
        match plan(&request(&r, "0.1.5", true)) {
            Plan::Install(got) => assert_eq!(got.version, "0.2.0"),
            other => panic!("expected Install, got {other:?}"),
        }
    }

    #[test]
    fn the_binary_is_never_replaced_while_an_install_runs() {
        // Not even with the opt-in: a half-written system plus a swapped
        // installer is the worst possible combination.
        let r = release("0.2.0");
        let mut req = request(&r, "0.1.5", true);
        req.install_running = true;
        assert_eq!(
            plan(&req),
            Plan::Blocked {
                release: r,
                reason: Block::InstallRunning
            }
        );
    }

    #[test]
    fn without_write_access_we_report_instead_of_trying() {
        let r = release("0.2.0");
        let mut req = request(&r, "0.1.5", true);
        req.can_write_target = false;
        assert_eq!(
            plan(&req),
            Plan::Blocked {
                release: r,
                reason: Block::NotPrivileged
            }
        );
    }

    // -------------------------------------------------------------- checksum

    #[test]
    fn a_matching_checksum_passes() {
        // SHA-256("abc")
        let expected = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        verify(b"abc", expected).unwrap();
    }

    #[test]
    fn a_mismatched_checksum_stops_everything() {
        let err = verify(b"abc", &"0".repeat(64)).unwrap_err();
        assert_eq!(err.code, ErrorCode::UpdateChecksumMismatch);
    }

    #[test]
    fn checksums_compare_case_insensitively() {
        verify(
            b"abc",
            "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD",
        )
        .unwrap();
    }

    // ------------------------------------------------------------- installing

    #[test]
    fn a_candidate_that_cannot_run_is_never_installed() {
        let dir = std::env::temp_dir().join(format!("camarelol-upd-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let target = dir.join("camarelol");
        let candidate = dir.join("candidate");
        std::fs::write(&target, "#!/bin/sh\nexit 0\n").unwrap();
        set_mode(&target, 0o755).unwrap();
        // Exits non-zero for --version: a broken build.
        std::fs::write(&candidate, "#!/bin/sh\nexit 3\n").unwrap();
        set_mode(&candidate, 0o755).unwrap();

        let err = install_candidate(&target, &candidate).unwrap_err();
        assert_eq!(err.code, ErrorCode::UpdateCandidateFailed);
        // The target is untouched: no half-installed state.
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            "#!/bin/sh\nexit 0\n"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_working_candidate_replaces_the_binary_and_leaves_a_rollback() {
        let dir = std::env::temp_dir().join(format!("camarelol-upd2-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let target = dir.join("camarelol");
        let candidate = dir.join("candidate");
        std::fs::write(&target, "#!/bin/sh\nexit 0\n").unwrap();
        set_mode(&target, 0o755).unwrap();
        // Succeeds for --version, and prints something recognisable.
        std::fs::write(
            &candidate,
            "#!/bin/sh\n[ \"$1\" = \"--version\" ] && echo camareLOL 9.9.9\nexit 0\n",
        )
        .unwrap();
        set_mode(&candidate, 0o755).unwrap();

        install_candidate(&target, &candidate).unwrap();
        assert!(std::fs::read_to_string(&target).unwrap().contains("9.9.9"));
        assert!(backup_path(&target).exists(), "a rollback copy must exist");

        // And the rollback puts the old one back.
        rollback(&target).unwrap();
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            "#!/bin/sh\nexit 0\n"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_staged_file_lands_next_to_the_target() {
        assert_eq!(
            staged_path(Path::new("/usr/bin/camarelol")),
            PathBuf::from("/usr/bin/camarelol.new")
        );
    }

    #[test]
    fn the_installed_binary_keeps_the_mode_of_the_one_it_replaced() {
        let dir = std::env::temp_dir().join(format!("camarelol-upd3-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let target = dir.join("camarelol");
        let candidate = dir.join("candidate");
        std::fs::write(&target, "#!/bin/sh\nexit 0\n").unwrap();
        set_mode(&target, 0o755).unwrap();
        std::fs::write(&candidate, "#!/bin/sh\nexit 0\n").unwrap();
        set_mode(&candidate, 0o700).unwrap();

        install_candidate(&target, &candidate).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&target).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o755, "the new binary must not end up 0700");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_backup_lands_next_to_the_target_with_a_predictable_name() {
        assert_eq!(
            backup_path(Path::new("/usr/bin/camarelol")),
            PathBuf::from("/usr/bin/camarelol.prev")
        );
    }

    #[test]
    fn rolling_back_without_a_backup_says_so() {
        let err = rollback(Path::new("/nonexistent/camarelol")).unwrap_err();
        assert_eq!(err.code, ErrorCode::UpdateRollbackFailed);
    }
}
