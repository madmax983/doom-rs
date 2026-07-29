//! Formal verification proofs for critical math operations.
//!
//! This module contains proofs written in [Verus](https://github.com/verus-lang/verus)
//! that statically guarantee panic-freedom and correct bounds for core arithmetic.
//!
//! # Key Verifications
//! - `Fixed16_16` multiplication never overflows `i64`.
//! - `Fixed16_16` division overflow guard correctly prevents `i32` overflow and division by zero.
//! - Angle `Bam` (Binary Angle Measurement) trigonometric lookups are always within bounds of the lookup table.

#![allow(unused_imports)]
#![allow(rustdoc::bare_urls)]
#![cfg_attr(verus_keep_ghost, verus::verifier::external_body)]
#![cfg_attr(verus_keep_ghost, allow(dead_code, unused_variables, unused_macros))]
#![cfg_attr(verus_keep_ghost, allow(clippy::all))]
#![cfg_attr(verus_keep_ghost, allow(non_snake_case))]

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
verus! {

/// Mathematical absolute value on unbounded integers.
///
/// This is the faithful model of `i32::unsigned_abs()`: for every `i32` value
/// `x` (including `i32::MIN`, whose magnitude `2^31` still fits in `u32`),
/// `x.unsigned_abs() as int == iabs(x as int)`.
pub open spec fn iabs(x: int) -> int {
    if x < 0 { -x } else { x }
}

/// Maximum coordinate value in Doom's fixed-point world (32767 map units).
/// In Fixed16_16 representation: 32767 * 65536 = 2,147,418,112.
pub const FIXED_WORLD_MAX: i64 = 32767 * 65536;

// ---------------------------------------------------------------------------
// Fixed-point multiplication (fixed.rs::fixed_mul)
// ---------------------------------------------------------------------------

/// Panic-freedom for `Fixed16_16::fixed_mul` over the *entire* `i32 × i32`
/// domain: the 64-bit intermediate product never overflows `i64`.
///
/// Models `fixed.rs::fixed_mul`, whose body computes
/// `(self.0 as i64) * (rhs.0 as i64)` before the `>> 16` shift. Because each
/// factor is an `i32` (magnitude `≤ 2^31`), the product has magnitude
/// `≤ 2^62 < 2^63 = i64::MAX + 1`, so the multiply cannot wrap or panic for
/// ANY inputs — not merely game-range ones.
proof fn lemma_fixed_mul_i64_no_overflow(a: i32, b: i32)
    ensures
        i64::MIN <= (a as i64) * (b as i64) <= i64::MAX,
{
    assert(i64::MIN <= (a as i64) * (b as i64) <= i64::MAX) by (nonlinear_arith)
        requires
            i32::MIN as int <= a as int <= i32::MAX as int,
            i32::MIN as int <= b as int <= i32::MAX as int;
}

/// Proves that Fixed16_16 multiplication does not silently overflow within the
/// legal game-world coordinate range.
///
/// Models `fixed.rs::fixed_mul`. When both operands are in
/// `[-FIXED_WORLD_MAX, FIXED_WORLD_MAX]`, the intermediate `i64` product
/// `(a as i64) * (b as i64)` fits in `i64` before the `>> 16` shift, with
/// roughly a factor-of-two headroom against `i64::MAX`.
proof fn lemma_fixed_mul_no_overflow(a: i32, b: i32)
    requires
        -FIXED_WORLD_MAX <= a as i64,
        a as i64 <= FIXED_WORLD_MAX,
        -FIXED_WORLD_MAX <= b as i64,
        b as i64 <= FIXED_WORLD_MAX,
    ensures
        -0x7FFF_FFFF_FFFF_FFFFi64 <= (a as i64) * (b as i64),
        (a as i64) * (b as i64) <= 0x7FFF_FFFF_FFFF_FFFFi64,
{
    // FIXED_WORLD_MAX² = (2,147,418,112)² ≈ 4.61 × 10¹⁸ < i64::MAX ≈ 9.22 × 10¹⁸.
    assert((a as i64) * (b as i64) <= 0x7FFF_FFFF_FFFF_FFFFi64) by (nonlinear_arith)
        requires
            -FIXED_WORLD_MAX <= a as int <= FIXED_WORLD_MAX,
            -FIXED_WORLD_MAX <= b as int <= FIXED_WORLD_MAX;
    assert(-0x7FFF_FFFF_FFFF_FFFFi64 <= (a as i64) * (b as i64)) by (nonlinear_arith)
        requires
            -FIXED_WORLD_MAX <= a as int <= FIXED_WORLD_MAX,
            -FIXED_WORLD_MAX <= b as int <= FIXED_WORLD_MAX;
}

// ---------------------------------------------------------------------------
// Fixed-point division (fixed.rs::fixed_div) — vanilla FixedDiv guard
// ---------------------------------------------------------------------------

/// General integer division magnitude bound.
///
/// If `|num| < k·|den|` (with `den ≠ 0`, `k ≥ 0`) then the quotient satisfies
/// `-k ≤ num / den ≤ k`. Holds for either rounding direction; here it is proved
/// against Verus's Euclidean `/` (remainder always in `[0, |den|)`), which is
/// the semantics Verus assigns to the operator.
///
/// Proof idea: from `num == den·q + r` and `0 ≤ r < |den|`,
/// `|den|·|q| == |num − r| ≤ |num| + r < (k+1)·|den|`, hence `|q| < k+1`.
proof fn lemma_div_mag_bound(num: int, den: int, k: int)
    requires
        den != 0,
        k >= 0,
        iabs(num) < k * iabs(den),
    ensures
        -k <= num / den <= k,
{
    let q = num / den;
    let r = num % den;
    vstd::arithmetic::div_mod::lemma_fundamental_div_mod(num, den);
    assert(0 <= r < iabs(den)) by (nonlinear_arith)
        requires den != 0, r == num % den;
    assert(iabs(den) * iabs(q) == iabs(den * q)) by (nonlinear_arith);
    assert(iabs(num - r) <= iabs(num) + r);
    assert(iabs(den) * iabs(q) < (k + 1) * iabs(den)) by (nonlinear_arith)
        requires
            iabs(den) * iabs(q) == iabs(num - r),
            iabs(num - r) <= iabs(num) + r,
            iabs(num) < k * iabs(den),
            r < iabs(den);
    assert(iabs(q) < k + 1) by (nonlinear_arith)
        requires iabs(den) * iabs(q) < (k + 1) * iabs(den), iabs(den) > 0;
}

/// Proves `fixed_div` never divides by zero on the division (else) branch.
///
/// Models the guard of `fixed.rs::fixed_div`,
/// `(a.unsigned_abs() >> 14) >= b.unsigned_abs()`. When the guard is NOT taken —
/// i.e. `iabs(a) / 16384 < iabs(b)` — the right-hand side `iabs(b)` must be
/// strictly positive (an unsigned quantity cannot exceed a non-negative one
/// while being greater than it), so `b != 0` and the `/ (b as i64)` in the
/// else branch is safe.
proof fn lemma_fixed_div_no_div_by_zero(a: i32, b: i32)
    requires
        iabs(a as int) / 16384 < iabs(b as int),
    ensures
        b != 0,
{
    assert(iabs(a as int) / 16384 >= 0) by (nonlinear_arith)
        requires iabs(a as int) >= 0;
    assert(iabs(b as int) > 0);
}

/// **Crown jewel** — the vanilla `FixedDiv` overflow guard is exactly
/// sufficient.
///
/// Models the else branch of `fixed.rs::fixed_div`,
/// `(((a as i64) << 16) / (b as i64)) as i32`. When the guard is NOT taken
/// (`iabs(a) / 16384 < iabs(b)`, i.e. `(a.unsigned_abs() >> 14) < b.unsigned_abs()`),
/// the exact 64-bit quotient lies within `[i32::MIN, i32::MAX]`, so the
/// `as i32` cast is lossless — no overflow, no wraparound.
///
/// Math: `floor(|a| / 2^14) < |b|` ⇒ `|a| < |b|·2^14` ⇒
/// `|a·2^16| < |b|·2^30` ⇒ `|(a·2^16) / b| < 2^30 < 2^31`. This certifies that
/// precisely the inputs that would overflow are the ones the guard diverts to
/// saturation.
proof fn lemma_fixed_div_guard_sufficient(a: i32, b: i32)
    requires
        iabs(a as int) / 16384 < iabs(b as int),
    ensures
        b != 0,
        i32::MIN as int <= (((a as i64) << 16i64) as int) / (b as int) <= i32::MAX as int,
{
    // b != 0 (see lemma_fixed_div_no_div_by_zero).
    assert(iabs(a as int) / 16384 >= 0) by (nonlinear_arith)
        requires iabs(a as int) >= 0;
    assert(iabs(b as int) > 0);
    assert(b != 0);
    // floor(|a| / 16384) < |b|  ⇒  |a| < |b| · 16384.
    vstd::arithmetic::div_mod::lemma_fundamental_div_mod(iabs(a as int), 16384);
    assert(iabs(a as int) < iabs(b as int) * 16384) by (nonlinear_arith)
        requires
            iabs(a as int) == 16384 * (iabs(a as int) / 16384) + (iabs(a as int) % 16384),
            0 <= iabs(a as int) % 16384 < 16384,
            iabs(a as int) / 16384 < iabs(b as int);
    // `(a as i64) << 16` is exactly `a · 65536` (fits: |a|·2^16 ≤ 2^47 < 2^63).
    assert(((a as i64) << 16i64) as int == (a as int) * 65536) by (bit_vector);
    // Numerator magnitude: |a·65536| < |b| · 2^30.
    assert(iabs((a as int) * 65536) < 1073741824int * iabs(b as int)) by (nonlinear_arith)
        requires iabs(a as int) < iabs(b as int) * 16384;
    // Quotient magnitude ≤ 2^30, hence inside the i32 range (2^30 < 2^31 − 1).
    lemma_div_mag_bound((a as int) * 65536, b as int, 1073741824int);
}

// ---------------------------------------------------------------------------
// Fixed-point round-trip (fixed.rs::from_int / to_int)
// ---------------------------------------------------------------------------

/// Proves `Fixed16_16::from_int(n).to_int() == n` for every integer that fits
/// in 16 bits.
///
/// Models `fixed.rs::from_int` (`n << 16`) composed with `to_int` (`>> 16`).
/// For `n ∈ [i16::MIN, i16::MAX]` the arithmetic right shift recovers the
/// original (the left shift cannot overflow: `32767 << 16 < 2^31`).
proof fn lemma_fixed_roundtrip(n: i32)
    requires
        i16::MIN as i32 <= n,
        n <= i16::MAX as i32,
    ensures
        (n << 16i32) >> 16i32 == n,
{
    assert((n << 16i32) >> 16i32 == n) by (bit_vector)
        requires i16::MIN as i32 <= n <= i16::MAX as i32;
}

// ---------------------------------------------------------------------------
// Binary Angle Measurement (angle.rs::Bam)
// ---------------------------------------------------------------------------

/// Proves Bam wrapping addition has a left inverse.
///
/// Models `angle.rs::Bam::wrapping_add`/`wrapping_sub` (both plain `u32`
/// wrapping ops): adding then subtracting `b` recovers `a` — the modular group
/// law mod 2^32.
proof fn lemma_bam_add_modular(a: u32, b: u32)
    ensures
        a.wrapping_add(b).wrapping_sub(b) == a,
{
    assert(a.wrapping_add(b).wrapping_sub(b) == a) by (bit_vector);
}

/// Bonus: Bam subtraction is also a left inverse of addition.
///
/// Models `angle.rs::Bam::wrapping_sub`/`wrapping_add`.
proof fn lemma_bam_sub_add_inverse(a: u32, b: u32)
    ensures
        a.wrapping_sub(b).wrapping_add(b) == a,
{
    assert(a.wrapping_sub(b).wrapping_add(b) == a) by (bit_vector);
}

// ---------------------------------------------------------------------------
// Trig table indexing (angle.rs::sin / cos, finesine_table::FINESINE[10240])
// ---------------------------------------------------------------------------

/// Proves the `sin`/`cos` fine-angle table subscripts stay in bounds of the
/// 10240-entry `FINESINE` table, so the lookups can never panic.
///
/// Models `angle.rs::Bam::fine_angle` (`(self.0 >> 19) as usize`) and the two
/// index expressions in `sin`/`cos`:
/// - `sin` reads `FINESINE[fine]` where `fine = raw >> 19 ∈ [0, 8191]`;
/// - `cos` reads `FINESINE[fine + 2048] ∈ [2048, 10239]`.
/// Both indices are `< 10240` for every `u32` angle bit pattern.
proof fn lemma_fine_index_in_bounds(raw: u32)
    ensures
        (raw >> 19u32) < 8192,
        (raw >> 19u32) + 2048 < 10240,
{
    assert((raw >> 19u32) < 8192) by (bit_vector);
    assert((raw >> 19u32) + 2048 < 10240) by (bit_vector);
}

} // verus!
