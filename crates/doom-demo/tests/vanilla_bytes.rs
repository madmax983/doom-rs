//! Golden-byte regression coverage for vanilla demo payloads.

use doom_demo::DemoPlayer;
use doom_game::bt;

#[test]
fn vanilla_lmp_bytes_roundtrip_buttons_and_turn() {
    let lmp = [
        109, 3, 1, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 10, 251, 0x12, bt::BT_ATTACK | bt::BT_USE, 0x80,
    ];

    let player = DemoPlayer::from_lmp(&lmp).expect("vanilla LMP bytes must parse");

    assert_eq!(player.total_tics(), 1);
    assert_eq!(player.header().to_bytes(), vec![109, 3, 1, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0]);

    let tic = player.peek_tic().expect("tic must be present");
    assert_eq!(tic.len(), 1);
    assert_eq!(tic[0].forward_move, 10);
    assert_eq!(tic[0].side_move, -5);
    assert_eq!(tic[0].angle_turn, 0x12);
    assert_eq!(tic[0].buttons, bt::BT_ATTACK | bt::BT_USE);
}
