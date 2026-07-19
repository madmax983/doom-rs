use crate::mobj::MobjHandle;
use crate::state::LockedDoorColor;
use doom_types::mobj_kind::MobjKind;

/// A sound event emitted by the game simulation.
///
/// The app (doom-app) drains `GameState::sound_queue` each tic and maps each
/// variant to the appropriate WAD lump name for playback.  The game crate
/// intentionally has no audio dependency — it only describes *what* happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundRequest {
    /// Monster spotted the player / woke up.
    /// Fields: (kind, origin_handle, map_x, map_y), where map_x/map_y are the
    /// emitter's position in Fixed16_16 map units.
    MonsterWake(
        MobjKind,
        MobjHandle,
        doom_types::Fixed16_16,
        doom_types::Fixed16_16,
    ),
    /// Monster was killed.  Fields: (kind, origin_handle, map_x, map_y).
    MonsterDie(
        MobjKind,
        MobjHandle,
        doom_types::Fixed16_16,
        doom_types::Fixed16_16,
    ),
    /// Monster fired a hitscan or projectile attack.
    /// Fields: (kind, origin_handle, map_x, map_y).
    MonsterAttack(
        MobjKind,
        MobjHandle,
        doom_types::Fixed16_16,
        doom_types::Fixed16_16,
    ),
    /// Player weapon actually fired this tic.
    PlayerWeaponFire(doom_types::weapons::WeaponType),
    /// Super shotgun break-open sound.
    PlayerSuperShotgunOpen,
    /// Super shotgun shell-load sound.
    PlayerSuperShotgunLoad,
    /// Super shotgun close-and-lock sound.
    PlayerSuperShotgunClose,
    /// Player died.
    PlayerDie,
    /// Player pressed use into a blocking ordinary wall.
    PlayerUseFail,
    /// Player tried to use a keyed door without the required key color.
    PlayerUseLockedDoor(LockedDoorColor),
}

impl SoundRequest {
    /// Returns the (x, y) map coordinates where this sound originated, if any.
    ///
    /// Useful for distance attenuation and stereo panning in the audio subsystem.
    /// Player-originated sounds will return the provided `player_x` and `player_y`.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_game::sound_prop::SoundRequest;
    /// use doom_types::mobj_kind::MobjKind;
    /// use doom_game::mobj::MobjHandle;
    /// use doom_types::Fixed16_16;
    ///
    /// // Simulate a handle allocation with index 1
    /// let mut slab = doom_game::mobj::MobjSlab::new();
    /// let mobj = doom_game::mobj::Mobj::new(
    ///     MobjKind::Player,
    ///     Fixed16_16::from_raw(100),
    ///     Fixed16_16::from_raw(200),
    ///     doom_types::Bam::ZERO
    /// );
    /// let handle = slab.alloc(mobj);
    ///
    /// let req = SoundRequest::MonsterWake(
    ///     MobjKind::Player,
    ///     handle,
    ///     Fixed16_16::from_int(100),
    ///     Fixed16_16::from_int(200),
    /// );
    ///
    /// let px = Fixed16_16::from_int(0);
    /// let py = Fixed16_16::from_int(0);
    ///
    /// let (x, y) = req.emitter(px, py).expect("item must exist in tests");
    /// assert_eq!(x, Fixed16_16::from_int(100));
    /// assert_eq!(y, Fixed16_16::from_int(200));
    /// ```
    pub fn emitter(
        &self,
        player_x: doom_types::Fixed16_16,
        player_y: doom_types::Fixed16_16,
    ) -> Option<(doom_types::Fixed16_16, doom_types::Fixed16_16)> {
        match *self {
            SoundRequest::MonsterWake(_, _, x, y)
            | SoundRequest::MonsterAttack(_, _, x, y)
            | SoundRequest::MonsterDie(_, _, x, y) => Some((x, y)),
            SoundRequest::PlayerWeaponFire(_)
            | SoundRequest::PlayerSuperShotgunOpen
            | SoundRequest::PlayerSuperShotgunLoad
            | SoundRequest::PlayerSuperShotgunClose => Some((player_x, player_y)),
            SoundRequest::PlayerDie
            | SoundRequest::PlayerUseFail
            | SoundRequest::PlayerUseLockedDoor(_) => None,
        }
    }

    /// Returns the handle of the mob object that generated this sound, if any.
    ///
    /// Useful for checking if the object is still alive or tracking its position
    /// dynamically as the sound plays. Player-originated sounds will return the
    /// provided `player_origin` handle.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_game::sound_prop::SoundRequest;
    /// use doom_types::mobj_kind::MobjKind;
    /// use doom_game::mobj::MobjHandle;
    ///
    /// // Simulate a handle allocation
    /// let mut slab = doom_game::mobj::MobjSlab::new();
    /// let mobj = doom_game::mobj::Mobj::new(
    ///     MobjKind::Player,
    ///     doom_types::Fixed16_16::ZERO,
    ///     doom_types::Fixed16_16::ZERO,
    ///     doom_types::Bam::ZERO
    /// );
    /// let handle = slab.alloc(mobj);
    ///
    /// let req = SoundRequest::MonsterAttack(
    ///     MobjKind::Player,
    ///     handle,
    ///     doom_types::Fixed16_16::ZERO,
    ///     doom_types::Fixed16_16::ZERO,
    /// );
    ///
    /// assert_eq!(req.origin_handle(None), Some(handle));
    /// ```
    pub fn origin_handle(
        &self,
        player_origin: Option<crate::mobj::MobjHandle>,
    ) -> Option<crate::mobj::MobjHandle> {
        match *self {
            SoundRequest::MonsterWake(_, handle, _, _)
            | SoundRequest::MonsterAttack(_, handle, _, _)
            | SoundRequest::MonsterDie(_, handle, _, _) => Some(handle),
            SoundRequest::PlayerWeaponFire(_)
            | SoundRequest::PlayerSuperShotgunOpen
            | SoundRequest::PlayerSuperShotgunLoad
            | SoundRequest::PlayerSuperShotgunClose => player_origin,
            SoundRequest::PlayerDie
            | SoundRequest::PlayerUseFail
            | SoundRequest::PlayerUseLockedDoor(_) => None,
        }
    }
}

/// Sound propagation and event queues.
///
/// Mirrors vanilla Doom's `P_RecursiveSound` state exactly:
/// - `sound_gen` is the global `validcount`, incremented once per `P_NoiseAlert`.
/// - `sound_valid[sec]` is the per-sector `validcount` stamp.
/// - `sound_traversed[sec]` is the per-sector `soundtraversed` value, i.e.
///   `soundblocks + 1` — the number of `ML_SOUNDBLOCK` lines crossed to reach
///   the sector, plus one.  A sector may be re-entered within the same flood if
///   it is reached at a strictly-lower block depth (see `recursive_sound`).
#[derive(Clone, Debug, Default)]
pub struct SoundPropagation {
    /// Per-sector sound target: which actor made noise that this sector "heard".
    /// Indexed by sector index. `None` = no noise has reached this sector.
    pub sound_targets: Vec<Option<MobjHandle>>,
    /// Per-sector `validcount` stamp (vanilla `sec->validcount`).
    pub sound_valid: Vec<u32>,
    /// Per-sector `soundtraversed` = `soundblocks + 1` (vanilla
    /// `sec->soundtraversed`).  Only meaningful when `sound_valid[sec]` equals
    /// the current `sound_gen`.
    pub sound_traversed: Vec<i32>,
    /// Global `validcount`, incremented once per `P_NoiseAlert`.
    pub sound_gen: u32,
    /// Sound events queued this tic.
    pub sound_queue: Vec<SoundRequest>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use doom_types::{Fixed16_16, weapons::WeaponType};

    #[test]
    fn origin_handle_with_no_player_returns_none() {
        let req1 = SoundRequest::PlayerUseFail;
        let req2 = SoundRequest::PlayerDie;
        let req3 = SoundRequest::PlayerUseLockedDoor(crate::state::LockedDoorColor::Red);

        let mut slab = crate::mobj::MobjSlab::new();
        let mobj = crate::mobj::Mobj::new(
            doom_types::mobj_kind::MobjKind::Player,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            doom_types::Bam::ZERO,
        );
        let dummy_handle = slab.alloc(mobj);

        assert_eq!(req1.origin_handle(Some(dummy_handle)), None);
        assert_eq!(req2.origin_handle(Some(dummy_handle)), None);
        assert_eq!(req3.origin_handle(Some(dummy_handle)), None);
    }

    #[test]
    fn origin_handle_with_player_returns_player() {
        let req1 = SoundRequest::PlayerWeaponFire(WeaponType::Pistol);
        let req2 = SoundRequest::PlayerSuperShotgunOpen;
        let req3 = SoundRequest::PlayerSuperShotgunLoad;
        let req4 = SoundRequest::PlayerSuperShotgunClose;

        let mut slab = crate::mobj::MobjSlab::new();
        let mobj = crate::mobj::Mobj::new(
            doom_types::mobj_kind::MobjKind::Player,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            doom_types::Bam::ZERO,
        );
        let dummy_handle = slab.alloc(mobj);

        assert_eq!(req1.origin_handle(Some(dummy_handle)), Some(dummy_handle));
        assert_eq!(req2.origin_handle(Some(dummy_handle)), Some(dummy_handle));
        assert_eq!(req3.origin_handle(Some(dummy_handle)), Some(dummy_handle));
        assert_eq!(req4.origin_handle(Some(dummy_handle)), Some(dummy_handle));
    }

    #[test]
    fn emitter_with_no_position_returns_none() {
        let req1 = SoundRequest::PlayerDie;
        let req2 = SoundRequest::PlayerUseFail;
        let req3 = SoundRequest::PlayerUseLockedDoor(crate::state::LockedDoorColor::Blue);

        let px = Fixed16_16::from_int(10);
        let py = Fixed16_16::from_int(20);

        assert_eq!(req1.emitter(px, py), None);
        assert_eq!(req2.emitter(px, py), None);
        assert_eq!(req3.emitter(px, py), None);
    }

    #[test]
    fn emitter_with_player_position_returns_position() {
        let req1 = SoundRequest::PlayerWeaponFire(WeaponType::Pistol);
        let req2 = SoundRequest::PlayerSuperShotgunOpen;
        let req3 = SoundRequest::PlayerSuperShotgunLoad;
        let req4 = SoundRequest::PlayerSuperShotgunClose;

        let px = Fixed16_16::from_int(30);
        let py = Fixed16_16::from_int(40);

        assert_eq!(req1.emitter(px, py), Some((px, py)));
        assert_eq!(req2.emitter(px, py), Some((px, py)));
        assert_eq!(req3.emitter(px, py), Some((px, py)));
        assert_eq!(req4.emitter(px, py), Some((px, py)));
    }
}
