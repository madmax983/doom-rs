//! Sight-line module: produces a 2–3 cell directional ray from the player
//! based on their facing angle, rendered in Phase 3 of the cogmind compositor.

use doom_types::Bam;

use super::glyphs::Rgb;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single cell in the player's sight-line ray.
pub struct SightCell {
    /// Terminal-space column offset from the player cell.
    pub dx: i32,
    /// Terminal-space row offset from the player cell (Y-flipped from Doom).
    pub dy: i32,
    /// Character to render at this cell.
    pub glyph: char,
    /// Foreground color.
    pub fg: Rgb,
}

// ---------------------------------------------------------------------------
// Colors
// ---------------------------------------------------------------------------

const CYAN_BRIGHT: Rgb = (0, 180, 220);
const CYAN_MID: Rgb = (0, 120, 160);
const CYAN_DIM: Rgb = (0, 60, 80);

// ---------------------------------------------------------------------------
// Octant table
// ---------------------------------------------------------------------------

/// `(dx, dy, glyph)` per octant in Doom space (Y+ = north).
const OCTANTS: [(i32, i32, char); 8] = [
    (1, 0, '\u{25B8}'),  // 0: East  ▸
    (1, 1, '\u{2571}'),  // 1: NE    ╱
    (0, 1, '\u{25B4}'),  // 2: North ▴
    (-1, 1, '\u{2572}'), // 3: NW    ╲
    (-1, 0, '\u{25C2}'), // 4: West  ◂
    (-1, -1, '\u{2571}'), // 5: SW   ╱
    (0, -1, '\u{25BE}'), // 6: South ▾
    (1, -1, '\u{2572}'), // 7: SE    ╲
];

// ---------------------------------------------------------------------------
// Functions
// ---------------------------------------------------------------------------

/// Convert a [`Bam`] angle to an octant index (0–7).
///
/// Adds half an octant (`ANG45 / 2 = 0x1000_0000`) so the octant boundaries
/// fall *between* cardinal/diagonal directions, then extracts the top 3 bits.
#[must_use]
pub fn angle_to_octant(angle: Bam) -> usize {
    let half_octant: u32 = 0x1000_0000; // ANG45 / 2
    let shifted = angle.raw().wrapping_add(half_octant);
    (shifted >> 29) as usize
}

/// Produce up to 3 sight-line cells stepping outward from the player.
///
/// * Cell 1 receives the directional arrow glyph and [`CYAN_BRIGHT`].
/// * Cells 2–3 receive `·` (`U+00B7`) and [`CYAN_MID`] / [`CYAN_DIM`].
///
/// `is_wall(dx, dy)` checks wall presence in **Doom space** offsets from the
/// player. The returned [`SightCell`]s have `dy` negated (terminal space:
/// row 0 at top, positive = down).
///
/// The ray stops early if `is_wall` returns `true` for the next step.
pub fn sight_line_cells(
    angle: Bam,
    is_wall: impl Fn(i32, i32) -> bool,
) -> Vec<SightCell> {
    let oct = angle_to_octant(angle);
    let (doom_dx, doom_dy, arrow) = OCTANTS[oct];

    let colors = [CYAN_BRIGHT, CYAN_MID, CYAN_DIM];
    let glyphs = [arrow, '\u{00B7}', '\u{00B7}']; // ·

    let mut cells = Vec::with_capacity(3);

    for step in 1..=3_i32 {
        let wx = doom_dx * step;
        let wy = doom_dy * step;

        // Stop *before* entering a wall cell.
        if is_wall(wx, wy) {
            break;
        }

        cells.push(SightCell {
            dx: wx,
            dy: -(doom_dy * step), // negate Y for terminal space
            glyph: glyphs[(step - 1) as usize],
            fg: colors[(step - 1) as usize],
        });
    }

    cells
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- angle_to_octant --------------------------------------------------

    #[test]
    fn angle_to_octant_east() {
        assert_eq!(angle_to_octant(Bam::from_raw(0x0000_0000)), 0);
    }

    #[test]
    fn angle_to_octant_north() {
        assert_eq!(angle_to_octant(Bam::from_raw(0x4000_0000)), 2);
    }

    #[test]
    fn angle_to_octant_west() {
        assert_eq!(angle_to_octant(Bam::from_raw(0x8000_0000)), 4);
    }

    #[test]
    fn angle_to_octant_south() {
        assert_eq!(angle_to_octant(Bam::from_raw(0xC000_0000)), 6);
    }

    #[test]
    fn angle_to_octant_ne() {
        assert_eq!(angle_to_octant(Bam::from_raw(0x2000_0000)), 1);
    }

    #[test]
    fn angle_to_octant_wraps() {
        // 0xFFFF_FFFF is just below 0, should still be octant 0 (East).
        assert_eq!(angle_to_octant(Bam::from_raw(0xFFFF_FFFF)), 0);
    }

    // -- sight_line_cells -------------------------------------------------

    #[test]
    fn sight_line_east_no_walls() {
        let cells = sight_line_cells(Bam::from_raw(0x0000_0000), |_, _| false);
        assert_eq!(cells.len(), 3);

        // dx increments: 1, 2, 3
        assert_eq!(cells[0].dx, 1);
        assert_eq!(cells[1].dx, 2);
        assert_eq!(cells[2].dx, 3);

        // dy = 0 for all (east is horizontal)
        assert_eq!(cells[0].dy, 0);
        assert_eq!(cells[1].dy, 0);
        assert_eq!(cells[2].dy, 0);

        // first cell is arrow, rest are middle-dot
        assert_eq!(cells[0].glyph, '\u{25B8}'); // ▸
        assert_eq!(cells[1].glyph, '\u{00B7}'); // ·
        assert_eq!(cells[2].glyph, '\u{00B7}'); // ·
    }

    #[test]
    fn sight_line_north_flips_y() {
        let cells = sight_line_cells(Bam::from_raw(0x4000_0000), |_, _| false);
        assert_eq!(cells.len(), 3);

        // North in doom: dy=+1 per step → terminal dy should be negative
        assert_eq!(cells[0].dy, -1);
        assert_eq!(cells[1].dy, -2);
        assert_eq!(cells[2].dy, -3);

        // dx = 0 for north
        assert_eq!(cells[0].dx, 0);
    }

    #[test]
    fn sight_line_stops_at_wall_step2() {
        // Wall at step 2 (dx=2, dy=0 in doom space for east) → only 1 cell
        let cells = sight_line_cells(Bam::from_raw(0x0000_0000), |dx, _dy| dx >= 2);
        assert_eq!(cells.len(), 1);
        assert_eq!(cells[0].dx, 1);
    }

    #[test]
    fn sight_line_wall_at_step1() {
        // Wall immediately at step 1 → 0 cells
        let cells = sight_line_cells(Bam::from_raw(0x0000_0000), |_, _| true);
        assert_eq!(cells.len(), 0);
    }
}
