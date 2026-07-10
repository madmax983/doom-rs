//! The Nemesis System tracks a specific enemy and grants it increased capabilities.
//!
//! A random enemy can be designated as a "Nemesis". They receive a huge boost to
//! health and speed, turning a normal cannon fodder into a mini-boss.

use crate::mobj::{Mobj, MobjHandle};

/// Tracks the currently active Nemesis target.
#[derive(Debug, Clone)]
pub struct NemesisSystem {
    /// The handle of the monster designated as the Nemesis.
    pub target: Option<MobjHandle>,
    /// Has the nemesis been defeated?
    pub defeated: bool,
}

impl Default for NemesisSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl NemesisSystem {
    /// Creates a new, empty Nemesis tracker.
    pub fn new() -> Self {
        Self {
            target: None,
            defeated: false,
        }
    }

    /// Designates a monster as the Nemesis, boosting its health.
    pub fn assign(&mut self, handle: MobjHandle, mobj: &mut Mobj) {
        self.target = Some(handle);
        self.defeated = false;
        // Boost health by 3x and add 50 base health
        mobj.health = mobj.health.saturating_mul(3).saturating_add(50);
    }

    /// Checks if the provided dead monster handle matches the Nemesis.
    pub fn check_death(&mut self, dead_handle: MobjHandle) -> bool {
        if let Some(nemesis_handle) = self.target {
            if nemesis_handle == dead_handle {
                self.defeated = true;
                self.target = None;
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use doom_types::mobj_kind::MobjKind;
    use doom_types::{Bam, Fixed16_16};

    #[test]
    fn test_nemesis_assignment() {
        let mut sys = NemesisSystem::new();
        let mut mobj = Mobj::new(
            MobjKind::BaronOfHell,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        mobj.health = 20;

        let dummy_handle = MobjHandle {
            index: 1,
            generation: 1,
        };
        sys.assign(dummy_handle, &mut mobj);

        assert_eq!(sys.target, Some(dummy_handle));
        assert_eq!(mobj.health, 110); // 20 * 3 + 50
    }

    #[test]
    fn test_nemesis_death() {
        let mut sys = NemesisSystem::new();
        let dummy_handle = MobjHandle {
            index: 1,
            generation: 1,
        };
        let other_handle = MobjHandle {
            index: 2,
            generation: 1,
        };

        let mut mobj = Mobj::new(
            MobjKind::BaronOfHell,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        sys.assign(dummy_handle, &mut mobj);

        assert!(!sys.check_death(other_handle));
        assert!(!sys.defeated);

        assert!(sys.check_death(dummy_handle));
        assert!(sys.defeated);
        assert_eq!(sys.target, None);
    }
}
