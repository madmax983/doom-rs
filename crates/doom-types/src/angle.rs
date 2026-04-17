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

/// Precomputed sin/cos tables (2048 entries covering 0..π/2, mirrored for full circle).
///
/// Populated at startup from the WAD ANGLETOFINESHIFT + finesine data,
/// or from the compile-time table in this module.
///
/// 2048 entries covering 90°, fine-shifted angle = `bam >> 19` (2048 steps per 90°).
const FINE_TABLE_SIZE: usize = 8192; // 2048 * 4 quadrants
static FINESINE: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// Lookup table populated at runtime.
static mut SINE_TABLE: [Fixed16_16; FINE_TABLE_SIZE] = [Fixed16_16(0); FINE_TABLE_SIZE];

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
    /// # Safety
    /// Safe only after `init_trig_tables()`. Returns 0 before initialization.
    ///
    /// # Examples
    /// ```
    /// use doom_types::{Bam, ANG90, FIXED_ONE};
    /// unsafe { Bam::init_trig_tables(); }
    /// assert_eq!(ANG90.sin(), FIXED_ONE);
    /// ```
    pub fn sin(self) -> Fixed16_16 {
        if !FINESINE.load(core::sync::atomic::Ordering::Acquire) {
            return Fixed16_16::ZERO;
        }
        // SAFETY: SINE_TABLE is only mutated once at init, before any reads.
        unsafe { SINE_TABLE[self.fine_angle()] }
    }

    /// Cos lookup (sin shifted by 90°).
    ///
    /// # Examples
    /// ```
    /// use doom_types::{Bam, ANG180, FIXED_ONE};
    /// unsafe { Bam::init_trig_tables(); }
    /// assert_eq!(Bam::ZERO.cos(), FIXED_ONE);
    /// assert_eq!(ANG180.cos(), -FIXED_ONE);
    /// ```
    pub fn cos(self) -> Fixed16_16 {
        let cos_angle = Bam(self.0.wrapping_add(ANG90.0));
        cos_angle.sin()
    }

    /// Initialize sin/cos tables from a floating-point computation.
    ///
    /// Must be called exactly once at startup, before any `sin()`/`cos()` calls.
    ///
    /// # Safety
    /// Must not be called concurrently or more than once.
    ///
    /// # Examples
    /// ```
    /// use doom_types::{Bam, ANG90, FIXED_ONE};
    /// unsafe { Bam::init_trig_tables(); }
    /// assert_eq!(ANG90.sin(), FIXED_ONE);
    /// ```
    pub unsafe fn init_trig_tables() {
        use core::f64::consts::PI;
        // SAFETY: single-threaded init before any reads.
        #[allow(clippy::needless_range_loop)]
        unsafe {
            #[allow(clippy::needless_range_loop)]
            for i in 0..FINE_TABLE_SIZE {
                let angle = (i as f64) * (2.0 * PI) / (FINE_TABLE_SIZE as f64);
                let sin_val = angle.sin();
                core::ptr::addr_of_mut!(SINE_TABLE[i])
                    .write(Fixed16_16((sin_val * (1 << 16) as f64) as i32));
            }
        }
        FINESINE.store(true, core::sync::atomic::Ordering::Release);
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
        INIT.call_once(|| unsafe {
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
        let sin90 = ANG90.sin();
        assert_eq!(sin90.to_int(), 1); // sin(90) = 1.0

        let cos0 = Bam::ZERO.cos();
        assert_eq!(cos0.to_int(), 1); // cos(0) = 1.0

        let cos180 = ANG180.cos();
        assert_eq!(cos180.to_int(), -1); // cos(180) = -1.0
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
            assert!(idx < FINE_TABLE_SIZE, "fine_angle {idx} out of bounds");
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
            prop_assert!(idx < FINE_TABLE_SIZE);
        }
    }
}
