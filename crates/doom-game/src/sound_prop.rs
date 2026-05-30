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
    use crate::mobj::{Mobj, MobjSlab};
    use doom_types::Bam;
    use doom_types::Fixed16_16;
    use doom_types::mobj_kind::MobjKind;

    // Helper to setup mock items
    fn setup_env() -> (MobjSlab, MobjHandle, Fixed16_16, Fixed16_16) {
        let mut slab = MobjSlab::new();
        let mobj = Mobj::new(
            MobjKind::Player,
            Fixed16_16::from_raw(100),
            Fixed16_16::from_raw(200),
            Bam::ZERO,
        );
        let handle = slab.alloc(mobj);
        (
            slab,
            handle,
            Fixed16_16::from_raw(500),
            Fixed16_16::from_raw(600),
        )
    }

    #[test]
    fn test_emitter_monster_events() {
        let (_slab, handle, px, py) = setup_env();
        let mx = Fixed16_16::from_raw(10);
        let my = Fixed16_16::from_raw(20);

        let wake = SoundRequest::MonsterWake(MobjKind::Player, handle, mx, my);
        assert_eq!(wake.emitter(px, py), Some((mx, my)));

        let attack = SoundRequest::MonsterAttack(MobjKind::Player, handle, mx, my);
        assert_eq!(attack.emitter(px, py), Some((mx, my)));

        let die = SoundRequest::MonsterDie(MobjKind::Player, handle, mx, my);
        assert_eq!(die.emitter(px, py), Some((mx, my)));
    }

    #[test]
    fn test_emitter_player_events() {
        let (_slab, _handle, px, py) = setup_env();

        let fire = SoundRequest::PlayerWeaponFire(doom_types::weapons::WeaponType::Pistol);
        assert_eq!(fire.emitter(px, py), Some((px, py)));

        let ssg_open = SoundRequest::PlayerSuperShotgunOpen;
        assert_eq!(ssg_open.emitter(px, py), Some((px, py)));

        let ssg_load = SoundRequest::PlayerSuperShotgunLoad;
        assert_eq!(ssg_load.emitter(px, py), Some((px, py)));

        let ssg_close = SoundRequest::PlayerSuperShotgunClose;
        assert_eq!(ssg_close.emitter(px, py), Some((px, py)));
    }

    #[test]
    fn test_emitter_no_location_events() {
        let (_slab, _handle, px, py) = setup_env();

        let die = SoundRequest::PlayerDie;
        assert_eq!(die.emitter(px, py), None);

        let use_fail = SoundRequest::PlayerUseFail;
        assert_eq!(use_fail.emitter(px, py), None);

        let locked = SoundRequest::PlayerUseLockedDoor(LockedDoorColor::Red);
        assert_eq!(locked.emitter(px, py), None);
    }

    #[test]
    fn test_origin_handle_monster_events() {
        let (_slab, handle, _px, _py) = setup_env();
        let p_handle = Some(handle); // doesn't matter for monster events

        let wake =
            SoundRequest::MonsterWake(MobjKind::Player, handle, Fixed16_16::ZERO, Fixed16_16::ZERO);
        assert_eq!(wake.origin_handle(p_handle), Some(handle));

        let attack = SoundRequest::MonsterAttack(
            MobjKind::Player,
            handle,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
        );
        assert_eq!(attack.origin_handle(p_handle), Some(handle));

        let die =
            SoundRequest::MonsterDie(MobjKind::Player, handle, Fixed16_16::ZERO, Fixed16_16::ZERO);
        assert_eq!(die.origin_handle(p_handle), Some(handle));
    }

    #[test]
    fn test_origin_handle_player_events() {
        let (_slab, handle, _px, _py) = setup_env();
        let p_handle = Some(handle);

        let fire = SoundRequest::PlayerWeaponFire(doom_types::weapons::WeaponType::Pistol);
        assert_eq!(fire.origin_handle(p_handle), p_handle);

        let ssg_open = SoundRequest::PlayerSuperShotgunOpen;
        assert_eq!(ssg_open.origin_handle(p_handle), p_handle);

        let ssg_load = SoundRequest::PlayerSuperShotgunLoad;
        assert_eq!(ssg_load.origin_handle(p_handle), p_handle);

        let ssg_close = SoundRequest::PlayerSuperShotgunClose;
        assert_eq!(ssg_close.origin_handle(p_handle), p_handle);
    }

    #[test]
    fn test_origin_handle_no_location_events() {
        let (_slab, handle, _px, _py) = setup_env();
        let p_handle = Some(handle);

        let die = SoundRequest::PlayerDie;
        assert_eq!(die.origin_handle(p_handle), None);

        let use_fail = SoundRequest::PlayerUseFail;
        assert_eq!(use_fail.origin_handle(p_handle), None);

        let locked = SoundRequest::PlayerUseLockedDoor(LockedDoorColor::Red);
        assert_eq!(locked.origin_handle(p_handle), None);
    }
}
