//! DeHackEd `.deh` patch parser and applicator.
//!
//! Supports DeHackEd v3 (vanilla Doom format).  Parses Thing, Frame, Weapon,
//! Ammo, Misc, Text, `[CODEPTR]`, and `[STRINGS]` sections.
//!
//! # Usage
//! ```rust
//! use doom_game::dehacked::DehPatch;
//!
//! let patch = DehPatch::parse("Thing 1\nHit points = 200\n").expect("dehacked patch parse must succeed");
//! assert_eq!(patch.things[0].hit_points, Some(200));
//! ```

use std::collections::HashMap;

use crate::mobj::{MobjStateEntry, StateNum};
use crate::mobjinfo::MobjInfo;
use doom_types::Fixed16_16;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can arise while parsing or applying a `.deh` patch.
#[derive(Debug, thiserror::Error)]
pub enum DehError {
    /// The section header could not be recognised.
    #[error("invalid section header: {0}")]
    BadHeader(String),
    /// A field line was malformed.
    #[error("invalid field: {0}")]
    BadField(String),
    /// A numeric index exceeded the table bounds.
    #[error("index out of range: {0}")]
    OutOfRange(String),
}

// ---------------------------------------------------------------------------
// Patch data structures
// ---------------------------------------------------------------------------

/// Overrides for one entry in the `MOBJINFO` table.
#[derive(Debug, Default, Clone)]
pub struct ThingPatch {
    /// 0-based index into `MOBJINFO`.
    pub thing_num: usize,
    /// Override for `spawn_health`.
    pub hit_points: Option<i32>,
    /// Override for `speed` (map units -- stored as raw integer, applied as
    /// `Fixed16_16`).
    pub speed: Option<i32>,
    /// Override for `radius` (map units; multiplied by `FRACUNIT` on apply).
    pub radius: Option<i32>,
    /// Override for `height` (map units; multiplied by `FRACUNIT` on apply).
    pub height: Option<i32>,
    /// Override for `mass`.
    pub mass: Option<i32>,
    /// Override for `flags` bitmask.
    pub bits: Option<u32>,
    /// Override for `pain_chance`.
    pub pain_chance: Option<i32>,
    /// Damage value stored for completeness (not present in `MobjInfo`).
    pub damage: Option<i32>,
    /// Reaction time stored for completeness (not present in `MobjInfo`).
    pub reaction_time: Option<i32>,
    /// Override for spawn state.
    pub spawn_state: Option<u16>,
    /// Override for see state.
    pub see_state: Option<u16>,
    /// Override for pain state.
    pub pain_state: Option<u16>,
    /// Override for melee state.
    pub melee_state: Option<u16>,
    /// Override for missile state.
    pub missile_state: Option<u16>,
    /// Override for death state.
    pub death_state: Option<u16>,
}

/// Overrides for one entry in the `STATES` table.
#[derive(Debug, Default, Clone)]
pub struct FramePatch {
    /// 0-based index into `STATES`.
    pub frame_num: usize,
    /// Override for `MobjStateEntry::tics` (stored as `i32`, cast on apply).
    pub duration: Option<i32>,
    /// Override for `MobjStateEntry::next_state`.
    pub next_frame: Option<usize>,
    /// Sprite number override for `MobjStateEntry::sprite`.
    pub sprite_number: Option<u8>,
    /// Sprite subnumber override for `MobjStateEntry::frame`.
    pub sprite_subnumber: Option<u8>,
    /// Action override from `[CODEPTR]` section (index into action table).
    pub action: Option<u16>,
}

/// Overrides for one weapon entry.
#[derive(Debug, Default, Clone)]
pub struct WeaponPatch {
    /// 0-based weapon index.
    pub weapon_num: usize,
    /// Ammo type override (5 = no ammo).
    pub ammo_type: Option<i32>,
    /// Ammo per shot override.
    pub min_ammo: Option<i32>,
    /// Deselect state override.
    pub deselect_state: Option<u16>,
    /// Select state override.
    pub select_state: Option<u16>,
    /// Fire state override.
    pub fire_state: Option<u16>,
    /// Flash state override.
    pub flash_state: Option<u16>,
}

/// Overrides for ammo maximum and per-pickup values.
#[derive(Debug, Default, Clone)]
pub struct AmmoPatch {
    /// 0-based ammo index.
    pub ammo_num: usize,
    /// Override for max ammo.
    pub max_ammo: Option<i32>,
    /// Override for per-pickup amount.
    pub per_pickup: Option<i32>,
}

/// A text replacement entry.
#[derive(Debug, Clone)]
pub struct TextReplacement {
    /// Original text to find.
    pub old_text: String,
    /// Replacement text.
    pub new_text: String,
}

/// Miscellaneous value override.
#[derive(Debug, Clone)]
pub struct MiscPatch {
    /// Key name (e.g. "Initial Health").
    pub key: String,
    /// Override value.
    pub value: i32,
}

/// The full parsed patch from a `.deh` file.
#[derive(Debug, Default, Clone)]
pub struct DehPatch {
    /// Thing (MOBJINFO) overrides.
    pub things: Vec<ThingPatch>,
    /// Frame (STATES) overrides.
    pub frames: Vec<FramePatch>,
    /// Weapon overrides.
    pub weapons: Vec<WeaponPatch>,
    /// Ammo overrides.
    pub ammo: Vec<AmmoPatch>,
    /// Miscellaneous value overrides.
    pub misc: Vec<MiscPatch>,
    /// Text substitutions as structured `TextReplacement` entries.
    pub texts: Vec<TextReplacement>,
    /// String table overrides (`KEY -> value`).
    pub strings: HashMap<String, String>,
    /// Code pointer overrides from `[CODEPTR]` section.
    /// Maps frame number -> action name string.
    pub code_pointers: HashMap<usize, String>,
    /// Doom version from the header (if present).
    pub doom_version: Option<i32>,
    /// Patch format from the header (if present).
    pub patch_format: Option<i32>,
}

// ---------------------------------------------------------------------------
// Internal parser state
// ---------------------------------------------------------------------------

/// Tracks which section is currently being parsed.
enum Section {
    None,
    Thing(ThingPatch),
    Frame(FramePatch),
    Weapon(WeaponPatch),
    Ammo(AmmoPatch),
    Misc,
    Strings,
    CodePtr,
}

// ---------------------------------------------------------------------------
// DehPatch implementation
// ---------------------------------------------------------------------------

impl DehPatch {
    /// Parse a `.deh` text patch from a string slice.
    ///
    /// Returns `Err` if any section header or field value is malformed.
    /// Malformed inputs that can be skipped (e.g. informational headers) are
    /// silently ignored.
    ///
    /// # Errors
    /// Returns [`DehError::BadField`] if a value cannot be parsed as the
    /// expected numeric type.
    pub fn parse(input: &str) -> Result<Self, DehError> {
        let mut patch = Self::default();
        let mut section = Section::None;

        // We work character-by-character for the Text section and line-by-line
        // elsewhere.  Build a peekable iterator over lines with byte offsets so
        // we can consume raw bytes when needed.
        let mut remaining = input;

        loop {
            // Peel off one line.
            let (line, rest) = match remaining.find('\n') {
                Some(pos) => (&remaining[..pos], &remaining[pos + 1..]),
                None => {
                    // Last line (possibly empty).
                    let line = remaining;
                    // Process the last line, then break.
                    let trimmed = line.trim();
                    if !trimmed.is_empty() && !trimmed.starts_with('#') {
                        Self::process_line(trimmed, &mut section, &mut patch)?;
                    }
                    break;
                }
            };
            remaining = rest;

            let trimmed = line.trim();

            // Skip blank lines and comment lines.
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Check for Text section header: "Text <old_len> <new_len>"
            // We handle this specially because we must read raw bytes.
            if let Some(text_args) = trimmed.strip_prefix("Text ") {
                // Finalise whatever section was open.
                Self::finalise_section(section, &mut patch);
                section = Section::None;

                let mut iter = text_args.split_whitespace();
                let old_len: usize = iter
                    .next()
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| DehError::BadHeader(trimmed.to_owned()))?;
                let new_len: usize = iter
                    .next()
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| DehError::BadHeader(trimmed.to_owned()))?;

                // Consume exactly old_len + new_len bytes from `remaining`.
                // The bytes may span multiple lines; we treat newlines as part
                // of the data only where they fall within the counts.
                let total = old_len.checked_add(new_len).ok_or_else(|| {
                    DehError::BadHeader("Text section length overflow".to_owned())
                })?;

                // Havoc 👺: Defend against OOM and huge allocations from fuzzed lengths.
                // Doom's executable is ~700KB, so text replacements will never logically exceed 1MB.
                // Without this, the fuzzer (or an attacker) can trigger an OOM by requesting
                // a 4GB allocation via `old_len` or `new_len`.
                if total > 1024 * 1024 {
                    return Err(DehError::BadHeader(
                        "Text section length exceeds maximum permitted size".to_owned(),
                    ));
                }

                if remaining.len() < total {
                    return Err(DehError::BadHeader(
                        "Unexpected end of input in Text section".to_owned(),
                    ));
                }

                let old_text = remaining
                    .get(..old_len)
                    .ok_or_else(|| DehError::BadHeader("Invalid text byte boundary".to_owned()))?
                    .to_owned();
                let new_text = remaining
                    .get(old_len..total)
                    .ok_or_else(|| DehError::BadHeader("Invalid text byte boundary".to_owned()))?
                    .to_owned();
                patch.texts.push(TextReplacement { old_text, new_text });
                // Skip the consumed bytes plus any trailing newline.
                remaining = remaining
                    .get(total..)
                    .ok_or_else(|| DehError::BadHeader("Invalid text byte boundary".to_owned()))?;
                if remaining.starts_with('\n') {
                    remaining = &remaining[1..];
                }
                continue;
            }

            Self::process_line(trimmed, &mut section, &mut patch)?;
        }

        // Finalise the last open section.
        Self::finalise_section(section, &mut patch);

        Ok(patch)
    }

    /// Process a single non-empty, non-comment line.
    fn process_line(
        trimmed: &str,
        section: &mut Section,
        patch: &mut DehPatch,
    ) -> Result<(), DehError> {
        // --- Early return for sections whose field lines look like headers ---
        if Self::try_parse_codeptr_or_misc(trimmed, section, patch)? {
            return Ok(());
        }

        // --- Section header detection ---
        if Self::try_parse_preamble_or_header(trimmed, section, patch)? {
            return Ok(());
        }

        // --- Field line: "key = value" ---
        Self::parse_field(trimmed, section, patch)
    }

    /// Push a finished section into the appropriate `patch` vec.
    fn finalise_section(section: Section, patch: &mut DehPatch) {
        match section {
            Section::None | Section::Strings | Section::CodePtr | Section::Misc => {}
            Section::Thing(tp) => patch.things.push(tp),
            Section::Frame(fp) => patch.frames.push(fp),
            Section::Weapon(wp) => patch.weapons.push(wp),
            Section::Ammo(ap) => patch.ammo.push(ap),
        }
    }

    // -----------------------------------------------------------------------
    // Parsing helpers
    // -----------------------------------------------------------------------
    /// Returns true if the line looks like a common section header.
    fn is_common_header_for_misc(trimmed: &str) -> bool {
        trimmed.eq_ignore_ascii_case("[STRINGS]")
            || trimmed.eq_ignore_ascii_case("[CODEPTR]")
            || trimmed.starts_with("Thing ")
            || trimmed.starts_with("Frame ")
            || trimmed.starts_with("Weapon ")
            || trimmed.starts_with("Ammo ")
            || trimmed.starts_with("Misc")
            || trimmed.starts_with("Text ")
    }

    /// Tries to parse a line as a CodePtr or Misc field.
    /// Returns `Ok(true)` if it handled the line, or `Ok(false)` if it should
    /// fall through to regular section header detection.
    fn try_parse_codeptr_or_misc(
        trimmed: &str,
        section: &Section,
        patch: &mut DehPatch,
    ) -> Result<bool, DehError> {
        match section {
            Section::CodePtr => {
                if trimmed.eq_ignore_ascii_case("[STRINGS]")
                    || trimmed.eq_ignore_ascii_case("[CODEPTR]")
                    || trimmed.starts_with("Thing ")
                    || trimmed.starts_with("Weapon ")
                    || trimmed.starts_with("Ammo ")
                    || trimmed.starts_with("Misc")
                    || trimmed.starts_with("Text ")
                {
                    return Ok(false);
                }
                if let Some(eq_pos) = trimmed.find('=') {
                    let lhs = trimmed[..eq_pos].trim();
                    let rhs = trimmed[eq_pos + 1..].trim();
                    if let Some(frame_part) = lhs.strip_prefix("Frame ") {
                        if let Ok(frame_num) = frame_part.trim().parse::<usize>() {
                            patch.code_pointers.insert(frame_num, rhs.to_owned());
                        }
                    }
                }
                Ok(true)
            }
            Section::Misc => {
                if Self::is_common_header_for_misc(trimmed) {
                    return Ok(false);
                }
                if let Ok((key, val)) = Self::split_field(trimmed) {
                    if let Ok(v) = Self::parse_i32(val) {
                        patch.misc.push(MiscPatch {
                            key: key.to_owned(),
                            value: v,
                        });
                    }
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }
    /// Checks for a section header or preamble line.
    /// Returns `Ok(true)` if it successfully matched and processed a header.
    fn try_parse_preamble_or_header(
        trimmed: &str,
        section: &mut Section,
        patch: &mut DehPatch,
    ) -> Result<bool, DehError> {
        // [STRINGS] section
        if trimmed.eq_ignore_ascii_case("[STRINGS]") {
            let old = std::mem::replace(section, Section::Strings);
            Self::finalise_section(old, patch);
            return Ok(true);
        }

        // [CODEPTR] section
        if trimmed.eq_ignore_ascii_case("[CODEPTR]") {
            let old = std::mem::replace(section, Section::CodePtr);
            Self::finalise_section(old, patch);
            return Ok(true);
        }

        // Informational headers (patch file preamble)
        if let Some(rest) = trimmed.strip_prefix("Doom version = ") {
            if let Ok(v) = rest.trim().parse::<i32>() {
                patch.doom_version = Some(v);
            }
            return Ok(true);
        }
        if trimmed.starts_with("Doom version =") {
            return Ok(true);
        }
        if let Some(rest) = trimmed.strip_prefix("Patch format = ") {
            if let Ok(v) = rest.trim().parse::<i32>() {
                patch.patch_format = Some(v);
            }
            return Ok(true);
        }
        if trimmed.starts_with("Patch format =") {
            return Ok(true);
        }

        // Skip "Patch File for DeHackEd" preamble lines
        if trimmed.starts_with("Patch File for") {
            return Ok(true);
        }

        // Thing N [optional name]
        if let Some(thing_part) = trimmed.strip_prefix("Thing ") {
            let num_str = thing_part.split_whitespace().next().unwrap_or("");
            if let Ok(n) = num_str.parse::<usize>() {
                let old = std::mem::replace(
                    section,
                    Section::Thing(ThingPatch {
                        thing_num: n.saturating_sub(1), // 1-based -> 0-based
                        ..ThingPatch::default()
                    }),
                );
                Self::finalise_section(old, patch);
                return Ok(true);
            }
        }

        // Frame N
        if let Some(frame_part) = trimmed.strip_prefix("Frame ") {
            let num_str = frame_part.split_whitespace().next().unwrap_or("");
            if let Ok(n) = num_str.parse::<usize>() {
                let old = std::mem::replace(
                    section,
                    Section::Frame(FramePatch {
                        frame_num: n,
                        ..FramePatch::default()
                    }),
                );
                Self::finalise_section(old, patch);
                return Ok(true);
            }
        }

        // Weapon N [optional name]
        if let Some(weapon_part) = trimmed.strip_prefix("Weapon ") {
            let num_str = weapon_part.split_whitespace().next().unwrap_or("");
            if let Ok(n) = num_str.parse::<usize>() {
                let old = std::mem::replace(
                    section,
                    Section::Weapon(WeaponPatch {
                        weapon_num: n,
                        ..WeaponPatch::default()
                    }),
                );
                Self::finalise_section(old, patch);
                return Ok(true);
            }
        }

        // Ammo N [optional name]
        if let Some(ammo_part) = trimmed.strip_prefix("Ammo ") {
            let num_str = ammo_part.split_whitespace().next().unwrap_or("");
            if let Ok(n) = num_str.parse::<usize>() {
                let old = std::mem::replace(
                    section,
                    Section::Ammo(AmmoPatch {
                        ammo_num: n,
                        ..AmmoPatch::default()
                    }),
                );
                Self::finalise_section(old, patch);
                return Ok(true);
            }
        }

        // Misc N
        if trimmed.starts_with("Misc") {
            let old = std::mem::replace(section, Section::Misc);
            Self::finalise_section(old, patch);
            return Ok(true);
        }

        Ok(false)
    }
    /// Parses a "key = value" field line for the currently active section.
    fn parse_field(
        trimmed: &str,
        section: &mut Section,
        patch: &mut DehPatch,
    ) -> Result<(), DehError> {
        match section {
            Section::None | Section::CodePtr | Section::Misc => {
                // Outside a recognised section (or CodePtr/Misc already handled
                // above) -- ignore unknown fields.
            }
            Section::Thing(tp) => {
                let (key, val) = Self::split_field(trimmed)?;
                match key {
                    "Hit points" => tp.hit_points = Some(Self::parse_i32(val)?),
                    "Speed" => tp.speed = Some(Self::parse_i32(val)?),
                    "Radius" => tp.radius = Some(Self::parse_i32(val)?),
                    "Height" => tp.height = Some(Self::parse_i32(val)?),
                    "Damage" => tp.damage = Some(Self::parse_i32(val)?),
                    "Reaction time" => tp.reaction_time = Some(Self::parse_i32(val)?),
                    "Pain chance" => tp.pain_chance = Some(Self::parse_i32(val)?),
                    "Bits" => tp.bits = Some(Self::parse_u32(val)?),
                    "Mass" => tp.mass = Some(Self::parse_i32(val)?),
                    "Initial frame" => tp.spawn_state = Some(Self::parse_u16(val)?),
                    "First moving frame" => tp.see_state = Some(Self::parse_u16(val)?),
                    "Injury frame" => tp.pain_state = Some(Self::parse_u16(val)?),
                    "Close attack frame" => tp.melee_state = Some(Self::parse_u16(val)?),
                    "Far attack frame" => tp.missile_state = Some(Self::parse_u16(val)?),
                    "Death frame" => tp.death_state = Some(Self::parse_u16(val)?),
                    _ => {} // Unknown thing field -- silently ignore.
                }
            }
            Section::Frame(fp) => {
                let (key, val) = Self::split_field(trimmed)?;
                match key {
                    "Sprite number" => fp.sprite_number = Some(Self::parse_u8(val)?),
                    "Sprite subnumber" => fp.sprite_subnumber = Some(Self::parse_u8(val)?),
                    "Duration" => fp.duration = Some(Self::parse_i32(val)?),
                    "Next frame" => fp.next_frame = Some(Self::parse_usize(val)?),
                    _ => {}
                }
            }
            Section::Weapon(wp) => {
                let (key, val) = Self::split_field(trimmed)?;
                match key {
                    "Ammo type" => wp.ammo_type = Some(Self::parse_i32(val)?),
                    "Min Ammo" | "Ammo per shot" => wp.min_ammo = Some(Self::parse_i32(val)?),
                    "Deselect frame" => wp.deselect_state = Some(Self::parse_u16(val)?),
                    "Select frame" => wp.select_state = Some(Self::parse_u16(val)?),
                    "Bobbing frame" | "Fire frame" => wp.fire_state = Some(Self::parse_u16(val)?),
                    "Shooting frame" | "Flash frame" => {
                        wp.flash_state = Some(Self::parse_u16(val)?)
                    }
                    _ => {}
                }
            }
            Section::Ammo(ap) => {
                let (key, val) = Self::split_field(trimmed)?;
                match key {
                    "Max ammo" => ap.max_ammo = Some(Self::parse_i32(val)?),
                    "Per ammo" | "Per pickup" => ap.per_pickup = Some(Self::parse_i32(val)?),
                    _ => {}
                }
            }
            Section::Strings => {
                // Lines like: KEY = value (key is uppercase, no spaces)
                if let Some(eq_pos) = trimmed.find('=') {
                    let key = trimmed[..eq_pos].trim().to_owned();
                    let value = trimmed[eq_pos + 1..].trim().to_owned();
                    if !key.is_empty() {
                        patch.strings.insert(key, value);
                    }
                }
                // Lines without '=' are silently ignored inside [STRINGS].
            }
        }

        Ok(())
    }

    /// Split `"key = value"` into `(trimmed_key, trimmed_value)`.
    fn split_field(line: &str) -> Result<(&str, &str), DehError> {
        let pos = line
            .find('=')
            .ok_or_else(|| DehError::BadField(line.to_owned()))?;
        let key = line[..pos].trim();
        let val = line[pos + 1..].trim();
        Ok((key, val))
    }

    fn parse_float_fallback(s: &str) -> Result<f64, ()> {
        let v = s.parse::<f64>().map_err(|_| ())?;
        if v.is_nan() { Err(()) } else { Ok(v) }
    }

    fn parse_i32(s: &str) -> Result<i32, DehError> {
        s.parse::<i32>()
            .or_else(|_: std::num::ParseIntError| {
                let v = Self::parse_float_fallback(s)?;
                if v.is_infinite() {
                    if v.is_sign_positive() {
                        Ok(i32::MAX)
                    } else {
                        Ok(i32::MIN)
                    }
                } else {
                    Ok(v.clamp(i32::MIN as f64, i32::MAX as f64) as i32)
                }
            })
            .map_err(|_: ()| DehError::BadField(format!("expected i32, got {s:?}")))
    }

    fn parse_u32(s: &str) -> Result<u32, DehError> {
        s.parse::<u32>()
            .or_else(|_: std::num::ParseIntError| {
                let v = Self::parse_float_fallback(s)?;
                if v.is_infinite() {
                    if v.is_sign_positive() {
                        Ok(u32::MAX)
                    } else {
                        Ok(0)
                    }
                } else {
                    Ok(v.clamp(0.0, u32::MAX as f64) as u32)
                }
            })
            .map_err(|_: ()| DehError::BadField(format!("expected u32, got {s:?}")))
    }

    fn parse_u16(s: &str) -> Result<u16, DehError> {
        s.parse::<u16>()
            .or_else(|_: std::num::ParseIntError| {
                let v = Self::parse_float_fallback(s)?;
                if v.is_infinite() {
                    if v.is_sign_positive() {
                        Ok(u16::MAX)
                    } else {
                        Ok(0)
                    }
                } else {
                    Ok(v.clamp(0.0, u16::MAX as f64) as u16)
                }
            })
            .map_err(|_: ()| DehError::BadField(format!("expected u16, got {s:?}")))
    }

    fn parse_u8(s: &str) -> Result<u8, DehError> {
        s.parse::<u8>()
            .or_else(|_: std::num::ParseIntError| {
                let v = Self::parse_float_fallback(s)?;
                if v.is_infinite() {
                    if v.is_sign_positive() {
                        Ok(u8::MAX)
                    } else {
                        Ok(0)
                    }
                } else {
                    Ok(v.clamp(0.0, u8::MAX as f64) as u8)
                }
            })
            .map_err(|_: ()| DehError::BadField(format!("expected u8, got {s:?}")))
    }

    fn parse_usize(s: &str) -> Result<usize, DehError> {
        s.parse::<usize>()
            .or_else(|_: std::num::ParseIntError| {
                let v = Self::parse_float_fallback(s)?;
                if v.is_infinite() {
                    if v.is_sign_positive() {
                        Ok(usize::MAX)
                    } else {
                        Ok(0)
                    }
                } else if v < 0.0 {
                    Ok(0)
                } else if v > usize::MAX as f64 {
                    Ok(usize::MAX)
                } else {
                    Ok(v as usize)
                }
            })
            .map_err(|_: ()| DehError::BadField(format!("expected usize, got {s:?}")))
    }

    // -----------------------------------------------------------------------
    // Apply
    // -----------------------------------------------------------------------

    /// Apply this patch to mutable copies of the game tables.
    ///
    /// Returns the number of individual field modifications applied.
    ///
    /// # Errors
    /// Returns [`DehError::OutOfRange`] if any index exceeds the table length.
    ///
    /// # Notes
    /// - `MobjInfo::speed`, `radius`, and `height` are stored as
    ///   [`Fixed16_16`]; integer values from the patch are scaled by `65536`
    ///   (`FRACUNIT`) before assignment.
    /// - Frame fields `sprite_number` and `sprite_subnumber` are applied to
    ///   `MobjStateEntry::sprite` and `MobjStateEntry::frame` respectively.
    /// - Weapon patches are stored in [`WeaponPatch`] but the weapon table is
    ///   internal to `weapons.rs` and is not accessible here; those patches
    ///   must be applied by the caller if needed.
    /// - Ammo patches and text replacements are stored but not applied to game
    ///   tables in this function (would need the ammo table and string table).
    pub fn apply(
        &self,
        mobjinfo: &mut [MobjInfo],
        states: &mut [MobjStateEntry],
    ) -> Result<usize, DehError> {
        let mut count = 0usize;

        for patch in &self.things {
            let info = mobjinfo
                .get_mut(patch.thing_num)
                .ok_or_else(|| DehError::OutOfRange(format!("thing {}", patch.thing_num)))?;

            if let Some(hp) = patch.hit_points {
                info.spawn_health = hp;
                count += 1;
            }
            if let Some(sp) = patch.speed {
                info.speed = Fixed16_16(sp << 16);
                count += 1;
            }
            if let Some(r) = patch.radius {
                info.radius = Fixed16_16(r << 16);
                count += 1;
            }
            if let Some(h) = patch.height {
                info.height = Fixed16_16(h << 16);
                count += 1;
            }
            if let Some(m) = patch.mass {
                info.mass = m;
                count += 1;
            }
            if let Some(b) = patch.bits {
                info.flags = b;
                count += 1;
            }
            if let Some(pc) = patch.pain_chance {
                // pain_chance is u8; clamp silently rather than panic.
                info.pain_chance = pc.clamp(0, 255) as u8;
                count += 1;
            }
            if let Some(ss) = patch.spawn_state {
                info.spawn_state = StateNum(ss);
                count += 1;
            }
            if let Some(ss) = patch.see_state {
                info.see_state = StateNum(ss);
                count += 1;
            }
            if let Some(ps) = patch.pain_state {
                info.pain_state = StateNum(ps);
                count += 1;
            }
            if let Some(ms) = patch.melee_state {
                info.melee_state = StateNum(ms);
                count += 1;
            }
            if let Some(ms) = patch.missile_state {
                info.missile_state = StateNum(ms);
                count += 1;
            }
            if let Some(ds) = patch.death_state {
                info.death_state = StateNum(ds);
                count += 1;
            }
            // `damage` and `reaction_time` have no corresponding fields in
            // `MobjInfo`; they are stored in `ThingPatch` for completeness but
            // are not applied.
        }

        for patch in &self.frames {
            let state = states
                .get_mut(patch.frame_num)
                .ok_or_else(|| DehError::OutOfRange(format!("frame {}", patch.frame_num)))?;

            if let Some(d) = patch.duration {
                // MobjStateEntry::tics is i16.
                state.tics = d.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
                count += 1;
            }
            if let Some(nf) = patch.next_frame {
                state.next_state = StateNum(nf.min(u16::MAX as usize) as u16);
                count += 1;
            }
            if let Some(sn) = patch.sprite_number {
                state.sprite = sn as u16;
                count += 1;
            }
            if let Some(sf) = patch.sprite_subnumber {
                state.frame = sf;
                count += 1;
            }
        }

        // Weapon patches: WeaponInfo is crate-private in weapons.rs and
        // cannot be mutated here.  Callers that need weapon patching must
        // apply WeaponPatch entries themselves against their own weapon table.

        // Ammo patches: stored but not applied here (no ammo table reference).

        // Text replacements: stored but not applied (no string table).

        Ok(count)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobjinfo::MOBJINFO;
    use crate::states::STATES;

    // -----------------------------------------------------------------------
    // 1. Empty input
    // -----------------------------------------------------------------------

    #[test]
    fn deh_parse_empty_input() {
        let patch = DehPatch::parse("").expect("empty input must parse OK");
        assert!(patch.things.is_empty());
        assert!(patch.frames.is_empty());
        assert!(patch.weapons.is_empty());
        assert!(patch.ammo.is_empty());
        assert!(patch.misc.is_empty());
        assert!(patch.texts.is_empty());
        assert!(patch.strings.is_empty());
        assert!(patch.code_pointers.is_empty());
    }

    // -----------------------------------------------------------------------
    // 2. Thing hit points
    // -----------------------------------------------------------------------

    #[test]
    fn deh_parse_thing_hit_points() {
        let patch = DehPatch::parse("Thing 1\nHit points = 200\n")
            .expect("valid Thing section must parse OK");
        assert_eq!(patch.things.len(), 1);
        let t = &patch.things[0];
        assert_eq!(t.thing_num, 0, "Thing 1 is 1-based -> 0-based index 0");
        assert_eq!(t.hit_points, Some(200));
    }

    // -----------------------------------------------------------------------
    // 3. Thing with multiple fields
    // -----------------------------------------------------------------------

    #[test]
    fn deh_parse_thing_multiple_fields() {
        let input = "\
Thing 2 (Shotgun Guy)
Hit points = 100
Speed = 12
Radius = 30
Height = 64
Mass = 200
Pain chance = 150
";
        let patch = DehPatch::parse(input).expect("multi-field Thing must parse OK");
        assert_eq!(patch.things.len(), 1);
        let t = &patch.things[0];
        assert_eq!(t.thing_num, 1);
        assert_eq!(t.hit_points, Some(100));
        assert_eq!(t.speed, Some(12));
        assert_eq!(t.radius, Some(30));
        assert_eq!(t.height, Some(64));
        assert_eq!(t.mass, Some(200));
        assert_eq!(t.pain_chance, Some(150));
    }

    // -----------------------------------------------------------------------
    // 4. Frame duration
    // -----------------------------------------------------------------------

    #[test]
    fn deh_parse_frame_duration() {
        let patch =
            DehPatch::parse("Frame 5\nDuration = 10\n").expect("valid Frame section must parse OK");
        assert_eq!(patch.frames.len(), 1);
        let f = &patch.frames[0];
        assert_eq!(f.frame_num, 5);
        assert_eq!(f.duration, Some(10));
    }

    // -----------------------------------------------------------------------
    // 5. Frame next_state
    // -----------------------------------------------------------------------

    #[test]
    fn deh_parse_frame_next_state() {
        let input = "Frame 10\nDuration = 4\nNext frame = 11\n";
        let patch = DehPatch::parse(input).expect("Frame next_state must parse OK");
        assert_eq!(patch.frames.len(), 1);
        let f = &patch.frames[0];
        assert_eq!(f.frame_num, 10);
        assert_eq!(f.duration, Some(4));
        assert_eq!(f.next_frame, Some(11));
    }

    // -----------------------------------------------------------------------
    // 6. Weapon ammo type
    // -----------------------------------------------------------------------

    #[test]
    fn deh_parse_weapon_ammo_type() {
        let patch = DehPatch::parse("Weapon 2\nAmmo type = 1\n")
            .expect("valid Weapon section must parse OK");
        assert_eq!(patch.weapons.len(), 1);
        let w = &patch.weapons[0];
        assert_eq!(w.weapon_num, 2);
        assert_eq!(w.ammo_type, Some(1));
    }

    // -----------------------------------------------------------------------
    // 7. Weapon section (full)
    // -----------------------------------------------------------------------

    #[test]
    fn deh_parse_weapon_full() {
        let input = "\
Weapon 0 (Fist)
Ammo type = 5
Ammo per shot = 0
";
        let patch = DehPatch::parse(input).expect("full weapon section must parse OK");
        assert_eq!(patch.weapons.len(), 1);
        let w = &patch.weapons[0];
        assert_eq!(w.weapon_num, 0);
        assert_eq!(w.ammo_type, Some(5));
        assert_eq!(w.min_ammo, Some(0));
    }

    // -----------------------------------------------------------------------
    // 8. Text section
    // -----------------------------------------------------------------------

    #[test]
    fn deh_parse_text_section() {
        // "Text 3 4" -> read 3 bytes ("old") then 4 bytes ("neww")
        let patch =
            DehPatch::parse("Text 3 4\noldneww\n").expect("valid Text section must parse OK");
        assert_eq!(patch.texts.len(), 1);
        assert_eq!(patch.texts[0].old_text, "old");
        assert_eq!(patch.texts[0].new_text, "neww");
    }

    // -----------------------------------------------------------------------
    // 9. Comment lines ignored
    // -----------------------------------------------------------------------

    #[test]
    fn deh_parse_ignores_comments() {
        let input = "# This is a comment\nThing 1\n# Another comment\nHit points = 50\n";
        let patch = DehPatch::parse(input).expect("commented input must parse OK");
        assert_eq!(patch.things.len(), 1);
        assert_eq!(patch.things[0].hit_points, Some(50));
    }

    // -----------------------------------------------------------------------
    // 10. Multiple sections in one file
    // -----------------------------------------------------------------------

    #[test]
    fn deh_parse_multiple_sections() {
        let input = "\
Thing 1
Hit points = 200

Frame 1
Duration = 8

Weapon 0 (Fist)
Ammo type = 5

Ammo 0 (Bullets)
Max ammo = 400
";
        let patch = DehPatch::parse(input).expect("multi-section must parse OK");
        assert_eq!(patch.things.len(), 1);
        assert_eq!(patch.things[0].hit_points, Some(200));
        assert_eq!(patch.frames.len(), 1);
        assert_eq!(patch.frames[0].duration, Some(8));
        assert_eq!(patch.weapons.len(), 1);
        assert_eq!(patch.weapons[0].ammo_type, Some(5));
        assert_eq!(patch.ammo.len(), 1);
        assert_eq!(patch.ammo[0].max_ammo, Some(400));
    }

    // -----------------------------------------------------------------------
    // 11. CODEPTR section
    // -----------------------------------------------------------------------

    #[test]
    fn deh_parse_codeptr_section() {
        let input = "\
[CODEPTR]
Frame 10 = A_FireBFG
Frame 20 = A_Chase
";
        let patch = DehPatch::parse(input).expect("CODEPTR section must parse OK");
        assert_eq!(patch.code_pointers.len(), 2);
        assert_eq!(
            patch.code_pointers.get(&10).map(String::as_str),
            Some("A_FireBFG")
        );
        assert_eq!(
            patch.code_pointers.get(&20).map(String::as_str),
            Some("A_Chase")
        );
    }

    // -----------------------------------------------------------------------
    // 12. Misc section
    // -----------------------------------------------------------------------

    #[test]
    fn deh_parse_misc_section() {
        let input = "\
Misc 0
Initial Health = 200
Initial Bullets = 100
";
        let patch = DehPatch::parse(input).expect("Misc section must parse OK");
        assert_eq!(patch.misc.len(), 2);
        assert_eq!(patch.misc[0].key, "Initial Health");
        assert_eq!(patch.misc[0].value, 200);
        assert_eq!(patch.misc[1].key, "Initial Bullets");
        assert_eq!(patch.misc[1].value, 100);
    }

    // -----------------------------------------------------------------------
    // 13. Ammo section
    // -----------------------------------------------------------------------

    #[test]
    fn deh_parse_ammo_section() {
        let input = "\
Ammo 0 (Bullets)
Max ammo = 400
Per ammo = 10
";
        let patch = DehPatch::parse(input).expect("Ammo section must parse OK");
        assert_eq!(patch.ammo.len(), 1);
        let a = &patch.ammo[0];
        assert_eq!(a.ammo_num, 0);
        assert_eq!(a.max_ammo, Some(400));
        assert_eq!(a.per_pickup, Some(10));
    }

    // -----------------------------------------------------------------------
    // 14. Apply thing health to MOBJINFO clone
    // -----------------------------------------------------------------------

    #[test]
    fn deh_apply_thing_health() {
        let patch = DehPatch::parse("Thing 1\nHit points = 999\n").expect("parse must succeed");

        let mut mobjinfo: Vec<MobjInfo> = MOBJINFO.to_vec();
        let mut states: Vec<MobjStateEntry> = STATES.to_vec();

        let count = patch
            .apply(&mut mobjinfo, &mut states)
            .expect("apply must succeed");

        assert_eq!(
            mobjinfo[0].spawn_health, 999,
            "thing 1 (index 0) spawn_health must be updated to 999"
        );
        assert_eq!(count, 1, "one field was modified");
    }

    // -----------------------------------------------------------------------
    // 15. Apply thing speed
    // -----------------------------------------------------------------------

    #[test]
    fn deh_apply_thing_speed() {
        let patch = DehPatch::parse("Thing 2\nSpeed = 16\n").expect("parse must succeed");

        let mut mobjinfo: Vec<MobjInfo> = MOBJINFO.to_vec();
        let mut states: Vec<MobjStateEntry> = STATES.to_vec();

        patch
            .apply(&mut mobjinfo, &mut states)
            .expect("apply must succeed");

        assert_eq!(
            mobjinfo[1].speed,
            Fixed16_16(16 << 16),
            "thing 2 (index 1) speed must be updated to fixed(16)"
        );
    }

    // -----------------------------------------------------------------------
    // 16. Apply state tics
    // -----------------------------------------------------------------------

    #[test]
    fn deh_apply_state_tics() {
        let patch = DehPatch::parse("Frame 1\nDuration = 20\n").expect("parse must succeed");

        let mut mobjinfo: Vec<MobjInfo> = MOBJINFO.to_vec();
        let mut states: Vec<MobjStateEntry> = STATES.to_vec();

        patch
            .apply(&mut mobjinfo, &mut states)
            .expect("apply must succeed");

        assert_eq!(states[1].tics, 20, "frame 1 tics must be updated to 20");
    }

    // -----------------------------------------------------------------------
    // 17. Apply state next_state
    // -----------------------------------------------------------------------

    #[test]
    fn deh_apply_state_next_state() {
        let patch = DehPatch::parse("Frame 1\nNext frame = 5\n").expect("parse must succeed");

        let mut mobjinfo: Vec<MobjInfo> = MOBJINFO.to_vec();
        let mut states: Vec<MobjStateEntry> = STATES.to_vec();

        patch
            .apply(&mut mobjinfo, &mut states)
            .expect("apply must succeed");

        assert_eq!(
            states[1].next_state,
            StateNum(5),
            "frame 1 next_state must be updated to 5"
        );
    }

    // -----------------------------------------------------------------------
    // 18. Apply with out-of-range index returns Err
    // -----------------------------------------------------------------------

    #[test]
    fn deh_apply_out_of_range_thing() {
        // Construct a ThingPatch with an impossibly large index directly.
        let patch = DehPatch {
            things: vec![ThingPatch {
                thing_num: 9999,
                hit_points: Some(1),
                ..ThingPatch::default()
            }],
            ..DehPatch::default()
        };

        let mut mobjinfo: Vec<MobjInfo> = MOBJINFO.to_vec();
        let mut states: Vec<MobjStateEntry> = STATES.to_vec();

        let result = patch.apply(&mut mobjinfo, &mut states);
        assert!(
            matches!(result, Err(DehError::OutOfRange(_))),
            "out-of-range thing index must return DehError::OutOfRange"
        );
    }

    // -----------------------------------------------------------------------
    // 19. Apply with out-of-range frame returns Err
    // -----------------------------------------------------------------------

    #[test]
    fn deh_apply_out_of_range_frame() {
        let patch = DehPatch {
            frames: vec![FramePatch {
                frame_num: 9999,
                duration: Some(1),
                ..FramePatch::default()
            }],
            ..DehPatch::default()
        };

        let mut mobjinfo: Vec<MobjInfo> = MOBJINFO.to_vec();
        let mut states: Vec<MobjStateEntry> = STATES.to_vec();

        let result = patch.apply(&mut mobjinfo, &mut states);
        assert!(
            matches!(result, Err(DehError::OutOfRange(_))),
            "out-of-range frame index must return DehError::OutOfRange"
        );
    }

    // -----------------------------------------------------------------------
    // 20. ThingPatch default has all None fields
    // -----------------------------------------------------------------------

    #[test]
    fn havoc_test_nan_does_not_panic_parser() {
        let result = DehPatch::parse("Thing 1\nHit points = NaN\n");
        assert!(result.is_err());
    }

    #[test]
    fn thing_patch_default_all_none() {
        let tp = ThingPatch::default();
        assert_eq!(tp.thing_num, 0);
        assert!(tp.hit_points.is_none());
        assert!(tp.speed.is_none());
        assert!(tp.radius.is_none());
        assert!(tp.height.is_none());
        assert!(tp.mass.is_none());
        assert!(tp.bits.is_none());
        assert!(tp.pain_chance.is_none());
        assert!(tp.damage.is_none());
        assert!(tp.reaction_time.is_none());
        assert!(tp.spawn_state.is_none());
        assert!(tp.see_state.is_none());
        assert!(tp.pain_state.is_none());
        assert!(tp.melee_state.is_none());
        assert!(tp.missile_state.is_none());
        assert!(tp.death_state.is_none());
    }

    // -----------------------------------------------------------------------
    // 21. DehPatch default is empty
    // -----------------------------------------------------------------------

    #[test]
    fn deh_patch_default_is_empty() {
        let patch = DehPatch::default();
        assert!(patch.things.is_empty());
        assert!(patch.frames.is_empty());
        assert!(patch.weapons.is_empty());
        assert!(patch.ammo.is_empty());
        assert!(patch.misc.is_empty());
        assert!(patch.texts.is_empty());
        assert!(patch.strings.is_empty());
        assert!(patch.code_pointers.is_empty());
        assert!(patch.doom_version.is_none());
        assert!(patch.patch_format.is_none());
    }

    // -----------------------------------------------------------------------
    // 22. Full patch parse + apply roundtrip
    // -----------------------------------------------------------------------

    #[test]
    fn deh_full_roundtrip() {
        let input = "\
Patch File for DeHackEd v3.0
Doom version = 19
Patch format = 6

Thing 1 (Player)
Hit points = 200
Speed = 0

Thing 2 (Trooper)
Hit points = 50
Mass = 200

Frame 1
Duration = 20
Next frame = 3

Text 4 6
PISTPISTOL
";
        let patch = DehPatch::parse(input).expect("full roundtrip parse must succeed");
        assert_eq!(patch.doom_version, Some(19));
        assert_eq!(patch.patch_format, Some(6));
        assert_eq!(patch.things.len(), 2);
        assert_eq!(patch.frames.len(), 1);
        assert_eq!(patch.texts.len(), 1);
        assert_eq!(patch.texts[0].old_text, "PIST");
        assert_eq!(patch.texts[0].new_text, "PISTOL");

        let mut mobjinfo: Vec<MobjInfo> = MOBJINFO.to_vec();
        let mut states: Vec<MobjStateEntry> = STATES.to_vec();

        let count = patch
            .apply(&mut mobjinfo, &mut states)
            .expect("full roundtrip apply must succeed");

        // Player: health + speed = 2 fields
        // Trooper: health + mass = 2 fields
        // Frame 1: duration + next_frame = 2 fields
        assert_eq!(count, 6);
        assert_eq!(mobjinfo[0].spawn_health, 200);
        assert_eq!(mobjinfo[0].speed, Fixed16_16(0));
        assert_eq!(mobjinfo[1].spawn_health, 50);
        assert_eq!(mobjinfo[1].mass, 200);
        assert_eq!(states[1].tics, 20);
        assert_eq!(states[1].next_state, StateNum(3));
    }

    // -----------------------------------------------------------------------
    // 23. Windows line endings (CRLF)
    // -----------------------------------------------------------------------

    #[test]
    fn deh_parse_crlf_line_endings() {
        let input = "Thing 1\r\nHit points = 200\r\nSpeed = 5\r\n";
        let patch = DehPatch::parse(input).expect("CRLF input must parse OK");
        assert_eq!(patch.things.len(), 1);
        assert_eq!(patch.things[0].hit_points, Some(200));
        assert_eq!(patch.things[0].speed, Some(5));
    }

    // -----------------------------------------------------------------------
    // 24. Parse patch format header
    // -----------------------------------------------------------------------

    #[test]
    fn deh_parse_patch_format_header() {
        let input = "\
Patch File for DeHackEd v3.0
Doom version = 19
Patch format = 6
";
        let patch = DehPatch::parse(input).expect("header must parse OK");
        assert_eq!(patch.doom_version, Some(19));
        assert_eq!(patch.patch_format, Some(6));
        // No actual patches were defined.
        assert!(patch.things.is_empty());
        assert!(patch.frames.is_empty());
    }

    // -----------------------------------------------------------------------
    // 25. [STRINGS] section
    // -----------------------------------------------------------------------

    #[test]
    fn deh_parse_strings_section() {
        let patch = DehPatch::parse("[STRINGS]\nGOTARMOR = Got it!\n")
            .expect("[STRINGS] section must parse OK");
        assert_eq!(
            patch.strings.get("GOTARMOR").map(String::as_str),
            Some("Got it!")
        );
    }

    // -----------------------------------------------------------------------
    // 26. Apply thing mass
    // -----------------------------------------------------------------------

    #[test]
    fn deh_apply_thing_mass() {
        let patch = DehPatch::parse("Thing 5 (Demon)\nMass = 9999\n").expect("parse");

        let mut mobjinfo: Vec<MobjInfo> = MOBJINFO.to_vec();
        let mut states: Vec<MobjStateEntry> = STATES.to_vec();

        patch.apply(&mut mobjinfo, &mut states).expect("apply");
        assert_eq!(mobjinfo[4].mass, 9999);
    }

    // -----------------------------------------------------------------------
    // 27. FramePatch default has all None fields
    // -----------------------------------------------------------------------

    #[test]
    fn frame_patch_default_all_none() {
        let fp = FramePatch::default();
        assert_eq!(fp.frame_num, 0);
        assert!(fp.duration.is_none());
        assert!(fp.next_frame.is_none());
        assert!(fp.sprite_number.is_none());
        assert!(fp.sprite_subnumber.is_none());
        assert!(fp.action.is_none());
    }

    // -----------------------------------------------------------------------
    // 28. WeaponPatch default
    // -----------------------------------------------------------------------

    #[test]
    fn weapon_patch_default_all_none() {
        let wp = WeaponPatch::default();
        assert_eq!(wp.weapon_num, 0);
        assert!(wp.ammo_type.is_none());
        assert!(wp.min_ammo.is_none());
        assert!(wp.deselect_state.is_none());
        assert!(wp.select_state.is_none());
        assert!(wp.fire_state.is_none());
        assert!(wp.flash_state.is_none());
    }

    // -----------------------------------------------------------------------
    // 29. AmmoPatch default
    // -----------------------------------------------------------------------

    #[test]
    fn ammo_patch_default_all_none() {
        let ap = AmmoPatch::default();
        assert_eq!(ap.ammo_num, 0);
        assert!(ap.max_ammo.is_none());
        assert!(ap.per_pickup.is_none());
    }

    // -----------------------------------------------------------------------
    // 30. Unknown sections are skipped gracefully
    // -----------------------------------------------------------------------

    #[test]
    fn deh_parse_unknown_sections_skipped() {
        let input = "\
SomeUnknownSection 42
Mystery field = 99

Thing 1
Hit points = 77
";
        // The "SomeUnknownSection" line will be ignored because it doesn't
        // match any known section prefix. "Mystery field = 99" falls into
        // Section::None and is also ignored.
        let patch = DehPatch::parse(input).expect("unknown section should be skipped");
        assert_eq!(patch.things.len(), 1);
        assert_eq!(patch.things[0].hit_points, Some(77));
    }

    // -----------------------------------------------------------------------
    // 31. Apply preserves unmodified entries
    // -----------------------------------------------------------------------

    #[test]
    fn deh_apply_preserves_unmodified() {
        let patch = DehPatch::parse("Thing 1\nHit points = 999\n").expect("parse");

        let mut mobjinfo: Vec<MobjInfo> = MOBJINFO.to_vec();
        let mut states: Vec<MobjStateEntry> = STATES.to_vec();

        let original_trooper_speed = mobjinfo[1].speed;
        let original_state1_tics = states[1].tics;

        patch.apply(&mut mobjinfo, &mut states).expect("apply");

        // Player (index 0) health was changed.
        assert_eq!(mobjinfo[0].spawn_health, 999);
        // Trooper (index 1) was NOT in the patch -- must remain unchanged.
        assert_eq!(mobjinfo[1].speed, original_trooper_speed);
        // States must remain unchanged.
        assert_eq!(states[1].tics, original_state1_tics);
    }

    // -----------------------------------------------------------------------
    // 32. Multiple CODEPTR entries
    // -----------------------------------------------------------------------

    #[test]
    fn deh_parse_codeptr_multiple() {
        let input = "\
[CODEPTR]
Frame 1 = A_Look
Frame 2 = A_Chase
Frame 50 = A_PosAttack
";
        let patch = DehPatch::parse(input).expect("parse");
        assert_eq!(patch.code_pointers.len(), 3);
        assert_eq!(
            patch.code_pointers.get(&1).map(String::as_str),
            Some("A_Look")
        );
        assert_eq!(
            patch.code_pointers.get(&2).map(String::as_str),
            Some("A_Chase")
        );
        assert_eq!(
            patch.code_pointers.get(&50).map(String::as_str),
            Some("A_PosAttack")
        );
    }

    // -----------------------------------------------------------------------
    // 33. Apply count is zero for empty patch
    // -----------------------------------------------------------------------

    #[test]
    fn deh_apply_empty_patch_zero_count() {
        let patch = DehPatch::default();
        let mut mobjinfo: Vec<MobjInfo> = MOBJINFO.to_vec();
        let mut states: Vec<MobjStateEntry> = STATES.to_vec();
        let count = patch.apply(&mut mobjinfo, &mut states).expect("apply");
        assert_eq!(count, 0);
    }

    // -----------------------------------------------------------------------
    // 34. Thing state overrides parse correctly
    // -----------------------------------------------------------------------

    #[test]
    fn deh_parse_thing_state_overrides() {
        let input = "\
Thing 2
Initial frame = 10
First moving frame = 11
Injury frame = 27
Close attack frame = 58
Far attack frame = 49
Death frame = 25
";
        let patch = DehPatch::parse(input).expect("parse");
        let t = &patch.things[0];
        assert_eq!(t.thing_num, 1);
        assert_eq!(t.spawn_state, Some(10));
        assert_eq!(t.see_state, Some(11));
        assert_eq!(t.pain_state, Some(27));
        assert_eq!(t.melee_state, Some(58));
        assert_eq!(t.missile_state, Some(49));
        assert_eq!(t.death_state, Some(25));
    }
}

#[cfg(test)]
mod tests_deh {
    use super::*;

    #[test]
    fn test_dehacked_parse_panic() {
        assert!(DehPatch::parse("Text 1 1\n\n").is_err());
    }

    #[test]
    fn test_dehacked_parse_panic_byte_index() {
        assert!(DehPatch::parse("Text 1 1\n😊").is_err());
    }

    #[test]
    fn test_dehacked_parse_text_overflow() {
        assert!(DehPatch::parse("Text 18446744073709551615 1\n").is_err());
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn parser_does_not_panic(s in "\\PC*") {
            let _ = DehPatch::parse(&s);
        }

        #[test]
        fn havoc_parse_does_not_panic_on_massive_digits(s in "[0-9]{50,100}") {
            assert!(DehPatch::parse_i32(&s).is_ok());
            assert!(DehPatch::parse_u32(&s).is_ok());
            assert!(DehPatch::parse_u16(&s).is_ok());
            assert!(DehPatch::parse_u8(&s).is_ok());
            assert!(DehPatch::parse_usize(&s).is_ok());
        }
    }
}
