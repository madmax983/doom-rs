//! Fuzz target for parsing DEHACKED patches.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    let _ = doom_game::dehacked::DehPatch::parse(data);
});
