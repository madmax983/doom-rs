//! 2D vector in fixed-point space.

use crate::fixed::Fixed16_16;
use core::ops::{Add, Neg, Sub};

/// 2D vector with fixed-point components.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Vec2Fixed {
    /// X component.
    pub x: Fixed16_16,
    /// Y component.
    pub y: Fixed16_16,
}

impl Vec2Fixed {
    /// Zero vector (0, 0).
    pub const ZERO: Self = Self {
        x: Fixed16_16::ZERO,
        y: Fixed16_16::ZERO,
    };

    /// Construct from raw fixed-point values.
    ///
    /// # Examples
    /// ```
    /// use doom_types::{Vec2Fixed, Fixed16_16};
    ///
    /// let v = Vec2Fixed::new(Fixed16_16::from_int(1), Fixed16_16::from_int(2));
    /// assert_eq!(v.x.to_int(), 1);
    /// assert_eq!(v.y.to_int(), 2);
    /// ```
    #[inline]
    pub const fn new(x: Fixed16_16, y: Fixed16_16) -> Self {
        Self { x, y }
    }

    /// Construct from integer coordinates.
    ///
    /// This automatically scales the provided integers into `Fixed16_16` format.
    ///
    /// # Examples
    /// ```
    /// use doom_types::{Vec2Fixed, Fixed16_16};
    ///
    /// let v = Vec2Fixed::from_ints(10, 20);
    /// assert_eq!(v.x, Fixed16_16::from_int(10));
    /// assert_eq!(v.y, Fixed16_16::from_int(20));
    /// ```
    #[inline]
    pub fn from_ints(x: i32, y: i32) -> Self {
        Self {
            x: Fixed16_16::from_int(x),
            y: Fixed16_16::from_int(y),
        }
    }

    /// Squared length (avoids an expensive square root operation).
    ///
    /// Extremely useful for fast distance comparisons.
    ///
    /// # Examples
    /// ```
    /// use doom_types::{Vec2Fixed, Fixed16_16};
    ///
    /// // A 3-4-5 right triangle
    /// let v = Vec2Fixed::from_ints(3, 4);
    /// assert_eq!(v.length_sq(), Fixed16_16::from_int(25));
    /// ```
    #[inline]
    pub fn length_sq(self) -> Fixed16_16 {
        self.x.fixed_mul(self.x) + self.y.fixed_mul(self.y)
    }

    /// Dot product.
    ///
    /// Useful for determining if two vectors are pointing in the same direction,
    /// or for projecting one vector onto another.
    ///
    /// # Examples
    /// ```
    /// use doom_types::{Vec2Fixed, Fixed16_16};
    ///
    /// let a = Vec2Fixed::from_ints(1, 0);
    /// let b = Vec2Fixed::from_ints(0, 1);
    ///
    /// // Orthogonal vectors have a dot product of 0
    /// assert_eq!(a.dot(b), Fixed16_16::ZERO);
    /// ```
    #[inline]
    pub fn dot(self, rhs: Self) -> Fixed16_16 {
        self.x.fixed_mul(rhs.x) + self.y.fixed_mul(rhs.y)
    }

    /// Component-wise scale by a fixed-point scalar.
    ///
    /// # Examples
    /// ```
    /// use doom_types::{Vec2Fixed, Fixed16_16};
    ///
    /// let v = Vec2Fixed::from_ints(2, 3);
    /// let s = Fixed16_16::from_int(2);
    ///
    /// let scaled = v.scale(s);
    /// assert_eq!(scaled.x, Fixed16_16::from_int(4));
    /// assert_eq!(scaled.y, Fixed16_16::from_int(6));
    /// ```
    #[inline]
    pub fn scale(self, s: Fixed16_16) -> Self {
        Self {
            x: self.x.fixed_mul(s),
            y: self.y.fixed_mul(s),
        }
    }
}

impl Add for Vec2Fixed {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub for Vec2Fixed {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl Neg for Vec2Fixed {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Self {
            x: -self.x,
            y: -self.y,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_sub_inverse() {
        let a = Vec2Fixed::from_ints(3, 4);
        let b = Vec2Fixed::from_ints(1, 2);
        assert_eq!((a + b) - b, a);
    }

    #[test]
    fn dot_product_orthogonal() {
        let x = Vec2Fixed::from_ints(1, 0);
        let y = Vec2Fixed::from_ints(0, 1);
        assert_eq!(x.dot(y), Fixed16_16::ZERO);
    }

    #[test]
    fn scale_by_zero_is_zero() {
        let v = Vec2Fixed::from_ints(100, 200);
        assert_eq!(v.scale(Fixed16_16::ZERO), Vec2Fixed::ZERO);
    }

    #[test]
    fn length_sq_calculation() {
        // 3-4-5 triangle: 3^2 + 4^2 = 9 + 16 = 25
        let v = Vec2Fixed::from_ints(3, 4);
        assert_eq!(v.length_sq(), Fixed16_16::from_int(25));

        let zero = Vec2Fixed::ZERO;
        assert_eq!(zero.length_sq(), Fixed16_16::ZERO);
    }

    #[test]
    fn new_and_neg_trait() {
        let v = Vec2Fixed::new(Fixed16_16::from_int(5), Fixed16_16::from_int(-2));
        assert_eq!(v.x.to_int(), 5);
        assert_eq!(v.y.to_int(), -2);

        let n = -v;
        assert_eq!(n.x.to_int(), -5);
        assert_eq!(n.y.to_int(), 2);
    }

    #[test]
    fn sub_trait() {
        let a = Vec2Fixed::from_ints(5, 5);
        let b = Vec2Fixed::from_ints(2, 3);
        let c = a - b;
        assert_eq!(c.x.to_int(), 3);
        assert_eq!(c.y.to_int(), 2);
    }
}
