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
    /// Zero vector.
    pub const ZERO: Self = Self {
        x: Fixed16_16::ZERO,
        y: Fixed16_16::ZERO,
    };

    /// Construct from raw fixed-point values.
    #[inline]
    pub const fn new(x: Fixed16_16, y: Fixed16_16) -> Self {
        Self { x, y }
    }

    /// Construct from integer coordinates.
    #[inline]
    pub fn from_ints(x: i32, y: i32) -> Self {
        Self {
            x: Fixed16_16::from_int(x),
            y: Fixed16_16::from_int(y),
        }
    }

    /// Squared length (avoids a sqrt).
    #[inline]
    pub fn length_sq(self) -> Fixed16_16 {
        self.x.fixed_mul(self.x) + self.y.fixed_mul(self.y)
    }

    /// Dot product.
    #[inline]
    pub fn dot(self, rhs: Self) -> Fixed16_16 {
        self.x.fixed_mul(rhs.x) + self.y.fixed_mul(rhs.y)
    }

    /// Component-wise scale by a fixed-point scalar.
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
}
