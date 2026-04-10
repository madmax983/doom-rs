//! Visplane allocator and span extraction.
//!
//! Doom groups floor/ceiling fragments that share `(height, pic, light)` into
//! visplanes. Each visplane stores per-column top/bottom bounds and can be
//! converted into horizontal draw spans for `R_DrawSpan`-style rasterization.

const SCREEN_W: usize = 320;
const UNUSED: i16 = -1;

/// Distinguishes ceiling and floor visplanes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlaneKind {
    Ceiling,
    Floor,
}

/// One allocated visplane.
#[derive(Debug, Clone)]
pub struct Visplane {
    pub kind: PlaneKind,
    pub height: i32,
    pub flat_name: [u8; 8],
    pub light_level: u8,
    pub min_x: usize,
    pub max_x: usize,
    top: [i16; SCREEN_W],
    bottom: [i16; SCREEN_W],
}

impl Visplane {
    /// Forges a new, empty visplane with a unique material identity.
    ///
    /// A `Visplane` represents a collection of floor or ceiling fragments that share
    /// the exact same height, texture (`flat_name`), and lighting. By grouping them,
    /// Doom can render the floor of a complex, non-convex room as a single continuous
    /// surface spanning multiple columns, dramatically improving performance.
    ///
    /// ## Examples
    /// ```
    /// use doom_renderer::visplane::{Visplane, PlaneKind};
    ///
    /// let floor = Visplane::new(PlaneKind::Floor, 0, *b"FLAT1\0\0\0", 255);
    /// assert!(floor.is_empty());
    /// ```
    #[must_use]
    pub fn new(kind: PlaneKind, height: i32, flat_name: [u8; 8], light_level: u8) -> Self {
        Self {
            kind,
            height,
            flat_name,
            light_level,
            min_x: SCREEN_W,
            max_x: 0,
            top: [UNUSED; SCREEN_W],
            bottom: [UNUSED; SCREEN_W],
        }
    }

    /// Checks if this visplane spans across the given screen column.
    ///
    /// Because a visplane merges geometry from a single sector, it might not
    /// cover the entire horizontal screen. It only exists in columns where a
    /// segment of that sector was actually drawn.
    ///
    /// ## Examples
    /// ```
    /// use doom_renderer::visplane::{VisplaneSet, PlaneKind};
    ///
    /// let mut set = VisplaneSet::new();
    /// let idx = set.r_find_plane(PlaneKind::Floor, 0, *b"FLAT1\0\0\0", 255);
    /// // Record a visible floor segment from column 10 to 20.
    /// set.r_check_plane(idx, 10, 20, 100, 120);
    ///
    /// let plane = &set.planes()[0];
    /// assert!(plane.has_column(15));
    /// assert!(!plane.has_column(5));
    /// ```
    #[must_use]
    pub fn has_column(&self, x: usize) -> bool {
        x < SCREEN_W && self.top[x] != UNUSED
    }

    /// Retrieves the vertical window `(top_y, bottom_y)` this plane occupies in column `x`.
    ///
    /// As the renderer traverses the BSP tree, it projects the sector's ceiling
    /// and floor heights into screen space. These vertical bounds dictate where
    /// the textured spans will ultimately be drawn.
    ///
    /// ## Examples
    /// ```
    /// use doom_renderer::visplane::{VisplaneSet, PlaneKind};
    ///
    /// let mut set = VisplaneSet::new();
    /// let idx = set.r_find_plane(PlaneKind::Floor, 0, *b"FLAT1\0\0\0", 255);
    /// set.r_check_plane(idx, 160, 160, 100, 150);
    ///
    /// let plane = &set.planes()[0];
    /// assert_eq!(plane.column_bounds(160), Some((100, 150)));
    /// assert_eq!(plane.column_bounds(0), None);
    /// ```
    #[must_use]
    pub fn column_bounds(&self, x: usize) -> Option<(i16, i16)> {
        if !self.has_column(x) {
            return None;
        }
        Some((self.top[x], self.bottom[x]))
    }

    fn set_column(&mut self, x: usize, top: i16, bottom: i16) {
        if x >= SCREEN_W || top > bottom {
            return;
        }
        self.top[x] = top;
        self.bottom[x] = bottom;
        self.min_x = self.min_x.min(x);
        self.max_x = self.max_x.max(x);
    }

    /// Determines if this visplane was allocated but never actually drawn to.
    ///
    /// An empty visplane occurs when a plane is requested via `R_FindPlane`
    /// but is completely occluded by nearer geometry before `R_CheckPlane`
    /// can assign any column spans to it.
    ///
    /// ## Examples
    /// ```
    /// use doom_renderer::visplane::{Visplane, PlaneKind};
    ///
    /// let floor = Visplane::new(PlaneKind::Floor, 0, *b"FLAT1\0\0\0", 255);
    /// assert!(floor.is_empty());
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.min_x >= SCREEN_W
    }
}

/// Horizontal run on a single screen row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpanRun {
    pub y: usize,
    pub x1: usize,
    pub x2: usize,
}

/// Convert one visplane's per-column bounds into row-major spans.
#[must_use]
pub fn visplane_to_spans(plane: &Visplane, screen_h: usize) -> Vec<SpanRun> {
    let mut out = Vec::new();
    if plane.is_empty() || plane.min_x > plane.max_x {
        return out;
    }

    for y in 0..screen_h as i16 {
        let mut run_start: Option<usize> = None;
        for x in plane.min_x..=plane.max_x {
            let visible = if let Some((top, bottom)) = plane.column_bounds(x) {
                y >= top && y <= bottom
            } else {
                false
            };
            match (run_start, visible) {
                (None, true) => run_start = Some(x),
                (Some(start), false) => {
                    out.push(SpanRun {
                        y: y as usize,
                        x1: start,
                        x2: x - 1,
                    });
                    run_start = None;
                }
                _ => {}
            }
        }
        if let Some(start) = run_start {
            out.push(SpanRun {
                y: y as usize,
                x1: start,
                x2: plane.max_x,
            });
        }
    }

    out
}

/// Collection of visplanes with Doom-style allocator helpers.
#[derive(Debug, Default)]
pub struct VisplaneSet {
    planes: Vec<Visplane>,
}

impl VisplaneSet {
    /// Initializes a fresh allocator for the current frame's visplanes.
    ///
    /// Doom builds a completely new set of visplanes every single frame.
    /// As the BSP is traversed front-to-back, this set dynamically grows
    /// to merge adjacent sector fragments or split them when occlusion
    /// breaks their continuity.
    ///
    /// ## Examples
    /// ```
    /// use doom_renderer::visplane::VisplaneSet;
    ///
    /// let set = VisplaneSet::new();
    /// assert_eq!(set.planes().len(), 0);
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Exposes the final roster of visplanes ready to be rasterized.
    ///
    /// After the wall pass completes, the renderer iterates over this slice,
    /// converting the vertical column bounds of each visplane into horizontal
    /// row spans (`SpanRun`) for perspective-correct texture mapping.
    ///
    /// ## Examples
    /// ```
    /// use doom_renderer::visplane::{VisplaneSet, PlaneKind};
    ///
    /// let mut set = VisplaneSet::new();
    /// set.r_find_plane(PlaneKind::Ceiling, 128, *b"CEIL1\0\0\0", 255);
    ///
    /// assert_eq!(set.planes().len(), 1);
    /// assert_eq!(set.planes()[0].kind, PlaneKind::Ceiling);
    /// ```
    #[must_use]
    pub fn planes(&self) -> &[Visplane] {
        &self.planes
    }

    /// Doom-style `R_FindPlane`: find or allocate by key.
    pub fn r_find_plane(
        &mut self,
        kind: PlaneKind,
        height: i32,
        flat_name: [u8; 8],
        light_level: u8,
    ) -> usize {
        if let Some((idx, _)) = self.planes.iter().enumerate().find(|(_, p)| {
            p.kind == kind
                && p.height == height
                && p.flat_name == flat_name
                && p.light_level == light_level
        }) {
            return idx;
        }
        self.planes
            .push(Visplane::new(kind, height, flat_name, light_level));
        self.planes.len() - 1
    }

    /// Doom-style `R_CheckPlane`: attach column range or split into new plane
    /// when that range conflicts with existing occupied columns.
    pub fn r_check_plane(
        &mut self,
        plane_idx: usize,
        start_x: usize,
        end_x: usize,
        top: i16,
        bottom: i16,
    ) -> usize {
        if plane_idx >= self.planes.len() || start_x > end_x || top > bottom {
            return plane_idx;
        }

        let x1 = start_x.min(SCREEN_W - 1);
        let x2 = end_x.min(SCREEN_W - 1);
        if x1 > x2 {
            return plane_idx;
        }

        let reuse_existing = {
            let p = &self.planes[plane_idx];
            let union_start = p.min_x.min(x1);
            let union_end = p.max_x.max(x2);
            let overlap_start = p.min_x.max(x1);
            let overlap_end = p.max_x.min(x2);

            if p.is_empty() || overlap_start > overlap_end {
                let _ = (union_start, union_end);
                true
            } else {
                (overlap_start..=overlap_end).all(|x| !p.has_column(x))
            }
        };

        let target_idx = if reuse_existing {
            plane_idx
        } else {
            let key = {
                let p = &self.planes[plane_idx];
                (p.kind, p.height, p.flat_name, p.light_level)
            };
            self.planes.push(Visplane::new(key.0, key.1, key.2, key.3));
            self.planes.len() - 1
        };

        for x in x1..=x2 {
            self.planes[target_idx].set_column(x, top, bottom);
        }
        target_idx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLAT1: [u8; 8] = *b"FLAT1\0\0\0";

    #[test]
    fn find_plane_reuses_matching_key() {
        let mut set = VisplaneSet::new();
        let a = set.r_find_plane(PlaneKind::Floor, 0, FLAT1, 160);
        let b = set.r_find_plane(PlaneKind::Floor, 0, FLAT1, 160);
        assert_eq!(a, b);
        assert_eq!(set.planes().len(), 1);
    }

    #[test]
    fn check_plane_extends_without_conflict() {
        let mut set = VisplaneSet::new();
        let p = set.r_find_plane(PlaneKind::Floor, 0, FLAT1, 160);
        let p = set.r_check_plane(p, 10, 12, 100, 120);
        let p = set.r_check_plane(p, 13, 15, 100, 120);
        assert_eq!(p, 0);
        assert_eq!(set.planes().len(), 1);
    }

    #[test]
    fn check_plane_splits_on_overlap_conflict() {
        let mut set = VisplaneSet::new();
        let p0 = set.r_find_plane(PlaneKind::Floor, 0, FLAT1, 160);
        let p0 = set.r_check_plane(p0, 10, 12, 100, 120);
        let p1 = set.r_check_plane(p0, 11, 13, 100, 120);
        assert_ne!(p0, p1);
        assert_eq!(set.planes().len(), 2);
    }

    #[test]
    fn visplane_reuse_overlapping_empty_window_stays_on_same_plane() {
        let mut set = VisplaneSet::new();
        let p0 = set.r_find_plane(PlaneKind::Floor, 0, FLAT1, 160);
        let p0 = set.r_check_plane(p0, 10, 10, 100, 120);
        let p0 = set.r_check_plane(p0, 14, 14, 100, 120);
        let p1 = set.r_check_plane(p0, 11, 13, 100, 120);

        assert_eq!(p0, p1);
        assert_eq!(set.planes().len(), 1);
        for x in 10..=14 {
            assert!(
                set.planes()[0].has_column(x),
                "column {x} should live on the reused plane"
            );
        }
    }

    #[test]
    fn visplane_reuse_conflict_allocates_fresh_plane_even_with_matching_sibling() {
        let mut set = VisplaneSet::new();
        let p0 = set.r_find_plane(PlaneKind::Floor, 0, FLAT1, 160);
        let p0 = set.r_check_plane(p0, 10, 10, 100, 120);
        let p1 = set.r_check_plane(p0, 10, 10, 100, 120);
        let p1 = set.r_check_plane(p1, 20, 20, 100, 120);

        let p2 = set.r_check_plane(p0, 10, 10, 100, 120);

        assert_ne!(p2, p0);
        assert_ne!(p2, p1);
        assert_eq!(set.planes().len(), 3);
        assert!(set.planes()[p2].has_column(10));
    }

    #[test]
    fn visplane_to_spans_emits_expected_rows() {
        let mut p = Visplane::new(PlaneKind::Floor, 0, FLAT1, 160);
        // Two columns visible on rows 5..6.
        p.set_column(20, 5, 6);
        p.set_column(21, 5, 6);

        let spans = visplane_to_spans(&p, 20);
        assert!(spans.contains(&SpanRun {
            y: 5,
            x1: 20,
            x2: 21
        }));
        assert!(spans.contains(&SpanRun {
            y: 6,
            x1: 20,
            x2: 21
        }));
    }
}
