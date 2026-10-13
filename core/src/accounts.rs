//! Hostname, users, passwords and sudo policy.
//!
//! Passwords are validated only (length/composition); they are **never** stored
//! in the configuration file, written to logs, or passed on a command line —
//! the installer passes them to `arch-chroot` via stdin only.

use crate::error::{CamarelolError, ErrorCode, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserAccount {
    pub username: String,
    /// Never serialized. Kept only for the duration of the install.
    #[serde(skip)]
    pub password: Option<String>,
    pub has_admin: bool,
    pub shell: String,
    pub extra_groups: BTreeSet<String>,
}

impl UserAccount {
    pub fn new(username: &str) -> Self {
        UserAccount {
            username: username.to_string(),
            password: None,
            has_admin: false,
            shell: "/bin/bash".into(),
            extra_groups: BTreeSet::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountPlan {
    pub hostname: String,
    pub root_password_set: bool,
    pub root_login_allowed: bool,
    pub users: Vec<UserAccount>,
}

/// Validate the hostname (RFC 952/1123 subset).
pub fn validate_hostname(host: &str) -> Result<()> {
    if host.is_empty() {
        return Err(CamarelolError::new(
            ErrorCode::InvalidAccount,
            "hostname must not be empty",
        ));
    }
    if host.len() > 63 {
        return Err(CamarelolError::new(
            ErrorCode::InvalidAccount,
            "hostname must be at most 63 characters",
        ));
    }
    let mut labels = 0;
    for label in host.split('.') {
        if label.is_empty() {
            return Err(CamarelolError::new(
                ErrorCode::InvalidAccount,
                "hostname has an empty label",
            ));
        }
        if label.len() > 63 {
            return Err(CamarelolError::new(
                ErrorCode::InvalidAccount,
                "hostname label is too long",
            ));
        }
        let first = label.chars().next().unwrap();
        let last = label.chars().last().unwrap();
        if !first.is_ascii_alphabetic() && !first.is_ascii_digit() {
            return Err(CamarelolError::new(
                ErrorCode::InvalidAccount,
                "hostname labels must start with a letter or digit",
            ));
        }
        if !(last.is_ascii_alphanumeric()) {
            return Err(CamarelolError::new(
                ErrorCode::InvalidAccount,
                "hostname labels must end with a letter or digit",
            ));
        }
        if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(CamarelolError::new(
                ErrorCode::InvalidAccount,
                "hostname may only contain letters, digits and hyphens",
            ));
        }
        labels += 1;
    }
    if labels == 0 {
        return Err(CamarelolError::new(
            ErrorCode::InvalidAccount,
            "hostname is invalid",
        ));
    }
    Ok(())
}

/// Validate a POSIX username.
pub fn validate_username(name: &str) -> Result<()> {
    if name.is_empty() || name.len() > 32 {
        return Err(CamarelolError::new(
            ErrorCode::InvalidAccount,
            "username must be 1..32 characters",
        ));
    }
    if !name.chars().next().unwrap().is_ascii_lowercase()
        && !name.chars().next().unwrap().is_ascii_digit()
    {
        return Err(CamarelolError::new(
            ErrorCode::InvalidAccount,
            "username must start with a lowercase letter or digit",
        ));
    }
    if name
        .chars()
        .any(|c| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-'))
    {
        return Err(CamarelolError::new(
            ErrorCode::InvalidAccount,
            "username may only contain lowercase letters, digits, `_` and `-`",
        ));
    }
    if name == "root" {
        return Err(CamarelolError::new(
            ErrorCode::InvalidAccount,
            "`root` is a reserved account",
        ));
    }
    Ok(())
}

/// Password policy: at least 8 characters, not a known-weak skeleton.
pub fn validate_password(password: &str) -> Result<()> {
    if password.len() < 8 {
        return Err(CamarelolError::new(
            ErrorCode::InvalidAccount,
            "password must be at least 8 characters",
        ));
    }
    let weak = [
        "password",
        "12345678",
        "qwerty",
        "archlinux",
        "letmein",
        "admin123",
    ];
    let lower = password.to_ascii_lowercase();
    for w in weak {
        if lower.contains(w) {
            return Err(CamarelolError::new(
                ErrorCode::InvalidAccount,
                "this password is too common",
            ));
        }
    }
    Ok(())
}

/// Check the whole plan; in particular refuse lock-outs with no recovery path.
pub fn validate_account_plan(plan: &AccountPlan) -> Result<()> {
    validate_hostname(&plan.hostname)?;
    let mut names = BTreeSet::new();
    for u in &plan.users {
        validate_username(&u.username)?;
        if !names.insert(u.username.clone()) {
            return Err(CamarelolError::new(
                ErrorCode::InvalidAccount,
                format!("duplicate user `{}`", u.username),
            ));
        }
        if let Some(p) = &u.password {
            validate_password(p)?;
        }
    }
    let any_admin = plan.users.iter().any(|u| u.has_admin);
    if !plan.root_login_allowed && !any_admin {
        return Err(CamarelolError::new(
            ErrorCode::InvalidAccount,
            "root login is disabled and no user has sudo rights — you would be locked out",
        )
        .with_remediation("grant sudo to at least one user or allow root login"));
    }
    if !plan.root_password_set && !plan.root_login_allowed {
        return Err(CamarelolError::new(
            ErrorCode::InvalidAccount,
            "root has no password and cannot log in",
        )
        .with_remediation("set a root password or enable an admin user"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hostname_rules() {
        assert!(validate_hostname("archbox").is_ok());
        assert!(validate_hostname("arch-box.local").is_ok());
        assert!(validate_hostname("-bad").is_err());
        assert!(validate_hostname("bad-").is_err());
        assert!(validate_hostname("has space").is_err());
        assert!(validate_hostname("").is_err());
    }

    #[test]
    fn username_rules() {
        assert!(validate_username("alice").is_ok());
        assert!(validate_username("alice-2").is_ok());
        assert!(validate_username("Alice").is_err());
        assert!(validate_username("root").is_err());
        assert!(validate_username("a").is_ok());
    }

    #[test]
    fn password_policy() {
        assert!(validate_password("Str0ng-Pass").is_ok());
        assert!(validate_password("short").is_err());
        assert!(validate_password("password123").is_err());
    }

    #[test]
    fn refuses_lockout() {
        let plan = AccountPlan {
            hostname: "arch".into(),
            root_password_set: false,
            root_login_allowed: false,
            users: vec![],
        };
        assert_eq!(
            validate_account_plan(&plan).unwrap_err().code,
            ErrorCode::InvalidAccount
        );
    }

    #[test]
    fn accepts_admin_user_without_root() {
        let plan = AccountPlan {
            hostname: "arch".into(),
            root_password_set: true,
            root_login_allowed: false,
            users: vec![UserAccount {
                has_admin: true,
                ..UserAccount::new("alice")
            }],
        };
        assert!(validate_account_plan(&plan).is_ok());
    }
}
