use doom_types::Fixed16_16;

/// State for the Boss Brain (Icon of Sin).
#[derive(Clone, Debug, Default)]
pub struct BossBrainState {
    /// Set `true` once the Boss Brain's see state fires; cubes only
    /// start spawning after this flag is set.
    pub awake: bool,
    /// Spawn spot positions collected from DoomEd thing type 87.
    pub targets: Vec<(Fixed16_16, Fixed16_16)>,
    /// Round-robin index into `targets` for the next cube.
    pub target_index: usize,
}

impl BossBrainState {
    pub fn new() -> Self {
        Self::default()
    }
}
