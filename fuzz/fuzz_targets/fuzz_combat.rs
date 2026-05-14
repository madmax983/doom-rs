#![no_main]

use libfuzzer_sys::fuzz_target;
use doom_game::{GameState, Mobj};
use doom_types::{Bam, Fixed16_16, mobj_kind::MobjKind};

fuzz_target!(|data: &[u8]| {
    if data.len() < 16 {
        return;
    }
    unsafe { doom_types::Bam::init_trig_tables() };

    let mut gs = GameState::new("E1M1");
    let mut offset = 0;

    let src_h = gs.mobjslab.alloc(Mobj::new(
        MobjKind::Player, Fixed16_16::ZERO, Fixed16_16::ZERO, Bam::ZERO
    ));
    gs.player = doom_game::player::PlayerState::pistol_start(src_h);

    while offset + 16 <= data.len() {
        let chunk = &data[offset..offset+16];

        let x = i32::from_le_bytes(chunk[0..4].try_into().unwrap());
        let y = i32::from_le_bytes(chunk[4..8].try_into().unwrap());
        let dmg = i32::from_le_bytes(chunk[8..12].try_into().unwrap());
        let rad = i32::from_le_bytes(chunk[12..16].try_into().unwrap());

        let t_h = gs.mobjslab.alloc(Mobj::new(
            MobjKind::Trooper, Fixed16_16(x), Fixed16_16(y), Bam::ZERO
        ));
        if let Some(t) = gs.mobjslab.get_mut(t_h) {
            t.flags |= doom_game::mobj::flags::MF_SHOOTABLE;
            t.health = 20;
        }

        doom_game::combat::p_radius_attack(&mut gs, src_h, dmg, Fixed16_16(rad), None);

        offset += 16;
    }
});
