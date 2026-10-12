//! Language, keyboard, locale and timezone selection.

use crate::error::{CamarelolError, ErrorCode, Result};
use serde::{Deserialize, Serialize};

/// UI languages. `vi` is a first-class citizen because camareLOL is Vietnamese.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum UiLanguage {
    Vietnamese,
    English,
}

impl UiLanguage {
    pub fn code(&self) -> &'static str {
        match self {
            UiLanguage::Vietnamese => "vi",
            UiLanguage::English => "en",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code.to_ascii_lowercase().as_str() {
            "vi" | "vi_vn" | "vie" => Some(UiLanguage::Vietnamese),
            "en" | "en_us" | "eng" => Some(UiLanguage::English),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            UiLanguage::Vietnamese => "Tiếng Việt",
            UiLanguage::English => "English",
        }
    }
}

/// System locales offered in the installer.
pub const SYSTEM_LOCALES: &[&str] = &["en_US.UTF-8", "vi_VN.UTF-8", "en_GB.UTF-8", "ja_JP.UTF-8"];
/// Console keymaps offered in the installer.
pub const KEYMAPS: &[&str] = &["us", "uk", "vn" /* fr, de ... available via custom */];
/// Timezones offered in the installer (extendable).
pub const TIMEZONES: &[&str] = &[
    "Asia/Ho_Chi_Minh",
    "Asia/Bangkok",
    "Asia/Tokyo",
    "Europe/London",
    "Europe/Berlin",
    "America/New_York",
    "UTC",
];

pub fn validate_locale(locale: &str) -> Result<()> {
    if locale.is_empty() || !locale.contains('.') {
        return Err(CamarelolError::new(
            ErrorCode::InvalidConfig,
            format!("`{locale}` is not a valid locale (expected like `en_US.UTF-8`)"),
        ));
    }
    if locale
        .chars()
        .any(|c| !(c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-'))
    {
        return Err(CamarelolError::new(
            ErrorCode::InvalidConfig,
            format!("`{locale}` contains invalid characters"),
        ));
    }
    Ok(())
}

pub fn validate_timezone(tz: &str) -> Result<()> {
    if tz.is_empty() || tz.contains("..") || tz.starts_with('/') {
        return Err(CamarelolError::new(
            ErrorCode::InvalidConfig,
            format!("`{tz}` is not a valid timezone (expected like `Asia/Ho_Chi_Minh`)"),
        ));
    }
    Ok(())
}

/// Suggested configuration for a given UI language.
pub fn suggest(ui: UiLanguage, detected_tz: Option<&str>) -> LocalePlan {
    match ui {
        UiLanguage::Vietnamese => LocalePlan {
            locale: "vi_VN.UTF-8".into(),
            keymap: "vn".into(),
            timezone: detected_tz.unwrap_or("Asia/Ho_Chi_Minh").to_string(),
        },
        UiLanguage::English => LocalePlan {
            locale: "en_US.UTF-8".into(),
            keymap: "us".into(),
            timezone: detected_tz.unwrap_or("UTC").to_string(),
        },
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalePlan {
    pub locale: String,
    pub keymap: String,
    pub timezone: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_codes_round_trip() {
        assert_eq!(UiLanguage::from_code("vi"), Some(UiLanguage::Vietnamese));
        assert_eq!(UiLanguage::from_code("en"), Some(UiLanguage::English));
        assert_eq!(UiLanguage::from_code("fr"), None);
    }

    #[test]
    fn locale_validation() {
        assert!(validate_locale("vi_VN.UTF-8").is_ok());
        assert!(validate_locale("vi_VN").is_err());
        assert!(validate_locale("vi VN; rm -rf /").is_err());
    }

    #[test]
    fn timezone_validation() {
        assert!(validate_timezone("Asia/Ho_Chi_Minh").is_ok());
        assert!(validate_timezone("../etc").is_err());
    }

    #[test]
    fn vietnamese_defaults_suggested() {
        let plan = suggest(UiLanguage::Vietnamese, None);
        assert_eq!(plan.locale, "vi_VN.UTF-8");
        assert_eq!(plan.timezone, "Asia/Ho_Chi_Minh");
    }
}
