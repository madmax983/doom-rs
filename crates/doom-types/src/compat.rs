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
    fn should_return_canonical_cli_string() {
        assert_eq!(CompatibilityProfile::Extended.as_str(), "extended");
        assert_eq!(CompatibilityProfile::VanillaStrict.as_str(), "vanilla-strict");
    }

    #[test]
    fn should_display_correctly() {
        assert_eq!(format!("{}", CompatibilityProfile::Extended), "extended");
        assert_eq!(format!("{}", CompatibilityProfile::VanillaStrict), "vanilla-strict");
    }

    #[test]
    fn should_parse_from_str_correctly() {
        assert_eq!(CompatibilityProfile::from_str("extended"), Ok(CompatibilityProfile::Extended));
        assert_eq!(CompatibilityProfile::from_str("vanilla-strict"), Ok(CompatibilityProfile::VanillaStrict));
        assert_eq!(CompatibilityProfile::from_str("invalid"), Err("expected `extended` or `vanilla-strict`"));
    }
}
