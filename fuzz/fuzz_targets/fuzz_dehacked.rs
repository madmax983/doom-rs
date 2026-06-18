#![no_main]

use doom_game::dehacked::DehPatch;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = DehPatch::parse(s);
    }
});
