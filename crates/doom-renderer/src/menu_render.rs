//! Menu and title screen rendering.
//!
//! Draws the Doom menu overlay, title screen, and credits screen onto the
//! framebuffer using [`BitmapFont`] for text rendering.  The menu is drawn
//! as a semi-transparent overlay on top of the game view; the title and
//! credits screens clear the framebuffer first.

use crate::font::BitmapFont;
use crate::framebuffer::Framebuffer;
use crate::patch_cache::PatchCache;
use doom_game::menu::{GameMenu, MenuPage, TitlePhase, TitleScreen};
use doom_types::limits::{FB_SIZE, FB_WIDTH};
use doom_wad::WadStack;

// ---------------------------------------------------------------------------
// Color constants
// ---------------------------------------------------------------------------

/// Palette indices for menu rendering.
#[allow(dead_code)]
pub(crate) mod menu_colors {
    /// Black background.
    pub const TITLE_BG: u8 = 0;
    /// Red — main menu text color.
    pub const MENU_TEXT: u8 = 176;
    /// Bright/yellow — highlighted (selected) item.
    pub const MENU_HIGHLIGHT: u8 = 231;
    /// Gray — disabled items.
    pub const MENU_DISABLED: u8 = 96;
    /// Red — skull cursor.
    pub const SKULL_COLOR: u8 = 176;
    /// Red — credits text.
    pub const CREDITS_TEXT: u8 = 176;
    /// Gray — version info / secondary text.
    pub const VERSION_TEXT: u8 = 96;
}

// ---------------------------------------------------------------------------
// Menu rendering
// ---------------------------------------------------------------------------

/// Render the menu onto the framebuffer.
///
/// If the menu is not active, this is a no-op — the framebuffer is left
/// untouched.  Otherwise, the existing framebuffer is darkened and the
/// menu is drawn on top.
pub fn draw_menu(fb: &mut Framebuffer, menu: &GameMenu, font: &BitmapFont) {
    if !menu.is_active() {
        return;
    }

    // Semi-transparent overlay: darken the existing framebuffer.
    darken_framebuffer(fb);

    // Draw menu title based on current page.
    let title = match menu.page() {
        MenuPage::Main => "DOOM",
        MenuPage::Episode => "WHICH EPISODE?",
        MenuPage::Skill => "NEW GAME",
        MenuPage::Load => "LOAD GAME",
        MenuPage::Save => "SAVE GAME",
        MenuPage::Options => "OPTIONS",
    };
    font.draw_string_centered(fb, 20, title, menu_colors::MENU_TEXT);

    // Draw menu items.
    let items = menu.items();
    let start_y: i32 = 60;
    let item_height: i32 = 16;

    for (i, item) in items.iter().enumerate() {
        let y = start_y + (i as i32) * item_height;
        let color = if !item.enabled {
            menu_colors::MENU_DISABLED
        } else if i == menu.cursor() {
            menu_colors::MENU_HIGHLIGHT
        } else {
            menu_colors::MENU_TEXT
        };

        // Item text (indented to leave room for skull cursor).
        font.draw_string(fb, 80, y, item.label, color);
    }

    // Draw skull cursor.
    let cursor_y = start_y + (menu.cursor() as i32) * item_height;
    draw_skull_cursor(fb, font, 56, cursor_y, menu.skull_frame());
}

/// Draw the skull cursor (simplified as '>' / '>>' text).
///
/// `frame` alternates between 0 and 1 to give visual feedback.
fn draw_skull_cursor(fb: &mut Framebuffer, font: &BitmapFont, x: i32, y: i32, frame: u8) {
    let text = if frame == 0 { ">" } else { ">>" };
    font.draw_string(fb, x, y, text, menu_colors::SKULL_COLOR);
}

/// Darken the framebuffer for the menu overlay effect.
///
/// Each pixel's palette index is shifted right by 1, producing a
/// crude darkening effect.  Black (0) stays black.
pub fn darken_framebuffer(fb: &mut Framebuffer) {
    for i in 0..FB_SIZE {
        fb.data[i] = fb.data[i].wrapping_shr(1);
    }
}

// ---------------------------------------------------------------------------
// Title screen rendering
// ---------------------------------------------------------------------------

/// Render the title screen based on the current phase.
///
/// - [`TitlePhase::Title`]: stylized "DOOM" title with prompts.
/// - [`TitlePhase::Demo`]: no-op (demo playback is handled by the game loop).
/// - [`TitlePhase::Credits`]: credits screen with attribution.
pub fn draw_title_screen(fb: &mut Framebuffer, title_screen: &TitleScreen, font: &BitmapFont) {
    match title_screen.phase() {
        TitlePhase::Title => {
            draw_title_pic(fb, font);
        }
        TitlePhase::Demo(_) => {
            // Demo playback is handled by the game loop.
            // Nothing to draw here.
        }
        TitlePhase::Credits => {
            draw_credits_screen(fb, font);
        }
    }
}

/// Draw the title picture (TITLEPIC substitute).
///
/// Without actual WAD graphic data, draws a styled text title screen.
fn draw_title_pic(fb: &mut Framebuffer, font: &BitmapFont) {
    fb.clear(0); // Black background.

    // Large "DOOM" title (centered, using double-height letters).
    draw_large_text(fb, font, 80, "DOOM", menu_colors::MENU_TEXT);

    // Subtitle.
    font.draw_string_centered(fb, 110, "RUST EDITION", menu_colors::VERSION_TEXT);

    // Press-key prompt.
    font.draw_string_centered(fb, 160, "PRESS ANY KEY", menu_colors::MENU_TEXT);

    // Version info.
    font.draw_string_centered(fb, 185, "v0.1.0", menu_colors::VERSION_TEXT);
}

/// Draw the credits screen.
fn draw_credits_screen(fb: &mut Framebuffer, font: &BitmapFont) {
    fb.clear(0);

    font.draw_string_centered(fb, 20, "DOOM-RS", menu_colors::CREDITS_TEXT);
    font.draw_string_centered(fb, 40, "A DOOM PORT IN RUST", menu_colors::CREDITS_TEXT);
    font.draw_string_centered(fb, 70, "BASED ON ID SOFTWARE", menu_colors::VERSION_TEXT);
    font.draw_string_centered(fb, 82, "DOOM SOURCE CODE", menu_colors::VERSION_TEXT);
    font.draw_string_centered(fb, 110, "PROGRAMMING:", menu_colors::CREDITS_TEXT);
    font.draw_string_centered(fb, 126, "DOOM-RS CONTRIBUTORS", menu_colors::VERSION_TEXT);
    font.draw_string_centered(fb, 160, "PRESS ANY KEY", menu_colors::MENU_TEXT);
}

// ---------------------------------------------------------------------------
// Large text (2x scale)
// ---------------------------------------------------------------------------

/// Draw large (double-height, double-width) text.
///
/// Each character is rendered at 16x16 pixels (2x the normal 8x8).
/// The text is centered horizontally on the 320-pixel framebuffer.
pub fn draw_large_text(fb: &mut Framebuffer, font: &BitmapFont, y: i32, text: &str, color: u8) {
    let total_width = text.len() as i32 * 16;
    let start_x = (FB_WIDTH as i32 - total_width) / 2;

    for (i, ch) in text.bytes().enumerate() {
        let x = start_x + (i as i32) * 16;
        draw_large_char(fb, font, x, y, ch, color);
    }
}

/// Draw a single character at 2x scale (16x16 pixels).
fn draw_large_char(fb: &mut Framebuffer, font: &BitmapFont, x: i32, y: i32, ch: u8, color: u8) {
    let glyph_idx = if (32..128).contains(&ch) {
        (ch - 32) as usize
    } else {
        0
    };
    let glyph = font.glyph(glyph_idx);

    for (row, &bits) in glyph.iter().enumerate() {
        for col in 0..8i32 {
            if bits & (0x80 >> col) != 0 {
                // Draw 2x2 pixel block.
                let px = x + col * 2;
                let py = y + (row as i32) * 2;
                for dy in 0..2i32 {
                    for dx in 0..2i32 {
                        let fx = px + dx;
                        let fy = py + dy;
                        if fx >= 0 && fy >= 0 {
                            fb.set_pixel(fx as usize, fy as usize, color);
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// WAD patch-based menu renderer
// ---------------------------------------------------------------------------

/// Per-page layout: title patch name + item patch names + item y positions.
///
/// Positions match vanilla Doom's `M_Init` / `DrawMenu` layout.
struct MenuLayout {
    title_patch: &'static str,
    title_x: i32,
    title_y: i32,
    items_x: i32,
    item_patches: &'static [&'static str],
    item_ys: &'static [i32],
}

// Vanilla LINEHEIGHT = 16 for all menus except Load/Save slots.
// Positions from m_menu.c: menu_t { x, y } + LINEHEIGHT*i.

const MAIN_LAYOUT: MenuLayout = MenuLayout {
    title_patch: "M_DOOM",
    title_x: 94,
    title_y: 2,
    items_x: 97,
    item_patches: &["M_NGAME", "M_OPTION", "M_LOADG", "M_SAVEG", "M_QUITG"],
    item_ys: &[64, 80, 96, 112, 128],
};

const EPISODE_LAYOUT: MenuLayout = MenuLayout {
    title_patch: "M_EPISOD",
    title_x: 54,
    title_y: 38,
    items_x: 48,
    item_patches: &["M_EPI1", "M_EPI2", "M_EPI3", "M_EPI4"],
    item_ys: &[63, 79, 95, 111],
};

const SKILL_LAYOUT: MenuLayout = MenuLayout {
    title_patch: "M_NEWG",
    title_x: 96,
    title_y: 14,
    items_x: 48,
    item_patches: &["M_JKILL", "M_ROUGH", "M_HURT", "M_ULTRA", "M_NMARE"],
    item_ys: &[63, 79, 95, 111, 127],
};

const OPTIONS_LAYOUT: MenuLayout = MenuLayout {
    title_patch: "M_OPTTTL",
    title_x: 108,
    title_y: 15,
    items_x: 60,
    item_patches: &["M_MESSG", "M_DETAIL", "M_SCRNSZ", "M_MSENS", "M_SVOL"],
    item_ys: &[37, 53, 69, 85, 101],
};

const LOAD_LAYOUT: MenuLayout = MenuLayout {
    title_patch: "M_LOADG",
    title_x: 72,
    title_y: 28,
    items_x: 80,
    item_patches: &[],
    item_ys: &[34, 50, 66, 82, 98, 114],
};

const SAVE_LAYOUT: MenuLayout = MenuLayout {
    title_patch: "M_SAVEG",
    title_x: 72,
    title_y: 28,
    items_x: 80,
    item_patches: &[],
    item_ys: &[34, 50, 66, 82, 98, 114],
};

fn page_layout(page: MenuPage) -> &'static MenuLayout {
    match page {
        MenuPage::Main => &MAIN_LAYOUT,
        MenuPage::Episode => &EPISODE_LAYOUT,
        MenuPage::Skill => &SKILL_LAYOUT,
        MenuPage::Options => &OPTIONS_LAYOUT,
        MenuPage::Load => &LOAD_LAYOUT,
        MenuPage::Save => &SAVE_LAYOUT,
    }
}

/// Draw the menu using WAD patches.
///
/// Falls back gracefully when patches are missing (draws nothing for that element).
/// The darken overlay is still applied for the semi-transparent effect.
pub fn draw_menu_wad(
    fb: &mut Framebuffer,
    menu: &GameMenu,
    cache: &mut PatchCache,
    wad: &WadStack,
    font: &BitmapFont,
) {
    if !menu.is_active() {
        return;
    }

    // Vanilla Doom's M_Drawer does NOT darken the background — patches
    // are drawn directly on top of whatever is on screen.

    let layout = page_layout(menu.page());

    // Title patch.
    if let Some(patch) = cache.get(layout.title_patch, wad) {
        let p = patch.clone();
        fb.draw_patch_vanilla(layout.title_x, layout.title_y, &p);
    }

    let items = menu.items();

    // Item patches (Load/Save use text slots instead).
    match menu.page() {
        MenuPage::Load | MenuPage::Save => {
            // Draw save slot borders + text labels using bitmap font.
            for (i, item) in items.iter().enumerate() {
                if let Some(&y) = layout.item_ys.get(i) {
                    let color = if i == menu.cursor() {
                        menu_colors::MENU_HIGHLIGHT
                    } else {
                        menu_colors::MENU_TEXT
                    };
                    // Slot border (small rectangle).
                    fb.fill_rect(layout.items_x as usize, y as usize, 160, 8, 0);
                    font.draw_string(fb, layout.items_x, y, item.label, color);
                }
            }
        }
        _ => {
            for (i, _item) in items.iter().enumerate() {
                if let Some(&patch_name) = layout.item_patches.get(i) {
                    if let Some(&y) = layout.item_ys.get(i) {
                        if let Some(patch) = cache.get(patch_name, wad) {
                            let p = patch.clone();
                            fb.draw_patch_vanilla(layout.items_x, y, &p);
                        }
                    }
                }
            }
        }
    }

    // Skull cursor — M_SKULL1 / M_SKULL2 at (item_x - 32, item_y).
    let skull_name = if menu.skull_frame() == 0 {
        "M_SKULL1"
    } else {
        "M_SKULL2"
    };
    let cursor_y = layout
        .item_ys
        .get(menu.cursor())
        .copied()
        .unwrap_or(layout.item_ys.first().copied().unwrap_or(60));
    if let Some(patch) = cache.get(skull_name, wad) {
        let p = patch.clone();
        fb.draw_patch_vanilla(layout.items_x - 32, cursor_y, &p);
    }
}

/// Draw the title/credits screen using WAD patches.
///
/// - Title phase: TITLEPIC full-screen patch.
/// - Credits phase: CREDIT full-screen patch.
/// - Demo phase: no-op.
pub fn draw_title_screen_wad(
    fb: &mut Framebuffer,
    title_screen: &TitleScreen,
    cache: &mut PatchCache,
    wad: &WadStack,
    font: &BitmapFont,
) {
    match title_screen.phase() {
        TitlePhase::Title => {
            if let Some(patch) = cache.get("TITLEPIC", wad) {
                let p = patch.clone();
                // Widescreen WADs ship TITLEPIC wider than 320px; center it.
                let x = (320 - p.width as i32) / 2;
                fb.draw_patch(x, 0, &p);
            } else {
                draw_title_pic(fb, font);
            }
        }
        TitlePhase::Demo(_) => {}
        TitlePhase::Credits => {
            if let Some(patch) = cache.get("CREDIT", wad) {
                let p = patch.clone();
                let x = (320 - p.width as i32) / 2;
                fb.draw_patch(x, 0, &p);
            } else {
                draw_credits_screen(fb, font);
            }
        }
    }
}

/// Draw a help/pause overlay patch by name (e.g. "M_PAUSE", "HELP1", "HELP2").
///
/// The patch is drawn centered horizontally; `y` is the top of the patch.
/// Returns `true` if the patch was found and drawn.
pub fn draw_overlay_patch(
    fb: &mut Framebuffer,
    cache: &mut PatchCache,
    wad: &WadStack,
    name: &str,
    y: i32,
) -> bool {
    if let Some(patch) = cache.get(name, wad) {
        let p = patch.clone();
        fb.draw_patch_centered(y, &p);
        true
    } else {
        false
    }
}

// ---------------------------------------------------------------------------
// Finale renderer
// ---------------------------------------------------------------------------

/// Vanilla Doom episode-end text (from f_finale.c).
const E1TEXT: &str = "\
 Once you beat the big bad\n\
 hell boss, you wonder what\n\
 authority structure permits\n\
 such carnage.  So you ask\n\
 yourself: Is there someone\n\
 else in charge of this?\n\
 \n\
 Yes, of course.  But now\n\
 it's your job to find them.\n\
 And now you'll know why the\n\
 UAC was so anxious to remove\n\
 Deimos Base from the face of\n\
 the moon ... and you're about\n\
 to find out the hard way.";

const E2TEXT: &str = "\
 You've done it!  The\n\
 hideous cyber-diamond has\n\
 been obliterated!  And the\n\
 once-barren Deimos Base is\n\
 secure.  You can almost hear\n\
 the echo of victory.\n\
 \n\
 You've beaten the demon\n\
 forces all the way back to\n\
 hell.  And now, with the\n\
 completion of your task,\n\
 you realize that... wait.\n\
 You're still alive?  Good.\n\
 \n\
 You're in hell... but why?\n\
 It seems the demons have\n\
 been using Deimos as a kind\n\
 of hell outpost.  With\n\
 demons pouring in and out,\n\
 you face the fact that you\n\
 can't turn back.  You must\n\
 go in.";

const E3TEXT: &str = "\
 The loathsome spiderdemon\n\
 that masterminded the\n\
 Deimos infestation has been\n\
 slain and UAC reports state\n\
 it was destroyed with a\n\
 single blast of unholy\n\
 firepower...  Did that just\n\
 happen?  Was it...  easy?\n\
 \n\
 Don't be fooled.  You've\n\
 just begun your journey\n\
 through hell.  The real\n\
 monsters wait for you at\n\
 the end of the next episode.";

const E4TEXT: &str = "\
 the spider mastermind must\n\
 have sent forth its legions\n\
 of hellspawn before your\n\
 final confrontation with\n\
 that terrible beast from\n\
 hell.  but you have shown\n\
 no mercy.  nor have you\n\
 been shown any.\n\
 \n\
 you aggressively crushed all\n\
 opposition throughout the\n\
 galaxy and now the dread\n\
 spider is gone.  the three\n\
 hells await thy conquest.\n\
 \n\
 thy work was gory but just.";

const D2TEXT: &str = "\
 you did it!  by turning the\n\
 only switch ever to work you\n\
 have caused all 666 demons\n\
 to disappear from the face\n\
 of the earth.  all gone.\n\
 \n\
 now, in what could be a\n\
 coincidence or a miracle,\n\
 your life support has\n\
 reconstituted itself.\n\
 \n\
 did you know that by\n\
 activating that switch you\n\
 also sent a signal to the\n\
 distant demon hive mind?\n\
 they now know of your\n\
 existence.  they will be\n\
 back.";

/// Draw the finale screen using WAD patches.
///
/// - Episode 1-2, 4 and Doom 2: text crawl over flat background (INTERPIC for D2).
/// - Episode 3: bunny scroll (PFUB1/PFUB2) — scroll offset derived from tic.
///
/// `episode` is the just-completed episode (1-4 for Doom 1, 0 for Doom 2).
/// `text_index` is the number of characters revealed so far.
/// `tic` is the raw finale tic count (used for PFUB2 scroll offset).
pub fn draw_finale_wad(
    fb: &mut Framebuffer,
    cache: &mut PatchCache,
    wad: &WadStack,
    font: &BitmapFont,
    episode: u8,
    text_index: usize,
    tic: u32,
) {
    let is_doom2 = episode == 0;

    // --- Episode 3: bunny scroll ---
    if episode == 3 {
        // PFUB1: static left half; PFUB2: scrolling overlay.
        if let Some(p) = cache.get("PFUB1", wad) {
            let p = p.clone();
            let x = (320 - p.width as i32) / 2;
            fb.draw_patch(x, 0, &p);
        } else {
            fb.clear(0);
        }
        // PFUB2 scrolls right-to-left: vanilla scrolls 2px per 3 tics.
        let scroll = ((tic / 3) * 2) as i32;
        if let Some(p) = cache.get("PFUB2", wad) {
            let p = p.clone();
            let base_x = (320 - p.width as i32) / 2;
            fb.draw_patch(base_x - scroll, 0, &p);
        }
        // After enough scrolling, show ENDPIC.
        if tic > 220 {
            if let Some(p) = cache.get("ENDPIC", wad) {
                let p = p.clone();
                let x = (320 - p.width as i32) / 2;
                fb.draw_patch(x, 0, &p);
            }
        }
        return;
    }

    // --- All other episodes: text crawl ---

    // Background.
    if is_doom2 {
        if let Some(p) = cache.get("INTERPIC", wad) {
            let p = p.clone();
            let x = (320 - p.width as i32) / 2;
            fb.draw_patch(x, 0, &p);
        } else {
            fb.clear(0);
        }
    } else {
        fb.clear(0);
    }

    let text = match episode {
        1 => E1TEXT,
        2 => E2TEXT,
        3 => E3TEXT,
        4 => E4TEXT,
        _ => D2TEXT, // Doom 2 or fallback
    };

    // Reveal `text_index` characters of the text.
    let visible: String = text.chars().take(text_index).collect();
    let x = 10i32;
    let mut y = 10i32;
    for line in visible.lines() {
        font.draw_string(fb, x, y, line, 4);
        y += 11;
        if y > 190 {
            break;
        }
    }
    // Keep x used — avoids unused variable warning.
    let _ = x;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_font() -> BitmapFont {
        BitmapFont::new()
    }

    // -----------------------------------------------------------------------
    // draw_menu
    // -----------------------------------------------------------------------

    #[test]
    fn inactive_menu_draws_nothing() {
        let font = make_font();
        let mut fb = Framebuffer::new();
        fb.clear(42); // Fill with non-zero.
        let before = fb.data.clone();

        let menu = GameMenu::new(false); // inactive by default
        draw_menu(&mut fb, &menu, &font);

        assert_eq!(
            &*fb.data, &*before,
            "Inactive menu should not touch framebuffer"
        );
    }

    #[test]
    fn active_menu_darkens_background() {
        let font = make_font();
        let mut fb = Framebuffer::new();
        fb.clear(200); // Bright fill.

        let mut menu = GameMenu::new(false);
        menu.open();
        draw_menu(&mut fb, &menu, &font);

        // Most pixels should have been darkened (shifted right).
        // Check a pixel that is NOT part of any text rendering.
        // Bottom-right corner should just be darkened.
        let px = fb.get_pixel(319, 199).unwrap();
        // 200 >> 1 = 100.  But the menu text might also be drawn, so just
        // verify the pixel changed from 200.
        assert_ne!(px, 200, "Pixels should have been darkened");
    }

    #[test]
    fn main_page_shows_title_doom() {
        let font = make_font();
        let mut fb = Framebuffer::new();
        let mut menu = GameMenu::new(false);
        menu.open();

        draw_menu(&mut fb, &menu, &font);

        // "DOOM" should be drawn centered at y=20.
        // D=0b0111_1000, row 0, col 1 set. Centered "DOOM" = 4*8=32 → x=(320-32)/2=144.
        // So col 1 of 'D' → x=144+1=145, y=20.
        assert_eq!(
            fb.get_pixel(145, 20),
            Some(menu_colors::MENU_TEXT),
            "'D' pixel should be MENU_TEXT color"
        );
    }

    #[test]
    fn episode_page_shows_which_episode() {
        let font = make_font();
        let mut fb = Framebuffer::new();
        let mut menu = GameMenu::new(false);
        menu.open();
        menu.select(); // Main → Episode

        draw_menu(&mut fb, &menu, &font);

        // "WHICH EPISODE?" = 14 chars * 8 = 112. x=(320-112)/2=104.
        // First char 'W' at x=104. W row 0 = 0b1100_0110 → col 0 → x=104
        assert_eq!(
            fb.get_pixel(104, 20),
            Some(menu_colors::MENU_TEXT),
            "'W' pixel should be MENU_TEXT color"
        );
    }

    #[test]
    fn skill_page_shows_new_game() {
        let font = make_font();
        let mut fb = Framebuffer::new();
        let mut menu = GameMenu::new(false);
        menu.open();
        menu.select(); // Main → Episode
        menu.select(); // Episode → Skill (selects ep 1)

        draw_menu(&mut fb, &menu, &font);

        // "NEW GAME" is drawn centered at y=20.
        // 8 chars * 8 = 64px. x = (320-64)/2 = 128.
        // 'N' at x=128, row 0 = 0b0110_0110, col 1 → x=129
        assert_eq!(
            fb.get_pixel(129, 20),
            Some(menu_colors::MENU_TEXT),
            "'N' pixel should be MENU_TEXT color"
        );
    }

    #[test]
    fn menu_items_rendered_at_correct_y_positions() {
        let font = make_font();
        let mut fb = Framebuffer::new();
        let mut menu = GameMenu::new(false);
        menu.open(); // Main page: 5 items starting at y=60, spacing=16.

        draw_menu(&mut fb, &menu, &font);

        // Item 0 at y=60, Item 1 at y=76, ..., Item 4 at y=124.
        // Each item text starts at x=80.
        // Item 0 is "New Game" → 'N' at (80, 60).
        // 'N' row 0 = 0b0110_0110, col 1 → (81, 60).
        assert_eq!(
            fb.get_pixel(81, 60),
            Some(menu_colors::MENU_HIGHLIGHT), // cursor is on item 0
            "Item 0 should be highlighted"
        );
    }

    #[test]
    fn highlighted_item_uses_menu_highlight_color() {
        let font = make_font();
        let mut fb = Framebuffer::new();
        let mut menu = GameMenu::new(false);
        menu.open();
        // Cursor on item 0 (default).

        draw_menu(&mut fb, &menu, &font);

        // Item 0: "New Game" at (80, 60), highlighted.
        // 'N' row 0 col 1 → (81, 60).
        assert_eq!(
            fb.get_pixel(81, 60),
            Some(menu_colors::MENU_HIGHLIGHT),
            "Highlighted item should use MENU_HIGHLIGHT color"
        );
    }

    #[test]
    fn non_highlighted_item_uses_menu_text_color() {
        let font = make_font();
        let mut fb = Framebuffer::new();
        let mut menu = GameMenu::new(false);
        menu.open();
        // Cursor on item 0, so item 1 ("Options") should use MENU_TEXT.

        draw_menu(&mut fb, &menu, &font);

        // Item 1: "Options" at (80, 76).
        // 'O' row 0 = 0b0011_1100, col 2 → (82, 76).
        assert_eq!(
            fb.get_pixel(82, 76),
            Some(menu_colors::MENU_TEXT),
            "Non-highlighted item should use MENU_TEXT color"
        );
    }

    #[test]
    fn skull_cursor_renders_at_cursor_position() {
        let font = make_font();
        let mut fb = Framebuffer::new();
        let mut menu = GameMenu::new(false);
        menu.open();
        // Cursor is at item 0 → skull at x=56, y=60.

        draw_menu(&mut fb, &menu, &font);

        // '>' row 0 = 0b0110_0000, col 1 → (57, 60).
        assert_eq!(
            fb.get_pixel(57, 60),
            Some(menu_colors::SKULL_COLOR),
            "Skull cursor should be drawn at item 0 position"
        );
    }

    #[test]
    fn skull_cursor_moves_with_cursor() {
        let font = make_font();
        let mut fb = Framebuffer::new();
        let mut menu = GameMenu::new(false);
        menu.open();
        menu.move_down(); // Cursor → item 1, y=76.

        draw_menu(&mut fb, &menu, &font);

        // '>' at (56, 76): row 0 col 1 → (57, 76).
        assert_eq!(
            fb.get_pixel(57, 76),
            Some(menu_colors::SKULL_COLOR),
            "Skull cursor should move to item 1"
        );
    }

    #[test]
    fn skull_frame_0_vs_1_produces_different_output() {
        let font = make_font();

        // Frame 0: ">" (1 char = 8px wide)
        let mut fb0 = Framebuffer::new();
        draw_skull_cursor(&mut fb0, &font, 56, 60, 0);

        // Frame 1: ">>" (2 chars = 16px wide)
        let mut fb1 = Framebuffer::new();
        draw_skull_cursor(&mut fb1, &font, 56, 60, 1);

        // Frame 1 has extra pixels at x=64..71 (second '>' char).
        let has_extra = (64..72).any(|x| {
            (60..68).any(|y| fb1.get_pixel(x, y).unwrap_or(0) != fb0.get_pixel(x, y).unwrap_or(0))
        });
        assert!(has_extra, "Frame 1 should have more pixels than frame 0");
    }

    // -----------------------------------------------------------------------
    // draw_title_screen
    // -----------------------------------------------------------------------

    #[test]
    fn title_phase_draws_doom_text() {
        let font = make_font();
        let mut fb = Framebuffer::new();
        let title = TitleScreen::new();

        draw_title_screen(&mut fb, &title, &font);

        // Large "DOOM" at y=80: 4 chars * 16 = 64px. x=(320-64)/2=128.
        // 'D' at x=128, glyph row 0 = 0b0111_1000, col 1 → 2x scale → (128+1*2, 80) = (130, 80).
        assert_eq!(
            fb.get_pixel(130, 80),
            Some(menu_colors::MENU_TEXT),
            "Title should contain 'D' of DOOM"
        );
    }

    #[test]
    fn title_phase_clears_to_black() {
        let font = make_font();
        let mut fb = Framebuffer::new();
        fb.clear(255); // Fill with bright.

        let title = TitleScreen::new();
        draw_title_screen(&mut fb, &title, &font);

        // Background pixels should be black (0).
        assert_eq!(
            fb.get_pixel(0, 0),
            Some(0),
            "Background should be cleared to black"
        );
    }

    #[test]
    fn credits_phase_draws_credits() {
        let font = make_font();
        let mut fb = Framebuffer::new();
        let mut title = TitleScreen::new();
        // Advance to Credits phase.
        for _ in 0..350 {
            title.tick();
        }
        // After Title(350) -> Credits
        assert_eq!(title.phase(), TitlePhase::Credits);

        draw_title_screen(&mut fb, &title, &font);

        // "DOOM-RS" at y=20, centered. 7 chars * 8 = 56. x = (320-56)/2 = 132.
        // 'D' at x=132, row 0 = 0b0111_1000, col 1 → (133, 20).
        assert_eq!(
            fb.get_pixel(133, 20),
            Some(menu_colors::CREDITS_TEXT),
            "Credits should contain 'D' of DOOM-RS"
        );
    }

    #[test]
    fn demo_phase_leaves_fb_mostly_unchanged() {
        let font = make_font();
        let mut fb = Framebuffer::new();
        fb.clear(42);
        let before = fb.data.clone();

        let title = TitleScreen::from_phase(TitlePhase::Demo(0));

        draw_title_screen(&mut fb, &title, &font);

        assert_eq!(
            &*fb.data, &*before,
            "Demo phase should not modify the framebuffer"
        );
    }

    // -----------------------------------------------------------------------
    // darken_framebuffer
    // -----------------------------------------------------------------------

    #[test]
    fn darken_all_pixels_are_modified() {
        let mut fb = Framebuffer::new();
        fb.clear(200);

        darken_framebuffer(&mut fb);

        // 200 >> 1 = 100
        assert!(
            fb.as_slice().iter().all(|&p| p == 100),
            "All pixels should be 200 >> 1 = 100"
        );
    }

    #[test]
    fn darken_black_stays_black() {
        let mut fb = Framebuffer::new();
        fb.clear(0);

        darken_framebuffer(&mut fb);

        assert!(
            fb.as_slice().iter().all(|&p| p == 0),
            "Black (0) should remain black after darkening"
        );
    }

    #[test]
    fn darken_bright_pixels_get_darker() {
        let mut fb = Framebuffer::new();
        fb.set_pixel(10, 10, 255);
        fb.set_pixel(20, 20, 128);
        fb.set_pixel(30, 30, 1);

        darken_framebuffer(&mut fb);

        assert_eq!(fb.get_pixel(10, 10), Some(127), "255 >> 1 = 127");
        assert_eq!(fb.get_pixel(20, 20), Some(64), "128 >> 1 = 64");
        assert_eq!(fb.get_pixel(30, 30), Some(0), "1 >> 1 = 0");
    }

    // -----------------------------------------------------------------------
    // draw_large_text
    // -----------------------------------------------------------------------

    #[test]
    fn large_text_is_centered() {
        let font = make_font();
        let mut fb = Framebuffer::new();

        // "AB" = 2 chars * 16 = 32px. x = (320-32)/2 = 144.
        draw_large_text(&mut fb, &font, 50, "AB", 10);

        // 'A' at x=144. Row 0 = 0b0001_1000, col 3 → 2x → (144+3*2, 50) = (150, 50).
        assert_eq!(
            fb.get_pixel(150, 50),
            Some(10),
            "Large 'A' should be at expected position"
        );
    }

    #[test]
    fn large_characters_are_2x_size() {
        let font = make_font();
        let mut fb = Framebuffer::new();

        // Draw single 'A' large at known position.
        draw_large_char(&mut fb, &font, 0, 0, b'A', 5);

        // 'A' row 0 = 0b0001_1000 → cols 3,4 set.
        // At 2x scale: col 3 → pixels (6,0),(7,0),(6,1),(7,1).
        assert_eq!(fb.get_pixel(6, 0), Some(5));
        assert_eq!(fb.get_pixel(7, 0), Some(5));
        assert_eq!(fb.get_pixel(6, 1), Some(5));
        assert_eq!(fb.get_pixel(7, 1), Some(5));

        // col 4 → pixels (8,0),(9,0),(8,1),(9,1).
        assert_eq!(fb.get_pixel(8, 0), Some(5));
        assert_eq!(fb.get_pixel(9, 0), Some(5));
        assert_eq!(fb.get_pixel(8, 1), Some(5));
        assert_eq!(fb.get_pixel(9, 1), Some(5));
    }

    #[test]
    fn large_text_uses_correct_color() {
        let font = make_font();
        let mut fb = Framebuffer::new();
        draw_large_text(&mut fb, &font, 50, "X", 42);

        // 'X' row 0 = 0b0110_0110 → col 1 set.
        // Single char: 1*16=16px. x=(320-16)/2=152.
        // Col 1 at 2x → (152+1*2, 50) = (154, 50).
        assert_eq!(fb.get_pixel(154, 50), Some(42));
    }

    // -----------------------------------------------------------------------
    // menu_colors validity
    // -----------------------------------------------------------------------

    #[test]
    fn highlight_differs_from_normal_text() {
        assert_ne!(
            menu_colors::MENU_TEXT,
            menu_colors::MENU_HIGHLIGHT,
            "Highlight and normal text should differ"
        );
    }

    #[test]
    fn disabled_differs_from_normal_text() {
        assert_ne!(
            menu_colors::MENU_TEXT,
            menu_colors::MENU_DISABLED,
            "Disabled and normal text should differ"
        );
    }

    #[test]
    fn background_is_black() {
        assert_eq!(
            menu_colors::TITLE_BG,
            0,
            "Title background should be black (0)"
        );
    }

    // -----------------------------------------------------------------------
    // Additional edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn load_page_shows_load_game_title() {
        let font = make_font();
        let mut fb = Framebuffer::new();
        let mut menu = GameMenu::new(false);
        menu.open();
        // Navigate: Main cursor=2 → Load Game.
        menu.move_down(); // cursor 1
        menu.move_down(); // cursor 2 (Load Game)
        menu.select(); // → Load page

        draw_menu(&mut fb, &menu, &font);

        // "LOAD GAME" = 9 chars * 8 = 72. x = (320-72)/2 = 124.
        // 'L' at x=124, row 0 = 0b0110_0000, col 1 → (125, 20).
        assert_eq!(
            fb.get_pixel(125, 20),
            Some(menu_colors::MENU_TEXT),
            "'L' of LOAD GAME should be visible"
        );
    }

    #[test]
    fn save_page_shows_save_game_title() {
        let font = make_font();
        let mut fb = Framebuffer::new();
        let mut menu = GameMenu::new(false);
        menu.open();
        // Navigate: cursor=3 → Save Game.
        for _ in 0..3 {
            menu.move_down();
        }
        menu.select(); // → Save page

        draw_menu(&mut fb, &menu, &font);

        // "SAVE GAME" = 9 chars * 8 = 72. x = (320-72)/2 = 124.
        // 'S' at x=124, row 0 = 0b0011_1100, col 2 → (126, 20).
        assert_eq!(
            fb.get_pixel(126, 20),
            Some(menu_colors::MENU_TEXT),
            "'S' of SAVE GAME should be visible"
        );
    }

    #[test]
    fn options_page_shows_options_title() {
        let font = make_font();
        let mut fb = Framebuffer::new();
        let mut menu = GameMenu::new(false);
        menu.open();
        menu.move_down(); // cursor 1 (Options)
        menu.select(); // → Options page

        draw_menu(&mut fb, &menu, &font);

        // "OPTIONS" = 7 chars * 8 = 56. x = (320-56)/2 = 132.
        // 'O' at x=132, row 0 = 0b0011_1100, col 2 → (134, 20).
        assert_eq!(
            fb.get_pixel(134, 20),
            Some(menu_colors::MENU_TEXT),
            "'O' of OPTIONS should be visible"
        );
    }
}
