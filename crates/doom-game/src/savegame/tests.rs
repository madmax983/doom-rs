use super::*;
use crate::mobj::{Mobj, StateNum};
use crate::player::PlayerState;
use crate::savegame::{ReadCursor, SaveError, detect_save_format, load_game, save_game};
use crate::state::GameState;
use doom_types::weapons::WeaponType;

use crate::mobj::flags;
use doom_types::mobj_kind::MobjKind;
use doom_types::{Bam, Fixed16_16};

/// Helper: create a default GameState with a player mobj.
fn test_game_state() -> GameState {
    let mut gs = GameState::new("E1M1");
    let mo = Mobj::new(
        MobjKind::Player,
        Fixed16_16::from_int(100),
        Fixed16_16::from_int(200),
        Bam(0x4000_0000),
    );
    let handle = gs.mobjslab.alloc(mo);
    gs.player = PlayerState::pistol_start(handle);
    gs
}

fn test_level_name() -> [u8; 8] {
    let mut name = [0u8; 8];
    name[..4].copy_from_slice(b"E1M1");
    name
}

fn vanilla_header_bytes() -> Vec<u8> {
    let mut data = Vec::new();
    let mut description = [0u8; 24];
    description[..12].copy_from_slice(b"Vanilla test");
    data.extend_from_slice(&description);

    let mut version = [0u8; 16];
    version[..11].copy_from_slice(b"version 109");
    data.extend_from_slice(&version);

    data.push(2);
    data.push(1);
    data.push(1);
    data.extend_from_slice(&[1, 0, 0, 0]);
    data.extend_from_slice(&[0x23, 0x01, 0x00]);
    data
}

#[test]
fn detect_save_format_distinguishes_doomrs_and_vanilla_headers() {
    let gs = test_game_state();
    let doomrs = save_game(&gs, &test_level_name(), 2, "format test");
    assert_eq!(
        detect_save_format(&doomrs).expect("Value must exist"),
        SaveFormat::DoomRs
    );

    let vanilla = vanilla_header_bytes();
    assert_eq!(
        detect_save_format(&vanilla).expect("Value must exist"),
        SaveFormat::VanillaDsg
    );
}

#[test]
fn explicit_vanilla_save_does_not_route_through_doomrs_serializer() {
    let gs = test_game_state();
    let result =
        save_game_with_format(&gs, &test_level_name(), 2, "strict", SaveFormat::VanillaDsg);

    assert_eq!(
        result.expect_err("Must be an error"),
        SaveError::UnsupportedVanillaDsg
    );
}

#[test]
fn explicit_vanilla_load_fails_with_unsupported_error() {
    let vanilla = vanilla_header_bytes();

    let err = load_game(&vanilla).expect_err("vanilla boundary should reject payload load");
    assert_eq!(err, SaveError::UnsupportedVanillaDsg);
}

#[test]
fn mobj_kind_roundtrip() {
    // Enumerate over all valid discriminants and ensure they parse properly.
    // And also make sure we test out of bounds.
    for disc in 0..=75 {
        let kind = MobjKind::from_repr(disc).expect("all 0..=75 must map to a MobjKind");
        let mut w = WriteCursor::new(2);
        write_mobj_kind(&mut w, kind);
        let mut r = ReadCursor::new(&w.buf);
        let parsed = read_mobj_kind(&mut r).expect("must parse back");
        assert_eq!(parsed, kind);
    }

    // 76 is out of bounds
    assert!(MobjKind::from_repr(76).is_none());
    assert!(MobjKind::from_repr(999).is_none());
    assert!(MobjKind::from_repr(0xFFFF).is_none());
}

#[test]
fn bad_version_load_game_returns_error() {
    let gs = test_game_state();
    let mut data = save_game(&gs, &test_level_name(), 2, "test save");
    // Tamper with the version byte to simulate an unsupported version
    data[4] = 0xFF;

    let result = load_game(&data);
    assert_eq!(result.expect_err("Must be an error"), SaveError::BadVersion);
}

#[test]
fn bad_version_load_game_doomrs_returns_error() {
    let gs = test_game_state();
    let mut data = save_game_doomrs(&gs, &test_level_name(), 2, "test save");
    data[4] = 0xFF;
    let result = load_game_doomrs(&data);
    assert_eq!(result.expect_err("Must be an error"), SaveError::BadVersion);
}

#[test]
fn load_game_doomrs_too_short() {
    let result = load_game_doomrs(b"TOO_SHORT");
    assert_eq!(result.expect_err("Must be an error"), SaveError::TooShort);
}

#[test]
fn load_game_doomrs_bad_magic() {
    let mut data = vec![0; 50];
    data[0..4].copy_from_slice(b"MOOD");
    let result = load_game_doomrs(&data);
    assert_eq!(result.expect_err("Must be an error"), SaveError::BadMagic);
}

#[test]
fn too_short_load_game_returns_error() {
    let result = load_game(b"DOOM");
    assert_eq!(result.expect_err("Must be an error"), SaveError::TooShort);
}

#[test]
fn bad_magic_load_game_returns_error() {
    let gs = test_game_state();
    let mut data = save_game(&gs, &test_level_name(), 2, "test save");
    data[0..4].copy_from_slice(b"MOOD");
    let result = load_game(&data);
    assert_eq!(result.expect_err("Must be an error"), SaveError::BadMagic);
}

#[test]
fn save_format_display() {
    assert_eq!(format!("{}", SaveFormat::DoomRs), "DoomRs");
    assert_eq!(format!("{}", SaveFormat::VanillaDsg), "VanillaDsg");
}

// --- Test 1: save_game produces bytes starting with SAVE_MAGIC ---
#[test]
fn save_starts_with_magic() {
    let gs = test_game_state();
    let data = save_game(&gs, &test_level_name(), 2, "test save");
    assert_eq!(&data[..4], &SAVE_MAGIC);
}

// --- Test 2: save_game header has correct version ---
#[test]
fn save_header_version() {
    let gs = test_game_state();
    let data = save_game(&gs, &test_level_name(), 2, "test save");
    let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    assert_eq!(version, SAVE_VERSION);
}

// --- Test 3: save_game header stores level_name ---
#[test]
fn save_header_level_name() {
    let gs = test_game_state();
    let ln = test_level_name();
    let data = save_game(&gs, &ln, 2, "test save");
    assert_eq!(&data[8..16], &ln);
}

// --- Test 4: load_game with empty bytes returns TooShort ---
#[test]
fn load_empty_returns_too_short() {
    assert_eq!(
        load_game(&[]).expect_err("Must be an error"),
        SaveError::TooShort
    );
}

// --- Test 6: load_game with bad version returns BadVersion ---
#[test]
fn load_bad_version() {
    let mut data = vec![0u8; 64];
    data[..4].copy_from_slice(&SAVE_MAGIC);
    data[4..8].copy_from_slice(&99u32.to_le_bytes());
    assert_eq!(
        load_game(&data).expect_err("Must be an error"),
        SaveError::BadVersion
    );
}

// --- Test 7: Roundtrip preserves player health ---
#[test]
fn roundtrip_player_health() {
    let mut gs = test_game_state();
    gs.player.apply_damage(30); // health = 70
    let data = save_game(&gs, &test_level_name(), 2, "health test");
    let loaded = load_game(&data).expect("load must succeed");
    assert_eq!(loaded.state.player.health(), 70);
}

// --- Test 8: Roundtrip preserves player armor ---
#[test]
fn roundtrip_player_armor() {
    let mut gs = test_game_state();
    gs.player.give_armor(150, 2);
    let data = save_game(&gs, &test_level_name(), 2, "armor test");
    let loaded = load_game(&data).expect("load must succeed");
    assert_eq!(loaded.state.player.armor(), 150);
    assert_eq!(loaded.state.player.armor_type, 2);
}

// --- Test 9: Roundtrip preserves player ammo ---
#[test]
fn roundtrip_player_ammo() {
    let mut gs = test_game_state();
    gs.player.give_ammo(0, 100); // bullets = 150 (50 start + 100)
    gs.player.give_ammo(1, 25); // shells = 25
    let data = save_game(&gs, &test_level_name(), 2, "ammo test");
    let loaded = load_game(&data).expect("load must succeed");
    assert_eq!(loaded.state.player.ammo(0), 150);
    assert_eq!(loaded.state.player.ammo(1), 25);
}

// --- Test 10: Roundtrip preserves player keys ---
#[test]
fn roundtrip_player_keys() {
    let mut gs = test_game_state();
    gs.player.give_key(0x01); // blue card
    gs.player.give_key(0x10); // yellow skull
    let data = save_game(&gs, &test_level_name(), 2, "keys test");
    let loaded = load_game(&data).expect("load must succeed");
    assert_eq!(loaded.state.player.keys, 0x11);
}

// --- Test 11: Roundtrip preserves player weapons ---
#[test]
fn roundtrip_player_weapons() {
    let mut gs = test_game_state();
    gs.player.weapons[WeaponType::Shotgun as usize] = true;
    gs.player.weapons[WeaponType::Chaingun as usize] = true;
    let data = save_game(&gs, &test_level_name(), 2, "weapons test");
    let loaded = load_game(&data).expect("load must succeed");
    assert!(loaded.state.player.weapons[WeaponType::Fist as usize]);
    assert!(loaded.state.player.weapons[WeaponType::Pistol as usize]);
    assert!(loaded.state.player.weapons[WeaponType::Shotgun as usize]);
    assert!(loaded.state.player.weapons[WeaponType::Chaingun as usize]);
    assert!(!loaded.state.player.weapons[WeaponType::RocketLauncher as usize]);
}

// --- Test 12: Roundtrip preserves level_time ---
#[test]
fn roundtrip_level_time() {
    let mut gs = test_game_state();
    gs.stats.level_time = 3500;
    let data = save_game(&gs, &test_level_name(), 2, "time test");
    let loaded = load_game(&data).expect("load must succeed");
    assert_eq!(loaded.state.stats.level_time, 3500);
}

// --- Test 13: Roundtrip preserves rng state ---
#[test]
fn roundtrip_rng_state() {
    let mut gs = test_game_state();
    for _ in 0..42 {
        gs.rng.next_byte();
    }
    let saved_index = gs.rng.index();
    let data = save_game(&gs, &test_level_name(), 2, "rng test");
    let loaded = load_game(&data).expect("load must succeed");
    assert_eq!(loaded.state.rng.index(), saved_index);
}

// --- Test 14: Roundtrip preserves total_kills/items/secrets ---
#[test]
fn roundtrip_totals() {
    let mut gs = test_game_state();
    gs.stats.total_kills = 50;
    gs.stats.total_items = 30;
    gs.stats.total_secrets = 5;
    gs.stats.kill_count = 10;
    gs.stats.item_count = 7;
    gs.stats.secret_count = 2;
    let data = save_game(&gs, &test_level_name(), 2, "totals test");
    let loaded = load_game(&data).expect("load must succeed");
    assert_eq!(loaded.state.stats.total_kills, 50);
    assert_eq!(loaded.state.stats.total_items, 30);
    assert_eq!(loaded.state.stats.total_secrets, 5);
    assert_eq!(loaded.state.stats.kill_count, 10);
    assert_eq!(loaded.state.stats.item_count, 7);
    assert_eq!(loaded.state.stats.secret_count, 2);
}

// --- Test 15: Roundtrip preserves exit_request (None) ---
#[test]
fn roundtrip_exit_request_none() {
    let gs = test_game_state();
    let data = save_game(&gs, &test_level_name(), 2, "exit test");
    let loaded = load_game(&data).expect("load must succeed");
    assert_eq!(loaded.state.exit_request, None);
}

// --- Test 15b: Roundtrip preserves exit_request (Normal) ---
#[test]
fn roundtrip_exit_request_normal() {
    let mut gs = test_game_state();
    gs.exit_request = Some(ExitRequest::Normal);
    let data = save_game(&gs, &test_level_name(), 2, "exit normal");
    let loaded = load_game(&data).expect("load must succeed");
    assert_eq!(loaded.state.exit_request, Some(ExitRequest::Normal));
}

// --- Test 15c: Roundtrip preserves exit_request (Secret) ---
#[test]
fn roundtrip_exit_request_secret() {
    let mut gs = test_game_state();
    gs.exit_request = Some(ExitRequest::Secret);
    let data = save_game(&gs, &test_level_name(), 2, "exit secret");
    let loaded = load_game(&data).expect("load must succeed");
    assert_eq!(loaded.state.exit_request, Some(ExitRequest::Secret));
}

// --- Test 16: Roundtrip with door movers preserves count ---
#[test]
fn roundtrip_door_movers() {
    let mut gs = test_game_state();
    gs.movers.active_doors.push(DoorMover {
        sector: 5,
        target_height: 128,
        current_height: 64,
        speed: 2,
        is_ceiling: true,
        wait_tics: 120,
        countdown: 60,
        reopen_height: 0,
        reopen_countdown: -1,
    });
    gs.movers.active_doors.push(DoorMover {
        sector: 10,
        target_height: 0,
        current_height: 100,
        speed: -2,
        is_ceiling: true,
        wait_tics: 0,
        countdown: -1,
        reopen_height: 0,
        reopen_countdown: -1,
    });
    let data = save_game(&gs, &test_level_name(), 2, "doors test");
    let loaded = load_game(&data).expect("load must succeed");
    assert_eq!(loaded.state.movers.active_doors.len(), 2);
    assert_eq!(loaded.state.movers.active_doors[0].sector, 5);
    assert_eq!(loaded.state.movers.active_doors[0].target_height, 128);
    assert_eq!(loaded.state.movers.active_doors[1].sector, 10);
    assert_eq!(loaded.state.movers.active_doors[1].speed, -2);
}

// --- Test 17: Roundtrip with floor movers preserves count ---
#[test]
fn roundtrip_floor_movers() {
    let mut gs = test_game_state();
    gs.movers.active_floors.push(FloorMover {
        sector_index: 3,
        target_height: -64,
        speed: 4,
        direction: MoveDirection::Down,
        wait_tics: 105,
        return_height: 0,
        waiting: false,
        wait_remaining: 0,
        crush: crate::state::CrushBehavior::Crush,
        tag: 7,
        floor_type: FloorType::LowerToLowest,
    });
    let data = save_game(&gs, &test_level_name(), 2, "floors test");
    let loaded = load_game(&data).expect("load must succeed");
    assert_eq!(loaded.state.movers.active_floors.len(), 1);
    assert_eq!(loaded.state.movers.active_floors[0].sector_index, 3);
    assert_eq!(loaded.state.movers.active_floors[0].target_height, -64);
    assert_eq!(
        loaded.state.movers.active_floors[0].direction,
        MoveDirection::Down
    );
    assert!(loaded.state.movers.active_floors[0].crush == crate::state::CrushBehavior::Crush);
}

// --- Test 18: save_slot_filename format ---
#[test]
fn slot_filename_format() {
    assert_eq!(save_slot_filename(0), "doomsav0.dsg");
    assert_eq!(save_slot_filename(5), "doomsav5.dsg");
}

// --- Test 19: SaveError derives PartialEq ---
#[test]
fn save_error_partial_eq() {
    assert_eq!(SaveError::TooShort, SaveError::TooShort);
    assert_ne!(SaveError::TooShort, SaveError::BadMagic);
    assert_ne!(SaveError::BadVersion, SaveError::Truncated);
}

// --- Test 20: WriteCursor/ReadCursor roundtrip for each primitive ---
#[test]
fn cursor_roundtrip_primitives() {
    let mut w = WriteCursor::new(64);
    w.write_u8(0xAB);
    w.write_i16(-1234);
    w.write_u16(0xBEEF);
    w.write_i32(-100_000);
    w.write_u32(0xDEAD_BEEF);
    w.write_bool(true);
    w.write_bool(false);

    let data = w.into_bytes();
    let mut r = ReadCursor::new(&data);

    assert_eq!(r.read_u8().expect("Value must exist"), 0xAB);
    assert_eq!(r.read_i16().expect("Value must exist"), -1234);
    assert_eq!(r.read_u16().expect("Value must exist"), 0xBEEF);
    assert_eq!(r.read_i32().expect("Value must exist"), -100_000);
    assert_eq!(r.read_u32().expect("Value must exist"), 0xDEAD_BEEF);
    assert!(r.read_bool().expect("Value must exist"));
    assert!(!r.read_bool().expect("Value must exist"));
}

#[test]
fn cursor_read_truncated() {
    let empty: [u8; 0] = [];
    let mut r = ReadCursor::new(&empty);
    assert_eq!(
        r.read_u8().expect_err("Must be an error"),
        SaveError::Truncated
    );
    assert_eq!(
        r.read_i16().expect_err("Must be an error"),
        SaveError::Truncated
    );
    assert_eq!(
        r.read_u16().expect_err("Must be an error"),
        SaveError::Truncated
    );
    assert_eq!(
        r.read_i32().expect_err("Must be an error"),
        SaveError::Truncated
    );
    assert_eq!(
        r.read_u32().expect_err("Must be an error"),
        SaveError::Truncated
    );
    assert_eq!(
        r.read_bool().expect_err("Must be an error"),
        SaveError::Truncated
    );

    let one_byte: [u8; 1] = [0xAB];
    let mut r2 = ReadCursor::new(&one_byte);
    assert_eq!(
        r2.read_i16().expect_err("Must be an error"),
        SaveError::Truncated
    );
    assert_eq!(
        r2.read_u16().expect_err("Must be an error"),
        SaveError::Truncated
    );
    assert_eq!(
        r2.read_i32().expect_err("Must be an error"),
        SaveError::Truncated
    );
    assert_eq!(
        r2.read_u32().expect_err("Must be an error"),
        SaveError::Truncated
    );

    let three_bytes: [u8; 3] = [0xAB, 0xCD, 0xEF];
    let mut r3 = ReadCursor::new(&three_bytes);
    assert_eq!(
        r3.read_i32().expect_err("Must be an error"),
        SaveError::Truncated
    );
    assert_eq!(
        r3.read_u32().expect_err("Must be an error"),
        SaveError::Truncated
    );
}

// --- Test 21: Roundtrip preserves mobj data ---
#[test]
fn roundtrip_mobj_data() {
    let mut gs = test_game_state();
    // Add a monster.
    let mut imp = Mobj::new(
        MobjKind::Imp,
        Fixed16_16::from_int(500),
        Fixed16_16::from_int(-300),
        Bam(0x8000_0000),
    );
    imp.health = 60;
    imp.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
    imp.momx = Fixed16_16::from_int(2);
    imp.state = StateNum(42);
    imp.tics = 10;
    gs.mobjslab.alloc(imp);

    let data = save_game(&gs, &test_level_name(), 2, "mobj test");
    let loaded = load_game(&data).expect("load must succeed");

    // Should have 2 mobjs (player + imp).
    assert_eq!(loaded.state.mobjslab.len(), 2);

    // Check that the imp's data survived.
    let mut handles = Vec::with_capacity(loaded.state.mobjslab.len());
    handles.extend(loaded.state.mobjslab.iter_handles());
    // Find the imp (index 1 since player was allocated first).
    let imp_handle = handles.iter().find(|h| {
        loaded
            .state
            .mobjslab
            .get(**h)
            .is_some_and(|m| m.kind == MobjKind::Imp)
    });
    assert!(imp_handle.is_some(), "imp must be present after load");
    let imp_loaded = loaded
        .state
        .mobjslab
        .get(*imp_handle.expect("Value must exist"))
        .expect("Value must exist");
    assert_eq!(imp_loaded.health, 60);
    assert_eq!(imp_loaded.x, Fixed16_16::from_int(500));
    assert_eq!(imp_loaded.y, Fixed16_16::from_int(-300));
    assert_eq!(imp_loaded.angle, Bam(0x8000_0000));
    assert_eq!(imp_loaded.state, StateNum(42));
    assert_eq!(imp_loaded.tics, 10);
}

// --- Test 22: Roundtrip preserves ceiling movers ---
#[test]
fn roundtrip_ceiling_movers() {
    let mut gs = test_game_state();
    gs.movers.active_ceilings.push(CeilingMover {
        sector_index: 7,
        top_height: 128,
        bottom_height: 8,
        speed: 1,
        normal_speed: 1,
        crush_damage: 10,
        direction: MoveDirection::Down,
        silent: false,
        remove_when_done: false,
        tag: 42,
        ceiling_type: CeilingType::CrushAndRaise,
    });
    let data = save_game(&gs, &test_level_name(), 2, "ceiling test");
    let loaded = load_game(&data).expect("load must succeed");
    assert_eq!(loaded.state.movers.active_ceilings.len(), 1);
    assert_eq!(loaded.state.movers.active_ceilings[0].sector_index, 7);
    assert_eq!(loaded.state.movers.active_ceilings[0].crush_damage, 10);
    assert_eq!(loaded.state.movers.active_ceilings[0].tag, 42);
}

// --- Test 23: Roundtrip preserves light specials ---
#[test]
fn roundtrip_light_specials() {
    let mut gs = test_game_state();
    gs.movers.active_lights.push(LightSpecial {
        sector: 2,
        timer: 15,
        period: 30,
        bright: 255,
        dark: 128,
        is_bright: true,
    });
    let data = save_game(&gs, &test_level_name(), 2, "light test");
    let loaded = load_game(&data).expect("load must succeed");
    assert_eq!(loaded.state.movers.active_lights.len(), 1);
    assert_eq!(loaded.state.movers.active_lights[0].sector, 2);
    assert_eq!(loaded.state.movers.active_lights[0].bright, 255);
    assert!(loaded.state.movers.active_lights[0].is_bright);
}

// --- Test 24: Roundtrip preserves tic_num ---
#[test]
fn roundtrip_tic_num() {
    let mut gs = test_game_state();
    gs.tic_num = 12345;
    let data = save_game(&gs, &test_level_name(), 2, "tic test");
    let loaded = load_game(&data).expect("load must succeed");
    assert_eq!(loaded.state.tic_num, 12345);
}

// --- Test 25: Roundtrip preserves player mobj handle validity ---
#[test]
fn roundtrip_player_mobj_handle() {
    let gs = test_game_state();
    let data = save_game(&gs, &test_level_name(), 2, "handle test");
    let loaded = load_game(&data).expect("load must succeed");
    // The player handle must point to a valid mobj.
    let player_mo = loaded.state.mobjslab.get(loaded.state.player.handle);
    assert!(player_mo.is_some(), "player handle must resolve after load");
    assert_eq!(player_mo.expect("Value must exist").kind, MobjKind::Player);
}

// --- Test 26: Roundtrip with multiple mobjs ---
#[test]
fn roundtrip_multiple_mobjs() {
    let mut gs = test_game_state();
    // Add several monsters.
    for i in 0..5 {
        let mut trooper = Mobj::new(
            MobjKind::Trooper,
            Fixed16_16::from_int(i * 100),
            Fixed16_16::from_int(i * 50),
            Bam::ZERO,
        );
        trooper.health = 20;
        gs.mobjslab.alloc(trooper);
    }
    gs.stats.total_kills = 5;
    let data = save_game(&gs, &test_level_name(), 2, "multi mobj");
    let loaded = load_game(&data).expect("load must succeed");
    // 1 player + 5 troopers.
    assert_eq!(loaded.state.mobjslab.len(), 6);
    assert_eq!(loaded.state.stats.total_kills, 5);
}

// --- Test 27: Header description is stored correctly ---

// --- Havoc Test: Description is invalid UTF-8 panics if unwrapped ---
#[test]
fn havoc_test_invalid_utf8_description() {
    let gs = test_game_state();
    let valid_data = save_game(&gs, &test_level_name(), 3, "My Cool Save");
    let mut data = valid_data.clone();

    let desc_start = 21; // offset of description
    data[desc_start] = 0x80; // Invalid UTF-8 byte

    // The save file may be truncated due to the invalid description if loaded
    if let Ok(loaded) = load_game(&data) {
        let desc = &loaded.header.description;
        let desc_str = core::str::from_utf8(desc)
            .unwrap_or("")
            .trim_end_matches('\0');
        assert_eq!(desc_str, "");
    }
}

#[test]
fn save_header_description() {
    let gs = test_game_state();
    let data = save_game(&gs, &test_level_name(), 3, "My Cool Save");
    let loaded = load_game(&data).expect("load must succeed");
    // Description should start with "My Cool Save" then be null-padded.
    let desc = &loaded.header.description;
    let desc_str = core::str::from_utf8(desc)
        .unwrap_or("")
        .trim_end_matches('\0');
    assert_eq!(desc_str, "My Cool Save");
    assert_eq!(loaded.header.skill, 3);
}

// --- Test 28: load_game with truncated data after header returns Truncated ---
#[test]
fn load_truncated_after_header() {
    let gs = test_game_state();
    let data = save_game(&gs, &test_level_name(), 2, "truncate test");
    // Truncate to just the header.
    let truncated = &data[..45];
    assert_eq!(
        load_game(truncated).expect_err("Must be an error"),
        SaveError::Truncated
    );
}

// --- Test 29: ReadCursor out of bounds returns Truncated ---
#[test]
fn read_cursor_out_of_bounds() {
    let data = [0u8; 3];
    let mut r = ReadCursor::new(&data);
    assert!(r.read_u8().is_ok());
    assert!(r.read_u8().is_ok());
    assert!(r.read_u8().is_ok());
    assert_eq!(r.read_u8(), Err(SaveError::Truncated));
}

// --- Test 30: ReadCursor read_i32 out of bounds ---
#[test]
fn read_cursor_i32_out_of_bounds() {
    let data = [0u8; 2];
    let mut r = ReadCursor::new(&data);
    assert_eq!(r.read_i32(), Err(SaveError::Truncated));
}

#[test]
fn read_cursor_bytes_out_of_bounds() {
    let data = [0u8; 3];
    let mut r = ReadCursor::new(&data);
    assert_eq!(r.read_bytes::<4>(), Err(SaveError::Truncated));
}

#[test]
fn read_cursor_bytes_exact_bounds() {
    let data = [1u8, 2u8, 3u8, 4u8];
    let mut r = ReadCursor::new(&data);
    assert_eq!(r.read_bytes::<4>(), Ok([1, 2, 3, 4]));
}

// --- Test 31: Roundtrip preserves player pending_weapon ---
#[test]
fn roundtrip_pending_weapon() {
    let mut gs = test_game_state();
    gs.player.pending_weapon = Some(WeaponType::Shotgun);
    let data = save_game(&gs, &test_level_name(), 2, "pending test");
    let loaded = load_game(&data).expect("load must succeed");
    assert_eq!(
        loaded.state.player.pending_weapon,
        Some(WeaponType::Shotgun)
    );
}

// --- Test 32: Roundtrip preserves player pending_weapon None ---
#[test]
fn roundtrip_pending_weapon_none() {
    let gs = test_game_state();
    let data = save_game(&gs, &test_level_name(), 2, "no pending");
    let loaded = load_game(&data).expect("load must succeed");
    assert_eq!(loaded.state.player.pending_weapon, None);
}

#[test]
fn roundtrip_attack_cooldown() {
    let mut gs = test_game_state();
    gs.player.attack_cooldown = 9;
    let data = save_game(&gs, &test_level_name(), 2, "cooldown test");
    let loaded = load_game(&data).expect("load must succeed");
    assert_eq!(loaded.state.player.attack_cooldown, 9);
}

#[test]
fn roundtrip_player_refire() {
    let mut gs = test_game_state();
    gs.player.refire = 7;
    let data = save_game(&gs, &test_level_name(), 2, "refire test");
    let loaded = load_game(&data).expect("load must succeed");
    assert_eq!(loaded.state.player.refire, 7);
}

#[test]
fn roundtrip_psprites() {
    let mut gs = test_game_state();
    gs.player.psprites[0] = PspriteState {
        state: StateNum(crate::states::ids::S_SGUN3),
        tics: 5,
        sx: 12,
        sy: 34,
    };
    gs.player.psprites[1] = PspriteState {
        state: StateNum(crate::states::ids::S_SGUN_FLASH1),
        tics: 2,
        sx: -3,
        sy: 99,
    };

    let data = save_game(&gs, &test_level_name(), 2, "psprite test");
    let loaded = load_game(&data).expect("load must succeed");

    assert_eq!(loaded.state.player.psprites, gs.player.psprites);
}

#[test]
fn roundtrip_player_extra_light() {
    let mut gs = test_game_state();
    gs.player.extra_light = 2;

    let data = save_game(&gs, &test_level_name(), 2, "extra light test");
    let loaded = load_game(&data).expect("load must succeed");

    assert_eq!(
        loaded.state.player.extra_light, 2,
        "player extra_light must survive save/load so weapon flash lighting stays deterministic"
    );
}

// --- Test 33: Havoc malicious string size ---
#[test]
fn load_game_malicious_string_length() {
    let gs = test_game_state();
    let mut data = save_game(&gs, &test_level_name(), 2, "havoc");

    for i in 0..(data.len() - 4) {
        // Find the string length
        if data[i] == 4 && data[i + 1] == 0 && data[i + 2] == 0 && data[i + 3] == 0 {
            // Confirm it's followed by E1M1
            if &data[i + 4..i + 8] == b"E1M1" {
                // Set length to u32::MAX
                data[i] = 0xFF;
                data[i + 1] = 0xFF;
                data[i + 2] = 0xFF;
                data[i + 3] = 0xFF;
                break;
            }
        }
    }

    let res = load_game(&data);
    assert_eq!(res.expect_err("Must be an error"), SaveError::Truncated);
}
