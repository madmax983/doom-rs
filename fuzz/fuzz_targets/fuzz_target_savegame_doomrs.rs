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
    blob.extend_from_slice(&2u32.to_le_bytes()); // version
    blob.extend_from_slice(b"E1M1\0\0\0\0"); // level_name
    blob.push(2); // skill
    blob.extend_from_slice(&0u32.to_le_bytes()); // level time
    blob.extend_from_slice(b"description goes here...\0"); // description

    // Add player state
    blob.extend_from_slice(&[0,0,0,0, 0,0,0,0]); // handle
    blob.extend_from_slice(&100i32.to_le_bytes()); // health
    blob.extend_from_slice(&0i32.to_le_bytes()); // armor
    blob.push(0); // armor_type
    blob.extend_from_slice(&[0; 4*4]); // ammo
    blob.extend_from_slice(&[0; 4*4]); // max ammo
    blob.extend_from_slice(&[0; 9]); // weapons
    blob.push(1); // weapon
    blob.push(0); // pending
    blob.push(0); // refire
    blob.push(0); // extralight
    blob.extend_from_slice(&[0; 14*2]); // psprites
    blob.push(0); // attack down
    blob.push(0); // attack cooldown
    blob.push(0); // use down
    blob.extend_from_slice(&[0; 6*4]); // powers
    blob.push(0); // keys
    blob.extend_from_slice(&[0; 5*4]); // counts

    // Add RNG + global counts
    blob.extend_from_slice(&[0; 9*4]);

    // level string
    blob.extend_from_slice(&4u32.to_le_bytes());
    blob.extend_from_slice(b"E1M1");

    // exit
    blob.push(0);

    // no active stuff
    blob.extend_from_slice(&0u32.to_le_bytes()); // doors
    blob.extend_from_slice(&0u32.to_le_bytes()); // lights
    blob.extend_from_slice(&0u32.to_le_bytes()); // ceil
    blob.extend_from_slice(&0u32.to_le_bytes()); // floor
    blob.extend_from_slice(&0u32.to_le_bytes()); // plats
    blob.extend_from_slice(&0u32.to_le_bytes()); // lifts
    blob.extend_from_slice(&0u32.to_le_bytes()); // scrolls
    blob.extend_from_slice(&0u32.to_le_bytes()); // convey

    blob.extend_from_slice(data); // rest of fuzzer data
    let _ = load_game(&blob);
});
