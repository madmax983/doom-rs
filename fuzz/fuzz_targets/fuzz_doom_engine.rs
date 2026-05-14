#![no_main]

use doom_types::TicCmd;
use doom_game::{GameState, tic::tick_player};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() < std::mem::size_of::<TicCmd>() {
        return;
    }

    // Unsafe: we need this for testing the game logic
    unsafe { doom_types::Bam::init_trig_tables() };

    // Setup game state to fuzz `tick_player`
    // Note: Creating a full game state without a level is safe enough
    // to test certain operations since `level` is passed as `None`.
    // It will do basic bounds clamping and angle wrapping
    let mut gs = GameState::new("E1M1");

    // Create a dummy player mobj
    let mut mo = doom_game::Mobj::new(
        doom_types::mobj_kind::MobjKind::Player,
        doom_types::Fixed16_16::ZERO,
        doom_types::Fixed16_16::ZERO,
        doom_types::Bam::ZERO,
    );
    mo.health = 100;
    mo.flags = doom_game::mobj::flags::MF_SOLID | doom_game::mobj::flags::MF_SHOOTABLE;
    let handle = gs.mobjslab.alloc(mo);
    gs.player = doom_game::player::PlayerState::pistol_start(handle);

    let mut offset = 0;
    while offset + std::mem::size_of::<TicCmd>() <= data.len() {
        let cmd = unsafe { std::ptr::read_unaligned(data.as_ptr().add(offset) as *const TicCmd) };
        tick_player(&mut gs, cmd, None);
        offset += std::mem::size_of::<TicCmd>();
    }
});
