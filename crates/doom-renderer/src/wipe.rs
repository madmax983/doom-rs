//! Screen wipe effect — the classic Doom "melt" screen transition.
//!
//! When transitioning between screens (e.g. level change), the old screen
//! melts downward revealing the new screen underneath.  Each column falls
//! at a slightly different speed, creating the distinctive liquid-drip look.
//!
//! The wipe uses its own independent PRNG (`wipe_random`), separate from the
//! game's `P_Random`, matching vanilla Doom's `M_Random` usage for wipes.

use crate::framebuffer::Framebuffer;
use doom_types::limits::{FB_HEIGHT, FB_SIZE, FB_WIDTH};

/// Independent PRNG for screen wipe stagger pattern.
///
/// This mirrors vanilla Doom's `M_Random` — a simple linear congruential
/// generator that is independent of the game-state deterministic `P_Random`.
#[inline]
fn wipe_random(seed: &mut u32) -> u32 {
    *seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
    (*seed >> 16) & 0x7FFF
}

/// Screen wipe state for the classic Doom melt transition.
///
/// The wipe captures "before" and "after" screen snapshots, then animates
/// columns falling from top to bottom at staggered speeds.
pub struct ScreenWipe {
    /// Column offsets — each column melts at a different speed.
    /// Values represent how many rows the column has scrolled down.
    /// Negative values mean the column hasn't started falling yet.
    y_offsets: [i32; FB_WIDTH],
    /// The "before" screen snapshot (palette-indexed).
    start_screen: Box<[u8; FB_SIZE]>,
    /// The "after" screen snapshot (palette-indexed).
    end_screen: Box<[u8; FB_SIZE]>,
    /// Whether the wipe is complete.
    done: bool,
}

impl ScreenWipe {
    /// Create a new screen wipe transition.
    ///
    /// Captures before/after screens and initializes column offsets with a
    /// random stagger pattern.  Column 0 gets a random negative offset in
    /// `[-15, 0]`, and each subsequent column is the previous offset plus
    /// a random delta in `{-1, 0, 1}`, clamped to `[-15, 0]`.
    pub fn new(start: &[u8; FB_SIZE], end: &[u8; FB_SIZE]) -> Self {
        Self::with_seed(start, end, 1)
    }

    /// Create a new screen wipe with a specific PRNG seed (for determinism in tests).
    pub fn with_seed(start: &[u8; FB_SIZE], end: &[u8; FB_SIZE], seed: u32) -> Self {
        let mut rng_seed = seed;
        let mut y_offsets = [0i32; FB_WIDTH];

        // Column 0: random offset in [-15, 0].
        y_offsets[0] = -((wipe_random(&mut rng_seed) % 16) as i32);

        // Each subsequent column: previous + random delta in {-1, 0, 1}, clamped.
        for x in 1..FB_WIDTH {
            let delta = (wipe_random(&mut rng_seed) % 3) as i32 - 1;
            y_offsets[x] = (y_offsets[x - 1] + delta).clamp(-15, 0);
        }

        let mut start_screen = Box::new([0u8; FB_SIZE]);
        start_screen.copy_from_slice(start);
        let mut end_screen = Box::new([0u8; FB_SIZE]);
        end_screen.copy_from_slice(end);

        Self {
            y_offsets,
            start_screen,
            end_screen,
            done: false,
        }
    }

    /// Advance the wipe by one tic.
    ///
    /// Columns with negative offsets increment toward 0 (haven't started yet).
    /// Columns at 0 or above fall downward with increasing speed.
    /// Returns `true` when all columns have reached `FB_HEIGHT` (wipe complete).
    pub fn tick(&mut self) -> bool {
        if self.done {
            return true;
        }

        let mut all_done = true;

        for x in 0..FB_WIDTH {
            if self.y_offsets[x] < 0 {
                // Column hasn't started falling yet — advance toward 0.
                self.y_offsets[x] += 1;
                all_done = false;
            } else if (self.y_offsets[x] as usize) < FB_HEIGHT {
                // Column is actively falling.
                // Speed increases as the column falls further (minimum 1 pixel/tic).
                let speed = (self.y_offsets[x] / 4 + 1).max(1);
                self.y_offsets[x] += speed;
                if self.y_offsets[x] > FB_HEIGHT as i32 {
                    self.y_offsets[x] = FB_HEIGHT as i32;
                }
                all_done = false;
            }
            // else: column already at or past FB_HEIGHT, done.
        }

        self.done = all_done;
        all_done
    }

    /// Composite the wipe into the framebuffer.
    ///
    /// For each column `x`:
    /// - Rows `[0, y_offset)` show the end screen (newly revealed).
    /// - Rows `[y_offset, FB_HEIGHT)` show the start screen (shifted up).
    ///
    /// When `y_offset <= 0` the entire column shows the start screen.
    /// When `y_offset >= FB_HEIGHT` the entire column shows the end screen.
    pub fn render(&self, fb: &mut Framebuffer) {
        for x in 0..FB_WIDTH {
            let y_off = self.y_offsets[x];

            if y_off <= 0 {
                // Column hasn't started falling — show start screen entirely.
                for y in 0..FB_HEIGHT {
                    fb.data[y * FB_WIDTH + x] = self.start_screen[y * FB_WIDTH + x];
                }
            } else if y_off >= FB_HEIGHT as i32 {
                // Column fully revealed — show end screen entirely.
                for y in 0..FB_HEIGHT {
                    fb.data[y * FB_WIDTH + x] = self.end_screen[y * FB_WIDTH + x];
                }
            } else {
                let split = y_off as usize;

                // Top portion: end screen (newly revealed).
                for y in 0..split {
                    fb.data[y * FB_WIDTH + x] = self.end_screen[y * FB_WIDTH + x];
                }

                // Bottom portion: start screen shifted up by `split` rows.
                for y in split..FB_HEIGHT {
                    let src_y = y - split;
                    fb.data[y * FB_WIDTH + x] = self.start_screen[src_y * FB_WIDTH + x];
                }
            }
        }
    }

    /// Whether the wipe transition is complete.
    pub fn is_done(&self) -> bool {
        self.done
    }

    /// Read-only access to column offsets (for testing/debugging).
    pub fn y_offsets(&self) -> &[i32; FB_WIDTH] {
        &self.y_offsets
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a screen filled with a single value.
    fn filled_screen(val: u8) -> Box<[u8; FB_SIZE]> {
        let mut screen = Box::new([0u8; FB_SIZE]);
        screen.fill(val);
        screen
    }

    #[test]
    fn wipe_random_is_deterministic() {
        let mut seed_a = 42u32;
        let mut seed_b = 42u32;
        for _ in 0..100 {
            assert_eq!(wipe_random(&mut seed_a), wipe_random(&mut seed_b));
        }
    }

    #[test]
    fn wipe_random_produces_different_values() {
        let mut seed = 1u32;
        let first = wipe_random(&mut seed);
        let second = wipe_random(&mut seed);
        // Very unlikely to be equal with different states.
        assert_ne!(first, second, "sequential random values should differ");
    }

    #[test]
    fn new_initializes_y_offsets_with_stagger() {
        let start = filled_screen(0);
        let end = filled_screen(255);
        let wipe = ScreenWipe::with_seed(&start, &end, 42);

        // All initial offsets should be in [-15, 0].
        for (i, &off) in wipe.y_offsets().iter().enumerate() {
            assert!(
                (-15..=0).contains(&off),
                "y_offsets[{i}] = {off}, expected in [-15, 0]"
            );
        }
    }

    #[test]
    fn new_offsets_vary_between_columns() {
        let start = filled_screen(0);
        let end = filled_screen(255);
        let wipe = ScreenWipe::with_seed(&start, &end, 7);

        // Not all columns should have the same offset (statistically near-impossible).
        let first = wipe.y_offsets()[0];
        let any_different = wipe.y_offsets().iter().any(|&off| off != first);
        assert!(any_different, "columns should have varied stagger offsets");
    }

    #[test]
    fn same_seed_produces_same_pattern() {
        let start = filled_screen(0);
        let end = filled_screen(255);
        let wipe_a = ScreenWipe::with_seed(&start, &end, 123);
        let wipe_b = ScreenWipe::with_seed(&start, &end, 123);
        assert_eq!(wipe_a.y_offsets(), wipe_b.y_offsets());
    }

    #[test]
    fn different_seeds_produce_different_patterns() {
        let start = filled_screen(0);
        let end = filled_screen(255);
        let wipe_a = ScreenWipe::with_seed(&start, &end, 1);
        let wipe_b = ScreenWipe::with_seed(&start, &end, 999);
        assert_ne!(
            wipe_a.y_offsets(),
            wipe_b.y_offsets(),
            "different seeds should produce different patterns"
        );
    }

    #[test]
    fn tick_advances_columns() {
        let start = filled_screen(0);
        let end = filled_screen(255);
        let mut wipe = ScreenWipe::with_seed(&start, &end, 42);

        let offsets_before: Vec<i32> = wipe.y_offsets().to_vec();
        wipe.tick();
        let offsets_after: Vec<i32> = wipe.y_offsets().to_vec();

        // At least some columns should have advanced.
        let any_advanced = offsets_before
            .iter()
            .zip(offsets_after.iter())
            .any(|(before, after)| after > before);
        assert!(any_advanced, "tick should advance at least some columns");
    }

    #[test]
    fn is_done_starts_false() {
        let start = filled_screen(0);
        let end = filled_screen(255);
        let wipe = ScreenWipe::with_seed(&start, &end, 1);
        assert!(!wipe.is_done());
    }

    #[test]
    fn multiple_ticks_eventually_complete() {
        let start = filled_screen(0);
        let end = filled_screen(255);
        let mut wipe = ScreenWipe::with_seed(&start, &end, 1);

        // The wipe should complete within a reasonable number of tics.
        // With acceleration, even the slowest column finishes well under 300 tics.
        let mut completed = false;
        for _ in 0..500 {
            if wipe.tick() {
                completed = true;
                break;
            }
        }
        assert!(completed, "wipe should eventually complete");
        assert!(wipe.is_done());
    }

    #[test]
    fn tick_returns_true_when_done() {
        let start = filled_screen(0);
        let end = filled_screen(255);
        let mut wipe = ScreenWipe::with_seed(&start, &end, 1);

        let mut last_result = false;
        for _ in 0..500 {
            last_result = wipe.tick();
            if last_result {
                break;
            }
        }
        assert!(last_result, "tick should return true when complete");
    }

    #[test]
    fn tick_after_done_remains_done() {
        let start = filled_screen(0);
        let end = filled_screen(255);
        let mut wipe = ScreenWipe::with_seed(&start, &end, 1);

        // Run to completion.
        for _ in 0..500 {
            if wipe.tick() {
                break;
            }
        }
        assert!(wipe.is_done());

        // Additional ticks should still return true.
        assert!(wipe.tick());
        assert!(wipe.is_done());
    }

    #[test]
    fn render_shows_start_screen_initially() {
        let start = filled_screen(100);
        let end = filled_screen(200);
        // Use a seed that gives some columns offset -15 (max delay).
        let wipe = ScreenWipe::with_seed(&start, &end, 42);

        let mut fb = Framebuffer::new();
        wipe.render(&mut fb);

        // Columns with negative offsets should show the start screen.
        // Find a column with negative offset.
        let neg_col = wipe
            .y_offsets()
            .iter()
            .position(|&off| off < 0)
            .expect("should have at least one negative-offset column");

        for y in 0..FB_HEIGHT {
            let pixel = fb.get_pixel(neg_col, y).expect("value must exist in test");
            assert_eq!(
                pixel, 100,
                "before tick, column {neg_col} should show start_screen"
            );
        }
    }

    #[test]
    fn render_shows_end_screen_when_done() {
        let start = filled_screen(50);
        let end = filled_screen(250);
        let mut wipe = ScreenWipe::with_seed(&start, &end, 1);

        // Run to completion.
        for _ in 0..500 {
            if wipe.tick() {
                break;
            }
        }

        let mut fb = Framebuffer::new();
        wipe.render(&mut fb);

        // Every pixel should be from the end screen.
        for y in 0..FB_HEIGHT {
            for x in 0..FB_WIDTH {
                assert_eq!(
                    fb.get_pixel(x, y).expect("value must exist in test"),
                    250,
                    "after wipe complete, pixel ({x},{y}) should be end_screen value"
                );
            }
        }
    }

    #[test]
    fn render_composites_partially_wiped_state() {
        let start = filled_screen(10);
        let end = filled_screen(200);
        let mut wipe = ScreenWipe::with_seed(&start, &end, 1);

        // Tick a few times to get partial progress.
        for _ in 0..20 {
            wipe.tick();
        }

        let mut fb = Framebuffer::new();
        wipe.render(&mut fb);

        // Find a column that has partially fallen (0 < y_off < FB_HEIGHT).
        let partial_col = wipe
            .y_offsets()
            .iter()
            .position(|&off| off > 0 && (off as usize) < FB_HEIGHT);

        if let Some(x) = partial_col {
            let split = wipe.y_offsets()[x] as usize;

            // Above the split: end_screen.
            for y in 0..split {
                assert_eq!(
                    fb.get_pixel(x, y).expect("value must exist in test"),
                    200,
                    "above split at column {x}, row {y} should be end_screen"
                );
            }
            // Below the split: start_screen (shifted).
            for y in split..FB_HEIGHT {
                assert_eq!(
                    fb.get_pixel(x, y).expect("value must exist in test"),
                    10,
                    "below split at column {x}, row {y} should be start_screen"
                );
            }
        }
    }

    #[test]
    fn render_handles_zero_offset_column() {
        // A column with y_offset = 0 should show the start screen entirely.
        let start = filled_screen(30);
        let end = filled_screen(220);

        // Manually verify: when y_offset is 0, render should show start_screen.
        let mut wipe = ScreenWipe::with_seed(&start, &end, 1);

        // Force a column to offset 0 by ticking until some column reaches 0.
        // Instead, just check after a few ticks.
        for _ in 0..15 {
            wipe.tick();
        }

        let mut fb = Framebuffer::new();
        wipe.render(&mut fb);

        // Columns at offset <= 0 show start, columns at offset >= FB_HEIGHT show end.
        for x in 0..FB_WIDTH {
            let off = wipe.y_offsets()[x];
            if off <= 0 {
                assert_eq!(
                    fb.get_pixel(x, 0).expect("value must exist in test"),
                    30,
                    "column {x} at offset {off} should show start_screen"
                );
            }
        }
    }

    #[test]
    fn column_offsets_are_clamped_to_valid_range() {
        let start = filled_screen(0);
        let end = filled_screen(255);
        let wipe = ScreenWipe::with_seed(&start, &end, 1);

        for (i, &off) in wipe.y_offsets().iter().enumerate() {
            assert!(off >= -15, "y_offsets[{i}] = {off}, should be >= -15");
            assert!(off <= 0, "y_offsets[{i}] = {off}, should be <= 0 initially");
        }
    }

    #[test]
    fn adjacent_columns_differ_by_at_most_one_initially() {
        let start = filled_screen(0);
        let end = filled_screen(255);
        // The stagger pattern adds delta of {-1, 0, 1} between adjacent columns,
        // but then clamps, so the difference could be more if clamping occurs.
        // We just verify the pattern is smooth-ish.
        let wipe = ScreenWipe::with_seed(&start, &end, 42);
        let offsets = wipe.y_offsets();

        for x in 1..FB_WIDTH {
            let diff = (offsets[x] - offsets[x - 1]).abs();
            // Before clamping, delta is at most 1. After clamping to [-15, 0],
            // the difference should still be at most 1 in practice.
            assert!(
                diff <= 1,
                "adjacent column diff at x={x}: {} vs {} (diff={diff})",
                offsets[x - 1],
                offsets[x]
            );
        }
    }

    #[test]
    fn captures_screen_data_correctly() {
        let mut start = [0u8; FB_SIZE];
        let mut end = [0u8; FB_SIZE];

        // Set distinctive patterns.
        for (i, b) in start.iter_mut().enumerate() {
            *b = (i % 256) as u8;
        }
        for (i, b) in end.iter_mut().enumerate() {
            *b = (255 - (i % 256)) as u8;
        }

        let wipe = ScreenWipe::with_seed(&start, &end, 1);

        // Render before any ticks — all columns have negative offsets, so
        // the start_screen should appear.
        let mut fb = Framebuffer::new();
        wipe.render(&mut fb);

        // Check a few pixels match the start pattern.
        for y in 0..10 {
            for x in 0..10 {
                let idx = y * FB_WIDTH + x;
                let expected = (idx % 256) as u8;
                assert_eq!(
                    fb.get_pixel(x, y).expect("value must exist in test"),
                    expected,
                    "pixel ({x},{y}) should match start_screen pattern"
                );
            }
        }
    }
}
