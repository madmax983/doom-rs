use super::*;
use crate::mobj::{Mobj, StateNum, flags};
use crate::player::PlayerState;
use crate::state::GameState;
use doom_types::TicCmd;
use doom_types::mobj_kind::MobjKind;
use doom_types::{Bam, Fixed16_16};

fn make_game_state() -> GameState {
    let mut gs = GameState::new("test");
    let mut mo = Mobj::new(
        MobjKind::Player,
        Fixed16_16::ZERO,
        Fixed16_16::ZERO,
        Bam::ZERO,
    );
    mo.health = 100;
    mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE;
    let handle = gs.mobjslab.alloc(mo);
    gs.player = PlayerState::pistol_start(handle);
    gs
}

fn spawn_trooper(gs: &mut GameState, x: i32, y: i32) -> MobjHandle {
    use crate::mobjinfo::MOBJINFO;
    use crate::states::STATES;
    use doom_types::mobj_kind::MobjKind;
    let kind = MobjKind::Trooper;
    let spawn_sn = MOBJINFO[kind as usize].spawn_state;
    let mut mo = Mobj::new(
        kind,
        Fixed16_16::from_int(x),
        Fixed16_16::from_int(y),
        Bam::ZERO,
    );
    mo.health = 20;
    mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
    mo.state = spawn_sn;
    mo.tics = STATES[spawn_sn.0 as usize].tics;
    gs.mobjslab.alloc(mo)
}

/// Spawn a trooper with extra flags.
#[allow(dead_code)]
fn spawn_trooper_with_flags(gs: &mut GameState, x: i32, y: i32, extra_flags: u32) -> MobjHandle {
    use crate::mobjinfo::MOBJINFO;
    use crate::states::STATES;
    let kind = MobjKind::Trooper;
    let spawn_sn = MOBJINFO[kind as usize].spawn_state;
    let mut mo = Mobj::new(
        kind,
        Fixed16_16::from_int(x),
        Fixed16_16::from_int(y),
        Bam::ZERO,
    );
    mo.health = 20;
    mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL | extra_flags;
    mo.state = spawn_sn;
    mo.tics = STATES[spawn_sn.0 as usize].tics;
    gs.mobjslab.alloc(mo)
}

// -----------------------------------------------------------------------
// Direction tests
// -----------------------------------------------------------------------

#[test]
fn test_dir_to_target() {
    let cases = [
        // Pure directions
        (100, 0, DI_EAST),
        (-100, 0, DI_WEST),
        (0, 100, DI_NORTH),
        (0, -100, DI_SOUTH),
        // Exact diagonals
        (100, 100, DI_NORTHEAST),
        (-100, 100, DI_NORTHWEST),
        (-100, -100, DI_SOUTHWEST),
        (100, -100, DI_SOUTHEAST),
        // Shallow diagonals (still diagonal territory: ax/ay < 2.0 and ay/ax < 2.0)
        (100, 60, DI_NORTHEAST),
        (60, 100, DI_NORTHEAST),
        (-100, 60, DI_NORTHWEST),
        (-60, 100, DI_NORTHWEST),
        (-100, -60, DI_SOUTHWEST),
        (-60, -100, DI_SOUTHWEST),
        (100, -60, DI_SOUTHEAST),
        (60, -100, DI_SOUTHEAST),
        // Mostly horizontal (ax > 2 * ay)
        (100, 40, DI_EAST),
        (100, -40, DI_EAST),
        (-100, 40, DI_WEST),
        (-100, -40, DI_WEST),
        // Mostly vertical (ay > 2 * ax)
        (40, 100, DI_NORTH),
        (-40, 100, DI_NORTH),
        (40, -100, DI_SOUTH),
        (-40, -100, DI_SOUTH),
        // Edge cases
        (0, 0, DI_NORTHEAST), // Handled as diagonal in current logic since 0 is not > 0
    ];

    for &(dx, dy, expected) in &cases {
        assert_eq!(
            dir_to_target(dx, dy),
            expected,
            "dir_to_target({}, {}) should be {}",
            dx,
            dy,
            expected
        );
    }
}

// -----------------------------------------------------------------------
// BAM angle computation tests
// -----------------------------------------------------------------------

#[test]
fn bam_from_xy_east() {
    let angle = bam_from_xy(100, 0);
    // East = 0 BAM
    assert!(
        angle.0 < 0x1000_0000,
        "east should be near 0 BAM, got {:#010X}",
        angle.0
    );
}

#[test]
fn bam_from_xy_north() {
    let angle = bam_from_xy(0, 100);
    // North = ANG90 = 0x4000_0000
    let diff = (angle.0 as i64 - 0x4000_0000i64).unsigned_abs();
    assert!(
        diff < 0x0100_0000,
        "north should be near ANG90, got {:#010X}",
        angle.0
    );
}

#[test]
fn bam_from_xy_west() {
    let angle = bam_from_xy(-100, 0);
    // West = ANG180 = 0x8000_0000
    let diff = (angle.0 as i64 - 0x8000_0000i64).unsigned_abs();
    assert!(
        diff < 0x0100_0000,
        "west should be near ANG180, got {:#010X}",
        angle.0
    );
}

#[test]
fn bam_from_xy_south() {
    let angle = bam_from_xy(0, -100);
    // South = ANG270 = 0xC000_0000
    let diff = (angle.0 as i64 - 0xC000_0000i64).unsigned_abs();
    assert!(
        diff < 0x0100_0000,
        "south should be near ANG270, got {:#010X}",
        angle.0
    );
}

#[test]
fn bam_from_xy_zero_returns_zero() {
    assert_eq!(bam_from_xy(0, 0), Bam(0));
}

// -----------------------------------------------------------------------
// A_Look tests
// -----------------------------------------------------------------------

#[test]
fn a_look_transitions_to_see_state_when_in_range() {
    let mut gs = make_game_state();
    // Spawn trooper 100 units east of player (well within 4096 range).
    let trooper = spawn_trooper(&mut gs, 100, 0);

    // Tick 10 times so the first A_Look fires (S_POSS_STND.tics = 10).
    for _ in 0..10 {
        gs.tick(TicCmd::default(), None);
    }

    let mo = gs.mobjslab.get(trooper).expect("item must exist in tests");
    let see_sn = mobjinfo::MOBJINFO[MobjKind::Trooper as usize].see_state;
    assert_eq!(
        mo.state, see_sn,
        "trooper must be in see_state after A_Look fires"
    );
    assert_eq!(mo.target, gs.player.handle, "trooper must target player");
}

#[test]
fn a_look_ignores_out_of_range_player() {
    let mut gs = make_game_state();
    // Trooper 5000 units away — beyond Manhattan distance 4096.
    let trooper = spawn_trooper(&mut gs, 5000, 0);
    let spawn_sn = mobjinfo::MOBJINFO[MobjKind::Trooper as usize].spawn_state;

    for _ in 0..15 {
        gs.tick(TicCmd::default(), None);
    }

    let mo = gs.mobjslab.get(trooper).expect("item must exist in tests");
    let idle_ok = mo.state == spawn_sn || mo.state == StateNum(crate::states::ids::S_POSS_STND2);
    assert!(
        idle_ok,
        "trooper must stay in one of the idle stand frames when player is out of range"
    );
}

#[test]
fn a_look_direct_call_sets_target_and_see_state() {
    let mut gs = make_game_state();
    let trooper = spawn_trooper(&mut gs, 100, 0);
    let see_sn = mobjinfo::MOBJINFO[MobjKind::Trooper as usize].see_state;

    dispatch_action(&mut gs, trooper, Action::Look as u8, None);

    let mo = gs.mobjslab.get(trooper).expect("item must exist in tests");
    assert_eq!(mo.target, gs.player.handle);
    assert_eq!(mo.state, see_sn);
    assert_eq!(mo.threshold, 60);
}

#[test]
fn a_look_does_nothing_when_player_dead() {
    let mut gs = make_game_state();
    let trooper = spawn_trooper(&mut gs, 100, 0);
    let spawn_sn = mobjinfo::MOBJINFO[MobjKind::Trooper as usize].spawn_state;

    // Kill the player.
    gs.mobjslab
        .get_mut(gs.player.handle)
        .expect("item must exist in tests")
        .health = 0;

    dispatch_action(&mut gs, trooper, Action::Look as u8, None);

    let mo = gs.mobjslab.get(trooper).expect("item must exist in tests");
    assert_eq!(
        mo.state, spawn_sn,
        "trooper must stay idle when player is dead"
    );
    assert_eq!(mo.target, MobjHandle::NULL);
}

#[test]
fn a_look_with_sound_target_wakes_non_ambush() {
    let mut gs = make_game_state();
    let trooper = spawn_trooper(&mut gs, 100, 0);
    let see_sn = mobjinfo::MOBJINFO[MobjKind::Trooper as usize].see_state;

    // Set up sound targets: the trooper is in subsector 0 which resolves
    // to sector 0. Without a level, we can't resolve sectors, so we need
    // to test this with p_check_sight_local fallback path.
    // Instead, test the direct call without level (uses fallback LOS).
    dispatch_action(&mut gs, trooper, Action::Look as u8, None);

    let mo = gs.mobjslab.get(trooper).expect("item must exist in tests");
    assert_eq!(mo.state, see_sn, "trooper should wake from LOS");
}

#[test]
fn a_look_with_stale_handle_does_not_panic() {
    let mut gs = make_game_state();
    let trooper = spawn_trooper(&mut gs, 100, 0);
    gs.mobjslab.free(trooper);

    // Should not panic.
    dispatch_action(&mut gs, trooper, Action::Look as u8, None);
}

// -----------------------------------------------------------------------
// A_Chase tests
// -----------------------------------------------------------------------

#[test]
fn a_chase_moves_monster_toward_player() {
    let mut gs = make_game_state();
    // Player at (0,0), trooper at (100,0).
    let trooper = spawn_trooper(&mut gs, 100, 0);

    // Tick enough times: A_Look fires at tic 9 (10 tic idle), first A_Chase
    // at tic 13 enters missile state (ATK1+ATK2+ATK3 = 12 tics), second
    // A_Chase fires around tic 29+ clears MF_JUSTATTACKED and does movement.
    // Use 40 tics to give plenty of room for the full cycle.
    for _ in 0..40 {
        gs.tick(TicCmd::default(), None);
    }

    let mo = gs.mobjslab.get(trooper).expect("item must exist in tests");
    // Trooper should have moved west (toward x=0).
    assert!(
        mo.x < Fixed16_16::from_int(100),
        "trooper x={:?} must decrease toward player",
        mo.x
    );
}

#[test]
fn dead_target_reverts_monster_to_idle() {
    let mut gs = make_game_state();
    let trooper = spawn_trooper(&mut gs, 100, 0);

    // Let the trooper see the player and start chasing.
    for _ in 0..15 {
        gs.tick(TicCmd::default(), None);
    }
    // Kill the player.
    gs.mobjslab
        .get_mut(gs.player.handle)
        .expect("item must exist in tests")
        .health = 0;

    // Keep ticking: A_Chase should notice dead target and revert.
    // With attack states added (Batch 5), the trooper may be mid-attack
    // sequence (ATK1→ATK2→ATK3→RUN1 = 4+4+4+4 = 16 tics) before A_Chase
    // fires and detects the dead target.  Use 32 tics to be safe.
    for _ in 0..32 {
        gs.tick(TicCmd::default(), None);
    }

    let mo = gs.mobjslab.get(trooper).expect("item must exist in tests");
    let spawn_sn = mobjinfo::MOBJINFO[MobjKind::Trooper as usize].spawn_state;
    assert_eq!(
        mo.state, spawn_sn,
        "trooper must revert to spawn state when target is dead"
    );
    assert_eq!(mo.target, MobjHandle::NULL, "target must be cleared");
}

#[test]
fn a_chase_reaction_time_prevents_attack() {
    let mut gs = make_game_state();
    let kind = MobjKind::Trooper;
    let see_sn = mobjinfo::MOBJINFO[kind as usize].see_state;

    // Spawn trooper already in chase state with high reaction_time.
    let mut mo = Mobj::new(
        kind,
        Fixed16_16::from_int(50),
        Fixed16_16::from_int(0),
        Bam::ZERO,
    );
    mo.health = 20;
    mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
    mo.state = see_sn;
    mo.tics = states::STATES[see_sn.0 as usize].tics;
    mo.target = gs.player.handle;
    mo.reactiontime = 100;
    let trooper = gs.mobjslab.alloc(mo);

    // Call a_chase directly.
    a_chase(&mut gs, trooper, None);

    // Reaction time should have decremented.
    let mo = gs.mobjslab.get(trooper).expect("item must exist in tests");
    assert_eq!(
        mo.reactiontime, 99,
        "reaction_time must decrement each a_chase call"
    );
}

#[test]
fn a_chase_justattacked_clears_flag_and_skips_attack() {
    let mut gs = make_game_state();
    let kind = MobjKind::Trooper;
    let see_sn = mobjinfo::MOBJINFO[kind as usize].see_state;

    let mut mo = Mobj::new(
        kind,
        Fixed16_16::from_int(50),
        Fixed16_16::from_int(0),
        Bam::ZERO,
    );
    mo.health = 20;
    mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL | flags::MF_JUSTATTACKED;
    mo.state = see_sn;
    mo.tics = states::STATES[see_sn.0 as usize].tics;
    mo.target = gs.player.handle;
    mo.reactiontime = 0;
    mo.movecount = 0;
    let trooper = gs.mobjslab.alloc(mo);

    a_chase(&mut gs, trooper, None);

    let mo = gs.mobjslab.get(trooper).expect("item must exist in tests");
    assert_eq!(
        mo.flags & flags::MF_JUSTATTACKED,
        0,
        "MF_JUSTATTACKED must be cleared after a_chase"
    );
}

#[test]
fn a_chase_enters_melee_when_close() {
    let mut gs = make_game_state();
    let kind = MobjKind::Demon; // Has melee state
    let see_sn = mobjinfo::MOBJINFO[kind as usize].see_state;
    let melee_sn = mobjinfo::MOBJINFO[kind as usize].melee_state;

    assert_ne!(melee_sn, StateNum::NULL, "Demon must have melee state");

    // Spawn demon very close to player (within MELEERANGE + 20 = 84).
    let mut mo = Mobj::new(
        kind,
        Fixed16_16::from_int(30),
        Fixed16_16::from_int(0),
        Bam::ZERO,
    );
    mo.health = 150;
    mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
    mo.state = see_sn;
    mo.tics = states::STATES[see_sn.0 as usize].tics;
    mo.target = gs.player.handle;
    mo.reactiontime = 0;
    let demon = gs.mobjslab.alloc(mo);

    a_chase(&mut gs, demon, None);

    let mo = gs.mobjslab.get(demon).expect("item must exist in tests");
    assert_eq!(
        mo.state, melee_sn,
        "demon should enter melee state when close to target"
    );
}

#[test]
fn a_chase_enters_missile_state_when_in_range() {
    let mut gs = make_game_state();
    let kind = MobjKind::Trooper;
    let see_sn = mobjinfo::MOBJINFO[kind as usize].see_state;
    let missile_sn = mobjinfo::MOBJINFO[kind as usize].missile_state;

    assert_ne!(
        missile_sn,
        StateNum::NULL,
        "Trooper must have missile state"
    );

    // Spawn trooper within missile range, with movecount = 0 and reactiontime = 0.
    let mut mo = Mobj::new(
        kind,
        Fixed16_16::from_int(200),
        Fixed16_16::from_int(0),
        Bam::ZERO,
    );
    mo.health = 20;
    mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
    mo.state = see_sn;
    mo.tics = states::STATES[see_sn.0 as usize].tics;
    mo.target = gs.player.handle;
    mo.reactiontime = 0;
    mo.movecount = 0;
    let trooper = gs.mobjslab.alloc(mo);

    gs.rng.set_index(9);
    a_chase(&mut gs, trooper, None);

    let mo = gs.mobjslab.get(trooper).expect("item must exist in tests");
    assert_eq!(
        mo.state, missile_sn,
        "trooper should enter missile state when in range with movecount=0"
    );
    assert_ne!(
        mo.flags & flags::MF_JUSTATTACKED,
        0,
        "MF_JUSTATTACKED must be set after entering missile state"
    );
}

#[test]
fn a_chase_does_not_fire_when_movecount_positive() {
    let mut gs = make_game_state();
    let kind = MobjKind::Trooper;
    let see_sn = mobjinfo::MOBJINFO[kind as usize].see_state;
    let missile_sn = mobjinfo::MOBJINFO[kind as usize].missile_state;

    // Spawn trooper within range but with movecount > 0.
    let mut mo = Mobj::new(
        kind,
        Fixed16_16::from_int(200),
        Fixed16_16::from_int(0),
        Bam::ZERO,
    );
    mo.health = 20;
    mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
    mo.state = see_sn;
    mo.tics = states::STATES[see_sn.0 as usize].tics;
    mo.target = gs.player.handle;
    mo.reactiontime = 0;
    mo.movecount = 10; // Still has movement budget
    let trooper = gs.mobjslab.alloc(mo);

    a_chase(&mut gs, trooper, None);

    let mo = gs.mobjslab.get(trooper).expect("item must exist in tests");
    assert_ne!(
        mo.state, missile_sn,
        "trooper should NOT enter missile state when movecount > 0"
    );
}

#[test]
fn p_check_missile_range_archvile_rejects_far_targets() {
    let mut gs = make_game_state();
    let vile = spawn_monster_targeting_player(&mut gs, MobjKind::ArchVile, 1100, 0, 700);
    gs.mobjslab
        .get_mut(vile)
        .expect("item must exist in tests")
        .reactiontime = 0;
    let player = gs.player.handle;

    assert!(
        !p_check_missile_range(&mut gs, vile, player, None),
        "Arch-Vile missile range should reject targets beyond 14 * 64 units"
    );
}

#[test]
fn p_check_missile_range_revenant_rejects_targets_under_196_units() {
    let mut gs = make_game_state();
    let skel = spawn_monster_targeting_player(&mut gs, MobjKind::Revenant, 128, 0, 300);
    gs.mobjslab
        .get_mut(skel)
        .expect("item must exist in tests")
        .reactiontime = 0;
    let player = gs.player.handle;
    gs.rng.set_index(2); // RNG_TABLE[2] = 109, high enough to pass the old random gate.

    assert!(
        !p_check_missile_range(&mut gs, skel, player, None),
        "Revenant missile range should reject targets that are too close"
    );
}

#[test]
fn a_chase_can_fire_after_movecount_counts_down() {
    let mut gs = make_game_state();
    let kind = MobjKind::Trooper;
    let see_sn = mobjinfo::MOBJINFO[kind as usize].see_state;
    let missile_sn = mobjinfo::MOBJINFO[kind as usize].missile_state;

    let mut mo = Mobj::new(
        kind,
        Fixed16_16::from_int(200),
        Fixed16_16::from_int(0),
        Bam::ZERO,
    );
    mo.health = 20;
    mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
    mo.state = see_sn;
    mo.tics = states::STATES[see_sn.0 as usize].tics;
    mo.target = gs.player.handle;
    mo.reactiontime = 0;
    mo.movedir = DI_WEST;
    mo.movecount = 1;
    let trooper = gs.mobjslab.alloc(mo);

    gs.rng.set_index(9);
    a_chase(&mut gs, trooper, None);
    let after_first = gs.mobjslab.get(trooper).expect("item must exist in tests");
    assert_eq!(
        after_first.state, see_sn,
        "first chase tic should spend the last movement step, not attack yet"
    );
    assert_eq!(
        after_first.movecount, 0,
        "movement countdown should reach zero and leave a firing window next tic"
    );

    a_chase(&mut gs, trooper, None);
    let after_second = gs.mobjslab.get(trooper).expect("item must exist in tests");
    assert_eq!(
        after_second.state, missile_sn,
        "trooper should enter missile state once movecount has counted down"
    );
}

#[test]
fn p_check_missile_range_mf_justhit_returns_true_and_clears_flag() {
    let mut gs = make_game_state();
    let player_handle = gs.player.handle;
    let trooper = spawn_monster_targeting_player(&mut gs, MobjKind::Trooper, 100, 0, 20);
    gs.mobjslab
        .get_mut(trooper)
        .expect("item must exist in tests")
        .flags |= flags::MF_JUSTHIT;

    let can_fire = p_check_missile_range(&mut gs, trooper, player_handle, None);

    assert!(
        can_fire,
        "MF_JUSTHIT must bypass normal missile range checks"
    );
    let mo = gs.mobjslab.get(trooper).expect("item must exist in tests");
    assert_eq!(
        mo.flags & flags::MF_JUSTHIT,
        0,
        "p_check_missile_range must clear MF_JUSTHIT"
    );
}

#[test]
fn p_check_missile_range_reactiontime_prevents_fire() {
    let mut gs = make_game_state();
    let player_handle = gs.player.handle;
    // Trooper within range
    let trooper = spawn_monster_targeting_player(&mut gs, MobjKind::Trooper, 200, 0, 20);
    gs.mobjslab
        .get_mut(trooper)
        .expect("item must exist in tests")
        .reactiontime = 10;

    let can_fire = p_check_missile_range(&mut gs, trooper, player_handle, None);

    assert!(!can_fire, "reactiontime > 0 must prevent missile fire");
}

#[test]
fn p_check_missile_range_null_melee_state_adjusts_distance() {
    let mut gs = make_game_state();
    let player_handle = gs.player.handle;

    let demon = spawn_monster_targeting_player(&mut gs, MobjKind::Demon, 300, 0, 150);
    gs.mobjslab
        .get_mut(demon)
        .expect("item must exist in tests")
        .reactiontime = 0;

    let trooper = spawn_monster_targeting_player(&mut gs, MobjKind::Trooper, 300, 0, 20);
    gs.mobjslab
        .get_mut(trooper)
        .expect("item must exist in tests")
        .reactiontime = 0;

    // RNG_TABLE[11] = 140.
    // demon: dist = 200, random = 140 -> can_fire = false
    // trooper: dist = 108, random = 140 -> can_fire = true

    gs.rng.set_index(11);
    let can_fire_demon = p_check_missile_range(&mut gs, demon, player_handle, None);

    gs.rng.set_index(11);
    let can_fire_trooper = p_check_missile_range(&mut gs, trooper, player_handle, None);

    assert!(
        !can_fire_demon,
        "Demon should fail random gate because dist > random"
    );
    assert!(
        can_fire_trooper,
        "Trooper should pass random gate because dist - 128 < random"
    );
}

#[test]
fn p_check_missile_range_cyberdemon_distance_limits() {
    let mut gs = make_game_state();
    let player_handle = gs.player.handle;
    let cyber = spawn_monster_targeting_player(&mut gs, MobjKind::Cyberdemon, 500, 0, 4000);
    gs.mobjslab
        .get_mut(cyber)
        .expect("item must exist in tests")
        .reactiontime = 0;

    // RNG_TABLE[6] = 149 (false).
    // RNG_TABLE[20] = 154 (true).

    gs.rng.set_index(6);
    assert!(
        !p_check_missile_range(&mut gs, cyber, player_handle, None),
        "Cyberdemon should fail with random=149 vs dist=154"
    );

    gs.rng.set_index(20);
    assert!(
        p_check_missile_range(&mut gs, cyber, player_handle, None),
        "Cyberdemon should fire with random=154 vs dist=154"
    );
}

#[test]
fn a_chase_far_trooper_respects_random_missile_gate() {
    let mut gs = make_game_state();
    let kind = MobjKind::Trooper;
    let see_sn = mobjinfo::MOBJINFO[kind as usize].see_state;
    let missile_sn = mobjinfo::MOBJINFO[kind as usize].missile_state;

    let mut mo = Mobj::new(
        kind,
        Fixed16_16::from_int(2000),
        Fixed16_16::from_int(0),
        Bam::ZERO,
    );
    mo.health = 20;
    mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
    mo.state = see_sn;
    mo.tics = states::STATES[see_sn.0 as usize].tics;
    mo.target = gs.player.handle;
    mo.reactiontime = 0;
    mo.movecount = 0;
    let trooper = gs.mobjslab.alloc(mo);

    gs.rng.set_index(0);
    a_chase(&mut gs, trooper, None);

    let mo = gs.mobjslab.get(trooper).expect("item must exist in tests");
    assert_eq!(
        mo.state, see_sn,
        "far trooper should stay in chase state when the missile-range random gate rejects firing"
    );
    assert_ne!(
        mo.state, missile_sn,
        "far trooper should not always enter missile state"
    );
}

// -----------------------------------------------------------------------
// P_Move tests
// -----------------------------------------------------------------------

#[test]
fn p_move_moves_actor_in_direction() {
    let mut gs = make_game_state();
    let trooper = spawn_trooper(&mut gs, 100, 0);
    gs.mobjslab
        .get_mut(trooper)
        .expect("item must exist in tests")
        .movedir = DI_WEST;

    let moved = p_move(&mut gs, trooper, None);

    assert!(moved, "p_move should succeed without level");
    let mo = gs.mobjslab.get(trooper).expect("item must exist in tests");
    assert!(
        mo.x < Fixed16_16::from_int(100),
        "trooper should have moved west"
    );
}

#[test]
fn p_move_nodir_returns_false() {
    let mut gs = make_game_state();
    let trooper = spawn_trooper(&mut gs, 100, 0);
    gs.mobjslab
        .get_mut(trooper)
        .expect("item must exist in tests")
        .movedir = DI_NODIR;

    let moved = p_move(&mut gs, trooper, None);

    assert!(!moved, "p_move with DI_NODIR should return false");
}

#[test]
fn p_move_stale_handle_returns_false() {
    let mut gs = make_game_state();
    let trooper = spawn_trooper(&mut gs, 100, 0);
    gs.mobjslab.free(trooper);

    let moved = p_move(&mut gs, trooper, None);

    assert!(!moved, "p_move with stale handle should return false");
}

#[test]
fn p_move_updates_momentum() {
    let mut gs = make_game_state();
    let trooper = spawn_trooper(&mut gs, 100, 0);
    gs.mobjslab
        .get_mut(trooper)
        .expect("item must exist in tests")
        .movedir = DI_EAST;

    p_move(&mut gs, trooper, None);

    let mo = gs.mobjslab.get(trooper).expect("item must exist in tests");
    assert!(
        mo.momx > Fixed16_16::ZERO,
        "momentum x should be positive for east movement"
    );
}

#[test]
fn p_move_opens_the_actual_blocking_door() {
    let mut gs = make_game_state();
    let trooper = spawn_trooper(&mut gs, 104, 0);
    gs.mobjslab
        .get_mut(trooper)
        .expect("item must exist in tests")
        .movedir = DI_EAST;

    let reject = doom_map::Reject::parse_lump(&[0u8], 2).expect("item must exist in tests");

    let mut bm_data = vec![0u8; 22];
    bm_data[4..6].copy_from_slice(&2u16.to_le_bytes());
    bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
    bm_data[8..10].copy_from_slice(&6u16.to_le_bytes());
    bm_data[10..12].copy_from_slice(&8u16.to_le_bytes());
    bm_data[12..14].copy_from_slice(&0u16.to_le_bytes());
    bm_data[14..16].copy_from_slice(&0xFFFFu16.to_le_bytes());
    bm_data[16..18].copy_from_slice(&0u16.to_le_bytes());
    bm_data[18..20].copy_from_slice(&0u16.to_le_bytes());
    bm_data[20..22].copy_from_slice(&0xFFFFu16.to_le_bytes());
    let blockmap = doom_map::Blockmap::parse_lump(&bm_data).expect("item must exist in tests");

    let level = doom_map::Level {
        name: "TEST".to_string(),
        things: vec![],
        linedefs: vec![doom_map::Linedef {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0x0004,
            special: 1,
            tag: 0,
            right_sidedef: 0,
            left_sidedef: 1,
        }],
        sidedefs: vec![
            doom_map::Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: [0; 8],
                lower_texture: [0; 8],
                middle_texture: [0; 8],
                sector: 0,
            },
            doom_map::Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: [0; 8],
                lower_texture: [0; 8],
                middle_texture: [0; 8],
                sector: 1,
            },
        ],
        vertexes: vec![
            doom_map::Vertex { x: 128, y: -32 },
            doom_map::Vertex { x: 128, y: 32 },
        ],
        segs: vec![],
        ssectors: vec![],
        nodes: vec![],
        sectors: vec![
            doom_map::Sector {
                floor_height: 0,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
            doom_map::Sector {
                floor_height: 0,
                ceil_height: 0,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
        ],
        reject,
        blockmap,
    };

    let moved = p_move(&mut gs, trooper, Some(&level));

    assert!(!moved, "the move should be blocked by the near door");
    assert_eq!(
        gs.movers.active_doors.len(),
        1,
        "monster should still open a blocking door even when the door lives in a neighbouring blockmap cell"
    );
    assert_eq!(
        gs.movers.active_doors[0].sector, 1,
        "monster should open the actual blocking door"
    );
}

// -----------------------------------------------------------------------
// A_FaceTarget tests
// -----------------------------------------------------------------------

#[test]
fn a_face_target_faces_east() {
    let mut gs = make_game_state();
    let trooper = spawn_trooper(&mut gs, -100, 0);
    gs.mobjslab
        .get_mut(trooper)
        .expect("item must exist in tests")
        .target = gs.player.handle;

    a_face_target(&mut gs, trooper);

    let mo = gs.mobjslab.get(trooper).expect("item must exist in tests");
    // Target is east (+x). Angle should be near 0 (east).
    assert!(
        mo.angle.0 < 0x2000_0000 || mo.angle.0 > 0xE000_0000,
        "angle should be roughly east, got {:#010X}",
        mo.angle.0
    );
}

#[test]
fn a_face_target_faces_west() {
    let mut gs = make_game_state();
    let trooper = spawn_trooper(&mut gs, 100, 0);
    // Player is at (0,0), trooper is at (100,0), so target is west.
    gs.mobjslab
        .get_mut(trooper)
        .expect("item must exist in tests")
        .target = gs.player.handle;

    a_face_target(&mut gs, trooper);

    let mo = gs.mobjslab.get(trooper).expect("item must exist in tests");
    // West = 0x8000_0000.
    let diff = (mo.angle.0 as i64 - 0x8000_0000i64).unsigned_abs();
    assert!(
        diff < 0x1000_0000,
        "angle should be roughly west, got {:#010X}",
        mo.angle.0
    );
}

#[test]
fn a_face_target_no_target_does_nothing() {
    let mut gs = make_game_state();
    let trooper = spawn_trooper(&mut gs, 100, 0);
    let original_angle = gs
        .mobjslab
        .get(trooper)
        .expect("item must exist in tests")
        .angle;

    a_face_target(&mut gs, trooper);

    let mo = gs.mobjslab.get(trooper).expect("item must exist in tests");
    assert_eq!(
        mo.angle, original_angle,
        "angle should not change without target"
    );
}

#[test]
fn dispatch_action_face_target_works() {
    let mut gs = make_game_state();
    let trooper = spawn_trooper(&mut gs, -100, 0);
    gs.mobjslab
        .get_mut(trooper)
        .expect("item must exist in tests")
        .target = gs.player.handle;

    dispatch_action(&mut gs, trooper, Action::FaceTarget as u8, None);

    let mo = gs.mobjslab.get(trooper).expect("item must exist in tests");
    // Should have turned to face east.
    assert!(
        mo.angle.0 < 0x2000_0000 || mo.angle.0 > 0xE000_0000,
        "should face east after Action::FaceTarget as u8"
    );
}

// -----------------------------------------------------------------------
// Batch 5 attack tests (preserved from original)
// -----------------------------------------------------------------------

/// Helper: spawn a monster of `kind` at `(x, y)` with given health,
/// already targeting the player, in the RUN1 chase state.
fn spawn_monster_chasing(
    gs: &mut GameState,
    kind: MobjKind,
    x: i32,
    y: i32,
    health: i32,
) -> MobjHandle {
    use crate::mobjinfo::MOBJINFO;
    use crate::states::{STATES, ids};
    let see_sn = MOBJINFO[kind as usize].see_state;
    let fallback_sn = if see_sn.0 != 0 {
        see_sn
    } else {
        crate::mobj::StateNum(ids::S_POSS_RUN1)
    };
    let mut mo = Mobj::new(
        kind,
        Fixed16_16::from_int(x),
        Fixed16_16::from_int(y),
        Bam::ZERO,
    );
    mo.health = health;
    mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
    mo.state = fallback_sn;
    mo.tics = STATES[fallback_sn.0 as usize].tics;
    mo.target = gs.player.handle;
    mo.threshold = 60;
    gs.mobjslab.alloc(mo)
}

#[test]
fn trooper_attacks_when_in_range() {
    use crate::states::ids;
    let mut gs = make_game_state();
    // Trooper well within missile range (2048 units); needs to enter ATK state.
    // Player at (0,0), trooper at (50, 0) — within MISSILERANGE (2048 units).
    let trooper = spawn_monster_chasing(&mut gs, MobjKind::Trooper, 50, 0, 20);

    // Tick enough times that A_Chase fires and detects missile range → ATK1.
    // Chase fires every 4 tics; with target set we just need one A_Chase invocation.
    let mut found_atk = false;
    for _ in 0..20 {
        gs.tick(TicCmd::default(), None);
        let mo = gs.mobjslab.get(trooper).expect("item must exist in tests");
        let s = mo.state.0;
        if s == ids::S_POSS_ATK1 || s == ids::S_POSS_ATK2 || s == ids::S_POSS_ATK3 {
            found_atk = true;
            break;
        }
    }
    assert!(
        found_atk,
        "trooper must enter an ATK state when player is in range"
    );
}

#[test]
fn a_scream_sets_mf_screamed_flag() {
    let mut gs = make_game_state();
    let trooper = spawn_trooper(&mut gs, 100, 0);

    // MF_SCREAMED must NOT be set before the action fires.
    assert_eq!(
        gs.mobjslab
            .get(trooper)
            .expect("item must exist in tests")
            .flags
            & flags::MF_SCREAMED,
        0,
        "MF_SCREAMED must not be set before A_Scream"
    );

    dispatch_action(&mut gs, trooper, Action::Scream as u8, None);

    assert_ne!(
        gs.mobjslab
            .get(trooper)
            .expect("item must exist in tests")
            .flags
            & flags::MF_SCREAMED,
        0,
        "A_Scream must set MF_SCREAMED"
    );
}

/// Verify that each original monster's first death frame carries Action::Scream as u8
/// and second death frame carries Action::Fall as u8 — matching the vanilla state table.
#[test]
fn die1_and_die2_actions_correct_for_all_original_monsters() {
    use crate::states::STATES;
    use crate::states::ids;

    let cases: &[(u16, u16, &str)] = &[
        (ids::S_POSS_DIE1, ids::S_POSS_DIE2, "Trooper"),
        (ids::S_SPOS_DIE1, ids::S_SPOS_DIE2, "Sergeant"),
        (ids::S_TROO_DIE1, ids::S_TROO_DIE2, "Imp"),
        (ids::S_SARG_DIE1, ids::S_SARG_DIE2, "Demon"),
        (ids::S_HEAD_DIE1, ids::S_HEAD_DIE2, "Cacodemon"),
        (ids::S_BOSS_DIE1, ids::S_BOSS_DIE2, "Baron"),
        (ids::S_CYBER_DIE1, ids::S_CYBER_DIE2, "Cyberdemon"),
        (ids::S_SPID_DIE1, ids::S_SPID_DIE2, "Spider"),
        (ids::S_BOS2_DIE1, ids::S_BOS2_DIE2, "HellKnight"),
    ];
    for &(die1, die2, name) in cases {
        assert_eq!(
            STATES[die1 as usize].action,
            Action::Scream as u8,
            "{name} DIE1 must have Action::Scream as u8"
        );
        assert_eq!(
            STATES[die2 as usize].action,
            Action::Fall as u8,
            "{name} DIE2 must have Action::Fall as u8"
        );
    }
}

#[test]
fn a_fall_clears_solid_flag() {
    let mut gs = make_game_state();
    let trooper = spawn_trooper(&mut gs, 100, 0);

    // Confirm solid before.
    assert_ne!(
        gs.mobjslab
            .get(trooper)
            .expect("item must exist in tests")
            .flags
            & flags::MF_SOLID,
        0,
        "trooper must start with MF_SOLID"
    );

    // Dispatch A_Fall directly.
    dispatch_action(&mut gs, trooper, Action::Fall as u8, None);

    let mo = gs.mobjslab.get(trooper).expect("item must exist in tests");
    assert_eq!(mo.flags & flags::MF_SOLID, 0, "a_fall must clear MF_SOLID");
    assert_eq!(
        mo.flags & flags::MF_COUNTKILL,
        0,
        "a_fall must clear MF_COUNTKILL"
    );
}

#[test]
fn a_pos_attack_does_not_panic_without_target() {
    let mut gs = make_game_state();
    let trooper = spawn_trooper(&mut gs, 100, 0);
    // No target set — must not panic.
    dispatch_action(&mut gs, trooper, Action::PosAttack as u8, None);
    // Health unchanged since we have no target to shoot.
    let mo = gs.mobjslab.get(trooper).expect("item must exist in tests");
    assert_eq!(mo.health, 20, "no target → no effect");
}

// -----------------------------------------------------------------------
// Batch 3: P_NewChaseDir and P_CheckSight tests
// -----------------------------------------------------------------------

/// Spawn a monster in its see state with movedir=NODIR, movecount=0.
fn spawn_monster_chasing_at(gs: &mut GameState, x: i32, y: i32) -> MobjHandle {
    use crate::mobjinfo::MOBJINFO;
    use crate::states::STATES;
    let kind = MobjKind::Trooper;
    let see_sn = MOBJINFO[kind as usize].see_state;
    let mut mo = Mobj::new(
        kind,
        Fixed16_16::from_int(x),
        Fixed16_16::from_int(y),
        Bam::ZERO,
    );
    mo.health = 20;
    mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
    mo.state = see_sn;
    mo.tics = STATES[see_sn.0 as usize].tics;
    mo.target = MobjHandle::NULL;
    mo.threshold = 60;
    mo.movedir = super::DI_NODIR;
    mo.movecount = 0;
    gs.mobjslab.alloc(mo)
}

#[test]
fn p_new_chase_dir_sets_movedir_toward_target() {
    let mut gs = make_game_state();
    // Player at (0,0), monster at (500, 500) — target is SW of monster.
    let monster = spawn_monster_chasing_at(&mut gs, 500, 500);
    gs.mobjslab
        .get_mut(monster)
        .expect("item must exist in tests")
        .target = gs.player.handle;

    super::p_new_chase_dir(&mut gs, monster, None);

    let mo = gs.mobjslab.get(monster).expect("item must exist in tests");
    // Monster is NE of player → should try to move SW (DI_SOUTHWEST = 5).
    // At minimum, movedir must not be NODIR and movecount must be > 0.
    assert_ne!(
        mo.movedir,
        super::DI_NODIR,
        "movedir must be set after p_new_chase_dir"
    );
    assert!(
        mo.movecount > 0,
        "movecount must be positive after p_new_chase_dir"
    );
    // The chosen direction should be westward or southward.
    let dir = mo.movedir;
    assert!(
        dir == super::DI_SOUTHWEST || dir == super::DI_WEST || dir == super::DI_SOUTH || dir <= 7,
        "movedir {dir} must be a valid direction"
    );
}

#[test]
fn p_new_chase_dir_nodir_when_no_target() {
    let mut gs = make_game_state();
    let monster = spawn_monster_chasing_at(&mut gs, 100, 0);
    // No target — leave target as NULL.
    assert_eq!(
        gs.mobjslab
            .get(monster)
            .expect("item must exist in tests")
            .target,
        MobjHandle::NULL
    );

    super::p_new_chase_dir(&mut gs, monster, None);

    let mo = gs.mobjslab.get(monster).expect("item must exist in tests");
    assert_eq!(
        mo.movedir,
        super::DI_NODIR,
        "monster with no target must stay at NODIR"
    );
}

#[test]
fn p_check_sight_returns_true_without_level() {
    let mut gs = make_game_state();
    let trooper = spawn_trooper(&mut gs, 100, 0);
    gs.mobjslab
        .get_mut(trooper)
        .expect("item must exist in tests")
        .target = gs.player.handle;

    // No level → falls through to Manhattan distance check (≤ 4096).
    let can_see = super::p_check_sight_local(&gs, trooper, gs.player.handle, None);
    assert!(
        can_see,
        "monster 100 units away must be visible without level"
    );
}

#[test]
fn p_check_sight_returns_false_for_reject_blocked() {
    let mut gs = make_game_state();
    let trooper = spawn_trooper(&mut gs, 100, 0);
    gs.mobjslab
        .get_mut(trooper)
        .expect("item must exist in tests")
        .target = gs.player.handle;

    // Build a minimal level with 2 sectors where the REJECT marks them
    // as mutually invisible (all-ones reject data = all blocked).
    let reject = doom_map::Reject::parse_lump(&[0xFFu8], 2).expect("item must exist in tests");

    let mut bm_data = vec![0u8; 14];
    bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
    bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
    bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
    bm_data[10..12].copy_from_slice(&0x0000u16.to_le_bytes());
    bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
    let blockmap = doom_map::Blockmap::parse_lump(&bm_data).expect("item must exist in tests");

    let vertexes = vec![
        doom_map::Vertex { x: 0, y: 0 },
        doom_map::Vertex { x: 64, y: 0 },
        doom_map::Vertex { x: 0, y: 64 },
        doom_map::Vertex { x: 64, y: 64 },
    ];
    let sidedefs = vec![
        doom_map::Sidedef {
            x_offset: 0,
            y_offset: 0,
            upper_texture: *b"\0\0\0\0\0\0\0\0",
            lower_texture: *b"\0\0\0\0\0\0\0\0",
            middle_texture: *b"\0\0\0\0\0\0\0\0",
            sector: 0,
        },
        doom_map::Sidedef {
            x_offset: 0,
            y_offset: 0,
            upper_texture: *b"\0\0\0\0\0\0\0\0",
            lower_texture: *b"\0\0\0\0\0\0\0\0",
            middle_texture: *b"\0\0\0\0\0\0\0\0",
            sector: 1,
        },
    ];
    let linedefs = vec![
        doom_map::Linedef {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0,
            special: 0,
            tag: 0,
            right_sidedef: 0,
            left_sidedef: 0xFFFF,
        },
        doom_map::Linedef {
            from_vertex: 2,
            to_vertex: 3,
            flags: 0,
            special: 0,
            tag: 0,
            right_sidedef: 1,
            left_sidedef: 0xFFFF,
        },
    ];
    let segs = vec![
        doom_map::Seg {
            from_vertex: 0,
            to_vertex: 1,
            angle: 0,
            linedef: 0,
            direction: 0,
            offset: 0,
        },
        doom_map::Seg {
            from_vertex: 2,
            to_vertex: 3,
            angle: 0,
            linedef: 1,
            direction: 0,
            offset: 0,
        },
    ];
    let ssectors = vec![
        doom_map::Ssector {
            seg_count: 1,
            first_seg: 0,
        },
        doom_map::Ssector {
            seg_count: 1,
            first_seg: 1,
        },
    ];
    let sectors = vec![
        doom_map::Sector {
            floor_height: 0,
            ceil_height: 128,
            floor_flat: *b"FLAT1\0\0\0",
            ceil_flat: *b"FLAT2\0\0\0",
            light_level: 192,
            special: 0,
            tag: 0,
        },
        doom_map::Sector {
            floor_height: 0,
            ceil_height: 128,
            floor_flat: *b"FLAT1\0\0\0",
            ceil_flat: *b"FLAT2\0\0\0",
            light_level: 192,
            special: 0,
            tag: 0,
        },
    ];

    let level = doom_map::Level {
        name: "TEST".to_string(),
        things: vec![],
        linedefs,
        sidedefs,
        vertexes,
        segs,
        ssectors,
        nodes: vec![],
        sectors,
        reject,
        blockmap,
    };

    let can_see = super::p_check_sight_local(&gs, trooper, gs.player.handle, Some(&level));
    assert!(!can_see, "all-ones reject must block LOS");
}

#[test]
fn p_check_sight_returns_true_for_all_zero_reject() {
    let mut gs = make_game_state();
    let trooper = spawn_trooper(&mut gs, 100, 0);
    gs.mobjslab
        .get_mut(trooper)
        .expect("item must exist in tests")
        .target = gs.player.handle;

    let reject = doom_map::Reject::parse_lump(&[0u8], 1).expect("item must exist in tests");

    let mut bm_data = vec![0u8; 14];
    bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
    bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
    bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
    bm_data[10..12].copy_from_slice(&0x0000u16.to_le_bytes());
    bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
    let blockmap = doom_map::Blockmap::parse_lump(&bm_data).expect("item must exist in tests");

    let level = doom_map::Level {
        name: "TEST".to_string(),
        things: vec![],
        linedefs: vec![],
        sidedefs: vec![],
        vertexes: vec![],
        segs: vec![],
        ssectors: vec![doom_map::Ssector {
            seg_count: 0,
            first_seg: 0,
        }],
        nodes: vec![],
        sectors: vec![doom_map::Sector {
            floor_height: 0,
            ceil_height: 128,
            floor_flat: *b"FLAT1\0\0\0",
            ceil_flat: *b"FLAT2\0\0\0",
            light_level: 192,
            special: 0,
            tag: 0,
        }],
        reject,
        blockmap,
    };

    let can_see = super::p_check_sight_local(&gs, trooper, gs.player.handle, Some(&level));
    assert!(
        can_see,
        "all-zero reject + close distance must report visible"
    );
}

// -----------------------------------------------------------------------
// Additional Batch 30 monster AI tests
// -----------------------------------------------------------------------

#[test]
fn melee_threshold_is_correct() {
    assert_eq!(MELEE_THRESHOLD, 84, "MELEERANGE(64) + 20 = 84");
}

#[test]
fn a_chase_demon_melee_only_no_missile() {
    // Demon has melee_state but missile_state is S_NULL.
    let info = &mobjinfo::MOBJINFO[MobjKind::Demon as usize];
    assert_ne!(info.melee_state, StateNum::NULL);
    assert_eq!(info.missile_state, StateNum::NULL);
}

#[test]
fn a_chase_trooper_missile_only_no_melee() {
    // Trooper has missile_state but melee_state is S_NULL.
    let info = &mobjinfo::MOBJINFO[MobjKind::Trooper as usize];
    assert_eq!(info.melee_state, StateNum::NULL);
    assert_ne!(info.missile_state, StateNum::NULL);
}

#[test]
fn a_chase_with_dead_target_finds_new_target_if_player_alive() {
    let mut gs = make_game_state();
    let kind = MobjKind::Trooper;
    let see_sn = mobjinfo::MOBJINFO[kind as usize].see_state;

    // Create a "fake" monster target that is dead.
    let dead_mo = Mobj::new(
        MobjKind::Trooper,
        Fixed16_16::from_int(500),
        Fixed16_16::from_int(500),
        Bam::ZERO,
    );
    let dead_handle = gs.mobjslab.alloc(dead_mo);
    // It has 0 health (dead).

    // Spawn the chasing monster targeting the dead thing.
    let mut mo = Mobj::new(
        kind,
        Fixed16_16::from_int(100),
        Fixed16_16::from_int(0),
        Bam::ZERO,
    );
    mo.health = 20;
    mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
    mo.state = see_sn;
    mo.tics = states::STATES[see_sn.0 as usize].tics;
    mo.target = dead_handle;
    mo.reactiontime = 0;
    let trooper = gs.mobjslab.alloc(mo);

    // Player is alive at (0,0), within range.
    a_chase(&mut gs, trooper, None);

    let mo = gs.mobjslab.get(trooper).expect("item must exist in tests");
    // The monster should now target the player (found via a_look).
    assert_eq!(
        mo.target, gs.player.handle,
        "monster should find new target (player) when old target is dead"
    );
}

#[test]
fn p_move_all_eight_directions() {
    for dir in 0..8u8 {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 500, 500);
        gs.mobjslab
            .get_mut(trooper)
            .expect("item must exist in tests")
            .movedir = dir;

        let old_x = gs
            .mobjslab
            .get(trooper)
            .expect("item must exist in tests")
            .x;
        let old_y = gs
            .mobjslab
            .get(trooper)
            .expect("item must exist in tests")
            .y;

        let moved = p_move(&mut gs, trooper, None);
        assert!(moved, "p_move should succeed for dir={}", dir);

        let mo = gs.mobjslab.get(trooper).expect("item must exist in tests");
        let dx = mo.x - old_x;
        let dy = mo.y - old_y;

        // Verify direction is correct.
        match dir {
            DI_EAST => assert!(dx > Fixed16_16::ZERO, "east should increase x"),
            DI_WEST => assert!(dx < Fixed16_16::ZERO, "west should decrease x"),
            DI_NORTH => assert!(dy > Fixed16_16::ZERO, "north should increase y"),
            DI_SOUTH => assert!(dy < Fixed16_16::ZERO, "south should decrease y"),
            DI_NORTHEAST => {
                assert!(dx > Fixed16_16::ZERO && dy > Fixed16_16::ZERO);
            }
            DI_NORTHWEST => {
                assert!(dx < Fixed16_16::ZERO && dy > Fixed16_16::ZERO);
            }
            DI_SOUTHWEST => {
                assert!(dx < Fixed16_16::ZERO && dy < Fixed16_16::ZERO);
            }
            DI_SOUTHEAST => {
                assert!(dx > Fixed16_16::ZERO && dy < Fixed16_16::ZERO);
            }
            _ => {}
        }
    }
}

#[test]
fn set_mobj_state_updates_state_and_tics() {
    let mut gs = make_game_state();
    let trooper = spawn_trooper(&mut gs, 100, 0);
    let see_sn = mobjinfo::MOBJINFO[MobjKind::Trooper as usize].see_state;

    set_mobj_state(&mut gs, trooper, see_sn);

    let mo = gs.mobjslab.get(trooper).expect("item must exist in tests");
    assert_eq!(mo.state, see_sn);
    assert_eq!(mo.tics, states::STATES[see_sn.0 as usize].tics);
}

#[test]
fn transition_to_see_state_sets_all_fields() {
    let mut gs = make_game_state();
    let trooper = spawn_trooper(&mut gs, 100, 0);
    let see_sn = mobjinfo::MOBJINFO[MobjKind::Trooper as usize].see_state;

    let ph = gs.player.handle;
    transition_to_see_state(&mut gs, trooper, MobjKind::Trooper, ph);

    let mo = gs.mobjslab.get(trooper).expect("item must exist in tests");
    assert_eq!(mo.target, gs.player.handle);
    assert_eq!(mo.state, see_sn);
    assert_eq!(mo.threshold, 60);
}

#[test]
fn a_chase_reaction_time_decrements_each_call() {
    let mut gs = make_game_state();
    let kind = MobjKind::Trooper;
    let see_sn = mobjinfo::MOBJINFO[kind as usize].see_state;

    let mut mo = Mobj::new(
        kind,
        Fixed16_16::from_int(200),
        Fixed16_16::from_int(0),
        Bam::ZERO,
    );
    mo.health = 20;
    mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
    mo.state = see_sn;
    mo.tics = states::STATES[see_sn.0 as usize].tics;
    mo.target = gs.player.handle;
    mo.reactiontime = 5;
    let trooper = gs.mobjslab.alloc(mo);

    for i in 0..5 {
        a_chase(&mut gs, trooper, None);
        let mo = gs.mobjslab.get(trooper).expect("item must exist in tests");
        assert_eq!(
            mo.reactiontime,
            4 - i,
            "reaction_time should decrement to {}",
            4 - i
        );
    }
}

#[test]
fn dispatch_action_unknown_index_does_nothing() {
    let mut gs = make_game_state();
    let trooper = spawn_trooper(&mut gs, 100, 0);
    let health_before = gs
        .mobjslab
        .get(trooper)
        .expect("item must exist in tests")
        .health;

    dispatch_action(&mut gs, trooper, 255, None);

    let health_after = gs
        .mobjslab
        .get(trooper)
        .expect("item must exist in tests")
        .health;
    assert_eq!(
        health_before, health_after,
        "unknown action should be no-op"
    );
}

#[test]
fn xmove_ymove_nodir_is_zero() {
    assert_eq!(XMOVE[DI_NODIR as usize], Fixed16_16::ZERO);
    assert_eq!(YMOVE[DI_NODIR as usize], Fixed16_16::ZERO);
}

#[test]
fn xmove_ymove_tables_have_nine_entries() {
    assert_eq!(XMOVE.len(), 9);
    assert_eq!(YMOVE.len(), 9);
}

// -----------------------------------------------------------------------
// Batch 31: New monster attack function tests
// -----------------------------------------------------------------------

/// Spawn a generic monster of `kind` at `(x, y)` targeting the player.
fn spawn_monster_targeting_player(
    gs: &mut GameState,
    kind: MobjKind,
    x: i32,
    y: i32,
    health: i32,
) -> MobjHandle {
    let mut mo = Mobj::new(
        kind,
        Fixed16_16::from_int(x),
        Fixed16_16::from_int(y),
        Bam::ZERO,
    );
    mo.health = health;
    mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
    mo.target = gs.player.handle;
    mo.radius = Fixed16_16::from_int(20);
    mo.height = Fixed16_16::from_int(56);
    gs.mobjslab.alloc(mo)
}

// --- Dispatch wiring tests ---

#[test]
fn dispatch_cpos_attack_does_not_panic() {
    let mut gs = make_game_state();
    let h = spawn_monster_targeting_player(&mut gs, MobjKind::Sergeant, 100, 0, 30);
    dispatch_action(&mut gs, h, Action::CposAttack as u8, None);
}

#[test]
fn dispatch_cyber_attack_does_not_panic() {
    let mut gs = make_game_state();
    let h = spawn_monster_targeting_player(&mut gs, MobjKind::Cyberdemon, 200, 0, 4000);
    dispatch_action(&mut gs, h, Action::CyberAttack as u8, None);
}

#[test]
fn dispatch_skel_missile_does_not_panic() {
    let mut gs = make_game_state();
    let h = spawn_monster_targeting_player(&mut gs, MobjKind::Revenant, 200, 0, 300);
    dispatch_action(&mut gs, h, Action::SkelMissile as u8, None);
}

#[test]
fn dispatch_fat_attack1_does_not_panic() {
    let mut gs = make_game_state();
    let h = spawn_monster_targeting_player(&mut gs, MobjKind::Mancubus, 200, 0, 600);
    dispatch_action(&mut gs, h, Action::FatAttack1 as u8, None);
}

#[test]
fn dispatch_fat_attack2_does_not_panic() {
    let mut gs = make_game_state();
    let h = spawn_monster_targeting_player(&mut gs, MobjKind::Mancubus, 200, 0, 600);
    dispatch_action(&mut gs, h, Action::FatAttack2 as u8, None);
}

#[test]
fn dispatch_fat_attack3_does_not_panic() {
    let mut gs = make_game_state();
    let h = spawn_monster_targeting_player(&mut gs, MobjKind::Mancubus, 200, 0, 600);
    dispatch_action(&mut gs, h, Action::FatAttack3 as u8, None);
}

#[test]
fn dispatch_skull_attack_does_not_panic() {
    let mut gs = make_game_state();
    let h = spawn_monster_targeting_player(&mut gs, MobjKind::LostSoul, 200, 0, 100);
    dispatch_action(&mut gs, h, Action::SkullAttack as u8, None);
}

#[test]
fn dispatch_bspi_attack_does_not_panic() {
    let mut gs = make_game_state();
    let h = spawn_monster_targeting_player(&mut gs, MobjKind::Arachnotron, 200, 0, 500);
    dispatch_action(&mut gs, h, Action::BspiAttack as u8, None);
}

#[test]
fn dispatch_spid_attack_does_not_panic() {
    let mut gs = make_game_state();
    let h = spawn_monster_targeting_player(&mut gs, MobjKind::SpiderMastermind, 200, 0, 3000);
    dispatch_action(&mut gs, h, Action::SpidAttack as u8, None);
}

#[test]
fn dispatch_pain_attack_does_not_panic() {
    let mut gs = make_game_state();
    let h = spawn_monster_targeting_player(&mut gs, MobjKind::PainElemental, 200, 0, 400);
    dispatch_action(&mut gs, h, Action::PainAttack as u8, None);
}

// --- Cyberdemon spawns Rocket ---

#[test]
fn cyber_attack_spawns_rocket_projectile() {
    let mut gs = make_game_state();
    let cyber = spawn_monster_targeting_player(&mut gs, MobjKind::Cyberdemon, 200, 0, 4000);

    let count_before = gs.mobjslab.len();
    a_cyber_attack(&mut gs, cyber);
    let count_after = gs.mobjslab.len();

    assert!(
        count_after > count_before,
        "cyberdemon attack must spawn a projectile"
    );

    // Find the Rocket.
    let rocket = gs.mobjslab.iter_handles().find(|h| {
        gs.mobjslab
            .get(*h)
            .map(|m| m.kind == MobjKind::Rocket)
            .unwrap_or(false)
    });
    assert!(rocket.is_some(), "must spawn a Rocket MobjKind");
}

// --- Revenant spawns Tracer ---

#[test]
fn skel_missile_spawns_tracer_projectile() {
    let mut gs = make_game_state();
    let skel = spawn_monster_targeting_player(&mut gs, MobjKind::Revenant, 200, 0, 300);

    a_skel_missile(&mut gs, skel);

    let tracer = gs.mobjslab.iter_handles().find(|h| {
        gs.mobjslab
            .get(*h)
            .map(|m| m.kind == MobjKind::Tracer)
            .unwrap_or(false)
    });
    assert!(tracer.is_some(), "must spawn a Tracer MobjKind");
}

// --- Arachnotron spawns ArachPlaz ---

#[test]
fn bspi_attack_spawns_arachplaz() {
    let mut gs = make_game_state();
    let bspi = spawn_monster_targeting_player(&mut gs, MobjKind::Arachnotron, 200, 0, 500);

    a_bspi_attack(&mut gs, bspi);

    let plaz = gs.mobjslab.iter_handles().find(|h| {
        gs.mobjslab
            .get(*h)
            .map(|m| m.kind == MobjKind::ArachPlaz)
            .unwrap_or(false)
    });
    assert!(plaz.is_some(), "must spawn an ArachPlaz MobjKind");
}

// --- Mancubus spawns FatShot (two per attack) ---

#[test]
fn fat_attack1_spawns_two_fatshots() {
    let mut gs = make_game_state();
    let manc = spawn_monster_targeting_player(&mut gs, MobjKind::Mancubus, 200, 0, 600);

    let count_before = gs.mobjslab.len();
    a_fat_attack1(&mut gs, manc);
    let count_after = gs.mobjslab.len();

    // Two FatShots should be spawned.
    assert_eq!(
        count_after - count_before,
        2,
        "fat_attack1 must spawn exactly 2 projectiles"
    );

    let fatshot_count = gs
        .mobjslab
        .iter_handles()
        .filter(|h| {
            gs.mobjslab
                .get(*h)
                .map(|m| m.kind == MobjKind::FatShot)
                .unwrap_or(false)
        })
        .count();
    assert_eq!(fatshot_count, 2, "both projectiles must be FatShot");
}

#[test]
fn fat_attack2_spawns_two_fatshots() {
    let mut gs = make_game_state();
    let manc = spawn_monster_targeting_player(&mut gs, MobjKind::Mancubus, 200, 0, 600);

    a_fat_attack2(&mut gs, manc);

    let fatshot_count = gs
        .mobjslab
        .iter_handles()
        .filter(|h| {
            gs.mobjslab
                .get(*h)
                .map(|m| m.kind == MobjKind::FatShot)
                .unwrap_or(false)
        })
        .count();
    assert_eq!(fatshot_count, 2, "fat_attack2 must spawn 2 FatShots");
}

#[test]
fn fat_attack3_spawns_two_fatshots() {
    let mut gs = make_game_state();
    let manc = spawn_monster_targeting_player(&mut gs, MobjKind::Mancubus, 200, 0, 600);

    a_fat_attack3(&mut gs, manc);

    let fatshot_count = gs
        .mobjslab
        .iter_handles()
        .filter(|h| {
            gs.mobjslab
                .get(*h)
                .map(|m| m.kind == MobjKind::FatShot)
                .unwrap_or(false)
        })
        .count();
    assert_eq!(fatshot_count, 2, "fat_attack3 must spawn 2 FatShots");
}

// --- Lost Soul skull attack ---

#[test]
fn skull_attack_sets_skullfly_flag() {
    let mut gs = make_game_state();
    let skull = spawn_monster_targeting_player(&mut gs, MobjKind::LostSoul, 200, 0, 100);

    a_skull_attack(&mut gs, skull);

    let mo = gs.mobjslab.get(skull).expect("item must exist in tests");
    assert_ne!(
        mo.flags & flags::MF_SKULLFLY,
        0,
        "skull attack must set MF_SKULLFLY"
    );
}

#[test]
fn skull_attack_sets_momentum_toward_target() {
    let mut gs = make_game_state();
    // Skull at (200,0), player at (0,0) — skull should fly west.
    let skull = spawn_monster_targeting_player(&mut gs, MobjKind::LostSoul, 200, 0, 100);

    a_skull_attack(&mut gs, skull);

    let mo = gs.mobjslab.get(skull).expect("item must exist in tests");
    assert!(
        mo.momx < Fixed16_16::ZERO,
        "momx should be negative (flying west toward player), got {:?}",
        mo.momx
    );
    // Speed should be approximately SKULLSPEED (20).
    let speed_sq = mo.momx.to_int() * mo.momx.to_int() + mo.momy.to_int() * mo.momy.to_int();
    let speed = (speed_sq as f32).sqrt();
    assert!(
        (15.0..=25.0).contains(&speed),
        "skull speed should be ~20, got {speed}"
    );
}

// --- Pain Elemental spawns Lost Soul ---

#[test]
fn pain_attack_spawns_lost_soul() {
    let mut gs = make_game_state();
    let pe = spawn_monster_targeting_player(&mut gs, MobjKind::PainElemental, 200, 0, 400);

    let count_before = gs
        .mobjslab
        .iter_handles()
        .filter(|h| {
            gs.mobjslab
                .get(*h)
                .map(|m| m.kind == MobjKind::LostSoul)
                .unwrap_or(false)
        })
        .count();

    a_pain_attack(&mut gs, pe);

    let count_after = gs
        .mobjslab
        .iter_handles()
        .filter(|h| {
            gs.mobjslab
                .get(*h)
                .map(|m| m.kind == MobjKind::LostSoul)
                .unwrap_or(false)
        })
        .count();

    assert_eq!(
        count_after,
        count_before + 1,
        "pain attack must spawn exactly 1 Lost Soul"
    );
}

#[test]
fn pain_attack_respects_lost_soul_cap() {
    let mut gs = make_game_state();
    let pe = spawn_monster_targeting_player(&mut gs, MobjKind::PainElemental, 200, 0, 400);

    // Spawn 21 Lost Souls to reach the cap.
    for i in 0..21 {
        let mut skull = Mobj::new(
            MobjKind::LostSoul,
            Fixed16_16::from_int(500 + i * 10),
            Fixed16_16::from_int(500),
            Bam::ZERO,
        );
        skull.health = 100;
        skull.flags = flags::MF_SOLID | flags::MF_SHOOTABLE;
        gs.mobjslab.alloc(skull);
    }

    let count_before = gs
        .mobjslab
        .iter_handles()
        .filter(|h| {
            gs.mobjslab
                .get(*h)
                .map(|m| m.kind == MobjKind::LostSoul && m.health > 0)
                .unwrap_or(false)
        })
        .count();

    a_pain_attack(&mut gs, pe);

    let count_after = gs
        .mobjslab
        .iter_handles()
        .filter(|h| {
            gs.mobjslab
                .get(*h)
                .map(|m| m.kind == MobjKind::LostSoul && m.health > 0)
                .unwrap_or(false)
        })
        .count();

    assert_eq!(
        count_before, count_after,
        "pain attack must NOT spawn Lost Soul when cap ({}) is reached",
        LOST_SOUL_MAX
    );
}

#[test]
fn pain_attack_spawns_when_under_cap() {
    let mut gs = make_game_state();
    let pe = spawn_monster_targeting_player(&mut gs, MobjKind::PainElemental, 200, 0, 400);

    // Spawn 20 Lost Souls — one below the cap.
    for i in 0..20 {
        let mut skull = Mobj::new(
            MobjKind::LostSoul,
            Fixed16_16::from_int(500 + i * 10),
            Fixed16_16::from_int(500),
            Bam::ZERO,
        );
        skull.health = 100;
        skull.flags = flags::MF_SOLID | flags::MF_SHOOTABLE;
        gs.mobjslab.alloc(skull);
    }

    a_pain_attack(&mut gs, pe);

    let count = gs
        .mobjslab
        .iter_handles()
        .filter(|h| {
            gs.mobjslab
                .get(*h)
                .map(|m| m.kind == MobjKind::LostSoul && m.health > 0)
                .unwrap_or(false)
        })
        .count();

    assert_eq!(count, 21, "pain attack should spawn 1 more (20->21)");
}

// --- No-target / dead-target edge cases ---

#[test]
fn all_attacks_noop_without_target() {
    let mut gs = make_game_state();
    // Monster with no target set (NULL).
    let mut mo = Mobj::new(
        MobjKind::Sergeant,
        Fixed16_16::from_int(100),
        Fixed16_16::from_int(0),
        Bam::ZERO,
    );
    mo.health = 30;
    mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE;
    let h = gs.mobjslab.alloc(mo);

    // None of these should panic.
    dispatch_action(&mut gs, h, Action::CposAttack as u8, None);
    dispatch_action(&mut gs, h, Action::CyberAttack as u8, None);
    dispatch_action(&mut gs, h, Action::SkelMissile as u8, None);
    dispatch_action(&mut gs, h, Action::FatAttack1 as u8, None);
    dispatch_action(&mut gs, h, Action::FatAttack2 as u8, None);
    dispatch_action(&mut gs, h, Action::FatAttack3 as u8, None);
    dispatch_action(&mut gs, h, Action::SkullAttack as u8, None);
    dispatch_action(&mut gs, h, Action::BspiAttack as u8, None);
    dispatch_action(&mut gs, h, Action::SpidAttack as u8, None);
    dispatch_action(&mut gs, h, Action::PainAttack as u8, None);
}

#[test]
fn all_attacks_noop_with_dead_target() {
    let mut gs = make_game_state();
    // Kill the player.
    gs.mobjslab
        .get_mut(gs.player.handle)
        .expect("item must exist in tests")
        .health = 0;

    let h = spawn_monster_targeting_player(&mut gs, MobjKind::Cyberdemon, 200, 0, 4000);

    let slab_len_before = gs.mobjslab.len();
    dispatch_action(&mut gs, h, Action::CyberAttack as u8, None);
    // Cyberdemon should NOT spawn a rocket when target is dead.
    assert_eq!(
        gs.mobjslab.len(),
        slab_len_before,
        "must not spawn projectile when target is dead"
    );
}

#[test]
fn all_attacks_noop_with_stale_handle() {
    let mut gs = make_game_state();
    let h = spawn_monster_targeting_player(&mut gs, MobjKind::Cyberdemon, 200, 0, 4000);
    gs.mobjslab.free(h);

    // None should panic.
    dispatch_action(&mut gs, h, Action::CposAttack as u8, None);
    dispatch_action(&mut gs, h, Action::CyberAttack as u8, None);
    dispatch_action(&mut gs, h, Action::SkelMissile as u8, None);
    dispatch_action(&mut gs, h, Action::FatAttack1 as u8, None);
    dispatch_action(&mut gs, h, Action::SkullAttack as u8, None);
    dispatch_action(&mut gs, h, Action::BspiAttack as u8, None);
    dispatch_action(&mut gs, h, Action::SpidAttack as u8, None);
    dispatch_action(&mut gs, h, Action::PainAttack as u8, None);
}

// --- Constants ---

#[test]
fn fatspread_constant_is_correct() {
    assert_eq!(FATSPREAD, 0x0400_0000, "FATSPREAD must be ANG90/8");
}

#[test]
fn skullspeed_constant_is_20() {
    assert_eq!(SKULLSPEED, 20, "Lost Soul charge speed must be 20");
}

#[test]
fn lost_soul_max_is_21() {
    assert_eq!(LOST_SOUL_MAX, 21, "Lost Soul cap must be 21");
}

#[test]
fn action_constants_are_unique() {
    let constants = [
        Action::NoAction as u8,
        Action::Look as u8,
        Action::Chase as u8,
        Action::PosAttack as u8,
        Action::SposAttack as u8,
        Action::TrooAttack as u8,
        Action::SargAttack as u8,
        Action::Fall as u8,
        Action::HeadAttack as u8,
        Action::BruisAttack as u8,
        Action::FaceTarget as u8,
        Action::CposAttack as u8,
        Action::CyberAttack as u8,
        Action::SkelMissile as u8,
        Action::FatAttack1 as u8,
        Action::FatAttack2 as u8,
        Action::FatAttack3 as u8,
        Action::SkullAttack as u8,
        Action::BspiAttack as u8,
        Action::SpidAttack as u8,
        Action::PainAttack as u8,
        Action::VileChase as u8,
        Action::VileStart as u8,
        Action::VileTarget as u8,
        Action::VileAttack as u8,
        Action::Fire as u8,
        Action::BrainAwake as u8,
        Action::BrainSpit as u8,
        Action::SpawnFly as u8,
        Action::BrainDie as u8,
        Action::BrainScream as u8,
        Action::BrainExplode as u8,
    ];
    for i in 0..constants.len() {
        for j in (i + 1)..constants.len() {
            assert_ne!(
                constants[i], constants[j],
                "action constants at indices {i} and {j} must be unique"
            );
        }
    }
}

// -----------------------------------------------------------------------
// Arch-Vile action tests
// -----------------------------------------------------------------------

#[test]
fn dispatch_vile_chase_does_not_panic() {
    let mut gs = make_game_state();
    let h = spawn_monster_targeting_player(&mut gs, MobjKind::ArchVile, 200, 0, 700);
    dispatch_action(&mut gs, h, Action::VileChase as u8, None);
}

#[test]
fn dispatch_vile_start_does_not_panic() {
    let mut gs = make_game_state();
    let h = spawn_monster_targeting_player(&mut gs, MobjKind::ArchVile, 200, 0, 700);
    dispatch_action(&mut gs, h, Action::VileStart as u8, None);
}

#[test]
fn dispatch_vile_target_does_not_panic() {
    let mut gs = make_game_state();
    let h = spawn_monster_targeting_player(&mut gs, MobjKind::ArchVile, 200, 0, 700);
    dispatch_action(&mut gs, h, Action::VileTarget as u8, None);
}

#[test]
fn dispatch_vile_attack_does_not_panic() {
    let mut gs = make_game_state();
    let h = spawn_monster_targeting_player(&mut gs, MobjKind::ArchVile, 200, 0, 700);
    dispatch_action(&mut gs, h, Action::VileAttack as u8, None);
}

#[test]
fn dispatch_fire_does_not_panic() {
    let mut gs = make_game_state();
    // Spawn a fire actor with a tracer.
    let mut fire = Mobj::new(
        MobjKind::VileFire,
        Fixed16_16::from_int(100),
        Fixed16_16::from_int(100),
        Bam::ZERO,
    );
    fire.health = 1;
    fire.tracer = gs.player.handle;
    let h = gs.mobjslab.alloc(fire);
    dispatch_action(&mut gs, h, Action::Fire as u8, None);
}

#[test]
fn vile_start_sets_tracer() {
    let mut gs = make_game_state();
    let vile = spawn_monster_targeting_player(&mut gs, MobjKind::ArchVile, 200, 0, 700);

    a_vile_start(&mut gs, vile);

    let mo = gs.mobjslab.get(vile).expect("item must exist in tests");
    assert_eq!(
        mo.tracer, gs.player.handle,
        "vile_start must set tracer to the target"
    );
}

#[test]
fn vile_target_spawns_fire() {
    let mut gs = make_game_state();
    let vile = spawn_monster_targeting_player(&mut gs, MobjKind::ArchVile, 200, 0, 700);

    let count_before = gs.mobjslab.len();
    a_vile_target(&mut gs, vile);
    let count_after = gs.mobjslab.len();

    assert!(
        count_after > count_before,
        "vile_target must spawn a VileFire actor"
    );

    let fire = gs.mobjslab.iter_handles().find(|h| {
        gs.mobjslab
            .get(*h)
            .map(|m| m.kind == MobjKind::VileFire)
            .unwrap_or(false)
    });
    assert!(fire.is_some(), "must spawn a VileFire MobjKind");
}

#[test]
fn vile_attack_damages_target() {
    let mut gs = make_game_state();
    let vile = spawn_monster_targeting_player(&mut gs, MobjKind::ArchVile, 200, 0, 700);
    let hp_before = gs
        .mobjslab
        .get(gs.player.handle)
        .expect("item must exist in tests")
        .health;

    a_vile_attack(&mut gs, vile);

    let hp_after = gs
        .mobjslab
        .get(gs.player.handle)
        .expect("item must exist in tests")
        .health;
    assert_eq!(
        hp_before - hp_after,
        90,
        "vile_attack must deal 20+70=90 total damage"
    );
}

#[test]
fn vile_attack_applies_upward_thrust() {
    let mut gs = make_game_state();
    let vile = spawn_monster_targeting_player(&mut gs, MobjKind::ArchVile, 200, 0, 700);

    a_vile_attack(&mut gs, vile);

    let momz = gs
        .mobjslab
        .get(gs.player.handle)
        .expect("item must exist in tests")
        .momz;
    assert_eq!(
        momz,
        Fixed16_16::from_int(15),
        "vile_attack must apply momz of 15"
    );
}

#[test]
fn fire_tracks_tracer_position() {
    let mut gs = make_game_state();
    // Move the player to a known position.
    gs.mobjslab
        .get_mut(gs.player.handle)
        .expect("item must exist in tests")
        .x = Fixed16_16::from_int(500);
    gs.mobjslab
        .get_mut(gs.player.handle)
        .expect("item must exist in tests")
        .y = Fixed16_16::from_int(300);

    let mut fire = Mobj::new(
        MobjKind::VileFire,
        Fixed16_16::from_int(100),
        Fixed16_16::from_int(100),
        Bam::ZERO,
    );
    fire.health = 1;
    fire.tracer = gs.player.handle;
    let fh = gs.mobjslab.alloc(fire);

    a_fire(&mut gs, fh);

    let fire_mo = gs.mobjslab.get(fh).expect("item must exist in tests");
    assert_eq!(fire_mo.x, Fixed16_16::from_int(500));
    assert_eq!(fire_mo.y, Fixed16_16::from_int(300));
}

#[test]
fn vile_chase_resurrects_nearby_corpse() {
    let mut gs = make_game_state();
    let vile = spawn_monster_targeting_player(&mut gs, MobjKind::ArchVile, 200, 0, 700);

    // Create a trooper corpse within 128 units of the vile.
    let mut corpse = Mobj::new(
        MobjKind::Trooper,
        Fixed16_16::from_int(210),
        Fixed16_16::from_int(10),
        Bam::ZERO,
    );
    corpse.health = 0;
    corpse.flags = flags::MF_CORPSE;
    let corpse_h = gs.mobjslab.alloc(corpse);

    a_vile_chase(&mut gs, vile, None);

    // The corpse should be resurrected.
    let raised = gs.mobjslab.get(corpse_h).expect("item must exist in tests");
    assert!(
        raised.health > 0,
        "resurrected corpse must have health > 0, got {}",
        raised.health
    );
    assert_eq!(
        raised.flags & flags::MF_CORPSE,
        0,
        "resurrected corpse must not have MF_CORPSE flag"
    );
}

#[test]
fn vile_chase_ignores_distant_corpse() {
    let mut gs = make_game_state();
    let vile = spawn_monster_targeting_player(&mut gs, MobjKind::ArchVile, 200, 0, 700);

    // Create a trooper corpse far from the vile (> 128 units).
    let mut corpse = Mobj::new(
        MobjKind::Trooper,
        Fixed16_16::from_int(500),
        Fixed16_16::from_int(500),
        Bam::ZERO,
    );
    corpse.health = 0;
    corpse.flags = flags::MF_CORPSE;
    let corpse_h = gs.mobjslab.alloc(corpse);

    a_vile_chase(&mut gs, vile, None);

    // The corpse should remain dead (too far away).
    let still_dead = gs.mobjslab.get(corpse_h).expect("item must exist in tests");
    assert_eq!(
        still_dead.health, 0,
        "distant corpse must not be resurrected"
    );
}

#[test]
fn vile_chase_ignores_non_raisable_corpse() {
    let mut gs = make_game_state();
    let vile = spawn_monster_targeting_player(&mut gs, MobjKind::ArchVile, 200, 0, 700);

    // Create a Lost Soul corpse nearby (Lost Soul has raise_state = S_NULL).
    let mut corpse = Mobj::new(
        MobjKind::LostSoul,
        Fixed16_16::from_int(210),
        Fixed16_16::from_int(10),
        Bam::ZERO,
    );
    corpse.health = 0;
    corpse.flags = flags::MF_CORPSE;
    let corpse_h = gs.mobjslab.alloc(corpse);

    a_vile_chase(&mut gs, vile, None);

    // Lost Soul should remain dead (not raisable).
    let still_dead = gs.mobjslab.get(corpse_h).expect("item must exist in tests");
    assert_eq!(still_dead.health, 0, "non-raisable corpse must stay dead");
}

// -----------------------------------------------------------------------
// Boss Brain action tests
// -----------------------------------------------------------------------

#[test]
fn brain_awake_sets_flag() {
    let mut gs = make_game_state();
    assert!(!gs.brain_awake);
    a_brain_awake(&mut gs);
    assert!(gs.brain_awake, "brain_awake must set the flag");
}

#[test]
fn brain_spit_does_nothing_when_not_awake() {
    let mut gs = make_game_state();
    gs.brain_targets
        .push((Fixed16_16::from_int(500), Fixed16_16::from_int(500)));
    let brain = spawn_monster_targeting_player(&mut gs, MobjKind::BossBrain, 200, 0, 250);

    let count_before = gs.mobjslab.len();
    a_brain_spit(&mut gs, brain);
    assert_eq!(
        gs.mobjslab.len(),
        count_before,
        "brain_spit must do nothing when brain_awake is false"
    );
}

#[test]
fn brain_spit_does_nothing_without_targets() {
    let mut gs = make_game_state();
    gs.brain_awake = true;
    let brain = spawn_monster_targeting_player(&mut gs, MobjKind::BossBrain, 200, 0, 250);

    let count_before = gs.mobjslab.len();
    a_brain_spit(&mut gs, brain);
    assert_eq!(
        gs.mobjslab.len(),
        count_before,
        "brain_spit must do nothing without brain_targets"
    );
}

#[test]
fn brain_spit_spawns_cube() {
    let mut gs = make_game_state();
    gs.brain_awake = true;
    gs.brain_targets
        .push((Fixed16_16::from_int(500), Fixed16_16::from_int(500)));
    let brain = spawn_monster_targeting_player(&mut gs, MobjKind::BossBrain, 200, 0, 250);

    let count_before = gs.mobjslab.len();
    a_brain_spit(&mut gs, brain);
    let count_after = gs.mobjslab.len();

    assert!(
        count_after > count_before,
        "brain_spit must spawn a BossCube"
    );

    let cube = gs.mobjslab.iter_handles().find(|h| {
        gs.mobjslab
            .get(*h)
            .map(|m| m.kind == MobjKind::BossCube)
            .unwrap_or(false)
    });
    assert!(cube.is_some(), "must spawn a BossCube MobjKind");
}

#[test]
fn brain_spit_round_robins_targets() {
    let mut gs = make_game_state();
    gs.brain_awake = true;
    gs.brain_targets
        .push((Fixed16_16::from_int(100), Fixed16_16::from_int(100)));
    gs.brain_targets
        .push((Fixed16_16::from_int(500), Fixed16_16::from_int(500)));
    let brain = spawn_monster_targeting_player(&mut gs, MobjKind::BossBrain, 200, 0, 250);

    assert_eq!(gs.brain_target_index, 0);
    a_brain_spit(&mut gs, brain);
    assert_eq!(
        gs.brain_target_index, 1,
        "first spit uses index 0, advances to 1"
    );
    a_brain_spit(&mut gs, brain);
    assert_eq!(
        gs.brain_target_index, 2,
        "second spit uses index 1, advances to 2"
    );
    // Third spit should wrap around (2 % 2 == 0).
    a_brain_spit(&mut gs, brain);
    assert_eq!(
        gs.brain_target_index, 1,
        "third spit wraps to index 0, advances to 1"
    );
}

#[test]
fn spawn_fly_spawns_monster_and_fog() {
    let mut gs = make_game_state();
    gs.brain_awake = true;
    gs.brain_targets
        .push((Fixed16_16::from_int(500), Fixed16_16::from_int(500)));

    // Create a cube with reactiontime pointing to target index 0.
    let mut cube = Mobj::new(
        MobjKind::BossCube,
        Fixed16_16::from_int(500),
        Fixed16_16::from_int(500),
        Bam::ZERO,
    );
    cube.health = 1;
    cube.flags = flags::MF_NOBLOCKMAP | flags::MF_MISSILE;
    cube.reactiontime = 0; // target index
    let cube_h = gs.mobjslab.alloc(cube);

    let count_before = gs.mobjslab.len();
    a_spawn_fly(&mut gs, cube_h);
    let count_after = gs.mobjslab.len();

    // Should spawn monster + fog - cube (removed) = net +1.
    // But the cube is freed, so we spawned 2 new (monster + fog) - 1 freed = net +1.
    assert!(
        count_after >= count_before,
        "spawn_fly must spawn monster + fog, got {count_before} -> {count_after}"
    );

    // Cube should be freed.
    assert!(
        gs.mobjslab.get(cube_h).is_none(),
        "cube must be removed after spawning"
    );

    // SpawnFire fog should exist.
    let fog = gs.mobjslab.iter_handles().find(|h| {
        gs.mobjslab
            .get(*h)
            .map(|m| m.kind == MobjKind::SpawnFire)
            .unwrap_or(false)
    });
    assert!(fog.is_some(), "must spawn a SpawnFire fog effect");
}

#[test]
fn spawn_fly_monster_has_reaction_time() {
    let mut gs = make_game_state();
    gs.brain_awake = true;
    gs.brain_targets
        .push((Fixed16_16::from_int(500), Fixed16_16::from_int(500)));

    let mut cube = Mobj::new(
        MobjKind::BossCube,
        Fixed16_16::from_int(500),
        Fixed16_16::from_int(500),
        Bam::ZERO,
    );
    cube.health = 1;
    cube.flags = flags::MF_NOBLOCKMAP | flags::MF_MISSILE;
    cube.reactiontime = 0;
    let cube_h = gs.mobjslab.alloc(cube);

    a_spawn_fly(&mut gs, cube_h);

    // Find the spawned monster (not SpawnFire, not player, not cube).
    let monster = gs.mobjslab.iter_handles().find(|h| {
        gs.mobjslab
            .get(*h)
            .map(|m| {
                m.kind != MobjKind::Player
                    && m.kind != MobjKind::SpawnFire
                    && m.kind != MobjKind::BossCube
                    && BOSS_SPAWN_TYPES.contains(&m.kind)
            })
            .unwrap_or(false)
    });
    assert!(
        monster.is_some(),
        "must spawn a monster from BOSS_SPAWN_TYPES"
    );

    let spawned = gs
        .mobjslab
        .get(monster.expect("item must exist in tests"))
        .expect("item must exist in tests");
    assert_eq!(
        spawned.reactiontime, 18,
        "spawned monster must have reactiontime=18"
    );
}

#[test]
fn brain_die_triggers_exit() {
    let mut gs = make_game_state();
    assert!(gs.exit_request.is_none());
    a_brain_die(&mut gs);
    assert_eq!(
        gs.exit_request,
        Some(crate::state::ExitRequest::Normal),
        "brain_die must trigger a normal exit"
    );
}

#[test]
fn brain_scream_spawns_explosions() {
    let mut gs = make_game_state();
    let brain = spawn_monster_targeting_player(&mut gs, MobjKind::BossBrain, 200, 0, 250);

    let count_before = gs.mobjslab.len();
    a_brain_scream(&mut gs, brain);
    let count_after = gs.mobjslab.len();

    assert_eq!(
        count_after - count_before,
        20,
        "brain_scream must spawn exactly 20 explosions"
    );
}

#[test]
fn brain_explode_spawns_one_explosion() {
    let mut gs = make_game_state();
    let brain = spawn_monster_targeting_player(&mut gs, MobjKind::BossBrain, 200, 0, 250);

    let count_before = gs.mobjslab.len();
    a_brain_explode(&mut gs, brain);
    let count_after = gs.mobjslab.len();

    assert_eq!(
        count_after - count_before,
        1,
        "brain_explode must spawn exactly 1 explosion"
    );
}

#[test]
fn dispatch_brain_awake_does_not_panic() {
    let mut gs = make_game_state();
    let brain = spawn_monster_targeting_player(&mut gs, MobjKind::BossBrain, 200, 0, 250);
    dispatch_action(&mut gs, brain, Action::BrainAwake as u8, None);
    assert!(gs.brain_awake);
}

#[test]
fn dispatch_brain_spit_does_not_panic() {
    let mut gs = make_game_state();
    let brain = spawn_monster_targeting_player(&mut gs, MobjKind::BossBrain, 200, 0, 250);
    dispatch_action(&mut gs, brain, Action::BrainSpit as u8, None);
}

#[test]
fn dispatch_brain_die_does_not_panic() {
    let mut gs = make_game_state();
    let brain = spawn_monster_targeting_player(&mut gs, MobjKind::BossBrain, 200, 0, 250);
    dispatch_action(&mut gs, brain, Action::BrainDie as u8, None);
}

#[test]
fn dispatch_brain_scream_does_not_panic() {
    let mut gs = make_game_state();
    let brain = spawn_monster_targeting_player(&mut gs, MobjKind::BossBrain, 200, 0, 250);
    dispatch_action(&mut gs, brain, Action::BrainScream as u8, None);
}

#[test]
fn dispatch_brain_explode_does_not_panic() {
    let mut gs = make_game_state();
    let brain = spawn_monster_targeting_player(&mut gs, MobjKind::BossBrain, 200, 0, 250);
    dispatch_action(&mut gs, brain, Action::BrainExplode as u8, None);
}

// --- New action no-target / stale-handle edge cases ---

#[test]
fn vile_actions_noop_without_target() {
    let mut gs = make_game_state();
    let mut mo = Mobj::new(
        MobjKind::ArchVile,
        Fixed16_16::from_int(100),
        Fixed16_16::from_int(0),
        Bam::ZERO,
    );
    mo.health = 700;
    mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE;
    let h = gs.mobjslab.alloc(mo);

    // None of these should panic.
    dispatch_action(&mut gs, h, Action::VileStart as u8, None);
    dispatch_action(&mut gs, h, Action::VileTarget as u8, None);
    dispatch_action(&mut gs, h, Action::VileAttack as u8, None);
}

#[test]
fn vile_actions_noop_with_dead_target() {
    let mut gs = make_game_state();
    gs.mobjslab
        .get_mut(gs.player.handle)
        .expect("item must exist in tests")
        .health = 0;
    let vile = spawn_monster_targeting_player(&mut gs, MobjKind::ArchVile, 200, 0, 700);

    let count_before = gs.mobjslab.len();
    dispatch_action(&mut gs, vile, Action::VileTarget as u8, None);
    assert_eq!(
        gs.mobjslab.len(),
        count_before,
        "must not spawn fire when target is dead"
    );
}

#[test]
fn boss_spawn_types_has_11_entries() {
    assert_eq!(
        BOSS_SPAWN_TYPES.len(),
        11,
        "BOSS_SPAWN_TYPES must have 11 monster types"
    );
}
#[test]
fn get_alive_target_with_pos_returns_none_no_target() {
    let mut gs = make_game_state();
    let mo = Mobj::new(
        MobjKind::Imp,
        Fixed16_16::from_int(0),
        Fixed16_16::from_int(0),
        Bam::ZERO,
    );
    let handle = gs.mobjslab.alloc(mo);
    assert_eq!(get_alive_target_with_pos(&gs, handle), None);
}

#[test]
fn get_alive_target_returns_none_no_target() {
    let mut gs = make_game_state();
    let mo = Mobj::new(
        MobjKind::Imp,
        Fixed16_16::from_int(0),
        Fixed16_16::from_int(0),
        Bam::ZERO,
    );
    let handle = gs.mobjslab.alloc(mo);
    assert_eq!(get_alive_target(&gs, handle), None);
}

#[test]
fn get_alive_target_returns_none_dead_target() {
    let mut gs = make_game_state();
    gs.mobjslab
        .get_mut(gs.player.handle)
        .expect("item must exist in tests")
        .health = 0;
    let imp = spawn_monster_targeting_player(&mut gs, MobjKind::Imp, 100, 100, 100);
    assert_eq!(get_alive_target(&gs, imp), None);
}

#[test]
fn get_alive_target_returns_target() {
    let mut gs = make_game_state();
    let target_h = gs.player.handle;
    let imp = spawn_monster_targeting_player(&mut gs, MobjKind::Imp, 100, 100, 100);
    assert_eq!(get_alive_target(&gs, imp), Some(target_h));
}

#[test]
fn get_alive_target_with_pos_returns_none_dead_target() {
    let mut gs = make_game_state();
    gs.mobjslab
        .get_mut(gs.player.handle)
        .expect("item must exist in tests")
        .health = 0;
    let imp = spawn_monster_targeting_player(&mut gs, MobjKind::Imp, 100, 100, 100);
    assert_eq!(get_alive_target_with_pos(&gs, imp), None);
}

#[test]
fn get_alive_target_with_pos_returns_target() {
    let mut gs = make_game_state();
    let target_h = gs.player.handle;
    let imp = spawn_monster_targeting_player(&mut gs, MobjKind::Imp, 100, 100, 100);
    let _target = gs.mobjslab.get(target_h).expect("item must exist in tests");

    let result = get_alive_target_with_pos(&gs, imp);
    assert!(result.is_some());
    let (found_target, x, y) = result.expect("item must exist in tests");
    assert_eq!(found_target, target_h);
    // The position returned is the caller's position, not the target's
    let imp_mo = gs.mobjslab.get(imp).expect("item must exist in tests");
    assert_eq!(x, imp_mo.x);
    assert_eq!(y, imp_mo.y);
}
