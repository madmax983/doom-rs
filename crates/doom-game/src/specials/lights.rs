#![allow(unused_imports)]
#![allow(dead_code)]
use doom_map::{Level, SIDEDEF_NONE};
use doom_types::{FIXED_ONE, Fixed16_16};

use super::*;
use crate::mobj::MobjHandle;
use crate::movers::*;
use crate::state::*;

// init_sector_lights / tick_sector_lights

/// Scan all sectors and create `SectorLightEffect` entries for extended
/// light-related sector specials.
///
/// Handles sector specials 1, 2, 3, 8, 12, 13, and 17.
/// Call this once after loading a level, before the first tic.
pub fn init_sector_lights(gs: &mut GameState, level: &Level) {
    for (i, sector) in level.sectors.iter().enumerate() {
        let Some(effect_type) = LightEffectType::from_repr(sector.special) else {
            continue;
        };

        let min_light = match effect_type {
            LightEffectType::BlinkRandom => 0,
            LightEffectType::Blink05s => 0,
            LightEffectType::Blink1s => 35,
            LightEffectType::Oscillate => sector.light_level / 2,
            LightEffectType::BlinkSync05s => 0,
            LightEffectType::BlinkSync1s => 35,
            LightEffectType::FireFlicker => sector.light_level.saturating_sub(16).max(0),
        };

        let timer = match effect_type {
            LightEffectType::BlinkRandom => BLINK_SLOW_PERIOD as u32,
            LightEffectType::Blink05s => BLINK_FAST_PERIOD as u32,
            LightEffectType::Blink1s => BLINK_SLOW_PERIOD as u32,
            LightEffectType::Oscillate => 1,
            LightEffectType::BlinkSync05s => BLINK_FAST_PERIOD as u32,
            LightEffectType::BlinkSync1s => BLINK_SLOW_PERIOD as u32,
            LightEffectType::FireFlicker => 4,
        };

        gs.movers.sector_lights.push(SectorLightEffect {
            sector_index: i,
            effect_type,
            base_light: sector.light_level,
            min_light,
            timer,
        });
    }
}

/// Advance all extended sector light effects by one tic.
///
/// Call this once per tic from `tick()`.
pub fn tick_sector_lights(gs: &mut GameState, level: &mut Level) {
    for effect in &mut gs.movers.sector_lights {
        if effect.sector_index >= level.sectors.len() {
            continue;
        }

        effect.timer = effect.timer.saturating_sub(1);
        if effect.timer > 0 {
            continue;
        }

        let sector = &mut level.sectors[effect.sector_index];

        match effect.effect_type {
            LightEffectType::BlinkRandom => {
                // Toggle between base and dark at random-ish intervals.
                if sector.light_level == effect.base_light {
                    sector.light_level = effect.min_light;
                    // Use a simple deterministic variation for the next period.
                    effect.timer = (BLINK_SLOW_PERIOD as u32)
                        .wrapping_add(effect.sector_index as u32 * 7)
                        % 40
                        + 10;
                } else {
                    sector.light_level = effect.base_light;
                    effect.timer = BLINK_SLOW_PERIOD as u32;
                }
            }
            LightEffectType::Blink05s | LightEffectType::BlinkSync05s => {
                if sector.light_level == effect.base_light {
                    sector.light_level = effect.min_light;
                } else {
                    sector.light_level = effect.base_light;
                }
                effect.timer = BLINK_FAST_PERIOD as u32;
            }
            LightEffectType::Blink1s | LightEffectType::BlinkSync1s => {
                if sector.light_level == effect.base_light {
                    sector.light_level = effect.min_light;
                } else {
                    sector.light_level = effect.base_light;
                }
                effect.timer = BLINK_SLOW_PERIOD as u32;
            }
            LightEffectType::Oscillate => {
                // Smooth oscillation: ramp light up and down.
                let range = effect.base_light - effect.min_light;
                if range <= 0 {
                    effect.timer = 1;
                    continue;
                }
                // Use level_time to create a smooth oscillation.
                let phase = (gs.stats.level_time % (range as u32 * 2)) as i16;
                sector.light_level = if phase < range {
                    effect.min_light + phase
                } else {
                    effect.base_light - (phase - range)
                };
                effect.timer = 1;
            }
            LightEffectType::FireFlicker => {
                // Random light variation within a small range.
                let variation = (gs.rng.next_byte() & 3) as i16;
                sector.light_level = (effect.base_light - variation * 4).max(effect.min_light);
                effect.timer = 4;
            }
        }
    }
}

// tick_lights

/// Advance all light specials by one tic.
pub fn tick_lights(gs: &mut GameState, level: &mut Level) {
    for light in &mut gs.movers.active_lights {
        light.timer -= 1;
        if light.timer <= 0 {
            light.is_bright = !light.is_bright;
            light.timer = light.period;
            if light.sector < level.sectors.len() {
                level.sectors[light.sector].light_level = if light.is_bright {
                    light.bright
                } else {
                    light.dark
                };
            }
        }
    }
}
