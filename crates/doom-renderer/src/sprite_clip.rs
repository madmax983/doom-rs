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
    /// Spawns a fresh `SpriteClipHistory` array on the stack.
    ///
    /// By allocating exactly 8 steps on the stack instead of using a `Vec`,
    /// the software renderer avoids thrashing the global allocator during the
    /// intense multi-pass sprite clipping phase. If a column somehow manages
    /// to clip behind more than 8 distinct portals, the extra clips are silently dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
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

    /// Records a new visual clip boundary.
    ///
    /// During BSP traversal, as we trace a vertical column front-to-back, every
    /// upper/lower portal edge we cross defines a clip bound for sprites drawn
    /// later. We push that bound onto this history. If we hit the hardcoded limit
    /// (8), we drop the clip to prioritize stable performance over perfect visual
    /// accuracy in extreme edge cases.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep { depth: 100.0, row: 10, silhouette_height: 5.0 });
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

    /// Retrieves the most distant clip boundary recorded so far.
    ///
    /// Because the BSP tree traverses front-to-back, the last clip step pushed
    /// represents the furthest known solid occlusion bound for this specific screen column.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep { depth: 50.0, row: 1, silhouette_height: 0.0 });
    /// assert_eq!(history.last().unwrap().depth, 50.0);
    /// ```
    pub fn last(&self) -> Option<&SpriteClipStep> {
        if self.len > 0 {
            Some(&self.steps[self.len - 1])
        } else {
            None
        }
    }

    /// Iterates through the stored clip steps, from nearest to furthest.
    ///
    /// When drawing a sprite column at depth `D`, the renderer loops through these
    /// steps to see if the sprite falls behind any solid geometry closer than `D`.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// let history = SpriteClipHistory::new();
    /// for step in history.iter() {
    ///     println!("Clip step: {:?}", step);
    /// }
    /// ```
    pub fn iter(&self) -> core::slice::Iter<'_, SpriteClipStep> {
        self.steps[..self.len].iter()
    }
}
