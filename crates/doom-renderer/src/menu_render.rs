//! Menu and title screen rendering.
//!
//! Draws the Doom menu overlay, title screen, and credits screen onto the
//! framebuffer using [`BitmapFont`] for text rendering.  The menu is drawn
//! as a semi-transparent overlay on top of the game view; the title and
//! credits screens clear the framebuffer first.

use crate::font::BitmapFont;
use crate::framebuffer::Framebuffer;
use doom_game::menu::{GameMenu, MenuPage, TitlePhase, TitleScreen};
use doom_types::limits::{FB_SIZE, FB_WIDTH};

// ---------------------------------------------------------------------------
// Color constants
// ---------------------------------------------------------------------------

/// Palette indices for menu rendering.
pub mod menu_colors {
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
    let glyph_idx = if ch >= 32 && ch < 128 {
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
        for _ in 0..700 {
            title.tick();
        }
        // After Title(350) → Demo(0), Demo(350) → Credits
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

        let mut title = TitleScreen::new();
        // Advance to Demo phase.
        for _ in 0..350 {
            title.tick();
        }
        assert!(matches!(title.phase(), TitlePhase::Demo(_)));

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
    fn all_color_values_in_valid_range() {
        // All u8 values are 0-255 by definition, but verify they are
        // different enough to be visually distinct.
        let colors = [
            menu_colors::TITLE_BG,
            menu_colors::MENU_TEXT,
            menu_colors::MENU_HIGHLIGHT,
            menu_colors::MENU_DISABLED,
            menu_colors::SKULL_COLOR,
            menu_colors::CREDITS_TEXT,
            menu_colors::VERSION_TEXT,
        ];
        // All within range (trivially true for u8, but let's be explicit).
        for &c in &colors {
            assert!(c <= 255, "Color {c} should be in 0-255 range");
        }
    }

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
