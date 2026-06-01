//! Sprite clipping history for the renderer.
//!
//! This module provides a minimal, fixed-capacity structure for storing sprite depth clipping
//! history across rendering passes.

use crate::render::SpriteClipStep;

/// Manages the history of depth clipping for sprite rendering.
///
/// `SpriteClipHistory` is a manual ArrayVec-like structure designed to avoid heap allocations
/// for short sprite clip histories. In Doom's rendering engine, a single column rarely clips
/// through more than 4-8 portals (like doors or transparent walls), making a fixed-size stack
/// ideal for tracking depth values without the overhead of a `Vec`.
///
/// # Examples
///
/// ```
/// use doom_renderer::sprite_clip::SpriteClipHistory;
/// use doom_renderer::render::SpriteClipStep;
///
/// let mut history = SpriteClipHistory::new();
/// history.push(SpriteClipStep { depth: 100.0, row: 50, silhouette_height: 10.0 });
///
/// assert_eq!(history.iter().count(), 1);
/// assert_eq!(history.last().unwrap().depth, 100.0);
/// ```
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
    /// # Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// let history = SpriteClipHistory::new();
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

    /// Pushes a new `SpriteClipStep` onto the history stack.
    ///
    /// If the stack is full (maximum 8 items), subsequent pushes are silently ignored
    /// to avoid heap allocations. This is safe because deeply nested clips are visually
    /// insignificant and rare in Doom engine geometry.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep { depth: 50.0, row: 10, silhouette_height: 5.0 });
    /// ```
    pub fn push(&mut self, step: SpriteClipStep) {
        if self.len < self.steps.len() {
            self.steps[self.len] = step;
            self.len += 1;
        } else {
            // Drop extra clips to avoid heap allocations
        }
    }

    /// Returns a reference to the most recently pushed `SpriteClipStep`, or `None` if empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// assert!(history.last().is_none());
    ///
    /// history.push(SpriteClipStep { depth: 50.0, row: 10, silhouette_height: 5.0 });
    /// assert!(history.last().is_some());
    /// ```
    pub fn last(&self) -> Option<&SpriteClipStep> {
        if self.len > 0 {
            Some(&self.steps[self.len - 1])
        } else {
            None
        }
    }

    /// Returns an iterator over the clip steps currently in the history.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep { depth: 50.0, row: 10, silhouette_height: 5.0 });
    ///
    /// for step in history.iter() {
    ///     println!("Clip depth: {}", step.depth);
    /// }
    /// ```
    pub fn iter(&self) -> core::slice::Iter<'_, SpriteClipStep> {
        self.steps[..self.len].iter()
    }
}
