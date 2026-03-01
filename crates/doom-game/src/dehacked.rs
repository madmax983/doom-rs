//! DeHackEd `.deh` patch parser and applicator.
//!
//! Supports DeHackEd v3 (vanilla Doom format).  Parses Thing, Frame, Weapon,
//! Text, and `[STRINGS]` sections.
//!
//! # Usage
//! ```rust
//! use doom_game::dehacked::DehPatch;
//!
//! let patch = DehPatch::parse("Thing 1\nHit points = 200\n").unwrap();
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
    /// Override for `speed` (map units — stored as raw integer, applied as
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
    /// Sprite number — stored but not applied (no `sprite` field on `MobjStateEntry`).
    pub sprite_number: Option<u8>,
    /// Sprite subnumber — stored but not applied (no `frame` field on `MobjStateEntry`).
    pub sprite_subnumber: Option<u8>,
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
    /// Text substitutions: `(old_text, new_text)`.
    pub texts: Vec<(String, String)>,
    /// String table overrides (`KEY → value`).
    pub strings: HashMap<String, String>,
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
    Strings,
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
                let total = old_len + new_len;
                if remaining.len() < total {
                    // Not enough data — store what we have and stop.
                    let old_text = remaining.get(..old_len.min(remaining.len()))
                        .unwrap_or(remaining)
                        .to_owned();
                    let start = old_len.min(remaining.len());
                    let new_text = remaining.get(start..)
                        .unwrap_or("")
                        .to_owned();
                    patch.texts.push((old_text, new_text));
                    break;
                }

                let old_text = remaining[..old_len].to_owned();
                let new_text = remaining[old_len..total].to_owned();
                patch.texts.push((old_text, new_text));
                // Skip the consumed bytes plus any trailing newline.
                remaining = &remaining[total..];
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
    fn process_line<'a>(
        trimmed: &'a str,
        section: &mut Section,
        patch: &mut DehPatch,
    ) -> Result<(), DehError> {
        // --- Section header detection ---

        // [STRINGS] section
        if trimmed.eq_ignore_ascii_case("[STRINGS]") {
            let old = std::mem::replace(section, Section::Strings);
            Self::finalise_section(old, patch);
            return Ok(());
        }

        // Informational / ignored headers
        if trimmed.starts_with("Doom version =") || trimmed.starts_with("Patch format =") {
            return Ok(());
        }

        // Thing N [optional name]
        if let Some(thing_part) = trimmed.strip_prefix("Thing ") {
            let num_str = thing_part
                .split_whitespace()
                .next()
                .unwrap_or("");
            if let Ok(n) = num_str.parse::<usize>() {
                let old = std::mem::replace(
                    section,
                    Section::Thing(ThingPatch {
                        thing_num: n.saturating_sub(1), // 1-based → 0-based
                        ..ThingPatch::default()
                    }),
                );
                Self::finalise_section(old, patch);
                return Ok(());
            }
        }

        // Frame N
        if let Some(frame_part) = trimmed.strip_prefix("Frame ") {
            let num_str = frame_part
                .split_whitespace()
                .next()
                .unwrap_or("");
            if let Ok(n) = num_str.parse::<usize>() {
                let old = std::mem::replace(
                    section,
                    Section::Frame(FramePatch {
                        frame_num: n,
                        ..FramePatch::default()
                    }),
                );
                Self::finalise_section(old, patch);
                return Ok(());
            }
        }

        // Weapon N [optional name]
        if let Some(weapon_part) = trimmed.strip_prefix("Weapon ") {
            let num_str = weapon_part
                .split_whitespace()
                .next()
                .unwrap_or("");
            if let Ok(n) = num_str.parse::<usize>() {
                let old = std::mem::replace(
                    section,
                    Section::Weapon(WeaponPatch {
                        weapon_num: n,
                        ..WeaponPatch::default()
                    }),
                );
                Self::finalise_section(old, patch);
                return Ok(());
            }
        }

        // --- Field line: "key = value" ---
        match section {
            Section::None => {
                // Outside a recognised section — ignore unknown fields.
            }
            Section::Thing(tp) => {
                let (key, val) = Self::split_field(trimmed)?;
                match key {
                    "Hit points"     => tp.hit_points    = Some(Self::parse_i32(val)?),
                    "Speed"          => tp.speed         = Some(Self::parse_i32(val)?),
                    "Radius"         => tp.radius        = Some(Self::parse_i32(val)?),
                    "Height"         => tp.height        = Some(Self::parse_i32(val)?),
                    "Damage"         => tp.damage        = Some(Self::parse_i32(val)?),
                    "Reaction time"  => tp.reaction_time = Some(Self::parse_i32(val)?),
                    "Pain chance"    => tp.pain_chance   = Some(Self::parse_i32(val)?),
                    "Bits"           => tp.bits          = Some(Self::parse_u32(val)?),
                    "Mass"           => tp.mass          = Some(Self::parse_i32(val)?),
                    _ => {} // Unknown thing field — silently ignore.
                }
            }
            Section::Frame(fp) => {
                let (key, val) = Self::split_field(trimmed)?;
                match key {
                    "Sprite number"    => fp.sprite_number    = Some(Self::parse_u8(val)?),
                    "Sprite subnumber" => fp.sprite_subnumber = Some(Self::parse_u8(val)?),
                    "Duration"         => fp.duration         = Some(Self::parse_i32(val)?),
                    "Next frame"       => fp.next_frame       = Some(Self::parse_usize(val)?),
                    _ => {}
                }
            }
            Section::Weapon(wp) => {
                let (key, val) = Self::split_field(trimmed)?;
                match key {
                    "Ammo type" => wp.ammo_type = Some(Self::parse_i32(val)?),
                    "Min Ammo"  => wp.min_ammo  = Some(Self::parse_i32(val)?),
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

    /// Push a finished section into the appropriate `patch` vec.
    fn finalise_section(section: Section, patch: &mut DehPatch) {
        match section {
            Section::None | Section::Strings => {}
            Section::Thing(tp) => patch.things.push(tp),
            Section::Frame(fp) => patch.frames.push(fp),
            Section::Weapon(wp) => patch.weapons.push(wp),
        }
    }

    // -----------------------------------------------------------------------
    // Parsing helpers
    // -----------------------------------------------------------------------

    /// Split `"key = value"` into `(trimmed_key, trimmed_value)`.
    fn split_field(line: &str) -> Result<(&str, &str), DehError> {
        let pos = line
            .find('=')
            .ok_or_else(|| DehError::BadField(line.to_owned()))?;
        let key = line[..pos].trim();
        let val = line[pos + 1..].trim();
        Ok((key, val))
    }

    fn parse_i32(s: &str) -> Result<i32, DehError> {
        s.parse::<i32>()
            .map_err(|_| DehError::BadField(format!("expected i32, got {s:?}")))
    }

    fn parse_u32(s: &str) -> Result<u32, DehError> {
        s.parse::<u32>()
            .map_err(|_| DehError::BadField(format!("expected u32, got {s:?}")))
    }

    fn parse_u8(s: &str) -> Result<u8, DehError> {
        s.parse::<u8>()
            .map_err(|_| DehError::BadField(format!("expected u8, got {s:?}")))
    }

    fn parse_usize(s: &str) -> Result<usize, DehError> {
        s.parse::<usize>()
            .map_err(|_| DehError::BadField(format!("expected usize, got {s:?}")))
    }

    // -----------------------------------------------------------------------
    // Apply
    // -----------------------------------------------------------------------

    /// Apply this patch to mutable copies of the game tables.
    ///
    /// # Errors
    /// Returns [`DehError::OutOfRange`] if any index exceeds the table length.
    ///
    /// # Notes
    /// - `MobjInfo::speed`, `radius`, and `height` are stored as
    ///   [`Fixed16_16`]; integer values from the patch are scaled by `65536`
    ///   (`FRACUNIT`) before assignment.
    /// - Frame fields `sprite_number` and `sprite_subnumber` are stored in
    ///   [`FramePatch`] but `MobjStateEntry` has no corresponding fields, so
    ///   they are ignored during apply.
    /// - Weapon patches are stored in [`WeaponPatch`] but the weapon table is
    ///   internal to `weapons.rs` and is not accessible here; those patches
    ///   must be applied by the caller if needed.
    pub fn apply(
        &self,
        mobjinfo: &mut Vec<MobjInfo>,
        states: &mut Vec<MobjStateEntry>,
    ) -> Result<(), DehError> {
        for patch in &self.things {
            let info = mobjinfo
                .get_mut(patch.thing_num)
                .ok_or_else(|| DehError::OutOfRange(format!("thing {}", patch.thing_num)))?;

            if let Some(hp) = patch.hit_points {
                info.spawn_health = hp;
            }
            if let Some(sp) = patch.speed {
                info.speed = Fixed16_16(sp << 16);
            }
            if let Some(r) = patch.radius {
                info.radius = Fixed16_16(r << 16);
            }
            if let Some(h) = patch.height {
                info.height = Fixed16_16(h << 16);
            }
            if let Some(m) = patch.mass {
                info.mass = m;
            }
            if let Some(b) = patch.bits {
                info.flags = b;
            }
            if let Some(pc) = patch.pain_chance {
                // pain_chance is u8; clamp silently rather than panic.
                info.pain_chance = pc.clamp(0, 255) as u8;
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
                state.tics = d as i16;
            }
            if let Some(nf) = patch.next_frame {
                state.next_state = StateNum(nf as u16);
            }
            // sprite_number / sprite_subnumber: no corresponding fields on
            // MobjStateEntry; stored only.
        }

        // Weapon patches: WeaponInfo is crate-private in weapons.rs and
        // cannot be mutated here.  Callers that need weapon patching must
        // apply WeaponPatch entries themselves against their own weapon table.

        Ok(())
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
        assert!(patch.texts.is_empty());
        assert!(patch.strings.is_empty());
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
        assert_eq!(t.thing_num, 0, "Thing 1 is 1-based → 0-based index 0");
        assert_eq!(t.hit_points, Some(200));
    }

    // -----------------------------------------------------------------------
    // 3. Frame duration
    // -----------------------------------------------------------------------

    #[test]
    fn deh_parse_frame_duration() {
        let patch = DehPatch::parse("Frame 5\nDuration = 10\n")
            .expect("valid Frame section must parse OK");
        assert_eq!(patch.frames.len(), 1);
        let f = &patch.frames[0];
        assert_eq!(f.frame_num, 5);
        assert_eq!(f.duration, Some(10));
    }

    // -----------------------------------------------------------------------
    // 4. Weapon ammo type
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
    // 5. Text section
    // -----------------------------------------------------------------------

    #[test]
    fn deh_parse_text_section() {
        // "Text 3 4" → read 3 bytes ("old") then 4 bytes ("neww")
        let patch = DehPatch::parse("Text 3 4\noldneww\n")
            .expect("valid Text section must parse OK");
        assert_eq!(patch.texts.len(), 1);
        // The raw bytes after the header line include a leading '\n' before "oldneww".
        // Our parser skips the header line then reads from `remaining`.
        // remaining after "Text 3 4\n" is "oldneww\n"
        // old_text = remaining[..3] = "old"
        // new_text = remaining[3..7] = "neww"
        assert_eq!(patch.texts[0].0, "old");
        assert_eq!(patch.texts[0].1, "neww");
    }

    // -----------------------------------------------------------------------
    // 6. [STRINGS] section
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
    // 7. Comment lines ignored
    // -----------------------------------------------------------------------

    #[test]
    fn deh_parse_ignores_comments() {
        let input = "# This is a comment\nThing 1\n# Another comment\nHit points = 50\n";
        let patch = DehPatch::parse(input).expect("commented input must parse OK");
        assert_eq!(patch.things.len(), 1);
        assert_eq!(patch.things[0].hit_points, Some(50));
    }

    // -----------------------------------------------------------------------
    // 8. Apply thing health to MOBJINFO clone
    // -----------------------------------------------------------------------

    #[test]
    fn deh_apply_thing_health() {
        let patch = DehPatch::parse("Thing 1\nHit points = 999\n")
            .expect("parse must succeed");

        let mut mobjinfo: Vec<MobjInfo> = MOBJINFO.to_vec();
        let mut states: Vec<MobjStateEntry> = STATES.to_vec();

        patch.apply(&mut mobjinfo, &mut states)
            .expect("apply must succeed");

        assert_eq!(
            mobjinfo[0].spawn_health, 999,
            "thing 1 (index 0) spawn_health must be updated to 999"
        );
    }

    // -----------------------------------------------------------------------
    // 9. Apply with out-of-range index returns Err
    // -----------------------------------------------------------------------

    #[test]
    fn deh_apply_out_of_range() {
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
}
