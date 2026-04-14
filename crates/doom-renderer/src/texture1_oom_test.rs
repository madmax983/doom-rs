//! Test for Out-Of-Memory (OOM) handling in the TEXTURE1 parser.
#[cfg(test)]
mod tests {
    use crate::texture_compose::parse_texture_lump;

    #[test]
    fn test_texture1_oom() {
        // 4-byte count = 4,000,000,000 (0xEE6B2800) -> Vec::with_capacity(4000000000)
        let data: [u8; 4] = [0x00, 0x28, 0x6b, 0xee];
        let _ = parse_texture_lump(&data);
    }
}
