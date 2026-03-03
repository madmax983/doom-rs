//! Render flags — per-thing rendering mode selectors.
//!
//! Each map thing can have a `RenderFlag` that tells the renderer how to
//! draw its sprite:
//!
//! - `Normal`      — standard textured sprite with light-level colormap.
//! - `Fuzz`        — partial invisibility shimmer (spectre, invisible player).
//! - `FullBright`  — ignore sector light; always render at full brightness.
//!
//! These flags are separate from the mobj flags (MF_SHADOW etc.) because
//! the renderer doesn't need to know about gameplay mechanics — it only
//! needs to know *how* to draw.

/// Rendering mode for a thing's sprite.
///
/// Determines which drawing path the renderer uses for this thing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RenderFlag {
    /// Standard sprite rendering: textured columns through the sector's
    /// light-level colormap.
    Normal,

    /// Partial invisibility (fuzz) effect: reads existing framebuffer
    /// pixels at offset positions and darkens them.  The sprite's actual
    /// texture is never drawn.
    ///
    /// Used for spectres (thing type 58) and players with the partial
    /// invisibility powerup.
    Fuzz,

    /// Full-brightness rendering: ignore sector light level and always
    /// use the identity colormap (row 0).
    ///
    /// Used for things that glow or emit light (e.g. fireballs, certain
    /// powerup items).
    FullBright,
}

impl Default for RenderFlag {
    fn default() -> Self {
        Self::Normal
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_flag_variants_are_distinct() {
        assert_ne!(RenderFlag::Normal, RenderFlag::Fuzz);
        assert_ne!(RenderFlag::Normal, RenderFlag::FullBright);
        assert_ne!(RenderFlag::Fuzz, RenderFlag::FullBright);
    }

    #[test]
    fn render_flag_default_is_normal() {
        assert_eq!(RenderFlag::default(), RenderFlag::Normal);
    }

    #[test]
    fn render_flag_is_copy() {
        let flag = RenderFlag::Fuzz;
        let copy = flag; // Copy
        assert_eq!(flag, copy);
    }

    #[test]
    fn render_flag_debug_output() {
        let dbg = format!("{:?}", RenderFlag::Fuzz);
        assert_eq!(dbg, "Fuzz");
    }

    #[test]
    fn render_flag_clone() {
        let flag = RenderFlag::FullBright;
        let cloned = flag.clone();
        assert_eq!(flag, cloned);
    }

    #[test]
    fn render_flag_hash_works() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(RenderFlag::Normal);
        set.insert(RenderFlag::Fuzz);
        set.insert(RenderFlag::FullBright);
        assert_eq!(set.len(), 3);

        // Duplicate insert should not increase size.
        set.insert(RenderFlag::Fuzz);
        assert_eq!(set.len(), 3);
    }
}
