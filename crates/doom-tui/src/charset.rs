//! Character ramps for luminance-mapped terminal rendering.
//!
//! Each variant defines a sequence of characters ordered by increasing
//! visual density (opacity).  The renderer maps a pixel's luminance
//! (0 = black, 255 = white) to an index into the ramp.
//!
//! Inspired by [`bevy_ratatui_camera`'s `CameraStrategy`](
//! https://github.com/cxreiff/bevy_ratatui_camera).

/// A character ramp for luminance → character mapping.
///
/// Each variant stores a static slice of `char`s sorted by increasing opacity.
/// The widget picks the character whose index is `luma * (len-1) / 255`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharSet {
    /// `' ', '.', ':', '+', '=', '!', '*', '?', '#', '%', '&', '@'`
    Ascii,
    /// Braille dots: `' ', '⠂', '⠒', '⠖', '⠶', '⠷', '⠿', '⡿', '⣿'`
    Braille,
    /// Block shading: `' ', '░', '▒', '▓', '█'`
    Shading,
    /// Rising blocks: `' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'`
    Blocks,
}

impl CharSet {
    /// The character ramp for this set, ordered by increasing opacity.
    pub const fn chars(self) -> &'static [char] {
        match self {
            Self::Ascii => &[' ', '.', ':', '+', '=', '!', '*', '?', '#', '%', '&', '@'],
            Self::Braille => &[' ', '⠂', '⠒', '⠖', '⠶', '⠷', '⠿', '⡿', '⣿'],
            Self::Shading => &[' ', '░', '▒', '▓', '█'],
            Self::Blocks => &[' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'],
        }
    }

    /// Map a luminance value (0–255) to the appropriate character.
    #[inline]
    pub fn map_luma(self, luma: u8) -> char {
        let chars = self.chars();
        let idx = (luma as usize * (chars.len() - 1)) / 255;
        chars[idx]
    }

    /// All variants in cycle order (for F2 toggling).
    pub const ALL: &'static [CharSet] = &[
        Self::Ascii,
        Self::Braille,
        Self::Shading,
        Self::Blocks,
    ];
}

/// The top-level renderer mode, unifying graphics protocols and character strategies.
///
/// Used for `--renderer` CLI flag and F2 runtime cycling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererMode {
    /// Half-block `▀` with fg/bg colors (2 vertical pixels per cell).
    Halfblocks,
    /// Sixel graphics protocol (pixel-perfect).
    Sixel,
    /// Kitty graphics protocol (pixel-perfect).
    Kitty,
    /// iTerm2 inline image protocol (pixel-perfect).
    Iterm2,
    /// Luminance-mapped characters from the given character set.
    CharMap(CharSet),
}

impl RendererMode {
    /// All variants in cycle order for F2 toggling.
    /// Graphics protocols that require terminal support are included;
    /// the event loop silently falls back to halfblocks if unsupported.
    pub const ALL: &'static [RendererMode] = &[
        Self::Halfblocks,
        Self::Sixel,
        Self::Kitty,
        Self::Iterm2,
        Self::CharMap(CharSet::Ascii),
        Self::CharMap(CharSet::Braille),
        Self::CharMap(CharSet::Shading),
        Self::CharMap(CharSet::Blocks),
    ];

    /// Short display name for the status bar.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Halfblocks => "halfblocks",
            Self::Sixel => "sixel",
            Self::Kitty => "kitty",
            Self::Iterm2 => "iterm2",
            Self::CharMap(CharSet::Ascii) => "ascii",
            Self::CharMap(CharSet::Braille) => "braille",
            Self::CharMap(CharSet::Shading) => "shading",
            Self::CharMap(CharSet::Blocks) => "blocks",
        }
    }

    /// Parse from a CLI string.  Returns `None` for unknown values.
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "halfblocks" | "half" => Some(Self::Halfblocks),
            "sixel" => Some(Self::Sixel),
            "kitty" => Some(Self::Kitty),
            "iterm2" | "iterm" => Some(Self::Iterm2),
            "ascii" => Some(Self::CharMap(CharSet::Ascii)),
            "braille" => Some(Self::CharMap(CharSet::Braille)),
            "shading" => Some(Self::CharMap(CharSet::Shading)),
            "blocks" => Some(Self::CharMap(CharSet::Blocks)),
            _ => None,
        }
    }

    /// Advance to the next mode in the cycle.
    pub fn next(self) -> Self {
        let all = Self::ALL;
        let idx = all.iter().position(|&m| m == self).unwrap_or(0);
        all[(idx + 1) % all.len()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn charset_map_luma_endpoints() {
        for &cs in CharSet::ALL {
            let chars = cs.chars();
            assert_eq!(cs.map_luma(0), chars[0], "luma 0 should be first char");
            assert_eq!(
                cs.map_luma(255),
                chars[chars.len() - 1],
                "luma 255 should be last char"
            );
        }
    }

    #[test]
    fn charset_map_luma_midpoint_is_in_range() {
        for &cs in CharSet::ALL {
            let c = cs.map_luma(128);
            assert!(cs.chars().contains(&c));
        }
    }

    #[test]
    fn renderer_mode_roundtrip_names() {
        for &mode in RendererMode::ALL {
            let parsed = RendererMode::from_str_loose(mode.name());
            assert_eq!(parsed, Some(mode), "roundtrip failed for {:?}", mode);
        }
    }

    #[test]
    fn renderer_mode_next_cycles() {
        let mut mode = RendererMode::ALL[0];
        for _ in 0..RendererMode::ALL.len() {
            mode = mode.next();
        }
        // After a full cycle, back to start.
        assert_eq!(mode, RendererMode::ALL[0]);
    }

    #[test]
    fn renderer_mode_from_str_aliases() {
        assert_eq!(
            RendererMode::from_str_loose("half"),
            Some(RendererMode::Halfblocks)
        );
        assert_eq!(
            RendererMode::from_str_loose("iterm"),
            Some(RendererMode::Iterm2)
        );
        assert_eq!(RendererMode::from_str_loose("nope"), None);
    }
}
