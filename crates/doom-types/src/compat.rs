//! Compatibility profiles for vanilla parity versus extended behavior.

use core::fmt;
use core::str::FromStr;

/// Compatibility profile for behavior that can differ between vanilla Doom and
/// the current extended engine.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CompatibilityProfile {
    /// Preserve the current extended/source-port behavior.
    #[default]
    Extended,
    /// Enforce vanilla-compatible behavior where the engine has a strict path.
    VanillaStrict,
}

impl CompatibilityProfile {
    /// Return the canonical CLI string for this profile.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Extended => "extended",
            Self::VanillaStrict => "vanilla-strict",
        }
    }
}

impl fmt::Display for CompatibilityProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for CompatibilityProfile {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "extended" => Ok(Self::Extended),
            "vanilla-strict" => Ok(Self::VanillaStrict),
            _ => Err("expected `extended` or `vanilla-strict`"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_extended() {
        assert_eq!(
            CompatibilityProfile::default(),
            CompatibilityProfile::Extended
        );
    }

    #[test]
    fn as_str_returns_correct_strings() {
        assert_eq!(CompatibilityProfile::Extended.as_str(), "extended");
        assert_eq!(
            CompatibilityProfile::VanillaStrict.as_str(),
            "vanilla-strict"
        );
    }

    #[test]
    fn display_formats_correctly() {
        assert_eq!(format!("{}", CompatibilityProfile::Extended), "extended");
        assert_eq!(
            format!("{}", CompatibilityProfile::VanillaStrict),
            "vanilla-strict"
        );
    }

    #[test]
    fn from_str_parses_valid_strings() {
        assert_eq!(
            "extended".parse::<CompatibilityProfile>(),
            Ok(CompatibilityProfile::Extended)
        );
        assert_eq!(
            "vanilla-strict".parse::<CompatibilityProfile>(),
            Ok(CompatibilityProfile::VanillaStrict)
        );
    }

    #[test]
    fn from_str_rejects_invalid_strings() {
        assert_eq!(
            "vanilla".parse::<CompatibilityProfile>(),
            Err("expected `extended` or `vanilla-strict`")
        );
        assert_eq!(
            "".parse::<CompatibilityProfile>(),
            Err("expected `extended` or `vanilla-strict`")
        );
    }
}
