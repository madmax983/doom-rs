//! Wall/plane clipping helpers for the software renderer.
//!
//! This module provides:
//! - Near-plane clipping for projected seg endpoints.
//! - A simple per-column solid wall occlusion mask.
//! - Row-wise clipping of visplane spans against per-column ceiling/floor clips.

#[allow(dead_code)]
const SCREEN_W: usize = 320;

/// Clip a view-space seg to the near plane (`vx = 1`).
///
/// Returns `None` if both endpoints are behind the near plane after clipping.
/// Returns `Some((vx1, vy1, vx2, vy2))` with clipped coordinates otherwise.
#[must_use]
#[allow(dead_code)]
pub fn clip_seg_to_near_plane(
    vx1: i64,
    vy1: i64,
    vx2: i64,
    vy2: i64,
) -> Option<(i64, i64, i64, i64)> {
    const NEAR: i64 = 1;

    if vx1 <= 0 && vx2 <= 0 {
        return None;
    }
    if vx1 > 0 && vx2 > 0 {
        return Some((vx1, vy1, vx2, vy2));
    }

    let t_num = NEAR - vx1;
    let t_den = vx2 - vx1;
    if t_den == 0 {
        return None;
    }

    if vx1 <= 0 {
        let ny = vy1 + t_num * (vy2 - vy1) / t_den;
        Some((NEAR, ny, vx2, vy2))
    } else {
        let t_num2 = NEAR - vx2;
        let t_den2 = vx1 - vx2;
        if t_den2 == 0 {
            return None;
        }
        let ny = vy2 + t_num2 * (vy1 - vy2) / t_den2;
        Some((vx1, vy1, NEAR, ny))
    }
}

/// Clip a view-space seg against one half-plane defined by signed distances
/// `d1` / `d2` from the plane at each endpoint (inside when `d >= 0`).
///
/// Returns the (possibly shortened) segment, or `None` if fully outside.
#[inline]
fn clip_one_plane(
    vx1: i64,
    vy1: i64,
    vx2: i64,
    vy2: i64,
    d1: i64,
    d2: i64,
) -> Option<(i64, i64, i64, i64)> {
    if d1 >= 0 && d2 >= 0 {
        return Some((vx1, vy1, vx2, vy2));
    }
    if d1 < 0 && d2 < 0 {
        return None;
    }
    // One endpoint is outside; compute the intersection point.
    if d1 < 0 {
        // p1 is outside — clip p1 toward p2.
        let t_num = -d1;
        let t_den = d2 - d1; // positive
        let nx = vx1 + t_num * (vx2 - vx1) / t_den;
        let ny = vy1 + t_num * (vy2 - vy1) / t_den;
        Some((nx, ny, vx2, vy2))
    } else {
        // p2 is outside — clip p2 toward p1.
        let t_num = d1;
        let t_den = d1 - d2; // positive
        let nx = vx1 + t_num * (vx2 - vx1) / t_den;
        let ny = vy1 + t_num * (vy2 - vy1) / t_den;
        Some((vx1, vy1, nx, ny))
    }
}

/// Clip a view-space seg to the full view frustum for a 90° FOV.
///
/// Applies three half-plane clips in sequence:
/// - **Near plane** `vx ≥ 1`: eliminates geometry behind the player.
/// - **Left frustum** `vx + vy ≥ 0`: corresponds to screen column 0.
/// - **Right frustum** `vx − vy ≥ 0`: corresponds to screen column 320.
///
/// After clipping, both endpoints project to screen columns within `[0, 320]`,
/// which prevents huge `span_w` values and the wall-texture warp that results
/// from them.  Returns `None` if the seg is entirely outside the frustum.
#[must_use]
pub fn clip_seg_to_view_frustum(
    vx1: i64,
    vy1: i64,
    vx2: i64,
    vy2: i64,
) -> Option<(i64, i64, i64, i64)> {
    // 1. Near plane: d = vx - 1
    let (vx1, vy1, vx2, vy2) = clip_one_plane(vx1, vy1, vx2, vy2, vx1 - 1, vx2 - 1)?;
    // 2. Left frustum: d = vx + vy  (inside when vy >= -vx)
    let (vx1, vy1, vx2, vy2) = clip_one_plane(vx1, vy1, vx2, vy2, vx1 + vy1, vx2 + vy2)?;
    // 3. Right frustum: d = vx - vy  (inside when vy <= vx)
    let (vx1, vy1, vx2, vy2) = clip_one_plane(vx1, vy1, vx2, vy2, vx1 - vy1, vx2 - vy2)?;
    Some((vx1, vy1, vx2, vy2))
}

/// Tracks which screen columns are already fully blocked by nearer one-sided walls.
///
/// This is a compact `solidsegs`-style approximation for a fixed 320-column
/// viewport: once a column is marked solid, farther seg columns in that screen
/// column are skipped.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SolidWallClipper {
    covered: [bool; SCREEN_W],
}

impl Default for SolidWallClipper {
    fn default() -> Self {
        Self {
            covered: [false; SCREEN_W],
        }
    }
}

#[allow(dead_code)]
impl SolidWallClipper {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mark_column(&mut self, x: usize) {
        if x < SCREEN_W {
            self.covered[x] = true;
        }
    }

    /// Return maximal uncovered runs within `[start_x, end_x]` (inclusive).
    #[must_use]
    pub fn uncovered_runs(&self, start_x: usize, end_x: usize) -> Vec<(usize, usize)> {
        if start_x >= SCREEN_W {
            return Vec::new();
        }
        let x1 = start_x.min(SCREEN_W - 1);
        let x2 = end_x.min(SCREEN_W - 1);
        if x1 > x2 {
            return Vec::new();
        }

        let mut out = Vec::new();
        let mut run_start: Option<usize> = None;
        for x in x1..=x2 {
            let open = !self.covered[x];
            match (run_start, open) {
                (None, true) => run_start = Some(x),
                (Some(start), false) => {
                    out.push((start, x - 1));
                    run_start = None;
                }
                _ => {}
            }
        }
        if let Some(start) = run_start {
            out.push((start, x2));
        }
        out
    }
}

/// Plane type used for row-wise span clipping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum PlaneClipKind {
    Ceiling,
    Floor,
}

/// Clip one row span `[x1, x2]` against per-column ceiling/floor clip bounds.
///
/// Returns visible sub-runs (inclusive ranges).
#[must_use]
#[allow(dead_code)]
pub fn clip_plane_span_runs(
    y: i32,
    x1: usize,
    x2: usize,
    kind: PlaneClipKind,
    ceilingclip: &[i32; SCREEN_W],
    floorclip: &[i32; SCREEN_W],
) -> Vec<(usize, usize)> {
    if x1 >= SCREEN_W {
        return Vec::new();
    }
    let sx1 = x1.min(SCREEN_W - 1);
    let sx2 = x2.min(SCREEN_W - 1);
    if sx1 > sx2 {
        return Vec::new();
    }

    let mut out = Vec::new();
    let mut run_start: Option<usize> = None;

    for x in sx1..=sx2 {
        let visible = match kind {
            PlaneClipKind::Ceiling => y <= ceilingclip[x],
            PlaneClipKind::Floor => y >= floorclip[x],
        };

        match (run_start, visible) {
            (None, true) => run_start = Some(x),
            (Some(start), false) => {
                out.push((start, x - 1));
                run_start = None;
            }
            _ => {}
        }
    }
    if let Some(start) = run_start {
        out.push((start, sx2));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn near_clip_both_behind_returns_none() {
        assert!(clip_seg_to_near_plane(-10, 0, -5, 0).is_none());
    }

    #[test]
    fn near_clip_both_in_front_unchanged() {
        let result = clip_seg_to_near_plane(5, 2, 10, 4);
        assert_eq!(result, Some((5, 2, 10, 4)));
    }

    #[test]
    fn near_clip_one_behind_clips() {
        let result = clip_seg_to_near_plane(-2, 0, 4, 6);
        assert!(result.is_some());
        let (vx1, _vy1, vx2, _vy2) = result.unwrap();
        assert!(vx1 >= 1 && vx2 >= 1);
    }

    #[test]
    fn solid_clipper_splits_uncovered_runs() {
        let mut clipper = SolidWallClipper::new();
        clipper.mark_column(11);
        let runs = clipper.uncovered_runs(10, 12);
        assert_eq!(runs, vec![(10, 10), (12, 12)]);
    }

    #[test]
    fn plane_span_clip_splits_on_blocked_column() {
        let mut ceilingclip = [199i32; SCREEN_W];
        let floorclip = [0i32; SCREEN_W];
        ceilingclip[11] = 40; // y=45 is blocked in this column for ceilings.

        let runs =
            clip_plane_span_runs(45, 10, 12, PlaneClipKind::Ceiling, &ceilingclip, &floorclip);
        assert_eq!(runs, vec![(10, 10), (12, 12)]);
    }
}
