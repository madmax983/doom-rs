#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bestiary {
    pub kills: [u32; 256],
}

impl Default for Bestiary {
    fn default() -> Self {
        Self::new()
    }
}

impl Bestiary {
    pub fn new() -> Self {
        Self { kills: [0; 256] }
    }
    pub fn record_kill(&mut self, kind: doom_types::mobj_kind::MobjKind) {
        let index = kind as usize;
        if index < 256 {
            self.kills[index] = self.kills[index].saturating_add(1);
        }
    }
    pub fn get_kills(&self, kind: doom_types::mobj_kind::MobjKind) -> u32 {
        let index = kind as usize;
        if index < 256 {
            self.kills[index]
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use doom_types::mobj_kind::MobjKind;

    #[test]
    fn test_record_kill() {
        let mut bestiary = Bestiary::new();
        bestiary.record_kill(MobjKind::Imp);
        assert_eq!(bestiary.get_kills(MobjKind::Imp), 1);
        assert_eq!(bestiary.get_kills(MobjKind::Demon), 0);
    }
}
