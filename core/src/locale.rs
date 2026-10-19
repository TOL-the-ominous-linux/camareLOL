//! Language, keyboard, locale and timezone selection.

use crate::error::{CamarelolError, ErrorCode, Result};
use serde::{Deserialize, Serialize};

/// UI languages. `vi` is a first-class citizen because camareLOL is Vietnamese.
///
/// The third language follows exactly the same shape as the first two: a
/// variant, a `code`, a set of accepted aliases in `from_code`, a `name` shown
/// in the picker, and a [`suggest`]ed system locale. Nothing about the
/// translation mechanism changed when it was added.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum UiLanguage {
    Vietnamese,
    English,
    /// Simplified Chinese (`--lang zh`). Wording follows `docs/zh/*` so the
    /// interface and the documentation call a partition layout a 分区布局 and a
    /// bootloader 引导加载程序 rather than inventing a third vocabulary.
    Chinese,
}

/// Every language the picker offers, in the order it offers them.
///
/// The TUI indexes this list directly, so adding a language here is the only
/// way it can appear in the picker — no second list to keep in sync.
pub const UI_LANGUAGES: &[UiLanguage] = &[
    UiLanguage::Vietnamese,
    UiLanguage::English,
    UiLanguage::Chinese,
];

impl UiLanguage {
    pub fn code(&self) -> &'static str {
        match self {
            UiLanguage::Vietnamese => "vi",
            UiLanguage::English => "en",
            UiLanguage::Chinese => "zh",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code.to_ascii_lowercase().as_str() {
            "vi" | "vi_vn" | "vie" => Some(UiLanguage::Vietnamese),
            "en" | "en_us" | "eng" => Some(UiLanguage::English),
            "zh" | "zh_cn" | "zh_hans" | "zho" => Some(UiLanguage::Chinese),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            UiLanguage::Vietnamese => "Tiếng Việt",
            UiLanguage::English => "English",
            UiLanguage::Chinese => "简体中文",
        }
    }

    /// Position in [`UI_LANGUAGES`], which is what the TUI cursor stores.
    pub fn index(&self) -> usize {
        UI_LANGUAGES
            .iter()
            .position(|l| l == self)
            .unwrap_or_default()
    }

    /// The language at a picker index, clamped so a stale cursor can never
    /// index out of the list.
    pub fn at(index: usize) -> Self {
        UI_LANGUAGES[index.min(UI_LANGUAGES.len() - 1)]
    }
}

/// System locales offered in the installer.
pub const SYSTEM_LOCALES: &[&str] = &[
    "en_US.UTF-8",
    "vi_VN.UTF-8",
    "zh_CN.UTF-8",
    "en_GB.UTF-8",
    "ja_JP.UTF-8",
];
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
        UiLanguage::Chinese => LocalePlan {
            locale: "zh_CN.UTF-8".into(),
            // The Chinese keymap on a US layout is pinyin-in-ANSI, which is
            // what a Chinese user on the default hardware expects; a dedicated
            // layout is available through the custom path.
            keymap: "us".into(),
            timezone: detected_tz.unwrap_or("Asia/Shanghai").to_string(),
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
        for lang in UI_LANGUAGES {
            assert_eq!(UiLanguage::from_code(lang.code()), Some(*lang));
        }
        assert_eq!(UiLanguage::from_code("fr"), None);
    }

    #[test]
    fn chinese_is_a_full_third_language() {
        // The variant exists, it parses, it has a name for the picker, and it
        // round-trips through the same code path the other two use.
        assert_eq!(UiLanguage::Chinese.code(), "zh");
        assert_eq!(UiLanguage::Chinese.name(), "简体中文");
        for alias in ["zh", "ZH", "zh_CN", "zh_hans", "zho"] {
            assert_eq!(
                UiLanguage::from_code(alias),
                Some(UiLanguage::Chinese),
                "{alias} must select Chinese"
            );
        }
        assert_eq!(UiLanguage::Chinese.index(), 2);
        assert_eq!(UiLanguage::at(2), UiLanguage::Chinese);
    }

    #[test]
    fn the_picker_list_holds_every_language_exactly_once() {
        assert_eq!(UI_LANGUAGES.len(), 3);
        for (i, lang) in UI_LANGUAGES.iter().enumerate() {
            assert_eq!(lang.index(), i, "{lang:?} is in the wrong slot");
            assert_eq!(UiLanguage::at(i), *lang);
            assert!(!lang.name().is_empty());
        }
        // And an out-of-range cursor clamps instead of panicking.
        assert_eq!(UiLanguage::at(99), UiLanguage::Chinese);
    }

    #[test]
    fn chinese_suggests_a_real_locale_and_timezone() {
        let plan = suggest(UiLanguage::Chinese, None);
        assert_eq!(plan.locale, "zh_CN.UTF-8");
        assert_eq!(plan.timezone, "Asia/Shanghai");
        assert!(validate_locale(&plan.locale).is_ok());
        assert!(validate_timezone(&plan.timezone).is_ok());
        assert!(SYSTEM_LOCALES.contains(&plan.locale.as_str()));
        // A detected timezone always wins over the default, in every language.
        let plan = suggest(UiLanguage::Chinese, Some("Asia/Ho_Chi_Minh"));
        assert_eq!(plan.timezone, "Asia/Ho_Chi_Minh");
    }

    #[test]
    fn every_language_has_a_usable_suggestion() {
        // A new variant added without a `suggest` arm would have been a
        // compile error, but a *wrong* suggestion would not be.
        for lang in UI_LANGUAGES {
            let plan = suggest(*lang, None);
            assert!(
                validate_locale(&plan.locale).is_ok(),
                "{lang:?}: {}",
                plan.locale
            );
            assert!(
                validate_timezone(&plan.timezone).is_ok(),
                "{lang:?}: {}",
                plan.timezone
            );
            assert!(!plan.keymap.is_empty(), "{lang:?}");
        }
    }

    #[test]
    fn a_chinese_config_validates_end_to_end() {
        // Same path the TUI uses: build a config with language = "zh" and run
        // the real validation. A missing `UiLanguage::Chinese` arm anywhere in
        // the chain surfaces here rather than on a user's machine.
        let cfg = crate::config::InstallationConfig {
            language: "zh".into(),
            system_locale: "zh_CN.UTF-8".into(),
            timezone: "Asia/Shanghai".into(),
            ..Default::default()
        };
        assert_eq!(cfg.ui_language().unwrap(), UiLanguage::Chinese);
        assert!(cfg.validate().is_ok());
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
