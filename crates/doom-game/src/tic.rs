//! Core simulation tick — `GameState::tick()`.
//!
//! Called exactly once per tic (35×/sec by the event loop).
//! Applies player input via `P_MovePlayer`, then runs the thinker loop
//! to advance all actor state machines by one step.
//!
//! # Player movement (P_MovePlayer / P_Thrust)
//!
//! The original Doom movement math:
//! ```text
//! mo->angle += cmd->angleturn << 16;          // 16-bit → 32-bit BAM
//! if (cmd->forwardmove)
//!     P_Thrust(player, mo->angle, cmd->forwardmove * 2048);
//! if (cmd->sidemove)
//!     P_Thrust(player, mo->angle - ANG90, cmd->sidemove * 2048);
//! // P_Thrust:
//!     mo->momx += FixedMul(move, finecosine[angle >> ANGLETOFINESHIFT]);
//!     mo->momy += FixedMul(move, finesine[angle >> ANGLETOFINESHIFT]);
//! ```
//! `P_TryMove` (blockmap collision) is a placeholder for batch 2.

use doom_map::Level;
use doom_types::{ANG90, Bam, Fixed16_16};

use crate::mobj::{MobjHandle, StateNum};
use crate::player::WeaponType;
use crate::state::GameState;

// ---------------------------------------------------------------------------
// TicCmd
// ---------------------------------------------------------------------------

/// One tic of player input — the wire-compatible command struct.
///
/// Layout is `repr(C)` with deterministic padding for netcode serialization.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct TicCmd {
    /// Forward/backward movement (-128..127, positive = forward).
    pub forward_move: i8,
    /// Lateral strafe (-128..127, positive = right).
    pub side_move: i8,
    /// Angle delta in 16-bit BAM units (shifted left 16 → 32-bit BAM).
    pub angle_turn: i16,
    /// Button bitfield (`bt::BT_*` flags).
    pub buttons: u8,
    /// ASCII chat character (0 = none).
    pub chatchar: u8,
    _pad: [u8; 2],
}

/// Button flag constants.
pub mod bt {
    /// Fire / attack.
    pub const BT_ATTACK: u8 = 0x01;
    /// Use / open / activate.
    pub const BT_USE: u8 = 0x02;
    /// Change weapon (weapon number encoded in `BT_WEAPONMASK`).
    pub const BT_CHANGE: u8 = 0x04;
    /// Bits 3–5 encode the target weapon number.
    pub const BT_WEAPONMASK: u8 = 0x38;
}

// ---------------------------------------------------------------------------
// Movement constants
// ---------------------------------------------------------------------------

/// Speed multiplier applied to `TicCmd::forward_move` and `side_move`.
///
/// Doom original: `cmd->forwardmove * 2048` (in fixed-point).
pub const PLAYER_SPEED_SCALE: i32 = 2048;

/// Ground friction coefficient (≈ 0.90625 in fixed-point: 59392/65536).
///
/// Doom original: `0xE800` stored as a fixed-point fraction.
pub const FRICTION: Fixed16_16 = Fixed16_16(0x0000_E800);

/// Maximum horizontal velocity per axis (15 map units).
pub const MAXMOVE: Fixed16_16 = Fixed16_16(15 << 16);

// ---------------------------------------------------------------------------
// GameState::tick
// ---------------------------------------------------------------------------

impl GameState {
    /// Advance the simulation by one tic.
    ///
    /// 1. Increment `tic_num`.
    /// 2. Apply `cmd` to the player Mobj (`P_MovePlayer`), including weapon
    ///    firing (`BT_ATTACK`) and use-key activation (`BT_USE`).
    /// 3. Tick sector specials (damage floors, crusher ceilings).
    /// 4. Run the thinker loop for all actors.
    ///
    /// `level` is `None` in unit tests (skips blockmap collision, BT_USE,
    /// and sector specials) and `Some(&mut level)` in real gameplay.
    pub fn tick(&mut self, cmd: TicCmd, mut level: Option<&mut Level>) {
        self.tic_num = self.tic_num.wrapping_add(1);

        // Movement + attack (immutable level borrow).
        self.p_move_player(cmd, level.as_deref());

        // Pickup check: scan MF_SPECIAL actors.
        if !self.player.is_dead() {
            crate::pickups::p_check_pickups(self);
        }

        // BT_ATTACK: fire current weapon.
        if cmd.buttons & bt::BT_ATTACK != 0 {
            let handle = self.player.handle;
            crate::weapons::fire_weapon(self, level.as_deref(), handle);
        }

        // BT_USE: activate linedef ahead of player.
        if cmd.buttons & bt::BT_USE != 0 {
            if let Some(lv) = level.as_deref_mut() {
                let handle = self.player.handle;
                crate::specials::p_use_lines(self, lv, handle);
            }
        }

        // BT_CHANGE: weapon switch.
        if cmd.buttons & bt::BT_CHANGE != 0 {
            let weapon_num = ((cmd.buttons & bt::BT_WEAPONMASK) >> 3) as usize;
            if let Some(weapon) = WeaponType::from_num(weapon_num) {
                if self.player.weapons[weapon as usize] {
                    self.player.weapon = weapon;
                }
            }
        }

        // Sector specials: damage floors, etc. (immutable level borrow).
        if let Some(lv) = level.as_deref() {
            let handle = self.player.handle;
            crate::specials::tick_sector_specials(self, lv, handle);
        }

        // Animated doors, ceilings, floors, and light specials (mutable level borrow).
        if let Some(lv) = level.as_deref_mut() {
            crate::specials::tick_doors(self, lv);
            crate::specials::tick_ceilings(self, lv);
            crate::specials::tick_floors(self, lv);
            crate::specials::tick_lights(self, lv);
        }

        // Thinker loop: advance all actor state machines.
        self.run_thinkers(level.as_deref());

        // Move projectiles: advance missile actors and check collisions.
        crate::projectile::p_move_projectiles(self, level.as_deref());
    }

    // -----------------------------------------------------------------------
    // P_MovePlayer — port of Doom's p_user.c: P_MovePlayer
    // -----------------------------------------------------------------------

    fn p_move_player(&mut self, cmd: TicCmd, level: Option<&Level>) {
        let handle = self.player.handle;

        // Thrust block: apply turn + acceleration, then release borrow.
        {
            let Some(mo) = self.mobjslab.get_mut(handle) else {
                return;
            };

            // 1. Turn: cmd.angle_turn is 16-bit BAM; shift to 32-bit BAM space.
            mo.angle = mo.angle + Bam((cmd.angle_turn as i32 as u32).wrapping_shl(16));

            // 2. Forward / backward thrust.
            if cmd.forward_move != 0 {
                let angle = mo.angle;
                p_thrust(mo, angle, cmd.forward_move);
            }

            // 3. Strafe: thrust perpendicular (90° left of facing direction).
            if cmd.side_move != 0 {
                let strafe_angle = mo.angle - ANG90;
                p_thrust(mo, strafe_angle, cmd.side_move);
            }
        }

        // 4. Compute proposed position, then collision-test (needs shared borrow).
        let (new_x, new_y) = match self.mobjslab.get(handle) {
            Some(mo) => (mo.x + mo.momx, mo.y + mo.momy),
            None => return,
        };
        let moved = match level {
            Some(lv) => crate::movement::p_try_move(&self.mobjslab, handle, new_x, new_y, lv),
            None => true,
        };

        // 5. Apply position + friction + clamp.
        let Some(mo) = self.mobjslab.get_mut(handle) else {
            return;
        };
        if moved {
            mo.x = new_x;
            mo.y = new_y;
        }
        mo.momx = mo.momx.fixed_mul(FRICTION);
        mo.momy = mo.momy.fixed_mul(FRICTION);
        mo.momx = mo.momx.clamp(-MAXMOVE, MAXMOVE);
        mo.momy = mo.momy.clamp(-MAXMOVE, MAXMOVE);
    }

    // -----------------------------------------------------------------------
    // Thinker loop — advance every actor's state machine by one step
    // -----------------------------------------------------------------------

    fn run_thinkers(&mut self, level: Option<&Level>) {
        // Collect handles first to avoid borrow conflicts during iteration.
        let handles: Vec<MobjHandle> = self.mobjslab.iter_handles().collect();

        for handle in handles {
            self.advance_mobj_state(handle, level);
        }
    }

    /// Advance `handle`'s state machine by one tic.
    ///
    /// When `tics` reaches zero the actor transitions to `next_state` and the
    /// new state's action function fires immediately (matching Doom's
    /// `P_SetMobjState` behaviour).
    fn advance_mobj_state(&mut self, handle: MobjHandle, level: Option<&Level>) {
        // Step 1: countdown.  Copy out the current StateNum on transition,
        // then release the borrow so dispatch_action can take &mut self.
        let cur_state: StateNum = {
            let Some(mo) = self.mobjslab.get_mut(handle) else {
                return;
            };
            // Infinite-duration states (tics < 0) hold until externally changed.
            if mo.tics < 0 {
                return;
            }
            mo.tics -= 1;
            if mo.tics > 0 {
                return;
            }
            mo.state // Copy: StateNum is Copy
        };

        // Step 2: look up next_state.
        let next_sn = crate::states::STATES
            .get(cur_state.0 as usize)
            .map(|e| e.next_state)
            .unwrap_or(StateNum::NULL);

        if next_sn == StateNum::NULL {
            // Terminal state: hold forever.
            if let Some(mo) = self.mobjslab.get_mut(handle) {
                mo.tics = -1;
            }
            return;
        }

        // Step 3: apply next state + read action, releasing borrow.
        let action = match crate::states::STATES.get(next_sn.0 as usize) {
            Some(entry) => {
                if let Some(mo) = self.mobjslab.get_mut(handle) {
                    mo.state = next_sn;
                    mo.tics = entry.tics;
                }
                entry.action
            }
            None => return,
        };

        // Step 4: fire action (needs &mut self — all borrows dropped above).
        crate::actions::dispatch_action(self, handle, action, level);
    }
}

// ---------------------------------------------------------------------------
// P_Thrust helper
// ---------------------------------------------------------------------------

/// Apply a thrust of `move_units` map-units/tic in direction `angle`.
///
/// Port of Doom's `P_Thrust`:
/// ```c
/// mo->momx += FixedMul(move, finecosine[angle >> ANGLETOFINESHIFT]);
/// mo->momy += FixedMul(move, finesine  [angle >> ANGLETOFINESHIFT]);
/// ```
///
/// Requires `Bam::init_trig_tables()` to have been called.
/// Returns zero thrust if tables have not been initialized
/// (safe startup behavior — trig returns 0 before init).
fn p_thrust(mo: &mut crate::mobj::Mobj, angle: Bam, move_units: i8) {
    let thrust = Fixed16_16::from_raw(move_units as i32 * PLAYER_SPEED_SCALE);
    mo.momx = mo.momx + angle.cos().fixed_mul(thrust);
    mo.momy = mo.momy + angle.sin().fixed_mul(thrust);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobj::{Mobj, MobjKind, flags};
    use crate::player::PlayerState;
    use doom_types::{Bam, Fixed16_16};

    /// Construct a game state with a live player Mobj at the origin.
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

    #[test]
    fn tick_increments_tic_num() {
        let mut gs = make_game_state();
        assert_eq!(gs.tic_num, 0);
        gs.tick(TicCmd::default(), None);
        assert_eq!(gs.tic_num, 1);
        gs.tick(TicCmd::default(), None);
        assert_eq!(gs.tic_num, 2);
    }

    #[test]
    fn tic_num_wraps_at_u32_max() {
        let mut gs = make_game_state();
        gs.tic_num = u32::MAX;
        gs.tick(TicCmd::default(), None);
        assert_eq!(gs.tic_num, 0);
    }

    #[test]
    fn no_input_keeps_player_at_origin() {
        // Trig tables not initialized → sin/cos = 0 → no thrust → no movement.
        let mut gs = make_game_state();
        gs.tick(TicCmd::default(), None);
        let mo = gs.mobjslab.get(gs.player.handle).unwrap();
        assert_eq!(mo.x, Fixed16_16::ZERO);
        assert_eq!(mo.y, Fixed16_16::ZERO);
    }

    #[test]
    fn angle_turn_updates_mobj_angle() {
        let mut gs = make_game_state();
        let cmd = TicCmd {
            angle_turn: 640,
            ..Default::default()
        };
        gs.tick(cmd, None);
        let mo = gs.mobjslab.get(gs.player.handle).unwrap();
        // angle_turn = 640 << 16 in 32-bit BAM.
        let expected = Bam((640i32 as u32).wrapping_shl(16));
        assert_eq!(mo.angle, expected);
    }

    #[test]
    fn friction_drains_existing_momentum() {
        let mut gs = make_game_state();
        // Inject momentum directly (bypassing thrust, since trig tables uninitialized).
        gs.mobjslab.get_mut(gs.player.handle).unwrap().momx = Fixed16_16::from_int(4);

        gs.tick(TicCmd::default(), None);

        let mo = gs.mobjslab.get(gs.player.handle).unwrap();
        // After FRICTION (≈ 0.906): 4 → ~3.625. Must be < 4 and > 0.
        assert!(
            mo.momx < Fixed16_16::from_int(4),
            "friction must reduce momx"
        );
        assert!(
            mo.momx > Fixed16_16::ZERO,
            "friction must not zero momx in one step"
        );
    }

    #[test]
    fn momentum_carries_position_forward() {
        let mut gs = make_game_state();
        gs.mobjslab.get_mut(gs.player.handle).unwrap().momx = Fixed16_16::from_int(2);

        gs.tick(TicCmd::default(), None);

        let mo = gs.mobjslab.get(gs.player.handle).unwrap();
        // x started at 0, momx = 2 → x = 2 after one tick.
        assert_eq!(mo.x, Fixed16_16::from_int(2));
    }

    #[test]
    fn maxmove_clamps_excessive_velocity() {
        let mut gs = make_game_state();
        // Inject extreme velocity.
        gs.mobjslab.get_mut(gs.player.handle).unwrap().momx = Fixed16_16::from_int(1000);

        gs.tick(TicCmd::default(), None);

        let mo = gs.mobjslab.get(gs.player.handle).unwrap();
        // After friction + clamp: must be ≤ MAXMOVE.
        assert!(
            mo.momx <= MAXMOVE,
            "momx={:?} must be clamped to MAXMOVE={:?}",
            mo.momx,
            MAXMOVE
        );
    }

    #[test]
    fn missing_player_handle_is_noop() {
        // If the player handle is NULL, p_move_player should silently do nothing.
        let mut gs = GameState::new("test");
        // player.handle defaults to MobjHandle::NULL — no panic expected.
        gs.tick(
            TicCmd {
                forward_move: 50,
                ..Default::default()
            },
            None,
        );
        assert_eq!(gs.tic_num, 1);
    }

    #[test]
    fn ticcmd_size_is_8_bytes() {
        assert_eq!(std::mem::size_of::<TicCmd>(), 8);
    }

    #[test]
    fn bt_change_switches_weapon_when_owned() {
        use crate::player::WeaponType;
        let mut gs = make_game_state();
        // Give player the shotgun.
        gs.player.weapons[WeaponType::Shotgun as usize] = true;
        // BT_CHANGE | (weapon_num=2 << 3) = 0x04 | 0x10 = 0x14
        let cmd = TicCmd {
            buttons: bt::BT_CHANGE | (2u8 << 3),
            ..Default::default()
        };
        gs.tick(cmd, None);
        assert_eq!(
            gs.player.weapon,
            WeaponType::Shotgun,
            "Should switch to Shotgun when owned"
        );
    }

    #[test]
    fn bt_change_ignores_unowned_weapon() {
        use crate::player::WeaponType;
        let mut gs = make_game_state();
        // Confirm player does NOT have the shotgun.
        gs.player.weapons[WeaponType::Shotgun as usize] = false;
        let original_weapon = gs.player.weapon;

        // Attempt to switch to shotgun (weapon_num=2).
        let cmd = TicCmd {
            buttons: bt::BT_CHANGE | (2u8 << 3),
            ..Default::default()
        };
        gs.tick(cmd, None);

        assert_eq!(
            gs.player.weapon, original_weapon,
            "Should not switch to unowned weapon"
        );
    }
}
