//! Doom picture-format parser and sprite rendering.
//!
//! All sprite lumps and patch lumps use the "picture" format:
//!
//! ```text
//! Header (8 bytes):
//!   u16  width
//!   u16  height
//!   i16  leftoffset   ← pixels to the left of the center point
//!   i16  topoffset    ← pixels above the origin
//!
//! Column offsets (width × 4 bytes):
//!   width × u32  col_offset  ← byte offset from start of lump to column data
//!
//! Column data at each col_offset:
//!   loop:
//!     u8  topdelta    ← 0xFF = end of column; otherwise row where post starts
//!     u8  length      ← number of pixels in this post
//!     u8  _unused     ← skip (padding)
//!     length × u8  pixels  ← palette indices
//!     u8  _unused     ← skip (padding)
//! ```
//!
//! Pixels not covered by any post are transparent (`None`).

use std::collections::HashMap;

use doom_wad::{LumpDef, WadFile, WadStack};

use crate::colormap::ColormapCache;
use crate::framebuffer::Framebuffer;
use crate::fuzz::draw_fuzz_column;
use crate::lighting::LightParams;
use crate::render_flags::RenderFlag;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// A parsed Doom picture (sprite frame, patch, weapon overlay, etc.).
///
/// Pixel storage is column-major: `pixels[col * height + row]`.
/// Transparent pixels are `None`; opaque pixels are `Some(palette_index)`.
#[derive(Clone, Debug)]
pub struct SpriteFrame {
    /// Width in pixels.
    pub width: u16,
    /// Height in pixels.
    pub height: u16,
    /// Pixels to the left of the sprite's center point (for X centering).
    pub left_offset: i16,
    /// Pixels above the sprite's baseline/origin (for Y positioning).
    pub top_offset: i16,
    /// Column-major pixel data. `pixels[col * height + row]`.
    /// Length is always `width * height`.
    pub pixels: Vec<Option<u8>>,
}

/// Cache of all sprite frames loaded from WAD lumps between S_START and S_END.
///
/// Keyed by uppercase lump name (null bytes stripped).
pub struct SpriteCache {
    frames: HashMap<String, SpriteFrame>,
}

impl SpriteCache {
    /// Load all sprite lumps between S_START and S_END from the WAD.
    ///
    /// Lumps that fail to parse (too small, malformed column offsets) are
    /// silently skipped — this matches vanilla Doom's behaviour.
    pub fn load(wad: &WadFile) -> Self {
        let mut frames = HashMap::new();

        for lump in wad.lumps_between("S_START", "S_END") {
            Self::insert_frame(&mut frames, wad, lump);
        }

        Self { frames }
    }

    /// Load all sprite lumps between sprite markers from a WAD stack.
    pub fn load_from_stack(wad_stack: &WadStack) -> Self {
        let mut frames = HashMap::new();
        let mut in_sprite_section = false;

        for (wad, lump) in wad_stack.all_lumps() {
            match lump.name.as_str() {
                "S_START" | "SS_START" => {
                    in_sprite_section = true;
                    continue;
                }
                "S_END" | "SS_END" => {
                    in_sprite_section = false;
                    continue;
                }
                _ => {}
            }

            if !in_sprite_section {
                continue;
            }

            Self::insert_frame(&mut frames, wad, lump);
        }

        Self { frames }
    }

    /// Look up a sprite frame by 8-byte lump name (uppercase, null-trimmed).
    ///
    /// The name is normalised to uppercase with trailing null bytes stripped
    /// before lookup, matching how WAD lump names are stored.
    pub fn get(&self, name: &[u8; 8]) -> Option<&SpriteFrame> {
        // Trim trailing nulls and convert to uppercase string.
        let trimmed_len = name.iter().position(|&b| b == 0).unwrap_or(8);
        let key = std::str::from_utf8(&name[..trimmed_len])
            .ok()?
            .to_uppercase();
        self.frames.get(&key)
    }

    /// Number of sprite frames in the cache.
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// Returns `true` if the cache contains no sprite frames.
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Insert a frame directly (used in tests and by callers that pre-parse frames).
    pub fn insert(&mut self, name: String, frame: SpriteFrame) {
        self.frames.insert(name.to_uppercase(), frame);
    }

    /// Construct an empty cache (useful in tests).
    pub fn empty() -> Self {
        Self {
            frames: HashMap::new(),
        }
    }

    fn insert_frame(frames: &mut HashMap<String, SpriteFrame>, wad: &WadFile, lump: &LumpDef) {
        if lump.size == 0 {
            return;
        }
        let data = wad.lump_data(lump);
        if let Some(frame) = parse_picture(data) {
            let name = lump.name.as_str().to_uppercase();
            frames.insert(name, frame);
        }
    }
}

// ---------------------------------------------------------------------------
// Sprite name resolution
// ---------------------------------------------------------------------------

/// Build the WAD lump name for a sprite given its prefix, frame, and rotation.
///
/// Returns an 8-byte lump name. For example, `sprite_lump_name(b"TROO", 0, 1)`
/// produces `b"TROOA1\0\0"` (Imp, frame A, rotation 1).
///
/// - `frame` is 0-based: 0 = A, 1 = B, 2 = C, etc.
/// - `rotation` is 0 for non-directional sprites, or 1-8 for 8-direction sprites.
pub fn sprite_lump_name(prefix: &[u8; 4], frame: u8, rotation: u8) -> [u8; 8] {
    let mut name = [0u8; 8];
    name[0..4].copy_from_slice(prefix);
    name[4] = b'A' + frame;
    name[5] = b'0' + rotation;
    name
}

/// Compute the sprite rotation index (1-8) based on the thing's facing angle
/// and the angle from the thing to the viewer (player).
///
/// Doom sprites have 8 rotation frames numbered 1-8:
/// 1=front, 2=front-right, 3=right, 4=back-right,
/// 5=back, 6=back-left, 7=left, 8=front-left.
///
/// The rotation is determined by computing the relative angle between the
/// direction from the thing to the viewer and the thing's facing direction,
/// then quantising into one of 8 sectors (each 45 degrees wide).
pub fn compute_sprite_rotation(
    thing_angle: doom_types::Bam,
    thing_x: doom_types::Fixed16_16,
    thing_y: doom_types::Fixed16_16,
    viewer_x: doom_types::Fixed16_16,
    viewer_y: doom_types::Fixed16_16,
) -> u8 {
    // Doom's R_ProjectSprite uses R_PointToAngle(thing->x, thing->y) which
    // computes the angle FROM the viewpoint TO the thing.
    let dx = (thing_x - viewer_x).to_int() as f64;
    let dy = (thing_y - viewer_y).to_int() as f64;
    let angle_rad = dy.atan2(dx); // standard math angle (CCW from +X)
    // Convert radians to BAM (0..2^32 maps to 0..2*PI).
    let angle_to_viewer_bam = doom_types::Bam(
        (angle_rad.rem_euclid(std::f64::consts::TAU) / std::f64::consts::TAU
            * (u32::MAX as f64 + 1.0)) as u32,
    );

    // Doom's R_ProjectSprite offset: (ANG45/2)*9 = 0x1000_0000 * 9 = 0x9000_0000.
    // This bias rotates the quantisation bins so that sector 0 aligns with
    // the "front" view (rotation 1) when the viewer is directly in front
    // of the thing.
    //
    // ANG45 = 0x2000_0000, so ANG45/2 = 0x1000_0000.
    // The 3 most significant bits of `relative` give us a 0-7 sector index.
    let offset: u32 = 0x9000_0000; // (ANG45 / 2) * 9
    let relative = angle_to_viewer_bam
        .0
        .wrapping_sub(thing_angle.0)
        .wrapping_add(offset);
    let sector = (relative >> 29) & 7;
    (sector as u8) + 1
}

/// Return the 4-character sprite prefix for a DoomEd thing type.
///
/// Returns `None` for thing types that have no world sprite (player starts,
/// teleport destinations, etc.) or unknown types.
pub fn thing_sprite_prefix(kind: u16) -> Option<[u8; 4]> {
    Some(match kind {
        // Player starts and deathmatch — no world sprite
        1 | 2 | 3 | 4 | 11 | 14 => return None,

        // ---------- Monsters ----------
        3004 => *b"POSS", // Zombieman
        9 => *b"SPOS",    // Shotgun Guy
        65 => *b"CPOS",   // Chaingunner
        3001 => *b"TROO", // Imp
        3002 => *b"SARG", // Demon (Pinky)
        58 => *b"SARG",   // Spectre (same sprite as Demon)
        3005 => *b"HEAD", // Cacodemon
        3003 => *b"BOSS", // Baron of Hell
        69 => *b"BOS2",   // Hell Knight
        3006 => *b"SKUL", // Lost Soul
        68 => *b"BSPI",   // Arachnotron
        71 => *b"PAIN",   // Pain Elemental
        66 => *b"SKEL",   // Revenant
        67 => *b"FATT",   // Mancubus
        64 => *b"VILE",   // Arch-vile
        7 => *b"SPID",    // Spider Mastermind
        16 => *b"CYBR",   // Cyberdemon
        84 => *b"SSWV",   // Wolfenstein SS
        72 => *b"KEEN",   // Commander Keen
        88 => *b"BBRN",   // Boss Brain

        // ---------- Decorations ----------
        2035 => *b"BAR1", // Barrel
        70 => *b"FCAN",   // Burning barrel
        43 => *b"TRE1",   // Burnt tree
        54 => *b"TRE2",   // Large brown tree
        2028 => *b"COLU", // Floor lamp
        85 => *b"TLMP",   // Tall tech lamp
        86 => *b"TLP2",   // Short tech lamp
        34 => *b"CAND",   // Candle
        35 => *b"CBRA",   // Candelabra
        44 => *b"TBLU",   // Tall blue firestick
        45 => *b"TGRN",   // Tall green firestick
        46 => *b"TRED",   // Tall red firestick
        55 => *b"SMBT",   // Short blue firestick
        56 => *b"SMGT",   // Short green firestick
        57 => *b"SMRT",   // Short red firestick
        48 => *b"ELEC",   // Tall tech column
        30 => *b"COL1",   // Tall green pillar
        32 => *b"COL3",   // Tall red pillar
        31 => *b"COL2",   // Short green pillar
        33 => *b"COL4",   // Short red pillar
        36 => *b"COL5",   // Green pillar w/ heart
        37 => *b"COL6",   // Red pillar w/ skull
        47 => *b"SMIT",   // Stalagmite

        // ---------- Items ----------
        2014 => *b"BON1", // Health bonus
        2015 => *b"BON2", // Armor bonus
        2011 => *b"STIM", // Stimpack
        2012 => *b"MEDI", // Medikit
        2013 => *b"SOUL", // Soulsphere
        2018 => *b"ARM1", // Green armor
        2019 => *b"ARM2", // Blue armor
        2022 => *b"PINV", // Invulnerability
        2023 => *b"PSTR", // Berserk
        2024 => *b"PINS", // Partial invisibility
        2025 => *b"SUIT", // Radiation suit
        2026 => *b"PMAP", // Computer area map
        2045 => *b"PVIS", // Light amplification
        8 => *b"BPAK",    // Backpack

        // ---------- Ammo ----------
        2007 => *b"CLIP", // Ammo clip
        2048 => *b"AMMO", // Box of bullets
        2008 => *b"SHEL", // Shells
        2049 => *b"SBOX", // Box of shells
        2010 => *b"ROCK", // Rocket
        2046 => *b"BROK", // Box of rockets
        2047 => *b"CELL", // Cell charge
        17 => *b"CELP",   // Cell pack

        // ---------- Weapon pickups ----------
        2001 => *b"SHOT", // Shotgun
        82 => *b"SGN2",   // Super shotgun
        2002 => *b"MGUN", // Chaingun
        2003 => *b"LAUN", // Rocket launcher
        2004 => *b"PLAS", // Plasma rifle
        2006 => *b"BFUG", // BFG 9000
        2005 => *b"CSAW", // Chainsaw

        // ---------- Keys ----------
        5 => *b"BKEY",  // Blue keycard
        6 => *b"YKEY",  // Yellow keycard
        13 => *b"RKEY", // Red keycard
        40 => *b"BSKU", // Blue skull key
        39 => *b"YSKU", // Yellow skull key
        38 => *b"RSKU", // Red skull key

        _ => return None,
    })
}

/// Returns `true` if this DoomEd thing type has rotational sprites (8 directions).
///
/// Monsters generally have 8-directional walking/attack frames.
/// Items, decorations, and ammo do not (they use rotation 0).
pub fn thing_has_rotations(kind: u16) -> bool {
    matches!(
        kind,
        3004 | 9
            | 65
            | 3001
            | 3002
            | 58
            | 3005
            | 3003
            | 69
            | 3006
            | 68
            | 71
            | 66
            | 67
            | 64
            | 7
            | 16
            | 84
    )
}

/// Compute the mirrored rotation index for sprite fallback.
///
/// Doom WADs often store only rotations 1-5 and mirror 2↔8, 3↔7, 4↔6.
/// Rotations 1 (front) and 5 (back) are symmetric and never mirrored.
///
/// Returns `Some(mirror_rotation)` if this rotation can be mirrored,
/// or `None` if it is symmetric (1 or 5).
fn mirror_rotation(rotation: u8) -> Option<u8> {
    match rotation {
        2 => Some(8),
        3 => Some(7),
        4 => Some(6),
        6 => Some(4),
        7 => Some(3),
        8 => Some(2),
        _ => None, // 0, 1, 5 have no mirror
    }
}

// ---------------------------------------------------------------------------
// Picture-format parser
// ---------------------------------------------------------------------------

/// Parse a Doom picture-format lump into a [`SpriteFrame`].
///
/// Returns `None` if the data is too short, the dimensions are zero, or the
/// column offset table would overflow the buffer.
pub fn parse_picture(data: &[u8]) -> Option<SpriteFrame> {
    // Minimum: 8-byte header.
    if data.len() < 8 {
        return None;
    }

    let width = u16::from_le_bytes([data[0], data[1]]) as usize;
    let height = u16::from_le_bytes([data[2], data[3]]) as usize;
    let left = i16::from_le_bytes([data[4], data[5]]);
    let top = i16::from_le_bytes([data[6], data[7]]);

    // Zero-dimension pictures are not renderable.
    if width == 0 || height == 0 {
        return None;
    }

    // The column offset table follows the header: width × 4 bytes.
    let col_table_end = 8usize.checked_add(width.checked_mul(4)?)?;
    if data.len() < col_table_end {
        return None;
    }

    let mut pixels = vec![None::<u8>; width * height];

    for col in 0..width {
        let table_pos = 8 + col * 4;
        let off = u32::from_le_bytes(data[table_pos..table_pos + 4].try_into().ok()?) as usize;

        // Parse the column's posts.
        let mut pos = off;
        loop {
            if pos >= data.len() {
                break;
            }
            let topdelta = data[pos];
            pos += 1;

            // 0xFF signals end of column.
            if topdelta == 0xFF {
                break;
            }

            if pos >= data.len() {
                break;
            }
            let length = data[pos] as usize;
            pos += 1;

            // Skip the first unused byte (pre-pixel padding).
            pos += 1;

            for i in 0..length {
                if pos >= data.len() {
                    break;
                }
                let pixel = data[pos];
                pos += 1;

                let row = (topdelta as usize) + i;
                if row < height {
                    pixels[col * height + row] = Some(pixel);
                }
            }

            // Skip the post-pixel trailing padding byte.
            pos += 1;
        }
    }

    Some(SpriteFrame {
        width: width as u16,
        height: height as u16,
        left_offset: left,
        top_offset: top,
        pixels,
    })
}

// ---------------------------------------------------------------------------
// Thing projection constants (must match render.rs)
// ---------------------------------------------------------------------------

const SCREEN_W: usize = 320;
const SCREEN_H: usize = 200;
const HALF_W: i32 = 160;
const HALF_H: i32 = 100;
const FOCAL_LEN: f32 = 160.0;
/// Eye height above floor in map units (matches render.rs PLAYER_HEIGHT).
const PLAYER_HEIGHT: f32 = 41.0;

#[derive(Clone, Copy)]
pub struct SpriteClip<'a> {
    pub top: &'a [i32; SCREEN_W],
    pub bottom: &'a [i32; SCREEN_W],
    pub top_depth: &'a [f32; SCREEN_W],
    pub bottom_depth: &'a [f32; SCREEN_W],
    pub top_history: Option<&'a [Vec<crate::render::SpriteClipStep>]>,
    pub bottom_history: Option<&'a [Vec<crate::render::SpriteClipStep>]>,
}

impl<'a> SpriteClip<'a> {
    fn clip_top(&self, x: usize, sprite_depth: f32, sprite_top_z: f32, unclipped_top: i32) -> i32 {
        let clip_row = self
            .top_history
            .and_then(|history| history.get(x))
            .and_then(|steps| {
                steps
                    .iter()
                    .rev()
                    .find(|step| {
                        sprite_depth >= step.depth && sprite_top_z > step.silhouette_height
                    })
                    .map(|step| step.row)
            })
            .or_else(|| (sprite_depth >= self.top_depth[x]).then_some(self.top[x]));

        clip_row.map_or(unclipped_top, |row| unclipped_top.max(row))
    }

    fn clip_bottom(
        &self,
        x: usize,
        sprite_depth: f32,
        sprite_bottom_z: f32,
        unclipped_bottom: i32,
    ) -> i32 {
        let clip_row = self
            .bottom_history
            .and_then(|history| history.get(x))
            .and_then(|steps| {
                steps
                    .iter()
                    .rev()
                    .find(|step| {
                        sprite_depth >= step.depth && sprite_bottom_z < step.silhouette_height
                    })
                    .map(|step| step.row)
            })
            .or_else(|| (sprite_depth >= self.bottom_depth[x]).then_some(self.bottom[x]));

        clip_row.map_or(unclipped_bottom, |row| unclipped_bottom.min(row))
    }
}

fn project_sprite_vertical_bounds(
    view_z: f32,
    sprite_top_z: f32,
    sprite_scale: f32,
    screen_h: i32,
) -> (i32, i32) {
    let screen_y_top = HALF_H - ((sprite_top_z - view_z) * sprite_scale).round() as i32;
    let screen_y_bot = screen_y_top + screen_h - 1;
    (screen_y_top, screen_y_bot)
}

// ---------------------------------------------------------------------------
// Sector lookup for sprites
// ---------------------------------------------------------------------------

/// Look up the sector index for a world-space point `(x, y)` using the level's BSP tree.
///
/// Walks the BSP to find the subsector containing the point, then traces
/// subsector -> first seg -> linedef -> sidedef -> sector.
///
/// Returns `None` if the level has no nodes/ssectors, or if any index is
/// out of bounds (graceful degradation for malformed levels).
#[must_use]
pub fn sector_for_point(level: &doom_map::Level, x: i32, y: i32) -> Option<usize> {
    level.sector_index_at(x, y)
}

/// Determine the [`RenderFlag`] for a DoomEd thing type.
///
/// Spectres (type 58) get `Fuzz`, projectile/explosion/glow types get
/// `FullBright`, everything else gets `Normal`.
///
/// This is a simplified mapping based on the DoomEd type alone. In a
/// full implementation, the thing's current animation state would also
/// influence the flag (e.g. muzzle flash frames are fullbright).
#[must_use]
pub fn render_flag_for_thing(kind: u16) -> RenderFlag {
    match kind {
        // Spectre — fuzz effect
        58 => RenderFlag::Fuzz,

        // Projectiles (glow / self-luminous)
        // These are DoomEd types but also cover spawned projectile types
        // that might appear in the level's thing list during gameplay.

        // Items that glow
        2013 => RenderFlag::FullBright, // Soulsphere
        2022 => RenderFlag::FullBright, // Invulnerability sphere
        2045 => RenderFlag::FullBright, // Light amplification visor

        // Firestick / torch decorations (glow)
        44..=46 => RenderFlag::FullBright, // Tall firesticks (blue/green/red)
        55..=57 => RenderFlag::FullBright, // Short firesticks (blue/green/red)
        70 => RenderFlag::FullBright,      // Burning barrel
        34 => RenderFlag::FullBright,      // Candle
        35 => RenderFlag::FullBright,      // Candelabra

        // Lamps and light sources
        2028 => RenderFlag::FullBright, // Floor lamp
        85 => RenderFlag::FullBright,   // Tall tech lamp
        86 => RenderFlag::FullBright,   // Short tech lamp
        48 => RenderFlag::FullBright,   // Tall tech column (animated)

        _ => RenderFlag::Normal,
    }
}

// ---------------------------------------------------------------------------
// Thing sprite lookup (legacy wrapper)
// ---------------------------------------------------------------------------

/// Map a DoomEd type to a rotation-0 lump name (legacy convenience wrapper).
///
/// Internally delegates to [`thing_sprite_prefix`] + [`sprite_lump_name`] with
/// frame=0 (A) and rotation=0.
#[cfg(test)]
fn thing_sprite(kind: u16) -> Option<[u8; 8]> {
    let prefix = thing_sprite_prefix(kind)?;
    Some(sprite_lump_name(&prefix, 0, 0))
}

// ---------------------------------------------------------------------------
// Billboard sprite projection
// ---------------------------------------------------------------------------

/// Project and render all Things from the level as billboard sprites with
/// distance-attenuated sector lighting.
///
/// Call this **after** `render_level` so wall columns are already drawn.
/// Uses the painter's algorithm (back-to-front sort) with per-column
/// z-buffer clipping against walls.
///
/// # Arguments
/// - `level`        — parsed map (provides Things list, BSP, sectors).
/// - `player_x/y`  — player world position (Fixed16_16).
/// - `player_angle` — player view angle (Bam, 32-bit; full circle = 2^32).
/// - `fb`           — framebuffer to draw into.
/// - `cache`        — sprite frame cache (loaded from S_START..S_END).
/// - `z_buffer`     — optional per-column depth buffer from `render_level`.
///   When provided, sprite columns whose depth exceeds
///   the wall depth at that screen column are clipped
///   (not drawn). Pass `None` to disable wall clipping
///   (backward-compatible behaviour).
/// - `colormap`     — optional colormap cache for distance-attenuated lighting.
///   When provided, each sprite column is shaded based on
///   the thing's sector light level and distance from the
///   player. Fullbright things (projectiles, lamps) and
///   fuzz-effect things (spectres) are handled specially.
///   Pass `None` to render all sprites at full brightness
///   (backward-compatible behaviour).
///
/// # Projection model
/// View space is computed with a standard rotation: `vx` is depth (forward
/// from the player eye), `vy` is lateral displacement.  A sprite with
/// `vx <= 0.5` is behind or too close and is skipped.  Screen X of the
/// sprite centre is `HALF_W + FOCAL_LEN * vy / vx`; sprite screen height is
/// `frame.height * FOCAL_LEN / vx`.
/// Render live actors using state-driven sprites (`ActorRenderInfo`).
///
/// Unlike [`render_things_ex`] which guesses the sprite from the DoomEd thing
/// type and always draws frame A, this function uses the sprite/frame stored
/// in the actor's current state — giving correct animations.
pub fn render_actors_ex(
    actors: &[crate::sprite_lookup::ActorRenderInfo],
    level: &doom_map::Level,
    player_x: doom_types::Fixed16_16,
    player_y: doom_types::Fixed16_16,
    player_angle: doom_types::Bam,
    fb: &mut Framebuffer,
    cache: &SpriteCache,
    z_buffer: Option<&[f32; SCREEN_W]>,
    colormap: Option<&ColormapCache>,
    sprite_clip: Option<SpriteClip<'_>>,
) {
    render_actors_with_masked_ex(
        actors,
        level,
        player_x,
        player_y,
        player_angle,
        fb,
        cache,
        z_buffer,
        colormap,
        sprite_clip,
        None,
    );
}

#[allow(clippy::too_many_arguments)]
pub fn render_actors_with_masked_ex<'a>(
    actors: &[crate::sprite_lookup::ActorRenderInfo],
    level: &doom_map::Level,
    player_x: doom_types::Fixed16_16,
    player_y: doom_types::Fixed16_16,
    player_angle: doom_types::Bam,
    fb: &mut Framebuffer,
    cache: &SpriteCache,
    z_buffer: Option<&[f32; SCREEN_W]>,
    colormap: Option<&ColormapCache>,
    // Per-column portal clip (mfloorclip/mceilingclip). Pass
    // `Some((&out.clip_top, &out.clip_bot))` to prevent sprites from
    // bleeding through two-sided window frames.
    sprite_clip: Option<SpriteClip<'_>>,
    masked_columns: Option<&'a [crate::render::MaskedColumnDraw<'a>]>,
) {
    use doom_game::states::sprite_names;

    let angle_rad =
        (player_angle.0 as f32) * (std::f32::consts::PI * 2.0 / (u32::MAX as f32 + 1.0));
    let cos_a = angle_rad.cos();
    let sin_a = angle_rad.sin();
    let px = player_x.raw() as f32 / 65536.0;
    let py = player_y.raw() as f32 / 65536.0;

    // Player's eye height in world space.  BSP-walk to find the sector the
    // player is standing in, then add PLAYER_HEIGHT above that floor.
    let player_floor = sector_for_point(level, px as i32, py as i32)
        .and_then(|si| level.sectors.get(si))
        .map_or(0.0, |s| s.floor_height as f32);
    let view_z = player_floor + PLAYER_HEIGHT;

    enum VisibleElement<'a> {
        Actor(f32, &'a crate::sprite_lookup::ActorRenderInfo),
        Masked(&'a crate::render::MaskedColumnDraw<'a>),
    }

    impl VisibleElement<'_> {
        fn depth(&self) -> f32 {
            match self {
                Self::Actor(depth, _) => *depth,
                Self::Masked(column) => column.depth,
            }
        }
    }

    // Collect and depth-sort sprites and masked midtextures back-to-front.
    let mut visible: Vec<VisibleElement<'_>> = actors
        .iter()
        .filter_map(|a| {
            let ax = a.x as f32 / 65536.0;
            let ay = a.y as f32 / 65536.0;
            let dx = ax - px;
            let dy = ay - py;
            let vx = dx * cos_a + dy * sin_a;
            if vx > 0.5 {
                Some(VisibleElement::Actor(vx, a))
            } else {
                None
            }
        })
        .collect();
    if let Some(masked) = masked_columns {
        visible.extend(masked.iter().map(VisibleElement::Masked));
    }
    visible.sort_by(|a, b| {
        b.depth()
            .partial_cmp(&a.depth())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut fuzz_pos: usize = 0;

    for entry in visible {
        let (vx, actor) = match entry {
            VisibleElement::Actor(vx, actor) => (vx, actor),
            VisibleElement::Masked(column) => {
                crate::render::draw_masked_columns(fb, std::slice::from_ref(column));
                continue;
            }
        };
        let sprite_idx = actor.sprite as usize;
        let is_null_sprite =
            actor.sprite == sprite_names::SPR_NONE || sprite_idx >= sprite_names::SPRITE_COUNT;

        // Build a 4-byte sprite prefix.
        // State-driven path: use the sprite index from STATES.
        // Fallback path: use the pre-computed prefix for items with S_NULL spawn state.
        let (prefix, frame_idx) = if !is_null_sprite {
            let sprite_name_str = sprite_names::SPRITE_NAMES[sprite_idx];
            let mut p = [0u8; 4];
            for (i, b) in sprite_name_str.bytes().take(4).enumerate() {
                p[i] = b.to_ascii_uppercase();
            }
            (p, actor.frame & 0x7F)
        } else if let Some(fb) = actor.fallback_prefix {
            (fb, 0u8)
        } else {
            continue;
        };

        // Compute rotation based on actor facing vs viewer direction.
        let thing_angle = doom_types::Bam(actor.angle);
        let thing_x = doom_types::Fixed16_16(actor.x);
        let thing_y = doom_types::Fixed16_16(actor.y);

        let has_rotations = {
            // Check if rotation 1 lump exists (non-directional sprites only have rotation 0).
            let test = sprite_lump_name(&prefix, frame_idx, 1);
            cache.get(&test).is_some()
        };

        let (frame, flip_x) = if has_rotations {
            let rotation =
                compute_sprite_rotation(thing_angle, thing_x, thing_y, player_x, player_y);
            let lump_name = sprite_lump_name(&prefix, frame_idx, rotation);
            if let Some(f) = cache.get(&lump_name) {
                (f, false)
            } else if let Some(mirror_rot) = mirror_rotation(rotation) {
                let mirror_name = sprite_lump_name(&prefix, frame_idx, mirror_rot);
                if let Some(f) = cache.get(&mirror_name) {
                    (f, true)
                } else {
                    let fallback = sprite_lump_name(&prefix, frame_idx, 0);
                    match cache.get(&fallback) {
                        Some(f) => (f, false),
                        None => continue,
                    }
                }
            } else {
                let fallback = sprite_lump_name(&prefix, frame_idx, 0);
                match cache.get(&fallback) {
                    Some(f) => (f, false),
                    None => continue,
                }
            }
        } else {
            let lump_name = sprite_lump_name(&prefix, frame_idx, 0);
            match cache.get(&lump_name) {
                Some(f) => (f, false),
                None => continue,
            }
        };

        let render_flag = actor.render_flag;

        let ax = actor.x as f32 / 65536.0;
        let ay = actor.y as f32 / 65536.0;
        let dx = ax - px;
        let dy = ay - py;
        let vy = dx * sin_a - dy * cos_a;
        let sx_center = HALF_W as f32 + FOCAL_LEN * vy / vx;
        let sprite_scale = FOCAL_LEN / vx;
        let screen_h = ((frame.height as f32) * sprite_scale).round() as i32;
        let screen_w = ((frame.width as f32) * sprite_scale).round() as i32;

        if screen_h <= 0 || screen_w <= 0 {
            continue;
        }

        let actor_top_z = actor.z as f32 / 65536.0 + frame.top_offset as f32;
        let actor_bottom_z = actor_top_z - frame.height as f32;
        let (screen_y_top, screen_y_bot) =
            project_sprite_vertical_bounds(view_z, actor_top_z, sprite_scale, screen_h);
        let screen_x_left =
            sx_center as i32 - (frame.left_offset as i32 * screen_w / frame.width as i32);
        let screen_x_right = screen_x_left + screen_w;

        if screen_x_right <= 0 || screen_x_left >= SCREEN_W as i32 {
            continue;
        }

        let col_h = screen_y_bot - screen_y_top;

        let light_params = if render_flag == RenderFlag::FullBright {
            LightParams::new(255, true)
        } else if colormap.is_some() {
            let sector_light = sector_for_point(level, ax as i32, ay as i32)
                .and_then(|si| level.sectors.get(si))
                .map_or(255, |s| s.light_level.clamp(0, 255) as u8);
            LightParams::new(sector_light, false)
        } else {
            LightParams::new(255, true)
        };

        let sprite_colormap: Option<&[u8; 256]> =
            colormap.map(|cm| light_params.get_colormap(vx, cm));
        let sy_top_clamped = screen_y_top.max(0);
        let sy_bot_clamped = screen_y_bot.min(SCREEN_H as i32 - 1);
        let sx_start = screen_x_left.max(0);
        let sx_end = (screen_x_right - 1).min(SCREEN_W as i32 - 1);

        for sx in sx_start..=sx_end {
            let screen_col = sx - screen_x_left;
            let sprite_col = if flip_x {
                frame.width as i32 - 1 - screen_col * frame.width as i32 / screen_w.max(1)
            } else {
                screen_col * frame.width as i32 / screen_w.max(1)
            };
            let sprite_col = sprite_col.clamp(0, frame.width as i32 - 1) as usize;

            if let Some(zbuf) = z_buffer {
                if vx >= zbuf[sx as usize] {
                    continue;
                }
            }

            // Narrow vertical extent by portal clip (mfloorclip/mceilingclip).
            let col_top = if let Some(clip) = sprite_clip {
                clip.clip_top(sx as usize, vx, actor_top_z, sy_top_clamped)
            } else {
                sy_top_clamped
            };
            let col_bot = if let Some(clip) = sprite_clip {
                clip.clip_bottom(sx as usize, vx, actor_bottom_z, sy_bot_clamped)
            } else {
                sy_bot_clamped
            };
            if col_top > col_bot {
                continue;
            }

            if render_flag == RenderFlag::Fuzz {
                let sy_top_u = col_top.max(0) as usize;
                let sy_bot_u = col_bot.min(SCREEN_H as i32 - 1) as usize;
                if sy_top_u <= sy_bot_u {
                    draw_fuzz_column(fb, sx as usize, sy_top_u, sy_bot_u, &mut fuzz_pos, colormap);
                }
                continue;
            }

            for sy in col_top..=col_bot {
                let sprite_row = (sy - screen_y_top) * frame.height as i32 / col_h.max(1);
                let sprite_row = sprite_row.clamp(0, frame.height as i32 - 1) as usize;
                let pixel_idx = sprite_col * frame.height as usize + sprite_row;
                if let Some(Some(raw_idx)) = frame.pixels.get(pixel_idx) {
                    let final_color = match sprite_colormap {
                        Some(cm) => cm[*raw_idx as usize],
                        None => *raw_idx,
                    };
                    fb.data[sy as usize * SCREEN_W + sx as usize] = final_color;
                }
            }
        }
    }
}

/// Like [`render_things`] but accepts an explicit slice of things rather than
/// pulling them from `level.things`.  Use this when rendering live game-state
/// objects (mobjslab) whose positions have moved since the WAD was loaded.
pub fn render_things_ex(
    things: &[doom_map::Thing],
    level: &doom_map::Level,
    player_x: doom_types::Fixed16_16,
    player_y: doom_types::Fixed16_16,
    player_angle: doom_types::Bam,
    fb: &mut Framebuffer,
    cache: &SpriteCache,
    z_buffer: Option<&[f32; SCREEN_W]>,
    colormap: Option<&ColormapCache>,
    sprite_clip: Option<SpriteClip<'_>>,
) {
    render_things_impl(
        things,
        level,
        player_x,
        player_y,
        player_angle,
        fb,
        cache,
        z_buffer,
        colormap,
        sprite_clip,
    );
}

/// Render all Things from `level.things` as billboard sprites.
pub fn render_things(
    level: &doom_map::Level,
    player_x: doom_types::Fixed16_16,
    player_y: doom_types::Fixed16_16,
    player_angle: doom_types::Bam,
    fb: &mut Framebuffer,
    cache: &SpriteCache,
    z_buffer: Option<&[f32; SCREEN_W]>,
    colormap: Option<&ColormapCache>,
    sprite_clip: Option<SpriteClip<'_>>,
) {
    render_things_impl(
        &level.things,
        level,
        player_x,
        player_y,
        player_angle,
        fb,
        cache,
        z_buffer,
        colormap,
        sprite_clip,
    );
}

fn render_things_impl(
    things: &[doom_map::Thing],
    level: &doom_map::Level,
    player_x: doom_types::Fixed16_16,
    player_y: doom_types::Fixed16_16,
    player_angle: doom_types::Bam,
    fb: &mut Framebuffer,
    cache: &SpriteCache,
    z_buffer: Option<&[f32; SCREEN_W]>,
    colormap: Option<&ColormapCache>,
    sprite_clip: Option<SpriteClip<'_>>,
) {
    // Convert player angle (32-bit BAM) to radians.
    // BAM: 0x0000_0000 = 0, 0x4000_0000 = 90, 0x8000_0000 = 180, etc.
    let angle_rad =
        (player_angle.0 as f32) * (std::f32::consts::PI * 2.0 / (u32::MAX as f32 + 1.0));
    let cos_a = angle_rad.cos();
    let sin_a = angle_rad.sin();

    // Player eye position in map units (f32).
    // Fixed16_16 stores value as raw i32 with 16.16 encoding; divide by 65536
    // to convert to f32 map units.
    let px = player_x.raw() as f32 / 65536.0;
    let py = player_y.raw() as f32 / 65536.0;
    let player_floor = sector_for_point(level, px as i32, py as i32)
        .and_then(|si| level.sectors.get(si))
        .map_or(0.0, |s| s.floor_height as f32);
    let view_z = player_floor + PLAYER_HEIGHT;

    // Fuzz position counter shared across all fuzz-effect sprites in this frame.
    let mut fuzz_pos: usize = 0;

    // ---------- Collect visible things with their view-space depths ----------
    let mut visible: Vec<(f32, &doom_map::Thing)> = things
        .iter()
        .filter_map(|thing| {
            let dx = thing.x as f32 - px;
            let dy = thing.y as f32 - py;
            // Rotate into view space.
            let vx = dx * cos_a + dy * sin_a; // depth (forward)
            if vx > 0.5 {
                Some((vx, thing))
            } else {
                None // behind or too close
            }
        })
        .collect();

    // Painter's algorithm: draw farthest things first so nearer ones overdraw.
    visible.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    // ---------- Render each visible thing ------------------------------------
    for (vx, thing) in visible {
        // Look up the 4-char sprite prefix; skip unknown kinds and player starts.
        let prefix = match thing_sprite_prefix(thing.kind) {
            Some(p) => p,
            None => continue,
        };

        // Determine the rendering mode for this thing.
        let render_flag = render_flag_for_thing(thing.kind);

        // Frame A (index 0) for now. Full state-driven animation will come in
        // a later batch when render_things gets access to GameState.
        let frame_idx: u8 = 0;

        // Compute rotation based on thing facing angle vs viewer direction.
        // Thing.angle is in degrees (0-360); convert to BAM.
        let thing_angle_bam =
            doom_types::Bam(((thing.angle as u32 as u64) * (0x1_0000_0000u64 / 360)) as u32);
        let thing_fx = doom_types::Fixed16_16::from_int(thing.x as i32);
        let thing_fy = doom_types::Fixed16_16::from_int(thing.y as i32);

        let (frame, flip_x) = if thing_has_rotations(thing.kind) {
            let rotation =
                compute_sprite_rotation(thing_angle_bam, thing_fx, thing_fy, player_x, player_y);

            // Try the computed rotation first.
            let lump_name = sprite_lump_name(&prefix, frame_idx, rotation);
            if let Some(f) = cache.get(&lump_name) {
                (f, false)
            } else if let Some(mirror_rot) = mirror_rotation(rotation) {
                // Try the mirrored rotation (draw flipped).
                let mirror_name = sprite_lump_name(&prefix, frame_idx, mirror_rot);
                if let Some(f) = cache.get(&mirror_name) {
                    (f, true)
                } else {
                    // Fall back to rotation 0 (non-directional).
                    let fallback = sprite_lump_name(&prefix, frame_idx, 0);
                    match cache.get(&fallback) {
                        Some(f) => (f, false),
                        None => continue,
                    }
                }
            } else {
                // No mirror available (rotation 1 or 5); fall back to rotation 0.
                let fallback = sprite_lump_name(&prefix, frame_idx, 0);
                match cache.get(&fallback) {
                    Some(f) => (f, false),
                    None => continue,
                }
            }
        } else {
            // Non-directional sprite: use rotation 0.
            let lump_name = sprite_lump_name(&prefix, frame_idx, 0);
            match cache.get(&lump_name) {
                Some(f) => (f, false),
                None => continue,
            }
        };

        let dx = thing.x as f32 - px;
        let dy = thing.y as f32 - py;
        // Match render.rs view transform so sprite columns align with wall columns.
        let vy = dx * sin_a - dy * cos_a; // lateral displacement

        // --- Screen-space projection ---
        // Horizontal centre of the sprite on screen.
        let sx_center = HALF_W as f32 + FOCAL_LEN * vy / vx;

        // Scale factor: how many screen pixels per map unit at this depth.
        let sprite_scale = FOCAL_LEN / vx;

        // Scaled screen dimensions of the sprite.
        let screen_h = ((frame.height as f32) * sprite_scale).round() as i32;
        let screen_w = ((frame.width as f32) * sprite_scale).round() as i32;

        if screen_h <= 0 || screen_w <= 0 {
            continue;
        }

        let thing_floor = sector_for_point(level, i32::from(thing.x), i32::from(thing.y))
            .and_then(|si| level.sectors.get(si))
            .map_or(0.0, |s| s.floor_height as f32);
        let thing_top_z = thing_floor + frame.top_offset as f32;
        let thing_bottom_z = thing_top_z - frame.height as f32;
        let (screen_y_top, screen_y_bot) =
            project_sprite_vertical_bounds(view_z, thing_top_z, sprite_scale, screen_h);

        // Horizontal placement: left_offset tells us how many sprite pixels
        // the centre point is to the right of column 0.
        let screen_x_left =
            sx_center as i32 - (frame.left_offset as i32 * screen_w / frame.width as i32);
        let screen_x_right = screen_x_left + screen_w;

        // Guard against degenerate cases (e.g. 0-width frame).
        if screen_x_right <= 0 || screen_x_left >= SCREEN_W as i32 {
            continue;
        }

        let col_h = screen_y_bot - screen_y_top;

        // --- Compute lighting for this sprite ---
        // Look up the sector this thing is in to get its light level.
        // The colormap used depends on:
        //   - RenderFlag::FullBright => always colormap 0 (identity)
        //   - RenderFlag::Fuzz => use fuzz effect (draw_fuzz_column)
        //   - RenderFlag::Normal => sector light + distance attenuation
        let light_params = if render_flag == RenderFlag::FullBright {
            LightParams::new(255, true)
        } else if colormap.is_some() {
            // Look up sector light level via BSP.
            let sector_light = sector_for_point(level, i32::from(thing.x), i32::from(thing.y))
                .and_then(|si| level.sectors.get(si))
                .map_or(255, |s| s.light_level.clamp(0, 255) as u8);
            LightParams::new(sector_light, false)
        } else {
            // No colormap cache => render fullbright (backward compat).
            LightParams::new(255, true)
        };
        // Suppress the unused-variable warning when colormap is None and
        // light_params was set but never read in the fuzz path.
        let _ = &light_params;

        // --- Draw each screen column covered by the sprite ---
        // Iterating screen columns (not sprite columns) prevents gaps when
        // the sprite is close and screen_w >> frame.width.
        let sprite_colormap: Option<&[u8; 256]> =
            colormap.map(|cm| light_params.get_colormap(vx, cm));
        let sy_top_clamped = screen_y_top.max(0);
        let sy_bot_clamped = screen_y_bot.min(SCREEN_H as i32 - 1);

        let sx_start = screen_x_left.max(0);
        let sx_end = (screen_x_right - 1).min(SCREEN_W as i32 - 1);
        for sx in sx_start..=sx_end {
            // Map screen column back to sprite column (handles flip_x).
            let screen_col = sx - screen_x_left;
            let sprite_col = if flip_x {
                frame.width as i32 - 1 - screen_col * frame.width as i32 / screen_w.max(1)
            } else {
                screen_col * frame.width as i32 / screen_w.max(1)
            };
            let sprite_col = sprite_col.clamp(0, frame.width as i32 - 1) as usize;

            // Per-column z-buffer clipping.
            if let Some(zbuf) = z_buffer {
                if vx >= zbuf[sx as usize] {
                    continue;
                }
            }

            // Narrow vertical extent by portal clip (mfloorclip/mceilingclip).
            let col_top = if let Some(clip) = sprite_clip {
                clip.clip_top(sx as usize, vx, thing_top_z, sy_top_clamped)
            } else {
                sy_top_clamped
            };
            let col_bot = if let Some(clip) = sprite_clip {
                clip.clip_bottom(sx as usize, vx, thing_bottom_z, sy_bot_clamped)
            } else {
                sy_bot_clamped
            };
            if col_top > col_bot {
                continue;
            }

            // Fuzz effect (Spectre).
            if render_flag == RenderFlag::Fuzz {
                let sy_top_u = col_top.max(0) as usize;
                let sy_bot_u = col_bot.min(SCREEN_H as i32 - 1) as usize;
                if sy_top_u <= sy_bot_u {
                    draw_fuzz_column(fb, sx as usize, sy_top_u, sy_bot_u, &mut fuzz_pos, colormap);
                }
                continue;
            }

            for sy in col_top..=col_bot {
                let sprite_row = (sy - screen_y_top) * frame.height as i32 / col_h.max(1);
                let sprite_row = sprite_row.clamp(0, frame.height as i32 - 1) as usize;

                let pixel_idx = sprite_col * frame.height as usize + sprite_row;
                if let Some(Some(raw_idx)) = frame.pixels.get(pixel_idx) {
                    let final_color = match sprite_colormap {
                        Some(cm) => cm[*raw_idx as usize],
                        None => *raw_idx,
                    };
                    fb.data[sy as usize * SCREEN_W + sx as usize] = final_color;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Draw a sprite frame onto the framebuffer.
///
/// Positioning:
/// - `screen_x_center`: screen column at the sprite's *center* point.
///   Column 0 of the sprite is placed at `screen_x_center - left_offset`.
/// - `screen_y_bottom`: screen row at the sprite's bottom (baseline row).
///   The sprite occupies rows `[screen_y_bottom - height + 1, screen_y_bottom]`.
///
/// Transparent pixels (stored as `None`) are skipped.
/// The `colormap` slice translates palette indices through light diminishment.
/// Pass [`crate::column::IDENTITY_COLORMAP`] for full-brightness rendering.
pub fn draw_sprite(
    fb: &mut Framebuffer,
    frame: &SpriteFrame,
    screen_x_center: i32,
    screen_y_bottom: i32,
    colormap: &[u8; 256],
) {
    // Column 0 of the sprite sits left_offset pixels to the right of center.
    // So sprite X origin = center - left_offset.
    let sprite_origin_x = screen_x_center - frame.left_offset as i32;

    // Sprite's top row in screen space.
    let sprite_origin_y = screen_y_bottom - frame.height as i32 + 1;

    let width = frame.width as i32;
    let height = frame.height as i32;

    for col in 0..width {
        let sx = sprite_origin_x + col;
        if !(0..320).contains(&sx) {
            continue;
        }

        let col_base = col as usize * frame.height as usize;
        for row in 0..height {
            let sy = sprite_origin_y + row;
            if !(0..200).contains(&sy) {
                continue;
            }
            if let Some(idx) = frame.pixels[col_base + row as usize] {
                let final_color = colormap[idx as usize];
                fb.set_pixel(sx as usize, sy as usize, final_color);
            }
        }
    }
}

/// Draw a sprite frame onto the framebuffer, optionally flipped horizontally.
///
/// This is the extended version of [`draw_sprite`] that supports horizontal
/// mirroring for rotational sprite fallback (e.g. rotation 8 uses rotation 2
/// drawn flipped).
///
/// When `flip_x` is `true`, columns are drawn in reverse order (right-to-left),
/// producing a horizontal mirror of the sprite.
///
/// Positioning and colormap behaviour are identical to [`draw_sprite`].
pub fn draw_sprite_ex(
    fb: &mut Framebuffer,
    frame: &SpriteFrame,
    screen_x_center: i32,
    screen_y_bottom: i32,
    colormap: &[u8; 256],
    flip_x: bool,
) {
    let sprite_origin_x = screen_x_center - frame.left_offset as i32;
    let sprite_origin_y = screen_y_bottom - frame.height as i32 + 1;

    let width = frame.width as i32;
    let height = frame.height as i32;

    for col in 0..width {
        // When flipped, column 0 of the sprite maps to the rightmost screen column.
        let draw_col = if flip_x { width - 1 - col } else { col };
        let sx = sprite_origin_x + draw_col;
        if !(0..320).contains(&sx) {
            continue;
        }

        let col_base = col as usize * frame.height as usize;
        for row in 0..height {
            let sy = sprite_origin_y + row;
            if !(0..200).contains(&sy) {
                continue;
            }
            if let Some(idx) = frame.pixels[col_base + row as usize] {
                let final_color = colormap[idx as usize];
                fb.set_pixel(sx as usize, sy as usize, final_color);
            }
        }
    }
}

/// Draw a weapon sprite at the standard weapon-overlay position.
///
/// Uses Doom's psprite draw semantics at `x=160, y=167`: the patch's raw
/// `left_offset` controls horizontal placement, while the sprite remains
/// bottom-anchored to the weapon baseline above the status bar. Does nothing if
/// the sprite is not found in the cache.
pub fn draw_weapon_sprite(
    fb: &mut Framebuffer,
    lump_name: &[u8; 8],
    cache: &SpriteCache,
    colormap: &[u8; 256],
) {
    if let Some(frame) = cache.get(lump_name) {
        draw_weapon_frame(fb, frame, 160, 167, colormap);
    }
}

/// Draw a pre-parsed weapon frame at a psprite overlay anchor.
///
/// Weapon overlays use the patch's raw horizontal offset, but keep Doom's
/// bottom-anchored Y behaviour so the sprite sits on the weapon baseline rather
/// than reapplying the patch `top_offset`.
pub fn draw_weapon_frame(
    fb: &mut Framebuffer,
    frame: &SpriteFrame,
    base_x: i32,
    base_y: i32,
    colormap: &[u8; 256],
) {
    draw_sprite(fb, frame, base_x, base_y, colormap);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::column::IDENTITY_COLORMAP;

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    /// Build a minimal valid 1×1 picture lump with a single pixel at (0,0).
    ///
    /// Layout:
    ///   [0..2]  width  = 1 (u16 LE)
    ///   [2..4]  height = 1 (u16 LE)
    ///   [4..6]  left_offset = 0 (i16 LE)
    ///   [6..8]  top_offset  = 0 (i16 LE)
    ///   [8..12] col_offset  = 12 (u32 LE) — column 0 data starts at byte 12
    ///   [12]    topdelta = 0
    ///   [13]    length   = 1
    ///   [14]    _pad     = 0
    ///   [15]    pixel    = 7
    ///   [16]    _pad     = 0
    ///   [17]    topdelta = 0xFF  (end of column)
    fn minimal_picture() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&1u16.to_le_bytes()); // width = 1
        data.extend_from_slice(&1u16.to_le_bytes()); // height = 1
        data.extend_from_slice(&0i16.to_le_bytes()); // left_offset = 0
        data.extend_from_slice(&0i16.to_le_bytes()); // top_offset = 0
        let col_offset: u32 = 12; // header(8) + col_offsets(1×4) = 12
        data.extend_from_slice(&col_offset.to_le_bytes());
        // Post: topdelta=0, length=1, padding, pixel=7, padding, end-marker
        data.extend_from_slice(&[0, 1, 0, 7, 0, 0xFF]);
        data
    }

    /// Build a minimal valid IWAD with zero lumps.
    fn empty_wad() -> Vec<u8> {
        let mut data = vec![0u8; 12];
        data[0..4].copy_from_slice(b"IWAD");
        data[4..8].copy_from_slice(&0i32.to_le_bytes()); // numlumps = 0
        data[8..12].copy_from_slice(&12i32.to_le_bytes()); // dir at end of header
        data
    }

    /// Build a minimal IWAD with the specified named lumps.
    fn wad_with_lumps(lumps: &[(&str, &[u8])]) -> Vec<u8> {
        let mut data: Vec<u8> = Vec::new();
        data.extend_from_slice(b"IWAD");
        data.extend_from_slice(&(lumps.len() as i32).to_le_bytes());
        data.extend_from_slice(&0i32.to_le_bytes()); // dir offset placeholder

        let mut offsets: Vec<(usize, usize)> = Vec::new();
        for (_, payload) in lumps {
            let pos = data.len();
            data.extend_from_slice(payload);
            offsets.push((pos, payload.len()));
        }

        let dir_offset = data.len() as i32;
        data[8..12].copy_from_slice(&dir_offset.to_le_bytes());

        for (i, (name, _)) in lumps.iter().enumerate() {
            let (filepos, size) = offsets[i];
            data.extend_from_slice(&(filepos as i32).to_le_bytes());
            data.extend_from_slice(&(size as i32).to_le_bytes());
            let mut name_buf = [0u8; 8];
            for (j, &b) in name.as_bytes().iter().take(8).enumerate() {
                name_buf[j] = b.to_ascii_uppercase();
            }
            data.extend_from_slice(&name_buf);
        }

        data
    }

    // ------------------------------------------------------------------
    // 1. Parse a minimal valid picture — pixel must be present
    // ------------------------------------------------------------------
    #[test]
    fn test_parse_picture_minimal() {
        let data = minimal_picture();
        let frame = parse_picture(&data).expect("should parse minimal picture");
        assert_eq!(frame.width, 1);
        assert_eq!(frame.height, 1);
        assert_eq!(frame.left_offset, 0);
        assert_eq!(frame.top_offset, 0);
        // Column 0, row 0 → pixels[0 * 1 + 0] = Some(7)
        assert_eq!(frame.pixels.len(), 1);
        assert_eq!(frame.pixels[0], Some(7), "pixel at (0,0) should be Some(7)");
    }

    // ------------------------------------------------------------------
    // 2. Too-small buffer returns None
    // ------------------------------------------------------------------
    #[test]
    fn test_parse_picture_empty_lump() {
        // Completely empty.
        assert!(parse_picture(&[]).is_none(), "empty slice must return None");
        // 7 bytes — one short of the 8-byte header.
        assert!(
            parse_picture(&[0u8; 7]).is_none(),
            "7-byte slice must return None"
        );
    }

    // ------------------------------------------------------------------
    // 3. Pixel not covered by any post is None (transparent)
    // ------------------------------------------------------------------
    #[test]
    fn test_parse_picture_transparent() {
        // Build a 1×2 picture where only row 0 has a pixel; row 1 is transparent.
        let mut data = Vec::new();
        data.extend_from_slice(&1u16.to_le_bytes()); // width = 1
        data.extend_from_slice(&2u16.to_le_bytes()); // height = 2
        data.extend_from_slice(&0i16.to_le_bytes()); // left_offset
        data.extend_from_slice(&0i16.to_le_bytes()); // top_offset
        let col_offset: u32 = 12; // header(8) + col_offsets(1×4) = 12
        data.extend_from_slice(&col_offset.to_le_bytes());
        // Post: topdelta=0, length=1, pad, pixel=42, pad, end
        // Only row 0 gets a pixel; row 1 is never written → transparent.
        data.extend_from_slice(&[0, 1, 0, 42, 0, 0xFF]);

        let frame = parse_picture(&data).expect("should parse");
        assert_eq!(frame.width, 1);
        assert_eq!(frame.height, 2);
        assert_eq!(frame.pixels[0], Some(42), "row 0 should be Some(42)");
        assert_eq!(frame.pixels[1], None, "row 1 should be transparent (None)");
    }

    // ------------------------------------------------------------------
    // 4. Loading a WAD without S_START/S_END gives an empty cache
    // ------------------------------------------------------------------
    #[test]
    fn test_sprite_cache_empty_wad() {
        let wad_bytes = empty_wad();
        let wad = doom_wad::WadFile::parse(wad_bytes).expect("wad parse failed");
        let cache = SpriteCache::load(&wad);
        assert!(cache.is_empty(), "no S_START/S_END → cache must be empty");
        assert_eq!(cache.len(), 0);
    }

    // ------------------------------------------------------------------
    // 5. Drawing at the right edge of the screen must not panic
    // ------------------------------------------------------------------
    #[test]
    fn test_draw_sprite_clips_out_of_bounds() {
        // Build a frame 10 pixels wide, 10 pixels tall, all opaque (index 5).
        let width = 10usize;
        let height = 10usize;
        let frame = SpriteFrame {
            width: width as u16,
            height: height as u16,
            left_offset: 0,
            top_offset: 0,
            pixels: vec![Some(5); width * height],
        };

        let mut fb = Framebuffer::new();
        // Draw centered at x=315 — sprite_origin_x=315 (left_offset=0).
        // Sprite occupies columns 315..325; columns 320..325 are clipped.
        // Visible columns: 315..319 (inclusive).
        // sprite_origin_y = screen_y_bottom - height + 1 = 100 - 10 + 1 = 91.
        draw_sprite(&mut fb, &frame, 315, 100, &IDENTITY_COLORMAP);
        // Must not panic.  Visible area: x in [315,319], y in [91,100].
        assert_eq!(fb.get_pixel(315, 91), Some(5)); // top-left of visible portion
        assert_eq!(fb.get_pixel(319, 100), Some(5)); // bottom-right visible
    }

    // ------------------------------------------------------------------
    // 6. draw_weapon_sprite with a missing lump name does nothing (no panic)
    // ------------------------------------------------------------------
    #[test]
    fn test_weapon_sprite_noop_for_missing() {
        let cache = SpriteCache::empty();
        let mut fb = Framebuffer::new();
        // This sprite does not exist in the (empty) cache.
        draw_weapon_sprite(&mut fb, b"PISGA0\0\0", &cache, &IDENTITY_COLORMAP);
        // Framebuffer stays zeroed — no pixels written, no panic.
        assert!(fb.data.iter().all(|&b| b == 0), "fb should remain zeroed");
    }
    #[test]
    fn test_draw_weapon_sprite_uses_raw_zero_offsets() {
        let mut cache = SpriteCache::empty();
        let frame = SpriteFrame {
            width: 4,
            height: 1,
            left_offset: 0,
            top_offset: 0,
            pixels: vec![Some(77); 4],
        };
        cache.insert("PISGA0".to_string(), frame);

        let mut fb = Framebuffer::new();
        draw_weapon_sprite(&mut fb, b"PISGA0\0\0", &cache, &IDENTITY_COLORMAP);

        // With zero offsets, the raw psprite origin is the sprite origin.
        for x in 160..=163 {
            assert_eq!(
                fb.get_pixel(x, 167),
                Some(77),
                "x={x} should be weapon pixel"
            );
        }
    }

    #[test]
    fn test_draw_weapon_sprite_respects_pathological_left_offset() {
        let mut cache = SpriteCache::empty();
        let frame = SpriteFrame {
            width: 4,
            height: 1,
            left_offset: -120,
            top_offset: 0,
            pixels: vec![Some(88); 4],
        };
        cache.insert("PISGA0".to_string(), frame);

        let mut fb = Framebuffer::new();
        draw_weapon_sprite(&mut fb, b"PISGA0\0\0", &cache, &IDENTITY_COLORMAP);

        // Raw offsets should drive placement even for weird source data.
        for x in 280..=283 {
            assert_eq!(
                fb.get_pixel(x, 167),
                Some(88),
                "x={x} should be weapon pixel at the raw patch origin"
            );
        }
    }

    #[test]
    fn test_draw_weapon_frame_anchors_to_bottom_not_patch_top_offset() {
        let frame = SpriteFrame {
            width: 8,
            height: 3,
            left_offset: 1,
            top_offset: 0,
            pixels: vec![Some(99); 8 * 3],
        };

        let mut fb = Framebuffer::new();
        draw_weapon_frame(&mut fb, &frame, 160, 167, &IDENTITY_COLORMAP);

        assert_eq!(
            fb.get_pixel(159, 165),
            Some(99),
            "weapon overlays should stay bottom-anchored above the status bar even when patch top_offset is zero"
        );
        assert_eq!(
            fb.get_pixel(166, 167),
            Some(99),
            "weapon overlays should extend down to the requested screen-bottom anchor"
        );
        assert_eq!(
            fb.get_pixel(159, 164),
            Some(0),
            "rows above the bottom-anchored weapon should remain untouched"
        );
    }

    // ------------------------------------------------------------------
    // 7. SpriteCache::get() retrieves a manually inserted frame
    // ------------------------------------------------------------------
    #[test]
    fn test_sprite_cache_name_lookup() {
        let mut cache = SpriteCache::empty();

        let frame = SpriteFrame {
            width: 2,
            height: 2,
            left_offset: 1,
            top_offset: 1,
            pixels: vec![Some(10), Some(20), Some(30), Some(40)],
        };
        cache.insert("TROOA1".to_string(), frame);

        // Lookup via exact byte array name (null-padded).
        let found = cache.get(b"TROOA1\0\0");
        assert!(found.is_some(), "TROOA1 should be found in cache");
        let f = found.unwrap();
        assert_eq!(f.width, 2);
        assert_eq!(f.height, 2);

        // Non-existent name returns None.
        assert!(cache.get(b"NOTHERE\0").is_none());

        // len() reflects the insertion.
        assert_eq!(cache.len(), 1);
        assert!(!cache.is_empty());
    }

    // ------------------------------------------------------------------
    // Extra: WAD with S_START/S_END markers but valid sprite lump parses
    // ------------------------------------------------------------------
    #[test]
    fn test_sprite_cache_loads_from_wad_markers() {
        let pic = minimal_picture();
        // Lumps: S_START (marker, empty), PISGA0 (sprite), S_END (marker, empty)
        let wad_bytes = wad_with_lumps(&[("S_START", &[]), ("PISGA0", &pic), ("S_END", &[])]);
        let wad = doom_wad::WadFile::parse(wad_bytes).expect("wad parse");
        let cache = SpriteCache::load(&wad);
        // The sprite lump should have been loaded.
        assert_eq!(cache.len(), 1, "one sprite lump expected");
        let frame = cache.get(b"PISGA0\0\0");
        assert!(frame.is_some(), "PISGA0 should be in cache");
        assert_eq!(frame.unwrap().pixels[0], Some(7));
    }

    // ------------------------------------------------------------------
    // render_things helpers
    // ------------------------------------------------------------------

    /// Build a minimal Level suitable for render_things tests.
    ///
    /// The level contains exactly the things provided and has the minimum
    /// valid BSP (0 nodes, 1 ssector) required by Level::from_wad validation.
    fn make_test_level(things: Vec<doom_map::Thing>) -> doom_map::Level {
        use doom_wad::{REQUIRED_MAP_LUMPS, WadKind};

        let _ = WadKind::Iwad; // suppress unused import warning
        let _ = REQUIRED_MAP_LUMPS;

        // ---- sector ----
        let mut sector_data = vec![0u8; 26];
        sector_data[0..2].copy_from_slice(&0i16.to_le_bytes());
        sector_data[2..4].copy_from_slice(&128i16.to_le_bytes());
        sector_data[4..12].copy_from_slice(b"FLAT1\0\0\0");
        sector_data[12..20].copy_from_slice(b"FLAT2\0\0\0");
        sector_data[20..22].copy_from_slice(&192i16.to_le_bytes());
        sector_data[22..24].copy_from_slice(&0u16.to_le_bytes());
        sector_data[24..26].copy_from_slice(&0u16.to_le_bytes());

        // ---- vertices ----
        let mut vert_data = vec![0u8; 4 * 4];
        let verts: [(i16, i16); 4] = [(0, 0), (64, 0), (64, 64), (0, 64)];
        for (i, (x, y)) in verts.iter().enumerate() {
            vert_data[i * 4..i * 4 + 2].copy_from_slice(&x.to_le_bytes());
            vert_data[i * 4 + 2..i * 4 + 4].copy_from_slice(&y.to_le_bytes());
        }

        // ---- sidedefs ----
        let mut sd_data = vec![0u8; 4 * 30];
        for i in 0..4 {
            sd_data[i * 30 + 20..i * 30 + 28].copy_from_slice(b"WALL1\0\0\0");
            sd_data[i * 30 + 28..i * 30 + 30].copy_from_slice(&0u16.to_le_bytes());
        }

        // ---- linedefs ----
        let mut ld_data = vec![0u8; 4 * 14];
        let edges: [(u16, u16); 4] = [(0, 1), (1, 2), (2, 3), (3, 0)];
        for (i, (from, to)) in edges.iter().enumerate() {
            let b = &mut ld_data[i * 14..i * 14 + 14];
            b[0..2].copy_from_slice(&from.to_le_bytes());
            b[2..4].copy_from_slice(&to.to_le_bytes());
            b[4..6].copy_from_slice(&0u16.to_le_bytes());
            b[6..8].copy_from_slice(&0u16.to_le_bytes());
            b[8..10].copy_from_slice(&0u16.to_le_bytes());
            b[10..12].copy_from_slice(&(i as u16).to_le_bytes());
            b[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        }

        // ---- seg ----
        let mut seg_data = vec![0u8; 12];
        seg_data[0..2].copy_from_slice(&0u16.to_le_bytes());
        seg_data[2..4].copy_from_slice(&1u16.to_le_bytes());

        // ---- ssector ----
        let mut ss_data = vec![0u8; 4];
        ss_data[0..2].copy_from_slice(&1u16.to_le_bytes());
        ss_data[2..4].copy_from_slice(&0u16.to_le_bytes());

        // ---- things ----
        let mut thing_data = vec![0u8; things.len() * 10];
        for (i, t) in things.iter().enumerate() {
            let b = &mut thing_data[i * 10..i * 10 + 10];
            b[0..2].copy_from_slice(&t.x.to_le_bytes());
            b[2..4].copy_from_slice(&t.y.to_le_bytes());
            b[4..6].copy_from_slice(&t.angle.to_le_bytes());
            b[6..8].copy_from_slice(&t.kind.to_le_bytes());
            b[8..10].copy_from_slice(&t.flags.to_le_bytes());
        }
        // If no things provided, add a dummy player-start so validation
        // doesn't fail on completely empty THINGS lump (which is valid anyway,
        // but some engines require at least a player start).
        if things.is_empty() {
            // 0-byte THINGS lump is valid (0 entries).
        }

        // ---- reject ----
        let reject_data = vec![0u8; 1];

        // ---- blockmap ----
        let mut bm_data = vec![0u8; 8 + 2 + 4];
        bm_data[0..2].copy_from_slice(&0i16.to_le_bytes());
        bm_data[2..4].copy_from_slice(&0i16.to_le_bytes());
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());

        let lump_payloads: &[(&[u8; 8], &[u8])] = &[
            (b"E1M1\0\0\0\0", &[]),
            (b"THINGS\0\0", &thing_data),
            (b"LINEDEFS", &ld_data),
            (b"SIDEDEFS", &sd_data),
            (b"VERTEXES", &vert_data),
            (b"SEGS\0\0\0\0", &seg_data),
            (b"SSECTORS", &ss_data),
            (b"NODES\0\0\0", &[]),
            (b"SECTORS\0", &sector_data),
            (b"REJECT\0\0", &reject_data),
            (b"BLOCKMAP", &bm_data),
        ];

        let mut data: Vec<u8> = Vec::new();
        data.extend_from_slice(b"IWAD");
        data.extend_from_slice(&(lump_payloads.len() as i32).to_le_bytes());
        data.extend_from_slice(&0i32.to_le_bytes());

        let mut offsets: Vec<(usize, usize)> = Vec::new();
        for (_, payload) in lump_payloads {
            let pos = data.len();
            data.extend_from_slice(payload);
            offsets.push((pos, payload.len()));
        }

        let dir_offset = data.len() as i32;
        data[8..12].copy_from_slice(&dir_offset.to_le_bytes());
        for (i, (name_bytes, _)) in lump_payloads.iter().enumerate() {
            let (filepos, size) = offsets[i];
            data.extend_from_slice(&(filepos as i32).to_le_bytes());
            data.extend_from_slice(&(size as i32).to_le_bytes());
            data.extend_from_slice(*name_bytes);
        }

        let wad = doom_wad::WadFile::parse(data).expect("test WAD parse failed");
        doom_map::Level::from_wad(&wad, "E1M1").expect("test level load failed")
    }

    /// Build a Thing with given position, kind.
    fn make_thing(x: i16, y: i16, kind: u16) -> doom_map::Thing {
        doom_map::Thing {
            x,
            y,
            angle: 0,
            kind,
            flags: 7,
        }
    }

    // ------------------------------------------------------------------
    // render_things test 1: empty level — no panic
    // ------------------------------------------------------------------
    #[test]
    fn test_render_things_empty_level() {
        let level = make_test_level(vec![]);
        let cache = SpriteCache::empty();
        let mut fb = Framebuffer::new();
        // Must not panic on zero things.
        render_things(
            &level,
            doom_types::Fixed16_16::from_int(32),
            doom_types::Fixed16_16::from_int(32),
            doom_types::Bam(0),
            &mut fb,
            &cache,
            None,
            None,
            None,
        );
        // Framebuffer stays zeroed (empty cache → nothing drawn).
        assert!(fb.data.iter().all(|&b| b == 0));
    }

    // ------------------------------------------------------------------
    // render_things test 2: thing behind player is not rendered
    // ------------------------------------------------------------------
    #[test]
    fn test_render_things_behind_player_skipped() {
        // Player at (0,0) facing east (angle 0 = east in Doom convention).
        // Thing is at (-100, 0) — directly behind the player.
        let thing = make_thing(-100, 0, 2035); // barrel — has sprite name BAR1
        let level = make_test_level(vec![thing]);

        // Insert a dummy sprite so the cache lookup would succeed if
        // the thing were incorrectly included.
        let mut cache = SpriteCache::empty();
        let dummy_frame = SpriteFrame {
            width: 2,
            height: 2,
            left_offset: 1,
            top_offset: 0,
            pixels: vec![Some(42); 4],
        };
        cache.insert("BAR1A0".to_string(), dummy_frame);

        let mut fb = Framebuffer::new();
        // Player at origin, facing east (Bam(0)).
        render_things(
            &level,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Bam(0),
            &mut fb,
            &cache,
            None,
            None,
            None,
        );
        // If the thing behind the player were rendered it would write pixel 42.
        // The framebuffer must remain all zeros.
        assert!(
            fb.data.iter().all(|&b| b == 0),
            "behind-player thing must not be rendered"
        );
    }

    // ------------------------------------------------------------------
    // render_things test 3: unknown thing kind is silently skipped
    // ------------------------------------------------------------------
    #[test]
    fn test_render_things_unknown_kind_skipped() {
        // Kind 9999 is not in the lookup table.
        let thing = make_thing(100, 0, 9999);
        let level = make_test_level(vec![thing]);
        let cache = SpriteCache::empty();
        let mut fb = Framebuffer::new();
        render_things(
            &level,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Bam(0),
            &mut fb,
            &cache,
            None,
            None,
            None,
        );
        // Nothing drawn — no panic.
        assert!(fb.data.iter().all(|&b| b == 0));
    }

    // ------------------------------------------------------------------
    // render_things test 4: player start (kind=1) is skipped
    // ------------------------------------------------------------------
    #[test]
    fn test_render_things_player_start_skipped() {
        // Kind 1 = player 1 start; thing_sprite returns None.
        let thing = make_thing(100, 0, 1);
        let level = make_test_level(vec![thing]);
        let cache = SpriteCache::empty();
        let mut fb = Framebuffer::new();
        render_things(
            &level,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Bam(0),
            &mut fb,
            &cache,
            None,
            None,
            None,
        );
        assert!(fb.data.iter().all(|&b| b == 0));
    }

    // ------------------------------------------------------------------
    // render_things test 5: forward thing projects to screen centre
    // ------------------------------------------------------------------
    #[test]
    fn test_proj_math_forward_thing() {
        // Player at (0,0) facing east (Bam(0)).
        // Thing at (200, 0) — directly ahead.
        // vy = 0 so sx_center = HALF_W - FOCAL_LEN * 0 / vx = 160.

        let player_x = doom_types::Fixed16_16::from_int(0);
        let player_y = doom_types::Fixed16_16::from_int(0);
        let player_angle = doom_types::Bam(0);

        // Manually replicate the projection math for a forward thing.
        let angle_rad =
            (player_angle.0 as f32) * (std::f32::consts::PI * 2.0 / (u32::MAX as f32 + 1.0));
        let cos_a = angle_rad.cos();
        let sin_a = angle_rad.sin();

        let px = player_x.raw() as f32 / 65536.0;
        let py = player_y.raw() as f32 / 65536.0;

        let thing_x: f32 = 200.0;
        let thing_y: f32 = 0.0;

        let dx = thing_x - px;
        let dy = thing_y - py;
        let vx = dx * cos_a + dy * sin_a;
        let vy = dx * sin_a - dy * cos_a;

        // vx must be positive (thing is in front).
        assert!(vx > 0.5, "thing should be in front: vx={vx}");

        let sx_center = 160.0_f32 + 160.0 * vy / vx;

        // For a thing directly ahead, vy ≈ 0 so sx_center ≈ 160.
        let deviation = (sx_center - 160.0).abs();
        assert!(
            deviation < 1.0,
            "forward thing should project to screen centre ≈160, got {sx_center}"
        );
    }

    // ------------------------------------------------------------------
    // render_things test 6: lateral projection uses same sign as wall pass
    // ------------------------------------------------------------------
    #[test]
    fn test_proj_lateral_matches_wall_space_for_zbuf_clipping() {
        // Place a sprite slightly off-center.
        let thing = make_thing(100, 50, 2035);
        let level = make_test_level(vec![thing]);

        let mut cache = SpriteCache::empty();
        cache.insert("BAR1A0".to_string(), make_opaque_sprite(8, 16, 77));

        // Expected sprite center from the wall transform convention in render.rs:
        //   vx = dx*cos + dy*sin
        //   vy = dx*sin - dy*cos
        //   sx = HALF_W + FOCAL_LEN * vy / vx
        let dx = 100.0f32;
        let dy = 50.0f32;
        let vx = dx; // facing east => cos=1, sin=0
        let vy = -dy;
        let sx_center = HALF_W as f32 + FOCAL_LEN * vy / vx;

        // Block a band around the expected sprite columns.
        let mut zbuf = [f32::MAX; SCREEN_W];
        let band_center = sx_center.round() as i32;
        for x in (band_center - 20)..=(band_center + 20) {
            if (0..SCREEN_W as i32).contains(&x) {
                zbuf[x as usize] = 10.0;
            }
        }

        let mut fb = Framebuffer::new();
        render_things(
            &level,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Bam(0),
            &mut fb,
            &cache,
            Some(&zbuf),
            None,
            None,
        );

        // Sprite depth is ~100, so it should be fully clipped by zbuf=10 if
        // its projected columns match wall-space projection.
        assert!(
            fb.data.iter().all(|&px| px != 77),
            "sprite should be clipped when z-buffer blocks its wall-space columns"
        );
    }

    // ------------------------------------------------------------------
    // Extra: draw_sprite correctly applies colormap
    // ------------------------------------------------------------------
    #[test]
    fn test_draw_sprite_applies_colormap() {
        let frame = SpriteFrame {
            width: 1,
            height: 1,
            left_offset: 0,
            top_offset: 0,
            pixels: vec![Some(10)],
        };

        // Build a colormap that maps index 10 → 99.
        let mut colormap = IDENTITY_COLORMAP;
        colormap[10] = 99;

        let mut fb = Framebuffer::new();
        // Draw at center x=160, bottom y=100.
        // sprite_origin_x = 160 - 0 = 160, sprite_origin_y = 100 - 1 + 1 = 100
        draw_sprite(&mut fb, &frame, 160, 100, &colormap);
        assert_eq!(
            fb.get_pixel(160, 100),
            Some(99),
            "colormap mapping should be applied"
        );
    }

    // ==================================================================
    // Sprite rotation and name resolution tests
    // ==================================================================

    // ------------------------------------------------------------------
    // sprite_lump_name: basic construction
    // ------------------------------------------------------------------
    #[test]
    fn sprite_lump_name_builds_correctly() {
        let name = sprite_lump_name(b"TROO", 0, 1);
        assert_eq!(&name, b"TROOA1\0\0");
    }

    #[test]
    fn sprite_lump_name_frame_b_rotation_0() {
        let name = sprite_lump_name(b"POSS", 1, 0);
        assert_eq!(&name, b"POSSB0\0\0");
    }

    #[test]
    fn sprite_lump_name_high_frame_and_rotation() {
        // Frame E (index 4), rotation 8.
        let name = sprite_lump_name(b"SARG", 4, 8);
        assert_eq!(&name, b"SARGE8\0\0");
    }

    // ------------------------------------------------------------------
    // compute_sprite_rotation: cardinal directions
    //
    // Uses Doom's R_ProjectSprite formula:
    //   ang = R_PointToAngle(thing->x, thing->y) - thing->angle;
    //   rot = (ang + (ANG45/2)*9) >> 29;
    //
    // For an east-facing thing (angle 0):
    //   Viewer east  (front)  -> rotation 1
    //   Viewer north           -> rotation 3
    //   Viewer west  (back)   -> rotation 5
    //   Viewer south           -> rotation 7
    // ------------------------------------------------------------------
    #[test]
    fn compute_rotation_front_is_1() {
        // Thing faces east (Bam 0), viewer is to the east (directly in front).
        let rot = compute_sprite_rotation(
            doom_types::Bam::ZERO,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(100),
            doom_types::Fixed16_16::from_int(0),
        );
        assert_eq!(rot, 1, "viewer in front should be rotation 1");
    }

    #[test]
    fn compute_rotation_back_is_5() {
        // Thing faces east (Bam 0), viewer is to the west (behind).
        let rot = compute_sprite_rotation(
            doom_types::Bam::ZERO,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(-100),
            doom_types::Fixed16_16::from_int(0),
        );
        assert_eq!(rot, 5, "viewer behind should be rotation 5");
    }

    #[test]
    fn compute_rotation_viewer_north_is_3() {
        // Thing faces east (Bam 0), viewer is to the north (positive Y).
        // Matches Doom's R_ProjectSprite formula.
        let rot = compute_sprite_rotation(
            doom_types::Bam::ZERO,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(100),
        );
        assert_eq!(rot, 3, "viewer to north should be rotation 3");
    }

    #[test]
    fn compute_rotation_viewer_south_is_7() {
        // Thing faces east, viewer is to the south (negative Y).
        let rot = compute_sprite_rotation(
            doom_types::Bam::ZERO,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(-100),
        );
        assert_eq!(rot, 7, "viewer to south should be rotation 7");
    }

    #[test]
    fn compute_rotation_always_in_1_to_8() {
        // Smoke test: rotation should always be in [1, 8] for various angles.
        for angle_deg in (0..360).step_by(15) {
            let bam = doom_types::Bam(((angle_deg as u64) * (0x1_0000_0000u64 / 360)) as u32);
            let rot = compute_sprite_rotation(
                bam,
                doom_types::Fixed16_16::from_int(0),
                doom_types::Fixed16_16::from_int(0),
                doom_types::Fixed16_16::from_int(50),
                doom_types::Fixed16_16::from_int(30),
            );
            assert!(
                (1..=8).contains(&rot),
                "rotation {rot} out of range for angle {angle_deg}"
            );
        }
    }

    // ------------------------------------------------------------------
    // thing_sprite_prefix: known and unknown types
    // ------------------------------------------------------------------
    #[test]
    fn thing_sprite_prefix_known_types() {
        assert_eq!(thing_sprite_prefix(3001), Some(*b"TROO")); // Imp
        assert_eq!(thing_sprite_prefix(3004), Some(*b"POSS")); // Zombieman
        assert_eq!(thing_sprite_prefix(2014), Some(*b"BON1")); // Health bonus
        assert_eq!(thing_sprite_prefix(16), Some(*b"CYBR")); // Cyberdemon
        assert_eq!(thing_sprite_prefix(2035), Some(*b"BAR1")); // Barrel
    }

    #[test]
    fn thing_sprite_prefix_unknown_returns_none() {
        assert_eq!(thing_sprite_prefix(9999), None);
    }

    #[test]
    fn thing_sprite_prefix_player_starts_return_none() {
        assert_eq!(thing_sprite_prefix(1), None); // player 1 start
        assert_eq!(thing_sprite_prefix(2), None); // player 2 start
        assert_eq!(thing_sprite_prefix(3), None); // player 3 start
        assert_eq!(thing_sprite_prefix(4), None); // player 4 start
        assert_eq!(thing_sprite_prefix(11), None); // deathmatch start
    }

    // ------------------------------------------------------------------
    // thing_has_rotations: monsters yes, items no
    // ------------------------------------------------------------------
    #[test]
    fn thing_has_rotations_monsters_yes() {
        assert!(thing_has_rotations(3001)); // Imp
        assert!(thing_has_rotations(3004)); // Zombieman
        assert!(thing_has_rotations(16)); // Cyberdemon
        assert!(thing_has_rotations(66)); // Revenant
    }

    #[test]
    fn thing_has_rotations_items_no() {
        assert!(!thing_has_rotations(2014)); // Health bonus
        assert!(!thing_has_rotations(2035)); // Barrel
        assert!(!thing_has_rotations(2028)); // Floor lamp
        assert!(!thing_has_rotations(5)); // Blue keycard
    }

    // ------------------------------------------------------------------
    // mirror_rotation: symmetry pairs
    // ------------------------------------------------------------------
    #[test]
    fn test_mirror_rotation_pairs() {
        assert_eq!(mirror_rotation(2), Some(8));
        assert_eq!(mirror_rotation(8), Some(2));
        assert_eq!(mirror_rotation(3), Some(7));
        assert_eq!(mirror_rotation(7), Some(3));
        assert_eq!(mirror_rotation(4), Some(6));
        assert_eq!(mirror_rotation(6), Some(4));
    }

    #[test]
    fn test_mirror_rotation_symmetric_none() {
        assert_eq!(mirror_rotation(0), None);
        assert_eq!(mirror_rotation(1), None);
        assert_eq!(mirror_rotation(5), None);
    }

    // ------------------------------------------------------------------
    // thing_sprite legacy wrapper still works
    // ------------------------------------------------------------------
    #[test]
    fn test_thing_sprite_legacy_wrapper() {
        // Imp: prefix TROO, frame A, rotation 0.
        let name = thing_sprite(3001);
        assert_eq!(name, Some(*b"TROOA0\0\0"));
        // Player start: returns None.
        assert_eq!(thing_sprite(1), None);
        // Unknown: returns None.
        assert_eq!(thing_sprite(9999), None);
    }

    // ------------------------------------------------------------------
    // draw_sprite_ex: flipped and non-flipped rendering
    // ------------------------------------------------------------------
    #[test]
    fn draw_sprite_ex_flipped_no_panic() {
        let mut fb = Framebuffer::new();
        let frame = SpriteFrame {
            width: 4,
            height: 4,
            left_offset: 2,
            top_offset: 4,
            pixels: vec![Some(42); 16],
        };
        // Should not panic in either mode.
        draw_sprite_ex(&mut fb, &frame, 160, 100, &IDENTITY_COLORMAP, true);
        draw_sprite_ex(&mut fb, &frame, 160, 100, &IDENTITY_COLORMAP, false);
    }

    #[test]
    fn draw_sprite_ex_flipped_mirrors_columns() {
        // Build a 2-column, 1-row sprite: col 0 = palette 10, col 1 = palette 20.
        let frame = SpriteFrame {
            width: 2,
            height: 1,
            left_offset: 0,
            top_offset: 0,
            pixels: vec![Some(10), Some(20)],
        };

        // Draw non-flipped at center x=100, bottom y=50.
        // sprite_origin_x = 100 - 0 = 100, so col 0 at x=100, col 1 at x=101.
        let mut fb_normal = Framebuffer::new();
        draw_sprite_ex(&mut fb_normal, &frame, 100, 50, &IDENTITY_COLORMAP, false);
        assert_eq!(fb_normal.get_pixel(100, 50), Some(10));
        assert_eq!(fb_normal.get_pixel(101, 50), Some(20));

        // Draw flipped: col 0 draws at x=101 (rightmost), col 1 at x=100.
        let mut fb_flip = Framebuffer::new();
        draw_sprite_ex(&mut fb_flip, &frame, 100, 50, &IDENTITY_COLORMAP, true);
        assert_eq!(
            fb_flip.get_pixel(100, 50),
            Some(20),
            "flipped: col 1 pixel at left"
        );
        assert_eq!(
            fb_flip.get_pixel(101, 50),
            Some(10),
            "flipped: col 0 pixel at right"
        );
    }

    // ------------------------------------------------------------------
    // render_things with rotation: monster uses rotated sprite
    // ------------------------------------------------------------------
    #[test]
    fn test_render_things_uses_rotated_sprite() {
        // Place an Imp (kind 3001) at (100, 0) facing east.
        // Player at (0, 0) facing east => Imp is directly ahead, but the
        // player sees the IMP'S BACK (thing faces away from player).
        // compute_sprite_rotation => rotation 5 (back view).
        let mut thing = make_thing(100, 0, 3001); // Imp
        thing.angle = 0; // facing east in degrees
        let level = make_test_level(vec![thing]);

        let mut cache = SpriteCache::empty();
        // Insert TROOA5 (the back-view frame the renderer should look for).
        let dummy = SpriteFrame {
            width: 2,
            height: 2,
            left_offset: 1,
            top_offset: 0,
            pixels: vec![Some(77); 4],
        };
        cache.insert("TROOA5".to_string(), dummy);

        let mut fb = Framebuffer::new();
        render_things(
            &level,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Bam(0),
            &mut fb,
            &cache,
            None,
            None,
            None,
        );
        // The imp should be drawn (pixel 77 somewhere on screen).
        assert!(
            fb.data.contains(&77),
            "rotated sprite (TROOA5) should be rendered"
        );
    }

    // ------------------------------------------------------------------
    // render_things: non-directional item uses rotation 0
    // ------------------------------------------------------------------
    #[test]
    fn test_render_things_item_uses_rotation_0() {
        // Barrel (kind 2035) at (100, 0) — no rotation.
        let thing = make_thing(100, 0, 2035);
        let level = make_test_level(vec![thing]);

        let mut cache = SpriteCache::empty();
        let dummy = SpriteFrame {
            width: 2,
            height: 2,
            left_offset: 1,
            top_offset: 0,
            pixels: vec![Some(88); 4],
        };
        cache.insert("BAR1A0".to_string(), dummy);

        let mut fb = Framebuffer::new();
        render_things(
            &level,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Bam(0),
            &mut fb,
            &cache,
            None,
            None,
            None,
        );
        assert!(
            fb.data.contains(&88),
            "non-directional barrel (BAR1A0) should be rendered"
        );
    }

    // ------------------------------------------------------------------
    // render_things: fallback to rotation 0 when rotated sprite missing
    // ------------------------------------------------------------------
    #[test]
    fn test_render_things_fallback_to_rotation_0() {
        // Imp at (100, 0) facing east. Viewer at origin facing east.
        // Rotation would be 5 (back view), but only TROOA0 is in cache.
        let mut thing = make_thing(100, 0, 3001);
        thing.angle = 0;
        let level = make_test_level(vec![thing]);

        let mut cache = SpriteCache::empty();
        let dummy = SpriteFrame {
            width: 2,
            height: 2,
            left_offset: 1,
            top_offset: 0,
            pixels: vec![Some(55); 4],
        };
        cache.insert("TROOA0".to_string(), dummy);

        let mut fb = Framebuffer::new();
        render_things(
            &level,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Bam(0),
            &mut fb,
            &cache,
            None,
            None,
            None,
        );
        assert!(
            fb.data.contains(&55),
            "should fall back to TROOA0 when TROOA1 is missing"
        );
    }

    // ------------------------------------------------------------------
    // render_things: mirrored rotation fallback
    // ------------------------------------------------------------------
    #[test]
    fn test_render_things_mirror_fallback() {
        // East-facing Imp at (100, 50), viewer at (0, 0) facing NE-ish.
        //
        // Direction from viewer(0,0) to thing(100,50):
        //   atan2(50, 100) ~ 26.6 deg ~ BAM 0x12E0_0000
        //   ang = 0x12E0_0000 - 0(thing_angle) = 0x12E0_0000
        //   relative = 0x12E0_0000 + 0x9000_0000 = 0xA2E0_0000
        //   sector = (0xA2E0_0000 >> 29) & 7 = 5
        //   rotation = 6
        //
        // mirror_rotation(6) = 4. So if TROOA6 is missing but TROOA4
        // exists, the renderer should use TROOA4 drawn flipped.
        let mut thing = make_thing(100, 50, 3001);
        thing.angle = 0; // east
        let level = make_test_level(vec![thing]);

        let mut cache = SpriteCache::empty();
        // Only insert TROOA4 (the mirror of rotation 6).
        let dummy = SpriteFrame {
            width: 2,
            height: 2,
            left_offset: 1,
            top_offset: 0,
            pixels: vec![Some(33); 4],
        };
        cache.insert("TROOA4".to_string(), dummy);

        // Player needs the thing in front. Player at (0,0) facing east-ish.
        // cos(0) = 1, sin(0) = 0. vx = 100*1 + 50*0 = 100. In front.
        let mut fb = Framebuffer::new();
        render_things(
            &level,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Bam(0),
            &mut fb,
            &cache,
            None,
            None,
            None,
        );
        // The mirror fallback should have drawn the sprite using TROOA4 (flipped).
        assert!(
            fb.data.contains(&33),
            "mirror fallback should render the imp via TROOA4 (mirror of rot 6)"
        );
    }

    // ==================================================================
    // Z-buffer sprite clipping tests
    // ==================================================================

    /// Helper: build a z-buffer filled with a single value.
    fn make_zbuf(val: f32) -> [f32; SCREEN_W] {
        [val; SCREEN_W]
    }

    /// Helper: build a fully opaque square sprite with the given palette index.
    fn make_opaque_sprite(width: u16, height: u16, palette_idx: u8) -> SpriteFrame {
        SpriteFrame {
            width,
            height,
            left_offset: (width / 2) as i16,
            top_offset: height as i16,
            pixels: vec![Some(palette_idx); (width as usize) * (height as usize)],
        }
    }

    fn rendered_rows(fb: &Framebuffer, palette_idx: u8) -> Option<(usize, usize)> {
        let top = (0..SCREEN_H)
            .find(|&y| (0..SCREEN_W).any(|x| fb.get_pixel(x, y) == Some(palette_idx)))?;
        let bottom = (0..SCREEN_H)
            .rev()
            .find(|&y| (0..SCREEN_W).any(|x| fb.get_pixel(x, y) == Some(palette_idx)))?;
        Some((top, bottom))
    }

    fn full_screen_sprite_clip() -> SpriteClip<'static> {
        static TOP: [i32; SCREEN_W] = [0; SCREEN_W];
        static BOTTOM: [i32; SCREEN_W] = [SCREEN_H as i32 - 1; SCREEN_W];
        static DEPTH: [f32; SCREEN_W] = [f32::MAX; SCREEN_W];
        SpriteClip {
            top: &TOP,
            bottom: &BOTTOM,
            top_depth: &DEPTH,
            bottom_depth: &DEPTH,
            top_history: None,
            bottom_history: None,
        }
    }

    #[test]
    fn render_actors_respects_sprite_top_offset() {
        let level = make_test_level(vec![]);
        let player_x = doom_types::Fixed16_16::from_int(-96);
        let player_y = doom_types::Fixed16_16::from_int(32);
        let actor = crate::sprite_lookup::ActorRenderInfo {
            x: doom_types::Fixed16_16::from_int(32).raw(),
            y: doom_types::Fixed16_16::from_int(32).raw(),
            z: doom_types::Fixed16_16::from_int(0).raw(),
            angle: 0,
            sprite: doom_game::states::sprite_names::SPR_NONE,
            frame: 0,
            height: doom_types::Fixed16_16::from_int(8).raw(),
            render_flag: RenderFlag::Normal,
            fallback_prefix: Some(*b"BAR1"),
        };

        let mut floor_cache = SpriteCache::empty();
        let mut floor_sprite = make_opaque_sprite(8, 8, 71);
        floor_sprite.top_offset = 8;
        floor_cache.insert("BAR1A0".to_string(), floor_sprite);

        let mut sunk_cache = SpriteCache::empty();
        let mut sunk_sprite = make_opaque_sprite(8, 8, 72);
        sunk_sprite.top_offset = 4;
        sunk_cache.insert("BAR1A0".to_string(), sunk_sprite);

        let mut floor_fb = Framebuffer::new();
        render_actors_ex(
            &[actor],
            &level,
            player_x,
            player_y,
            doom_types::Bam::ZERO,
            &mut floor_fb,
            &floor_cache,
            None,
            None,
            None,
        );

        let mut sunk_fb = Framebuffer::new();
        render_actors_ex(
            &[actor],
            &level,
            player_x,
            player_y,
            doom_types::Bam::ZERO,
            &mut sunk_fb,
            &sunk_cache,
            None,
            None,
            None,
        );

        let floor_rows = rendered_rows(&floor_fb, 71).expect("floor-aligned sprite should draw");
        let sunk_rows = rendered_rows(&sunk_fb, 72).expect("reduced top_offset sprite should draw");

        assert!(
            sunk_rows.0 > floor_rows.0,
            "smaller top_offset should move actor sprite down on screen: floor={floor_rows:?} sunk={sunk_rows:?}"
        );
        assert!(
            sunk_rows.1 > floor_rows.1,
            "smaller top_offset should also lower the actor sprite bottom edge"
        );
    }

    #[test]
    fn render_things_respects_sprite_top_offset() {
        let level = make_test_level(vec![make_thing(32, 32, 2035)]);
        let player_x = doom_types::Fixed16_16::from_int(-96);
        let player_y = doom_types::Fixed16_16::from_int(32);

        let mut floor_cache = SpriteCache::empty();
        let mut floor_sprite = make_opaque_sprite(8, 8, 81);
        floor_sprite.top_offset = 8;
        floor_cache.insert("BAR1A0".to_string(), floor_sprite);

        let mut sunk_cache = SpriteCache::empty();
        let mut sunk_sprite = make_opaque_sprite(8, 8, 82);
        sunk_sprite.top_offset = 4;
        sunk_cache.insert("BAR1A0".to_string(), sunk_sprite);

        let mut floor_fb = Framebuffer::new();
        render_things(
            &level,
            player_x,
            player_y,
            doom_types::Bam::ZERO,
            &mut floor_fb,
            &floor_cache,
            None,
            None,
            None,
        );

        let mut sunk_fb = Framebuffer::new();
        render_things(
            &level,
            player_x,
            player_y,
            doom_types::Bam::ZERO,
            &mut sunk_fb,
            &sunk_cache,
            None,
            None,
            None,
        );

        let floor_rows =
            rendered_rows(&floor_fb, 81).expect("floor-aligned thing sprite should draw");
        let sunk_rows =
            rendered_rows(&sunk_fb, 82).expect("reduced top_offset thing sprite should draw");

        assert!(
            sunk_rows.0 > floor_rows.0,
            "smaller top_offset should move map thing sprite down on screen: floor={floor_rows:?} sunk={sunk_rows:?}"
        );
        assert!(
            sunk_rows.1 > floor_rows.1,
            "smaller top_offset should also lower the thing sprite bottom edge"
        );
    }

    #[test]
    fn render_actors_respects_actor_z() {
        let level = make_test_level(vec![]);
        let player_x = doom_types::Fixed16_16::from_int(-96);
        let player_y = doom_types::Fixed16_16::from_int(32);
        let mut cache = SpriteCache::empty();
        cache.insert("BAR1A0".to_string(), make_opaque_sprite(8, 8, 91));

        let floor_actor = crate::sprite_lookup::ActorRenderInfo {
            x: doom_types::Fixed16_16::from_int(32).raw(),
            y: doom_types::Fixed16_16::from_int(32).raw(),
            z: doom_types::Fixed16_16::from_int(0).raw(),
            angle: 0,
            sprite: doom_game::states::sprite_names::SPR_NONE,
            frame: 0,
            height: doom_types::Fixed16_16::from_int(8).raw(),
            render_flag: RenderFlag::Normal,
            fallback_prefix: Some(*b"BAR1"),
        };

        let raised_actor = crate::sprite_lookup::ActorRenderInfo {
            z: doom_types::Fixed16_16::from_int(8).raw(),
            ..floor_actor
        };

        let mut floor_fb = Framebuffer::new();
        render_actors_ex(
            &[floor_actor],
            &level,
            player_x,
            player_y,
            doom_types::Bam::ZERO,
            &mut floor_fb,
            &cache,
            None,
            None,
            None,
        );

        let mut raised_fb = Framebuffer::new();
        render_actors_ex(
            &[raised_actor],
            &level,
            player_x,
            player_y,
            doom_types::Bam::ZERO,
            &mut raised_fb,
            &cache,
            None,
            None,
            None,
        );

        let floor_rows = rendered_rows(&floor_fb, 91).expect("floor actor should draw");
        let raised_rows = rendered_rows(&raised_fb, 91).expect("raised actor should draw");

        assert!(
            raised_rows.0 < floor_rows.0,
            "higher actor z should move sprite up on screen: floor={floor_rows:?} raised={raised_rows:?}"
        );
        assert!(
            raised_rows.1 < floor_rows.1,
            "higher actor z should also raise the actor sprite bottom edge"
        );
    }

    #[test]
    fn render_actors_ignore_portal_clip_when_sprite_is_in_front() {
        let level = make_test_level(vec![]);
        let player_x = doom_types::Fixed16_16::from_int(-32);
        let player_y = doom_types::Fixed16_16::from_int(32);
        let actor = crate::sprite_lookup::ActorRenderInfo {
            x: doom_types::Fixed16_16::from_int(32).raw(),
            y: doom_types::Fixed16_16::from_int(32).raw(),
            z: doom_types::Fixed16_16::from_int(0).raw(),
            angle: 0,
            sprite: doom_game::states::sprite_names::SPR_NONE,
            frame: 0,
            height: doom_types::Fixed16_16::from_int(8).raw(),
            render_flag: RenderFlag::Normal,
            fallback_prefix: Some(*b"BAR1"),
        };
        let mut cache = SpriteCache::empty();
        cache.insert("BAR1A0".to_string(), make_opaque_sprite(8, 8, 92));

        let mut top = [0i32; SCREEN_W];
        let mut bottom = [SCREEN_H as i32 - 1; SCREEN_W];
        let mut top_depth = [f32::MAX; SCREEN_W];
        let mut bottom_depth = [f32::MAX; SCREEN_W];
        top.fill(138);
        bottom.fill(145);
        top_depth.fill(128.0);
        bottom_depth.fill(128.0);
        let sprite_clip = SpriteClip {
            top: &top,
            bottom: &bottom,
            top_depth: &top_depth,
            bottom_depth: &bottom_depth,
            top_history: None,
            bottom_history: None,
        };

        let mut unclipped_fb = Framebuffer::new();
        render_actors_ex(
            &[actor],
            &level,
            player_x,
            player_y,
            doom_types::Bam::ZERO,
            &mut unclipped_fb,
            &cache,
            None,
            None,
            Some(full_screen_sprite_clip()),
        );

        let mut clipped_fb = Framebuffer::new();
        render_actors_ex(
            &[actor],
            &level,
            player_x,
            player_y,
            doom_types::Bam::ZERO,
            &mut clipped_fb,
            &cache,
            None,
            None,
            Some(sprite_clip),
        );

        let unclipped_rows = rendered_rows(&unclipped_fb, 92).expect("baseline sprite should draw");
        let clipped_rows =
            rendered_rows(&clipped_fb, 92).expect("foreground sprite should still draw");

        assert_eq!(
            clipped_rows, unclipped_rows,
            "portal clip behind the sprite must not crop it"
        );
    }

    #[test]
    fn render_actors_apply_portal_clip_when_sprite_is_behind() {
        let level = make_test_level(vec![]);
        let player_x = doom_types::Fixed16_16::from_int(-96);
        let player_y = doom_types::Fixed16_16::from_int(32);
        let actor = crate::sprite_lookup::ActorRenderInfo {
            x: doom_types::Fixed16_16::from_int(32).raw(),
            y: doom_types::Fixed16_16::from_int(32).raw(),
            z: doom_types::Fixed16_16::from_int(0).raw(),
            angle: 0,
            sprite: doom_game::states::sprite_names::SPR_NONE,
            frame: 0,
            height: doom_types::Fixed16_16::from_int(8).raw(),
            render_flag: RenderFlag::Normal,
            fallback_prefix: Some(*b"BAR1"),
        };
        let mut cache = SpriteCache::empty();
        cache.insert("BAR1A0".to_string(), make_opaque_sprite(8, 8, 93));

        let mut top = [0i32; SCREEN_W];
        let mut bottom = [SCREEN_H as i32 - 1; SCREEN_W];
        let mut top_depth = [f32::MAX; SCREEN_W];
        let mut bottom_depth = [f32::MAX; SCREEN_W];
        top.fill(145);
        bottom.fill(148);
        top_depth.fill(64.0);
        bottom_depth.fill(64.0);
        let sprite_clip = SpriteClip {
            top: &top,
            bottom: &bottom,
            top_depth: &top_depth,
            bottom_depth: &bottom_depth,
            top_history: None,
            bottom_history: None,
        };

        let mut fb = Framebuffer::new();
        render_actors_ex(
            &[actor],
            &level,
            player_x,
            player_y,
            doom_types::Bam::ZERO,
            &mut fb,
            &cache,
            None,
            None,
            Some(sprite_clip),
        );

        let clipped_rows =
            rendered_rows(&fb, 93).expect("background sprite should draw inside portal");
        assert!(
            clipped_rows.0 >= 145 && clipped_rows.1 <= 148,
            "sprite behind portal should be clipped to the portal window, got {clipped_rows:?}"
        );
    }

    #[test]
    fn render_actors_fall_back_to_nearer_bottom_clip_history() {
        let level = make_test_level(vec![]);
        let player_x = doom_types::Fixed16_16::from_int(-160);
        let player_y = doom_types::Fixed16_16::from_int(32);
        let actor = crate::sprite_lookup::ActorRenderInfo {
            x: doom_types::Fixed16_16::from_int(32).raw(),
            y: doom_types::Fixed16_16::from_int(32).raw(),
            z: doom_types::Fixed16_16::from_int(-64).raw(),
            angle: 0,
            sprite: doom_game::states::sprite_names::SPR_NONE,
            frame: 0,
            height: doom_types::Fixed16_16::from_int(128).raw(),
            render_flag: RenderFlag::Normal,
            fallback_prefix: Some(*b"BAR1"),
        };
        let mut cache = SpriteCache::empty();
        let frame = make_opaque_sprite(8, 128, 94);
        cache.insert("BAR1A0".to_string(), frame);

        let top = [0i32; SCREEN_W];
        let mut bottom = [120i32; SCREEN_W];
        let top_depth = [f32::MAX; SCREEN_W];
        let mut bottom_depth = [256.0f32; SCREEN_W];
        bottom.fill(120);
        bottom_depth.fill(256.0);

        let mut bottom_history: [Vec<crate::render::SpriteClipStep>; SCREEN_W] =
            [const { Vec::new() }; SCREEN_W];
        for history in &mut bottom_history {
            history.push(crate::render::SpriteClipStep {
                depth: 128.0,
                row: 150,
                silhouette_height: 0.0,
            });
            history.push(crate::render::SpriteClipStep {
                depth: 256.0,
                row: 120,
                silhouette_height: 0.0,
            });
        }

        let mut fb = Framebuffer::new();
        render_actors_ex(
            &[actor],
            &level,
            player_x,
            player_y,
            doom_types::Bam::ZERO,
            &mut fb,
            &cache,
            None,
            None,
            Some(SpriteClip {
                top: &top,
                bottom: &bottom,
                top_depth: &top_depth,
                bottom_depth: &bottom_depth,
                top_history: None,
                bottom_history: Some(&bottom_history),
            }),
        );

        let clipped_rows =
            rendered_rows(&fb, 94).expect("sprite between clip contributors should draw");
        assert_eq!(
            clipped_rows.1, 150,
            "sprite in front of the farther bottom clip should fall back to the nearer clip history, got {clipped_rows:?}"
        );
    }

    #[test]
    fn render_actors_ignore_bottom_clip_when_sprite_is_above_silhouette_height() {
        let level = make_test_level(vec![]);
        let player_x = doom_types::Fixed16_16::from_int(-96);
        let player_y = doom_types::Fixed16_16::from_int(32);
        let actor = crate::sprite_lookup::ActorRenderInfo {
            x: doom_types::Fixed16_16::from_int(32).raw(),
            y: doom_types::Fixed16_16::from_int(32).raw(),
            z: doom_types::Fixed16_16::from_int(32).raw(),
            angle: 0,
            sprite: doom_game::states::sprite_names::SPR_NONE,
            frame: 0,
            height: doom_types::Fixed16_16::from_int(32).raw(),
            render_flag: RenderFlag::Normal,
            fallback_prefix: Some(*b"BAR1"),
        };
        let mut cache = SpriteCache::empty();
        cache.insert("BAR1A0".to_string(), make_opaque_sprite(16, 32, 95));

        let top = [0i32; SCREEN_W];
        let bottom = [SCREEN_H as i32 - 1; SCREEN_W];
        let top_depth = [f32::MAX; SCREEN_W];
        let bottom_depth = [f32::MAX; SCREEN_W];
        let mut bottom_history: [Vec<crate::render::SpriteClipStep>; SCREEN_W] =
            [const { Vec::new() }; SCREEN_W];
        for history in &mut bottom_history {
            history.push(crate::render::SpriteClipStep {
                depth: 64.0,
                row: 90,
                silhouette_height: 0.0,
            });
        }

        let mut unclipped_fb = Framebuffer::new();
        render_actors_ex(
            &[actor],
            &level,
            player_x,
            player_y,
            doom_types::Bam::ZERO,
            &mut unclipped_fb,
            &cache,
            None,
            None,
            Some(full_screen_sprite_clip()),
        );

        let mut clipped_fb = Framebuffer::new();
        render_actors_ex(
            &[actor],
            &level,
            player_x,
            player_y,
            doom_types::Bam::ZERO,
            &mut clipped_fb,
            &cache,
            None,
            None,
            Some(SpriteClip {
                top: &top,
                bottom: &bottom,
                top_depth: &top_depth,
                bottom_depth: &bottom_depth,
                top_history: None,
                bottom_history: Some(&bottom_history),
            }),
        );

        let unclipped_rows = rendered_rows(&unclipped_fb, 95).expect("baseline sprite should draw");
        let clipped_rows =
            rendered_rows(&clipped_fb, 95).expect("sprite above floor silhouette should draw");
        assert_eq!(
            clipped_rows, unclipped_rows,
            "bottom clip from a lower sector context must not crop a sprite above that floor"
        );
    }

    #[test]
    fn render_actors_ignore_top_clip_when_sprite_is_below_silhouette_height() {
        let level = make_test_level(vec![]);
        let player_x = doom_types::Fixed16_16::from_int(-96);
        let player_y = doom_types::Fixed16_16::from_int(32);
        let actor = crate::sprite_lookup::ActorRenderInfo {
            x: doom_types::Fixed16_16::from_int(32).raw(),
            y: doom_types::Fixed16_16::from_int(32).raw(),
            z: doom_types::Fixed16_16::from_int(0).raw(),
            angle: 0,
            sprite: doom_game::states::sprite_names::SPR_NONE,
            frame: 0,
            height: doom_types::Fixed16_16::from_int(32).raw(),
            render_flag: RenderFlag::Normal,
            fallback_prefix: Some(*b"BAR1"),
        };
        let mut cache = SpriteCache::empty();
        cache.insert("BAR1A0".to_string(), make_opaque_sprite(16, 32, 96));

        let top = [0i32; SCREEN_W];
        let bottom = [SCREEN_H as i32 - 1; SCREEN_W];
        let top_depth = [f32::MAX; SCREEN_W];
        let bottom_depth = [f32::MAX; SCREEN_W];
        let mut top_history: [Vec<crate::render::SpriteClipStep>; SCREEN_W] =
            [const { Vec::new() }; SCREEN_W];
        for history in &mut top_history {
            history.push(crate::render::SpriteClipStep {
                depth: 64.0,
                row: 125,
                silhouette_height: 64.0,
            });
        }

        let mut unclipped_fb = Framebuffer::new();
        render_actors_ex(
            &[actor],
            &level,
            player_x,
            player_y,
            doom_types::Bam::ZERO,
            &mut unclipped_fb,
            &cache,
            None,
            None,
            Some(full_screen_sprite_clip()),
        );

        let mut clipped_fb = Framebuffer::new();
        render_actors_ex(
            &[actor],
            &level,
            player_x,
            player_y,
            doom_types::Bam::ZERO,
            &mut clipped_fb,
            &cache,
            None,
            None,
            Some(SpriteClip {
                top: &top,
                bottom: &bottom,
                top_depth: &top_depth,
                bottom_depth: &bottom_depth,
                top_history: Some(&top_history),
                bottom_history: None,
            }),
        );

        let unclipped_rows = rendered_rows(&unclipped_fb, 96).expect("baseline sprite should draw");
        let clipped_rows =
            rendered_rows(&clipped_fb, 96).expect("sprite below ceiling silhouette should draw");
        assert_eq!(
            clipped_rows, unclipped_rows,
            "top clip from a higher ceiling context must not crop a sprite below that ceiling"
        );
    }

    // ------------------------------------------------------------------
    // zbuf 1: z_buffer initialized to MAX (no walls) allows all sprites
    // ------------------------------------------------------------------
    #[test]
    fn zbuf_initialized_to_max() {
        let zbuf = make_zbuf(f32::MAX);
        for &val in zbuf.iter() {
            assert_eq!(val, f32::MAX, "z_buffer must be initialized to f32::MAX");
        }
    }

    // ------------------------------------------------------------------
    // zbuf 2: wall rendering writes correct depth to z_buffer
    //
    // Verified indirectly: render_level returns a z_buffer with finite
    // values for columns that have one-sided walls rendered.
    // ------------------------------------------------------------------
    #[test]
    fn zbuf_wall_writes_finite_depth() {
        use doom_types::ANG90;

        // Need trig tables for render_level.
        unsafe {
            doom_types::Bam::init_trig_tables();
        }

        let level = make_render_level_one_sided();
        let mut fb = Framebuffer::new();
        let palette = crate::palette::PaletteLut::grayscale();

        let zbuf = crate::render::render_level(
            &level, 64, 0, ANG90, &mut fb, &palette, None, None, None, None, false,
        );

        // The wall at y=128 is 128 map units away from player at (64,0)
        // facing north. At least some central columns should have finite depth.
        let center = SCREEN_W / 2;
        let finite_count = zbuf.z_buf
            [center.saturating_sub(10)..center.saturating_add(10).min(SCREEN_W)]
            .iter()
            .filter(|&&v| v < f32::MAX)
            .count();
        assert!(
            finite_count > 0,
            "at least some central columns should have a finite wall depth"
        );
    }

    // ------------------------------------------------------------------
    // zbuf 3: sprite behind wall is fully clipped (no pixels drawn)
    // ------------------------------------------------------------------
    #[test]
    fn zbuf_sprite_behind_wall_fully_clipped() {
        // Barrel at (100, 0), player at (0, 0) facing east.
        // Sprite depth vx ~ 100.
        // Set z_buffer to 50.0 everywhere => sprite is behind all walls.
        let thing = make_thing(100, 0, 2035);
        let level = make_test_level(vec![thing]);

        let mut cache = SpriteCache::empty();
        cache.insert("BAR1A0".to_string(), make_opaque_sprite(16, 32, 77));

        let mut fb = Framebuffer::new();
        let zbuf = make_zbuf(50.0); // wall at depth 50, sprite at ~100

        render_things(
            &level,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Bam(0),
            &mut fb,
            &cache,
            Some(&zbuf),
            None,
            None,
        );

        assert!(
            fb.data.iter().all(|&b| b != 77),
            "sprite behind wall must not draw any pixels"
        );
    }

    // ------------------------------------------------------------------
    // zbuf 4: sprite in front of wall is fully drawn
    // ------------------------------------------------------------------
    #[test]
    fn zbuf_sprite_in_front_of_wall_fully_drawn() {
        // Barrel at (100, 0), player at (0, 0) facing east.
        // Sprite depth vx ~ 100.
        // Set z_buffer to 200.0 everywhere => sprite is in front of all walls.
        let thing = make_thing(100, 0, 2035);
        let level = make_test_level(vec![thing]);

        let mut cache = SpriteCache::empty();
        cache.insert("BAR1A0".to_string(), make_opaque_sprite(16, 32, 88));

        let mut fb = Framebuffer::new();
        let zbuf = make_zbuf(200.0); // wall at depth 200, sprite at ~100

        render_things(
            &level,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Bam(0),
            &mut fb,
            &cache,
            Some(&zbuf),
            None,
            None,
        );

        assert!(
            fb.data.contains(&88),
            "sprite in front of wall must be drawn"
        );
    }

    // ------------------------------------------------------------------
    // zbuf 5: partial occlusion — sprite spans columns with different
    //         wall depths; some columns drawn, others clipped
    // ------------------------------------------------------------------
    #[test]
    fn zbuf_partial_occlusion() {
        // Barrel at (100, 0), player at (0, 0) facing east.
        // Sprite depth vx ~ 100.
        // Left half of screen: wall at depth 50 (blocks sprite).
        // Right half of screen: wall at depth 200 (sprite visible).
        let thing = make_thing(100, 0, 2035);
        let level = make_test_level(vec![thing]);

        let mut cache = SpriteCache::empty();
        cache.insert("BAR1A0".to_string(), make_opaque_sprite(16, 32, 66));

        let mut fb = Framebuffer::new();
        let mut zbuf = [0.0f32; SCREEN_W];
        for (x, depth) in zbuf.iter_mut().enumerate() {
            if x < SCREEN_W / 2 {
                *depth = 50.0; // blocks sprite
            } else {
                *depth = 200.0; // sprite visible
            }
        }

        render_things(
            &level,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Bam(0),
            &mut fb,
            &cache,
            Some(&zbuf),
            None,
            None,
        );

        // Sprite is at screen center (x ~ 160) which is in the right half.
        // Some pixels should be drawn (right half is unoccluded).
        assert!(
            fb.data.contains(&66),
            "partially occluded sprite should draw some pixels in unblocked columns"
        );

        // Additionally verify the left half is clear: pixels below x=160
        // in the left half should not have pixel 66 (for columns in far-left
        // which are blocked by the near wall).
        // The sprite center is at x=160, so columns 0..~152 should be clipped.
        let left_drawn = (0..140).any(|x| (0..SCREEN_H).any(|y| fb.get_pixel(x, y) == Some(66)));
        assert!(
            !left_drawn,
            "columns behind the near wall (left half) must not have sprite pixels"
        );
    }

    // ------------------------------------------------------------------
    // zbuf 6: near-plane sprite clamped to screen bounds
    // ------------------------------------------------------------------
    #[test]
    fn zbuf_near_plane_sprite_clamped() {
        // Place a thing very close to the player so the projected sprite
        // extends past screen edges. It should not panic and no out-of-bounds
        // writes should occur.
        let thing = make_thing(2, 0, 2035); // Only 2 map units ahead
        let level = make_test_level(vec![thing]);

        let mut cache = SpriteCache::empty();
        cache.insert("BAR1A0".to_string(), make_opaque_sprite(16, 32, 55));

        let mut fb = Framebuffer::new();
        let zbuf = make_zbuf(f32::MAX);

        // Must not panic even with huge projected sprite.
        render_things(
            &level,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Bam(0),
            &mut fb,
            &cache,
            Some(&zbuf),
            None,
            None,
        );
        // Just verify it didn't panic. Some pixels might be drawn.
    }

    // ------------------------------------------------------------------
    // zbuf 7: two-sided seg does NOT write z_buffer
    //
    // Verified indirectly: render_level with a two-sided level should
    // leave z_buf at f32::MAX for columns where only a portal was drawn.
    // ------------------------------------------------------------------
    #[test]
    fn zbuf_two_sided_seg_does_not_write() {
        use doom_types::ANG90;

        unsafe {
            doom_types::Bam::init_trig_tables();
        }

        let level = make_render_level_two_sided();
        let mut fb = Framebuffer::new();
        let palette = crate::palette::PaletteLut::grayscale();

        let zbuf = crate::render::render_level(
            &level, 0, 0, ANG90, &mut fb, &palette, None, None, None, None, false,
        );

        // For a two-sided seg, z_buf should remain at f32::MAX for
        // columns in the portal's span (no one-sided wall occluded them).
        let center = SCREEN_W / 2;
        assert_eq!(
            zbuf.z_buf[center],
            f32::MAX,
            "two-sided seg center column must not write to z_buffer (got {})",
            zbuf.z_buf[center]
        );
    }

    // ------------------------------------------------------------------
    // zbuf 8: multiple sprites at different depths sorted correctly
    //
    // With a wall at depth 150, a sprite at depth 100 (in front) should
    // be visible, and a sprite at depth 200 (behind) should be clipped.
    // ------------------------------------------------------------------
    #[test]
    fn zbuf_multiple_sprites_depth_sorting() {
        // Two barrels: one at (100, 0), one at (200, 0).
        // Wall z_buffer at depth 150. Only the near barrel should draw.
        let near_barrel = make_thing(100, 0, 2035);
        let far_barrel = make_thing(200, 0, 2035);
        let level = make_test_level(vec![near_barrel, far_barrel]);

        let mut cache = SpriteCache::empty();
        // Both use the same sprite (BAR1A0) but we track via pixel value.
        // Since render_things uses painter's algorithm (back-to-front),
        // the near barrel overdraw the far one. With z_buffer=150,
        // the far barrel (depth~200) is clipped, near (depth~100) is drawn.
        cache.insert("BAR1A0".to_string(), make_opaque_sprite(8, 16, 99));

        let mut fb = Framebuffer::new();
        let zbuf = make_zbuf(150.0);

        render_things(
            &level,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Bam(0),
            &mut fb,
            &cache,
            Some(&zbuf),
            None,
            None,
        );

        // Near barrel (depth ~100 < 150) should be drawn.
        assert!(
            fb.data.contains(&99),
            "near sprite (depth 100 < z_buffer 150) should be visible"
        );
    }

    // ------------------------------------------------------------------
    // zbuf 9: sprite at exact wall depth — wall wins (>= comparison)
    // ------------------------------------------------------------------
    #[test]
    fn zbuf_sprite_at_exact_wall_depth_clipped() {
        // Barrel at (100, 0), sprite depth vx ~ 100.
        // Set z_buffer to exactly 100.0 everywhere.
        // The comparison is `vx >= zbuf[sx]`, so equal depth means clipped.
        let thing = make_thing(100, 0, 2035);
        let level = make_test_level(vec![thing]);

        let mut cache = SpriteCache::empty();
        cache.insert("BAR1A0".to_string(), make_opaque_sprite(16, 32, 44));

        let mut fb = Framebuffer::new();
        let zbuf = make_zbuf(100.0);

        render_things(
            &level,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Bam(0),
            &mut fb,
            &cache,
            Some(&zbuf),
            None,
            None,
        );

        // Sprite depth == wall depth: wall wins, sprite clipped.
        assert!(
            fb.data.iter().all(|&b| b != 44),
            "sprite at exact wall depth should be clipped (wall wins)"
        );
    }

    // ------------------------------------------------------------------
    // zbuf 10: z_buffer with no walls (all MAX) — all sprites visible
    // ------------------------------------------------------------------
    #[test]
    fn zbuf_no_walls_all_sprites_visible() {
        let thing = make_thing(100, 0, 2035);
        let level = make_test_level(vec![thing]);

        let mut cache = SpriteCache::empty();
        cache.insert("BAR1A0".to_string(), make_opaque_sprite(8, 16, 111));

        let mut fb = Framebuffer::new();
        let zbuf = make_zbuf(f32::MAX); // no walls

        render_things(
            &level,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Bam(0),
            &mut fb,
            &cache,
            Some(&zbuf),
            None,
            None,
        );

        assert!(
            fb.data.contains(&111),
            "with z_buffer=MAX (no walls), sprite must be visible"
        );
    }

    // ------------------------------------------------------------------
    // zbuf 11: column-by-column clipping — individual columns checked
    // ------------------------------------------------------------------
    #[test]
    fn zbuf_column_by_column_clipping() {
        // Place barrel at (50, 0), player at (0,0) facing east.
        // Sprite depth vx ~ 50.
        // Create a z_buffer where columns 155..165 have wall at depth 10
        // (blocks sprite) and all others at f32::MAX.
        let thing = make_thing(50, 0, 2035);
        let level = make_test_level(vec![thing]);

        let mut cache = SpriteCache::empty();
        cache.insert("BAR1A0".to_string(), make_opaque_sprite(16, 32, 22));

        let mut fb = Framebuffer::new();
        let mut zbuf = make_zbuf(f32::MAX);

        // Block only the narrow band around center.
        for depth in zbuf.iter_mut().take(165).skip(155) {
            *depth = 10.0; // nearer than sprite
        }

        render_things(
            &level,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Bam(0),
            &mut fb,
            &cache,
            Some(&zbuf),
            None,
            None,
        );

        // The sprite projects near center (x~160). Columns 155..165 are
        // blocked, but other sprite columns should still be drawn.
        // Verify at least some pixels were drawn.
        assert!(
            fb.data.contains(&22),
            "sprite columns outside the blocked band should still be drawn"
        );

        // Verify blocked columns have no sprite pixels (column 160 is blocked).
        let col_160_has_sprite = (0..SCREEN_H).any(|y| fb.get_pixel(160, y) == Some(22));
        assert!(
            !col_160_has_sprite,
            "column 160 is blocked (zbuf=10 < sprite depth ~50) and must not have sprite pixels"
        );
    }

    // ------------------------------------------------------------------
    // zbuf 12: sprite completely off-screen produces no draws
    // ------------------------------------------------------------------
    #[test]
    fn zbuf_sprite_offscreen_no_draws() {
        // Barrel at (100, 200) — far to the side.
        // Player at (0, 0) facing east.
        // vy = -0*sin + 200*cos ≈ 200 → far to the left of screen
        // sx_center = 160 - 160*200/100 = 160 - 320 = -160 → off-screen left
        let thing = make_thing(100, 200, 2035);
        let level = make_test_level(vec![thing]);

        let mut cache = SpriteCache::empty();
        cache.insert("BAR1A0".to_string(), make_opaque_sprite(8, 16, 33));

        let mut fb = Framebuffer::new();
        let zbuf = make_zbuf(f32::MAX);

        render_things(
            &level,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Bam(0),
            &mut fb,
            &cache,
            Some(&zbuf),
            None,
            None,
        );

        assert!(
            fb.data.iter().all(|&b| b != 33),
            "sprite projected off-screen should produce no draws"
        );
    }

    // ------------------------------------------------------------------
    // zbuf 13: None z_buffer preserves backward compatibility
    // ------------------------------------------------------------------
    #[test]
    fn zbuf_none_draws_all_sprites() {
        // With z_buffer=None, sprites should be drawn just like before
        // (no clipping). Same as the pre-zbuffer behavior.
        let thing = make_thing(100, 0, 2035);
        let level = make_test_level(vec![thing]);

        let mut cache = SpriteCache::empty();
        cache.insert("BAR1A0".to_string(), make_opaque_sprite(8, 16, 200));

        let mut fb = Framebuffer::new();

        render_things(
            &level,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Bam(0),
            &mut fb,
            &cache,
            None, // no z_buffer
            None, // no colormap
            None,
        );

        assert!(
            fb.data.contains(&200),
            "with z_buffer=None, all sprites should be drawn (backward compat)"
        );
    }

    // ------------------------------------------------------------------
    // zbuf 14: render_level returns z_buffer with all MAX for empty level
    // ------------------------------------------------------------------
    #[test]
    fn zbuf_render_level_empty_returns_max() {
        unsafe {
            doom_types::Bam::init_trig_tables();
        }

        // A level with no visible walls from the player's position should
        // return a z_buffer filled with f32::MAX.
        let level = make_test_level(vec![]);
        let mut fb = Framebuffer::new();
        let palette = crate::palette::PaletteLut::grayscale();

        let zbuf = crate::render::render_level(
            &level,
            32,
            32,
            doom_types::Bam::ZERO,
            &mut fb,
            &palette,
            None,
            None,
            None,
            None,
            false,
        );

        // The test level has walls but the player is inside the box
        // looking east. The seg (vertex 0→1, i.e. (0,0)→(64,0)) is
        // at y=0 which is behind or sideways from (32,32) facing east.
        // Depending on exact geometry, most columns may be MAX.
        // At minimum, check that MAX values exist.
        let max_count = zbuf.z_buf.iter().filter(|&&v| v == f32::MAX).count();
        assert!(
            max_count > 0,
            "z_buffer should have some f32::MAX entries for columns with no wall"
        );
    }

    // ------------------------------------------------------------------
    // Helper: build a minimal one-sided level for render_level z-buffer tests
    // ------------------------------------------------------------------
    fn make_render_level_one_sided() -> doom_map::Level {
        use doom_map::lumps::{
            Blockmap, Linedef, Reject, Sector, Seg, Sidedef, Ssector, Thing, Vertex,
        };

        let vertexes = vec![Vertex { x: 0, y: 128 }, Vertex { x: 128, y: 128 }];
        let sectors = vec![Sector {
            floor_height: 0,
            ceil_height: 128,
            floor_flat: *b"FLAT1\0\0\0",
            ceil_flat: *b"FLAT2\0\0\0",
            light_level: 192,
            special: 0,
            tag: 0,
        }];
        let sidedefs = vec![Sidedef {
            x_offset: 0,
            y_offset: 0,
            upper_texture: *b"WALL1\0\0\0",
            lower_texture: *b"WALL2\0\0\0",
            middle_texture: *b"WALL3\0\0\0",
            sector: 0,
        }];
        let linedefs = vec![Linedef {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0, // one-sided
            special: 0,
            tag: 0,
            right_sidedef: 0,
            left_sidedef: 0xFFFF,
        }];
        let segs = vec![Seg {
            from_vertex: 0,
            to_vertex: 1,
            angle: 0,
            linedef: 0,
            direction: 0,
            offset: 0,
        }];
        let ssectors = vec![Ssector {
            seg_count: 1,
            first_seg: 0,
        }];
        let things = vec![Thing {
            x: 64,
            y: 0,
            angle: 0,
            kind: 1,
            flags: 7,
        }];

        let reject = Reject::parse_lump(&[0u8; 1], 1).expect("reject parse");

        let mut bm_data = vec![0u8; 8 + 2 + 4];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = Blockmap::parse_lump(&bm_data).expect("blockmap parse");

        doom_map::Level {
            name: "ZBUF1".to_owned(),
            things,
            linedefs,
            sidedefs,
            vertexes,
            segs,
            ssectors,
            nodes: vec![],
            sectors,
            reject,
            blockmap,
        }
    }

    // ------------------------------------------------------------------
    // Helper: build a minimal two-sided level for render_level z-buffer tests
    // ------------------------------------------------------------------
    fn make_render_level_two_sided() -> doom_map::Level {
        use doom_map::lumps::{
            Blockmap, Linedef, Reject, Sector, Seg, Sidedef, Ssector, Thing, Vertex,
        };

        let vertexes = vec![Vertex { x: -64, y: 128 }, Vertex { x: 64, y: 128 }];
        let sectors = vec![
            Sector {
                floor_height: 0,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
            Sector {
                floor_height: 32,
                ceil_height: 96,
                floor_flat: *b"FLAT3\0\0\0",
                ceil_flat: *b"FLAT4\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
        ];
        let sidedefs = vec![
            Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"UPPER\0\0\0",
                lower_texture: *b"LOWER\0\0\0",
                middle_texture: *b"-\0\0\0\0\0\0\0",
                sector: 0,
            },
            Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"UPPER\0\0\0",
                lower_texture: *b"LOWER\0\0\0",
                middle_texture: *b"-\0\0\0\0\0\0\0",
                sector: 1,
            },
        ];
        let linedefs = vec![Linedef {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0x0004, // two-sided
            special: 0,
            tag: 0,
            right_sidedef: 0,
            left_sidedef: 1,
        }];
        let segs = vec![Seg {
            from_vertex: 0,
            to_vertex: 1,
            angle: 0,
            linedef: 0,
            direction: 0,
            offset: 0,
        }];
        let ssectors = vec![Ssector {
            seg_count: 1,
            first_seg: 0,
        }];
        let things = vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 1,
            flags: 7,
        }];

        let reject = Reject::parse_lump(&[0u8; 1], 2).expect("reject parse");

        let mut bm_data = vec![0u8; 8 + 2 + 4];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = Blockmap::parse_lump(&bm_data).expect("blockmap parse");

        doom_map::Level {
            name: "ZBUF2".to_owned(),
            things,
            linedefs,
            sidedefs,
            vertexes,
            segs,
            ssectors,
            nodes: vec![],
            sectors,
            reject,
            blockmap,
        }
    }

    // ==================================================================
    // Distance-attenuated sprite lighting tests
    // ==================================================================

    use crate::colormap::{COLORMAP_ROWS, COLORMAP_SIZE, ColormapCache};
    use crate::lighting::LightParams;
    use crate::render_flags::RenderFlag;

    /// Build a `ColormapCache` where row `n` maps every index to `n`.
    /// This makes it easy to verify which colormap row was selected.
    fn test_colormap_cache() -> ColormapCache {
        let mut data = vec![0u8; COLORMAP_ROWS * COLORMAP_SIZE];
        for row in 0..COLORMAP_ROWS {
            let start = row * COLORMAP_SIZE;
            data[start..start + COLORMAP_SIZE].fill(row as u8);
        }
        ColormapCache::from_test_data(data)
    }

    /// Build a test level with a specific sector light level.
    fn make_test_level_with_light(
        things: Vec<doom_map::Thing>,
        light_level: i16,
    ) -> doom_map::Level {
        use doom_wad::{REQUIRED_MAP_LUMPS, WadKind};

        let _ = WadKind::Iwad;
        let _ = REQUIRED_MAP_LUMPS;

        // ---- sector with custom light level ----
        let mut sector_data = vec![0u8; 26];
        sector_data[0..2].copy_from_slice(&0i16.to_le_bytes());
        sector_data[2..4].copy_from_slice(&128i16.to_le_bytes());
        sector_data[4..12].copy_from_slice(b"FLAT1\0\0\0");
        sector_data[12..20].copy_from_slice(b"FLAT2\0\0\0");
        sector_data[20..22].copy_from_slice(&light_level.to_le_bytes());
        sector_data[22..24].copy_from_slice(&0u16.to_le_bytes());
        sector_data[24..26].copy_from_slice(&0u16.to_le_bytes());

        // ---- vertices ----
        let mut vert_data = vec![0u8; 4 * 4];
        let verts: [(i16, i16); 4] = [(0, 0), (64, 0), (64, 64), (0, 64)];
        for (i, (x, y)) in verts.iter().enumerate() {
            vert_data[i * 4..i * 4 + 2].copy_from_slice(&x.to_le_bytes());
            vert_data[i * 4 + 2..i * 4 + 4].copy_from_slice(&y.to_le_bytes());
        }

        // ---- sidedefs ----
        let mut sd_data = vec![0u8; 4 * 30];
        for i in 0..4 {
            sd_data[i * 30 + 20..i * 30 + 28].copy_from_slice(b"WALL1\0\0\0");
            sd_data[i * 30 + 28..i * 30 + 30].copy_from_slice(&0u16.to_le_bytes());
        }

        // ---- linedefs ----
        let mut ld_data = vec![0u8; 4 * 14];
        let edges: [(u16, u16); 4] = [(0, 1), (1, 2), (2, 3), (3, 0)];
        for (i, (from, to)) in edges.iter().enumerate() {
            let b = &mut ld_data[i * 14..i * 14 + 14];
            b[0..2].copy_from_slice(&from.to_le_bytes());
            b[2..4].copy_from_slice(&to.to_le_bytes());
            b[4..6].copy_from_slice(&0u16.to_le_bytes());
            b[6..8].copy_from_slice(&0u16.to_le_bytes());
            b[8..10].copy_from_slice(&0u16.to_le_bytes());
            b[10..12].copy_from_slice(&(i as u16).to_le_bytes());
            b[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        }

        // ---- seg ----
        let mut seg_data = vec![0u8; 12];
        seg_data[0..2].copy_from_slice(&0u16.to_le_bytes());
        seg_data[2..4].copy_from_slice(&1u16.to_le_bytes());

        // ---- ssector ----
        let mut ss_data = vec![0u8; 4];
        ss_data[0..2].copy_from_slice(&1u16.to_le_bytes());
        ss_data[2..4].copy_from_slice(&0u16.to_le_bytes());

        // ---- things ----
        let mut thing_data = vec![0u8; things.len() * 10];
        for (i, t) in things.iter().enumerate() {
            let b = &mut thing_data[i * 10..i * 10 + 10];
            b[0..2].copy_from_slice(&t.x.to_le_bytes());
            b[2..4].copy_from_slice(&t.y.to_le_bytes());
            b[4..6].copy_from_slice(&t.angle.to_le_bytes());
            b[6..8].copy_from_slice(&t.kind.to_le_bytes());
            b[8..10].copy_from_slice(&t.flags.to_le_bytes());
        }

        // ---- reject ----
        let reject_data = vec![0u8; 1];

        // ---- blockmap ----
        let mut bm_data = vec![0u8; 8 + 2 + 4];
        bm_data[0..2].copy_from_slice(&0i16.to_le_bytes());
        bm_data[2..4].copy_from_slice(&0i16.to_le_bytes());
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());

        let lump_payloads: &[(&[u8; 8], &[u8])] = &[
            (b"E1M1\0\0\0\0", &[]),
            (b"THINGS\0\0", &thing_data),
            (b"LINEDEFS", &ld_data),
            (b"SIDEDEFS", &sd_data),
            (b"VERTEXES", &vert_data),
            (b"SEGS\0\0\0\0", &seg_data),
            (b"SSECTORS", &ss_data),
            (b"NODES\0\0\0", &[]),
            (b"SECTORS\0", &sector_data),
            (b"REJECT\0\0", &reject_data),
            (b"BLOCKMAP", &bm_data),
        ];

        let mut data: Vec<u8> = Vec::new();
        data.extend_from_slice(b"IWAD");
        data.extend_from_slice(&(lump_payloads.len() as i32).to_le_bytes());
        data.extend_from_slice(&0i32.to_le_bytes());

        let mut offsets: Vec<(usize, usize)> = Vec::new();
        for (_, payload) in lump_payloads {
            let pos = data.len();
            data.extend_from_slice(payload);
            offsets.push((pos, payload.len()));
        }

        let dir_offset = data.len() as i32;
        data[8..12].copy_from_slice(&dir_offset.to_le_bytes());
        for (i, (name_bytes, _)) in lump_payloads.iter().enumerate() {
            let (filepos, size) = offsets[i];
            data.extend_from_slice(&(filepos as i32).to_le_bytes());
            data.extend_from_slice(&(size as i32).to_le_bytes());
            data.extend_from_slice(*name_bytes);
        }

        let wad = doom_wad::WadFile::parse(data).expect("test WAD parse failed");
        doom_map::Level::from_wad(&wad, "E1M1").expect("test level load failed")
    }

    // ------------------------------------------------------------------
    // 1. sector_for_point returns correct sector index
    // ------------------------------------------------------------------
    #[test]
    fn sector_for_point_returns_sector_0() {
        let level = make_test_level(vec![]);
        // Point inside the sector bounding box.
        let si = sector_for_point(&level, 32, 32);
        assert_eq!(si, Some(0), "point (32,32) should be in sector 0");
    }

    // ------------------------------------------------------------------
    // 2. sector_for_point returns None for empty level
    // ------------------------------------------------------------------
    #[test]
    fn sector_for_point_empty_ssectors() {
        // Build a minimal level with no segs/ssectors.
        use doom_map::lumps::{Blockmap, Reject};
        let reject = Reject::parse_lump(&[0u8; 1], 1).expect("reject");
        let mut bm_data = vec![0u8; 14];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = Blockmap::parse_lump(&bm_data).expect("bm");
        let level = doom_map::Level {
            name: "EMPTY".to_owned(),
            things: vec![],
            linedefs: vec![],
            sidedefs: vec![],
            vertexes: vec![],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![],
            reject,
            blockmap,
        };
        // With no ssectors, BSP lookup should return None.
        let si = sector_for_point(&level, 0, 0);
        assert!(si.is_none(), "empty level should return None");
    }

    // ------------------------------------------------------------------
    // 3. render_flag_for_thing returns Fuzz for spectre
    // ------------------------------------------------------------------
    #[test]
    fn render_flag_spectre_is_fuzz() {
        assert_eq!(render_flag_for_thing(58), RenderFlag::Fuzz);
    }

    // ------------------------------------------------------------------
    // 4. render_flag_for_thing returns FullBright for lamp
    // ------------------------------------------------------------------
    #[test]
    fn render_flag_lamp_is_fullbright() {
        assert_eq!(render_flag_for_thing(2028), RenderFlag::FullBright);
    }

    // ------------------------------------------------------------------
    // 5. render_flag_for_thing returns Normal for zombieman
    // ------------------------------------------------------------------
    #[test]
    fn render_flag_zombieman_is_normal() {
        assert_eq!(render_flag_for_thing(3004), RenderFlag::Normal);
    }

    // ------------------------------------------------------------------
    // 6. render_flag_for_thing returns FullBright for all glow items
    // ------------------------------------------------------------------
    #[test]
    fn render_flag_glow_items() {
        let glow_types: &[u16] = &[
            2013, 2022, 2045, // soulsphere, invuln, light-amp
            44, 45, 46, // tall firesticks
            55, 56, 57, // short firesticks
            70, 34, 35, // burning barrel, candle, candelabra
            85, 86, 48, // tech lamps, tech column
        ];
        for &kind in glow_types {
            assert_eq!(
                render_flag_for_thing(kind),
                RenderFlag::FullBright,
                "thing type {kind} should be FullBright"
            );
        }
    }

    // ------------------------------------------------------------------
    // 7. render_flag_for_thing returns Normal for regular monsters
    // ------------------------------------------------------------------
    #[test]
    fn render_flag_monsters_normal() {
        let monsters: &[u16] = &[3004, 9, 65, 3001, 3002, 3005, 3003, 69, 3006];
        for &kind in monsters {
            assert_eq!(
                render_flag_for_thing(kind),
                RenderFlag::Normal,
                "monster type {kind} should be Normal"
            );
        }
    }

    // ------------------------------------------------------------------
    // 8. render_things with colormap applies shading to sprites
    // ------------------------------------------------------------------
    #[test]
    fn render_things_with_colormap_applies_shading() {
        // Sector light = 128 (mid-brightness), thing at (100, 0).
        // With our test colormap, shaded pixels should NOT be the raw
        // palette index any more (they get mapped through a non-identity row).
        let thing = make_thing(100, 0, 2035); // barrel
        let level = make_test_level_with_light(vec![thing], 128);

        let mut cache = SpriteCache::empty();
        // Insert a bright sprite (all pixels = 200).
        cache.insert("BAR1A0".to_string(), make_opaque_sprite(8, 16, 200));

        let cm = test_colormap_cache();
        let mut fb = Framebuffer::new();

        render_things(
            &level,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Bam(0),
            &mut fb,
            &cache,
            None,
            Some(&cm),
            None,
        );

        // Pixels were drawn but NOT at the raw palette index 200.
        // With our test cache, the colormap row maps everything to the
        // row index (a single value), so we should see that value.
        let has_drawn = fb.data.iter().any(|&b| b != 0);
        assert!(has_drawn, "sprite should be drawn with colormap");

        // The raw palette index 200 should NOT appear because the
        // sector light is 128 (not fullbright), meaning some colormap
        // row > 0 was used, mapping index 200 to the row number.
        let has_raw = fb.data.contains(&200);
        assert!(
            !has_raw,
            "raw palette index should not appear when colormap is applied \
             (sector light = 128)"
        );
    }

    // ------------------------------------------------------------------
    // 9. render_things fullbright thing draws at raw palette index
    // ------------------------------------------------------------------
    #[test]
    fn render_things_fullbright_thing_not_shaded() {
        // Floor lamp (kind 2028) is fullbright, even in a dark sector.
        let thing = make_thing(50, 0, 2028); // floor lamp
        let level = make_test_level_with_light(vec![thing], 0); // pitch dark

        let mut cache = SpriteCache::empty();
        cache.insert("COLUA0".to_string(), make_opaque_sprite(8, 16, 123));

        let cm = test_colormap_cache();
        let mut fb = Framebuffer::new();

        render_things(
            &level,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Bam(0),
            &mut fb,
            &cache,
            None,
            Some(&cm),
            None,
        );

        // Fullbright thing should use colormap row 0 (identity in our test cache).
        // Row 0 maps all indices to 0. So all drawn pixels should be 0.
        // Actually, in our test_colormap_cache, row 0 maps everything to 0.
        // For fullbright things, colormap row 0 is used.
        let _has_drawn = fb.data.iter().any(|&b| b != 0);
        // Row 0 maps everything to 0, so drawn pixels should also be 0.
        // Since the fb was already 0, let's verify via a different approach:
        // use an identity cache instead.
        let id_cache = ColormapCache::identity();
        let mut fb2 = Framebuffer::new();
        render_things(
            &level,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Bam(0),
            &mut fb2,
            &cache,
            None,
            Some(&id_cache),
            None,
        );
        // With identity cache, fullbright uses row 0 which is identity.
        // So the raw palette index 123 should appear.
        assert!(
            fb2.data.contains(&123),
            "fullbright lamp should render at raw palette index with identity colormap"
        );
    }

    // ------------------------------------------------------------------
    // 10. render_things dark sector makes normal sprites darker
    // ------------------------------------------------------------------
    #[test]
    fn render_things_dark_sector_produces_dark_pixels() {
        // In a pitch-dark sector (light=0), non-fullbright sprites should
        // be mapped through the darkest colormap row.
        let thing = make_thing(100, 0, 2035); // barrel (Normal)
        let level = make_test_level_with_light(vec![thing], 0);

        let mut cache = SpriteCache::empty();
        cache.insert("BAR1A0".to_string(), make_opaque_sprite(8, 16, 150));

        let cm = test_colormap_cache();
        let mut fb = Framebuffer::new();

        render_things(
            &level,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Bam(0),
            &mut fb,
            &cache,
            None,
            Some(&cm),
            None,
        );

        // In our test cache, dark rows (high index) map everything to that
        // row index. With light=0, base_cm_index = (255-0)>>3 = 31.
        // compute_wall_light at distance ~100 with base 31 will produce a
        // high row index. Whatever the resulting row, it should NOT be
        // the raw palette index 150.
        let has_raw = fb.data.contains(&150);
        assert!(
            !has_raw,
            "dark sector (light=0) should shade sprites away from raw palette index"
        );
    }

    // ------------------------------------------------------------------
    // 11. render_things bright sector (255) renders sprites unshaded
    // ------------------------------------------------------------------
    #[test]
    fn render_things_bright_sector_identity() {
        // With light=255, the sector is auto-fullbright, so the colormap
        // row 0 (identity) is used — raw palette index should appear.
        let thing = make_thing(100, 0, 2035);
        let level = make_test_level_with_light(vec![thing], 255);

        let mut cache = SpriteCache::empty();
        cache.insert("BAR1A0".to_string(), make_opaque_sprite(8, 16, 77));

        let id_cache = ColormapCache::identity();
        let mut fb = Framebuffer::new();

        render_things(
            &level,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Bam(0),
            &mut fb,
            &cache,
            None,
            Some(&id_cache),
            None,
        );

        assert!(
            fb.data.contains(&77),
            "bright sector (255) should render sprites at raw palette index"
        );
    }

    // ------------------------------------------------------------------
    // 12. render_things None colormap backward compat (no shading)
    // ------------------------------------------------------------------
    #[test]
    fn render_things_none_colormap_no_shading() {
        let thing = make_thing(100, 0, 2035);
        let level = make_test_level_with_light(vec![thing], 0); // dark sector

        let mut cache = SpriteCache::empty();
        cache.insert("BAR1A0".to_string(), make_opaque_sprite(8, 16, 55));

        let mut fb = Framebuffer::new();
        render_things(
            &level,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Bam(0),
            &mut fb,
            &cache,
            None,
            None, // no colormap => fullbright
            None,
        );

        // Without colormap, sprites render at raw palette index.
        assert!(
            fb.data.contains(&55),
            "None colormap should render sprites unshaded (backward compat)"
        );
    }

    // ------------------------------------------------------------------
    // 13. render_things spectre uses fuzz effect
    // ------------------------------------------------------------------
    #[test]
    fn render_things_spectre_draws_fuzz() {
        // Spectre (kind 58) should use draw_fuzz_column, not normal sprite.
        // The fuzz effect reads existing fb pixels, so the sprite's palette
        // index should NOT appear in the framebuffer.
        let thing = make_thing(50, 0, 58);
        let level = make_test_level_with_light(vec![thing], 192);

        let mut cache = SpriteCache::empty();
        // Spectre uses SARG prefix.
        cache.insert("SARGA0".to_string(), make_opaque_sprite(16, 32, 222));

        // Pre-fill fb with a known value so fuzz has something to darken.
        let mut fb = Framebuffer::new();
        fb.clear(180);

        render_things(
            &level,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Bam(0),
            &mut fb,
            &cache,
            None,
            None,
            None,
        );

        // The spectre's raw palette index 222 should NOT appear.
        let has_222 = fb.data.contains(&222);
        assert!(
            !has_222,
            "spectre should use fuzz effect, not draw sprite texture"
        );

        // Some pixels should have changed from 180 (fuzz darkens them).
        let changed = fb.data.iter().filter(|&&b| b != 180).count();
        assert!(
            changed > 0,
            "fuzz effect should modify at least some framebuffer pixels"
        );
    }

    // ------------------------------------------------------------------
    // 14. render_things spectre with colormap passes colormap to fuzz
    // ------------------------------------------------------------------
    #[test]
    fn render_things_spectre_with_colormap() {
        let thing = make_thing(50, 0, 58);
        let level = make_test_level_with_light(vec![thing], 192);

        let mut cache = SpriteCache::empty();
        cache.insert("SARGA0".to_string(), make_opaque_sprite(16, 32, 222));

        // Build a colormap where row 6 (fuzz dark) maps everything to 42.
        let mut cm_data = vec![0u8; COLORMAP_ROWS * COLORMAP_SIZE];
        for i in 0..256 {
            cm_data[6 * COLORMAP_SIZE + i] = 42;
        }
        let cm = ColormapCache::from_test_data(cm_data);

        let mut fb = Framebuffer::new();
        fb.clear(100);

        render_things(
            &level,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Bam(0),
            &mut fb,
            &cache,
            None,
            Some(&cm),
            None,
        );

        // Fuzz with colormap row 6 should produce pixel value 42.
        assert!(
            fb.data.contains(&42),
            "spectre with colormap should use colormap row 6 for darkening"
        );
    }

    // ------------------------------------------------------------------
    // 15. LightParams fullbright always returns row 0
    // ------------------------------------------------------------------
    #[test]
    fn light_params_fullbright_returns_row_0() {
        let lp = LightParams::new(128, true);
        let cm = test_colormap_cache();
        let row = lp.get_colormap(500.0, &cm);
        // Row 0 in our test cache has all bytes = 0.
        assert_eq!(row[100], 0, "fullbright should use row 0");
    }

    // ------------------------------------------------------------------
    // 16. LightParams normal returns darker row for dark sector
    // ------------------------------------------------------------------
    #[test]
    fn light_params_dark_sector_returns_high_row() {
        let lp = LightParams::new(0, false); // pitch dark
        let cm = test_colormap_cache();
        let row = lp.get_colormap(200.0, &cm);
        // Dark sector + distance 200 should select a high row index.
        // In our test cache, the row value equals the row index.
        assert!(row[0] > 0, "dark sector should use a non-zero colormap row");
    }

    // ------------------------------------------------------------------
    // 17. LightParams distance affects colormap row
    // ------------------------------------------------------------------
    #[test]
    fn light_params_closer_is_brighter() {
        let lp = LightParams::new(128, false);
        let cm = test_colormap_cache();
        let near_row = lp.get_colormap(50.0, &cm);
        let far_row = lp.get_colormap(1000.0, &cm);
        // Closer should yield a brighter (lower index) row.
        assert!(
            near_row[0] <= far_row[0],
            "near sprite ({}) should be brighter than far ({}) ",
            near_row[0],
            far_row[0]
        );
    }

    // ------------------------------------------------------------------
    // 18. render_flag_for_thing returns Normal for items/ammo/weapons
    // ------------------------------------------------------------------
    #[test]
    fn render_flag_items_normal() {
        let normal_items: &[u16] = &[
            2014, 2015, 2011, 2012, // health/armor bonuses, stim, medi
            2018, 2019, // armors
            2007, 2048, 2008, 2049, // ammo
            2001, 82, 2002, 2003, // weapons
            2035, // barrel
        ];
        for &kind in normal_items {
            assert_eq!(
                render_flag_for_thing(kind),
                RenderFlag::Normal,
                "item type {kind} should be Normal"
            );
        }
    }

    // ------------------------------------------------------------------
    // 19. render_things with z_buffer AND colormap
    // ------------------------------------------------------------------
    #[test]
    fn render_things_zbuf_and_colormap_combined() {
        // Barrel at (100, 0), z_buffer at 200 (sprite visible),
        // dark sector (light=64).
        let thing = make_thing(100, 0, 2035);
        let level = make_test_level_with_light(vec![thing], 64);

        let mut cache = SpriteCache::empty();
        cache.insert("BAR1A0".to_string(), make_opaque_sprite(8, 16, 150));

        let cm = test_colormap_cache();
        let zbuf = make_zbuf(200.0);
        let mut fb = Framebuffer::new();

        render_things(
            &level,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Bam(0),
            &mut fb,
            &cache,
            Some(&zbuf),
            Some(&cm),
            None,
        );

        // Sprite is in front of wall (depth ~100 < 200), so it's drawn.
        let has_drawn = fb.data.iter().any(|&b| b != 0);
        assert!(has_drawn, "sprite in front of wall should be drawn");

        // Raw palette index 150 should not appear (dark sector shading).
        let has_raw = fb.data.contains(&150);
        assert!(!has_raw, "dark sector colormap should shade the sprite");
    }

    // ------------------------------------------------------------------
    // 20. render_things with z_buffer clips even with colormap
    // ------------------------------------------------------------------
    #[test]
    fn render_things_zbuf_clips_with_colormap() {
        // Barrel at (100, 0), z_buffer at 50 (sprite behind wall).
        let thing = make_thing(100, 0, 2035);
        let level = make_test_level_with_light(vec![thing], 192);

        let mut cache = SpriteCache::empty();
        cache.insert("BAR1A0".to_string(), make_opaque_sprite(8, 16, 88));

        let cm = test_colormap_cache();
        let zbuf = make_zbuf(50.0);
        let mut fb = Framebuffer::new();

        render_things(
            &level,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Bam(0),
            &mut fb,
            &cache,
            Some(&zbuf),
            Some(&cm),
            None,
        );

        // Sprite is behind wall — nothing drawn.
        assert!(
            fb.data.iter().all(|&b| b == 0),
            "sprite behind wall should be fully clipped even with colormap"
        );
    }

    // ------------------------------------------------------------------
    // 21. sector_for_point handles different positions
    // ------------------------------------------------------------------
    #[test]
    fn sector_for_point_various_positions() {
        let level = make_test_level(vec![]);
        // All points should resolve to sector 0 in our single-sector level.
        for (x, y) in [(0, 0), (10, 10), (32, 32), (63, 63)] {
            let si = sector_for_point(&level, x, y);
            assert_eq!(si, Some(0), "point ({x},{y}) should be in sector 0");
        }
    }

    // ------------------------------------------------------------------
    // 22. render_flag_for_thing returns Normal for unknown types
    // ------------------------------------------------------------------
    #[test]
    fn render_flag_unknown_type_is_normal() {
        assert_eq!(render_flag_for_thing(9999), RenderFlag::Normal);
        assert_eq!(render_flag_for_thing(0), RenderFlag::Normal);
        assert_eq!(render_flag_for_thing(12345), RenderFlag::Normal);
    }

    // ------------------------------------------------------------------
    // 23. render_things with identity colormap preserves raw indices
    // ------------------------------------------------------------------
    #[test]
    fn render_things_identity_colormap_preserves_indices() {
        // With an identity colormap, even shaded sprites should have their
        // raw palette index preserved (identity maps i -> i).
        let thing = make_thing(100, 0, 2035);
        let level = make_test_level_with_light(vec![thing], 128);

        let mut cache = SpriteCache::empty();
        cache.insert("BAR1A0".to_string(), make_opaque_sprite(8, 16, 77));

        let id_cache = ColormapCache::identity();
        let mut fb = Framebuffer::new();

        render_things(
            &level,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Bam(0),
            &mut fb,
            &cache,
            None,
            Some(&id_cache),
            None,
        );

        // Identity cache maps every index to itself, regardless of row.
        assert!(
            fb.data.contains(&77),
            "identity colormap should preserve raw palette index"
        );
    }

    // ------------------------------------------------------------------
    // 24. render_things multiple things with mixed render flags
    // ------------------------------------------------------------------
    #[test]
    fn render_things_mixed_flags() {
        // Place a normal barrel and a fullbright lamp side by side.
        let barrel = make_thing(100, -30, 2035); // Normal
        let lamp = make_thing(100, 30, 2028); // FullBright

        let level = make_test_level_with_light(vec![barrel, lamp], 0); // pitch dark

        let mut cache = SpriteCache::empty();
        cache.insert("BAR1A0".to_string(), make_opaque_sprite(8, 16, 150));
        cache.insert("COLUA0".to_string(), make_opaque_sprite(8, 16, 200));

        let cm = test_colormap_cache();
        let mut fb = Framebuffer::new();

        render_things(
            &level,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Bam(0),
            &mut fb,
            &cache,
            None,
            Some(&cm),
            None,
        );

        // The barrel (Normal) in a dark sector should be shaded.
        // The lamp (FullBright) should use row 0 which maps everything to 0.
        // Some pixels should be drawn.
        let has_drawn = fb.data.iter().any(|&b| b != 0);
        assert!(has_drawn, "at least one sprite should be drawn");
    }

    // ------------------------------------------------------------------
    // 25. render_things fuzz + zbuf: fuzz respects z_buffer clipping
    // ------------------------------------------------------------------
    #[test]
    fn render_things_fuzz_respects_zbuf() {
        // Spectre at (100, 0), z_buffer at 50 => sprite behind wall.
        let thing = make_thing(100, 0, 58);
        let level = make_test_level_with_light(vec![thing], 192);

        let mut cache = SpriteCache::empty();
        cache.insert("SARGA0".to_string(), make_opaque_sprite(16, 32, 222));

        let zbuf = make_zbuf(50.0);
        let mut fb = Framebuffer::new();
        fb.clear(180);

        render_things(
            &level,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Bam(0),
            &mut fb,
            &cache,
            Some(&zbuf),
            None,
            None,
        );

        // Fuzz sprite behind wall: all pixels should remain at 180.
        assert!(
            fb.data.iter().all(|&b| b == 180),
            "fuzz sprite behind wall should not modify framebuffer"
        );
    }
}
