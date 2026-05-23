//! Fuzz target for `DehPatch` parsing.
//!
//! This fuzz target ensures that parsing arbitrary Dehacked patch data does not
//! result in panics or crashes, providing robustness against malformed input.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    let _ = doom_game::dehacked::DehPatch::parse(data);
});
