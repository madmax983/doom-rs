//! Intermission screen renderer — the level-completion tally screen.
//!
//! Displays kill/item/secret percentages and par time comparison after a
//! level exit, with animated counting and phase transitions matching the
//! classic Doom intermission flow.

use crate::framebuffer::Framebuffer;
use crate::statusbar::{DIGIT_W, LETTER_W, draw_digit, draw_text};
use doom_game::IntermissionStats;

// ---------------------------------------------------------------------------
// Color palette indices
// ---------------------------------------------------------------------------

/// Background color for the intermission screen (black).
const COLOR_BG: u8 = 0;
/// Default text/number color (light gray).
const COLOR_DEFAULT: u8 = 4;
/// Title/header color (red).
const COLOR_TITLE: u8 = 176;
/// Percentage color (yellow).
const COLOR_PERCENT: u8 = 231;
/// Under-par time color (green).
const COLOR_UNDER_PAR: u8 = 112;
/// Over-par time color (red).
const COLOR_OVER_PAR: u8 = 176;
/// "Press any key" prompt color (medium gray).
const COLOR_PROMPT: u8 = 96;

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

/// Y position for the "FINISHED" header.
const HEADER_Y: usize = 2;
/// Y position for the episode/map title.
const MAP_TITLE_Y: usize = 25;
/// Y position for the "KILLS" line.
const KILLS_Y: usize = 50;
/// Y position for the "ITEMS" line.
const ITEMS_Y: usize = 70;
/// Y position for the "SECRETS" line.
const SECRETS_Y: usize = 90;
/// Y position for the "TIME" line.
const TIME_Y: usize = 120;
/// Y position for the "PAR" line.
const PAR_Y: usize = 140;
/// Y position for the "PRESS ANY KEY" prompt.
const PROMPT_Y: usize = 180;
/// X indent for labels.
const LABEL_X: usize = 50;
/// X position for value display (right side of the label area).
const VALUE_X: usize = 200;

/// How many tics to show the time phase before advancing to Done.
const TIME_DISPLAY_TICS: u32 = 35;
/// Increment per tic for percentage counters.
const COUNT_SPEED: u8 = 2;

// ---------------------------------------------------------------------------
// IntermissionPhase
// ---------------------------------------------------------------------------

/// Current animation phase of the intermission screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntermissionPhase {
    /// Animating kill percentage counter.
    CountingKills,
    /// Animating item percentage counter.
    CountingItems,
    /// Animating secret percentage counter.
    CountingSecrets,
    /// Showing par/level time comparison.
    ShowingTime,
    /// All animations complete, waiting for keypress.
    Done,
}

// ---------------------------------------------------------------------------
// IntermissionRenderer
// ---------------------------------------------------------------------------

/// State machine for the intermission screen animation.
#[derive(Clone, Debug)]
pub struct IntermissionRenderer {
    /// Current animation tic counter.
    pub tic: u32,
    /// Current animation phase.
    pub phase: IntermissionPhase,
    /// Displayed kill percentage (animated counter).
    pub shown_kills: u8,
    /// Displayed items percentage.
    pub shown_items: u8,
    /// Displayed secrets percentage.
    pub shown_secrets: u8,
    /// Target kill percentage (from IntermissionStats).
    pub target_kills: u8,
    /// Target items percentage.
    pub target_items: u8,
    /// Target secrets percentage.
    pub target_secrets: u8,
    /// Par time in seconds.
    pub par_time: u32,
    /// Level time in seconds.
    pub level_time: u32,
    /// Episode number (1-3 for Doom 1, 0 for Doom 2).
    pub episode: u8,
    /// Map number.
    pub map: u8,
    /// Internal tic counter for the ShowingTime phase.
    time_phase_tics: u32,
}

/// Compute a percentage from a numerator and denominator, capped at 100.
///
/// Returns 100 when the denominator is 0 (Doom convention: 0/0 = 100%).
fn percent(num: u32, denom: u32) -> u8 {
    if denom == 0 {
        return 100;
    }
    let p = (num * 100) / denom;
    p.min(100) as u8
}

/// Convert tics (35/sec) to whole seconds.
fn tics_to_secs(tics: u32) -> u32 {
    tics / 35
}

/// Parse a level name like "E1M3" or "MAP07" into (episode, map).
///
/// Returns `(episode, map)` where episode > 0 for Doom 1 and episode == 0
/// for Doom 2 format. Falls back to (1, 1) for unrecognized formats.
fn parse_level_name(name: &str) -> (u8, u8) {
    let name = name.trim().to_uppercase();

    // Doom 1: ExMy
    if name.len() == 4 && name.as_bytes()[0] == b'E' && name.as_bytes()[2] == b'M' {
        let ep = (name.as_bytes()[1] as char).to_digit(10).unwrap_or(1) as u8;
        let map = (name.as_bytes()[3] as char).to_digit(10).unwrap_or(1) as u8;
        return (ep, map);
    }

    // Doom 2: MAPxx
    if name.len() == 5 && name.starts_with("MAP") {
        if let Ok(num) = name[3..].parse::<u8>() {
            return (0, num);
        }
    }

    (1, 1) // fallback
}

impl IntermissionRenderer {
    /// Create a new `IntermissionRenderer` from end-of-level stats and a
    /// level name string (e.g., `"E1M3"` or `"MAP07"`).
    ///
    /// Computes percentages from raw counts and converts tics to seconds.
    /// The renderer starts in the `CountingKills` phase with all shown
    /// counters at zero.
    pub fn new(stats: &IntermissionStats, level_name: &str) -> Self {
        let (episode, map) = parse_level_name(level_name);
        Self {
            tic: 0,
            phase: IntermissionPhase::CountingKills,
            shown_kills: 0,
            shown_items: 0,
            shown_secrets: 0,
            target_kills: percent(stats.kills, stats.total_kills),
            target_items: percent(stats.items, stats.total_items),
            target_secrets: percent(stats.secrets, stats.total_secrets),
            par_time: tics_to_secs(stats.par_time_tics),
            level_time: tics_to_secs(stats.time_tics),
            episode,
            map,
            time_phase_tics: 0,
        }
    }

    /// Create an `IntermissionRenderer` directly from pre-computed values.
    ///
    /// Useful for tests and when stats are already in percentage/seconds form.
    pub fn from_values(
        target_kills: u8,
        target_items: u8,
        target_secrets: u8,
        par_time: u32,
        level_time: u32,
        episode: u8,
        map: u8,
    ) -> Self {
        Self {
            tic: 0,
            phase: IntermissionPhase::CountingKills,
            shown_kills: 0,
            shown_items: 0,
            shown_secrets: 0,
            target_kills,
            target_items,
            target_secrets,
            par_time,
            level_time,
            episode,
            map,
            time_phase_tics: 0,
        }
    }

    /// Advance the intermission animation by one game tic (1/35 sec).
    ///
    /// During counting phases the displayed percentage increments by
    /// [`COUNT_SPEED`] per tic until it reaches the target, then the phase
    /// advances. After all counting phases, the time display is held for
    /// [`TIME_DISPLAY_TICS`] tics before transitioning to `Done`.
    pub fn tick(&mut self) {
        self.tic += 1;

        match self.phase {
            IntermissionPhase::CountingKills => {
                if self.shown_kills < self.target_kills {
                    let next = self.shown_kills.saturating_add(COUNT_SPEED);
                    self.shown_kills = next.min(self.target_kills);
                }
                if self.shown_kills >= self.target_kills {
                    self.shown_kills = self.target_kills;
                    self.phase = IntermissionPhase::CountingItems;
                }
            }
            IntermissionPhase::CountingItems => {
                if self.shown_items < self.target_items {
                    let next = self.shown_items.saturating_add(COUNT_SPEED);
                    self.shown_items = next.min(self.target_items);
                }
                if self.shown_items >= self.target_items {
                    self.shown_items = self.target_items;
                    self.phase = IntermissionPhase::CountingSecrets;
                }
            }
            IntermissionPhase::CountingSecrets => {
                if self.shown_secrets < self.target_secrets {
                    let next = self.shown_secrets.saturating_add(COUNT_SPEED);
                    self.shown_secrets = next.min(self.target_secrets);
                }
                if self.shown_secrets >= self.target_secrets {
                    self.shown_secrets = self.target_secrets;
                    self.phase = IntermissionPhase::ShowingTime;
                    self.time_phase_tics = 0;
                }
            }
            IntermissionPhase::ShowingTime => {
                self.time_phase_tics += 1;
                if self.time_phase_tics >= TIME_DISPLAY_TICS {
                    self.phase = IntermissionPhase::Done;
                }
            }
            IntermissionPhase::Done => {
                // Nothing to animate — waiting for player input.
            }
        }
    }

    /// Returns `true` when all animations are complete and the screen is
    /// waiting for a keypress.
    pub fn is_done(&self) -> bool {
        self.phase == IntermissionPhase::Done
    }

    /// Skip all remaining animations, snapping every counter to its target
    /// value and jumping to the `Done` phase.
    pub fn skip(&mut self) {
        self.shown_kills = self.target_kills;
        self.shown_items = self.target_items;
        self.shown_secrets = self.target_secrets;
        self.phase = IntermissionPhase::Done;
    }
}

// ---------------------------------------------------------------------------
// Helper drawing functions
// ---------------------------------------------------------------------------

/// Draw a string at `(x, y)` using the 5x7 letter bitmaps.
///
/// Delegates to [`crate::statusbar::draw_text`] but accepts `&str` for
/// convenience.
pub fn draw_intermission_text(fb: &mut Framebuffer, x: usize, y: usize, text: &str, color: u8) {
    draw_text(fb, x as i32, y as i32, text.as_bytes(), color);
}

/// Draw a percentage value (0-100) followed by a `%` sign using the large
/// 7x9 digit font.
///
/// The digits are drawn left-to-right at `(x, y)`, followed by a `%` glyph
/// rendered via [`draw_char`].
pub fn draw_percentage(fb: &mut Framebuffer, x: usize, y: usize, value: u8, color: u8) {
    let val = value as u32;
    let ix = x as i32;
    let iy = y as i32;
    let stride = DIGIT_W + 1; // 8 px per digit position

    // Determine how many digits to draw (at least 1).
    let digits = if val >= 100 {
        3
    } else if val >= 10 {
        2
    } else {
        1
    };

    let mut remaining = val;
    // Draw digits from most-significant to least-significant.
    let mut pos = 0i32;
    if digits >= 3 {
        let d = (remaining / 100) as u8;
        draw_digit(fb, ix + pos * stride, iy, d, color);
        remaining %= 100;
        pos += 1;
    }
    if digits >= 2 {
        let d = (remaining / 10) as u8;
        draw_digit(fb, ix + pos * stride, iy, d, color);
        remaining %= 10;
        pos += 1;
    }
    {
        let d = remaining as u8;
        draw_digit(fb, ix + pos * stride, iy, d, color);
        pos += 1;
    }

    // '%' glyph: draw as a pair of dots separated by a slash.
    let pct_x = ix + pos * stride;
    draw_percent_sign(fb, pct_x, iy, color);
}

/// Draw a simple `%` glyph at `(x, y)` using a 7x9 bitmap.
fn draw_percent_sign(fb: &mut Framebuffer, x: i32, y: i32, color: u8) {
    // Top-left dot (2x2).
    set_px(fb, x, y + 1, color);
    set_px(fb, x + 1, y + 1, color);
    set_px(fb, x, y + 2, color);
    set_px(fb, x + 1, y + 2, color);
    // Diagonal slash from bottom-left to top-right.
    for i in 0..7 {
        set_px(fb, x + 6 - i, y + 1 + i, color);
    }
    // Bottom-right dot (2x2).
    set_px(fb, x + 5, y + 6, color);
    set_px(fb, x + 6, y + 6, color);
    set_px(fb, x + 5, y + 7, color);
    set_px(fb, x + 6, y + 7, color);
}

/// Bounds-checked pixel setter.
#[inline]
fn set_px(fb: &mut Framebuffer, x: i32, y: i32, color: u8) {
    if x >= 0 && y >= 0 {
        fb.set_pixel(x as usize, y as usize, color);
    }
}

/// Draw a time value in `MM:SS` format using the large 7x9 digit font.
///
/// `seconds` is the total elapsed time; values >= 6000 (100 minutes) are
/// clamped. The colon is rendered as two dots.
pub fn draw_time(fb: &mut Framebuffer, x: usize, y: usize, seconds: u32, color: u8) {
    let clamped = seconds.min(5999); // cap at 99:59
    let mins = clamped / 60;
    let secs = clamped % 60;

    let ix = x as i32;
    let iy = y as i32;
    let stride = DIGIT_W + 1; // 8 px

    // Minutes: two digits.
    draw_digit(fb, ix, iy, (mins / 10) as u8, color);
    draw_digit(fb, ix + stride, iy, (mins % 10) as u8, color);

    // Colon: two vertically spaced dots.
    let colon_x = ix + stride * 2 + 2;
    set_px(fb, colon_x, iy + 2, color);
    set_px(fb, colon_x + 1, iy + 2, color);
    set_px(fb, colon_x, iy + 6, color);
    set_px(fb, colon_x + 1, iy + 6, color);

    // Seconds: two digits.
    let sec_x = ix + stride * 2 + 6;
    draw_digit(fb, sec_x, iy, (secs / 10) as u8, color);
    draw_digit(fb, sec_x + stride, iy, (secs % 10) as u8, color);
}

/// Format a map name string from episode and map numbers.
///
/// - Episode > 0 (Doom 1): returns `"E{ep}M{map}"` (e.g., `"E1M1"`).
/// - Episode == 0 (Doom 2): returns `"MAP{map:02}"` (e.g., `"MAP01"`).
pub fn format_map_name(episode: u8, map: u8) -> String {
    if episode > 0 {
        format!("E{}M{}", episode, map)
    } else {
        format!("MAP{:02}", map)
    }
}

// ---------------------------------------------------------------------------
// Main draw function
// ---------------------------------------------------------------------------

/// Render the full intermission screen into the framebuffer.
///
/// # Layout
///
/// ```text
/// ┌──────────────────────────────────┐
/// │         FINISHED                 │  y=2
/// │                                  │
/// │     EPISODE X - MAP Y            │  y=25
/// │                                  │
/// │     KILLS      XXX%              │  y=50
/// │     ITEMS      XXX%              │  y=70
/// │     SECRETS    XXX%              │  y=90
/// │                                  │
/// │     TIME   MM:SS                 │  y=120
/// │     PAR    MM:SS                 │  y=140
/// │                                  │
/// │     PRESS ANY KEY TO CONTINUE    │  y=180
/// └──────────────────────────────────┘
/// ```
pub fn draw_intermission(fb: &mut Framebuffer, renderer: &IntermissionRenderer) {
    // --- Background ---
    fb.clear(COLOR_BG);

    // --- "FINISHED" header ---
    let header_text = b"FINISHED";
    let header_width = header_text.len() as i32 * (LETTER_W + 1);
    let header_x = (Framebuffer::width() as i32 - header_width) / 2;
    draw_text(fb, header_x, HEADER_Y as i32, header_text, COLOR_TITLE);

    // --- Map name ---
    let map_name = format_map_name(renderer.episode, renderer.map);
    let map_text = map_name.as_bytes();
    let map_width = map_text.len() as i32 * (LETTER_W + 1);
    let map_x = (Framebuffer::width() as i32 - map_width) / 2;
    draw_text(fb, map_x, MAP_TITLE_Y as i32, map_text, COLOR_DEFAULT);

    // --- Kill / Item / Secret labels and percentages ---
    draw_text(fb, LABEL_X as i32, KILLS_Y as i32, b"KILLS", COLOR_DEFAULT);
    draw_percentage(fb, VALUE_X, KILLS_Y, renderer.shown_kills, COLOR_PERCENT);

    draw_text(fb, LABEL_X as i32, ITEMS_Y as i32, b"ITEMS", COLOR_DEFAULT);
    draw_percentage(fb, VALUE_X, ITEMS_Y, renderer.shown_items, COLOR_PERCENT);

    draw_text(
        fb,
        LABEL_X as i32,
        SECRETS_Y as i32,
        b"SECRETS",
        COLOR_DEFAULT,
    );
    draw_percentage(
        fb,
        VALUE_X,
        SECRETS_Y,
        renderer.shown_secrets,
        COLOR_PERCENT,
    );

    // --- Time / Par ---
    let time_color = if renderer.level_time <= renderer.par_time {
        COLOR_UNDER_PAR
    } else {
        COLOR_OVER_PAR
    };

    draw_text(fb, LABEL_X as i32, TIME_Y as i32, b"TIME", COLOR_DEFAULT);
    draw_time(fb, VALUE_X, TIME_Y, renderer.level_time, time_color);

    draw_text(fb, LABEL_X as i32, PAR_Y as i32, b"PAR", COLOR_DEFAULT);
    draw_time(fb, VALUE_X, PAR_Y, renderer.par_time, COLOR_DEFAULT);

    // --- Prompt ---
    if renderer.is_done() {
        let prompt = b"PRESS ANY KEY TO CONTINUE";
        let prompt_width = prompt.len() as i32 * (LETTER_W + 1);
        let prompt_x = (Framebuffer::width() as i32 - prompt_width) / 2;
        draw_text(fb, prompt_x, PROMPT_Y as i32, prompt, COLOR_PROMPT);
    }
}

// ===========================================================================
// WAD patch-based intermission renderer
// ===========================================================================

use crate::patch_cache::PatchCache;
use doom_wad::WadStack;

/// Draw `value` using WINUM digit patches, left-to-right starting at `x`.
/// Returns the x position after the last digit drawn.
fn wi_draw_number(
    fb: &mut Framebuffer,
    cache: &mut PatchCache,
    wad: &WadStack,
    mut x: i32,
    y: i32,
    value: u32,
) -> i32 {
    let s = value.to_string();
    for ch in s.bytes() {
        let digit = ch - b'0';
        let name = format!("WINUM{digit}");
        if let Some(p) = cache.get(&name, wad) {
            let p = p.clone();
            fb.draw_patch_vanilla(x, y, &p);
            x += p.width as i32;
        }
    }
    x
}

/// Draw a percentage using WINUM digits + WIPCNT, starting at `x`.
fn wi_draw_percent(
    fb: &mut Framebuffer,
    cache: &mut PatchCache,
    wad: &WadStack,
    x: i32,
    y: i32,
    value: u8,
) {
    let x2 = wi_draw_number(fb, cache, wad, x, y, value as u32);
    if let Some(p) = cache.get("WIPCNT", wad) {
        let p = p.clone();
        fb.draw_patch_vanilla(x2, y, &p);
    }
}

/// Draw a time in MM:SS format using WINUM digits + WICOLON.
fn wi_draw_time(
    fb: &mut Framebuffer,
    cache: &mut PatchCache,
    wad: &WadStack,
    x: i32,
    y: i32,
    secs: u32,
) {
    let minutes = (secs / 60).min(99);
    let seconds = secs % 60;
    let mut cx = x;
    cx = wi_draw_number(fb, cache, wad, cx, y, minutes);
    if let Some(p) = cache.get("WICOLON", wad) {
        let p = p.clone();
        fb.draw_patch_vanilla(cx, y, &p);
        cx += p.width as i32;
    }
    // Always draw two digits for seconds.
    if seconds < 10 {
        let name = "WINUM0";
        if let Some(p) = cache.get(name, wad) {
            let p = p.clone();
            fb.draw_patch_vanilla(cx, y, &p);
            cx += p.width as i32;
        }
    }
    wi_draw_number(fb, cache, wad, cx, y, seconds);
}

/// Draw the intermission screen using WAD patches — matching vanilla Doom's
/// single-player intermission layout from `wi_stuff.c`.
///
/// Vanilla pixel positions (from wi_stuff.c / wi_stuff.h):
/// - Background:    WIMAP{ep} or INTERPIC, centered
/// - "Finished":    WIF  at (84, 16)
/// - Level leaving: WILV{ep}{map} at (160, 16)
/// - "Entering":    WIENTER at (84, 84)
/// - Level entering: WILV patch at (160, 84)   (when Done)
/// - Kills label:   WIOSTK at (50, 114),  value at (200, 114)
/// - Items label:   WIOSTI at (50, 134),  value at (200, 134)
/// - Secrets label: WISCRT2 at (50, 154), value at (200, 154)
/// - Time label:    WITIME at (16, 180),  value at (96, 180)
/// - Par label:     WIPAR  at (232, 180), value at (296, 180)
pub fn draw_intermission_wad(
    fb: &mut Framebuffer,
    cache: &mut PatchCache,
    wad: &WadStack,
    renderer: &IntermissionRenderer,
) {
    let ep = renderer.episode;
    let map = renderer.map;

    // 1. Background — WIMAP0/1/2 (Doom 1) or INTERPIC (Doom 2), centered.
    let bg_name = if ep > 0 {
        format!("WIMAP{}", ep - 1)
    } else {
        "INTERPIC".to_string()
    };
    if let Some(p) = cache.get(&bg_name, wad) {
        let p = p.clone();
        let x = (320 - p.width as i32) / 2;
        fb.draw_patch(x, 0, &p);
    } else {
        fb.clear(0);
    }

    // Helper: level name patch name.  Doom 1: WILV{ep-1}{map-1}; Doom 2: CWILV{map-1:02}.
    let level_patch = |ep: u8, map: u8| -> String {
        if ep > 0 {
            format!("WILV{}{}", ep - 1, map - 1)
        } else {
            format!("CWILV{:02}", map - 1)
        }
    };

    // 2. "Finished" + current level name.
    if let Some(p) = cache.get("WIF", wad) {
        let p = p.clone();
        fb.draw_patch_vanilla(84, 16, &p);
    }
    let lvname = level_patch(ep, map);
    if let Some(p) = cache.get(&lvname, wad) {
        let p = p.clone();
        fb.draw_patch_vanilla(160, 16, &p);
    }

    // 3. Stats (shown as counting progresses).
    let phase = renderer.phase;
    let show_kills = !matches!(phase, IntermissionPhase::CountingKills);
    let show_items = !matches!(
        phase,
        IntermissionPhase::CountingKills | IntermissionPhase::CountingItems
    );
    let show_secrets = matches!(
        phase,
        IntermissionPhase::ShowingTime | IntermissionPhase::Done
    );
    let show_time = matches!(
        phase,
        IntermissionPhase::ShowingTime | IntermissionPhase::Done
    );

    if let Some(p) = cache.get("WIOSTK", wad) {
        let p = p.clone();
        fb.draw_patch_vanilla(50, 114, &p);
    }
    if show_kills {
        wi_draw_percent(fb, cache, wad, 200, 114, renderer.shown_kills);
    }

    if let Some(p) = cache.get("WIOSTI", wad) {
        let p = p.clone();
        fb.draw_patch_vanilla(50, 134, &p);
    }
    if show_items {
        wi_draw_percent(fb, cache, wad, 200, 134, renderer.shown_items);
    }

    if let Some(p) = cache.get("WISCRT2", wad) {
        let p = p.clone();
        fb.draw_patch_vanilla(50, 154, &p);
    }
    if show_secrets {
        wi_draw_percent(fb, cache, wad, 200, 154, renderer.shown_secrets);
    }

    if show_time {
        if let Some(p) = cache.get("WITIME", wad) {
            let p = p.clone();
            fb.draw_patch_vanilla(16, 180, &p);
        }
        wi_draw_time(fb, cache, wad, 96, 180, renderer.level_time);

        if let Some(p) = cache.get("WIPAR", wad) {
            let p = p.clone();
            fb.draw_patch_vanilla(232, 180, &p);
        }
        wi_draw_time(fb, cache, wad, 296, 180, renderer.par_time);
    }

    // 4. "Entering" + next level name (only when Done).
    if phase == IntermissionPhase::Done {
        if let Some(p) = cache.get("WIENTER", wad) {
            let p = p.clone();
            fb.draw_patch_vanilla(84, 84, &p);
        }
        // Next map: vanilla advances map by 1 (wrapping per episode handled by game).
        let next_map = map + 1;
        let next_patch = level_patch(ep, next_map);
        if let Some(p) = cache.get(&next_patch, wad) {
            let p = p.clone();
            fb.draw_patch_vanilla(160, 84, &p);
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::statusbar::DIGIT_H;

    // -----------------------------------------------------------------------
    // Helper: build a renderer from simple values
    // -----------------------------------------------------------------------

    fn make_renderer(
        target_kills: u8,
        target_items: u8,
        target_secrets: u8,
        par_time: u32,
        level_time: u32,
        episode: u8,
        map: u8,
    ) -> IntermissionRenderer {
        IntermissionRenderer::from_values(
            target_kills,
            target_items,
            target_secrets,
            par_time,
            level_time,
            episode,
            map,
        )
    }

    // --- Test 1: IntermissionRenderer::from_values initializes correctly ---
    #[test]
    fn new_initializes_from_values() {
        let r = make_renderer(80, 50, 100, 120, 90, 1, 3);
        assert_eq!(r.target_kills, 80);
        assert_eq!(r.target_items, 50);
        assert_eq!(r.target_secrets, 100);
        assert_eq!(r.par_time, 120);
        assert_eq!(r.level_time, 90);
        assert_eq!(r.episode, 1);
        assert_eq!(r.map, 3);
        assert_eq!(r.shown_kills, 0);
        assert_eq!(r.shown_items, 0);
        assert_eq!(r.shown_secrets, 0);
        assert_eq!(r.tic, 0);
    }

    // --- Test 2: Phase starts at CountingKills ---
    #[test]
    fn phase_starts_at_counting_kills() {
        let r = make_renderer(50, 50, 50, 60, 60, 1, 1);
        assert_eq!(r.phase, IntermissionPhase::CountingKills);
    }

    // --- Test 3: tick advances shown_kills toward target ---
    #[test]
    fn tick_advances_shown_kills() {
        let mut r = make_renderer(10, 0, 0, 0, 0, 1, 1);
        r.tick();
        assert_eq!(r.shown_kills, 2);
        assert_eq!(r.phase, IntermissionPhase::CountingKills);
    }

    // --- Test 4: tick transitions from CountingKills to CountingItems ---
    #[test]
    fn tick_transitions_kills_to_items() {
        let mut r = make_renderer(4, 50, 50, 60, 60, 1, 1);
        // 4 / 2 = 2 tics to reach target
        r.tick(); // shown_kills = 2
        assert_eq!(r.phase, IntermissionPhase::CountingKills);
        r.tick(); // shown_kills = 4 -> transition
        assert_eq!(r.shown_kills, 4);
        assert_eq!(r.phase, IntermissionPhase::CountingItems);
    }

    // --- Test 5: tick transitions through all phases to Done ---
    #[test]
    fn tick_transitions_all_phases_to_done() {
        // All targets are 0, so each counting phase immediately transitions.
        let mut r = make_renderer(0, 0, 0, 60, 60, 1, 1);

        // CountingKills -> shown_kills already at target (0), transition immediately.
        r.tick();
        assert_eq!(r.phase, IntermissionPhase::CountingItems);

        // CountingItems -> same.
        r.tick();
        assert_eq!(r.phase, IntermissionPhase::CountingSecrets);

        // CountingSecrets -> same.
        r.tick();
        assert_eq!(r.phase, IntermissionPhase::ShowingTime);

        // ShowingTime: needs TIME_DISPLAY_TICS (35) tics.
        for _ in 0..35 {
            r.tick();
        }
        assert_eq!(r.phase, IntermissionPhase::Done);
    }

    // --- Test 6: is_done returns false during counting ---
    #[test]
    fn is_done_false_during_counting() {
        let r = make_renderer(100, 100, 100, 60, 60, 1, 1);
        assert!(!r.is_done());
    }

    // --- Test 7: is_done returns true when Done ---
    #[test]
    fn is_done_true_when_done() {
        let mut r = make_renderer(0, 0, 0, 0, 0, 1, 1);
        r.skip();
        assert!(r.is_done());
    }

    // --- Test 8: skip sets all counters to targets and phase to Done ---
    #[test]
    fn skip_sets_all_to_targets() {
        let mut r = make_renderer(80, 60, 40, 120, 90, 2, 5);
        r.skip();
        assert_eq!(r.shown_kills, 80);
        assert_eq!(r.shown_items, 60);
        assert_eq!(r.shown_secrets, 40);
        assert_eq!(r.phase, IntermissionPhase::Done);
    }

    // --- Test 9: draw_intermission doesn't panic with valid renderer ---
    #[test]
    fn draw_intermission_no_panic() {
        let mut fb = Framebuffer::new();
        let r = make_renderer(100, 80, 50, 120, 100, 1, 1);
        draw_intermission(&mut fb, &r);
        // If we got here, no panic occurred. Verify some pixels were drawn.
        assert!(fb.data.iter().any(|&b| b != 0));
    }

    // --- Test 10: draw_intermission_text renders characters ---
    #[test]
    fn draw_text_renders_characters() {
        let mut fb = Framebuffer::new();
        draw_intermission_text(&mut fb, 10, 10, "AB", COLOR_DEFAULT);
        // The letter 'A' at (10, 10) has its top row with bits 01110 = .XXX.
        // Pixel (11, 10) should be set (bit 3 of 01110).
        assert_ne!(fb.get_pixel(11, 10), Some(0));
    }

    // --- Test 11: draw_percentage renders digits and % sign ---
    #[test]
    fn draw_percentage_renders_digits() {
        let mut fb = Framebuffer::new();
        draw_percentage(&mut fb, 10, 10, 75, COLOR_PERCENT);
        // Should have drawn '7' at (10,10) and '5' at (18,10) plus a % sign.
        // Verify some pixels in the digit region are non-zero.
        let mut has_pixels = false;
        for y in 10..10 + DIGIT_H as usize {
            for x in 10..10 + (DIGIT_W as usize + 1) * 3 {
                if fb.get_pixel(x, y) == Some(COLOR_PERCENT) {
                    has_pixels = true;
                }
            }
        }
        assert!(has_pixels, "draw_percentage should render visible pixels");
    }

    // --- Test 12: draw_time formats MM:SS correctly ---
    #[test]
    fn draw_time_formats_correctly() {
        let mut fb = Framebuffer::new();
        draw_time(&mut fb, 10, 10, 125, COLOR_DEFAULT);
        // 125 seconds = 02:05. Some pixels should be drawn.
        let mut has_pixels = false;
        for y in 10..10 + DIGIT_H as usize {
            for x in 10..60 {
                if fb.get_pixel(x, y) == Some(COLOR_DEFAULT) {
                    has_pixels = true;
                }
            }
        }
        assert!(has_pixels, "draw_time should render visible pixels");
    }

    // --- Test 13: draw_time with 0 seconds shows 00:00 ---
    #[test]
    fn draw_time_zero_seconds() {
        let mut fb = Framebuffer::new();
        draw_time(&mut fb, 10, 10, 0, COLOR_DEFAULT);
        // Should draw "00:00" -- digits at known positions should be rendered.
        // The '0' digit at (10,10) has top row 0b0111110 = .XXXXX.
        // Pixel (11,10) = bit 5 of 0b0111110 should be set.
        assert_eq!(
            fb.get_pixel(11, 10),
            Some(COLOR_DEFAULT),
            "first zero digit should be visible"
        );
    }

    // --- Test 14: draw_time with 125 seconds shows 02:05 ---
    #[test]
    fn draw_time_125_seconds_is_02_05() {
        let mut fb = Framebuffer::new();
        // Draw "02:05" and verify key pixels.
        draw_time(&mut fb, 10, 10, 125, COLOR_DEFAULT);

        // The first digit at x=10 should be '0' (mins tens).
        // The second digit at x=18 should be '2' (mins ones).
        // Verify the '2' digit's bottom row (0b1111111) is present.
        let stride = (DIGIT_W + 1) as usize;
        let digit2_x = 10 + stride; // x=18
        let digit2_bottom_y = 10 + (DIGIT_H - 1) as usize; // y=18
        // '2' bottom row = 0b1111111 = all 7 columns set.
        assert_eq!(
            fb.get_pixel(digit2_x, digit2_bottom_y),
            Some(COLOR_DEFAULT),
            "'2' digit bottom-left pixel should be set"
        );
    }

    // --- Test 15: format_map_name for episode 1 map 1 returns "E1M1" ---
    #[test]
    fn format_map_name_doom1_e1m1() {
        assert_eq!(format_map_name(1, 1), "E1M1");
    }

    // --- Test 16: format_map_name for episode 0 map 12 returns "MAP12" ---
    #[test]
    fn format_map_name_doom2_map12() {
        assert_eq!(format_map_name(0, 12), "MAP12");
    }

    // --- Test 17: Par time comparison (under par vs over par) ---
    #[test]
    fn par_time_color_comparison() {
        // Under par: level_time < par_time -> green
        let r_under = make_renderer(100, 100, 100, 120, 60, 1, 1);
        let mut fb = Framebuffer::new();
        draw_intermission(&mut fb, &r_under);
        // The TIME value should use COLOR_UNDER_PAR (green = 112).
        // Check the TIME row for green pixels.
        let mut has_green = false;
        for x in VALUE_X..VALUE_X + 60 {
            if fb.get_pixel(x, TIME_Y) == Some(COLOR_UNDER_PAR) {
                has_green = true;
            }
        }
        assert!(has_green, "under-par time should render in green");

        // Over par: level_time > par_time -> red
        let r_over = make_renderer(100, 100, 100, 60, 120, 1, 1);
        let mut fb2 = Framebuffer::new();
        draw_intermission(&mut fb2, &r_over);
        let mut has_red = false;
        for x in VALUE_X..VALUE_X + 60 {
            if fb2.get_pixel(x, TIME_Y) == Some(COLOR_OVER_PAR) {
                has_red = true;
            }
        }
        assert!(has_red, "over-par time should render in red");
    }

    // --- Test 18: CountingKills phase increments by 2 per tic ---
    #[test]
    fn counting_kills_increments_by_2() {
        let mut r = make_renderer(20, 0, 0, 0, 0, 1, 1);
        r.tick();
        assert_eq!(r.shown_kills, 2);
        r.tick();
        assert_eq!(r.shown_kills, 4);
        r.tick();
        assert_eq!(r.shown_kills, 6);
    }

    // --- Test 19: percent helper ---
    #[test]
    fn percent_helper_correctness() {
        assert_eq!(percent(10, 20), 50);
        assert_eq!(percent(0, 100), 0);
        assert_eq!(percent(100, 100), 100);
        assert_eq!(percent(0, 0), 100); // Doom convention
        assert_eq!(percent(200, 100), 100); // capped
    }

    // --- Test 20: parse_level_name ---
    #[test]
    fn parse_level_name_doom1() {
        assert_eq!(parse_level_name("E1M1"), (1, 1));
        assert_eq!(parse_level_name("E3M7"), (3, 7));
    }

    #[test]
    fn parse_level_name_doom2() {
        assert_eq!(parse_level_name("MAP01"), (0, 1));
        assert_eq!(parse_level_name("MAP12"), (0, 12));
    }

    // --- Test 21: Draw intermission with Done phase shows prompt ---
    #[test]
    fn draw_intermission_done_shows_prompt() {
        let mut fb = Framebuffer::new();
        let mut r = make_renderer(0, 0, 0, 0, 0, 1, 1);
        r.skip();
        draw_intermission(&mut fb, &r);
        // The "PRESS ANY KEY" prompt should appear on row PROMPT_Y.
        let mut has_prompt_pixels = false;
        for x in 0..320 {
            if fb.get_pixel(x, PROMPT_Y) == Some(COLOR_PROMPT) {
                has_prompt_pixels = true;
            }
        }
        assert!(has_prompt_pixels, "Done phase should show prompt text");
    }

    // --- Test 22: Prompt not shown during counting ---
    #[test]
    fn prompt_not_shown_during_counting() {
        let mut fb = Framebuffer::new();
        let r = make_renderer(100, 100, 100, 60, 60, 1, 1);
        draw_intermission(&mut fb, &r);
        // Should NOT have prompt pixels on PROMPT_Y row.
        let mut has_prompt_pixels = false;
        for x in 0..320 {
            if fb.get_pixel(x, PROMPT_Y) == Some(COLOR_PROMPT) {
                has_prompt_pixels = true;
            }
        }
        assert!(
            !has_prompt_pixels,
            "prompt should not appear during counting"
        );
    }

    // --- Test 23: CountingItems phase works ---
    #[test]
    fn counting_items_phase_increments() {
        let mut r = make_renderer(0, 10, 0, 0, 0, 1, 1);
        // Advance past kills phase (target_kills=0, immediate transition).
        r.tick();
        assert_eq!(r.phase, IntermissionPhase::CountingItems);
        r.tick();
        assert_eq!(r.shown_items, 2);
    }

    // --- Test 24: ShowingTime phase lasts 35 tics ---
    #[test]
    fn showing_time_lasts_35_tics() {
        let mut r = make_renderer(0, 0, 0, 60, 60, 1, 1);
        // Skip through counting phases.
        r.tick(); // -> CountingItems
        r.tick(); // -> CountingSecrets
        r.tick(); // -> ShowingTime
        assert_eq!(r.phase, IntermissionPhase::ShowingTime);

        for _ in 0..34 {
            r.tick();
            assert_eq!(r.phase, IntermissionPhase::ShowingTime);
        }
        r.tick(); // 35th tic -> Done
        assert_eq!(r.phase, IntermissionPhase::Done);
    }

    // --- Test 25: format_map_name zero-pads Doom 2 maps ---
    #[test]
    fn format_map_name_zero_pads() {
        assert_eq!(format_map_name(0, 1), "MAP01");
        assert_eq!(format_map_name(0, 9), "MAP09");
        assert_eq!(format_map_name(0, 32), "MAP32");
    }

    // --- Test 26: draw_percentage with 100% ---
    #[test]
    fn draw_percentage_100() {
        let mut fb = Framebuffer::new();
        draw_percentage(&mut fb, 10, 10, 100, COLOR_PERCENT);
        // '1' at (10,10), '0' at (18,10), '0' at (26,10), '%' at (34,10).
        // '1' has row 0 = 0b0001100 -> pixel at x=13 should be set.
        assert_eq!(
            fb.get_pixel(13, 10),
            Some(COLOR_PERCENT),
            "'1' digit should be drawn for 100%"
        );
    }

    // --- Test 27: draw_percentage with 0% ---
    #[test]
    fn draw_percentage_zero() {
        let mut fb = Framebuffer::new();
        draw_percentage(&mut fb, 10, 10, 0, COLOR_PERCENT);
        // Single '0' at (10,10).
        // '0' top row = 0b0111110 -> pixel (11,10) should be set.
        assert_eq!(
            fb.get_pixel(11, 10),
            Some(COLOR_PERCENT),
            "'0' digit should be drawn for 0%"
        );
    }
}
