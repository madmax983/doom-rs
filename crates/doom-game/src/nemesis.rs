//! The Nemesis System tracks and promotes a monster that deals significant damage.

use crate::mobj::MobjHandle;

/// Tracks the current Nemesis.
#[derive(Debug, Clone)]
pub struct NemesisManager {
    pub handle: Option<MobjHandle>,
    pub health_boost: i32,
}

impl Default for NemesisManager {
    fn default() -> Self {
        Self::new()
    }
}

impl NemesisManager {
    pub fn new() -> Self {
        Self {
            handle: None,
            health_boost: 200,
        }
    }

    /// Promotes a monster to Nemesis if it deals > 10 damage and there isn't one already.
    pub fn try_promote(&mut self, source: MobjHandle, damage: i32) -> bool {
        if self.handle.is_none() && damage > 10 && source != MobjHandle::NULL {
            self.handle = Some(source);
            true
        } else {
            false
        }
    }

    /// Clears the nemesis when defeated, returning the health reward.
    pub fn defeat(&mut self, killed: MobjHandle) -> Option<i32> {
        if let Some(nemesis) = self.handle {
            if nemesis == killed {
                self.handle = None;
                return Some(self.health_boost);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nemesis_promotion_and_defeat() {
        let mut nm = NemesisManager::new();
        let monster = MobjHandle {
            index: 1,
            generation: 1,
        };

        // Damage <= 10 does not promote
        assert!(!nm.try_promote(monster, 10));
        assert!(nm.handle.is_none());

        // Damage > 10 promotes
        assert!(nm.try_promote(monster, 15));
        assert_eq!(nm.handle, Some(monster));

        // Already have a nemesis, do not promote another
        let monster2 = MobjHandle {
            index: 2,
            generation: 1,
        };
        assert!(!nm.try_promote(monster2, 50));
        assert_eq!(nm.handle, Some(monster));

        // Defeating the wrong monster yields no reward
        assert_eq!(nm.defeat(monster2), None);
        assert_eq!(nm.handle, Some(monster));

        // Defeating the nemesis yields a reward and clears it
        assert_eq!(nm.defeat(monster), Some(200));
        assert!(nm.handle.is_none());
    }
}
