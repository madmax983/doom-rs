//! Fixed-step Doom game loop: 35 tic/sec simulation, variable render rate.
//!
//! Ported from `abrash/src/platform/tui.rs` with these changes:
//! - Game simulation ticks at a fixed 35 Hz regardless of render rate.
//! - Render runs at monitor Hz (queried via Windows API, else 60 Hz fallback).
//! - Input collected via `InputState` → `TicInput` each tic.
//! - Framebuffer is palette-indexed `Framebuffer` + `PaletteLut`, not ARGB.
//!
//! # Game loop pseudocode
//! ```text
//! loop:
//!   elapsed = time since last frame
//!   accumulate elapsed into tic_timer
//!   while tic_timer >= TIC_DURATION:
//!       input = held_keys → TicInput
//!       app.tick(input)
//!       tic_timer -= TIC_DURATION
//!   app.render(fb)
//!   blit fb to terminal
//!   sleep to fill remaining frame budget (vsync approximation)
//! ```

use crate::input::{InputState, TicInput};
use crate::scaler::ScalingMode;
use crate::sixel::DoomSixelWidget;
use crate::widget::DoomFramebufferWidget;
use crossterm::{
    event::{
        self, KeyCode, KeyEventKind, KeyModifiers, KeyboardEnhancementFlags,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
        supports_keyboard_enhancement,
    },
};
use doom_renderer::{Framebuffer, PaletteLut};
use image::{DynamicImage, RgbImage};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::Paragraph,
};
use ratatui_image::{Resize, StatefulImage, picker::Picker, picker::ProtocolType};
use std::io::{Stdout, stdout};
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;

#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

/// Doom simulation runs at exactly 35 tics per second.
pub const TIC_RATE_HZ: u32 = 35;

/// Wall-clock duration of one tic (≈ 28.571 ms).
pub const TIC_DURATION: Duration = Duration::from_nanos(1_000_000_000 / TIC_RATE_HZ as u64);

const DEFAULT_REFRESH_HZ: u32 = 60;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ModifierSnapshot {
    shift: Option<bool>,
    control: Option<bool>,
}

#[cfg(test)]
static MODIFIER_SAMPLE_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Errors that can occur during event loop setup or execution.
#[derive(Debug, Error)]
pub enum EventLoopError {
    #[error("terminal setup failed: {0}")]
    TerminalSetup(String),
    #[error("terminal draw error: {0}")]
    Draw(String),
}

/// Application trait — implement this to integrate your game with the event loop.
pub trait DoomApp {
    /// Called once per tic (35×/sec) with the synthesized input for that tic.
    fn tick(&mut self, input: TicInput);

    /// Called once per render frame to write palette-indexed pixels into `fb`.
    ///
    /// `fb` is reused across frames; clear or overwrite it as needed.
    fn render(&mut self, fb: &mut Framebuffer);

    /// Which of the 14 PLAYPAL entries to display (0 = normal, 1-8 = pain, etc.).
    fn active_palette(&self) -> usize {
        0
    }
}

/// The Doom terminal event loop.
///
/// Manages terminal lifecycle, fixed-rate tic accumulation, input tracking,
/// and render pacing. Call [`DoomEventLoop::run`] to start the loop.
pub struct DoomEventLoop {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    input: InputState,
    is_running: bool,

    /// Target time per rendered frame (1 / monitor Hz).
    target_frame_time: Duration,
    /// Accumulated real time waiting to be consumed as tics.
    tic_accumulator: Duration,

    // Timing bookkeeping.
    frame_start: Instant,
    fps: f64,
    frames_since_update: u64,
    last_fps_update: Instant,
    frame_count: u64,

    /// Scaling algorithm used when blitting the framebuffer to the terminal (halfblocks path).
    scaling_mode: ScalingMode,

    /// Graphics protocol picker — detects Kitty/Sixel/iTerm2 or falls back to halfblocks.
    picker: Picker,

    /// When true, use the detected graphics protocol (Kitty/Sixel/iTerm2) instead of halfblocks.
    ///
    /// Defaults to `false` because Sixel/iTerm2 encoding is too slow for 35+ Hz game rendering
    /// on most terminals.  Enable with [`DoomEventLoop::set_graphics_protocol`] or let the user
    /// toggle in-game.
    use_graphics_protocol: bool,

    /// Whether keyboard enhancement flags were successfully pushed at startup.
    ///
    /// When active, the terminal sends real `KeyEventKind::Release` events so the
    /// `held` set is properly cleared on key-up.  Without this, Release events
    /// never arrive and keys can get stuck.
    keyboard_enhancement_active: bool,
}

/// Query the primary monitor's refresh rate via platform APIs.
///
/// On Windows, uses `EnumDisplaySettingsW`; falls back to `DEFAULT_REFRESH_HZ`
/// on non-Windows platforms or if the query fails.
#[allow(clippy::missing_const_for_fn)]
fn query_refresh_rate() -> u32 {
    #[cfg(target_os = "windows")]
    {
        // SAFETY: EnumDisplaySettingsW with a null device name queries the
        // primary monitor's current display settings.  DEVMODEW must be
        // zero-initialized with `dmSize` set before the call.
        unsafe {
            use std::mem;
            use windows_sys::Win32::Graphics::Gdi::{
                DEVMODEW, ENUM_CURRENT_SETTINGS, EnumDisplaySettingsW,
            };

            let mut devmode: DEVMODEW = mem::zeroed();
            devmode.dmSize = mem::size_of::<DEVMODEW>() as u16;

            if EnumDisplaySettingsW(std::ptr::null(), ENUM_CURRENT_SETTINGS, &raw mut devmode) != 0
                && devmode.dmDisplayFrequency > 0
            {
                devmode.dmDisplayFrequency
            } else {
                DEFAULT_REFRESH_HZ
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        DEFAULT_REFRESH_HZ
    }
}

fn sampled_modifier_snapshot() -> ModifierSnapshot {
    #[cfg(test)]
    MODIFIER_SAMPLE_COUNT.fetch_add(1, Ordering::Relaxed);

    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
            GetAsyncKeyState, VK_CONTROL, VK_SHIFT,
        };

        // SAFETY: `GetAsyncKeyState` is a pure Win32 query for the current
        // asynchronous key state. We only read the high bit to determine if
        // either Shift or Control is physically down right now.
        let is_down =
            |virtual_key: i32| unsafe { (GetAsyncKeyState(virtual_key) as u16 & 0x8000) != 0 };

        ModifierSnapshot {
            shift: Some(is_down(VK_SHIFT as i32)),
            control: Some(is_down(VK_CONTROL as i32)),
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        ModifierSnapshot::default()
    }
}

impl DoomEventLoop {
    fn sync_sampled_modifiers(&mut self) {
        let snapshot = sampled_modifier_snapshot();
        self.input.sync_modifiers(snapshot.shift, snapshot.control);
    }

    fn drain_ready_tics<A: DoomApp>(&mut self, app: &mut A) {
        while self.tic_accumulator >= TIC_DURATION {
            self.sync_sampled_modifiers();
            let tic_input = self.input.to_tic_input();
            app.tick(tic_input);
            self.tic_accumulator -= TIC_DURATION;
        }
    }

    /// Set up the terminal (raw mode, alternate screen) and return the event loop.
    pub fn new() -> Result<Self, EventLoopError> {
        enable_raw_mode().map_err(|e| EventLoopError::TerminalSetup(e.to_string()))?;

        let mut out = stdout();
        execute!(out, EnterAlternateScreen)
            .map_err(|e| EventLoopError::TerminalSetup(e.to_string()))?;

        // Enable keyboard enhancement so we receive KeyEventKind::Release events.
        // This prevents keys from getting "stuck" in the held set when the terminal
        // doesn't send release events by default (which is most terminals without this).
        // We request: disambiguate escape codes + report all keys (for modifier-only keys).
        // Falls back gracefully if the terminal doesn't support it.
        let keyboard_enhancement_active = supports_keyboard_enhancement().unwrap_or(false)
            && execute!(
                out,
                PushKeyboardEnhancementFlags(
                    KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                        | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                )
            )
            .is_ok();

        // Query the terminal for graphics protocol support AFTER entering alternate screen
        // but BEFORE consuming any terminal events.  Falls back to halfblocks if the query
        // fails or the terminal doesn't support any graphics protocol.
        let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());

        let backend = CrosstermBackend::new(out);
        let mut terminal =
            Terminal::new(backend).map_err(|e| EventLoopError::TerminalSetup(e.to_string()))?;
        terminal.clear().ok();

        let hz = query_refresh_rate();
        let target_frame_time = Duration::from_secs_f64(1.0 / f64::from(hz));

        Ok(Self {
            terminal,
            input: InputState::new(),
            is_running: true,
            target_frame_time,
            tic_accumulator: Duration::ZERO,
            frame_start: Instant::now(),
            fps: 0.0,
            frames_since_update: 0,
            last_fps_update: Instant::now(),
            frame_count: 0,
            scaling_mode: ScalingMode::Nearest,
            picker,
            use_graphics_protocol: false,
            keyboard_enhancement_active,
        })
    }

    /// Return which graphics protocol was detected at startup.
    pub fn protocol_type(&self) -> ProtocolType {
        self.picker.protocol_type()
    }

    /// Enable or disable the graphics protocol (Kitty/Sixel/iTerm2) renderer.
    ///
    /// Has no effect if the terminal only supports halfblocks.
    pub fn set_graphics_protocol(&mut self, enable: bool) {
        self.use_graphics_protocol = enable;
    }

    /// Toggle between halfblocks and the detected graphics protocol.
    ///
    /// Returns whether graphics protocol is now active.
    pub fn toggle_graphics_protocol(&mut self) -> bool {
        if self.picker.protocol_type() != ProtocolType::Halfblocks {
            self.use_graphics_protocol = !self.use_graphics_protocol;
        }
        self.use_graphics_protocol
    }

    /// Set the scaling algorithm used when blitting the framebuffer to the terminal.
    ///
    /// Defaults to [`ScalingMode::Nearest`] (no behavior change from earlier versions).
    pub fn set_scaling_mode(&mut self, mode: ScalingMode) {
        self.scaling_mode = mode;
    }

    /// Run the game loop until the user quits (Q or Escape).
    ///
    /// `lut` is used to resolve palette indices to RGB at blit time.
    pub fn run<A: DoomApp>(&mut self, app: &mut A, lut: &PaletteLut) -> Result<(), EventLoopError> {
        let mut fb = Framebuffer::new();
        let mut last_frame = Instant::now();

        while self.is_running {
            self.frame_start = Instant::now();
            let elapsed = last_frame.elapsed();
            last_frame = Instant::now();

            // --- Input ---
            self.poll_events();

            // --- Fixed-step tic simulation ---
            self.tic_accumulator += elapsed;
            self.drain_ready_tics(app);

            // --- Render ---
            app.render(&mut fb);
            let active_palette = app.active_palette();
            self.blit(&fb, lut, active_palette)?;

            // --- Frame pacing: sleep to fill remaining budget ---
            let draw_elapsed = self.frame_start.elapsed();
            if let Some(remaining) = self.target_frame_time.checked_sub(draw_elapsed) {
                thread::sleep(remaining);
            }
        }

        Ok(())
    }

    /// Non-blocking drain of all pending crossterm events.
    fn poll_events(&mut self) {
        while event::poll(Duration::from_millis(0)).unwrap_or(false) {
            match event::read() {
                Ok(event::Event::Key(key)) => {
                    // Mirror shift state into InputState for strafe detection.
                    self.input.sync_modifiers(
                        Some(key.modifiers.contains(KeyModifiers::SHIFT)),
                        Some(key.modifiers.contains(KeyModifiers::CONTROL)),
                    );

                    match key.kind {
                        KeyEventKind::Press => {
                            // Q = hard quit; Escape is forwarded to the app as an
                            // edge-triggered signal so it can handle menu/console logic.
                            if key.code == KeyCode::Char('q') {
                                self.is_running = false;
                            }
                            if key.code == KeyCode::Esc {
                                self.input.push_escape();
                            }
                            // Quick save / load via F5 / F9.
                            if key.code == KeyCode::F(5) {
                                self.input.push_f5();
                            } else if key.code == KeyCode::F(9) {
                                self.input.push_f9();
                            } else if key.code == KeyCode::Tab {
                                self.input.push_tab();
                            }
                            // Edge-triggered menu navigation (Up/Down/Enter).
                            // These are also registered as held keys below for gameplay.
                            if key.code == KeyCode::Up {
                                self.input.push_menu_up();
                            } else if key.code == KeyCode::Down {
                                self.input.push_menu_down();
                            } else if key.code == KeyCode::Enter {
                                self.input.push_menu_select();
                            }
                            // Queue raw char for console/cheat processing.
                            if let KeyCode::Char(ch) = key.code {
                                self.input.push_console_char(ch);
                            } else if key.code == KeyCode::Enter {
                                self.input.push_console_char('\n');
                            } else if key.code == KeyCode::Backspace {
                                self.input.push_console_char('\x08');
                            }
                            self.input.key_down(key.code);
                        }
                        KeyEventKind::Release => {
                            self.input.key_up(key.code);
                        }
                        _ => {}
                    }
                }
                Ok(event::Event::FocusLost) => {
                    // Release all keys when the window loses focus to prevent
                    // stuck keys after alt-tab.
                    self.input.clear();
                }
                _ => {}
            }
        }
    }

    /// Blit the framebuffer to the terminal.
    ///
    /// If the terminal supports a graphics protocol (Kitty/Sixel/iTerm2) the framebuffer is
    /// converted to an RGB image and rendered via ratatui-image at true pixel resolution.
    /// Otherwise the existing half-block `▀` widget is used as a fallback.
    fn blit(
        &mut self,
        fb: &Framebuffer,
        lut: &PaletteLut,
        active_palette: usize,
    ) -> Result<(), EventLoopError> {
        // Update FPS counter.
        self.frame_count += 1;
        self.frames_since_update += 1;
        let now = Instant::now();
        let since_update = now.duration_since(self.last_fps_update);
        if since_update.as_secs_f64() >= 1.0 {
            self.fps = self.frames_since_update as f64 / since_update.as_secs_f64();
            self.frames_since_update = 0;
            self.last_fps_update = now;
        }

        let fps = self.fps;
        let frame_count = self.frame_count;
        let scaling_mode = self.scaling_mode;
        let protocol_type = self.picker.protocol_type();
        let font_size = self.picker.font_size();
        let use_gfx = self.use_graphics_protocol && protocol_type != ProtocolType::Halfblocks;

        // For Kitty/iTerm2: build a StatefulProtocol before the draw closure
        // (borrow checker: can't hold &self.picker and &mut self.terminal simultaneously).
        let mut img_proto = match (use_gfx, protocol_type) {
            (true, ProtocolType::Kitty) | (true, ProtocolType::Iterm2) => {
                let rgb_data: Vec<u8> = fb
                    .as_slice()
                    .iter()
                    .flat_map(|&idx| {
                        let rgb = lut.get(active_palette, idx);
                        [rgb.r, rgb.g, rgb.b]
                    })
                    .collect();
                RgbImage::from_raw(
                    Framebuffer::width() as u32,
                    Framebuffer::height() as u32,
                    rgb_data,
                )
                .map(|img| {
                    self.picker
                        .new_resize_protocol(DynamicImage::ImageRgb8(img))
                })
            }
            _ => None,
        };

        self.terminal
            .draw(|f| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(0), Constraint::Length(1)])
                    .split(f.area());

                if use_gfx && protocol_type == ProtocolType::Sixel {
                    // Fast palette-aware sixel: no color quantization, scales to fill terminal.
                    let widget = DoomSixelWidget::new(fb, lut, active_palette, font_size);
                    f.render_widget(widget, chunks[0]);
                } else if let Some(ref mut proto) = img_proto {
                    // Kitty / iTerm2: use ratatui-image StatefulProtocol.
                    let widget = StatefulImage::default().resize(Resize::Scale(None));
                    f.render_stateful_widget(widget, chunks[0], proto);
                } else {
                    // Halfblocks (default): palette-aware, zero RGB conversion.
                    let widget = DoomFramebufferWidget::new(fb, lut, active_palette)
                        .with_scaling(scaling_mode);
                    f.render_widget(widget, chunks[0]);
                }

                let proto_name = match (use_gfx, protocol_type) {
                    (true, ProtocolType::Sixel) => "sixel*",
                    (true, ProtocolType::Kitty) => "kitty",
                    (true, ProtocolType::Iterm2) => "iterm2",
                    _ => "halfblocks",
                };
                let status = format!(
                    " DOOM | FPS: {fps:.1} | Frame: {frame_count} | {proto_name} | [Q/Esc] Quit "
                );
                let status_bar = Paragraph::new(status)
                    .style(Style::default().fg(Color::Black).bg(Color::Yellow));
                f.render_widget(status_bar, chunks[1]);
            })
            .map_err(|e| EventLoopError::Draw(e.to_string()))?;

        Ok(())
    }
}

impl Drop for DoomEventLoop {
    fn drop(&mut self) {
        // Restore terminal state on exit, even if we panic.
        if self.keyboard_enhancement_active {
            let _ = execute!(self.terminal.backend_mut(), PopKeyboardEnhancementFlags);
        }
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reset_modifier_sample_count() {
        MODIFIER_SAMPLE_COUNT.store(0, Ordering::Relaxed);
    }

    fn modifier_sample_count() -> usize {
        MODIFIER_SAMPLE_COUNT.load(Ordering::Relaxed)
    }

    fn make_test_event_loop() -> DoomEventLoop {
        let backend = CrosstermBackend::new(stdout());
        let terminal = Terminal::new(backend).expect("test terminal");
        DoomEventLoop {
            terminal,
            input: InputState::new(),
            is_running: true,
            target_frame_time: Duration::from_secs_f64(1.0 / f64::from(DEFAULT_REFRESH_HZ)),
            tic_accumulator: Duration::ZERO,
            frame_start: Instant::now(),
            fps: 0.0,
            frames_since_update: 0,
            last_fps_update: Instant::now(),
            frame_count: 0,
            scaling_mode: ScalingMode::Nearest,
            picker: Picker::halfblocks(),
            use_graphics_protocol: false,
            keyboard_enhancement_active: false,
        }
    }

    #[test]
    fn tic_duration_is_approx_28ms() {
        // 1/35s ≈ 28.571 ms — check we're within 1µs of the exact value.
        let expected_ns = 1_000_000_000u64 / 35;
        assert_eq!(TIC_DURATION.as_nanos() as u64, expected_ns);
    }

    #[test]
    fn tic_accumulator_arithmetic_drains_correctly() {
        // Simulate 3 tics worth of elapsed time arriving in one batch.
        let mut acc = Duration::ZERO;
        let elapsed = TIC_DURATION * 3 + Duration::from_millis(5); // 3 full tics + leftover
        acc += elapsed;

        let mut tic_count = 0u32;
        while acc >= TIC_DURATION {
            tic_count += 1;
            acc -= TIC_DURATION;
        }
        assert_eq!(tic_count, 3);
        // Leftover < TIC_DURATION.
        assert!(acc < TIC_DURATION);
        assert!(acc >= Duration::from_millis(5));
    }

    #[test]
    fn tic_rate_constant_is_35() {
        assert_eq!(TIC_RATE_HZ, 35);
    }

    /// Dummy app for testing the tic dispatch logic.
    struct CountingApp {
        ticks: u32,
    }

    impl DoomApp for CountingApp {
        fn tick(&mut self, _input: TicInput) {
            self.ticks += 1;
        }
        fn render(&mut self, _fb: &mut Framebuffer) {}
    }

    #[test]
    fn dummy_app_tick_count() {
        let mut app = CountingApp { ticks: 0 };
        let mut acc = Duration::ZERO;

        // Feed 2.5 tics worth of time.
        let elapsed = TIC_DURATION * 2 + TIC_DURATION / 2;
        acc += elapsed;

        while acc >= TIC_DURATION {
            let input = TicInput::default();
            app.tick(input);
            acc -= TIC_DURATION;
        }

        assert_eq!(app.ticks, 2);
    }

    #[test]
    fn poll_events_does_not_sample_modifiers() {
        reset_modifier_sample_count();
        let mut loop_ = make_test_event_loop();

        loop_.poll_events();

        assert_eq!(modifier_sample_count(), 0);
    }

    #[test]
    fn drain_ready_tics_samples_modifiers_once_per_tic() {
        reset_modifier_sample_count();
        let mut loop_ = make_test_event_loop();
        let mut app = CountingApp { ticks: 0 };
        loop_.tic_accumulator = TIC_DURATION * 2 + TIC_DURATION / 2;

        loop_.drain_ready_tics(&mut app);

        assert_eq!(app.ticks, 2);
        assert_eq!(modifier_sample_count(), 2);
        assert_eq!(loop_.tic_accumulator, TIC_DURATION / 2);
    }

    #[test]
    fn default_active_palette_is_zero() {
        let app = CountingApp { ticks: 0 };
        assert_eq!(app.active_palette(), 0);
    }
}
