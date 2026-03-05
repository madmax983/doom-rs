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
use crate::widget::DoomFramebufferWidget;
use crossterm::{
    event::{self, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use doom_renderer::{Framebuffer, PaletteLut};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::Paragraph,
};
use std::io::{Stdout, stdout};
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;

/// Doom simulation runs at exactly 35 tics per second.
pub const TIC_RATE_HZ: u32 = 35;

/// Wall-clock duration of one tic (≈ 28.571 ms).
pub const TIC_DURATION: Duration = Duration::from_nanos(1_000_000_000 / TIC_RATE_HZ as u64);

const DEFAULT_REFRESH_HZ: u32 = 60;

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

    /// Scaling algorithm used when blitting the framebuffer to the terminal.
    scaling_mode: ScalingMode,
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

impl DoomEventLoop {
    /// Set up the terminal (raw mode, alternate screen) and return the event loop.
    pub fn new() -> Result<Self, EventLoopError> {
        enable_raw_mode().map_err(|e| EventLoopError::TerminalSetup(e.to_string()))?;

        let mut out = stdout();
        execute!(out, EnterAlternateScreen)
            .map_err(|e| EventLoopError::TerminalSetup(e.to_string()))?;

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
        })
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
            while self.tic_accumulator >= TIC_DURATION {
                let tic_input = self.input.to_tic_input(); // consumes pending_console_char
                app.tick(tic_input);
                self.tic_accumulator -= TIC_DURATION;
            }

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
                    self.input
                        .set_shift(key.modifiers.contains(KeyModifiers::SHIFT));

                    match key.kind {
                        KeyEventKind::Press => {
                            if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) {
                                self.is_running = false;
                            }
                            // Quick save / load via F5 / F9.
                            if key.code == KeyCode::F(5) {
                                self.input.push_f5();
                            } else if key.code == KeyCode::F(9) {
                                self.input.push_f9();
                            } else if key.code == KeyCode::Tab {
                                self.input.push_tab();
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

    /// Blit the framebuffer to the terminal via `DoomFramebufferWidget`.
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

        self.terminal
            .draw(|f| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(0), Constraint::Length(1)])
                    .split(f.area());

                let widget =
                    DoomFramebufferWidget::new(fb, lut, active_palette).with_scaling(scaling_mode);
                f.render_widget(widget, chunks[0]);

                let status =
                    format!(" DOOM | FPS: {fps:.1} | Frame: {frame_count} | [Q/Esc] Quit ");
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
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn default_active_palette_is_zero() {
        let app = CountingApp { ticks: 0 };
        assert_eq!(app.active_palette(), 0);
    }
}
