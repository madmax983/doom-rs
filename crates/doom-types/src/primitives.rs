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
///
/// ## Examples
/// ```
/// use doom_types::primitives::SectorSpecial;
///
/// let special = SectorSpecial::new(9).unwrap();
/// assert_eq!(special.raw(), 9);
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
    /// Validates and constructs a `SkillLevel`.
    ///
    /// The Doom engine requires skill levels to be strictly bounded between 0 and 4.
    /// This prevents out-of-bounds array access when querying skill-dependent tables,
    /// such as entity spawn flags or skill-specific modifications. Returns `None` if
    /// the provided value falls outside the classic 0-4 range.
    ///
    /// # Examples
    /// ```
    /// use doom_types::primitives::SkillLevel;
    ///
    /// // Valid skill level.
    /// let skill = SkillLevel::new(2).unwrap();
    /// assert_eq!(skill.raw(), 2);
    /// ```
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

    /// "I'm Too Young To Die" - The easiest difficulty, featuring reduced damage and doubled ammo.
    pub const ITYTD: Self = Self(0);
    /// "Hey, Not Too Rough" - A gentle introduction, equivalent to normal monster placement with standard ammo.
    pub const HNTR: Self = Self(1);
    /// "Hurt Me Plenty" - The default Doom experience, offering a balanced challenge.
    pub const HMP: Self = Self(2);
    /// "Ultra-Violence" - For seasoned players, maximizing monster spawns and aggressiveness.
    pub const UV: Self = Self(3);
    /// "Nightmare!" - A brutal, fast-paced challenge with respawning monsters and double ammo.
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
///
/// ## Examples
/// ```
/// use doom_types::primitives::PlayerNum;
///
/// let player = PlayerNum::new(0).unwrap();
/// assert_eq!(player.raw(), 0);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PlayerNum(u8);

impl PlayerNum {
    /// Validates and constructs a `PlayerNum`.
    ///
    /// The classic Doom engine supports a maximum of 4 concurrent players, indexed 0 through 3.
    /// By enforcing this constraint at construction time, networking and multiplayer logic can
    /// safely assume a valid player target. Returns `None` if `v > 3`.
    ///
    /// # Examples
    /// ```
    /// use doom_types::primitives::PlayerNum;
    ///
    /// // Creating the host player (Player 1, index 0).
    /// let player = PlayerNum::new(0).unwrap();
    /// assert_eq!(player.raw(), 0);
    /// ```
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
///
/// ## Examples
/// ```
/// use doom_types::primitives::Coord;
///
/// let coord = Coord::new(100);
/// assert_eq!(coord.raw(), 100);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Coord(pub i16);

impl Coord {
    /// Constructs a vertex `Coord` in map space.
    ///
    /// Doom maps exist on a strictly defined 16-bit grid. By explicitly wrapping the primitive `i16`,
    /// this type signals the intent that the coordinate is part of the map geometry space.
    ///
    /// # Examples
    /// ```
    /// use doom_types::primitives::Coord;
    ///
    /// // Map coordinates are public fields.
    /// let coord = Coord::new(1024);
    /// assert_eq!(coord.0, 1024);
    /// ```
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
    fn raw_methods_return_internal_value() {
        let b = Brightness::new(128).unwrap();
        assert_eq!(b.raw(), 128);

        let s = SectorSpecial::NORMAL;
        assert_eq!(s.raw(), 0);
        let s2 = SectorSpecial::new(5).unwrap();
        assert_eq!(s2.raw(), 5);

        let skill = SkillLevel::UV;
        assert_eq!(skill.raw(), 3);
        let skill2 = SkillLevel::new(2).unwrap();
        assert_eq!(skill2.raw(), 2);

        let p = PlayerNum::new(2).unwrap();
        assert_eq!(p.raw(), 2);
    }

    #[test]
    fn coord_new_and_raw() {
        let c = Coord::new(-1024);
        assert_eq!(c.raw(), -1024);
        let c2 = Coord::new(32767);
        assert_eq!(c2.raw(), 32767);
    }

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
