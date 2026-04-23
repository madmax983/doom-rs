//! Positional SFX with distance attenuation and stereo panning.
//!
//! Computes volume and panning parameters for a sound emitter relative to a
//! listener position and facing direction, using the Doom engine's coordinate
//! system (`Fixed16_16` positions, `Bam` angles).

use doom_types::{Bam, Fixed16_16};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum distance (in map units) at which sounds are still audible.
pub const MAX_SFX_DIST: i32 = 1200;

/// Minimum distance (in map units) at which sounds start to attenuate.
/// Below this, sounds play at full volume.
pub const CLOSE_DIST: i32 = 200;

// ---------------------------------------------------------------------------
// SfxEmitter
// ---------------------------------------------------------------------------

/// A sound emitter with a position in the game world.
#[derive(Clone, Debug)]
pub struct SfxEmitter {
    /// X position of the sound source (`Fixed16_16` map units).
    pub x: Fixed16_16,
    /// Y position of the sound source (`Fixed16_16` map units).
    pub y: Fixed16_16,
}

// ---------------------------------------------------------------------------
// SpatialParams
// ---------------------------------------------------------------------------

/// Result of computing spatial audio parameters.
#[derive(Clone, Debug)]
pub struct SpatialParams {
    /// Volume multiplier in `[0.0, 1.0]`. `0.0` = inaudible, `1.0` = full volume.
    pub volume: f32,
    /// Stereo panning in `[-1.0, 1.0]`. `-1.0` = full left, `0.0` = center, `1.0` = full right.
    pub pan: f32,
}

// ---------------------------------------------------------------------------
// compute_spatial
// ---------------------------------------------------------------------------

/// Compute volume and panning for a sound emitted at `emitter` position,
/// heard by a listener at `listener_x, listener_y` facing `listener_angle`.
///
/// # Volume attenuation
///
/// - If distance <= [`CLOSE_DIST`]: volume = 1.0
/// - If distance >= [`MAX_SFX_DIST`]: volume = 0.0
/// - Otherwise: linear falloff between `CLOSE_DIST` and `MAX_SFX_DIST`
///
/// # Panning
///
/// The stereo pan is computed from the sine of the relative angle between
/// the listener's facing direction and the direction to the emitter.
/// A positive pan value means the sound is to the listener's right; negative
/// means left.
///
/// ## Examples
/// ```
/// use doom_audio::spatial::{compute_spatial, SfxEmitter};
/// use doom_types::{Fixed16_16, Bam};
///
/// let emitter = SfxEmitter {
///     x: Fixed16_16::from_int(100),
///     y: Fixed16_16::ZERO,
/// };
///
/// // Listener is at the origin, facing North. The sound is directly to their right (East).
/// let params = compute_spatial(&emitter, Fixed16_16::ZERO, Fixed16_16::ZERO, Bam(0x4000_0000));
///
/// // The sound is close, so volume is 1.0. Pan is > 0 since it's to the right.
/// assert_eq!(params.volume, 1.0);
/// assert!(params.pan > 0.5);
/// ```
#[must_use]
#[allow(clippy::cast_precision_loss)] // f32 is intentional for audio path calculations
pub fn compute_spatial(
    emitter: &SfxEmitter,
    listener_x: Fixed16_16,
    listener_y: Fixed16_16,
    listener_angle: Bam,
) -> SpatialParams {
    let dx = (emitter.x - listener_x).0 as f32 / 65536.0;
    let dy = (emitter.y - listener_y).0 as f32 / 65536.0;

    let dist = (dx * dx + dy * dy).sqrt();

    // --- Volume attenuation ---
    let close = CLOSE_DIST as f32;
    let max = MAX_SFX_DIST as f32;
    let volume = if dist <= close {
        1.0
    } else if dist >= max {
        0.0
    } else {
        1.0 - (dist - close) / (max - close)
    };

    // --- Panning ---
    // Angle from listener to emitter in radians.
    let angle_to_emitter = dy.atan2(dx);

    // Convert BAM listener angle to radians.
    // BAM: 0 = East, 0x4000_0000 = North (90 degrees).
    // Full circle = 2^32 BAM units = 2*pi radians.
    let listener_rad = (listener_angle.0 as f64) * std::f64::consts::TAU / (u32::MAX as f64 + 1.0);
    let listener_rad = listener_rad as f32;

    // Relative angle: positive means emitter is to the left of listener facing.
    let relative = angle_to_emitter - listener_rad;

    // sin(relative) gives left/right separation.
    // In Doom's coordinate system (BAM 0 = East, increasing = counter-clockwise),
    // a negative sine means the emitter is to the right of the listener's facing
    // direction. We negate so that "right of facing" = positive pan.
    let pan = -(relative.sin().clamp(-1.0, 1.0));

    SpatialParams { volume, pan }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_spatial_at_origin_full_volume() {
        let emitter = SfxEmitter {
            x: Fixed16_16::ZERO,
            y: Fixed16_16::ZERO,
        };
        let params = compute_spatial(&emitter, Fixed16_16::ZERO, Fixed16_16::ZERO, Bam::ZERO);
        assert!(
            (params.volume - 1.0).abs() < 0.01,
            "co-located emitter should be full volume, got {}",
            params.volume
        );
    }

    #[test]
    fn compute_spatial_beyond_max_dist_silent() {
        let emitter = SfxEmitter {
            x: Fixed16_16::from_int(MAX_SFX_DIST + 100),
            y: Fixed16_16::ZERO,
        };
        let params = compute_spatial(&emitter, Fixed16_16::ZERO, Fixed16_16::ZERO, Bam::ZERO);
        assert!(
            params.volume < 0.01,
            "emitter beyond MAX_SFX_DIST should be silent, got {}",
            params.volume
        );
    }

    #[test]
    fn compute_spatial_halfway_half_volume() {
        let mid = (CLOSE_DIST + MAX_SFX_DIST) / 2;
        let emitter = SfxEmitter {
            x: Fixed16_16::from_int(mid),
            y: Fixed16_16::ZERO,
        };
        let params = compute_spatial(&emitter, Fixed16_16::ZERO, Fixed16_16::ZERO, Bam::ZERO);
        assert!(
            (params.volume - 0.5).abs() < 0.1,
            "emitter halfway should be ~0.5 volume, got {}",
            params.volume
        );
    }

    #[test]
    fn compute_spatial_within_close_dist_full_volume() {
        let emitter = SfxEmitter {
            x: Fixed16_16::from_int(100),
            y: Fixed16_16::ZERO,
        };
        let params = compute_spatial(&emitter, Fixed16_16::ZERO, Fixed16_16::ZERO, Bam::ZERO);
        assert!(
            (params.volume - 1.0).abs() < 0.01,
            "emitter within CLOSE_DIST should be full volume, got {}",
            params.volume
        );
    }

    #[test]
    fn compute_spatial_pan_right_when_source_is_right() {
        // Listener faces north (ANG90 in BAM = 0x4000_0000), emitter is to the east (right).
        // ANG90 means facing +Y (north). Emitter at +X is to the right.
        let emitter = SfxEmitter {
            x: Fixed16_16::from_int(500),
            y: Fixed16_16::ZERO,
        };
        let params = compute_spatial(
            &emitter,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam(0x4000_0000), // ANG90 = north
        );
        assert!(
            params.pan > 0.3,
            "Sound to the right should pan right, got {}",
            params.pan
        );
    }

    #[test]
    fn compute_spatial_pan_left_when_source_is_left() {
        // Listener faces north (ANG90), emitter is to the west (left, -X).
        let emitter = SfxEmitter {
            x: Fixed16_16::from_int(-500),
            y: Fixed16_16::ZERO,
        };
        let params = compute_spatial(
            &emitter,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam(0x4000_0000), // ANG90 = north
        );
        assert!(
            params.pan < -0.3,
            "Sound to the left should pan left, got {}",
            params.pan
        );
    }

    #[test]
    fn compute_spatial_pan_center_when_source_ahead() {
        // Listener faces east (angle 0), emitter is directly ahead (east, +X).
        let emitter = SfxEmitter {
            x: Fixed16_16::from_int(500),
            y: Fixed16_16::ZERO,
        };
        let params = compute_spatial(&emitter, Fixed16_16::ZERO, Fixed16_16::ZERO, Bam::ZERO);
        assert!(
            params.pan.abs() < 0.2,
            "Sound ahead should be near center, got {}",
            params.pan
        );
    }

    #[test]
    fn compute_spatial_pan_center_when_source_behind() {
        // Listener faces east (angle 0), emitter is directly behind (west, -X).
        let emitter = SfxEmitter {
            x: Fixed16_16::from_int(-500),
            y: Fixed16_16::ZERO,
        };
        let params = compute_spatial(&emitter, Fixed16_16::ZERO, Fixed16_16::ZERO, Bam::ZERO);
        // Behind = pan ~0 (dead center, since sin(pi) = 0)
        assert!(
            params.pan.abs() < 0.2,
            "Sound behind should be near center (sin(pi)~=0), got {}",
            params.pan
        );
    }

    #[test]
    fn compute_spatial_at_exact_max_dist_is_silent() {
        let emitter = SfxEmitter {
            x: Fixed16_16::from_int(MAX_SFX_DIST),
            y: Fixed16_16::ZERO,
        };
        let params = compute_spatial(&emitter, Fixed16_16::ZERO, Fixed16_16::ZERO, Bam::ZERO);
        assert!(
            params.volume < 0.01,
            "emitter at exactly MAX_SFX_DIST should be silent, got {}",
            params.volume
        );
    }

    #[test]
    fn compute_spatial_at_exact_close_dist_full_volume() {
        let emitter = SfxEmitter {
            x: Fixed16_16::from_int(CLOSE_DIST),
            y: Fixed16_16::ZERO,
        };
        let params = compute_spatial(&emitter, Fixed16_16::ZERO, Fixed16_16::ZERO, Bam::ZERO);
        assert!(
            (params.volume - 1.0).abs() < 0.01,
            "emitter at exactly CLOSE_DIST should be full volume, got {}",
            params.volume
        );
    }

    #[test]
    fn compute_spatial_diagonal_distance_correct() {
        // Place emitter at (848, 849) -- roughly 1200 map units diagonal from origin.
        // sqrt(848^2 + 849^2) ~ 1199.6, just under MAX_SFX_DIST.
        let emitter = SfxEmitter {
            x: Fixed16_16::from_int(848),
            y: Fixed16_16::from_int(849),
        };
        let params = compute_spatial(&emitter, Fixed16_16::ZERO, Fixed16_16::ZERO, Bam::ZERO);
        // Should be barely audible but not zero.
        assert!(
            params.volume > 0.0 && params.volume < 0.05,
            "diagonal emitter near max dist should be barely audible, got {}",
            params.volume
        );
    }
}
