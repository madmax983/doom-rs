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
//!   try_send(encoded_frame) to blit thread   ← non-blocking; drop if blit still busy
//!   sleep to fill remaining frame budget (vsync approximation)
//! ```
//!
//! # Double-buffer async blit
//!
//! Terminal I/O (sixel in particular) takes ~28 ms per frame — far too long to
//! block the game loop.  The blit thread owns the `Terminal` handle and calls
//! `terminal.draw()` at its own rate.  The main loop prepares the next frame
//! (encode sixel, clone framebuffer) and hands it off via a bounded channel
//! (`sync_channel(1)`), then continues immediately.  If the channel is full the
//! frame is silently dropped; the blit thread will display the next one instead.

use crate::charset::{CharSet, RendererMode};
use crate::cogmind::{CogmindFrame, CogmindHud, CogmindHudWidget, CogmindWidget};
use crate::input::{InputState, TicInput};
use crate::scaler::ScalingMode;
use crate::sixel::encode_doom_sixel;
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
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
    mpsc::{Receiver, SyncSender, sync_channel},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use thiserror::Error;

#[cfg(test)]
use std::sync::atomic::AtomicUsize;

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

/// Doom simulation runs at exactly 35 tics per second.
pub const TIC_RATE_HZ: u32 = 35;

/// Wall-clock duration of one tic (≈ 28.571 ms).
pub const TIC_DURATION: Duration = Duration::from_nanos(1_000_000_000 / TIC_RATE_HZ as u64);

const DEFAULT_REFRESH_HZ: u32 = 60;

// ─────────────────────────────────────────────────────────────────────────────
// Blit thread types
// ─────────────────────────────────────────────────────────────────────────────

/// Render content for one frame, sent from the main loop to the blit thread.
enum BlitPayload {
    /// Half-block or character-mapped: 320×200 palette-indexed framebuffer.
    Halfblocks {
        fb: Framebuffer,
        active_palette: usize,
        scaling_mode: ScalingMode,
        char_set: Option<CharSet>,
    },
    /// Pre-encoded sixel DCS string (dirty-frame cache already applied on main thread).
    Sixel(String),
    /// Kitty / iTerm2: pre-converted RGB image.
    ImageProtocol { image: DynamicImage },
    /// Cogmind mode: pre-rendered grid of styled cells.
    Cogmind(CogmindFrame),
}

struct BlitFrame {
    payload: BlitPayload,
    /// Status bar text to render at the bottom of the terminal.
    status: String,
    /// Cogmind HUD data (replaces status bar in cogmind mode).
    cogmind_hud: Option<CogmindHud>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Blit thread
// ─────────────────────────────────────────────────────────────────────────────

/// Runs on the dedicated blit thread.
///
/// Owns the `Terminal` handle exclusively.  The main loop sends `BlitFrame`
/// values through a bounded channel; this function calls `terminal.draw()` for
/// each one, stores the elapsed time in `blit_elapsed_us`, and handles terminal
/// cleanup when the channel closes.
fn run_blit_thread(
    rx: Receiver<BlitFrame>,
    mut terminal: Terminal<CrosstermBackend<Stdout>>,
    lut: PaletteLut,
    picker: Picker,
    keyboard_enhancement_active: bool,
    blit_elapsed_us: Arc<AtomicU64>,
) {
    while let Ok(frame) = rx.recv() {
        let t = Instant::now();

        terminal
            .draw(|f| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(0), Constraint::Length(1)])
                    .split(f.area());

                match &frame.payload {
                    BlitPayload::Halfblocks {
                        fb,
                        active_palette,
                        scaling_mode,
                        char_set,
                    } => {
                        let widget = DoomFramebufferWidget::new(fb, &lut, *active_palette)
                            .with_scaling(*scaling_mode)
                            .with_char_set(*char_set);
                        f.render_widget(widget, chunks[0]);
                    }

                    BlitPayload::Sixel(sixel_str) => {
                        // Direct buffer write: identical strings between frames → ratatui
                        // emits zero bytes for this cell → only status bar is flushed.
                        if let Some(cell) = f.buffer_mut().cell_mut((chunks[0].x, chunks[0].y)) {
                            cell.set_symbol(sixel_str);
                        }
                        let mut past_first = false;
                        for y in chunks[0].top()..chunks[0].bottom() {
                            for x in chunks[0].left()..chunks[0].right() {
                                if !past_first {
                                    past_first = true;
                                    continue;
                                }
                                f.buffer_mut().cell_mut((x, y)).map(|c| c.set_skip(true));
                            }
                        }
                    }

                    BlitPayload::ImageProtocol { image } => {
                        // Create a fresh protocol per frame (kitty's incremental update
                        // is not used; each frame is a complete image transmission).
                        let mut proto = picker.new_resize_protocol(image.clone());
                        let widget = StatefulImage::default().resize(Resize::Scale(None));
                        f.render_stateful_widget(widget, chunks[0], &mut proto);
                    }

                    BlitPayload::Cogmind(frame) => {
                        let widget = CogmindWidget::new(frame);
                        f.render_widget(widget, chunks[0]);
                    }
                }

                if let Some(hud) = &frame.cogmind_hud {
                    let widget = CogmindHudWidget::new(hud);
                    f.render_widget(widget, chunks[1]);
                } else {
                    let status_bar = Paragraph::new(frame.status.as_str())
                        .style(Style::default().fg(Color::Black).bg(Color::Yellow));
                    f.render_widget(status_bar, chunks[1]);
                }
            })
            .ok();

        blit_elapsed_us.store(t.elapsed().as_micros() as u64, Ordering::Relaxed);
    }

    // Channel closed (sender dropped in DoomEventLoop::drop) → restore terminal.
    if keyboard_enhancement_active {
        let _ = execute!(terminal.backend_mut(), PopKeyboardEnhancementFlags);
    }
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();
}

// ─────────────────────────────────────────────────────────────────────────────
// Modifier polling
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ModifierSnapshot {
    shift: Option<bool>,
    control: Option<bool>,
}

#[cfg(test)]
static MODIFIER_SAMPLE_COUNT: AtomicUsize = AtomicUsize::new(0);

// ─────────────────────────────────────────────────────────────────────────────
// Error type
// ─────────────────────────────────────────────────────────────────────────────

/// Errors that can occur during event loop setup or execution.
#[derive(Debug, Error)]
pub enum EventLoopError {
    /// Failed to configure crossterm (raw mode or alternate screen).
    #[error("terminal setup failed: {0}")]
    TerminalSetup(String),
    /// Failed to draw the frame layout via Ratatui.
    #[error("terminal draw error: {0}")]
    Draw(String),
}

// ─────────────────────────────────────────────────────────────────────────────
// DoomApp trait
// ─────────────────────────────────────────────────────────────────────────────

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

    /// Called once per rendered frame after the frame has been dispatched to the
    /// blit thread, with timing data for that frame:
    ///
    /// - `tick_us`   — time spent in `drain_ready_tics` (game simulation)
    /// - `render_us` — time spent in `render()` (doom-renderer)
    /// - `blit_us`   — actual `terminal.draw()` duration on the blit thread
    ///   (one frame stale; read from a shared atomic)
    ///
    /// Default: no-op.  Override to write to a debug log or update in-game stats.
    fn on_frame_timings(&mut self, _tick_us: u64, _render_us: u64, _blit_us: u64) {}

    /// Render a Cogmind-mode frame for the given terminal dimensions.
    ///
    /// Returns `Some(frame)` if the app supports Cogmind mode, `None` otherwise.
    /// Default: `None` (not supported).
    fn render_cogmind(&mut self, _term_w: u16, _term_h: u16) -> Option<CogmindFrame> {
        None
    }

    /// Return HUD data for cogmind-mode status bar.
    ///
    /// Default: `None` (falls back to generic status bar).
    fn cogmind_hud(&self) -> Option<CogmindHud> {
        None
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// DoomEventLoop
// ─────────────────────────────────────────────────────────────────────────────

/// The Doom terminal event loop.
///
/// Manages terminal lifecycle, fixed-rate tic accumulation, input tracking,
/// and render pacing.  The `Terminal` handle is moved into a background blit
/// thread on the first call to [`DoomEventLoop::run()`]; the main thread never
/// writes to stdout after that point.
///
/// Call [`DoomEventLoop::run()`] to start the loop.
pub struct DoomEventLoop {
    /// Terminal handle — `Some` until `run()` is first called, at which point it
    /// is moved into the blit thread.  `None` while the loop is running.
    pending_terminal: Option<Terminal<CrosstermBackend<Stdout>>>,

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

    /// Per-phase timings from the previous frame (microseconds).
    last_tick_us: u64,
    last_render_us: u64,

    /// Scaling algorithm used for halfblocks rendering.
    scaling_mode: ScalingMode,

    /// Protocol / font-size information detected at startup.
    /// Kept on the main thread for toggling and status display;
    /// a clone is sent to the blit thread.
    picker: Picker,

    /// The current renderer mode (halfblocks, sixel, kitty, iterm2, or character-mapped).
    renderer_mode: RendererMode,

    /// Character set for character-mapped modes (derived from renderer_mode).
    char_set: Option<CharSet>,

    /// Whether keyboard enhancement flags were pushed at startup.
    /// Passed to the blit thread so it can pop them on exit.
    keyboard_enhancement_active: bool,

    // ── Sixel dirty-frame cache ──────────────────────────────────────────────
    // Pre-encode the sixel string on the main thread (fast, ~4 ms) and cache it.
    // On clean frames the same string is sent to the blit thread; ratatui's cell
    // diff skips the pty write for that cell entirely (~0 ms instead of ~28 ms).
    sixel_cache: String,
    sixel_prev_fb: Vec<u8>,
    sixel_prev_palette: usize,
    sixel_prev_term_size: (u16, u16),

    // ── Async blit thread ────────────────────────────────────────────────────
    /// Sender half of the frame channel.  Dropped in `Drop` to signal the blit
    /// thread to exit.
    blit_tx: Option<SyncSender<BlitFrame>>,
    /// Join handle for the blit thread.
    blit_thread: Option<JoinHandle<()>>,
    /// Elapsed time of the most-recent `terminal.draw()` call on the blit thread
    /// (microseconds).  Written by the blit thread; read by the main thread for
    /// status-bar display and `on_frame_timings`.
    blit_elapsed_us: Arc<AtomicU64>,
    /// When true, simulation advances only in response to discrete actions.
    turn_based_mode: bool,
    /// Remaining simulated tics in the current recovery phase.
    turn_recovery_tics: u32,
    /// Prevent repeated turns while action keys remain held.
    turn_waiting_for_release: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// Platform helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Query the primary monitor's refresh rate via platform APIs.
#[allow(clippy::missing_const_for_fn)]
fn query_refresh_rate() -> u32 {
    #[cfg(target_os = "windows")]
    {
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

// ─────────────────────────────────────────────────────────────────────────────
// impl DoomEventLoop
// ─────────────────────────────────────────────────────────────────────────────

impl DoomEventLoop {
    fn tic_has_turn_action(input: &TicInput) -> bool {
        input.wait_pressed
            || input.forward_move != 0
            || input.side_move != 0
            || input.angle_turn != 0
            || (input.buttons != 0)
    }

    fn turn_action_cost(input: &TicInput) -> u32 {
        if input.wait_pressed {
            6
        } else if (input.buttons & crate::input::buttons::BT_ATTACK) != 0 {
            8
        } else if (input.buttons & crate::input::buttons::BT_USE) != 0 {
            7
        } else if (input.buttons & crate::input::buttons::BT_CHANGE) != 0 {
            4
        } else {
            6
        }
    }

    fn tick_turn_based<A: DoomApp>(&mut self, app: &mut A) {
        if self.turn_recovery_tics > 0 {
            app.tick(TicInput::default());
            self.turn_recovery_tics -= 1;
            return;
        }

        self.sync_sampled_modifiers();
        let tic_input = self.input.to_tic_input();

        let held_action = tic_input.forward_move != 0
            || tic_input.side_move != 0
            || tic_input.angle_turn != 0
            || ((tic_input.buttons
                & (crate::input::buttons::BT_ATTACK
                    | crate::input::buttons::BT_USE
                    | crate::input::buttons::BT_CHANGE))
                != 0);

        if self.turn_waiting_for_release {
            if !held_action {
                self.turn_waiting_for_release = false;
            }
            return;
        }

        if Self::tic_has_turn_action(&tic_input) {
            app.tick(tic_input);
            self.turn_recovery_tics = Self::turn_action_cost(&tic_input).saturating_sub(1);
            self.turn_waiting_for_release = held_action;
        }
    }

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
    ///
    /// The `Terminal` handle is stored internally until [`run`](DoomEventLoop::run) is called, at
    /// which point it is moved into the blit thread.
    pub fn new() -> Result<Self, EventLoopError> {
        enable_raw_mode().map_err(|e| EventLoopError::TerminalSetup(e.to_string()))?;

        let mut out = stdout();
        execute!(out, EnterAlternateScreen)
            .map_err(|e| EventLoopError::TerminalSetup(e.to_string()))?;

        let keyboard_enhancement_active = supports_keyboard_enhancement().unwrap_or(false)
            && execute!(
                out,
                PushKeyboardEnhancementFlags(
                    KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                        | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                )
            )
            .is_ok();

        let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());

        let backend = CrosstermBackend::new(out);
        let mut terminal =
            Terminal::new(backend).map_err(|e| EventLoopError::TerminalSetup(e.to_string()))?;
        terminal.clear().ok();

        let hz = query_refresh_rate();
        let target_frame_time = Duration::from_secs_f64(1.0 / f64::from(hz));

        Ok(Self {
            pending_terminal: Some(terminal),
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
            renderer_mode: RendererMode::Halfblocks,
            char_set: None,
            keyboard_enhancement_active,
            last_tick_us: 0,
            last_render_us: 0,
            sixel_cache: String::new(),
            sixel_prev_fb: Vec::new(),
            sixel_prev_palette: usize::MAX,
            sixel_prev_term_size: (0, 0),
            blit_tx: None,
            blit_thread: None,
            blit_elapsed_us: Arc::new(AtomicU64::new(0)),
            turn_based_mode: false,
            turn_recovery_tics: 0,
            turn_waiting_for_release: false,
        })
    }

    /// Return which graphics protocol was detected at startup.
    pub fn protocol_type(&self) -> ProtocolType {
        self.picker.protocol_type()
    }

    /// Return the current renderer mode.
    pub fn renderer_mode(&self) -> RendererMode {
        self.renderer_mode
    }

    /// Set the renderer mode.
    ///
    /// Graphics protocols (Sixel/Kitty/iTerm2) silently fall back to
    /// halfblocks if the terminal doesn't support them.
    pub fn set_renderer_mode(&mut self, mode: RendererMode) {
        self.renderer_mode = mode;
        self.char_set = match mode {
            RendererMode::CharMap(cs) => Some(cs),
            _ => None,
        };
    }

    /// Cycle to the next renderer mode (for F2 toggle).
    ///
    /// Returns the new mode.
    pub fn cycle_renderer_mode(&mut self) -> RendererMode {
        let next = self.renderer_mode.next();
        self.set_renderer_mode(next);
        next
    }

    /// Legacy: enable or disable the detected graphics protocol.
    pub fn set_graphics_protocol(&mut self, enable: bool) {
        if enable {
            // Pick the best available graphics protocol.
            let mode = match self.picker.protocol_type() {
                ProtocolType::Sixel => RendererMode::Sixel,
                ProtocolType::Kitty => RendererMode::Kitty,
                ProtocolType::Iterm2 => RendererMode::Iterm2,
                _ => RendererMode::Halfblocks,
            };
            self.set_renderer_mode(mode);
        } else {
            self.set_renderer_mode(RendererMode::Halfblocks);
        }
    }

    /// Legacy: toggle between halfblocks and the detected graphics protocol.
    pub fn toggle_graphics_protocol(&mut self) -> bool {
        let is_gfx = matches!(
            self.renderer_mode,
            RendererMode::Sixel | RendererMode::Kitty | RendererMode::Iterm2
        );
        if is_gfx {
            self.set_renderer_mode(RendererMode::Halfblocks);
            false
        } else if self.picker.protocol_type() != ProtocolType::Halfblocks {
            self.set_graphics_protocol(true);
            true
        } else {
            false
        }
    }

    /// Set the scaling algorithm used for halfblocks rendering.
    pub fn set_scaling_mode(&mut self, mode: ScalingMode) {
        self.scaling_mode = mode;
    }

    /// Enable/disable turn-based execution.
    pub fn set_turn_based_mode(&mut self, enabled: bool) {
        self.turn_based_mode = enabled;
        if !enabled {
            self.turn_recovery_tics = 0;
            self.turn_waiting_for_release = false;
        }
    }

    fn effective_renderer_mode(&self) -> RendererMode {
        match self.renderer_mode {
            RendererMode::Sixel if self.picker.protocol_type() != ProtocolType::Sixel => {
                RendererMode::Halfblocks
            }
            RendererMode::Kitty if self.picker.protocol_type() != ProtocolType::Kitty => {
                RendererMode::Halfblocks
            }
            RendererMode::Iterm2 if self.picker.protocol_type() != ProtocolType::Iterm2 => {
                RendererMode::Halfblocks
            }
            other => other,
        }
    }

    /// Run the game loop until the user quits (Q or Escape).
    ///
    /// On the first call, the `Terminal` handle is moved into the blit thread
    /// along with a clone of `lut`.  Subsequent calls reuse the same thread.
    pub fn run<A: DoomApp>(&mut self, app: &mut A, lut: &PaletteLut) -> Result<(), EventLoopError> {
        // ── Spawn blit thread on first call ─────────────────────────────────
        if self.blit_tx.is_none() {
            let terminal = self
                .pending_terminal
                .take()
                .ok_or_else(|| EventLoopError::TerminalSetup("terminal already consumed".into()))?;

            let lut_clone = lut.clone();
            let picker_clone = self.picker.clone();
            let kea = self.keyboard_enhancement_active;
            let elapsed_arc = Arc::clone(&self.blit_elapsed_us);

            // Capacity 1: main thread can prepare one frame ahead without blocking.
            // If the channel is full (blit thread still writing), try_send drops the
            // new frame — the display rate is naturally limited to the terminal's
            // render throughput (~35 fps for a 28 ms sixel blit).
            let (tx, rx) = sync_channel::<BlitFrame>(1);

            let handle = thread::spawn(move || {
                run_blit_thread(rx, terminal, lut_clone, picker_clone, kea, elapsed_arc);
            });

            self.blit_tx = Some(tx);
            self.blit_thread = Some(handle);
        }

        let mut fb = Framebuffer::new();
        let mut last_frame = Instant::now();

        while self.is_running {
            self.frame_start = Instant::now();
            let elapsed = last_frame.elapsed();
            last_frame = Instant::now();

            // ── Input ────────────────────────────────────────────────────────
            self.poll_events();

            // ── Fixed-step tic simulation ────────────────────────────────────
            let t0 = Instant::now();
            if self.turn_based_mode {
                self.tick_turn_based(app);
            } else {
                self.tic_accumulator += elapsed;
                self.drain_ready_tics(app);
            }
            self.last_tick_us = t0.elapsed().as_micros() as u64;

            // ── Render ───────────────────────────────────────────────────────
            let t1 = Instant::now();
            app.render(&mut fb);
            self.last_render_us = t1.elapsed().as_micros() as u64;

            // ── Cogmind frame (computed before blit so we can pass it) ───────
            let cogmind_frame = if self.renderer_mode == RendererMode::Cogmind {
                let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
                let game_h = rows.saturating_sub(1);
                app.render_cogmind(cols, game_h)
            } else {
                None
            };

            // ── Blit (non-blocking dispatch to blit thread) ──────────────────
            let active_palette = app.active_palette();
            let cogmind_hud = if self.renderer_mode == RendererMode::Cogmind {
                app.cogmind_hud()
            } else {
                None
            };
            self.blit(&fb, lut, active_palette, cogmind_frame, cogmind_hud);

            // Report the blit thread's timing from the previous frame (1 frame stale).
            let blit_us = self.blit_elapsed_us.load(Ordering::Relaxed);
            app.on_frame_timings(self.last_tick_us, self.last_render_us, blit_us);

            // ── Frame pacing: sleep to fill remaining budget ──────────────────
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
                    self.input.sync_modifiers(
                        Some(key.modifiers.contains(KeyModifiers::SHIFT)),
                        Some(key.modifiers.contains(KeyModifiers::CONTROL)),
                    );

                    match key.kind {
                        KeyEventKind::Press => {
                            if key.code == KeyCode::Char('q') {
                                self.is_running = false;
                            }
                            if key.code == KeyCode::Esc {
                                self.input.push_escape();
                            }
                            if key.code == KeyCode::F(2) {
                                self.cycle_renderer_mode();
                            } else if key.code == KeyCode::F(5) {
                                self.input.push_f5();
                            } else if key.code == KeyCode::F(9) {
                                self.input.push_f9();
                            } else if key.code == KeyCode::Tab {
                                self.input.push_tab();
                            }
                            if key.code == KeyCode::Up {
                                self.input.push_menu_up();
                            } else if key.code == KeyCode::Down {
                                self.input.push_menu_down();
                            } else if key.code == KeyCode::Enter {
                                self.input.push_menu_select();
                            }
                            if let KeyCode::Char(ch) = key.code {
                                self.input.push_console_char(ch);
                                if ch == '.' {
                                    self.input.push_wait();
                                }
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
                    self.input.clear();
                }
                _ => {}
            }
        }
    }

    /// Prepare a `BlitFrame` and try to send it to the blit thread.
    ///
    /// For sixel: encodes the frame (or reuses the cached string) on this thread,
    /// then `try_send`s to the blit thread.  If the channel is full (blit still
    /// writing the previous frame), the new frame is silently dropped.
    ///
    /// For all other paths: clones the framebuffer (64 KB) into the payload.
    fn blit(
        &mut self,
        fb: &Framebuffer,
        lut: &PaletteLut,
        active_palette: usize,
        cogmind_frame: Option<CogmindFrame>,
        cogmind_hud: Option<CogmindHud>,
    ) {
        // ── FPS counter ──────────────────────────────────────────────────────
        self.frame_count += 1;
        self.frames_since_update += 1;
        let now = Instant::now();
        let since_update = now.duration_since(self.last_fps_update);
        if since_update.as_secs_f64() >= 1.0 {
            self.fps = self.frames_since_update as f64 / since_update.as_secs_f64();
            self.frames_since_update = 0;
            self.last_fps_update = now;
        }

        let Some(ref tx) = self.blit_tx else {
            return;
        };

        let font_size = self.picker.font_size();

        // Resolve the effective mode: if the user picked a graphics protocol the
        // terminal doesn't support, silently fall back to halfblocks.
        let effective = self.effective_renderer_mode();

        // ── Build payload ────────────────────────────────────────────────────
        let payload = match effective {
            RendererMode::Sixel => {
                // Sixel: pre-encode (or reuse cache) on main thread.
                let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
                let game_h = rows.saturating_sub(1);
                let (fw, fh) = font_size;
                let dst_w = cols as usize * fw as usize;
                let dst_h = game_h as usize * fh as usize;
                let term_key = (cols, rows);

                if dst_w == 0 || dst_h == 0 {
                    return;
                }

                let fb_data = fb.as_slice();
                let is_dirty = self.sixel_prev_term_size != term_key
                    || active_palette != self.sixel_prev_palette
                    || fb_data != self.sixel_prev_fb.as_slice();

                let sixel_str = if is_dirty {
                    let encoded = encode_doom_sixel(
                        fb_data,
                        lut,
                        active_palette,
                        Framebuffer::width(),
                        Framebuffer::height(),
                        dst_w,
                        dst_h,
                        cols,
                    );
                    self.sixel_prev_fb.clear();
                    self.sixel_prev_fb.extend_from_slice(fb_data);
                    self.sixel_prev_palette = active_palette;
                    self.sixel_prev_term_size = term_key;
                    self.sixel_cache = encoded.clone();
                    encoded
                } else {
                    self.sixel_cache.clone()
                };

                BlitPayload::Sixel(sixel_str)
            }
            RendererMode::Kitty | RendererMode::Iterm2 => {
                // Kitty / iTerm2: convert palette-indexed → RGB on main thread.
                let rgb_data: Vec<u8> = fb
                    .as_slice()
                    .iter()
                    .flat_map(|&idx| {
                        let rgb = lut.get(active_palette, idx);
                        [rgb.r, rgb.g, rgb.b]
                    })
                    .collect();
                match RgbImage::from_raw(
                    Framebuffer::width() as u32,
                    Framebuffer::height() as u32,
                    rgb_data,
                ) {
                    Some(img) => BlitPayload::ImageProtocol {
                        image: DynamicImage::ImageRgb8(img),
                    },
                    None => return,
                }
            }
            RendererMode::Cogmind => {
                // Cogmind mode: use the pre-rendered frame if available,
                // otherwise fall back to halfblocks.
                if let Some(frame) = cogmind_frame {
                    BlitPayload::Cogmind(frame)
                } else {
                    BlitPayload::Halfblocks {
                        fb: fb.clone(),
                        active_palette,
                        scaling_mode: self.scaling_mode,
                        char_set: None,
                    }
                }
            }
            RendererMode::Halfblocks | RendererMode::CharMap(_) => {
                let cs = match effective {
                    RendererMode::CharMap(cs) => Some(cs),
                    _ => None,
                };
                BlitPayload::Halfblocks {
                    fb: fb.clone(),
                    active_palette,
                    scaling_mode: self.scaling_mode,
                    char_set: cs,
                }
            }
        };

        // ── Status bar text ──────────────────────────────────────────────────
        let fps = self.fps;
        let tick_ms = self.last_tick_us as f64 / 1000.0;
        let render_ms = self.last_render_us as f64 / 1000.0;
        let blit_ms = self.blit_elapsed_us.load(Ordering::Relaxed) as f64 / 1000.0;
        let mode_name = effective.name();
        let status = format!(
            " DOOM | {fps:.0}fps | tick:{tick_ms:.1} rnd:{render_ms:.1} blit:{blit_ms:.1}ms | {mode_name} | [F2] cycle | [Q] "
        );

        // ── Dispatch (non-blocking) ──────────────────────────────────────────
        // `try_send` returns Err if the channel is full (blit thread busy) or
        // disconnected (blit thread has exited).  Both cases are safe to ignore:
        // the frame is simply dropped and the next one will be sent instead.
        let _ = tx.try_send(BlitFrame {
            payload,
            status,
            cogmind_hud,
        });
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Drop
// ─────────────────────────────────────────────────────────────────────────────

impl Drop for DoomEventLoop {
    fn drop(&mut self) {
        // Case 1: run() was never called — terminal is still in pending_terminal.
        if let Some(mut terminal) = self.pending_terminal.take() {
            if self.keyboard_enhancement_active {
                let _ = execute!(terminal.backend_mut(), PopKeyboardEnhancementFlags);
            }
            let _ = disable_raw_mode();
            let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
            let _ = terminal.show_cursor();
            return;
        }

        // Case 2: blit thread is running.  Drop the sender so the blit thread
        // sees RecvError, runs its cleanup (restore terminal), and exits.
        drop(self.blit_tx.take());
        if let Some(handle) = self.blit_thread.take() {
            let _ = handle.join();
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    static MODIFIER_COUNT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

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
            pending_terminal: Some(terminal),
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
            renderer_mode: RendererMode::Halfblocks,
            char_set: None,
            keyboard_enhancement_active: false,
            last_tick_us: 0,
            last_render_us: 0,
            sixel_cache: String::new(),
            sixel_prev_fb: Vec::new(),
            sixel_prev_palette: usize::MAX,
            sixel_prev_term_size: (0, 0),
            blit_tx: None,
            blit_thread: None,
            blit_elapsed_us: Arc::new(AtomicU64::new(0)),
            turn_based_mode: false,
            turn_recovery_tics: 0,
            turn_waiting_for_release: false,
        }
    }

    #[test]
    fn tic_duration_is_approx_28ms() {
        let expected_ns = 1_000_000_000u64 / 35;
        assert_eq!(TIC_DURATION.as_nanos() as u64, expected_ns);
    }

    #[test]
    fn tic_accumulator_arithmetic_drains_correctly() {
        let mut acc = Duration::ZERO;
        let elapsed = TIC_DURATION * 3 + Duration::from_millis(5);
        acc += elapsed;

        let mut tic_count = 0u32;
        while acc >= TIC_DURATION {
            tic_count += 1;
            acc -= TIC_DURATION;
        }
        assert_eq!(tic_count, 3);
        assert!(acc < TIC_DURATION);
        assert!(acc >= Duration::from_millis(5));
    }

    #[test]
    fn tic_rate_constant_is_35() {
        assert_eq!(TIC_RATE_HZ, 35);
    }

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
        let mut loop_ = make_test_event_loop();
        let _guard = MODIFIER_COUNT_LOCK.lock().unwrap();
        reset_modifier_sample_count();

        loop_.poll_events();

        let count_before = modifier_sample_count();
        loop_.poll_events();
        assert_eq!(modifier_sample_count(), count_before);
    }

    #[test]
    fn drain_ready_tics_samples_modifiers_once_per_tic() {
        let mut loop_ = make_test_event_loop();
        let _guard = MODIFIER_COUNT_LOCK.lock().unwrap();
        reset_modifier_sample_count();

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

    #[test]
    fn turn_based_wait_generates_recovery_tics() {
        let mut loop_ = make_test_event_loop();
        let _guard = MODIFIER_COUNT_LOCK.lock().unwrap();
        loop_.set_turn_based_mode(true);
        loop_.input.push_wait();
        let mut app = CountingApp { ticks: 0 };

        loop_.tick_turn_based(&mut app);
        assert_eq!(app.ticks, 1);
        assert_eq!(loop_.turn_recovery_tics, 5);

        loop_.tick_turn_based(&mut app);
        assert_eq!(app.ticks, 2);
        assert_eq!(loop_.turn_recovery_tics, 4);
    }

    #[test]
    fn turn_based_held_action_waits_for_release() {
        let mut loop_ = make_test_event_loop();
        let _guard = MODIFIER_COUNT_LOCK.lock().unwrap();
        loop_.set_turn_based_mode(true);
        loop_.input.key_down(KeyCode::Char('w'));
        let mut app = CountingApp { ticks: 0 };

        loop_.tick_turn_based(&mut app);
        assert_eq!(app.ticks, 1);
        loop_.turn_recovery_tics = 0;
        loop_.tick_turn_based(&mut app);
        assert_eq!(app.ticks, 1);

        loop_.input.key_up(KeyCode::Char('w'));
        loop_.tick_turn_based(&mut app);
        assert_eq!(app.ticks, 1);
        assert!(!loop_.turn_waiting_for_release);
    }
}
