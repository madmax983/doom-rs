fn main() {
    let mut delta: u32 = 0;
    let shift = 28;
    let b: u8 = 0x80 | 0x20;
    let res = u32::from(b & 0x7F).checked_shl(shift).unwrap_or(0);
    delta |= res;
    println!("delta: {:?}", delta);
}
