//! Fuzz target for the Dehacked parser.
//!
//! This target ensures that `DehPatch::parse` handles arbitrary, potentially
//! malformed text without panicking.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    let _ = doom_game::dehacked::DehPatch::parse(data);
});
