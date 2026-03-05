//! Domain-constrained primitive newtypes from the Unofficial Doom Specs.
//!
//! Each newtype has:
//! - A `const fn new(v) -> Option<Self>` constructor that enforces spec bounds.
//! - A `const fn raw(self) -> T` accessor.
//!
//! These are candidates for Verus `#[invariant]` annotations.

/// Light level 0..=255.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Brightness(u8);

impl Brightness {
    /// Create, returning `None` if out of range (always succeeds for `u8`).
    #[inline]
    pub const fn new(v: u8) -> Option<Self> {
        Some(Self(v))
    }

    #[inline]
    pub const fn raw(self) -> u8 {
        self.0
    }

    /// Minimum brightness (pitch black).
    pub const MIN: Self = Self(0);
    /// Maximum brightness (full light).
    pub const MAX: Self = Self(255);
}

/// Sector special type 0..=16 (standard Doom).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SectorSpecial(u8);

impl SectorSpecial {
    /// Returns `None` if `v > 16`.
    #[inline]
    pub const fn new(v: u8) -> Option<Self> {
        if v <= 16 { Some(Self(v)) } else { None }
    }

    #[inline]
    pub const fn raw(self) -> u8 {
        self.0
    }

    /// Normal sector (no special).
    pub const NORMAL: Self = Self(0);
}

/// Skill level 0..=4.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SkillLevel(u8);

impl SkillLevel {
    #[inline]
    pub const fn new(v: u8) -> Option<Self> {
        if v <= 4 { Some(Self(v)) } else { None }
    }

    #[inline]
    pub const fn raw(self) -> u8 {
        self.0
    }

    pub const ITYTD: Self = Self(0);
    pub const HNTR: Self = Self(1);
    pub const HMP: Self = Self(2);
    pub const UV: Self = Self(3);
    pub const NM: Self = Self(4);
}

/// Player number 0..=3.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PlayerNum(u8);

impl PlayerNum {
    #[inline]
    pub const fn new(v: u8) -> Option<Self> {
        if v <= 3 { Some(Self(v)) } else { None }
    }

    #[inline]
    pub const fn raw(self) -> u8 {
        self.0
    }
}

/// Map vertex coordinate — spec range [-32768, 32767] = `i16`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Coord(pub i16);

impl Coord {
    #[inline]
    pub const fn new(v: i16) -> Self {
        Self(v)
    }

    #[inline]
    pub const fn raw(self) -> i16 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sector_special_rejects_over_16() {
        assert!(SectorSpecial::new(17).is_none());
        assert!(SectorSpecial::new(16).is_some());
    }

    #[test]
    fn skill_level_rejects_over_4() {
        assert!(SkillLevel::new(5).is_none());
        assert_eq!(SkillLevel::new(4).unwrap().raw(), 4);
    }

    #[test]
    fn player_num_rejects_over_3() {
        assert!(PlayerNum::new(4).is_none());
        assert!(PlayerNum::new(3).is_some());
    }

    #[test]
    fn brightness_always_valid() {
        assert!(Brightness::new(255).is_some());
        assert!(Brightness::new(0).is_some());
    }
}
