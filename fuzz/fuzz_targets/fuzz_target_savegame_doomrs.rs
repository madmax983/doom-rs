#![no_main]
use libfuzzer_sys::fuzz_target;
use doom_game::savegame::load_game;

fuzz_target!(|data: &[u8]| {
    if data.len() < 40 {
        return;
    }
    // Try to craft a valid-looking start of header so it gets past the basic checks
    let mut blob = Vec::with_capacity(45 + data.len());
    blob.extend_from_slice(b"DMR\0"); // magic
    blob.extend_from_slice(&1u32.to_le_bytes()); // version
    blob.extend_from_slice(b"E1M1\0\0\0\0"); // level_name
    blob.push(2); // skill
    blob.extend_from_slice(&0u32.to_le_bytes()); // level time
    blob.extend_from_slice(b"description goes here...\0"); // description

    blob.extend_from_slice(data); // rest of fuzzer data
    let _ = load_game(&blob);
});
