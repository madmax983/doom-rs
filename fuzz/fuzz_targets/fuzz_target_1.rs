#![no_main]

use libfuzzer_sys::fuzz_target;
use doom_types::Fixed16_16;

fuzz_target!(|data: &[u8]| {
    if data.len() < 8 {
        return;
    }

    let a_bytes = [data[0], data[1], data[2], data[3]];
    let b_bytes = [data[4], data[5], data[6], data[7]];

    let a_val = i32::from_be_bytes(a_bytes);
    let b_val = i32::from_be_bytes(b_bytes);

    let a = Fixed16_16::from_raw(a_val);
    let b = Fixed16_16::from_raw(b_val);

    let _ = a.fixed_mul(b);
});
