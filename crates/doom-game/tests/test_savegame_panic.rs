#![allow(missing_docs)]
use doom_game::{GameState, savegame::*};

#[test]
fn load_game_panic() {
    let mut gs = GameState::new("E1M1");
    // Insert a mobj to make sure it's serialized.
    let mobj = doom_game::Mobj::new(
        doom_types::mobj_kind::MobjKind::Imp,
        doom_types::Fixed16_16::from_int(100),
        doom_types::Fixed16_16::from_int(100),
        doom_types::Bam::ZERO,
    );
    gs.mobjslab.alloc(mobj);

    let mut data = save_game(&gs, b"E1M1\0\0\0\0", 2, "havoc");

    let len = data.len();
    data[len - 8] = 0xff;
    data[len - 7] = 0xff;
    data[len - 6] = 0xff;
    data[len - 5] = 0xff;

    let res = load_game(&data);
    assert_eq!(res.unwrap_err(), SaveError::Truncated);
}
