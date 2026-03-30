//! Player input command struct and button constants.

/// One tic of player input — the wire-compatible command struct.
///
/// Layout is `repr(C)` with deterministic padding for netcode serialization.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct TicCmd {
    /// Forward/backward movement (-128..127, positive = forward).
    pub forward_move: i8,
    /// Lateral strafe (-128..127, positive = right).
    pub side_move: i8,
    /// Angle delta in 16-bit BAM units (shifted left 16 -> 32-bit BAM).
    pub angle_turn: i16,
    /// Button bitfield (`bt::BT_*` flags).
    pub buttons: u8,
    /// ASCII chat character (0 = none).
    pub chatchar: u8,
    #[doc(hidden)]
    pub _pad: [u8; 2],
}

/// Button flag constants.
pub mod bt {
    /// Fire / attack.
    pub const BT_ATTACK: u8 = 0x01;
    /// Use / open / activate.
    pub const BT_USE: u8 = 0x02;
    /// Change weapon (weapon number encoded in `BT_WEAPONMASK`).
    pub const BT_CHANGE: u8 = 0x04;
    /// Bits 3-5 encode the target weapon number.
    pub const BT_WEAPONMASK: u8 = 0x38;
}
