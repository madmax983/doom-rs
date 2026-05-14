use crate::render::SpriteClipStep;

/// A manual ArrayVec-like structure to avoid allocating Vecs on the heap for short sprite clip histories.
///
/// In Doom, a single column rarely clips through more than 4-8 portals. This structure provides
/// a fixed-size buffer to store these clips without requiring dynamic memory allocation.
///
/// # Examples
///
/// ```
/// use doom_renderer::sprite_clip::SpriteClipHistory;
/// use doom_renderer::render::SpriteClipStep;
///
/// let mut history = SpriteClipHistory::new();
/// history.push(SpriteClipStep {
///     depth: 10.0,
///     row: 20,
///     silhouette_height: 5.0,
/// });
///
/// assert_eq!(history.iter().count(), 1);
/// assert_eq!(history.last().unwrap().depth, 10.0);
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

    /// Appends a new `SpriteClipStep` to the history buffer.
    ///
    /// If the buffer is full (i.e., it already contains 8 steps), new additions
    /// are silently dropped to avoid runtime panics or heap allocations.
    pub fn push(&mut self, step: SpriteClipStep) {
        if self.len < self.steps.len() {
            self.steps[self.len] = step;
            self.len += 1;
        } else {
            // Drop extra clips to avoid heap allocations
        }
    }

    /// Returns a reference to the most recently added `SpriteClipStep`.
    ///
    /// Returns `None` if the history buffer is empty.
    pub fn last(&self) -> Option<&SpriteClipStep> {
        if self.len > 0 {
            Some(&self.steps[self.len - 1])
        } else {
            None
        }
    }

    /// Returns an iterator over the stored `SpriteClipStep`s.
    ///
    /// The iterator yields the steps in the order they were pushed.
    pub fn iter(&self) -> core::slice::Iter<'_, SpriteClipStep> {
        self.steps[..self.len].iter()
    }
}
