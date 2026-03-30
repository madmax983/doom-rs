//! Axis-aligned bounding box in fixed-point space.
//!
//! # Verus invariant
//! A valid `BBox` always satisfies `ymax >= ymin` and `xmax >= xmin`.

use crate::fixed::Fixed16_16;
use crate::vec2::Vec2Fixed;

/// Axis-aligned bounding box. Stored as top/bottom/left/right to match
/// the Doom node format (ymax = top, ymin = bottom, xmin = left, xmax = right).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BBox {
    /// Maximum Y (top of box in world space, where +Y is up).
    pub ymax: Fixed16_16,
    /// Minimum Y (bottom).
    pub ymin: Fixed16_16,
    /// Minimum X (left).
    pub xmin: Fixed16_16,
    /// Maximum X (right).
    pub xmax: Fixed16_16,
}

impl BBox {
    /// Construct from corner points, normalizing so invariants hold.
    ///
    /// # Examples
    /// ```
    /// use doom_types::{BBox, Vec2Fixed, Fixed16_16};
    ///
    /// let p1 = Vec2Fixed::from_ints(5, 10);
    /// let p2 = Vec2Fixed::from_ints(1, 2);
    /// let bbox = BBox::from_corners(p1, p2);
    ///
    /// assert_eq!(bbox.xmin, Fixed16_16::from_int(1));
    /// assert_eq!(bbox.xmax, Fixed16_16::from_int(5));
    /// assert!(bbox.is_valid());
    /// ```
    #[inline]
    pub fn from_corners(a: Vec2Fixed, b: Vec2Fixed) -> Self {
        Self {
            ymax: if a.y > b.y { a.y } else { b.y },
            ymin: if a.y < b.y { a.y } else { b.y },
            xmin: if a.x < b.x { a.x } else { b.x },
            xmax: if a.x > b.x { a.x } else { b.x },
        }
    }

    /// Construct directly from (ymax, ymin, xmin, xmax).
    ///
    /// Caller must ensure `ymax >= ymin` and `xmax >= xmin`.
    ///
    /// # Examples
    /// ```
    /// use doom_types::{BBox, Fixed16_16};
    ///
    /// let int = Fixed16_16::from_int;
    /// let bbox = BBox::new(int(10), int(0), int(0), int(10));
    /// assert!(bbox.is_valid());
    /// ```
    #[inline]
    pub const fn new(
        ymax: Fixed16_16,
        ymin: Fixed16_16,
        xmin: Fixed16_16,
        xmax: Fixed16_16,
    ) -> Self {
        Self {
            ymax,
            ymin,
            xmin,
            xmax,
        }
    }

    /// Returns `true` if the invariant holds (non-degenerate box).
    ///
    /// A box is valid if its top edge is above or equal to its bottom edge,
    /// and its right edge is at or to the right of its left edge.
    ///
    /// # Examples
    /// ```
    /// use doom_types::{BBox, Fixed16_16};
    ///
    /// let int = Fixed16_16::from_int;
    /// // Invalid: min > max
    /// let bbox = BBox::new(int(0), int(10), int(0), int(10));
    /// assert!(!bbox.is_valid());
    /// ```
    #[inline]
    pub fn is_valid(self) -> bool {
        self.ymax >= self.ymin && self.xmax >= self.xmin
    }

    /// Returns `true` if `point` is inside or on the boundary of the box.
    ///
    /// # Examples
    /// ```
    /// use doom_types::{BBox, Vec2Fixed, Fixed16_16};
    ///
    /// let int = Fixed16_16::from_int;
    /// let bbox = BBox::new(int(10), int(0), int(0), int(10));
    ///
    /// assert!(bbox.contains(Vec2Fixed::from_ints(5, 5)));
    /// assert!(!bbox.contains(Vec2Fixed::from_ints(15, 5)));
    /// ```
    #[inline]
    pub fn contains(self, point: Vec2Fixed) -> bool {
        point.x >= self.xmin && point.x <= self.xmax && point.y >= self.ymin && point.y <= self.ymax
    }

    /// Returns `true` if this box overlaps `other` (touching counts as overlap).
    ///
    /// # Examples
    /// ```
    /// use doom_types::{BBox, Fixed16_16};
    ///
    /// let int = Fixed16_16::from_int;
    /// let a = BBox::new(int(10), int(0), int(0), int(10));
    /// let b = BBox::new(int(15), int(5), int(5), int(15));
    ///
    /// assert!(a.overlaps(b));
    /// ```
    #[inline]
    pub fn overlaps(self, other: Self) -> bool {
        self.xmin <= other.xmax
            && self.xmax >= other.xmin
            && self.ymin <= other.ymax
            && self.ymax >= other.ymin
    }

    /// Width of the box in fixed-point units.
    ///
    /// # Examples
    /// ```
    /// use doom_types::{BBox, Fixed16_16};
    ///
    /// let int = Fixed16_16::from_int;
    /// let bbox = BBox::new(int(10), int(0), int(2), int(8));
    /// assert_eq!(bbox.width(), int(6));
    /// ```
    #[inline]
    pub fn width(self) -> Fixed16_16 {
        self.xmax - self.xmin
    }

    /// Height of the box in fixed-point units.
    ///
    /// # Examples
    /// ```
    /// use doom_types::{BBox, Fixed16_16};
    ///
    /// let int = Fixed16_16::from_int;
    /// let bbox = BBox::new(int(15), int(5), int(0), int(10));
    /// assert_eq!(bbox.height(), int(10));
    /// ```
    #[inline]
    pub fn height(self) -> Fixed16_16 {
        self.ymax - self.ymin
    }

    /// Expand the box to also contain `other`.
    ///
    /// # Examples
    /// ```
    /// use doom_types::{BBox, Fixed16_16};
    ///
    /// let int = Fixed16_16::from_int;
    /// let a = BBox::new(int(5), int(0), int(0), int(5));
    /// let b = BBox::new(int(10), int(3), int(2), int(8));
    ///
    /// let u = a.union(b);
    /// assert_eq!(u.ymax, int(10));
    /// assert_eq!(u.xmax, int(8));
    /// ```
    #[inline]
    pub fn union(self, other: Self) -> Self {
        Self {
            ymax: if self.ymax > other.ymax {
                self.ymax
            } else {
                other.ymax
            },
            ymin: if self.ymin < other.ymin {
                self.ymin
            } else {
                other.ymin
            },
            xmin: if self.xmin < other.xmin {
                self.xmin
            } else {
                other.xmin
            },
            xmax: if self.xmax > other.xmax {
                self.xmax
            } else {
                other.xmax
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn int(n: i32) -> Fixed16_16 {
        Fixed16_16::from_int(n)
    }

    #[test]
    fn from_corners_normalizes() {
        let a = Vec2Fixed::from_ints(5, 10);
        let b = Vec2Fixed::from_ints(1, 2);
        let bb = BBox::from_corners(a, b);
        assert!(bb.is_valid());
        assert_eq!(bb.xmin, int(1));
        assert_eq!(bb.xmax, int(5));
    }

    #[test]
    fn contains_center() {
        let bb = BBox::new(int(10), int(0), int(0), int(10));
        assert!(bb.contains(Vec2Fixed::from_ints(5, 5)));
    }

    #[test]
    fn does_not_contain_outside() {
        let bb = BBox::new(int(10), int(0), int(0), int(10));
        assert!(!bb.contains(Vec2Fixed::from_ints(11, 5)));
    }

    #[test]
    fn union_grows_box() {
        let a = BBox::new(int(5), int(0), int(0), int(5));
        let b = BBox::new(int(10), int(3), int(2), int(8));
        let u = a.union(b);
        assert_eq!(u.ymax, int(10));
        assert_eq!(u.xmax, int(8));

        // Test else branches of union where self values are retained
        let c = BBox::new(int(15), int(-5), int(-5), int(15));
        let d = BBox::new(int(10), int(0), int(0), int(10));
        let v = c.union(d);
        assert_eq!(v.ymax, int(15));
        assert_eq!(v.ymin, int(-5));
        assert_eq!(v.xmin, int(-5));
        assert_eq!(v.xmax, int(15));

        // Test mixed branches
        let e = BBox::new(int(5), int(0), int(-5), int(5));
        let f = BBox::new(int(10), int(-5), int(0), int(10));
        let w = e.union(f);
        assert_eq!(w.ymax, int(10));
        assert_eq!(w.ymin, int(-5));
        assert_eq!(w.xmin, int(-5));
        assert_eq!(w.xmax, int(10));

        let g = BBox::new(int(5), int(0), int(5), int(10));
        let h = BBox::new(int(10), int(-5), int(0), int(5));
        let x = g.union(h);
        assert_eq!(x.ymax, int(10));
        assert_eq!(x.ymin, int(-5));
        assert_eq!(x.xmin, int(0));
        assert_eq!(x.xmax, int(10));
    }

    #[test]
    fn overlaps_detects_intersection() {
        let a = BBox::new(int(5), int(0), int(0), int(5));

        // Touching (overlaps)
        let b = BBox::new(int(10), int(5), int(5), int(10));
        assert!(a.overlaps(b));
        assert!(b.overlaps(a));

        // Inside (overlaps)
        let c = BBox::new(int(4), int(1), int(1), int(4));
        assert!(a.overlaps(c));
        assert!(c.overlaps(a));

        // Outside (does not overlap)
        let d = BBox::new(int(15), int(10), int(10), int(15));
        assert!(!a.overlaps(d));
        assert!(!d.overlaps(a));
    }

    #[test]
    fn is_valid_checks_bounds() {
        let valid = BBox::new(int(10), int(0), int(0), int(10));
        assert!(valid.is_valid());

        let invalid_y = BBox::new(int(0), int(10), int(0), int(10));
        assert!(!invalid_y.is_valid());

        let invalid_x = BBox::new(int(10), int(0), int(10), int(0));
        assert!(!invalid_x.is_valid());

        let invalid_both = BBox::new(int(0), int(10), int(10), int(0));
        assert!(!invalid_both.is_valid());
    }

    #[test]
    fn width_and_height() {
        let bb = BBox::new(int(20), int(5), int(2), int(10));
        assert_eq!(bb.width(), int(8));
        assert_eq!(bb.height(), int(15));
    }
}
