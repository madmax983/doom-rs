//! Fuzz target for Dehacked patch parsing.
//!
//! Feeds random byte sequences into `DehPatch::parse` to ensure it does not panic
//! on malformed input.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    let _ = doom_game::dehacked::DehPatch::parse(data);
});
