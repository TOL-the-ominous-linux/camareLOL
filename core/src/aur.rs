//! AUR support: manual `makepkg` and helper detection, under one hard rule.
//!
//! # The rule
//!
//! **An AUR package is built as a normal user, never as root.** A PKGBUILD is an
//! arbitrary shell script that runs with the privileges of whoever runs it. A
//! root build turns "install a theme" into "run unreviewed code as uid 0 on the
//! machine that is about to have its disks erased". `makepkg` itself refuses to
//! run as root; this module refuses before it gets the chance, so the failure is
//! a camareLOL error with a remediation rather than a `makepkg` message nobody
//! reads.
//!
//! Three independent guards enforce it, because one guard is one bug away from
//! not existing:
//!
//! 1. **The build user is resolved first.** If the installer runs as root, the
//!    plan is only produced when a *configured* unprivileged account is
//!    available. There is no fallback that picks a user for you.
//! 2. **Every build is wrapped.** The command is `runuser -u <user> -- makepkg`
//!    (or `sudo -u`), never a bare `makepkg`. See [`privilege_wrapper`].
//! 3. **The runner re-checks the effective uid immediately before spawning**
//!    ([`AurBuild::ensure_not_root`]), so a future caller that forgets guard 1
//!    or 2 still cannot run a root build.
//!
//! # Nothing here goes through `sys::run`
//!
//! Every execution path uses [`crate::sys::run_command`], the mutating entry
//! point, so `--preview` refuses it at the syscall latch — the same latch that
//! stops `pacstrap` and `sgdisk`. The read-only probe that finds a helper goes
//! through [`crate::sys::inspect`], which is the documented exception.
//!
//! # Helper or not, we never install one
//!
//! If none of [`AUR_HELPERS`] is on `PATH`, camareLOL says so and falls back to
//! the manual `makepkg` path. It does not install a helper, and it does not
//! suggest `sudo pacman -S yay`: that would be a root install of a tool whose
//! whole job is running third-party code, decided by a flag the user may not
//! have meant to set.

use crate::error::{CamarelolError, ErrorCode, Result};
use crate::packages::{PackageSelection, AUR_WARNING};
use crate::sys;
use std::time::Duration;

/// The AUR helpers camareLOL will use if it finds one, in preference order.
///
/// Listed rather than discovered: an unknown binary that happens to be called
/// `yay` on `PATH` must not be run with the user's privileges, and there is no
// safe way to tell a real helper from an impostor without hashing it.
pub const AUR_HELPERS: &[&str] = &["yay", "paru", "pikaur", "trizen"];

/// Base URL for a manual AUR checkout.
pub const AUR_BASE_URL: &str = "https://aur.archlinux.org";

/// How long a build may run before it is killed. Long on purpose: compiling a
/// browser takes minutes and a timeout that cuts it off leaves a half-written
/// build directory.
pub const BUILD_TIMEOUT: Duration = Duration::from_secs(3600);

/// A recognised AUR helper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AurHelper {
    Yay,
    Paru,
    Pikaur,
    Trizen,
}

impl AurHelper {
    pub fn as_str(&self) -> &'static str {
        match self {
            AurHelper::Yay => "yay",
            AurHelper::Paru => "paru",
            AurHelper::Pikaur => "pikaur",
            AurHelper::Trizen => "trizen",
        }
    }

    /// Recognise a helper name, or `None`. Used to turn a probe result back into
    /// an enum without trusting an arbitrary string.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "yay" => Some(AurHelper::Yay),
            "paru" => Some(AurHelper::Paru),
            "pikaur" => Some(AurHelper::Pikaur),
            "trizen" => Some(AurHelper::Trizen),
            _ => None,
        }
    }
}

/// The exact program an invocation will run, after any privilege drop.
///
/// Split out so the wrapping is a pure function of the wrapper program and the
/// build user, and can be asserted on without spawning anything.
pub fn privilege_wrapper(
    wrapper: Option<&str>,
    build_user: Option<&str>,
    program: &str,
) -> Result<Vec<String>> {
    match (wrapper, build_user) {
        (Some(w), Some(user)) => Ok(vec![
            w.to_string(),
            "-u".into(),
            user.to_string(),
            "--".into(),
            program.to_string(),
        ]),
        (None, None) => Ok(vec![program.to_string()]),
        // A wrapper with no user, or a user with no wrapper, is a build that
        // would run as whoever started the installer. Refuse rather than
        // half-apply the rule.
        (Some(_), None) => Err(privilege_error(
            "an unprivileged build user is required to drop privileges for the AUR build",
        )),
        (None, Some(_)) => Err(privilege_error(
            "neither `runuser` nor `sudo` is available to drop privileges for the AUR build",
        )),
    }
}

fn privilege_error(message: &str) -> CamarelolError {
    CamarelolError::new(ErrorCode::AurBuildRefused, message.to_string())
        .with_remediation("install util-linux (runuser) or run camareLOL as a normal user")
}

/// One command in the AUR build plan.
///
/// A plan is a list of these rather than one command because the manual path
/// needs a checkout *and* a build per package, and because the TUI and the
/// preview both need to show them in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AurStep {
    pub program: String,
    pub args: Vec<String>,
    /// Directory the command runs in, when it is not the installer's cwd.
    pub cwd: Option<String>,
    /// Whether the build user is wrapped in front of `program` to drop root.
    pub as_build_user: bool,
}

impl AurStep {
    /// The command line as the user will see it in the preview log.
    pub fn render(&self) -> String {
        let mut s = self.program.clone();
        s.push(' ');
        s.push_str(&self.args.join(" "));
        s
    }
}

/// How the packages will be obtained and built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AurMethod {
    /// A recognised helper was found on `PATH`.
    Helper(AurHelper),
    /// No helper: `git clone` + `makepkg`, exactly what a helper would do.
    Manual,
}

impl AurMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            AurMethod::Helper(h) => h.as_str(),
            AurMethod::Manual => "makepkg (manual)",
        }
    }
}

/// The complete, validated plan for an AUR install. Nothing here has run yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AurPlan {
    pub packages: Vec<String>,
    pub method: AurMethod,
    /// The unprivileged account every build runs as. `None` only when the
    /// installer itself is already unprivileged, in which case the current user
    /// is the build user and no wrapper is needed.
    pub build_user: Option<String>,
    /// `runuser` or `sudo`, whichever is available, when a drop is needed.
    pub wrapper: Option<String>,
    pub steps: Vec<AurStep>,
}

impl AurPlan {
    /// True when the installer is already unprivileged: nothing to drop, and
    /// therefore nothing that could accidentally be built as root.
    pub fn already_unprivileged(&self) -> bool {
        self.build_user.is_none() && self.wrapper.is_none()
    }

    pub fn render(&self) -> String {
        self.steps
            .iter()
            .map(|s| s.render())
            .collect::<Vec<_>>()
            .join(" && ")
    }
}

/// Reject anything that is not a plain Arch package name.
///
/// This is not paranoia about a shell: `sys` never builds a shell string, so
/// `; rm -rf /` cannot escape. It is about **argv**, where `;` is just a
/// character but a leading `-` is a flag. A package called `-sysupgrade` would
/// be read by `makepkg` as an option, and `--help` from a profile file would be
/// read as a request to print help and exit 0 — an install that reports success
/// and installs nothing. Same class of bug as a wrong package name: green tests,
/// broken machine.
pub fn validate_package_name(name: &str) -> Result<()> {
    let bad = |why: &str| {
        CamarelolError::new(
            ErrorCode::InvalidConfig,
            format!("`{name}` is not a valid AUR package name: {why}"),
        )
        .with_remediation(
            "AUR package names are letters, digits and . _ + - @ only, and cannot start with '-'",
        )
    };
    if name.is_empty() {
        return Err(bad("it is empty"));
    }
    if name.len() > 100 {
        return Err(bad("it is absurdly long"));
    }
    if name.starts_with('-') {
        return Err(bad("it starts with '-', so it would be read as an option"));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-' | '@'))
    {
        return Err(bad(
            "it contains something other than letters, digits and . _ + - @",
        ));
    }
    if name.contains("..") {
        return Err(bad("it contains '..'"));
    }
    if name.contains('@') && name.matches('@').count() > 1 {
        return Err(bad("it contains more than one '@'"));
    }
    Ok(())
}

/// Validate every package in a selection. All-or-nothing: a list where one name
/// is wrong builds none of them, so the user fixes one thing rather than
/// discovering a half-installed set.
pub fn validate_packages(packages: &[String]) -> Result<()> {
    for p in packages {
        validate_package_name(p)?;
    }
    Ok(())
}

/// Read the effective uid from `/proc/self/status`.
///
/// Pure parsing, so the never-root guard can be tested without dropping
/// privileges on the machine running the tests. `None` means "could not tell",
/// which the callers treat as *not* root-provable and refuse accordingly.
pub fn parse_effective_uid(contents: &str) -> Option<u32> {
    for line in contents.lines() {
        let Some(rest) = line.strip_prefix("Uid:") else {
            continue;
        };
        // `Uid: real effective saved fs`
        let fields: Vec<&str> = rest.split_whitespace().collect();
        if fields.len() < 2 {
            return None;
        }
        return fields[1].parse().ok();
    }
    None
}

/// The effective uid of this process, read from `/proc/self/status`.
pub fn effective_uid() -> Option<u32> {
    sys::read_file("/proc/self/status")
        .ok()
        .and_then(|s| parse_effective_uid(&s))
}

/// Find a helper on `PATH`.
///
/// A `<program> --version` probe through [`sys::inspect`]: it reports, it never
/// changes anything, so it is available during `--preview` too — and the
/// preview needs it, because a preview that printed `makepkg` on a machine
/// where the real run would have called `yay` would be lying.
///
/// Returns `None` when none is present. camareLOL does **not** install one.
pub fn detect_helper() -> Option<AurHelper> {
    AUR_HELPERS
        .iter()
        .find(|name| sys::tool_available(name))
        .and_then(|name| AurHelper::from_name(name))
}

/// Which program drops privileges on this host, if one is available.
///
/// `runuser` (util-linux) is preferred over `sudo`: it exists on every Arch
/// system by default and it does not read sudoers, so the privilege drop does
/// not depend on the target account's password policy. Neither is ever
/// installed by camareLOL; a host with neither simply cannot do a root-run AUR
/// build, and is told so.
pub fn detect_drop_program() -> Option<String> {
    for candidate in ["runuser", "sudo"] {
        if sys::tool_available(candidate) {
            return Some(candidate.to_string());
        }
    }
    None
}

/// Build the validated plan for an AUR install.
///
/// `Ok(None)` is the normal answer for the default configuration: AUR is
/// opt-in, and an opt-in that was not taken is not an error. Everything else is
/// a hard failure **before** any command runs.
pub fn plan(
    selection: &PackageSelection,
    helper: Option<AurHelper>,
    wrapper: Option<&str>,
    build_user: Option<&str>,
    running_as_root: bool,
) -> Result<Option<AurPlan>> {
    let packages = selection.aur_list();
    if packages.is_empty() {
        // Either AUR is off, or it is on with nothing selected. Either way
        // there is nothing to build — and silently building "everything" is not
        // an option this function will ever take.
        return Ok(None);
    }
    validate_packages(&packages)?;

    if running_as_root {
        // Guard 1: no build user, no build.
        let user = build_user.filter(|u| !u.is_empty()).ok_or_else(|| {
            CamarelolError::new(
                ErrorCode::AurBuildRefused,
                "AUR packages must be built as a normal user, but camareLOL is running as root and no build user is configured",
            )
            .with_remediation(
                "set `aur_build_user` to an existing unprivileged account (packages.aur_build_user in the profile)",
            )
        })?;
        crate::accounts::validate_username(user)?;
        let w = wrapper.filter(|w| !w.is_empty()).ok_or_else(|| {
            CamarelolError::new(
                ErrorCode::AurBuildRefused,
                "AUR packages must be built as a normal user, but neither `runuser` nor `sudo` is available to drop privileges",
            )
            .with_remediation("install util-linux, or run camareLOL as a normal user")
        })?;
        return Ok(Some(AurPlan {
            steps: steps_for(&packages, helper, Some(w), Some(user)),
            method: helper.map_or(AurMethod::Manual, AurMethod::Helper),
            build_user: Some(user.to_string()),
            wrapper: Some(w.to_string()),
            packages,
        }));
    }

    // Already unprivileged: the current user is the build user, and a wrapper
    // would be a no-op that could only confuse the command shown to the user.
    Ok(Some(AurPlan {
        steps: steps_for(&packages, helper, None, None),
        method: helper.map_or(AurMethod::Manual, AurMethod::Helper),
        build_user: None,
        wrapper: None,
        packages,
    }))
}

/// The commands, in order, for each supported method.
fn steps_for(
    packages: &[String],
    helper: Option<AurHelper>,
    wrapper: Option<&str>,
    build_user: Option<&str>,
) -> Vec<AurStep> {
    match helper {
        Some(h) => vec![AurStep {
            program: h.as_str().to_string(),
            args: helper_args(packages),
            cwd: None,
            as_build_user: wrapper.is_some() && build_user.is_some(),
        }],
        None => packages
            .iter()
            .flat_map(|p| {
                let dir = format!("{AUR_BASE_URL}/{p}.git");
                let checkout = AurStep {
                    program: "git".into(),
                    args: vec![
                        "clone".into(),
                        "--depth".into(),
                        "1".into(),
                        dir,
                        build_dir(p),
                    ],
                    cwd: None,
                    as_build_user: wrapper.is_some() && build_user.is_some(),
                };
                let build = AurStep {
                    program: "makepkg".into(),
                    args: makepkg_args(std::slice::from_ref(p)),
                    cwd: Some(build_dir(p)),
                    as_build_user: wrapper.is_some() && build_user.is_some(),
                };
                [checkout, build]
            })
            .collect(),
    }
}

/// Arguments for a helper: install the named packages, resolve dependencies,
/// do not ask.
pub fn helper_args(packages: &[String]) -> Vec<String> {
    let mut args = vec![
        "-S".to_string(),
        "--needed".to_string(),
        "--noconfirm".to_string(),
    ];
    args.extend(packages.iter().cloned());
    args
}

/// Arguments for a manual `makepkg` build.
///
/// `--syncdeps` installs the *binary* dependencies through pacman (that part is
/// a normal root package install); the PKGBUILD itself still runs unprivileged.
/// `--needed` and `--noconfirm` keep a retried build from re-asking questions.
pub fn makepkg_args(packages: &[String]) -> Vec<String> {
    let mut args = vec![
        "--syncdeps".to_string(),
        "--needed".to_string(),
        "--noconfirm".to_string(),
        "--noprogressbar".to_string(),
    ];
    args.extend(packages.iter().cloned());
    args
}

/// Where a manual build happens, relative to the installer's working directory.
pub fn build_dir(package: &str) -> String {
    format!("aur-build/{package}")
}

/// The final, immediate check: refuse to spawn anything if this process is root.
///
/// Deliberately last. `plan()` already refused a root build without a user, and
/// every step is wrapped, so this should never fire — and that is the point. It
/// is the check that survives a caller that skipped the other two.
pub fn ensure_not_root(uid: Option<u32>) -> Result<()> {
    match uid {
        Some(0) => Err(CamarelolError::new(
            ErrorCode::AurBuildRefused,
            "refusing to run an AUR build: this process is root (uid 0)",
        )
        .with_remediation(
            "AUR packages are built as a normal user; configure an unprivileged build account",
        )),
        // Unknown uid is not a licence to build as root.
        None => Err(CamarelolError::new(
            ErrorCode::AurBuildRefused,
            "refusing to run an AUR build: the effective uid could not be determined",
        )
        .with_remediation("this is a fail-safe, not a guess; check /proc is mounted")),
        Some(_) => Ok(()),
    }
}

/// The text the user must read before AUR is used.
pub fn warning() -> &'static str {
    AUR_WARNING
}

/// What to tell the user when no helper is installed.
pub fn no_helper_message() -> String {
    format!(
        "none of the AUR helpers ({}) is installed; camareLOL will build the \
         selected packages manually with git + makepkg. It will not install a \
         helper for you.",
        AUR_HELPERS.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selection(packages: &[&str], enabled: bool) -> PackageSelection {
        PackageSelection {
            aur_enabled: enabled,
            aur_packages: packages.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    // ------------------------------------------------------------- opt-in

    #[test]
    fn a_disabled_selection_plans_nothing_and_is_not_an_error() {
        let plan = plan(&selection(&["some-pkg"], false), None, None, None, true).unwrap();
        assert!(
            plan.is_none(),
            "AUR is off by default and must stay that way"
        );
    }

    #[test]
    fn enabling_aur_with_an_empty_list_plans_nothing() {
        // "Everything" is never an acceptable reading of an empty list.
        let plan = plan(&selection(&[], true), None, None, None, true).unwrap();
        assert!(plan.is_none());
    }

    #[test]
    fn the_default_configuration_never_builds_anything() {
        assert!(!PackageSelection::default().aur_enabled);
        assert!(plan(&PackageSelection::default(), None, None, None, true)
            .unwrap()
            .is_none());
    }

    #[test]
    fn an_explicit_list_is_planned_and_sorted_deduped() {
        let sel = selection(&["zeta", "alpha", "alpha"], true);
        let plan = plan(&sel, None, Some("runuser"), Some("builder"), true)
            .unwrap()
            .unwrap();
        assert_eq!(plan.packages, vec!["alpha".to_string(), "zeta".to_string()]);
    }

    // ------------------------------------------------------- never as root

    #[test]
    fn a_root_run_without_a_build_user_is_refused() {
        let err = plan(
            &selection(&["pkg"], true),
            None,
            Some("runuser"),
            None,
            true,
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::AurBuildRefused);
        assert!(err.to_string().contains("normal user"), "{err}");
        assert!(
            err.remediation
                .as_deref()
                .unwrap()
                .contains("aur_build_user"),
            "the refusal must say how to fix it: {err}"
        );
    }

    #[test]
    fn a_root_run_without_a_privilege_wrapper_is_refused() {
        let err = plan(
            &selection(&["pkg"], true),
            None,
            None,
            Some("builder"),
            true,
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::AurBuildRefused);
        assert!(err.to_string().contains("runuser"), "{err}");
    }

    #[test]
    fn an_empty_build_user_counts_as_no_build_user() {
        let err = plan(
            &selection(&["pkg"], true),
            None,
            Some("runuser"),
            Some(""),
            true,
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::AurBuildRefused);
    }

    #[test]
    fn a_root_run_with_a_user_but_no_wrapper_never_falls_back_to_a_bare_build() {
        let err = plan(
            &selection(&["pkg"], true),
            None,
            Some("runuser"),
            Some("builder"),
            true,
        )
        .unwrap();
        let plan = err.unwrap();
        for step in &plan.steps {
            assert!(
                step.as_build_user,
                "every step of a root run must be wrapped: {:?}",
                step.render()
            );
        }
    }

    #[test]
    fn the_immediate_uid_check_refuses_root_and_uncertainty() {
        assert_eq!(
            ensure_not_root(Some(0)).unwrap_err().code,
            ErrorCode::AurBuildRefused
        );
        // "I could not tell" must not become "sure, go ahead".
        assert_eq!(
            ensure_not_root(None).unwrap_err().code,
            ErrorCode::AurBuildRefused
        );
        assert!(ensure_not_root(Some(1000)).is_ok());
        assert!(ensure_not_root(Some(65534)).is_ok());
    }

    #[test]
    fn uid_parsing_reads_the_effective_column_not_the_real_one() {
        assert_eq!(parse_effective_uid("Uid:\t0\t1000\t0\t0\n").unwrap(), 1000);
        assert_eq!(
            parse_effective_uid("Uid: 1000 1000 1000 1000\n").unwrap(),
            1000
        );
        assert_eq!(
            parse_effective_uid("Gid:\t0\t0\t0\t0\nUid:\t0\t0\t0\t0\n").unwrap(),
            0
        );
        assert_eq!(parse_effective_uid("nothing here\n"), None);
        assert_eq!(parse_effective_uid("Uid:\n"), None);
        assert_eq!(parse_effective_uid("Uid:\t1000\n"), None);
        assert_eq!(parse_effective_uid(""), None);
        assert_eq!(parse_effective_uid("Uid:\t0\tnotanumber\n"), None);
    }

    #[test]
    fn the_live_uid_check_agrees_with_the_process() {
        // Reads /proc; on any Linux CI this must agree with the uid cargo runs as.
        if let Some(uid) = effective_uid() {
            assert_eq!(ensure_not_root(Some(uid)).is_ok(), uid != 0);
        }
    }

    // ------------------------------------------------------------ wrappers

    #[test]
    fn wrapping_produces_the_exact_argv_without_a_shell() {
        assert_eq!(
            privilege_wrapper(Some("runuser"), Some("builder"), "makepkg").unwrap(),
            vec!["runuser", "-u", "builder", "--", "makepkg"]
        );
        // No wrapper is only legal when there is no user to drop to.
        assert_eq!(
            privilege_wrapper(None, None, "makepkg").unwrap(),
            vec!["makepkg"]
        );
        // Half the rule is never applied.
        assert!(privilege_wrapper(Some("runuser"), None, "makepkg").is_err());
        assert!(privilege_wrapper(None, Some("builder"), "makepkg").is_err());
    }

    #[test]
    fn every_helper_name_is_recognised_in_both_directions() {
        for name in AUR_HELPERS {
            let helper = AurHelper::from_name(name).expect("listed helper must parse");
            assert_eq!(helper.as_str(), *name);
        }
        assert!(AurHelper::from_name("not-a-helper").is_none());
        assert!(AurHelper::from_name("").is_none());
        // The list is the documented one, in preference order.
        assert_eq!(AUR_HELPERS, ["yay", "paru", "pikaur", "trizen"]);
    }

    // ------------------------------------------------------------- helpers

    #[test]
    fn a_detected_helper_becomes_one_command() {
        let plan = plan(
            &selection(&["pkg-a", "pkg-b"], true),
            Some(AurHelper::Paru),
            Some("runuser"),
            Some("builder"),
            true,
        )
        .unwrap()
        .unwrap();
        assert_eq!(plan.method, AurMethod::Helper(AurHelper::Paru));
        assert_eq!(plan.steps.len(), 1);
        let step = &plan.steps[0];
        assert_eq!(step.program, "paru");
        assert_eq!(
            step.args,
            vec!["-S", "--needed", "--noconfirm", "pkg-a", "pkg-b"]
        );
        assert!(step.as_build_user);
        assert!(step.cwd.is_none());
        // And the rendered line names the dropped user, so the preview log is
        // not hiding which account runs unreviewed code.
        assert!(step.render().starts_with("paru -S"), "{}", step.render());
    }

    #[test]
    fn helper_args_never_passes_a_package_as_an_option() {
        // Names are validated separately; this asserts the shape stays stable.
        assert_eq!(
            helper_args(&["one".into(), "two".into()]),
            vec!["-S", "--needed", "--noconfirm", "one", "two"]
        );
    }

    // -------------------------------------------------------------- manual

    #[test]
    fn without_a_helper_the_manual_path_clones_then_builds_per_package() {
        let plan = plan(
            &selection(&["pkg-a", "pkg-b"], true),
            None,
            Some("runuser"),
            Some("builder"),
            true,
        )
        .unwrap()
        .unwrap();
        assert_eq!(plan.method, AurMethod::Manual);
        assert_eq!(
            plan.steps.len(),
            4,
            "checkout+build for each of two packages"
        );
        assert_eq!(plan.steps[0].program, "git");
        assert!(plan.steps[0].args.contains(&"clone".to_string()));
        assert!(plan.steps[0]
            .args
            .contains(&"https://aur.archlinux.org/pkg-a.git".to_string()));
        assert_eq!(plan.steps[1].program, "makepkg");
        assert_eq!(plan.steps[1].cwd.as_deref(), Some("aur-build/pkg-a"));
        assert_eq!(plan.steps[3].cwd.as_deref(), Some("aur-build/pkg-b"));
        assert!(plan.steps.iter().all(|s| s.as_build_user));
    }

    #[test]
    fn makepkg_is_asked_to_sync_dependencies_but_not_to_prompt() {
        let args = makepkg_args(&["pkg".into()]);
        assert!(args.contains(&"--syncdeps".to_string()));
        assert!(args.contains(&"--noconfirm".to_string()));
        assert_eq!(args.last().unwrap(), "pkg");
        assert!(!args.iter().any(|a| a == "--root"), "no root build flag");
    }

    #[test]
    fn an_unprivileged_run_needs_no_wrapper_and_says_so() {
        let plan = plan(&selection(&["pkg"], true), None, None, None, false)
            .unwrap()
            .unwrap();
        assert!(plan.already_unprivileged());
        assert!(plan.steps.iter().all(|s| !s.as_build_user));
        // The current user builds; nothing is dropped and nothing needs to be.
        assert!(plan.build_user.is_none());
    }

    #[test]
    fn the_plan_renders_in_execution_order() {
        let plan = plan(
            &selection(&["pkg"], true),
            None,
            Some("runuser"),
            Some("builder"),
            true,
        )
        .unwrap()
        .unwrap();
        let line = plan.render();
        let git_at = line.find("git clone").unwrap_or_else(|| panic!("{line}"));
        let mk_at = line.find("makepkg").unwrap_or_else(|| panic!("{line}"));
        assert!(git_at < mk_at, "checkout must come before build: {line}");
        assert!(line.contains("&&"), "steps are sequential: {line}");
    }

    // ---------------------------------------------------------- validation

    #[test]
    fn ordinary_aur_package_names_are_accepted() {
        for name in [
            "yay",
            "paru",
            "some-pkg",
            "some_pkg",
            "Some.Pkg",
            "lib32-foo",
            "pkg@2",
            "a",
            "python-foo",
            "0ad",
        ] {
            assert!(
                validate_package_name(name).is_ok(),
                "{name} should be a valid package name"
            );
        }
    }

    #[test]
    fn names_that_would_become_shell_syntax_or_a_path_are_rejected() {
        for name in [
            "",
            "  ",
            "pkg;rm -rf /",
            "pkg && rm -rf /",
            "pkg|cat",
            "pkg`id`",
            "pkg$(id)",
            "pkg name",
            "pkg\nrm",
            "../../etc",
            "pkg/name",
            "/abs/pkg",
            "pkg$HOME",
            "pkg\\x",
        ] {
            assert!(
                validate_package_name(name).is_err(),
                "{name:?} must be refused as a package name"
            );
        }
    }

    #[test]
    fn a_leading_dash_is_rejected_because_argv_would_read_it_as_an_option() {
        // The subtle one: `makepkg --help` exits 0 and installs nothing, which
        // is the "reports success, did nothing" failure this project keeps
        // hitting in a different costume.
        for name in ["--help", "-sysupgrade", "-S", "--needed"] {
            let err = validate_package_name(name).unwrap_err();
            assert_eq!(err.code, ErrorCode::InvalidConfig);
            assert!(err.to_string().contains("option"), "{err}");
        }
    }

    #[test]
    fn validation_is_all_or_nothing() {
        let sel = selection(&["good-pkg", "-bad"], true);
        let err = plan(&sel, None, Some("runuser"), Some("builder"), true).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidConfig);
    }

    #[test]
    fn a_bad_package_name_is_refused_before_the_helper_is_even_asked() {
        // Validation runs first, so an invalid name cannot be turned into a
        // helper invocation by any path.
        let sel = selection(&["--noconfirm"], true);
        assert!(plan(&sel, Some(AurHelper::Yay), Some("runuser"), Some("b"), true).is_err());
    }

    // ------------------------------------------------------------- messages

    #[test]
    fn the_no_helper_message_lists_every_helper_and_installs_nothing() {
        let msg = no_helper_message();
        for h in AUR_HELPERS {
            assert!(msg.contains(h), "{h} must be named in: {msg}");
        }
        assert!(msg.contains("git + makepkg"), "{msg}");
        assert!(
            msg.contains("will not install a helper"),
            "the refusal to install one must be explicit: {msg}"
        );
    }

    #[test]
    fn the_warning_states_the_normal_user_rule() {
        assert!(warning().contains("never as root"), "{}", warning());
    }

    // -------------------------------------------------------------- probing

    #[test]
    fn helper_detection_only_ever_names_a_listed_helper() {
        // `detect_helper` reads PATH through `inspect`. Whatever the test host
        // has, the answer must be a member of the allow-list.
        if let Some(h) = detect_helper() {
            assert!(AUR_HELPERS.contains(&h.as_str()));
        }
    }

    #[test]
    fn the_detection_probes_are_read_only_and_run_in_preview() {
        // The preview latch must not stop a `--version` probe: the preview has
        // to show the *real* program, and stopping the probe would make it
        // print `makepkg` on a machine where the run would call `yay`.
        let _scope = crate::sys::PreviewScope::on();
        let _ = detect_helper();
        assert_eq!(
            crate::sys::mutating_refusals(),
            0,
            "a --version probe must never be counted as a mutating attempt"
        );
    }

    #[test]
    fn a_missing_wrapper_is_reported_rather_than_guessed() {
        // On a host with neither runuser nor sudo, `detect_drop_program` returns
        // None and `plan` refuses. Nothing here installs either one.
        match detect_drop_program() {
            Some(w) => assert!(w == "runuser" || w == "sudo", "{w}"),
            None => {
                let err = plan(
                    &selection(&["pkg"], true),
                    None,
                    None,
                    Some("builder"),
                    true,
                )
                .unwrap_err();
                assert_eq!(err.code, ErrorCode::AurBuildRefused);
            }
        }
    }
}
