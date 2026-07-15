//! 👹 Havoc: Dehacked Patch Fuzzing Target
//!
//! This fuzz target feeds arbitrary, mutated byte streams into the Dehacked (`DehPatch`)
//! parser. The goal is to discover unexpected panics or out-of-bounds accesses caused
//! by malformed text input, ensuring the parser remains robust against garbage data.



#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    let _ = doom_game::dehacked::DehPatch::parse(data);
});
