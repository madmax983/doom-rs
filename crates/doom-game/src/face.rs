//! Mugshot face animation FSM (ST_updateFaceWidget in vanilla Doom).
//!
//! Tracks which face expression to show in the status bar based on player
//! state.  Expressions are prioritized: Dead > God > Ouch > Pain > EvilGrin >
//! Rampage > Normal.
//!
//! Call `tick_face` once per tic after resolving damage.  Call `face_patch_name`
//! to get the WAD lump name to render.

use doom_types::angle::Bam;

// ---------------------------------------------------------------------------
// Face direction
// ---------------------------------------------------------------------------

/// The direction the face is looking (for idle/damage expressions).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaceDir {
    /// Looking straight ahead.
    Forward,
    /// Turned left.
    Left,
    /// Turned right.
    Right,
}

// ---------------------------------------------------------------------------
// FaceKind
// ---------------------------------------------------------------------------

/// Which mugshot expression is currently displayed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaceKind {
    /// Normal idle / damage-turn.  `tier` is 0 (80-100% HP) through 4 (0-19%).
    Normal { tier: u8, dir: FaceDir },
    /// Took any damage (< 20 HP in one hit).
    Pain { tier: u8 },
    /// Took 20+ HP damage in one tic — big ouch face.
    Ouch { tier: u8 },
    /// Just picked up a new weapon — evil grin.
    EvilGrin,
    /// Firing continuously (rampage).
    Rampage { tier: u8 },
    /// Invulnerability active — gold eyes.
    GodMode,
    /// Dead (health ≤ 0).
    Dead,
    /// Gibbed (extreme damage death).
    XDead,
}

// ---------------------------------------------------------------------------
// Priority
// ---------------------------------------------------------------------------

fn priority(kind: FaceKind) -> u8 {
    match kind {
        FaceKind::XDead | FaceKind::Dead => 8,
        FaceKind::GodMode => 7,
        FaceKind::Ouch { .. } => 6,
        FaceKind::Pain { .. } => 5,
        FaceKind::EvilGrin => 4,
        FaceKind::Rampage { .. } => 3,
        FaceKind::Normal { .. } => 0,
    }
}

// ---------------------------------------------------------------------------
// FaceState
// ---------------------------------------------------------------------------

/// Per-player face animation state.
#[derive(Clone, Debug)]
pub struct FaceState {
    /// Currently displayed expression.
    pub kind: FaceKind,
    /// Tics remaining for the current expression (0 = may be replaced).
    pub display_tics: u32,
    /// Running count of consecutive tics the player has been firing.
    pub firing_tics: u32,
    /// How many tics until we do the next random idle glance.
    pub idle_countdown: u32,
    /// Angle of the attacker at the last damage event (for turn direction).
    pub last_attack_angle: Bam,
    /// Was the player firing last tic?
    pub was_firing: bool,
    /// Flag set for one tic when a new weapon is picked up.
    pub new_weapon_flag: bool,
    /// Damage received this tic (set by damage_mobj, cleared each tic).
    pub damage_this_tic: i32,
    /// Whether the player was dead last tic (to avoid re-triggering).
    last_was_dead: bool,
}

impl FaceState {
    /// Create initial face state (alive, tier 0, facing forward).
    pub fn new() -> Self {
        Self {
            kind: FaceKind::Normal {
                tier: 0,
                dir: FaceDir::Forward,
            },
            display_tics: 0,
            firing_tics: 0,
            idle_countdown: IDLE_GLANCE_INTERVAL,
            last_attack_angle: Bam(0),
            was_firing: false,
            new_weapon_flag: false,
            damage_this_tic: 0,
            last_was_dead: false,
        }
    }

    /// Advance the face FSM by one tic.
    ///
    /// `health` — current player HP (can be ≤ 0 when dead).
    /// `is_firing` — true if the player fired a weapon this tic.
    /// `is_invulnerable` — true if the invulnerability sphere is active.
    /// `attacker_angle` — angle from player to last attacker (for turn dir).
    /// `player_angle` — player's facing angle (for turn dir calculation).
    pub fn tick(
        &mut self,
        health: i32,
        is_firing: bool,
        is_invulnerable: bool,
        attacker_angle: Option<Bam>,
        player_angle: Bam,
    ) {
        let tier = health_tier(health);

        // --- Firing streak counter ---
        if is_firing {
            self.firing_tics = self.firing_tics.saturating_add(1);
        } else {
            self.firing_tics = 0;
        }
        self.was_firing = is_firing;

        // --- Idle glance countdown ---
        if self.idle_countdown > 0 {
            self.idle_countdown -= 1;
        }

        // --- Tick down current expression ---
        if self.display_tics > 0 {
            self.display_tics -= 1;
        }

        // --- Priority cascade (highest wins) ---

        // 1. Dead / gibbed
        if health <= 0 {
            if !self.last_was_dead {
                self.kind = FaceKind::Dead;
                self.display_tics = 0; // hold until game reset
            }
            self.last_was_dead = true;
            self.damage_this_tic = 0;
            return;
        }
        self.last_was_dead = false;

        // 2. God mode
        if is_invulnerable {
            self.set_face(FaceKind::GodMode, GOD_HOLD_TICS);
            self.damage_this_tic = 0;
            return;
        }

        // 3. Ouch (20+ damage in one tic)
        if self.damage_this_tic >= 20 {
            let dir = damage_dir(attacker_angle, player_angle);
            let kind = FaceKind::Ouch { tier };
            // Ouch is based on direction but uses the pain patches — just
            // store direction in Normal after ouch expires; for now use Ouch.
            self.set_face_if_higher(kind, OUCH_HOLD_TICS);
            // Also remember attacker direction so normal face looks that way
            if let Some(angle) = attacker_angle {
                self.last_attack_angle = angle;
            }
            self.damage_this_tic = 0;
            return;
        }

        // 4. Pain (any damage)
        if self.damage_this_tic > 0 {
            self.set_face_if_higher(FaceKind::Pain { tier }, PAIN_HOLD_TICS);
            // Store direction so normal face uses it after pain expires.
            if let Some(angle) = attacker_angle {
                self.last_attack_angle = angle;
            }
            self.damage_this_tic = 0;
            return;
        }

        self.damage_this_tic = 0;

        // 5. Evil grin (new weapon)
        if self.new_weapon_flag {
            self.new_weapon_flag = false;
            self.set_face_if_higher(FaceKind::EvilGrin, GRIN_HOLD_TICS);
            return;
        }

        // 6. Rampage (firing 2+ consecutive tics)
        if self.firing_tics >= RAMPAGE_THRESHOLD {
            self.set_face_if_higher(FaceKind::Rampage { tier }, RAMPAGE_HOLD_TICS);
            return;
        }

        // 7. Normal — if current expression expired, go back to idle
        if self.display_tics == 0 {
            let dir = if self.idle_countdown == 0 {
                // Time for a random idle glance.
                self.idle_countdown = IDLE_GLANCE_INTERVAL;
                // Alternate L/R based on a simple counter.
                static GLANCE_TOGGLE: std::sync::atomic::AtomicBool =
                    std::sync::atomic::AtomicBool::new(false);
                let toggle = GLANCE_TOGGLE.fetch_xor(true, std::sync::atomic::Ordering::Relaxed);
                if toggle {
                    FaceDir::Right
                } else {
                    FaceDir::Left
                }
            } else {
                FaceDir::Forward
            };
            self.kind = FaceKind::Normal { tier, dir };
        } else if let FaceKind::Normal { dir, .. } = self.kind {
            // Update tier while keeping direction.
            self.kind = FaceKind::Normal { tier, dir };
        }
    }

    fn set_face(&mut self, kind: FaceKind, hold_tics: u32) {
        self.kind = kind;
        self.display_tics = hold_tics;
    }

    fn set_face_if_higher(&mut self, kind: FaceKind, hold_tics: u32) {
        if self.display_tics == 0 || priority(kind) >= priority(self.kind) {
            self.set_face(kind, hold_tics);
        }
    }

    /// Signal that the player picked up a new weapon (sets the grin flag).
    pub fn on_new_weapon(&mut self) {
        self.new_weapon_flag = true;
    }

    /// Signal that the player took `amount` damage from direction `angle`.
    pub fn on_damage(&mut self, amount: i32, angle: Bam) {
        self.damage_this_tic += amount;
        self.last_attack_angle = angle;
    }

    /// Reset to fresh state (new level / respawn).
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for FaceState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Map health to tier: 0 (80-100%) … 4 (0-19%).
pub fn health_tier(health: i32) -> u8 {
    if health > 80 {
        0
    } else if health > 60 {
        1
    } else if health > 40 {
        2
    } else if health > 20 {
        3
    } else {
        4
    }
}

/// Determine face direction based on attacker angle relative to player facing.
fn damage_dir(attacker_angle: Option<Bam>, player_angle: Bam) -> FaceDir {
    let Some(att) = attacker_angle else {
        return FaceDir::Forward;
    };
    // Difference: positive = attacker is to the left of player's facing.
    let diff = att.0.wrapping_sub(player_angle.0);
    if diff < 0x4000_0000 || diff > 0xC000_0000 {
        FaceDir::Forward
    } else if diff < 0x8000_0000 {
        FaceDir::Right
    } else {
        FaceDir::Left
    }
}

// ---------------------------------------------------------------------------
// Hold durations (tics at 35 Hz)
// ---------------------------------------------------------------------------

const OUCH_HOLD_TICS: u32 = 35;
const PAIN_HOLD_TICS: u32 = 35;
const GRIN_HOLD_TICS: u32 = 35;
const GOD_HOLD_TICS: u32 = 1; // re-evaluated every tic while active
const RAMPAGE_HOLD_TICS: u32 = 2;
const RAMPAGE_THRESHOLD: u32 = 2;
const IDLE_GLANCE_INTERVAL: u32 = 105;

// ---------------------------------------------------------------------------
// Patch name resolution
// ---------------------------------------------------------------------------

/// Return the WAD lump name for the given face expression.
///
/// Names match vanilla Doom's `ST_lib.c` face patch naming scheme.
pub fn face_patch_name(kind: FaceKind) -> String {
    match kind {
        FaceKind::Normal { tier, dir } => {
            let t = tier.min(4);
            match dir {
                FaceDir::Forward => format!("STFST{t}0"),
                FaceDir::Left => format!("STFTL{t}0"),
                FaceDir::Right => format!("STFTR{t}0"),
            }
        }
        FaceKind::Pain { tier } => {
            // Pain uses the same ST_F_xxx00 scheme but with the ouch variant
            // in vanilla; we use the damage-direction approach: straight.
            let t = tier.min(4);
            format!("STFST{t}0")
        }
        FaceKind::Ouch { tier } => {
            let t = tier.min(4);
            format!("STFOUCH{t}")
        }
        FaceKind::EvilGrin => "STFEVL0".to_string(),
        FaceKind::Rampage { tier } => {
            let t = tier.min(4);
            format!("STFKLL{t}0")
        }
        FaceKind::GodMode => "STFGOD0".to_string(),
        FaceKind::Dead => "STFDEAD0".to_string(),
        FaceKind::XDead => "STFXDTH1".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_face() -> FaceState {
        FaceState::new()
    }

    fn tick_simple(face: &mut FaceState, health: i32) {
        face.tick(health, false, false, None, Bam(0));
    }

    #[test]
    fn initial_state_is_normal_forward_tier0() {
        let face = make_face();
        assert_eq!(
            face.kind,
            FaceKind::Normal {
                tier: 0,
                dir: FaceDir::Forward
            }
        );
    }

    #[test]
    fn death_takes_highest_priority() {
        let mut face = make_face();
        // Simulate god mode + death simultaneously — dead wins.
        face.tick(0, false, true, None, Bam(0));
        assert_eq!(face.kind, FaceKind::Dead);
    }

    #[test]
    fn god_mode_overrides_normal() {
        let mut face = make_face();
        face.tick(100, false, true, None, Bam(0));
        assert_eq!(face.kind, FaceKind::GodMode);
    }

    #[test]
    fn ouch_on_20_damage() {
        let mut face = make_face();
        face.on_damage(20, Bam(0));
        face.tick(80, false, false, Some(Bam(0)), Bam(0));
        assert!(matches!(face.kind, FaceKind::Ouch { .. }));
    }

    #[test]
    fn pain_on_small_damage() {
        let mut face = make_face();
        face.on_damage(5, Bam(0));
        face.tick(95, false, false, Some(Bam(0)), Bam(0));
        assert!(matches!(face.kind, FaceKind::Pain { .. }));
    }

    #[test]
    fn evil_grin_on_new_weapon() {
        let mut face = make_face();
        face.on_new_weapon();
        tick_simple(&mut face, 100);
        assert_eq!(face.kind, FaceKind::EvilGrin);
    }

    #[test]
    fn rampage_after_sustained_firing() {
        let mut face = make_face();
        // Fire for RAMPAGE_THRESHOLD tics.
        for _ in 0..RAMPAGE_THRESHOLD {
            face.tick(100, true, false, None, Bam(0));
        }
        assert!(matches!(face.kind, FaceKind::Rampage { .. }));
    }

    #[test]
    fn dead_face_holds() {
        let mut face = make_face();
        tick_simple(&mut face, 0);
        assert_eq!(face.kind, FaceKind::Dead);
        // More ticks at 0 health: stays dead.
        tick_simple(&mut face, 0);
        assert_eq!(face.kind, FaceKind::Dead);
    }

    // -----------------------------------------------------------------------
    // health_tier
    // -----------------------------------------------------------------------

    #[test]
    fn health_tier_mapping() {
        assert_eq!(health_tier(100), 0);
        assert_eq!(health_tier(81), 0);
        assert_eq!(health_tier(80), 1);
        assert_eq!(health_tier(61), 1);
        assert_eq!(health_tier(60), 2);
        assert_eq!(health_tier(41), 2);
        assert_eq!(health_tier(40), 3);
        assert_eq!(health_tier(21), 3);
        assert_eq!(health_tier(20), 4);
        assert_eq!(health_tier(1), 4);
        assert_eq!(health_tier(0), 4);
    }

    // -----------------------------------------------------------------------
    // face_patch_name
    // -----------------------------------------------------------------------

    #[test]
    fn patch_names_match_vanilla() {
        assert_eq!(
            face_patch_name(FaceKind::Normal {
                tier: 0,
                dir: FaceDir::Forward
            }),
            "STFST00"
        );
        assert_eq!(
            face_patch_name(FaceKind::Normal {
                tier: 2,
                dir: FaceDir::Left
            }),
            "STFTL20"
        );
        assert_eq!(
            face_patch_name(FaceKind::Normal {
                tier: 4,
                dir: FaceDir::Right
            }),
            "STFTR40"
        );
        assert_eq!(face_patch_name(FaceKind::Ouch { tier: 3 }), "STFOUCH3");
        assert_eq!(face_patch_name(FaceKind::EvilGrin), "STFEVL0");
        assert_eq!(face_patch_name(FaceKind::Rampage { tier: 1 }), "STFKLL10");
        assert_eq!(face_patch_name(FaceKind::GodMode), "STFGOD0");
        assert_eq!(face_patch_name(FaceKind::Dead), "STFDEAD0");
        assert_eq!(face_patch_name(FaceKind::XDead), "STFXDTH1");
    }
}
