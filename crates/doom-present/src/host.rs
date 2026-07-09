//! Windowed Doom host: an `abrash` [`WindowApp`] that drives a [`DoomApp`] at a
//! fixed 35 Hz, expands its paletted framebuffer to ARGB, integer-scales it to
//! 4:3, and presents it through `abrash`'s software presenter.
//!
//! This module is the one piece that cannot be unit-tested headlessly (it needs
//! a real window/event loop), so it is kept deliberately thin — all the logic
//! worth testing lives in [`crate::scale`], [`crate::input_adapter`], and
//! [`crate::tics`], plus [`PaletteLut::expand_argb`](doom_renderer::PaletteLut::expand_argb).

use std::error::Error;
use std::fmt;

use abrash::framebuffer::Framebuffer as AbrashFramebuffer;
use abrash::platform::{
    SoftwarePresenter, WindowApp, WindowContext, WindowHostConfig,
    run_windowed as abrash_run_windowed,
};
use crossterm::event::KeyCode;
use doom_renderer::{Framebuffer, PaletteLut};
use doom_tui::{DoomApp, InputState};
use winit::event::{ElementState, WindowEvent};

use crate::input_adapter::{apply_press, apply_release, winit_key_to_crossterm};
use crate::scale::{SRC_W, STRETCH_H, fit_scale, scale_nearest_4_3};
use crate::tics::TicAccumulator;

/// Default integer scale (S=3): a 960×720 window showing 320×200 → 4:3.
const DEFAULT_SCALE: usize = 3;
const DEFAULT_WINDOW_W: u32 = 960; // 320 * 3
const DEFAULT_WINDOW_H: u32 = 720; // 240 * 3

/// Error type for the windowed presenter.
#[derive(Debug)]
pub struct PresentError(pub String);

impl fmt::Display for PresentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "windowed presenter error: {}", self.0)
    }
}

impl Error for PresentError {}

/// An `abrash` window application that hosts a doom-rs [`DoomApp`].
///
/// Generic over the app type so `doom-app` can pass its own `DoomGame` without
/// this crate depending on the binary.
pub struct DoomWindowApp<A: DoomApp> {
    app: A,
    lut: PaletteLut,
    fb: Framebuffer,
    input: InputState,
    accumulator: TicAccumulator,
    presenter: Option<SoftwarePresenter>,
    /// Current integer scale factor, recomputed on resize from the window size.
    scale: usize,
    title: String,
}

impl<A: DoomApp> DoomWindowApp<A> {
    /// Construct a host wrapping `app`, using `palette` for index→ARGB expansion.
    #[must_use]
    pub fn new(app: A, palette: PaletteLut) -> Self {
        Self {
            app,
            lut: palette,
            fb: Framebuffer::new(),
            input: InputState::new(),
            accumulator: TicAccumulator::new(),
            presenter: None,
            scale: DEFAULT_SCALE,
            title: "doom-rs".to_string(),
        }
    }
}

impl<A: DoomApp> WindowApp for DoomWindowApp<A> {
    type Error = PresentError;

    fn config(&self) -> WindowHostConfig {
        WindowHostConfig {
            title: self.title.clone(),
            width: DEFAULT_WINDOW_W,
            height: DEFAULT_WINDOW_H,
            vsync: true,
        }
    }

    fn init(&mut self, ctx: WindowContext<'_>) -> Result<(), Self::Error> {
        let presenter =
            SoftwarePresenter::new(ctx.window.clone()).map_err(|e| PresentError(e.to_string()))?;
        self.presenter = Some(presenter);
        let size = ctx.window.inner_size();
        self.scale = fit_scale(size.width, size.height);
        Ok(())
    }

    fn resize(
        &mut self,
        _ctx: WindowContext<'_>,
        width: u32,
        height: u32,
    ) -> Result<(), Self::Error> {
        self.scale = fit_scale(width, height);
        Ok(())
    }

    fn input(&mut self, ctx: WindowContext<'_>, event: &WindowEvent) -> Result<(), Self::Error> {
        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                let Some(code) = winit_key_to_crossterm(&event.logical_key) else {
                    return Ok(());
                };
                match event.state {
                    // Ignore auto-repeat presses to mirror the terminal loop,
                    // which only reacts to crossterm Press/Release (not Repeat).
                    ElementState::Pressed if !event.repeat => {
                        // 'q' quits, like the terminal loop's Q handler.
                        if code == KeyCode::Char('q') {
                            ctx.event_loop.exit();
                        }
                        apply_press(&mut self.input, code);
                    }
                    ElementState::Released => apply_release(&mut self.input, code),
                    ElementState::Pressed => {}
                }
            }
            WindowEvent::ModifiersChanged(mods) => {
                let state = mods.state();
                self.input
                    .sync_modifiers(Some(state.shift_key()), Some(state.control_key()));
            }
            // Drop held keys when focus is lost (matches terminal FocusLost).
            WindowEvent::Focused(false) => self.input.clear(),
            _ => {}
        }
        Ok(())
    }

    fn update(&mut self, ctx: WindowContext<'_>) -> Result<(), Self::Error> {
        let tics = self.accumulator.advance_secs(ctx.dt_seconds);
        for _ in 0..tics {
            let tic_input = self.input.to_tic_input();
            self.app.tick(tic_input);
        }
        Ok(())
    }

    fn render(&mut self, _ctx: WindowContext<'_>) -> Result<(), Self::Error> {
        // 1. Game renders its 320×200 paletted frame.
        self.app.render(&mut self.fb);
        // 2. Canonical palette → 0xFFRRGGBB expansion (shared with doom-renderer).
        let argb = self.lut.expand_argb(&self.fb, self.app.active_palette());
        // 3. Nearest-neighbour integer scale to 4:3 (320·S × 240·S).
        let scaled = scale_nearest_4_3(&argb, self.scale);
        let out_w = (SRC_W * self.scale) as u32;
        let out_h = (STRETCH_H * self.scale) as u32;
        // 4. Wrap in an abrash framebuffer and present. abrash's present() resizes
        //    the surface to these dimensions and copies 1:1 (no scaling), which is
        //    why the integer scaling above is our responsibility.
        let mut ab =
            AbrashFramebuffer::new(out_w, out_h).map_err(|e| PresentError(e.to_string()))?;
        ab.as_mut_slice().copy_from_slice(&scaled);
        if let Some(presenter) = self.presenter.as_mut() {
            presenter
                .present(&ab)
                .map_err(|e| PresentError(e.to_string()))?;
        }
        Ok(())
    }
}

/// Open a native window and run `app` inside it, presenting Doom's 320×200
/// framebuffer integer-scaled to 4:3.
///
/// `palette` is the same [`PaletteLut`] the terminal path uses for RGB
/// conversion. This blocks until the window is closed. On an unrecoverable
/// window/host error, `abrash` prints a formatted error and terminates the
/// process (matching its `run_windowed` contract).
///
/// # Errors
///
/// Currently always returns `Ok(())` after the event loop exits normally; the
/// `Result` is kept so future non-fatal setup failures can be surfaced.
pub fn run_windowed<A: DoomApp>(app: A, palette: PaletteLut) -> Result<(), PresentError> {
    let host = DoomWindowApp::new(app, palette);
    abrash_run_windowed(host);
    Ok(())
}
