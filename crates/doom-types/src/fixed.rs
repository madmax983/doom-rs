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
///
/// ## Examples
/// ```
/// use doom_types::fixed::Fixed16_16;
///
/// let a = Fixed16_16::from_int(2);
/// let b = Fixed16_16::from_int(3);
/// assert_eq!(a + b, Fixed16_16::from_int(5));
/// ```
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

    /// Compute `self / rhs`, an exact port of vanilla Doom's `FixedDiv`
    /// (`m_fixed.c`):
    ///
    /// ```c
    /// fixed_t FixedDiv(fixed_t a, fixed_t b) {
    ///     if ((abs(a) >> 14) >= abs(b))
    ///         return (a ^ b) < 0 ? INT_MIN : INT_MAX;
    ///     return (fixed_t)(((int64_t) a << 16) / b);
    /// }
    /// ```
    ///
    /// The overflow guard doubles as the divide-by-zero guard (`abs(b) == 0`
    /// always triggers it), so this never panics and never divides by zero —
    /// it saturates to `INT_MIN`/`INT_MAX` according to the sign of `a ^ b`,
    /// matching vanilla bit-for-bit (no clamping of an out-of-range quotient).
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
        let a = self.0;
        let b = rhs.0;
        // `(abs(a) >> 14) >= abs(b)` — unsigned_abs avoids the INT_MIN abs UB.
        if (a.unsigned_abs() >> 14) >= b.unsigned_abs() {
            if (a ^ b) < 0 {
                Self(i32::MIN)
            } else {
                Self(i32::MAX)
            }
        } else {
            Self((((a as i64) << FRAC_BITS) / (b as i64)) as i32)
        }
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
    fn fixed_raw_returns_internal_value() {
        let f = Fixed16_16::from_int(1);
        assert_eq!(f.raw(), 65536);
        let f2 = Fixed16_16::from_raw(0xABCD);
        assert_eq!(f2.raw(), 0xABCD);
    }

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
    fn display_format() {
        let a = Fixed16_16::from_int(42) + Fixed16_16::from_raw(1 << 15);
        assert_eq!(format!("{}", a), "42.50000");
    }

    #[test]
    fn traits_add_sub_assign() {
        let mut a = Fixed16_16::from_int(5);
        a += Fixed16_16::from_int(3);
        assert_eq!(a, Fixed16_16::from_int(8));
        a -= Fixed16_16::from_int(4);
        assert_eq!(a, Fixed16_16::from_int(4));
    }

    #[test]
    fn traits_neg_mul_div() {
        let a = Fixed16_16::from_int(5);
        assert_eq!(-a, Fixed16_16::from_int(-5));
        let b = Fixed16_16::from_int(2);
        assert_eq!(a * b, Fixed16_16::from_int(10));
        assert_eq!(a / b, Fixed16_16::from_int(2) + Fixed16_16(1 << 15));
    }

    #[test]
    fn traits_from() {
        let a: Fixed16_16 = 42.into();
        assert_eq!(a, Fixed16_16::from_int(42));
        let b: i32 = a.into();
        assert_eq!(b, 42);
    }

    #[test]
    fn fixed_div_overflow_clamping() {
        let a = Fixed16_16::from_int(32767);
        let b = Fixed16_16::from_raw(1); // very small positive
        assert_eq!(a.fixed_div(b), Fixed16_16::from_raw(i32::MAX));

        let c = Fixed16_16::from_int(-32768);
        let d = Fixed16_16::from_raw(1); // very small positive
        assert_eq!(c.fixed_div(d), Fixed16_16::from_raw(i32::MIN));

        let min_val = Fixed16_16::from_raw(i32::MIN);
        let neg_one = Fixed16_16::from_int(-1);
        assert_eq!(min_val.fixed_div(neg_one), Fixed16_16::from_raw(i32::MAX));

        let min_val2 = Fixed16_16::from_raw(i32::MIN);
        let neg_one2 = Fixed16_16::from_raw(-1);
        assert_eq!(min_val2.fixed_div(neg_one2), Fixed16_16::from_raw(i32::MAX));
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
    fn fixed_div_by_zero_saturates_like_vanilla() {
        // Vanilla FixedDiv does not special-case zero: `(abs(a) >> 14) >= 0`
        // is always true, so it saturates to INT_MIN/INT_MAX by the sign of
        // `a ^ b` (with b == 0, that is the sign of a). It never panics.
        let pos = Fixed16_16::from_int(10);
        let neg = Fixed16_16::from_int(-10);
        let zero = Fixed16_16::ZERO;
        assert_eq!(pos.fixed_div(zero), Fixed16_16::from_raw(i32::MAX));
        assert_eq!(neg.fixed_div(zero), Fixed16_16::from_raw(i32::MIN));
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

        #[test]
        fn fixed_div_does_not_panic_on_negative_one(a in any::<i32>()) {
            // Only requirement is rhs != 0
            let dividend = Fixed16_16(a);
            let divisor = Fixed16_16(-1);
            let _ = dividend.fixed_div(divisor); // Shouldn't panic!
        }

        #[test]
        fn fixed_div_does_not_panic(a in any::<i32>(), b in any::<i32>()) {
            let dividend = Fixed16_16(a);
            let divisor = Fixed16_16(b);
            if b != 0 {
                let _ = dividend.fixed_div(divisor);
            }
        }
    }
}
