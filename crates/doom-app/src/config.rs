//! Vanilla-compatible `default.cfg` configuration store.
//!
//! Ports the DOS-vanilla `m_misc.c` `defaults[]` table (see `M_LoadDefaults` /
//! `M_SaveDefaults`) as a `key value` text store. The format matches vanilla:
//! one entry per line, integers written as decimal, string values written
//! quoted (`key "value"`). The parser also accepts hexadecimal integers
//! (`0x...`), tolerates blank lines, and preserves unknown keys so a file
//! written by another Doom port round-trips faithfully.
//!
//! Round-trip fidelity: seeding starts from the full vanilla defaults (vanilla
//! persists the entire defaults set on every save), a loaded file overrides the
//! values of keys it contains in place, and any keys not in the vanilla table
//! are appended in file order. Serialization walks the entries in that stable
//! order (known keys first, in vanilla `defaults[]` order, then unknown keys).
//!
//! Only a subset of keys have a live engine binding today. Keys without one are
//! still loaded into the store and round-tripped verbatim for compatibility;
//! they are annotated with `// note:` below.

use std::collections::HashMap;
use std::path::Path;

/// A single configuration value: either a decimal/hex integer or a quoted
/// string. Ints re-serialize as decimal, strings re-serialize quoted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// Integer value (movement keys, volumes, screen size, ...).
    Int(i64),
    /// String value (chat macros, device names, ...).
    Str(String),
}

impl Value {
    /// Integer view, or `None` if this is a string value.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(n) => Some(*n),
            Value::Str(_) => None,
        }
    }

    /// String view, or `None` if this is an integer value.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s.as_str()),
            Value::Int(_) => None,
        }
    }

    /// Render this value the way vanilla writes it to `default.cfg`.
    fn serialize(&self) -> String {
        match self {
            Value::Int(n) => n.to_string(),
            Value::Str(s) => format!("\"{s}\""),
        }
    }
}

/// An ordered key/value configuration store compatible with vanilla
/// `default.cfg`. Insertion order is preserved for stable serialization.
#[derive(Debug, Clone)]
pub struct Config {
    /// Entries in stable serialization order (vanilla `defaults[]` order for
    /// known keys, then unknown keys in the order they were first seen).
    entries: Vec<(String, Value)>,
    /// key -> index into `entries` for O(1) lookup/update.
    index: HashMap<String, usize>,
}

impl Config {
    /// An empty store (no defaults). Prefer [`Config::with_defaults`].
    pub fn new() -> Self {
        Config {
            entries: Vec::new(),
            index: HashMap::new(),
        }
    }

    /// A store seeded with the full vanilla `defaults[]` table, in vanilla
    /// order. This mirrors vanilla, which persists every default on save.
    pub fn with_defaults() -> Self {
        let mut cfg = Config::new();
        for (key, value) in default_entries() {
            cfg.set(key, value);
        }
        cfg
    }

    /// Number of entries currently held.
    #[allow(dead_code)] // part of the config API; exercised by tests
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the store holds no entries.
    #[allow(dead_code)] // part of the config API; exercised by tests
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.index.get(key).map(|&i| &self.entries[i].1)
    }

    /// Integer view of `key`, if present and integer-typed.
    pub fn get_int(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(Value::as_int)
    }

    /// String view of `key`, if present and string-typed.
    #[allow(dead_code)] // part of the config API; exercised by tests
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }

    /// Insert or update `key`. Existing keys keep their position (and thus
    /// their serialization order); new keys are appended.
    pub fn set(&mut self, key: impl Into<String>, value: Value) {
        let key = key.into();
        if let Some(&i) = self.index.get(&key) {
            self.entries[i].1 = value;
        } else {
            let i = self.entries.len();
            self.entries.push((key.clone(), value));
            self.index.insert(key, i);
        }
    }

    /// Iterate entries in stable serialization order.
    #[allow(dead_code)] // part of the config API; exercised by tests
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Parse a `default.cfg` body on top of the vanilla defaults. Keys present
    /// in `text` override the defaults in place; unknown keys are appended in
    /// file order. Blank lines are ignored. Malformed lines are skipped.
    pub fn parse_with_defaults(text: &str) -> Self {
        let mut cfg = Config::with_defaults();
        cfg.apply_text(text);
        cfg
    }

    /// Parse a `default.cfg` body with NO defaults seeded — only the keys the
    /// text actually contains. Useful for exact round-trip assertions.
    #[allow(dead_code)] // part of the config API; exercised by tests
    pub fn parse_raw(text: &str) -> Self {
        let mut cfg = Config::new();
        cfg.apply_text(text);
        cfg
    }

    /// Apply the key/value lines in `text` to this store (updating known keys
    /// in place, appending unknown keys).
    fn apply_text(&mut self, text: &str) {
        for line in text.lines() {
            if let Some((key, value)) = parse_line(line) {
                self.set(key, value);
            }
        }
    }

    /// Serialize the store to a `default.cfg` body (trailing newline per line).
    pub fn serialize(&self) -> String {
        let mut out = String::new();
        for (key, value) in &self.entries {
            out.push_str(key);
            out.push(' ');
            out.push_str(&value.serialize());
            out.push('\n');
        }
        out
    }

    /// Load a config from `path`, seeded with vanilla defaults. If the file is
    /// absent, returns the pure defaults. I/O errors other than "not found"
    /// propagate.
    pub fn load_or_defaults(path: &Path) -> std::io::Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => Ok(Config::parse_with_defaults(&text)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::with_defaults()),
            Err(e) => Err(e),
        }
    }

    /// Write the store to `path` in vanilla `default.cfg` format.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        std::fs::write(path, self.serialize())
    }
}

impl Default for Config {
    fn default() -> Self {
        Config::with_defaults()
    }
}

/// Parse a single `key value` line. Returns `None` for blank lines and lines
/// without a value token. Accepts quoted strings (`"..."`), hexadecimal
/// integers (`0x...`), and decimal integers; a non-numeric bareword falls back
/// to a string value so no data is lost.
fn parse_line(line: &str) -> Option<(String, Value)> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Split off the first whitespace-delimited token as the key.
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let key = parts.next()?.to_string();
    let raw = parts.next()?.trim();
    if raw.is_empty() {
        return None;
    }
    let value = parse_value(raw);
    Some((key, value))
}

/// Parse the value token of a config line.
fn parse_value(raw: &str) -> Value {
    if let Some(inner) = raw.strip_prefix('"') {
        // Quoted string: take everything up to the closing quote (vanilla does
        // not support escapes inside chat macros).
        let s = match inner.rfind('"') {
            Some(end) => &inner[..end],
            None => inner,
        };
        return Value::Str(s.to_string());
    }
    if let Some(hex) = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) {
        if let Ok(n) = i64::from_str_radix(hex, 16) {
            return Value::Int(n);
        }
    }
    if let Ok(n) = raw.parse::<i64>() {
        return Value::Int(n);
    }
    // Non-numeric bareword: preserve it as a string so unknown keys survive.
    Value::Str(raw.to_string())
}

// ---------------------------------------------------------------------------
// Vanilla defaults[] table (DOS `m_misc.c`).
// ---------------------------------------------------------------------------

// Vanilla DOS key scancode constants (doomdef.h). These are the raw values
// vanilla stores in default.cfg for the movement/action bindings.
const KEY_RIGHTARROW: i64 = 0xae; // 174
const KEY_LEFTARROW: i64 = 0xac; // 172
const KEY_UPARROW: i64 = 0xad; // 173
const KEY_DOWNARROW: i64 = 0xaf; // 175
const KEY_RCTRL: i64 = 0x80 + 0x1d; // 157
const KEY_RSHIFT: i64 = 0x80 + 0x36; // 182
const KEY_RALT: i64 = 0x80 + 0x38; // 184

/// The vanilla DOS `defaults[]` table, in file order, with exact default
/// values. `Value::Int` for numeric bindings, `Value::Str` for chat macros.
fn default_entries() -> Vec<(&'static str, Value)> {
    use Value::{Int, Str};
    vec![
        ("mouse_sensitivity", Int(5)),
        ("sfx_volume", Int(8)),
        ("music_volume", Int(8)),
        ("show_messages", Int(1)),
        ("key_right", Int(KEY_RIGHTARROW)),
        ("key_left", Int(KEY_LEFTARROW)),
        ("key_up", Int(KEY_UPARROW)),
        ("key_down", Int(KEY_DOWNARROW)),
        ("key_strafeleft", Int(i64::from(b','))), // 44
        ("key_straferight", Int(i64::from(b'.'))), // 46
        ("key_fire", Int(KEY_RCTRL)),
        ("key_use", Int(i64::from(b' '))), // 32
        ("key_strafe", Int(KEY_RALT)),
        ("key_speed", Int(KEY_RSHIFT)),
        ("use_mouse", Int(1)),
        ("mouseb_fire", Int(0)),
        ("mouseb_strafe", Int(1)),
        ("mouseb_forward", Int(2)),
        ("use_joystick", Int(0)),
        ("joyb_fire", Int(0)),
        ("joyb_strafe", Int(1)),
        ("joyb_use", Int(3)),
        ("joyb_speed", Int(2)),
        ("screenblocks", Int(9)),
        ("detaillevel", Int(0)),
        ("snd_channels", Int(3)),
        // note: DOS sound-device selectors. Persisted for default.cfg
        // compatibility; not wired to the doom-rs audio backend.
        ("snd_musicdevice", Int(0)),
        ("snd_sfxdevice", Int(0)),
        ("snd_sbport", Int(0)),
        ("snd_sbirq", Int(0)),
        ("snd_sbdma", Int(0)),
        ("snd_mport", Int(0)),
        ("usegamma", Int(0)),
        // note: chat macros are persisted for compatibility; multiplayer chat
        // macro playback is not yet wired.
        ("chatmacro0", Str("No".to_string())),
        ("chatmacro1", Str("I'm ready to kick butt!".to_string())),
        ("chatmacro2", Str("I'm OK.".to_string())),
        ("chatmacro3", Str("I'm not looking too good!".to_string())),
        ("chatmacro4", Str("Help!".to_string())),
        ("chatmacro5", Str("You suck!".to_string())),
        ("chatmacro6", Str("Next time, scumbag...".to_string())),
        ("chatmacro7", Str("Come here!".to_string())),
        ("chatmacro8", Str("I'll take care of it.".to_string())),
        ("chatmacro9", Str("Yes".to_string())),
    ]
}

// ---------------------------------------------------------------------------
// Precedence resolution: config value vs. explicitly-provided CLI value.
// ---------------------------------------------------------------------------

/// Resolve the effective integer value for a setting that can come from either
/// the config file or a CLI flag.
///
/// Precedence (matching the task contract): the config value is the baseline;
/// an **explicitly** provided CLI value wins over it; a CLI flag left at its
/// clap default does NOT clobber the config value. When neither source
/// provides a value, `fallback` is used.
///
/// `cli_explicit` should be derived from clap's
/// [`clap::ArgMatches::value_source`] (== `Some(ValueSource::CommandLine)`),
/// so a default-valued flag is distinguishable from one the user actually set.
pub fn resolve_int(
    config_value: Option<i64>,
    cli_value: i64,
    cli_explicit: bool,
    fallback: i64,
) -> i64 {
    if cli_explicit {
        cli_value
    } else if let Some(v) = config_value {
        v
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_have_expected_vanilla_values() {
        let cfg = Config::with_defaults();
        assert_eq!(cfg.get_int("mouse_sensitivity"), Some(5));
        assert_eq!(cfg.get_int("sfx_volume"), Some(8));
        assert_eq!(cfg.get_int("music_volume"), Some(8));
        assert_eq!(cfg.get_int("show_messages"), Some(1));
        assert_eq!(cfg.get_int("screenblocks"), Some(9));
        assert_eq!(cfg.get_int("detaillevel"), Some(0));
        assert_eq!(cfg.get_int("snd_channels"), Some(3));
        assert_eq!(cfg.get_int("usegamma"), Some(0));
        // Movement/action key bindings (vanilla DOS scancodes).
        assert_eq!(cfg.get_int("key_right"), Some(174));
        assert_eq!(cfg.get_int("key_left"), Some(172));
        assert_eq!(cfg.get_int("key_up"), Some(173));
        assert_eq!(cfg.get_int("key_down"), Some(175));
        assert_eq!(cfg.get_int("key_strafeleft"), Some(44));
        assert_eq!(cfg.get_int("key_straferight"), Some(46));
        assert_eq!(cfg.get_int("key_fire"), Some(157));
        assert_eq!(cfg.get_int("key_use"), Some(32));
        assert_eq!(cfg.get_int("key_strafe"), Some(184));
        assert_eq!(cfg.get_int("key_speed"), Some(182));
        // Mouse / joystick buttons.
        assert_eq!(cfg.get_int("mouseb_fire"), Some(0));
        assert_eq!(cfg.get_int("mouseb_strafe"), Some(1));
        assert_eq!(cfg.get_int("mouseb_forward"), Some(2));
        assert_eq!(cfg.get_int("joyb_fire"), Some(0));
        assert_eq!(cfg.get_int("joyb_strafe"), Some(1));
        assert_eq!(cfg.get_int("joyb_use"), Some(3));
        assert_eq!(cfg.get_int("joyb_speed"), Some(2));
        // Chat macros (string-typed).
        assert_eq!(cfg.get_str("chatmacro0"), Some("No"));
        assert_eq!(cfg.get_str("chatmacro1"), Some("I'm ready to kick butt!"));
        assert_eq!(cfg.get_str("chatmacro9"), Some("Yes"));
    }

    #[test]
    fn default_generation_writes_expected_keys() {
        let cfg = Config::with_defaults();
        let text = cfg.serialize();
        // Spot-check the serialized form: ints bare, strings quoted.
        assert!(text.contains("mouse_sensitivity 5\n"));
        assert!(text.contains("screenblocks 9\n"));
        assert!(text.contains("key_right 174\n"));
        assert!(text.contains("chatmacro0 \"No\"\n"));
        assert!(text.contains("chatmacro1 \"I'm ready to kick butt!\"\n"));
        // Full vanilla key set present (43 entries).
        assert_eq!(cfg.len(), default_entries().len());
    }

    #[test]
    fn roundtrip_preserves_known_unknown_quoted_and_ints() {
        // A sample vanilla-ish default.cfg with known keys (overriding
        // defaults), quoted strings with spaces, a hex int, blank lines, and
        // two unknown keys from another port.
        let sample = "\
mouse_sensitivity 7

sfx_volume 11
screenblocks 6
key_fire 0x9d
chatmacro0 \"Frag them all\"
show_messages 0

vanilla_extra_key 42
snd_pitchshift 1
";
        let cfg = Config::parse_with_defaults(sample);

        // Known keys took the file's values.
        assert_eq!(cfg.get_int("mouse_sensitivity"), Some(7));
        assert_eq!(cfg.get_int("sfx_volume"), Some(11));
        assert_eq!(cfg.get_int("screenblocks"), Some(6));
        assert_eq!(cfg.get_int("key_fire"), Some(157)); // 0x9d parsed
        assert_eq!(cfg.get_str("chatmacro0"), Some("Frag them all"));
        assert_eq!(cfg.get_int("show_messages"), Some(0));
        // Unknown keys preserved.
        assert_eq!(cfg.get_int("vanilla_extra_key"), Some(42));
        assert_eq!(cfg.get_int("snd_pitchshift"), Some(1));

        // Serialize -> reparse (raw) -> every original key/value preserved.
        let text = cfg.serialize();
        let reparsed = Config::parse_raw(&text);
        for (k, v) in cfg.iter() {
            assert_eq!(reparsed.get(k), Some(v), "key {k} lost on round-trip");
        }
        // Ordering: known keys come before appended unknown keys.
        let keys: Vec<&str> = cfg.iter().map(|(k, _)| k).collect();
        let idx_known = keys.iter().position(|&k| k == "usegamma").unwrap();
        let idx_unknown = keys.iter().position(|&k| k == "vanilla_extra_key").unwrap();
        assert!(idx_known < idx_unknown, "unknown keys must be appended");
    }

    #[test]
    fn parser_tolerates_blank_lines_and_spaces_in_strings() {
        let sample = "\n\n   \nchatmacro5 \"You really suck!\"\n\n";
        let cfg = Config::parse_raw(sample);
        assert_eq!(cfg.len(), 1);
        assert_eq!(cfg.get_str("chatmacro5"), Some("You really suck!"));
    }

    #[test]
    fn precedence_config_baseline_cli_override_default_noclobber() {
        // Config sets screenblocks = 6.
        let cfg = Config::parse_with_defaults("screenblocks 6\n");
        let config_value = cfg.get_int("screenblocks");
        assert_eq!(config_value, Some(6));

        // No CLI override (flag at its clap default 9): config value wins.
        let effective = resolve_int(config_value, 9, /*cli_explicit=*/ false, 9);
        assert_eq!(effective, 6, "default-valued CLI flag must not clobber config");

        // Explicit CLI override: CLI value wins.
        let effective = resolve_int(config_value, 4, /*cli_explicit=*/ true, 9);
        assert_eq!(effective, 4, "explicit CLI value must win over config");

        // No config entry and no explicit CLI: fallback is used.
        let effective = resolve_int(None, 9, /*cli_explicit=*/ false, 9);
        assert_eq!(effective, 9);
    }
}
