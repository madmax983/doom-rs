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
    use doom_types::weapons::WeaponType;

    #[test]
    fn test_sound_request_emitter_monster_wake() {
        let mut slab = crate::mobj::MobjSlab::new();
        let mobj = crate::mobj::Mobj::new(
            MobjKind::Player,
            doom_types::Fixed16_16::from_raw(100),
            doom_types::Fixed16_16::from_raw(200),
            doom_types::Bam::ZERO,
        );
        let handle = slab.alloc(mobj);

        let req = SoundRequest::MonsterWake(
            MobjKind::Player,
            handle,
            doom_types::Fixed16_16::from_int(100),
            doom_types::Fixed16_16::from_int(200),
        );

        let px = doom_types::Fixed16_16::from_int(0);
        let py = doom_types::Fixed16_16::from_int(0);

        let (x, y) = req.emitter(px, py).expect("must exist");
        assert_eq!(x, doom_types::Fixed16_16::from_int(100));
        assert_eq!(y, doom_types::Fixed16_16::from_int(200));
    }

    #[test]
    fn test_sound_request_emitter_monster_attack() {
        let mut slab = crate::mobj::MobjSlab::new();
        let mobj = crate::mobj::Mobj::new(
            MobjKind::Player,
            doom_types::Fixed16_16::from_raw(100),
            doom_types::Fixed16_16::from_raw(200),
            doom_types::Bam::ZERO,
        );
        let handle = slab.alloc(mobj);

        let req = SoundRequest::MonsterAttack(
            MobjKind::Player,
            handle,
            doom_types::Fixed16_16::from_int(300),
            doom_types::Fixed16_16::from_int(400),
        );

        let px = doom_types::Fixed16_16::from_int(0);
        let py = doom_types::Fixed16_16::from_int(0);

        let (x, y) = req.emitter(px, py).expect("must exist");
        assert_eq!(x, doom_types::Fixed16_16::from_int(300));
        assert_eq!(y, doom_types::Fixed16_16::from_int(400));
    }

    #[test]
    fn test_sound_request_emitter_monster_die() {
        let mut slab = crate::mobj::MobjSlab::new();
        let mobj = crate::mobj::Mobj::new(
            MobjKind::Player,
            doom_types::Fixed16_16::from_raw(100),
            doom_types::Fixed16_16::from_raw(200),
            doom_types::Bam::ZERO,
        );
        let handle = slab.alloc(mobj);

        let req = SoundRequest::MonsterDie(
            MobjKind::Player,
            handle,
            doom_types::Fixed16_16::from_int(500),
            doom_types::Fixed16_16::from_int(600),
        );

        let px = doom_types::Fixed16_16::from_int(0);
        let py = doom_types::Fixed16_16::from_int(0);

        let (x, y) = req.emitter(px, py).expect("must exist");
        assert_eq!(x, doom_types::Fixed16_16::from_int(500));
        assert_eq!(y, doom_types::Fixed16_16::from_int(600));
    }

    #[test]
    fn test_sound_request_emitter_player_weapon_fire() {
        let req = SoundRequest::PlayerWeaponFire(WeaponType::Pistol);
        let px = doom_types::Fixed16_16::from_int(700);
        let py = doom_types::Fixed16_16::from_int(800);

        let (x, y) = req.emitter(px, py).expect("must exist");
        assert_eq!(x, px);
        assert_eq!(y, py);
    }

    #[test]
    fn test_sound_request_emitter_player_ssg_open() {
        let req = SoundRequest::PlayerSuperShotgunOpen;
        let px = doom_types::Fixed16_16::from_int(10);
        let py = doom_types::Fixed16_16::from_int(20);

        let (x, y) = req.emitter(px, py).expect("must exist");
        assert_eq!(x, px);
        assert_eq!(y, py);
    }

    #[test]
    fn test_sound_request_emitter_player_ssg_load() {
        let req = SoundRequest::PlayerSuperShotgunLoad;
        let px = doom_types::Fixed16_16::from_int(30);
        let py = doom_types::Fixed16_16::from_int(40);

        let (x, y) = req.emitter(px, py).expect("must exist");
        assert_eq!(x, px);
        assert_eq!(y, py);
    }

    #[test]
    fn test_sound_request_emitter_player_ssg_close() {
        let req = SoundRequest::PlayerSuperShotgunClose;
        let px = doom_types::Fixed16_16::from_int(50);
        let py = doom_types::Fixed16_16::from_int(60);

        let (x, y) = req.emitter(px, py).expect("must exist");
        assert_eq!(x, px);
        assert_eq!(y, py);
    }

    #[test]
    fn test_sound_request_emitter_player_die() {
        let req = SoundRequest::PlayerDie;
        let px = doom_types::Fixed16_16::from_int(10);
        let py = doom_types::Fixed16_16::from_int(20);

        assert_eq!(req.emitter(px, py), None);
    }

    #[test]
    fn test_sound_request_emitter_player_use_fail() {
        let req = SoundRequest::PlayerUseFail;
        let px = doom_types::Fixed16_16::from_int(10);
        let py = doom_types::Fixed16_16::from_int(20);

        assert_eq!(req.emitter(px, py), None);
    }

    #[test]
    fn test_sound_request_emitter_player_use_locked_door() {
        let req = SoundRequest::PlayerUseLockedDoor(LockedDoorColor::Red);
        let px = doom_types::Fixed16_16::from_int(10);
        let py = doom_types::Fixed16_16::from_int(20);

        assert_eq!(req.emitter(px, py), None);
    }

    #[test]
    fn test_sound_request_origin_handle_monster_wake() {
        let mut slab = crate::mobj::MobjSlab::new();
        let mobj = crate::mobj::Mobj::new(
            MobjKind::Player,
            doom_types::Fixed16_16::from_raw(100),
            doom_types::Fixed16_16::from_raw(200),
            doom_types::Bam::ZERO,
        );
        let handle = slab.alloc(mobj);

        let req = SoundRequest::MonsterWake(
            MobjKind::Player,
            handle,
            doom_types::Fixed16_16::from_int(100),
            doom_types::Fixed16_16::from_int(200),
        );

        assert_eq!(req.origin_handle(None), Some(handle));
    }

    #[test]
    fn test_sound_request_origin_handle_monster_attack() {
        let mut slab = crate::mobj::MobjSlab::new();
        let mobj = crate::mobj::Mobj::new(
            MobjKind::Player,
            doom_types::Fixed16_16::from_raw(100),
            doom_types::Fixed16_16::from_raw(200),
            doom_types::Bam::ZERO,
        );
        let handle = slab.alloc(mobj);

        let req = SoundRequest::MonsterAttack(
            MobjKind::Player,
            handle,
            doom_types::Fixed16_16::from_int(100),
            doom_types::Fixed16_16::from_int(200),
        );

        assert_eq!(req.origin_handle(None), Some(handle));
    }

    #[test]
    fn test_sound_request_origin_handle_monster_die() {
        let mut slab = crate::mobj::MobjSlab::new();
        let mobj = crate::mobj::Mobj::new(
            MobjKind::Player,
            doom_types::Fixed16_16::from_raw(100),
            doom_types::Fixed16_16::from_raw(200),
            doom_types::Bam::ZERO,
        );
        let handle = slab.alloc(mobj);

        let req = SoundRequest::MonsterDie(
            MobjKind::Player,
            handle,
            doom_types::Fixed16_16::from_int(100),
            doom_types::Fixed16_16::from_int(200),
        );

        assert_eq!(req.origin_handle(None), Some(handle));
    }

    #[test]
    fn test_sound_request_origin_handle_player_weapon_fire() {
        let mut slab = crate::mobj::MobjSlab::new();
        let mobj = crate::mobj::Mobj::new(
            MobjKind::Player,
            doom_types::Fixed16_16::from_raw(100),
            doom_types::Fixed16_16::from_raw(200),
            doom_types::Bam::ZERO,
        );
        let player_handle = slab.alloc(mobj);

        let req = SoundRequest::PlayerWeaponFire(WeaponType::Pistol);
        assert_eq!(req.origin_handle(Some(player_handle)), Some(player_handle));
    }

    #[test]
    fn test_sound_request_origin_handle_player_ssg_open() {
        let mut slab = crate::mobj::MobjSlab::new();
        let mobj = crate::mobj::Mobj::new(
            MobjKind::Player,
            doom_types::Fixed16_16::from_raw(100),
            doom_types::Fixed16_16::from_raw(200),
            doom_types::Bam::ZERO,
        );
        let player_handle = slab.alloc(mobj);

        let req = SoundRequest::PlayerSuperShotgunOpen;
        assert_eq!(req.origin_handle(Some(player_handle)), Some(player_handle));
    }

    #[test]
    fn test_sound_request_origin_handle_player_ssg_load() {
        let mut slab = crate::mobj::MobjSlab::new();
        let mobj = crate::mobj::Mobj::new(
            MobjKind::Player,
            doom_types::Fixed16_16::from_raw(100),
            doom_types::Fixed16_16::from_raw(200),
            doom_types::Bam::ZERO,
        );
        let player_handle = slab.alloc(mobj);

        let req = SoundRequest::PlayerSuperShotgunLoad;
        assert_eq!(req.origin_handle(Some(player_handle)), Some(player_handle));
    }

    #[test]
    fn test_sound_request_origin_handle_player_ssg_close() {
        let mut slab = crate::mobj::MobjSlab::new();
        let mobj = crate::mobj::Mobj::new(
            MobjKind::Player,
            doom_types::Fixed16_16::from_raw(100),
            doom_types::Fixed16_16::from_raw(200),
            doom_types::Bam::ZERO,
        );
        let player_handle = slab.alloc(mobj);

        let req = SoundRequest::PlayerSuperShotgunClose;
        assert_eq!(req.origin_handle(Some(player_handle)), Some(player_handle));
    }

    #[test]
    fn test_sound_request_origin_handle_player_die() {
        let mut slab = crate::mobj::MobjSlab::new();
        let mobj = crate::mobj::Mobj::new(
            MobjKind::Player,
            doom_types::Fixed16_16::from_raw(100),
            doom_types::Fixed16_16::from_raw(200),
            doom_types::Bam::ZERO,
        );
        let player_handle = slab.alloc(mobj);

        let req = SoundRequest::PlayerDie;
        assert_eq!(req.origin_handle(Some(player_handle)), None);
    }

    #[test]
    fn test_sound_request_origin_handle_player_use_fail() {
        let mut slab = crate::mobj::MobjSlab::new();
        let mobj = crate::mobj::Mobj::new(
            MobjKind::Player,
            doom_types::Fixed16_16::from_raw(100),
            doom_types::Fixed16_16::from_raw(200),
            doom_types::Bam::ZERO,
        );
        let player_handle = slab.alloc(mobj);

        let req = SoundRequest::PlayerUseFail;
        assert_eq!(req.origin_handle(Some(player_handle)), None);
    }

    #[test]
    fn test_sound_request_origin_handle_player_use_locked_door() {
        let mut slab = crate::mobj::MobjSlab::new();
        let mobj = crate::mobj::Mobj::new(
            MobjKind::Player,
            doom_types::Fixed16_16::from_raw(100),
            doom_types::Fixed16_16::from_raw(200),
            doom_types::Bam::ZERO,
        );
        let player_handle = slab.alloc(mobj);

        let req = SoundRequest::PlayerUseLockedDoor(LockedDoorColor::Red);
        assert_eq!(req.origin_handle(Some(player_handle)), None);
    }
}
