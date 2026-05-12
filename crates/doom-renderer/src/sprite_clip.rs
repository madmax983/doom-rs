use crate::render::SpriteClipStep;

/// A manual ArrayVec-like structure to avoid allocating Vecs on the heap for short sprite clip histories.
///
/// In Doom, a single column rarely clips through more than 4-8 portals.
/// This structure provides a fixed-size buffer that silently drops extra elements
/// when it exceeds its capacity, ensuring stable performance without dynamic allocation.
///
/// ## Examples
///
/// ```rust
/// use doom_renderer::sprite_clip::SpriteClipHistory;
/// use doom_renderer::render::SpriteClipStep;
///
/// let mut history = SpriteClipHistory::new();
/// let step = SpriteClipStep {
///     depth: 10.0,
///     row: 5,
///     silhouette_height: 20.0,
/// };
///
/// history.push(step);
///
/// assert_eq!(history.last(), Some(&step));
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
    /// ## Examples
    ///
    /// ```rust
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

    /// Pushes a new `SpriteClipStep` onto the history.
    ///
    /// If the history is already at its maximum capacity (8 elements),
    /// the new element is ignored to prevent heap allocations.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep { depth: 1.0, row: 0, silhouette_height: 0.0 });
    /// assert!(history.last().is_some());
    /// ```
    pub fn push(&mut self, step: SpriteClipStep) {
        if self.len < self.steps.len() {
            self.steps[self.len] = step;
            self.len += 1;
        } else {
            // Drop extra clips to avoid heap allocations
        }
    }

    /// Returns a reference to the last `SpriteClipStep` pushed, or `None` if the history is empty.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// assert_eq!(history.last(), None);
    ///
    /// let step = SpriteClipStep { depth: 5.0, row: 10, silhouette_height: 15.0 };
    /// history.push(step);
    /// assert_eq!(history.last(), Some(&step));
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
    /// ```rust
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep { depth: 1.0, row: 0, silhouette_height: 0.0 });
    /// history.push(SpriteClipStep { depth: 2.0, row: 1, silhouette_height: 0.0 });
    ///
    /// let mut iter = history.iter();
    /// assert_eq!(iter.next().unwrap().depth, 1.0);
    /// assert_eq!(iter.next().unwrap().depth, 2.0);
    /// assert!(iter.next().is_none());
    /// ```
    pub fn iter(&self) -> core::slice::Iter<'_, SpriteClipStep> {
        self.steps[..self.len].iter()
    }
}
