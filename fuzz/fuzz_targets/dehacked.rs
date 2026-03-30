#![no_main]

use doom_game::DehPatch;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    let _ = DehPatch::parse(data);
});
