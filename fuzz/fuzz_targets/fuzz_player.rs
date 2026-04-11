#![no_main]

use libfuzzer_sys::fuzz_target;
use doom_game::{
    player::PlayerState,
    mobj::MobjHandle,
};

fuzz_target!(|data: &[u8]| {
    if data.len() < 8 { return; }
    let mut p = PlayerState::pistol_start(MobjHandle::NULL);

    let a = i32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let b = i32::from_le_bytes([data[4], data[5], data[6], data[7]]);

    p.heal(a);
    p.heal_overheal(a, b);
    p.set_health_capped(a, b);
    p.give_armor(a, data[0]);
    p.deduct_armor(b);
});
