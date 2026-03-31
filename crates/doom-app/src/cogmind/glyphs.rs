//! Glyph table and tile-type definitions for cogmind-mode rendering.
//!
//! Defines the visual vocabulary: which characters and colors represent each
//! map element and entity in the top-down ASCII view.

use doom_game::MobjKind;

// ---------------------------------------------------------------------------
// Tile types
// ---------------------------------------------------------------------------

/// Classification of a map tile for rendering purposes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TileKind {
    Floor,
    Wall,
    DoorClosed,
    DoorOpen,
    HeightChange,
    Void,
    Nukage,
    Lava,
}

// ---------------------------------------------------------------------------
// Glyph representation
// ---------------------------------------------------------------------------

/// An RGB color triple.
pub type Rgb = (u8, u8, u8);

/// A single tile's visual representation: character + foreground/background.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TileGlyph {
    pub(crate) glyph: char,
    pub(crate) fg: Rgb,
    pub(crate) bg: Rgb,
}

// ---------------------------------------------------------------------------
// Tile glyphs
// ---------------------------------------------------------------------------

/// Return the base glyph for a map tile.
#[must_use]
pub(crate) fn tile_glyph(kind: TileKind) -> TileGlyph {
    match kind {
        TileKind::Floor | TileKind::DoorOpen => TileGlyph {
            glyph: '\u{00B7}', // ·
            fg: (60, 60, 60),
            bg: (0, 0, 0),
        },
        TileKind::Wall => TileGlyph {
            glyph: '\u{2588}', // █
            fg: (80, 80, 80),
            bg: (0, 0, 0),
        },
        TileKind::DoorClosed => TileGlyph {
            glyph: '\u{25AF}', // ▯
            fg: (180, 180, 100),
            bg: (0, 0, 0),
        },
        TileKind::HeightChange => TileGlyph {
            glyph: '\u{2500}', // ─
            fg: (100, 100, 60),
            bg: (0, 0, 0),
        },
        TileKind::Void => TileGlyph {
            glyph: ' ',
            fg: (0, 0, 0),
            bg: (0, 0, 0),
        },
        TileKind::Nukage => TileGlyph {
            glyph: '\u{00B7}', // ·
            fg: (0, 100, 0),
            bg: (0, 20, 0),
        },
        TileKind::Lava => TileGlyph {
            glyph: '\u{00B7}', // ·
            fg: (160, 40, 0),
            bg: (40, 0, 0),
        },
    }
}

// ---------------------------------------------------------------------------
// Monster detection
// ---------------------------------------------------------------------------

/// Returns `true` if the given `MobjKind` is a monster (not the player).
#[must_use]
pub(crate) fn is_monster(kind: MobjKind) -> bool {
    matches!(
        kind,
        MobjKind::Trooper
            | MobjKind::Sergeant
            | MobjKind::Imp
            | MobjKind::Demon
            | MobjKind::Spectre
            | MobjKind::LostSoul
            | MobjKind::Cacodemon
            | MobjKind::BaronOfHell
            | MobjKind::HellKnight
            | MobjKind::Arachnotron
            | MobjKind::PainElemental
            | MobjKind::Revenant
            | MobjKind::Mancubus
            | MobjKind::ArchVile
            | MobjKind::SpiderMastermind
            | MobjKind::Cyberdemon
            | MobjKind::WolfSS
            | MobjKind::BossBrain
            | MobjKind::CommanderKeen
    )
}

// ---------------------------------------------------------------------------
// Entity glyphs
// ---------------------------------------------------------------------------

/// Return the glyph for a map entity (mobj).
///
/// Dead monsters (health <= 0) display as corpses (`%`). The player is never
/// treated as a "monster" corpse even when dead.
#[must_use]
pub(crate) fn entity_glyph(kind: MobjKind, health: i32) -> TileGlyph {
    // Dead monster corpse
    if health <= 0 && is_monster(kind) {
        return TileGlyph {
            glyph: '%',
            fg: (100, 20, 20),
            bg: (0, 0, 0),
        };
    }

    match kind {
        // -- Player --
        MobjKind::Player => TileGlyph {
            glyph: '@',
            fg: (255, 255, 255),
            bg: (0, 0, 0),
        },

        // -- Monsters --
        MobjKind::Trooper => monster_glyph('z', (0, 120, 0)),
        MobjKind::Sergeant => monster_glyph('s', (120, 0, 0)),
        MobjKind::Imp => monster_glyph('i', (160, 100, 40)),
        MobjKind::Demon => monster_glyph('D', (200, 100, 120)),
        MobjKind::Spectre => monster_glyph('D', (60, 60, 80)),
        MobjKind::LostSoul => monster_glyph('L', (255, 160, 0)),
        MobjKind::Cacodemon => monster_glyph('C', (200, 0, 0)),
        MobjKind::BaronOfHell => monster_glyph('B', (0, 180, 0)),
        MobjKind::HellKnight => monster_glyph('H', (120, 80, 60)),
        MobjKind::Arachnotron => monster_glyph('A', (160, 160, 0)),
        MobjKind::PainElemental => monster_glyph('P', (180, 80, 0)),
        MobjKind::Revenant => monster_glyph('R', (200, 200, 180)),
        MobjKind::Mancubus => monster_glyph('M', (140, 100, 60)),
        MobjKind::ArchVile => monster_glyph('V', (255, 220, 100)),
        MobjKind::SpiderMastermind => monster_glyph('S', (120, 120, 120)),
        MobjKind::Cyberdemon => monster_glyph('X', (180, 0, 0)),
        MobjKind::WolfSS => monster_glyph('w', (100, 100, 160)),
        MobjKind::BossBrain => monster_glyph('$', (180, 0, 0)),
        MobjKind::CommanderKeen => monster_glyph('K', (0, 160, 0)),

        // -- Projectiles --
        MobjKind::Rocket => projectile_glyph((255, 160, 0)),
        MobjKind::PlasmaBall => projectile_glyph((80, 80, 255)),
        MobjKind::BfgBall => projectile_glyph((0, 255, 0)),
        MobjKind::ArachPlaz => projectile_glyph((0, 200, 0)),
        MobjKind::Tracer => projectile_glyph((255, 100, 0)),
        MobjKind::BfgExtra => projectile_glyph((80, 255, 80)),
        MobjKind::ImpFireball => projectile_glyph((200, 100, 40)),
        MobjKind::CacoFireball => projectile_glyph((200, 0, 200)),
        MobjKind::BaronBall => projectile_glyph((0, 200, 0)),
        MobjKind::FatShot => projectile_glyph((255, 80, 0)),
        MobjKind::BossCube => projectile_glyph((200, 0, 0)),

        // -- Keys --
        MobjKind::BlueCard | MobjKind::BlueSkull => pickup_key((0, 0, 255)),
        MobjKind::RedCard | MobjKind::RedSkull => pickup_key((255, 0, 0)),
        MobjKind::YellowCard | MobjKind::YellowSkull => pickup_key((255, 255, 0)),

        // -- Weapon pickups --
        MobjKind::BfgPickup
        | MobjKind::Chaingun
        | MobjKind::Chainsaw
        | MobjKind::RocketLauncher
        | MobjKind::PlasmaRifle
        | MobjKind::Shotgun
        | MobjKind::SuperShotgun => TileGlyph {
            glyph: ')',
            fg: (255, 255, 0),
            bg: (0, 0, 0),
        },

        // -- Ammo pickups --
        MobjKind::Clip
        | MobjKind::ClipBox
        | MobjKind::RocketAmmo
        | MobjKind::RocketBox
        | MobjKind::Cell
        | MobjKind::CellPack
        | MobjKind::Shell
        | MobjKind::ShellBox => TileGlyph {
            glyph: '|',
            fg: (255, 160, 0),
            bg: (0, 0, 0),
        },

        // -- Health & armor pickups --
        MobjKind::HealthBonus | MobjKind::Stimpack | MobjKind::Medikit | MobjKind::Soulsphere => {
            TileGlyph {
                glyph: '+',
                fg: (80, 80, 255),
                bg: (0, 0, 0),
            }
        }
        MobjKind::ArmorBonus | MobjKind::GreenArmor | MobjKind::BlueArmor => TileGlyph {
            glyph: '[',
            fg: (0, 200, 0),
            bg: (0, 0, 0),
        },
        MobjKind::Megasphere => TileGlyph {
            glyph: '+',
            fg: (80, 80, 255),
            bg: (0, 0, 0),
        },

        // -- Power-ups --
        MobjKind::Berserk
        | MobjKind::BlurSphere
        | MobjKind::RadSuit
        | MobjKind::Allmap
        | MobjKind::Infrared
        | MobjKind::InvulnerabilitySphere => TileGlyph {
            glyph: '!',
            fg: (200, 0, 255),
            bg: (0, 0, 0),
        },

        // -- Backpack (ammo pickup category) --
        MobjKind::Backpack => TileGlyph {
            glyph: '|',
            fg: (255, 160, 0),
            bg: (0, 0, 0),
        },

        // -- Props --
        MobjKind::Barrel => TileGlyph {
            glyph: '0',
            fg: (0, 180, 0),
            bg: (0, 0, 0),
        },
        MobjKind::Column | MobjKind::TechLamp | MobjKind::TechLamp2 => TileGlyph {
            glyph: '|',
            fg: (120, 120, 120),
            bg: (0, 0, 0),
        },

        // -- Visual effects --
        MobjKind::BulletPuff
        | MobjKind::Blood
        | MobjKind::SmokeTrail
        | MobjKind::SpawnFire
        | MobjKind::VileFire => TileGlyph {
            glyph: '.',
            fg: (60, 60, 60),
            bg: (0, 0, 0),
        },
    }
}

// ---------------------------------------------------------------------------
// Wall glyph (box-drawing)
// ---------------------------------------------------------------------------

/// Return a box-drawing character based on a 4-bit neighbor mask.
///
/// Bit layout: 0 = north, 1 = east, 2 = south, 3 = west.
/// A set bit means the neighbor in that direction is also a wall.
#[must_use]
pub(crate) fn wall_glyph(neighbors: u8) -> char {
    match neighbors & 0x0F {
        0b0000 => '\u{2588}', // █  isolated
        0b0001 => '\u{2551}', // ║  dead end north
        0b0010 => '\u{2550}', // ═  dead end east
        0b0011 => '\u{255A}', // ╚  corner NE (north + east)
        0b0100 => '\u{2551}', // ║  dead end south
        0b0101 => '\u{2551}', // ║  vertical (N+S)
        0b0110 => '\u{2554}', // ╔  corner SE (south + east)
        0b0111 => '\u{2560}', // ╠  T-junction (N+E+S)
        0b1000 => '\u{2550}', // ═  dead end west
        0b1001 => '\u{255D}', // ╝  corner NW (north + west)
        0b1010 => '\u{2550}', // ═  horizontal (E+W)
        0b1011 => '\u{2569}', // ╩  T-junction (N+E+W)
        0b1100 => '\u{2557}', // ╗  corner SW (south + west)
        0b1101 => '\u{2563}', // ╣  T-junction (N+S+W)
        0b1110 => '\u{2566}', // ╦  T-junction (E+S+W)
        0b1111 => '\u{256C}', // ╬  cross (all four)
        _ => unreachable!(),
    }
}

// ---------------------------------------------------------------------------
// Lighting helpers
// ---------------------------------------------------------------------------

/// Scale an RGB color by a light level (0..=255).
#[must_use]
pub(crate) fn apply_light(color: Rgb, light: u8) -> Rgb {
    let l = u16::from(light);
    (
        (u16::from(color.0) * l / 255) as u8,
        (u16::from(color.1) * l / 255) as u8,
        (u16::from(color.2) * l / 255) as u8,
    )
}

/// Desaturate and dim a color to ~40% brightness for "remembered" fog-of-war.
#[must_use]
pub(crate) fn dim_remembered(color: Rgb) -> Rgb {
    // Convert to grayscale via luminance weights (approx ITU-R BT.601),
    // then scale to 40%.
    let gray = (u16::from(color.0) * 77 + u16::from(color.1) * 150 + u16::from(color.2) * 29) / 256;
    let dimmed = (gray * 102 / 255) as u8; // 102/255 ≈ 0.40
    (dimmed, dimmed, dimmed)
}

// ---------------------------------------------------------------------------
// Sector classification
// ---------------------------------------------------------------------------

/// Classify a sector's floor type based on its `special` field.
#[must_use]
pub(crate) fn sector_floor_kind(special: u16) -> TileKind {
    match special {
        5 | 7 | 16 => TileKind::Nukage,
        4 | 11 => TileKind::Lava,
        _ => TileKind::Floor,
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn monster_glyph(ch: char, fg: Rgb) -> TileGlyph {
    TileGlyph {
        glyph: ch,
        fg,
        bg: (0, 0, 0),
    }
}

fn projectile_glyph(fg: Rgb) -> TileGlyph {
    TileGlyph {
        glyph: '*',
        fg,
        bg: (0, 0, 0),
    }
}

fn pickup_key(fg: Rgb) -> TileGlyph {
    TileGlyph {
        glyph: 'k',
        fg,
        bg: (0, 0, 0),
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- entity_glyph --

    #[test]
    fn player_glyph_is_at() {
        let g = entity_glyph(MobjKind::Player, 100);
        assert_eq!(g.glyph, '@');
        assert_eq!(g.fg, (255, 255, 255));
    }

    #[test]
    fn dead_monster_is_corpse() {
        let g = entity_glyph(MobjKind::Imp, 0);
        assert_eq!(g.glyph, '%');
        assert_eq!(g.fg, (100, 20, 20));
    }

    #[test]
    fn dead_player_is_not_corpse() {
        // Player is NOT a monster, so dead player should still be '@'.
        let g = entity_glyph(MobjKind::Player, 0);
        assert_eq!(g.glyph, '@');
    }

    #[test]
    fn living_monster_shows_letter() {
        let g = entity_glyph(MobjKind::Cacodemon, 400);
        assert_eq!(g.glyph, 'C');
        assert_eq!(g.fg, (200, 0, 0));
    }

    #[test]
    fn projectile_is_star() {
        let g = entity_glyph(MobjKind::Rocket, 1);
        assert_eq!(g.glyph, '*');
        assert_eq!(g.fg, (255, 160, 0));
    }

    #[test]
    fn key_glyph_colored() {
        let blue = entity_glyph(MobjKind::BlueCard, 1);
        let red = entity_glyph(MobjKind::RedSkull, 1);
        assert_eq!(blue.glyph, 'k');
        assert_eq!(blue.fg, (0, 0, 255));
        assert_eq!(red.glyph, 'k');
        assert_eq!(red.fg, (255, 0, 0));
    }

    #[test]
    fn weapon_pickup_glyph() {
        let g = entity_glyph(MobjKind::Chaingun, 1);
        assert_eq!(g.glyph, ')');
        assert_eq!(g.fg, (255, 255, 0));
    }

    #[test]
    fn health_pickup_glyph() {
        let g = entity_glyph(MobjKind::Medikit, 1);
        assert_eq!(g.glyph, '+');
        assert_eq!(g.fg, (80, 80, 255));
    }

    #[test]
    fn armor_pickup_glyph() {
        let g = entity_glyph(MobjKind::GreenArmor, 1);
        assert_eq!(g.glyph, '[');
        assert_eq!(g.fg, (0, 200, 0));
    }

    #[test]
    fn powerup_glyph() {
        let g = entity_glyph(MobjKind::Berserk, 1);
        assert_eq!(g.glyph, '!');
        assert_eq!(g.fg, (200, 0, 255));
    }

    #[test]
    fn barrel_glyph() {
        let g = entity_glyph(MobjKind::Barrel, 1);
        assert_eq!(g.glyph, '0');
        assert_eq!(g.fg, (0, 180, 0));
    }

    #[test]
    fn effect_glyph() {
        let g = entity_glyph(MobjKind::BulletPuff, 1);
        assert_eq!(g.glyph, '.');
        assert_eq!(g.fg, (60, 60, 60));
    }

    // -- is_monster --

    #[test]
    fn all_monsters_detected() {
        let monsters = [
            MobjKind::Trooper,
            MobjKind::Sergeant,
            MobjKind::Imp,
            MobjKind::Demon,
            MobjKind::Spectre,
            MobjKind::LostSoul,
            MobjKind::Cacodemon,
            MobjKind::BaronOfHell,
            MobjKind::HellKnight,
            MobjKind::Arachnotron,
            MobjKind::PainElemental,
            MobjKind::Revenant,
            MobjKind::Mancubus,
            MobjKind::ArchVile,
            MobjKind::SpiderMastermind,
            MobjKind::Cyberdemon,
            MobjKind::WolfSS,
            MobjKind::BossBrain,
            MobjKind::CommanderKeen,
        ];
        for m in monsters {
            assert!(is_monster(m), "{m:?} should be a monster");
        }
    }

    #[test]
    fn player_is_not_monster() {
        assert!(!is_monster(MobjKind::Player));
    }

    #[test]
    fn projectile_is_not_monster() {
        assert!(!is_monster(MobjKind::Rocket));
    }

    // -- wall_glyph --

    #[test]
    fn wall_glyph_isolated() {
        assert_eq!(wall_glyph(0b0000), '\u{2588}'); // █
    }

    #[test]
    fn wall_glyph_horizontal_run() {
        assert_eq!(wall_glyph(0b1010), '\u{2550}'); // ═
    }

    #[test]
    fn wall_glyph_vertical_run() {
        assert_eq!(wall_glyph(0b0101), '\u{2551}'); // ║
    }

    #[test]
    fn wall_glyph_cross() {
        assert_eq!(wall_glyph(0b1111), '\u{256C}'); // ╬
    }

    #[test]
    fn wall_glyph_corners() {
        assert_eq!(wall_glyph(0b0011), '\u{255A}'); // ╚  NE
        assert_eq!(wall_glyph(0b0110), '\u{2554}'); // ╔  SE
        assert_eq!(wall_glyph(0b1100), '\u{2557}'); // ╗  SW
        assert_eq!(wall_glyph(0b1001), '\u{255D}'); // ╝  NW
    }

    #[test]
    fn wall_glyph_t_junctions() {
        assert_eq!(wall_glyph(0b0111), '\u{2560}'); // ╠  N+E+S
        assert_eq!(wall_glyph(0b1011), '\u{2569}'); // ╩  N+E+W
        assert_eq!(wall_glyph(0b1101), '\u{2563}'); // ╣  N+S+W
        assert_eq!(wall_glyph(0b1110), '\u{2566}'); // ╦  E+S+W
    }

    // -- apply_light --

    #[test]
    fn apply_light_full() {
        assert_eq!(apply_light((200, 100, 50), 255), (200, 100, 50));
    }

    #[test]
    fn apply_light_half() {
        let result = apply_light((200, 100, 50), 128);
        // 200*128/255 ≈ 100, 100*128/255 ≈ 50, 50*128/255 ≈ 25
        assert_eq!(result, (100, 50, 25));
    }

    #[test]
    fn apply_light_zero() {
        assert_eq!(apply_light((200, 100, 50), 0), (0, 0, 0));
    }

    // -- dim_remembered --

    #[test]
    fn dim_remembered_grayscale() {
        let result = dim_remembered((255, 255, 255));
        // gray = (255*77 + 255*150 + 255*29)/256 = 255
        // dimmed = 255 * 102 / 255 = 102
        assert_eq!(result.0, result.1);
        assert_eq!(result.1, result.2);
        assert_eq!(result.0, 102);
    }

    #[test]
    fn dim_remembered_black_stays_black() {
        assert_eq!(dim_remembered((0, 0, 0)), (0, 0, 0));
    }

    // -- sector_floor_kind --

    #[test]
    fn nukage_sector_classified() {
        assert_eq!(sector_floor_kind(5), TileKind::Nukage);
        assert_eq!(sector_floor_kind(7), TileKind::Nukage);
        assert_eq!(sector_floor_kind(16), TileKind::Nukage);
    }

    #[test]
    fn lava_sector_classified() {
        assert_eq!(sector_floor_kind(4), TileKind::Lava);
        assert_eq!(sector_floor_kind(11), TileKind::Lava);
    }

    #[test]
    fn normal_sector_classified() {
        assert_eq!(sector_floor_kind(0), TileKind::Floor);
        assert_eq!(sector_floor_kind(1), TileKind::Floor);
        assert_eq!(sector_floor_kind(99), TileKind::Floor);
    }

    // -- tile_glyph --

    #[test]
    fn floor_glyph() {
        let g = tile_glyph(TileKind::Floor);
        assert_eq!(g.glyph, '\u{00B7}');
        assert_eq!(g.fg, (60, 60, 60));
    }

    #[test]
    fn wall_tile_glyph() {
        let g = tile_glyph(TileKind::Wall);
        assert_eq!(g.glyph, '\u{2588}');
        assert_eq!(g.fg, (80, 80, 80));
    }

    #[test]
    fn void_is_all_black() {
        let g = tile_glyph(TileKind::Void);
        assert_eq!(g.glyph, ' ');
        assert_eq!(g.fg, (0, 0, 0));
        assert_eq!(g.bg, (0, 0, 0));
    }

    #[test]
    fn nukage_has_green_bg() {
        let g = tile_glyph(TileKind::Nukage);
        assert_eq!(g.bg, (0, 20, 0));
    }

    #[test]
    fn lava_has_red_bg() {
        let g = tile_glyph(TileKind::Lava);
        assert_eq!(g.bg, (40, 0, 0));
    }

    // -- exhaustiveness check --

    #[test]
    fn all_mobj_kinds_have_glyphs() {
        // Ensure every MobjKind variant produces a glyph without panicking.
        for i in 0..=75_u16 {
            // Safety: MobjKind is repr(u16) with contiguous values 0..=75.
            let kind: MobjKind = unsafe { std::mem::transmute(i) };
            let _g = entity_glyph(kind, 100);
            let _g_dead = entity_glyph(kind, 0);
        }
    }
}
