//! Sprite clipping arrays to avoid heap allocations during BSP traversal.
//!
//! # The Problem
//!
//! During the rendering of the BSP tree, solid walls and portals clip sprites
//! so that they don't draw over geometry that is closer to the viewer. When
//! an opening (like a window) obscures the top or bottom of a sprite, the
//! renderer records a `SpriteClipStep`.
//!
//! Normally, this would be stored in a `Vec`. However, allocating a heap vector
//! for every single column of a sprite inside the hot render loop would severely
//! degrade performance.
//!
//! # The Solution
//!
//! `SpriteClipHistory` acts as a fixed-capacity, stack-allocated array (similar
//! to `ArrayVec`). In the classic engine, it's exceptionally rare for a single
//! vertical strip of a sprite to be clipped by more than 8 distinct portals at
//! varying depths. If the limit is exceeded, further clip steps are safely ignored.
//!
//! # Usage
//!
//! This module is primarily internal to the renderer's sprite plotting loop,
//! but its types are exposed for the `Seg` and `Visplane` boundaries.

use crate::render::SpriteClipStep;

/// A manual ArrayVec-like structure to avoid allocating Vecs on the heap for short sprite clip histories.
/// In Doom, a single column rarely clips through more than 4-8 portals.
///
/// # Examples
///
/// ```
/// use doom_renderer::sprite_clip::SpriteClipHistory;
/// use doom_renderer::render::SpriteClipStep;
///
/// let mut history = SpriteClipHistory::new();
/// assert_eq!(history.last(), None);
///
/// history.push(SpriteClipStep { depth: 100.0, row: 50, silhouette_height: 32.0 });
/// assert_eq!(history.iter().count(), 1);
/// assert_eq!(history.last().unwrap().row, 50);
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
    /// Creates a new, empty `SpriteClipHistory` with zero recorded steps.
    ///
    /// This is `const fn` so it can be used to initialize large arrays of clips
    /// (e.g. for the entire screen width) at compile time.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// const HISTORY: SpriteClipHistory = SpriteClipHistory::new();
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

    /// Appends a new clipping step to the history.
    ///
    /// If the fixed capacity (8 steps) is already full, the incoming step
    /// is discarded to avoid heap allocations.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep { depth: 100.0, row: 50, silhouette_height: 32.0 });
    /// ```
    pub fn push(&mut self, step: SpriteClipStep) {
        if self.len < self.steps.len() {
            self.steps[self.len] = step;
            self.len += 1;
        } else {
            // Drop extra clips to avoid heap allocations
        }
    }

    /// Returns a reference to the most recently added clip step, or `None` if empty.
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
    /// history.push(SpriteClipStep { depth: 100.0, row: 50, silhouette_height: 32.0 });
    /// assert_eq!(history.last().unwrap().row, 50);
    /// ```
    pub fn last(&self) -> Option<&SpriteClipStep> {
        if self.len > 0 {
            Some(&self.steps[self.len - 1])
        } else {
            None
        }
    }

    /// Returns an iterator over the recorded clip steps.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep { depth: 100.0, row: 50, silhouette_height: 32.0 });
    /// history.push(SpriteClipStep { depth: 200.0, row: 100, silhouette_height: 64.0 });
    ///
    /// for step in history.iter() {
    ///     println!("Clip at depth {}", step.depth);
    /// }
    /// ```
    pub fn iter(&self) -> core::slice::Iter<'_, SpriteClipStep> {
        self.steps[..self.len].iter()
    }
}
