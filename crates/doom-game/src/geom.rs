//! Bit-exact ports of the vanilla Doom fixed-point geometry primitives used by
//! movement / wall sliding (`tables.c`, `r_main.c`, `p_maputl.c`).
//!
//! All functions operate on raw `i32` fixed-point (16.16) values, matching the
//! C originals exactly (including truncation / wrap-around behaviour) so the
//! demo simulation stays in lock-step with vanilla.

use crate::tantoangle::TANTOANGLE;
use doom_types::Bam;

/// `SLOPERANGE` from `tables.h`.
const SLOPERANGE: u32 = 2048;

/// Binary-angle constants (`tables.h`).
pub const ANG90: u32 = 0x4000_0000;
/// Binary angle for 180 degrees.
pub const ANG180: u32 = 0x8000_0000;
/// Binary angle for 270 degrees.
pub const ANG270: u32 = 0xC000_0000;

/// Fixed-point multiply matching vanilla `FixedMul`.
#[inline]
pub fn fixed_mul(a: i32, b: i32) -> i32 {
    (((a as i64) * (b as i64)) >> 16) as i32
}

/// Fixed-point divide matching vanilla `FixedDiv`.
#[inline]
pub fn fixed_div(a: i32, b: i32) -> i32 {
    doom_types::Fixed16_16::from_raw(a)
        .fixed_div(doom_types::Fixed16_16::from_raw(b))
        .raw()
}

/// Vanilla `P_AproxDistance` — cheap Manhattan-ish distance approximation.
#[inline]
pub fn p_aprox_distance(dx: i32, dy: i32) -> i32 {
    let dx = dx.wrapping_abs();
    let dy = dy.wrapping_abs();
    if dx < dy {
        dx.wrapping_add(dy).wrapping_sub(dx >> 1)
    } else {
        dx.wrapping_add(dy).wrapping_sub(dy >> 1)
    }
}

/// Vanilla `SlopeDiv` (`tables.c`).
#[inline]
fn slope_div(num: u32, den: u32) -> u32 {
    if den < 512 {
        return SLOPERANGE;
    }
    let ans = (num << 3) / (den >> 8);
    ans.min(SLOPERANGE)
}

/// Vanilla `R_PointToAngle2` (`r_main.c`) — the exact `tantoangle`/`SlopeDiv`
/// octant lookup used for `finesine`/`finecosine`-indexed slide clipping.
pub fn r_point_to_angle2(x1: i32, y1: i32, x2: i32, y2: i32) -> u32 {
    let x = x2.wrapping_sub(x1);
    let y = y2.wrapping_sub(y1);

    if x == 0 && y == 0 {
        return 0;
    }

    if x >= 0 {
        if y >= 0 {
            if x > y {
                // octant 0
                TANTOANGLE[slope_div(y as u32, x as u32) as usize]
            } else {
                // octant 1
                (ANG90 - 1).wrapping_sub(TANTOANGLE[slope_div(x as u32, y as u32) as usize])
            }
        } else {
            let y = -y;
            if x > y {
                // octant 8
                0u32.wrapping_sub(TANTOANGLE[slope_div(y as u32, x as u32) as usize])
            } else {
                // octant 7
                ANG270.wrapping_add(TANTOANGLE[slope_div(x as u32, y as u32) as usize])
            }
        }
    } else {
        let x = -x;
        if y >= 0 {
            if x > y {
                // octant 3
                (ANG180 - 1).wrapping_sub(TANTOANGLE[slope_div(y as u32, x as u32) as usize])
            } else {
                // octant 2
                ANG90.wrapping_add(TANTOANGLE[slope_div(x as u32, y as u32) as usize])
            }
        } else {
            let y = -y;
            if x > y {
                // octant 4
                ANG180.wrapping_add(TANTOANGLE[slope_div(y as u32, x as u32) as usize])
            } else {
                // octant 5
                (ANG270 - 1).wrapping_sub(TANTOANGLE[slope_div(x as u32, y as u32) as usize])
            }
        }
    }
}

/// `finecosine[angle >> ANGLETOFINESHIFT]` as raw 16.16.
#[inline]
pub fn fine_cosine(angle: u32) -> i32 {
    Bam(angle).cos().raw()
}

/// `finesine[angle >> ANGLETOFINESHIFT]` as raw 16.16.
#[inline]
pub fn fine_sine(angle: u32) -> i32 {
    Bam(angle).sin().raw()
}

/// A directed line (`divline_t`) in raw fixed-point.
#[derive(Clone, Copy, Debug)]
pub struct DivLine {
    /// The x-coordinate of the line's origin.
    pub x: i32,
    /// The y-coordinate of the line's origin.
    pub y: i32,
    /// The x-component of the line's direction.
    pub dx: i32,
    /// The y-component of the line's direction.
    pub dy: i32,
}

/// Vanilla `P_PointOnLineSide` (`p_maputl.c`) for a line given by its raw
/// fixed-point endpoints. Returns 0 (front) or 1 (back).
pub fn p_point_on_line_side(x: i32, y: i32, v1x: i32, v1y: i32, ldx: i32, ldy: i32) -> i32 {
    if ldx == 0 {
        if x <= v1x {
            return (ldy > 0) as i32;
        }
        return (ldy < 0) as i32;
    }
    if ldy == 0 {
        if y <= v1y {
            return (ldx < 0) as i32;
        }
        return (ldx > 0) as i32;
    }

    let dx = x.wrapping_sub(v1x);
    let dy = y.wrapping_sub(v1y);

    let left = fixed_mul(ldy >> 16, dx);
    let right = fixed_mul(dy, ldx >> 16);

    if right < left { 0 } else { 1 }
}

/// Vanilla `P_PointOnDivlineSide` (`p_maputl.c`). Returns 0 or 1.
pub fn p_point_on_divline_side(x: i32, y: i32, line: &DivLine) -> i32 {
    if line.dx == 0 {
        if x <= line.x {
            return (line.dy > 0) as i32;
        }
        return (line.dy < 0) as i32;
    }
    if line.dy == 0 {
        if y <= line.y {
            return (line.dx < 0) as i32;
        }
        return (line.dx > 0) as i32;
    }

    let dx = x.wrapping_sub(line.x);
    let dy = y.wrapping_sub(line.y);

    // Sign-bit fast path (matches vanilla bit-twiddle exactly).
    if ((line.dy ^ line.dx ^ dx ^ dy) as u32 & 0x8000_0000) != 0 {
        if ((line.dy ^ dx) as u32 & 0x8000_0000) != 0 {
            return 1;
        }
        return 0;
    }

    let left = fixed_mul(line.dy >> 8, dx >> 8);
    let right = fixed_mul(dy >> 8, line.dx >> 8);

    if right < left { 0 } else { 1 }
}

/// Vanilla `P_InterceptVector` (`p_maputl.c`): the fractional distance along
/// `v2` at which it crosses `v1` (16.16), or 0 if parallel.
pub fn p_intercept_vector(v2: &DivLine, v1: &DivLine) -> i32 {
    let den = fixed_mul(v1.dy >> 8, v2.dx) - fixed_mul(v1.dx >> 8, v2.dy);
    if den == 0 {
        return 0;
    }
    let num = fixed_mul((v1.x - v2.x) >> 8, v1.dy) + fixed_mul((v2.y - v1.y) >> 8, v1.dx);
    fixed_div(num, den)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tantoangle_endpoints() {
        assert_eq!(TANTOANGLE[0], 0);
        assert_eq!(TANTOANGLE[2048], 0x2000_0000); // ANG45
    }

    #[test]
    fn r_point_to_angle2_cardinals_match_vanilla_boundaries() {
        // Exact vanilla octant-boundary values: East and South land exactly on
        // 0 / ANG270, while North and West are the "minus one" boundary results
        // (ANG90-1-tantoangle[0] and ANG180-1-tantoangle[0]).
        assert_eq!(r_point_to_angle2(0, 0, 1 << 16, 0), 0);
        assert_eq!(r_point_to_angle2(0, 0, 0, 1 << 16), ANG90 - 1);
        assert_eq!(r_point_to_angle2(0, 0, -(1 << 16), 0), ANG180 - 1);
        assert_eq!(r_point_to_angle2(0, 0, 0, -(1 << 16)), ANG270);
    }

    #[test]
    fn r_point_to_angle2_diagonal_boundary() {
        // Due NE lands on the octant-1 boundary: ANG90-1-tantoangle[2048]
        // = 0x3FFFFFFF - 0x20000000 = 0x1FFFFFFF (one below ANG45).
        assert_eq!(r_point_to_angle2(0, 0, 1 << 16, 1 << 16), 0x1FFF_FFFF);
        // Octant 0 (x > y): straightforward tantoangle lookup.
        assert_eq!(r_point_to_angle2(0, 0, 2 << 16, 1 << 16), TANTOANGLE[1024]);
    }

    #[test]
    fn aprox_distance_matches_formula() {
        // dx >= dy branch: 100 + 40 - 20 = 120
        assert_eq!(p_aprox_distance(100, 40), 120);
        // dx < dy branch: 40 + 100 - 20 = 120
        assert_eq!(p_aprox_distance(40, 100), 120);
    }
}
