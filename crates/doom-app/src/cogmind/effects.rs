//! Particle effect layer for combat debris, projectile trails, and ambient dust.
//!
//! Effects render as Phase 2.5 (after entities, before sight line). The
//! `EffectLayer` lives on `CogmindState` and persists across frames.

use std::collections::HashSet;

use doom_game::mobj::MobjHandle;
use doom_game::{GameState, MobjKind};

use super::glyphs::Rgb;
use super::lighting::cosm_rand;
use super::tile_grid::CELL_SIZE;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const MAX_EFFECTS: usize = 128;
const MAX_DUST: usize = 15;

// ---------------------------------------------------------------------------
// Effect
// ---------------------------------------------------------------------------

/// A single cosmetic particle (debris, trail, dust).
pub(crate) struct Effect {
    /// Map-unit X position.
    pub x: i32,
    /// Map-unit Y position.
    pub y: i32,
    /// Display character.
    pub glyph: char,
    /// Base foreground color.
    pub fg: Rgb,
    /// Tics remaining before this effect expires.
    pub lifetime: u8,
    /// Original lifetime, used for fade calculation.
    pub max_lifetime: u8,
    /// When `true`, dim `fg` proportional to remaining life.
    pub fade: bool,
    /// Whether this particle counts toward the dust cap.
    pub is_dust: bool,
}

impl Effect {
    /// Return the current foreground color, applying fade if enabled.
    ///
    /// When `fade` is `true`, each channel is scaled by `lifetime / max_lifetime`.
    /// At full lifetime the color is unchanged; at 0 it would be black.
    #[must_use]
    pub(crate) fn current_fg(&self) -> Rgb {
        if !self.fade || self.max_lifetime == 0 {
            return self.fg;
        }
        let scale = u16::from(self.lifetime) * 255 / u16::from(self.max_lifetime);
        (
            (u16::from(self.fg.0) * scale / 255) as u8,
            (u16::from(self.fg.1) * scale / 255) as u8,
            (u16::from(self.fg.2) * scale / 255) as u8,
        )
    }
}

// ---------------------------------------------------------------------------
// EffectLayer
// ---------------------------------------------------------------------------

/// Manages the pool of active particle effects.
pub(crate) struct EffectLayer {
    /// Active effects (newest at end).
    pub effects: Vec<Effect>,
    /// Countdown timer throttling ambient dust spawns.
    dust_timer: u8,
    /// Cosmetic PRNG state.
    rng: u32,
    /// Handles already processed for combat debris to avoid duplicate spawns.
    spawned_puffs: HashSet<MobjHandle>,
}

impl EffectLayer {
    /// Create an empty effect layer.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            effects: Vec::new(),
            dust_timer: 0,
            rng: 0xDEAD_BEEF,
            spawned_puffs: HashSet::new(),
        }
    }

    /// Advance the cosmetic PRNG and return the new state.
    fn rand(&mut self) -> u32 {
        self.rng = cosm_rand(self.rng);
        self.rng
    }

    /// Tick all effects: decrement lifetimes and remove expired ones.
    pub(crate) fn tick(&mut self) {
        for eff in &mut self.effects {
            eff.lifetime = eff.lifetime.saturating_sub(1);
        }
        self.effects.retain(|e| e.lifetime > 0);

        if self.dust_timer > 0 {
            self.dust_timer -= 1;
        }
    }

    /// Push a new effect, evicting the oldest if at capacity.
    pub(crate) fn push(&mut self, effect: Effect) {
        if self.effects.len() >= MAX_EFFECTS {
            self.effects.remove(0);
        }
        self.effects.push(effect);
    }

    /// Spawn 1-2 debris particles for each new `BulletPuff` or `Blood` mobj.
    pub(crate) fn spawn_combat_debris(&mut self, gs: &GameState) {
        for handle in gs.mobjslab.iter_handles() {
            if self.spawned_puffs.contains(&handle) {
                continue;
            }
            let Some(mobj) = gs.mobjslab.get(handle) else {
                continue;
            };
            let (glyph, fg) = match mobj.kind {
                MobjKind::BulletPuff => ('\'', (180_u8, 160, 80)),
                MobjKind::Blood => ('.', (160_u8, 30, 30)),
                _ => continue,
            };

            self.spawned_puffs.insert(handle);

            let base_x = mobj.x.to_int();
            let base_y = mobj.y.to_int();

            // Spawn 1-2 particles at adjacent cells.
            let count = (self.rand() % 2 + 1) as u8; // 1 or 2
            for _ in 0..count {
                let dx = (self.rand() % 3) as i32 - 1; // -1, 0, or 1
                let dy = (self.rand() % 3) as i32 - 1;
                let lifetime = (self.rand() % 3 + 3) as u8; // 3-5
                self.push(Effect {
                    x: base_x + dx * CELL_SIZE,
                    y: base_y + dy * CELL_SIZE,
                    glyph,
                    fg,
                    lifetime,
                    max_lifetime: lifetime,
                    fade: true,
                    is_dust: false,
                });
            }
        }
    }

    /// Spawn trail particles behind each active projectile.
    pub(crate) fn spawn_projectile_trails(&mut self, gs: &GameState) {
        for handle in gs.mobjslab.iter_handles() {
            let Some(mobj) = gs.mobjslab.get(handle) else {
                continue;
            };
            let base_color: Option<Rgb> = match mobj.kind {
                MobjKind::Rocket => Some((127, 80, 0)),
                MobjKind::PlasmaBall => Some((40, 40, 127)),
                MobjKind::BfgBall => Some((0, 127, 0)),
                MobjKind::ArachPlaz => Some((0, 100, 0)),
                MobjKind::Tracer => Some((127, 50, 0)),
                MobjKind::ImpFireball => Some((100, 50, 20)),
                MobjKind::CacoFireball => Some((100, 0, 100)),
                MobjKind::BaronBall => Some((0, 100, 0)),
                MobjKind::FatShot => Some((127, 40, 0)),
                MobjKind::BossCube => Some((100, 0, 0)),
                _ => None,
            };
            let Some(color) = base_color else {
                continue;
            };

            // 50% brightness trail.
            let fg = (color.0 / 2, color.1 / 2, color.2 / 2);
            let lifetime = (self.rand() % 3 + 4) as u8; // 4-6

            self.push(Effect {
                x: mobj.x.to_int(),
                y: mobj.y.to_int(),
                glyph: '\u{00B7}', // middle dot
                fg,
                lifetime,
                max_lifetime: lifetime,
                fade: true,
                is_dust: false,
            });
        }
    }

    /// Spawn ambient dust on random visible floor positions, throttled.
    pub(crate) fn spawn_ambient_dust(&mut self, visible_floors: &[(i32, i32)]) {
        if self.dust_timer > 0 || visible_floors.is_empty() {
            return;
        }

        // Reset throttle timer: 8-12 tics.
        self.dust_timer = (self.rand() % 5 + 8) as u8;

        let current_dust = self.effects.iter().filter(|e| e.is_dust).count();
        if current_dust >= MAX_DUST {
            return;
        }

        let count = (self.rand() % 2 + 1) as u8; // 1 or 2
        for _ in 0..count {
            let dust_count = self.effects.iter().filter(|e| e.is_dust).count();
            if dust_count >= MAX_DUST {
                break;
            }

            let idx = self.rand() as usize % visible_floors.len();
            let (fx, fy) = visible_floors[idx];

            let glyph = if self.rand().is_multiple_of(2) {
                '\u{00B7}' // middle dot
            } else {
                '\''
            };
            let lifetime = (self.rand() % 5 + 6) as u8; // 6-10

            self.push(Effect {
                x: fx,
                y: fy,
                glyph,
                fg: (40, 40, 45),
                lifetime,
                max_lifetime: lifetime,
                fade: true,
                is_dust: true,
            });
        }
    }

    /// Remove stale handles from `spawned_puffs` that no longer exist in the slab.
    pub(crate) fn clean_stale_handles(&mut self, gs: &GameState) {
        self.spawned_puffs
            .retain(|handle| gs.mobjslab.get(*handle).is_some());
    }
}

impl Default for EffectLayer {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- Effect::current_fg --

    #[test]
    fn effect_fade_full_life() {
        let e = Effect {
            x: 0,
            y: 0,
            glyph: '.',
            fg: (200, 100, 50),
            lifetime: 10,
            max_lifetime: 10,
            fade: true,
            is_dust: false,
        };
        // At full lifetime the fg should be unchanged.
        assert_eq!(e.current_fg(), (200, 100, 50));
    }

    #[test]
    fn effect_fade_half_life() {
        let e = Effect {
            x: 0,
            y: 0,
            glyph: '.',
            fg: (200, 100, 50),
            lifetime: 5,
            max_lifetime: 10,
            fade: true,
            is_dust: false,
        };
        let fg = e.current_fg();
        // scale = 5*255/10 = 127
        // r = 200*127/255 = 99, g = 100*127/255 = 49, b = 50*127/255 = 24
        assert!(fg.0 >= 95 && fg.0 <= 105, "red {} not ~100", fg.0);
        assert!(fg.1 >= 45 && fg.1 <= 55, "green {} not ~50", fg.1);
        assert!(fg.2 >= 20 && fg.2 <= 30, "blue {} not ~25", fg.2);
    }

    #[test]
    fn effect_no_fade() {
        let e = Effect {
            x: 0,
            y: 0,
            glyph: '.',
            fg: (200, 100, 50),
            lifetime: 1,
            max_lifetime: 10,
            fade: false,
            is_dust: false,
        };
        // With fade disabled, fg should be unchanged regardless of lifetime.
        assert_eq!(e.current_fg(), (200, 100, 50));
    }

    // -- EffectLayer::tick --

    #[test]
    fn tick_decrements_and_removes() {
        let mut layer = EffectLayer::new();
        layer.push(Effect {
            x: 0,
            y: 0,
            glyph: '.',
            fg: (255, 0, 0),
            lifetime: 1,
            max_lifetime: 1,
            fade: false,
            is_dust: false,
        });
        layer.push(Effect {
            x: 10,
            y: 10,
            glyph: '*',
            fg: (0, 255, 0),
            lifetime: 3,
            max_lifetime: 3,
            fade: false,
            is_dust: false,
        });

        assert_eq!(layer.effects.len(), 2);

        // First tick: lifetime 1 -> 0 (removed), lifetime 3 -> 2.
        layer.tick();
        assert_eq!(layer.effects.len(), 1);
        assert_eq!(layer.effects[0].glyph, '*');
        assert_eq!(layer.effects[0].lifetime, 2);
    }

    // -- EffectLayer::push eviction --

    #[test]
    fn push_evicts_oldest_at_capacity() {
        let mut layer = EffectLayer::new();

        // Fill to MAX_EFFECTS.
        for i in 0..MAX_EFFECTS {
            layer.push(Effect {
                x: i as i32,
                y: 0,
                glyph: '.',
                fg: (0, 0, 0),
                lifetime: 10,
                max_lifetime: 10,
                fade: false,
                is_dust: false,
            });
        }
        assert_eq!(layer.effects.len(), MAX_EFFECTS);
        assert_eq!(layer.effects[0].x, 0); // oldest

        // Push one more.
        layer.push(Effect {
            x: 999,
            y: 0,
            glyph: '!',
            fg: (255, 255, 255),
            lifetime: 10,
            max_lifetime: 10,
            fade: false,
            is_dust: false,
        });

        assert_eq!(layer.effects.len(), MAX_EFFECTS);
        // Oldest (x=0) should be evicted; first element is now x=1.
        assert_eq!(layer.effects[0].x, 1);
        // Newest is at the end.
        assert_eq!(layer.effects[MAX_EFFECTS - 1].x, 999);
    }

    // -- Dust throttling --

    #[test]
    fn dust_throttled_by_timer() {
        let mut layer = EffectLayer::new();
        let floors = vec![(100, 100), (200, 200), (300, 300)];

        // First call should spawn dust and set timer.
        layer.spawn_ambient_dust(&floors);
        let count_after_first = layer.effects.len();
        assert!(count_after_first > 0, "first call should spawn dust");

        // Immediate second call should NOT spawn more (timer > 0).
        layer.spawn_ambient_dust(&floors);
        assert_eq!(
            layer.effects.len(),
            count_after_first,
            "second call should not spawn while timer active"
        );
    }

    #[test]
    fn dust_respects_cap() {
        let mut layer = EffectLayer::new();

        // Pre-fill with MAX_DUST dust particles.
        for i in 0..MAX_DUST {
            layer.push(Effect {
                x: i as i32,
                y: 0,
                glyph: '.',
                fg: (40, 40, 45),
                lifetime: 20,
                max_lifetime: 20,
                fade: true,
                is_dust: true,
            });
        }

        let floors = vec![(100, 100), (200, 200)];
        layer.spawn_ambient_dust(&floors);

        let dust_count = layer.effects.iter().filter(|e| e.is_dust).count();
        assert!(
            dust_count <= MAX_DUST,
            "dust count {dust_count} exceeded cap {MAX_DUST}"
        );
    }

    // -- Constructor --

    #[test]
    fn effect_layer_new_is_empty() {
        let layer = EffectLayer::new();
        assert!(layer.effects.is_empty());
        assert_eq!(layer.dust_timer, 0);
        assert_eq!(layer.rng, 0xDEAD_BEEF);
        assert!(layer.spawned_puffs.is_empty());
    }
}
