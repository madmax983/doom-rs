//! Domain-constrained primitive newtypes from the Unofficial Doom Specs.
//!
//! # The Story of Tamed Primitives
//!
//! In the original C implementation of Doom, variables like player numbers, skill levels,
//! and sector specials were often passed around as raw `int`s or `short`s. This made it
//! easy to accidentally pass a brightness level (0-255) to a function expecting a
//! skill level (0-4), resulting in silent logic bugs.
//!
//! This module uses Rust's **Newtype Pattern** to encapsulate raw numbers into
//! strictly validated, distinct types. By doing so, we shift the responsibility of
//! validation from the *callee* (checking bounds on every function call) to the
//! *caller* (constructing the type once). Once you have a `SkillLevel`, the compiler
//! guarantees it is valid, and you can never accidentally mix it up with a `PlayerNum`.
//!
//! Each newtype provides:
//! - A `const fn new(v) -> Option<Self>` constructor that strictly enforces spec bounds.
//! - A `const fn raw(self) -> T` accessor to retrieve the underlying primitive.
//!
//! These are candidates for Verus `#[invariant]` annotations.

/// Light level 0..=255.
///
/// Doom's software renderer processes lighting in bands, but the map format
/// defines brightness as a value from 0 (pitch black) to 255 (full bright).
///
/// # Examples
/// ```
/// use doom_types::primitives::Brightness;
///
/// let dark = Brightness::new(0).unwrap();
/// assert_eq!(dark.raw(), 0);
///
/// let bright = Brightness::MAX;
/// assert_eq!(bright.raw(), 255);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Brightness(u8);

impl Brightness {
    /// Create, returning `None` if out of range (always succeeds for `u8`).
    #[inline]
    pub const fn new(v: u8) -> Option<Self> {
        Some(Self(v))
    }

    /// # Examples
    /// ```
    /// use doom_types::primitives::Brightness;
    /// let b = Brightness::new(128).unwrap();
    /// assert_eq!(b.raw(), 128);
    /// ```
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
///
/// Defines special behaviors for a sector (e.g., blinking lights, damage floors).
/// Standard vanilla Doom expects this to be in the 0 to 16 range.
///
/// # Examples
/// ```
/// use doom_types::primitives::SectorSpecial;
///
/// // A normal, non-special sector.
/// let normal = SectorSpecial::NORMAL;
/// assert_eq!(normal.raw(), 0);
///
/// // Constructing a valid special (e.g., 9 = Secret).
/// let secret = SectorSpecial::new(9).unwrap();
/// assert_eq!(secret.raw(), 9);
///
/// // Invalid specials are rejected at construction.
/// assert!(SectorSpecial::new(20).is_none());
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SectorSpecial(u8);

impl SectorSpecial {
    /// Returns `None` if `v > 16`.
    #[inline]
    pub const fn new(v: u8) -> Option<Self> {
        if v <= 16 { Some(Self(v)) } else { None }
    }

    /// # Examples
    /// ```
    /// use doom_types::primitives::SectorSpecial;
    /// let s = SectorSpecial::NORMAL;
    /// assert_eq!(s.raw(), 0);
    /// ```
    #[inline]
    pub const fn raw(self) -> u8 {
        self.0
    }

    /// Normal sector (no special).
    pub const NORMAL: Self = Self(0);
}

/// Skill level 0..=4.
///
/// Represents the five classic Doom difficulty levels.
/// Constraining this to 0-4 prevents out-of-bounds array access when spawning
/// entities (which often have `skill` bit flags).
///
/// # Examples
/// ```
/// use doom_types::primitives::SkillLevel;
///
/// // "Hurt Me Plenty"
/// let hmp = SkillLevel::HMP;
/// assert_eq!(hmp.raw(), 2);
///
/// // Invalid skill level.
/// assert!(SkillLevel::new(5).is_none());
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SkillLevel(u8);

impl SkillLevel {
    #[inline]
    pub const fn new(v: u8) -> Option<Self> {
        if v <= 4 { Some(Self(v)) } else { None }
    }

    /// # Examples
    /// ```
    /// use doom_types::primitives::SkillLevel;
    /// let s = SkillLevel::UV;
    /// assert_eq!(s.raw(), 3);
    /// ```
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
///
/// Doom supports a maximum of 4 players (0-3). This newtype prevents targeting
/// a non-existent player in multiplayer routines.
///
/// # Examples
/// ```
/// use doom_types::primitives::PlayerNum;
///
/// let p1 = PlayerNum::new(0).unwrap();
/// assert_eq!(p1.raw(), 0);
///
/// // Player 5 does not exist.
/// assert!(PlayerNum::new(4).is_none());
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PlayerNum(u8);

impl PlayerNum {
    #[inline]
    pub const fn new(v: u8) -> Option<Self> {
        if v <= 3 { Some(Self(v)) } else { None }
    }

    /// # Examples
    /// ```
    /// use doom_types::primitives::PlayerNum;
    /// let p = PlayerNum::new(2).unwrap();
    /// assert_eq!(p.raw(), 2);
    /// ```
    #[inline]
    pub const fn raw(self) -> u8 {
        self.0
    }
}

/// Map vertex coordinate — spec range [-32768, 32767] = `i16`.
///
/// Doom maps are constructed using a 16-bit grid. This strictly enforces the
/// coordinate space for map geometry.
///
/// # Examples
/// ```
/// use doom_types::primitives::Coord;
///
/// let x = Coord::new(-1024);
/// assert_eq!(x.raw(), -1024);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Coord(pub i16);

impl Coord {
    #[inline]
    pub const fn new(v: i16) -> Self {
        Self(v)
    }

    /// # Examples
    /// ```
    /// use doom_types::primitives::Coord;
    /// let c = Coord::new(32);
    /// assert_eq!(c.raw(), 32);
    /// ```
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

    #[test]
    fn test_coverage_primitives() {
        let b = Brightness::new(128).unwrap();
        assert_eq!(b.raw(), 128);
        assert_eq!(b, Brightness::new(128).unwrap());
        #[allow(clippy::clone_on_copy)]
        let b2 = b.clone();
        assert_eq!(format!("{:?}", b), format!("{:?}", b2));
        assert!(b <= Brightness::MAX);
        assert!(b >= Brightness::MIN);

        let s = SectorSpecial::NORMAL;
        assert_eq!(s.raw(), 0);
        assert_eq!(s, SectorSpecial::new(0).unwrap());
        #[allow(clippy::clone_on_copy)]
        let s2 = s.clone();
        assert_eq!(format!("{:?}", s), format!("{:?}", s2));

        let sl = SkillLevel::UV;
        assert_eq!(sl.raw(), 3);
        assert_eq!(sl, SkillLevel::new(3).unwrap());
        assert_eq!(SkillLevel::ITYTD.raw(), 0);
        assert_eq!(SkillLevel::HNTR.raw(), 1);
        assert_eq!(SkillLevel::HMP.raw(), 2);
        assert_eq!(SkillLevel::NM.raw(), 4);
        #[allow(clippy::clone_on_copy)]
        let sl2 = sl.clone();
        assert_eq!(format!("{:?}", sl), format!("{:?}", sl2));
        assert!(sl <= SkillLevel::NM);

        let p = PlayerNum::new(2).unwrap();
        assert_eq!(p.raw(), 2);
        assert_eq!(p, PlayerNum::new(2).unwrap());
        #[allow(clippy::clone_on_copy)]
        let p2 = p.clone();
        assert_eq!(format!("{:?}", p), format!("{:?}", p2));
        assert!(p >= PlayerNum::new(0).unwrap());

        let c = Coord::new(32);
        assert_eq!(c.raw(), 32);
        assert_eq!(c, Coord::new(32));
        #[allow(clippy::clone_on_copy)]
        let c2 = c.clone();
        assert_eq!(format!("{:?}", c), format!("{:?}", c2));
        assert!(c <= Coord::new(100));
    }
}
