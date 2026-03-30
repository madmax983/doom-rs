#![no_main]

use doom_game::dehacked::DehPatch;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    let _ = DehPatch::parse(data);
});
