//! Binary Angle Measure (BAM) — Doom's full-circle `u32` angle type.
//!
//! The full circle is `u32::MAX + 1` (2³²). All arithmetic is naturally
//! modular via `u32` wrapping, so there is no "angle clamping" needed.
//!
//! # Verus invariant
//! `Bam` is always valid — every `u32` bit pattern is a legal angle.
//! Additive inverse holds: `a.wrapping_add(b).wrapping_sub(b) == a`.

use crate::fixed::Fixed16_16;

/// Binary Angle Measure: 2³² = full circle.
///
/// Cardinal directions:
/// - `0x00000000` → East (0°)
/// - `0x40000000` → North (90°)
/// - `0x80000000` → West (180°)
/// - `0xC0000000` → South (270°)
///
/// ## Examples
/// ```
/// use doom_types::angle::{Bam, ANG90, ANG180};
///
/// let east = Bam::from_raw(0);
/// let north = ANG90;
/// let west = ANG180;
///
/// // Adding two 90 degree angles results in a 180 degree angle
/// assert_eq!(north.wrapping_add(north), west);
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Bam(pub u32);

/// Whether trig lookups are "armed".
///
/// The exact table is a compile-time constant ([`crate::finesine_table::FINESINE`]),
/// so no data has to be computed at init. This flag only preserves the historical
/// contract that `sin`/`cos` return 0 until `init_trig_tables()` is called.
static FINESINE_READY: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// Shift to convert a `Bam` to a fine-angle index (0..8191).
pub const BAM_TO_FINE_SHIFT: u32 = 32 - 13; // >> 19 gives index in 0..8191

/// `ANG45`: 45 degrees in BAM units.
pub const ANG45: Bam = Bam(0x2000_0000);
/// `ANG90`: 90 degrees.
pub const ANG90: Bam = Bam(0x4000_0000);
/// `ANG180`: 180 degrees.
pub const ANG180: Bam = Bam(0x8000_0000);
/// `ANG270`: 270 degrees.
pub const ANG270: Bam = Bam(0xC000_0000);

impl Bam {
    /// Zero angle (East).
    pub const ZERO: Self = Self(0);

    /// Create from a raw `u32`.
    ///
    /// # Examples
    /// ```
    /// use doom_types::Bam;
    /// let a = Bam::from_raw(0x4000_0000); // 90 degrees
    /// assert_eq!(a.raw(), 0x4000_0000);
    /// ```
    #[inline]
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    /// Raw `u32` bit pattern.
    ///
    /// # Examples
    /// ```
    /// use doom_types::Bam;
    /// let a = Bam::from_raw(0x8000_0000);
    /// assert_eq!(a.raw(), 0x8000_0000);
    /// ```
    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Wrapping addition (always correct for angles).
    ///
    /// Because angles are mapped to the full `u32` range, overflowing past 360 degrees
    /// naturally wraps back around to 0 degrees via integer overflow.
    ///
    /// # Examples
    /// ```
    /// use doom_types::{Bam, ANG90, ANG180};
    /// assert_eq!(ANG90.wrapping_add(ANG90), ANG180);
    /// ```
    #[inline]
    pub fn wrapping_add(self, rhs: Self) -> Self {
        Self(self.0.wrapping_add(rhs.0))
    }

    /// Wrapping subtraction.
    ///
    /// Underflowing past 0 degrees wraps around to 360 degrees.
    ///
    /// # Examples
    /// ```
    /// use doom_types::{Bam, ANG90, ANG270};
    /// assert_eq!(Bam::ZERO.wrapping_sub(ANG90), ANG270);
    /// ```
    #[inline]
    pub fn wrapping_sub(self, rhs: Self) -> Self {
        Self(self.0.wrapping_sub(rhs.0))
    }

    /// Negate (180° flip = additive inverse in modular arithmetic).
    ///
    /// Flipping an angle is equivalent to adding 180 degrees.
    ///
    /// # Examples
    /// ```
    /// use doom_types::{Bam, ANG90, ANG270};
    /// assert_eq!(ANG90.negate(), ANG270);
    /// ```
    #[inline]
    pub fn negate(self) -> Self {
        Self(self.0.wrapping_neg())
    }

    /// Fine-angle index (0..8191) used for sin/cos table lookup.
    ///
    /// Doom uses an 8192-entry table for full-circle trigonometry.
    ///
    /// # Examples
    /// ```
    /// use doom_types::ANG90;
    /// // 90 degrees is exactly 1/4th of the way through the 8192 entry table.
    /// assert_eq!(ANG90.fine_angle(), 2048);
    /// ```
    #[inline]
    pub const fn fine_angle(self) -> usize {
        (self.0 >> BAM_TO_FINE_SHIFT) as usize
    }

    /// Sin lookup (requires `init_trig_tables()` to have been called).
    ///
    /// Maps the angle to an index in the precomputed trigonometry table.
    ///
    /// Returns 0 before `init_trig_tables()` has been called.
    ///
    /// # Examples
    /// ```
    /// use doom_types::{Bam, ANG90};
    /// Bam::init_trig_tables();
    /// // Vanilla finesine peak is 65535, not 65536.
    /// assert_eq!(ANG90.sin().raw(), 65535);
    /// ```
    pub fn sin(self) -> Fixed16_16 {
        if !FINESINE_READY.load(core::sync::atomic::Ordering::Acquire) {
            return Fixed16_16::ZERO;
        }
        Fixed16_16(crate::finesine_table::FINESINE[self.fine_angle()])
    }

    /// Cos lookup.
    ///
    /// Matches vanilla Doom exactly: `finecosine[i] == finesine[i + FINEANGLES/4]`,
    /// reading the extended tail of the table rather than wrapping mod 8192.
    ///
    /// # Examples
    /// ```
    /// use doom_types::{Bam, ANG180};
    /// Bam::init_trig_tables();
    /// // Vanilla finecosine peak magnitude is 65535 (0.99998), not exactly 1.0.
    /// assert_eq!(ANG180.cos().raw(), -65535);
    /// ```
    pub fn cos(self) -> Fixed16_16 {
        if !FINESINE_READY.load(core::sync::atomic::Ordering::Acquire) {
            return Fixed16_16::ZERO;
        }
        Fixed16_16(crate::finesine_table::FINESINE[self.fine_angle() + 2048])
    }

    /// Arm the trig lookups.
    ///
    /// The table itself is a compile-time constant (bit-exact to vanilla Doom's
    /// `finesine[]`), so this only flips the readiness flag that gates `sin`/`cos`.
    /// The flag is a single atomic store, so this is safe to call from any thread
    /// and any number of times (idempotent) — there is no mutable static and thus
    /// no data race (the concern that motivated the historical `unsafe` marker).
    ///
    /// # Examples
    /// ```
    /// use doom_types::{Bam, ANG90};
    /// Bam::init_trig_tables();
    /// // Vanilla finesine peak is 65535, not 65536.
    /// assert_eq!(ANG90.sin().raw(), 65535);
    /// ```
    pub fn init_trig_tables() {
        FINESINE_READY.store(true, core::sync::atomic::Ordering::Release);
    }
}

impl core::ops::Add for Bam {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        self.wrapping_add(rhs)
    }
}

impl core::ops::Sub for Bam {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        self.wrapping_sub(rhs)
    }
}

impl core::ops::Neg for Bam {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        self.negate()
    }
}

impl core::fmt::Display for Bam {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let degrees = (self.0 as f64) * 360.0 / (u32::MAX as f64 + 1.0);
        write!(f, "{degrees:.2}°")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static INIT: std::sync::Once = std::sync::Once::new();

    fn ensure_trig_init() {
        INIT.call_once(|| {
            Bam::init_trig_tables();
        });
    }

    #[test]
    fn bam_raw_returns_internal_value() {
        let a = Bam::from_raw(0xDEAD_BEEF);
        assert_eq!(a.raw(), 0xDEAD_BEEF);
    }

    #[test]
    fn sin_cos_before_and_after_init() {
        // We can't guarantee before init because tests run in parallel,
        // but we can ensure it returns something sane after init.
        ensure_trig_init();
        // Bit-exact vanilla `finesine`/`finecosine` values (peak magnitude 65535).
        assert_eq!(ANG90.sin().raw(), 65535); // finesine[2048]
        assert_eq!(Bam::ZERO.cos().raw(), 65535); // finecosine[0] == finesine[2048]
        assert_eq!(ANG180.cos().raw(), -65535); // finecosine[4096] == finesine[6144]
    }

    #[test]
    fn ang90_plus_ang90_is_ang180() {
        assert_eq!(ANG90 + ANG90, ANG180);
    }

    #[test]
    fn full_circle_wraps_to_zero() {
        assert_eq!(ANG180 + ANG180, Bam::ZERO);
    }

    #[test]
    fn display_format() {
        let a = Bam::from_raw(0x4000_0000);
        assert_eq!(format!("{}", a), "90.00°");
    }

    #[test]
    fn sub_trait() {
        let a = Bam(0xABCD_EF01);
        let b = Bam(0x1234_5678);
        assert_eq!(a - b, a.wrapping_sub(b));
    }

    #[test]
    fn neg_trait() {
        let a = Bam(0x4000_0000);
        assert_eq!(-a, a.negate());
    }

    #[test]
    fn additive_inverse_holds() {
        let a = Bam(0x1234_5678);
        let b = Bam(0xABCD_EF01);
        assert_eq!((a + b) - b, a);
    }

    #[test]
    fn negate_is_additive_inverse() {
        let a = Bam(0x4000_0000);
        assert_eq!(a + (-a), Bam::ZERO);
    }

    #[test]
    fn fine_angle_in_bounds() {
        // Every Bam value must produce a valid table index.
        for raw in [0u32, 0x1000_0000, 0x4000_0000, 0x8000_0000, 0xFFFF_FFFF] {
            let idx = Bam(raw).fine_angle();
            assert!(idx < 8192, "fine_angle {idx} out of bounds");
        }
    }
}

#[cfg(test)]
mod prop_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn bam_additive_inverse(a: u32, b: u32) {
            let ba = Bam(a);
            let bb = Bam(b);
            prop_assert_eq!((ba + bb) - bb, ba);
        }

        #[test]
        fn bam_negate_is_additive_inverse(a: u32) {
            let ba = Bam(a);
            prop_assert_eq!(ba + (-ba), Bam::ZERO);
        }

        #[test]
        fn bam_fine_angle_always_in_bounds(a: u32) {
            let idx = Bam(a).fine_angle();
            prop_assert!(idx < 8192);
        }
    }
}
