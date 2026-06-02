//! A manual ArrayVec-like structure for tracking sprite clip boundaries during rendering.
//!
//! As the software renderer traverses sectors front-to-back, portal openings
//! (two-sided segs) act as windows that restrict where sprites behind them can be drawn.
//! The `SpriteClipHistory` stores a short list of these boundary steps, avoiding
//! heap allocations on the hot path since a single column rarely clips through
//! more than a few portals.

use crate::render::SpriteClipStep;

/// A manual ArrayVec-like structure to avoid allocating Vecs on the heap for short sprite clip histories.
/// In Doom, a single column rarely clips through more than 4-8 portals.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpriteClipHistory {
    steps: [SpriteClipStep; 8],
    len: usize,
}

impl Default for SpriteClipHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl SpriteClipHistory {
    /// Creates a new, empty `SpriteClipHistory`.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    ///
    /// let history = SpriteClipHistory::new();
    /// assert_eq!(history.iter().count(), 0);
    /// ```
    pub const fn new() -> Self {
        Self {
            steps: [SpriteClipStep {
                depth: 0.0,
                row: 0,
                silhouette_height: 0.0,
            }; 8],
            len: 0,
        }
    }

    /// Appends a new `SpriteClipStep` to the history.
    ///
    /// If the history is already full (contains 8 elements), new steps are silently dropped
    /// to avoid heap allocations. This is acceptable since drawing more than 8 portals
    /// deep is extremely rare and only causes minor visual glitches on distant sprites.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// let step = SpriteClipStep { depth: 100.0, row: 50, silhouette_height: 32.0 };
    /// history.push(step);
    ///
    /// assert_eq!(history.last().unwrap().row, 50);
    /// ```
    pub fn push(&mut self, step: SpriteClipStep) {
        if self.len < self.steps.len() {
            self.steps[self.len] = step;
            self.len += 1;
        } else {
            // Drop extra clips to avoid heap allocations
        }
    }

    /// Returns a reference to the most recently added `SpriteClipStep`, or `None` if empty.
    ///
    /// This is frequently used by the renderer to avoid pushing duplicate clip steps when
    /// the silhouette height hasn't changed.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    ///
    /// let history = SpriteClipHistory::new();
    /// assert!(history.last().is_none());
    /// ```
    pub fn last(&self) -> Option<&SpriteClipStep> {
        if self.len > 0 {
            Some(&self.steps[self.len - 1])
        } else {
            None
        }
    }

    /// Returns an iterator over the stored `SpriteClipStep`s.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep { depth: 100.0, row: 50, silhouette_height: 32.0 });
    /// history.push(SpriteClipStep { depth: 200.0, row: 60, silhouette_height: 64.0 });
    ///
    /// let mut iter = history.iter();
    /// assert_eq!(iter.next().unwrap().depth, 100.0);
    /// assert_eq!(iter.next().unwrap().depth, 200.0);
    /// assert!(iter.next().is_none());
    /// ```
    pub fn iter(&self) -> core::slice::Iter<'_, SpriteClipStep> {
        self.steps[..self.len].iter()
    }
}
