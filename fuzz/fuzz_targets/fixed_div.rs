#![no_main]
use libfuzzer_sys::fuzz_target;
use doom_types::Fixed16_16;

fuzz_target!(|data: (i32, i32)| {
    let (a, b) = data;
    if b != 0 {
        let _ = Fixed16_16(a).fixed_div(Fixed16_16(b));
    }
});
