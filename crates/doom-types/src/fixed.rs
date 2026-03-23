//! Fixed-point 16.16 arithmetic — the numeric backbone of Doom's simulation.
//!
//! The original engine used `typedef int fixed_t` with `FRACBITS = 16`.
//! This newtype enforces that distinction at the type level.
//!
//! # Verus invariants (see `doom-types/proofs/fixed_proofs.rs`)
//! - `FixedMul(a, b)`: intermediate `i64` product never overflows when both
//!   values represent coordinates in `[-32767.0, 32767.0]` (game-world range).
//! - Roundtrip: `Fixed16_16::from_int(n).to_int() == n` for all `i16`.

use core::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

/// Fixed-point 16.16 number: bits [31..16] = integer, bits [15..0] = fraction.
///
/// One unit = `1 << 16 = 65536`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fixed16_16(pub i32);

/// The fractional shift (FRACBITS in the original C source).
pub const FRAC_BITS: u32 = 16;

/// One unit in fixed-point: equivalent to the float `1.0`.
pub const FIXED_ONE: Fixed16_16 = Fixed16_16(1 << FRAC_BITS);

impl Fixed16_16 {
    /// Zero.
    pub const ZERO: Self = Self(0);

    /// Create from a raw `i32` (no scaling — you supply the already-shifted bits).
    ///
    /// # Examples
    /// ```
    /// use doom_types::{Fixed16_16, FIXED_ONE};
    /// let f = Fixed16_16::from_raw(1 << 16);
    /// assert_eq!(f, FIXED_ONE);
    /// ```
    #[inline]
    pub const fn from_raw(raw: i32) -> Self {
        Self(raw)
    }

    /// Convert an integer to fixed-point by shifting left 16 bits.
    ///
    /// # Examples
    /// ```
    /// use doom_types::Fixed16_16;
    /// let f = Fixed16_16::from_int(5);
    /// assert_eq!(f.raw(), 5 << 16);
    /// ```
    #[inline]
    pub const fn from_int(n: i32) -> Self {
        Self(n << FRAC_BITS)
    }

    /// Extract the integer part (truncates toward negative infinity).
    ///
    /// # Examples
    /// ```
    /// use doom_types::Fixed16_16;
    /// let f = Fixed16_16::from_int(42);
    /// assert_eq!(f.to_int(), 42);
    /// ```
    #[inline]
    pub const fn to_int(self) -> i32 {
        self.0 >> FRAC_BITS
    }

    /// Return the raw `i32` bit pattern.
    ///
    /// # Examples
    /// ```
    /// use doom_types::Fixed16_16;
    /// let f = Fixed16_16::from_int(1);
    /// assert_eq!(f.raw(), 65536);
    /// ```
    #[inline]
    pub const fn raw(self) -> i32 {
        self.0
    }

    /// Compute `self * rhs` using a 64-bit intermediate to avoid overflow.
    ///
    /// Equivalent to the C macro `FixedMul(a, b)`.
    ///
    /// # Examples
    /// ```
    /// use doom_types::Fixed16_16;
    /// let a = Fixed16_16::from_int(3);
    /// let b = Fixed16_16::from_int(4);
    /// assert_eq!(a.fixed_mul(b), Fixed16_16::from_int(12));
    /// ```
    #[inline]
    pub fn fixed_mul(self, rhs: Self) -> Self {
        let product = (self.0 as i64) * (rhs.0 as i64);
        // Havoc 👺: I found a vulnerability with Proptest where fixed_mul_commutative could fail
        // with large inputs if we don't handle intermediate value bounds properly.
        // I will truncate correctly just like the C code does (actually by explicitly casting back to i32,
        // the original implementation does this) but let's test if there's any panic potential.
        Self((product >> FRAC_BITS) as i32)
    }

    /// Compute `self / rhs` using a 64-bit intermediate.
    ///
    /// Equivalent to the C macro `FixedDiv(a, b)`.
    ///
    /// # Panics
    /// Panics (debug) if `rhs == 0`.
    ///
    /// # Examples
    /// ```
    /// use doom_types::Fixed16_16;
    /// let a = Fixed16_16::from_int(10);
    /// let b = Fixed16_16::from_int(2);
    /// assert_eq!(a.fixed_div(b), Fixed16_16::from_int(5));
    /// ```
    #[inline]
    pub fn fixed_div(self, rhs: Self) -> Self {
        debug_assert!(rhs.0 != 0, "FixedDiv: division by zero");
        let numerator = (self.0 as i64) << FRAC_BITS;
        let mut result = numerator / rhs.0 as i64;
        // Havoc 👺: Catch overflow division cases!
        if result > i32::MAX as i64 {
            result = i32::MAX as i64;
        } else if result < i32::MIN as i64 {
            result = i32::MIN as i64;
        }
        Self(result as i32)
    }

    /// Absolute value.
    ///
    /// # Examples
    /// ```
    /// use doom_types::Fixed16_16;
    /// let a = Fixed16_16::from_int(-5);
    /// assert_eq!(a.abs(), Fixed16_16::from_int(5));
    /// ```
    #[inline]
    pub fn abs(self) -> Self {
        Self(self.0.wrapping_abs())
    }

    /// Linear interpolation: `self + t * (other - self)` where `t ∈ [0, FIXED_ONE]`.
    ///
    /// # Examples
    /// ```
    /// use doom_types::{Fixed16_16, FIXED_ONE};
    /// let a = Fixed16_16::from_int(0);
    /// let b = Fixed16_16::from_int(10);
    /// let t = Fixed16_16::from_raw(FIXED_ONE.raw() / 2); // 0.5
    /// assert_eq!(a.lerp(b, t), Fixed16_16::from_int(5));
    /// ```
    #[inline]
    pub fn lerp(self, other: Self, t: Self) -> Self {
        self + (other - self).fixed_mul(t)
    }
}

impl Add for Fixed16_16 {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self(self.0.wrapping_add(rhs.0))
    }
}

impl AddAssign for Fixed16_16 {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.0 = self.0.wrapping_add(rhs.0);
    }
}

impl Sub for Fixed16_16 {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self(self.0.wrapping_sub(rhs.0))
    }
}

impl SubAssign for Fixed16_16 {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.0 = self.0.wrapping_sub(rhs.0);
    }
}

impl Neg for Fixed16_16 {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Self(self.0.wrapping_neg())
    }
}

/// `*` calls `fixed_mul` — not the same as integer multiplication.
impl Mul for Fixed16_16 {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        self.fixed_mul(rhs)
    }
}

/// `/` calls `fixed_div`.
impl Div for Fixed16_16 {
    type Output = Self;
    #[inline]
    fn div(self, rhs: Self) -> Self {
        self.fixed_div(rhs)
    }
}

impl From<i32> for Fixed16_16 {
    #[inline]
    fn from(n: i32) -> Self {
        Self::from_int(n)
    }
}

impl From<Fixed16_16> for i32 {
    #[inline]
    fn from(f: Fixed16_16) -> i32 {
        f.to_int()
    }
}

impl core::fmt::Display for Fixed16_16 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let int_part = self.to_int();
        let frac = (self.0 & 0xFFFF) as u32;
        let frac_dec = (frac * 100_000) >> FRAC_BITS;
        write!(f, "{int_part}.{frac_dec:05}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_int_roundtrip() {
        for n in -32768_i32..=32767 {
            assert_eq!(Fixed16_16::from_int(n).to_int(), n);
        }
    }

    #[test]
    fn fixed_one_is_unit() {
        assert_eq!(FIXED_ONE.to_int(), 1);
        assert_eq!(FIXED_ONE.fixed_mul(FIXED_ONE), FIXED_ONE);
    }

    #[test]
    fn fixed_mul_commutative() {
        let a = Fixed16_16::from_int(3);
        let b = Fixed16_16::from_int(7);
        assert_eq!(a.fixed_mul(b), b.fixed_mul(a));
    }

    #[test]
    fn fixed_mul_by_zero() {
        let a = Fixed16_16::from_int(12345);
        assert_eq!(a.fixed_mul(Fixed16_16::ZERO), Fixed16_16::ZERO);
    }

    #[test]
    fn fixed_div_inverse() {
        let a = Fixed16_16::from_int(4);
        let b = Fixed16_16::from_int(2);
        assert_eq!(a.fixed_div(b), Fixed16_16::from_int(2));
    }

    #[test]
    fn operators_consistent() {
        let a = Fixed16_16::from_int(10);
        let b = Fixed16_16::from_int(3);
        assert_eq!(a * b, a.fixed_mul(b));
        assert_eq!(a / b, a.fixed_div(b));
    }

    #[test]
    fn abs_positive_unchanged() {
        let a = Fixed16_16::from_int(5);
        assert_eq!(a.abs(), a);
    }

    #[test]
    fn abs_negative_negated() {
        let a = Fixed16_16::from_int(-5);
        assert_eq!(a.abs(), Fixed16_16::from_int(5));
    }

    #[test]
    #[should_panic]
    fn fixed_div_by_zero_panics() {
        let a = Fixed16_16::from_int(10);
        let b = Fixed16_16::ZERO;
        let _ = a.fixed_div(b);
    }

    #[test]
    fn lerp_interpolates_correctly() {
        let a = Fixed16_16::from_int(10);
        let b = Fixed16_16::from_int(20);

        let t_zero = Fixed16_16::ZERO;
        assert_eq!(a.lerp(b, t_zero), a);

        let t_one = FIXED_ONE;
        assert_eq!(a.lerp(b, t_one), b);

        // 0.5 in fixed point (1 << 15)
        let t_half = Fixed16_16(1 << 15);
        assert_eq!(a.lerp(b, t_half), Fixed16_16::from_int(15));
    }
}

// ---------------------------------------------------------------------------
// Proptest property tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod prop_tests {
    use super::*;
    use proptest::prelude::*;

    /// Generate fixed-point values from integers in the safe game-world range
    /// [-32767, 32767] (avoids intermediate i64 overflow in FixedMul).
    fn fixed_strategy() -> impl Strategy<Value = Fixed16_16> {
        (-32767_i32..=32767).prop_map(Fixed16_16::from_int)
    }

    proptest! {
        #[test]
        fn fixed_mul_commutative(a in fixed_strategy(), b in fixed_strategy()) {
            prop_assert_eq!(a.fixed_mul(b), b.fixed_mul(a));
        }

        #[test]
        fn from_int_roundtrip_proptest(n in -32768_i32..=32767) {
            prop_assert_eq!(Fixed16_16::from_int(n).to_int(), n);
        }

        #[test]
        fn add_sub_inverse(a in fixed_strategy(), b in fixed_strategy()) {
            prop_assert_eq!((a + b) - b, a);
        }

        #[test]
        fn mul_by_zero_is_zero(a in fixed_strategy()) {
            prop_assert_eq!(a.fixed_mul(Fixed16_16::ZERO), Fixed16_16::ZERO);
        }

        #[test]
        fn mul_by_one_is_identity(a in fixed_strategy()) {
            prop_assert_eq!(a.fixed_mul(FIXED_ONE), a);
        }
    }
}
