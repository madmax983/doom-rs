//! CRC32 (IEEE 802.3) checksum for desync detection.
//!
//! Table-driven implementation using the standard polynomial `0xEDB8_8320`
//! (reflected form of `0x04C1_1DB7`).  Not cryptographic -- purely for
//! detecting state divergence between networked game clients.

/// IEEE 802.3 CRC32 polynomial in reflected (LSB-first) form.
const CRC32_POLY: u32 = 0xEDB8_8320;

/// Precomputed CRC32 lookup table (256 entries, one per byte value).
///
/// Generated at compile time via `const fn`.
pub const CRC32_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut i: usize = 0;
    while i < 256 {
        let mut crc = i as u32;
        let mut bit = 0;
        while bit < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ CRC32_POLY;
            } else {
                crc >>= 1;
            }
            bit += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
};

/// Compute the CRC32 (IEEE) checksum of `data`.
///
/// Returns `0` for an empty slice (the complement of `0xFFFF_FFFF` XOR
/// `0xFFFF_FFFF`).  This is only used for desync detection, not security.
#[must_use]
/// Computes a deterministic checksum over the given data slice.
pub fn compute_checksum(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        let index = ((crc ^ u32::from(byte)) & 0xFF) as usize;
        crc = (crc >> 8) ^ CRC32_TABLE[index];
    }
    crc ^ 0xFFFF_FFFF
}

/// Returns `true` if two checksums match.
///
/// Trivial wrapper -- exists so call sites read as intent rather than `==`.
#[must_use]
pub const fn checksums_match(local: u32, remote: u32) -> bool {
    local == remote
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_empty_data() {
        // CRC32 of empty input is 0x0000_0000 by IEEE definition.
        let crc = compute_checksum(&[]);
        assert_eq!(crc, 0x0000_0000, "CRC32 of empty data must be 0");
    }

    #[test]
    fn checksum_known_value() {
        // CRC32 of ASCII "123456789" is 0xCBF4_3926 (standard test vector).
        let crc = compute_checksum(b"123456789");
        assert_eq!(
            crc, 0xCBF4_3926,
            "CRC32 of '123456789' must match the IEEE test vector"
        );
    }

    #[test]
    fn checksums_match_identical() {
        assert!(
            checksums_match(0xDEAD_BEEF, 0xDEAD_BEEF),
            "identical checksums must match"
        );
    }

    #[test]
    fn checksums_match_different() {
        assert!(
            !checksums_match(0xDEAD_BEEF, 0xCAFE_BABE),
            "different checksums must not match"
        );
    }

    #[test]
    fn checksum_single_byte() {
        // Sanity: CRC32 of a single zero byte.
        let crc = compute_checksum(&[0x00]);
        // Known value: CRC32(0x00) = 0xD202_EF8D
        assert_eq!(crc, 0xD202_EF8D);
    }

    #[test]
    fn checksum_table_first_entry_is_zero() {
        assert_eq!(CRC32_TABLE[0], 0, "CRC32 table[0] must be 0");
    }

    #[test]
    fn checksum_deterministic() {
        let data = b"doom rollback netcode";
        let a = compute_checksum(data);
        let b = compute_checksum(data);
        assert_eq!(a, b, "CRC32 must be deterministic");
    }
}
