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
#[derive(Clone, Debug, Default)]
pub struct SoundPropagation {
    /// Per-sector sound target: which actor made noise that this sector "heard".
    /// Indexed by sector index. `None` = no noise has reached this sector.
    pub sound_targets: Vec<Option<MobjHandle>>,
    /// Per-sector generation counter for flood-fill visited tracking.
    pub sound_traversed: Vec<u32>,
    /// Current sound generation counter.
    pub sound_gen: u32,
    /// Sound events queued this tic.
    pub sound_queue: Vec<SoundRequest>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::LockedDoorColor;
    use doom_types::Fixed16_16;
    use doom_types::mobj_kind::MobjKind;
    use doom_types::weapons::WeaponType;

    #[test]
    fn should_return_monster_coordinates_when_emitter_is_monster() {
        let handle = MobjHandle::NULL;
        let x = Fixed16_16::ZERO;
        let y = Fixed16_16::ZERO;
        let px = Fixed16_16::from_raw(100);
        let py = Fixed16_16::from_raw(200);

        let req = SoundRequest::MonsterWake(MobjKind::Player, handle, x, y);
        assert_eq!(req.emitter(px, py), Some((x, y)));

        let req2 = SoundRequest::MonsterAttack(MobjKind::Player, handle, x, y);
        assert_eq!(req2.emitter(px, py), Some((x, y)));

        let req3 = SoundRequest::MonsterDie(MobjKind::Player, handle, x, y);
        assert_eq!(req3.emitter(px, py), Some((x, y)));
    }

    #[test]
    fn should_return_player_coordinates_when_emitter_is_player_weapon() {
        let px = Fixed16_16::from_raw(100);
        let py = Fixed16_16::from_raw(200);

        let req = SoundRequest::PlayerWeaponFire(WeaponType::Pistol);
        assert_eq!(req.emitter(px, py), Some((px, py)));

        let req2 = SoundRequest::PlayerSuperShotgunOpen;
        assert_eq!(req2.emitter(px, py), Some((px, py)));

        let req3 = SoundRequest::PlayerSuperShotgunLoad;
        assert_eq!(req3.emitter(px, py), Some((px, py)));

        let req4 = SoundRequest::PlayerSuperShotgunClose;
        assert_eq!(req4.emitter(px, py), Some((px, py)));
    }

    #[test]
    fn should_return_none_when_emitter_is_unlocalized_player_event() {
        let px = Fixed16_16::from_raw(100);
        let py = Fixed16_16::from_raw(200);

        let req = SoundRequest::PlayerDie;
        assert_eq!(req.emitter(px, py), None);

        let req2 = SoundRequest::PlayerUseFail;
        assert_eq!(req2.emitter(px, py), None);

        let req3 = SoundRequest::PlayerUseLockedDoor(LockedDoorColor::Blue);
        assert_eq!(req3.emitter(px, py), None);
    }

    #[test]
    fn should_return_monster_handle_when_origin_is_monster() {
        let handle = MobjHandle::NULL;
        let p_handle = MobjHandle::NULL;

        let req =
            SoundRequest::MonsterWake(MobjKind::Player, handle, Fixed16_16::ZERO, Fixed16_16::ZERO);
        assert_eq!(req.origin_handle(Some(p_handle)), Some(handle));

        let req2 = SoundRequest::MonsterAttack(
            MobjKind::Player,
            handle,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
        );
        assert_eq!(req2.origin_handle(Some(p_handle)), Some(handle));

        let req3 =
            SoundRequest::MonsterDie(MobjKind::Player, handle, Fixed16_16::ZERO, Fixed16_16::ZERO);
        assert_eq!(req3.origin_handle(Some(p_handle)), Some(handle));
    }

    #[test]
    fn should_return_player_handle_when_origin_is_player_weapon() {
        let p_handle = MobjHandle::NULL;

        let req = SoundRequest::PlayerWeaponFire(WeaponType::Pistol);
        assert_eq!(req.origin_handle(Some(p_handle)), Some(p_handle));

        let req2 = SoundRequest::PlayerSuperShotgunOpen;
        assert_eq!(req2.origin_handle(Some(p_handle)), Some(p_handle));

        let req3 = SoundRequest::PlayerSuperShotgunLoad;
        assert_eq!(req3.origin_handle(Some(p_handle)), Some(p_handle));

        let req4 = SoundRequest::PlayerSuperShotgunClose;
        assert_eq!(req4.origin_handle(Some(p_handle)), Some(p_handle));
    }

    #[test]
    fn should_return_none_handle_when_origin_is_unlocalized_player_event() {
        let p_handle = MobjHandle::NULL;

        let req = SoundRequest::PlayerDie;
        assert_eq!(req.origin_handle(Some(p_handle)), None);

        let req2 = SoundRequest::PlayerUseFail;
        assert_eq!(req2.origin_handle(Some(p_handle)), None);

        let req3 = SoundRequest::PlayerUseLockedDoor(LockedDoorColor::Blue);
        assert_eq!(req3.origin_handle(Some(p_handle)), None);
    }
}
