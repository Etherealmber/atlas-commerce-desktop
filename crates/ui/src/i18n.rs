//! Bundled interface languages. English is the source language and the
//! fallback; French is the product default and ships a complete catalog.

include!(concat!(env!("OUT_DIR"), "/catalogs.rs"));

pub use fastframe_i18n::{gettext, ngettext, pgettext};

/// The languages the app ships.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Locale {
    /// French — the default for every install.
    #[default]
    French,
    /// English — the gettext source language, no catalog needed.
    English,
}

impl fastframe_i18n::Locale for Locale {
    fn catalog(self) -> Option<&'static dyn fastframe_i18n::Translator> {
        match self {
            Self::French => Some(&fr::Translator),
            Self::English => None,
        }
    }
}

impl Locale {
    #[must_use]
    pub fn tag(self) -> &'static str {
        match self {
            Self::French => "fr",
            Self::English => "en",
        }
    }

    #[must_use]
    pub fn native_name(self) -> &'static str {
        match self {
            Self::French => "Français",
            Self::English => "English",
        }
    }

    #[must_use]
    pub fn from_tag(tag: &str) -> Self {
        match tag.trim().to_ascii_lowercase().as_str() {
            "en" | "en-us" | "en-gb" => Self::English,
            _ => Self::French,
        }
    }
}

/// Translates a static English source string for the given locale.
#[macro_export]
macro_rules! t {
    ($locale:expr, $source:literal) => {
        $crate::i18n::gettext($locale, $source)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn french_is_the_default_and_english_the_source() {
        assert_eq!(Locale::default(), Locale::French);
        assert_eq!(Locale::French.tag(), "fr");
        assert_eq!(Locale::from_tag("en-GB"), Locale::English);
        assert_eq!(Locale::from_tag("fr-FR"), Locale::French);
        assert_eq!(Locale::from_tag("anything"), Locale::French);
    }

    #[test]
    fn catalogs_are_compiled_in() {
        // The French catalog exists; an untranslated id falls back to the
        // English source for both locales.
        let source = "This string is intentionally untranslated";
        assert_eq!(gettext(Locale::English, source), source);
        assert_eq!(gettext(Locale::French, source), source);
    }
}
