// Verus spine proofs for doom-types.
//
// These proofs are verified with the Verus verifier (verus.exe), NOT rustc.
// Compilation: verus --crate-type lib crates/doom-types/src/proofs.rs
//
// The proof file uses #[cfg(verus_keep_ghost)] to be ignored by rustc
// but processed by verus.
//
// See: https://verus-lang.github.io/verus/

// Only compiled when verus processes this file.
#![cfg(verus_keep_ghost)]

use vstd::prelude::*;

verus! {

/// Maximum coordinate value in Doom's fixed-point world (32767 map units).
/// In Fixed16_16 representation: 32767 * 65536 = 2,147,418,112.
pub const FIXED_WORLD_MAX: i64 = 32767 * 65536;

/// Proves that Fixed16_16 multiplication does not silently overflow.
///
/// When both operands are in the legal game-world range
/// [-FIXED_WORLD_MAX, FIXED_WORLD_MAX], the intermediate i64 product
/// `(a as i64) * (b as i64)` fits in i64 before the >> 16 shift.
///
/// # Formal statement
/// ∀ a b: i32,
///   |a| ≤ FIXED_WORLD_MAX ∧ |b| ≤ FIXED_WORLD_MAX
///   → |a as i64 * b as i64| ≤ i64::MAX
proof fn lemma_fixed_mul_no_overflow(a: i32, b: i32)
    requires
        -FIXED_WORLD_MAX <= a as i64,
        a as i64 <= FIXED_WORLD_MAX,
        -FIXED_WORLD_MAX <= b as i64,
        b as i64 <= FIXED_WORLD_MAX,
    ensures
        // Product fits in i64 (no overflow in the intermediate computation).
        -0x7FFF_FFFF_FFFF_FFFFi64 <= (a as i64) * (b as i64),
        (a as i64) * (b as i64) <= 0x7FFF_FFFF_FFFF_FFFFi64,
{
    // FIXED_WORLD_MAX² = (32767 * 65536)² = (2,147,418,112)² ≈ 4.61 × 10¹⁸
    // i64::MAX = 9,223,372,036,854,775,807 ≈ 9.22 × 10¹⁸
    // So the product fits with a factor of ~2 headroom.
    assert(FIXED_WORLD_MAX * FIXED_WORLD_MAX <= 0x7FFF_FFFF_FFFF_FFFFi64) by (compute);
}

/// Proves that Bam (32-bit angle) wrapping addition has a left inverse.
///
/// For all angles a and offsets b, adding then subtracting b recovers a.
/// This is the standard modular arithmetic identity.
///
/// # Formal statement
/// ∀ a b: u32,  a.wrapping_add(b).wrapping_sub(b) == a
proof fn lemma_bam_add_modular(a: u32, b: u32)
    ensures
        a.wrapping_add(b).wrapping_sub(b) == a,
{
    // u32 wrapping arithmetic satisfies the group law mod 2^32.
    // Verus can discharge this via bitvector arithmetic.
    assert(a.wrapping_add(b).wrapping_sub(b) == a) by (bit_vector);
}

/// Proves that Fixed16_16::from_int followed by to_int is the identity
/// for all integers in [i16::MIN, i16::MAX].
///
/// # Formal statement
/// ∀ n: i32, i16::MIN ≤ n ≤ i16::MAX → (n << 16) >> 16 == n
proof fn lemma_fixed_roundtrip(n: i32)
    requires
        i16::MIN as i32 <= n,
        n <= i16::MAX as i32,
    ensures
        // (n << 16) >> 16 == n  (arithmetic right shift recovers the original)
        (n << 16i32) >> 16i32 == n,
{
    // The shift-left by 16 and shift-right by 16 cancel when n fits in 16 bits.
    // Verus discharges this with bitvector arithmetic.
    assert((n << 16i32) >> 16i32 == n) by (bit_vector);
}

/// Bonus: Bam subtraction is also a left inverse of addition.
proof fn lemma_bam_sub_add_inverse(a: u32, b: u32)
    ensures
        a.wrapping_sub(b).wrapping_add(b) == a,
{
    assert(a.wrapping_sub(b).wrapping_add(b) == a) by (bit_vector);
}

} // verus!
