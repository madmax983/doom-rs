//! Save/Load game system — binary serialization of `GameState`.
//!
//! Uses a simple little-endian binary format with no external dependencies
//! (no serde). The format is versioned via `SaveHeader::version` so future
//! changes can be detected and handled.
//!
//! # Roundtrip invariant
//! `load_game(&save_game(gs, ...)).expect("Value must exist").state` must produce a `GameState`
//! equivalent to the original.

use doom_types::limits::{NUM_AMMO, NUM_WEAPONS};
use doom_types::{Bam, Fixed16_16};

use crate::mobj::{Mobj, MobjHandle, MobjSlab, StateNum};
use crate::player::{PlayerState, PspriteState};
use crate::savegame_vanilla;
use crate::state::{
    CeilingMover, CeilingType, ConveyorBelt, DoorMover, ExitRequest, FloorMover, FloorType,
    GameState, LiftMover, LiftStatus, LightSpecial, MoveDirection, PerpetualPlatform,
    PlatformStatus, ScrollingWall,
};
use doom_types::limits::{NUM_POWERS, NUM_PSPRITES};
use doom_types::mobj_kind::MobjKind;
use doom_types::weapons::WeaponType;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Magic bytes identifying a doom-rs save file.
pub const SAVE_MAGIC: [u8; 4] = *b"DRS1";

/// Maximum number of save slots (0..5).
pub const MAX_SAVE_SLOTS: usize = 6;

/// Current save format version.
const SAVE_VERSION: u32 = 3;

/// Supported binary savegame formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveFormat {
    /// The project-native `DRS1` save format.
    DoomRs,
    /// A vanilla Doom `.dsg` save header/payload boundary.
    VanillaDsg,
}

// ---------------------------------------------------------------------------
// SaveHeader
// ---------------------------------------------------------------------------

/// Header for a save file — identifies format, level, and slot description.
#[derive(Debug, Clone)]
pub struct SaveHeader {
    /// Magic bytes (`SAVE_MAGIC`).
    pub magic: [u8; 4],
    /// Format version (currently 2).
    pub version: u32,
    /// Level name, null-padded to 8 bytes (e.g. `b"E1M1\0\0\0\0"`).
    pub level_name: [u8; 8],
    /// Skill level (0=Baby .. 4=Nightmare).
    pub skill: u8,
    /// Tics elapsed in the current level at save time.
    pub level_time: u32,
    /// Player-facing slot description, null-padded to 24 bytes.
    pub description: [u8; 24],
}

// ---------------------------------------------------------------------------
// SaveGame
// ---------------------------------------------------------------------------

/// The result of a successful `load_game` call.
#[derive(Debug, Clone)]
pub struct SaveGame {
    /// The header read from the save data.
    pub header: SaveHeader,
    /// The restored game state.
    pub state: GameState,
}

// ---------------------------------------------------------------------------
// SaveError
// ---------------------------------------------------------------------------

/// Errors that can occur during `load_game`.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum SaveError {
    /// Input data is too short to contain even a header.
    #[error("save data too short for header")]
    TooShort,
    /// Save data does not match any recognized header.
    #[error("unrecognized save file header")]
    BadMagic,
    /// Format version is not supported.
    #[error("unsupported save format version")]
    BadVersion,
    /// Data ended before all fields could be read.
    #[error("save data truncated")]
    Truncated,
    /// Vanilla DSG payload support is not implemented yet.
    #[error("vanilla DSG payload support is not implemented yet")]
    UnsupportedVanillaDsg,
}

// ---------------------------------------------------------------------------
// WriteCursor — helper for serialization
// ---------------------------------------------------------------------------

/// A write cursor wrapping a `Vec<u8>` with typed little-endian writers.
#[derive(Debug)]
pub struct WriteCursor {
    buf: Vec<u8>,
}

impl WriteCursor {
    /// Create a new cursor with the given initial capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            buf: Vec::with_capacity(capacity.min(10 * 1024 * 1024)), // Cap at 10MB to avoid OOM
        }
    }

    /// Write a single byte.
    pub fn write_u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    /// Write a signed 16-bit integer (little-endian).
    pub fn write_i16(&mut self, v: i16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// Write an unsigned 16-bit integer (little-endian).
    pub fn write_u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// Write a signed 32-bit integer (little-endian).
    pub fn write_i32(&mut self, v: i32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// Write an unsigned 32-bit integer (little-endian).
    pub fn write_u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// Write a boolean as a single byte (0 or 1).
    pub fn write_bool(&mut self, v: bool) {
        self.buf.push(u8::from(v));
    }

    /// Write a raw byte slice.
    pub fn write_bytes(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
    }

    /// Consume the cursor and return the underlying buffer.
    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }
}

// ---------------------------------------------------------------------------
// ReadCursor — helper for deserialization
// ---------------------------------------------------------------------------

/// A read cursor wrapping `&[u8]` with typed little-endian readers.
#[derive(Debug)]
pub struct ReadCursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> ReadCursor<'a> {
    /// Create a new cursor at position 0.
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /// Read a single byte.
    pub fn read_u8(&mut self) -> Result<u8, SaveError> {
        if self.pos >= self.data.len() {
            return Err(SaveError::Truncated);
        }
        let v = self.data[self.pos];
        self.pos += 1;
        Ok(v)
    }

    /// Read a signed 16-bit integer (little-endian).
    pub fn read_i16(&mut self) -> Result<i16, SaveError> {
        if self
            .pos
            .checked_add(2)
            .is_none_or(|end| end > self.data.len())
        {
            return Err(SaveError::Truncated);
        }
        let v = i16::from_le_bytes([self.data[self.pos], self.data[self.pos + 1]]);
        self.pos += 2;
        Ok(v)
    }

    /// Read an unsigned 16-bit integer (little-endian).
    pub fn read_u16(&mut self) -> Result<u16, SaveError> {
        if self
            .pos
            .checked_add(2)
            .is_none_or(|end| end > self.data.len())
        {
            return Err(SaveError::Truncated);
        }
        let v = u16::from_le_bytes([self.data[self.pos], self.data[self.pos + 1]]);
        self.pos += 2;
        Ok(v)
    }

    /// Read a signed 32-bit integer (little-endian).
    pub fn read_i32(&mut self) -> Result<i32, SaveError> {
        if self
            .pos
            .checked_add(4)
            .is_none_or(|end| end > self.data.len())
        {
            return Err(SaveError::Truncated);
        }
        let v = i32::from_le_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(v)
    }

    /// Read an unsigned 32-bit integer (little-endian).
    pub fn read_u32(&mut self) -> Result<u32, SaveError> {
        if self
            .pos
            .checked_add(4)
            .is_none_or(|end| end > self.data.len())
        {
            return Err(SaveError::Truncated);
        }
        let v = u32::from_le_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(v)
    }

    /// Read a boolean from a single byte (0 = false, nonzero = true).
    pub fn read_bool(&mut self) -> Result<bool, SaveError> {
        Ok(self.read_u8()? != 0)
    }

    /// Read exactly `n` bytes into a fixed-size array.
    pub fn read_bytes<const N: usize>(&mut self) -> Result<[u8; N], SaveError> {
        if self
            .pos
            .checked_add(N)
            .is_none_or(|end| end > self.data.len())
        {
            return Err(SaveError::Truncated);
        }
        let mut arr = [0u8; N];
        arr.copy_from_slice(&self.data[self.pos..self.pos + N]);
        self.pos += N;
        Ok(arr)
    }
}

// ---------------------------------------------------------------------------
// Serialization helpers
// ---------------------------------------------------------------------------

fn write_fixed(w: &mut WriteCursor, v: Fixed16_16) {
    w.write_i32(v.0);
}

fn read_fixed(r: &mut ReadCursor<'_>) -> Result<Fixed16_16, SaveError> {
    Ok(Fixed16_16(r.read_i32()?))
}

fn write_bam(w: &mut WriteCursor, v: Bam) {
    w.write_u32(v.0);
}

fn read_bam(r: &mut ReadCursor<'_>) -> Result<Bam, SaveError> {
    Ok(Bam(r.read_u32()?))
}

fn write_mobj_handle(w: &mut WriteCursor, h: MobjHandle) {
    w.write_u32(h.index);
    w.write_u32(h.generation);
}

fn read_mobj_handle(r: &mut ReadCursor<'_>) -> Result<MobjHandle, SaveError> {
    let index = r.read_u32()?;
    let generation = r.read_u32()?;
    Ok(MobjHandle { index, generation })
}

fn write_state_num(w: &mut WriteCursor, s: StateNum) {
    w.write_u16(s.0);
}

fn read_state_num(r: &mut ReadCursor<'_>) -> Result<StateNum, SaveError> {
    Ok(StateNum(r.read_u16()?))
}

fn write_psprite_state(w: &mut WriteCursor, ps: &PspriteState) {
    write_state_num(w, ps.state);
    w.write_i32(ps.tics);
    w.write_i32(ps.sx);
    w.write_i32(ps.sy);
}

fn read_psprite_state(r: &mut ReadCursor<'_>) -> Result<PspriteState, SaveError> {
    Ok(PspriteState {
        state: read_state_num(r)?,
        tics: r.read_i32()?,
        sx: r.read_i32()?,
        sy: r.read_i32()?,
    })
}

fn write_mobj_kind(w: &mut WriteCursor, k: MobjKind) {
    w.write_u16(k as u16);
}

fn read_mobj_kind(r: &mut ReadCursor<'_>) -> Result<MobjKind, SaveError> {
    let disc = r.read_u16()?;
    MobjKind::from_repr(disc).ok_or(SaveError::Truncated)
}

fn write_weapon_type(w: &mut WriteCursor, wt: WeaponType) {
    w.write_u8(wt as u8);
}

fn read_weapon_type(r: &mut ReadCursor<'_>) -> Result<WeaponType, SaveError> {
    let v = r.read_u8()?;
    WeaponType::from_num(v as usize).ok_or(SaveError::Truncated)
}

fn write_move_direction(w: &mut WriteCursor, d: MoveDirection) {
    w.write_u8(d as u8);
}

fn read_move_direction(r: &mut ReadCursor<'_>) -> Result<MoveDirection, SaveError> {
    MoveDirection::from_repr(r.read_u8()?).ok_or(SaveError::Truncated)
}

// ---------------------------------------------------------------------------
// Mobj serialization
// ---------------------------------------------------------------------------

fn write_mobj(w: &mut WriteCursor, mo: &Mobj) {
    write_mobj_kind(w, mo.kind);
    write_fixed(w, mo.x);
    write_fixed(w, mo.y);
    write_fixed(w, mo.z);
    write_bam(w, mo.angle);
    w.write_u32(mo.flags);
    write_fixed(w, mo.radius);
    write_fixed(w, mo.height);
    w.write_i32(mo.health);
    write_fixed(w, mo.momx);
    write_fixed(w, mo.momy);
    write_fixed(w, mo.momz);
    write_state_num(w, mo.state);
    w.write_i16(mo.tics);
    write_mobj_handle(w, mo.target);
    w.write_u8(mo.movedir);
    w.write_i32(mo.movecount);
    w.write_i32(mo.reactiontime);
    w.write_i32(mo.threshold);
    w.write_u32(mo.subsector);
}

fn read_mobj(r: &mut ReadCursor<'_>) -> Result<Mobj, SaveError> {
    let kind = read_mobj_kind(r)?;
    let x = read_fixed(r)?;
    let y = read_fixed(r)?;
    let z = read_fixed(r)?;
    let angle = read_bam(r)?;
    let flags = r.read_u32()?;
    let radius = read_fixed(r)?;
    let height = read_fixed(r)?;
    let health = r.read_i32()?;
    let momx = read_fixed(r)?;
    let momy = read_fixed(r)?;
    let momz = read_fixed(r)?;
    let state = read_state_num(r)?;
    let tics = r.read_i16()?;
    let target = read_mobj_handle(r)?;
    let movedir = r.read_u8()?;
    let movecount = r.read_i32()?;
    let reactiontime = r.read_i32()?;
    let threshold = r.read_i32()?;
    let subsector = r.read_u32()?;

    Ok(Mobj {
        kind,
        x,
        y,
        z,
        angle,
        flags,
        radius,
        height,
        health,
        momx,
        momy,
        momz,
        state,
        tics,
        target,
        tracer: MobjHandle::NULL,
        movedir,
        movecount,
        reactiontime,
        threshold,
        subsector,
        spawn_x: Fixed16_16::ZERO,
        spawn_y: Fixed16_16::ZERO,
        spawn_angle: Bam::ZERO,
        spawn_type: 0,
    })
}

// ---------------------------------------------------------------------------
// PlayerState serialization
// ---------------------------------------------------------------------------

fn write_player_state(w: &mut WriteCursor, p: &PlayerState) {
    write_mobj_handle(w, p.handle);
    w.write_i32(p.health());
    w.write_i32(p.armor());
    w.write_u8(p.armor_type);
    for (i, _) in p.max_ammo.iter().enumerate() {
        w.write_u32(p.ammo(i));
    }
    for max_ammo in p.max_ammo.iter().copied() {
        w.write_u32(max_ammo);
    }
    for i in 0..NUM_WEAPONS {
        w.write_bool(p.weapons[i]);
    }
    write_weapon_type(w, p.weapon);
    // pending_weapon: Option<WeaponType>
    match p.pending_weapon {
        Some(wt) => {
            w.write_u8(1);
            write_weapon_type(w, wt);
        }
        None => {
            w.write_u8(0);
        }
    }
    w.write_u8(p.refire);
    w.write_u8(p.extra_light);
    for psprite in &p.psprites {
        write_psprite_state(w, psprite);
    }
    w.write_bool(p.attack_down);
    w.write_u8(p.attack_cooldown);
    w.write_bool(p.use_down);
    for i in 0..NUM_POWERS {
        w.write_u32(p.powers[i]);
    }
    w.write_u8(p.keys);
    w.write_u32(p.bonus_count);
    w.write_u32(p.damage_count);
    w.write_u32(p.kill_count);
    w.write_u32(p.item_count);
    w.write_u32(p.secret_count);
}

fn read_player_state(r: &mut ReadCursor<'_>) -> Result<PlayerState, SaveError> {
    let handle = read_mobj_handle(r)?;
    let health = r.read_i32()?;
    let armor = r.read_i32()?;
    let armor_type = r.read_u8()?;

    let mut ammo = [0u32; NUM_AMMO];
    for slot in &mut ammo {
        *slot = r.read_u32()?;
    }

    let mut max_ammo = [0u32; NUM_AMMO];
    for slot in &mut max_ammo {
        *slot = r.read_u32()?;
    }

    let mut weapons = [false; NUM_WEAPONS];
    for slot in &mut weapons {
        *slot = r.read_bool()?;
    }

    let weapon = read_weapon_type(r)?;

    let pending_weapon = match r.read_u8()? {
        0 => None,
        _ => Some(read_weapon_type(r)?),
    };

    let refire = r.read_u8()?;
    let extra_light = r.read_u8()?;
    let mut psprites = [PspriteState::default(); NUM_PSPRITES];
    for psprite in &mut psprites {
        *psprite = read_psprite_state(r)?;
    }

    let attack_down = r.read_bool()?;
    let attack_cooldown = r.read_u8()?;
    let use_down = r.read_bool()?;

    let mut powers = [0u32; NUM_POWERS];
    for slot in &mut powers {
        *slot = r.read_u32()?;
    }

    let keys = r.read_u8()?;
    let bonus_count = r.read_u32()?;
    let damage_count = r.read_u32()?;
    let kill_count = r.read_u32()?;
    let item_count = r.read_u32()?;
    let secret_count = r.read_u32()?;

    // Build PlayerState using pistol_start then overwrite fields.
    // We need to set private fields (health, armor, ammo) through the public API.
    let mut ps = PlayerState::pistol_start(handle);

    // Set health: apply_damage from 100 to reach target, or heal_overheal.
    // Simpler: use set_health_capped with a very high cap to allow any value.
    // Actually health can be negative (dead player), so we need direct access.
    // Use apply_damage to get from MAX_HEALTH to the target value.
    if health > ps.health() {
        ps.set_health_capped(health, i32::MAX);
    } else {
        ps.apply_damage(ps.health().saturating_sub(health));
    }

    // Set armor via give_armor. But give_armor only upgrades, so we need a
    // workaround. Since PlayerState starts with armor=0, give_armor(points, type)
    // will set it if points > 0. For 0 armor, the default is already 0.
    if armor > 0 {
        ps.give_armor(armor, armor_type);
    }
    ps.armor_type = armor_type;

    // Set ammo: drain the default 50 bullets first, then give exact amounts.
    for i in 0..NUM_AMMO {
        // Drain all existing ammo by using a huge amount.
        let _ = ps.use_ammo(i, ps.ammo(i));
        // Now give the exact saved amount (may exceed default max if backpack was collected).
    }
    ps.max_ammo = max_ammo;
    for (i, amount) in ammo.iter().copied().enumerate() {
        ps.give_ammo(i, amount);
    }

    ps.weapons = weapons;
    ps.weapon = weapon;
    ps.pending_weapon = pending_weapon;
    ps.refire = refire;
    ps.extra_light = extra_light;
    ps.psprites = psprites;
    ps.attack_down = attack_down;
    ps.attack_cooldown = attack_cooldown;
    ps.use_down = use_down;
    ps.powers = powers;
    ps.keys = keys;
    ps.bonus_count = bonus_count;
    ps.damage_count = damage_count;
    ps.kill_count = kill_count;
    ps.item_count = item_count;
    ps.secret_count = secret_count;

    Ok(ps)
}

// ---------------------------------------------------------------------------
// Sector mover serialization
// ---------------------------------------------------------------------------

fn write_door_mover(w: &mut WriteCursor, d: &DoorMover) {
    w.write_u32(d.sector as u32);
    w.write_i16(d.target_height);
    w.write_i16(d.current_height);
    w.write_i16(d.speed);
    w.write_bool(d.is_ceiling);
    w.write_i32(d.wait_tics);
    w.write_i32(d.countdown);
    w.write_i16(d.reopen_height);
    w.write_i32(d.reopen_countdown);
}

fn read_door_mover(r: &mut ReadCursor<'_>) -> Result<DoorMover, SaveError> {
    Ok(DoorMover {
        sector: r.read_u32()? as usize,
        target_height: r.read_i16()?,
        current_height: r.read_i16()?,
        speed: r.read_i16()?,
        is_ceiling: r.read_bool()?,
        wait_tics: r.read_i32()?,
        countdown: r.read_i32()?,
        reopen_height: r.read_i16()?,
        reopen_countdown: r.read_i32()?,
    })
}

fn write_light_special(w: &mut WriteCursor, l: &LightSpecial) {
    w.write_u32(l.sector as u32);
    w.write_i32(l.timer);
    w.write_i32(l.period);
    w.write_i16(l.bright);
    w.write_i16(l.dark);
    w.write_bool(l.is_bright);
}

fn read_light_special(r: &mut ReadCursor<'_>) -> Result<LightSpecial, SaveError> {
    Ok(LightSpecial {
        sector: r.read_u32()? as usize,
        timer: r.read_i32()?,
        period: r.read_i32()?,
        bright: r.read_i16()?,
        dark: r.read_i16()?,
        is_bright: r.read_bool()?,
    })
}

fn write_ceiling_type(w: &mut WriteCursor, ct: CeilingType) {
    w.write_u8(ct as u8);
}

fn read_ceiling_type(r: &mut ReadCursor<'_>) -> Result<CeilingType, SaveError> {
    CeilingType::from_repr(r.read_u8()?).ok_or(SaveError::Truncated)
}

fn write_ceiling_mover(w: &mut WriteCursor, c: &CeilingMover) {
    w.write_u32(c.sector_index as u32);
    w.write_i16(c.top_height);
    w.write_i16(c.bottom_height);
    w.write_i16(c.speed);
    w.write_i16(c.normal_speed);
    w.write_i32(c.crush_damage);
    write_move_direction(w, c.direction);
    w.write_bool(c.silent);
    w.write_bool(c.remove_when_done);
    w.write_u16(c.tag);
    write_ceiling_type(w, c.ceiling_type);
}

fn read_ceiling_mover(r: &mut ReadCursor<'_>) -> Result<CeilingMover, SaveError> {
    Ok(CeilingMover {
        sector_index: r.read_u32()? as usize,
        top_height: r.read_i16()?,
        bottom_height: r.read_i16()?,
        speed: r.read_i16()?,
        normal_speed: r.read_i16()?,
        crush_damage: r.read_i32()?,
        direction: read_move_direction(r)?,
        silent: r.read_bool()?,
        remove_when_done: r.read_bool()?,
        tag: r.read_u16()?,
        ceiling_type: read_ceiling_type(r)?,
    })
}

fn write_floor_type(w: &mut WriteCursor, ft: FloorType) {
    w.write_u8(ft as u8);
}

fn read_floor_type(r: &mut ReadCursor<'_>) -> Result<FloorType, SaveError> {
    FloorType::from_repr(r.read_u8()?).ok_or(SaveError::Truncated)
}

fn write_floor_mover(w: &mut WriteCursor, fm: &FloorMover) {
    w.write_u32(fm.sector_index as u32);
    w.write_i16(fm.target_height);
    w.write_i16(fm.speed);
    write_move_direction(w, fm.direction);
    w.write_i32(fm.wait_tics);
    w.write_i16(fm.return_height);
    w.write_bool(fm.waiting);
    w.write_i32(fm.wait_remaining);
    w.write_bool(fm.crush == crate::state::CrushBehavior::Crush);
    w.write_u16(fm.tag);
    write_floor_type(w, fm.floor_type);
}

fn read_floor_mover(r: &mut ReadCursor<'_>) -> Result<FloorMover, SaveError> {
    Ok(FloorMover {
        sector_index: r.read_u32()? as usize,
        target_height: r.read_i16()?,
        speed: r.read_i16()?,
        direction: read_move_direction(r)?,
        wait_tics: r.read_i32()?,
        return_height: r.read_i16()?,
        waiting: r.read_bool()?,
        wait_remaining: r.read_i32()?,
        crush: if r.read_bool()? {
            crate::state::CrushBehavior::Crush
        } else {
            crate::state::CrushBehavior::NoCrush
        },
        tag: r.read_u16()?,
        floor_type: read_floor_type(r)?,
    })
}

fn write_platform_status(w: &mut WriteCursor, status: PlatformStatus) {
    w.write_u8(status as u8);
}

fn read_platform_status(r: &mut ReadCursor<'_>) -> Result<PlatformStatus, SaveError> {
    PlatformStatus::from_repr(r.read_u8()?).ok_or(SaveError::Truncated)
}

fn write_perpetual_platform(w: &mut WriteCursor, p: &PerpetualPlatform) {
    w.write_u32(p.sector_index as u32);
    w.write_i16(p.low_height);
    w.write_i16(p.high_height);
    w.write_i16(p.speed);
    w.write_i32(p.wait_tics);
    w.write_i32(p.wait_remaining);
    write_platform_status(w, p.status);
    w.write_u16(p.tag);
}

fn read_perpetual_platform(r: &mut ReadCursor<'_>) -> Result<PerpetualPlatform, SaveError> {
    Ok(PerpetualPlatform {
        sector_index: r.read_u32()? as usize,
        low_height: r.read_i16()?,
        high_height: r.read_i16()?,
        speed: r.read_i16()?,
        wait_tics: r.read_i32()?,
        wait_remaining: r.read_i32()?,
        status: read_platform_status(r)?,
        tag: r.read_u16()?,
    })
}

fn write_lift_status(w: &mut WriteCursor, status: LiftStatus) {
    w.write_u8(status as u8);
}

fn read_lift_status(r: &mut ReadCursor<'_>) -> Result<LiftStatus, SaveError> {
    LiftStatus::from_repr(r.read_u8()?).ok_or(SaveError::Truncated)
}

fn write_lift_mover(w: &mut WriteCursor, lm: &LiftMover) {
    w.write_u32(lm.sector_index as u32);
    w.write_i16(lm.low_height);
    w.write_i16(lm.high_height);
    w.write_i16(lm.speed);
    w.write_i32(lm.wait_tics);
    w.write_i32(lm.wait_remaining);
    write_lift_status(w, lm.status);
}

fn read_lift_mover(r: &mut ReadCursor<'_>) -> Result<LiftMover, SaveError> {
    Ok(LiftMover {
        sector_index: r.read_u32()? as usize,
        low_height: r.read_i16()?,
        high_height: r.read_i16()?,
        speed: r.read_i16()?,
        wait_tics: r.read_i32()?,
        wait_remaining: r.read_i32()?,
        status: read_lift_status(r)?,
    })
}

// ---------------------------------------------------------------------------
// ScrollingWall serialization
// ---------------------------------------------------------------------------

fn write_scrolling_wall(w: &mut WriteCursor, sw: &ScrollingWall) {
    w.write_u32(sw.linedef_index as u32);
    w.write_i16(sw.speed_x);
    w.write_i16(sw.speed_y);
    w.write_i32(sw.accumulated_x);
    w.write_i32(sw.accumulated_y);
}

fn read_scrolling_wall(r: &mut ReadCursor<'_>) -> Result<ScrollingWall, SaveError> {
    Ok(ScrollingWall {
        linedef_index: r.read_u32()? as usize,
        speed_x: r.read_i16()?,
        speed_y: r.read_i16()?,
        accumulated_x: r.read_i32()?,
        accumulated_y: r.read_i32()?,
    })
}

// ---------------------------------------------------------------------------
// ConveyorBelt serialization
// ---------------------------------------------------------------------------

fn write_conveyor_belt(w: &mut WriteCursor, cb: &ConveyorBelt) {
    w.write_u32(cb.sector_index as u32);
    w.write_i32(cb.push_x);
    w.write_i32(cb.push_y);
    w.write_i16(cb.direction);
    w.write_i16(cb.speed);
}

fn read_conveyor_belt(r: &mut ReadCursor<'_>) -> Result<ConveyorBelt, SaveError> {
    Ok(ConveyorBelt {
        sector_index: r.read_u32()? as usize,
        push_x: r.read_i32()?,
        push_y: r.read_i32()?,
        direction: r.read_i16()?,
        speed: r.read_i16()?,
    })
}

// ---------------------------------------------------------------------------
// Top-level save/load
// ---------------------------------------------------------------------------

/// Detect which savegame format a blob uses.
pub fn detect_save_format(data: &[u8]) -> Result<SaveFormat, SaveError> {
    if data.len() >= SAVE_MAGIC.len() && data[..SAVE_MAGIC.len()] == SAVE_MAGIC {
        return Ok(SaveFormat::DoomRs);
    }
    if savegame_vanilla::looks_like_vanilla_dsg(data) {
        return Ok(SaveFormat::VanillaDsg);
    }
    if data.len() < savegame_vanilla::VANILLA_HEADER_LEN {
        return Err(SaveError::TooShort);
    }
    Err(SaveError::BadMagic)
}

/// Serialize a `GameState` to a binary save blob.
///
/// The resulting `Vec<u8>` can be written to disk or transmitted over the
/// network.  Use `load_game` to deserialize it back.
pub fn save_game(gs: &GameState, level_name: &[u8; 8], skill: u8, description: &str) -> Vec<u8> {
    save_game_doomrs(gs, level_name, skill, description)
}

/// Serialize a `GameState` using an explicit binary format.
pub fn save_game_with_format(
    gs: &GameState,
    level_name: &[u8; 8],
    skill: u8,
    description: &str,
    format: SaveFormat,
) -> Result<Vec<u8>, SaveError> {
    match format {
        SaveFormat::DoomRs => Ok(save_game_doomrs(gs, level_name, skill, description)),
        SaveFormat::VanillaDsg => savegame_vanilla::save_game(gs, level_name, skill, description),
    }
}

fn save_game_doomrs(gs: &GameState, level_name: &[u8; 8], skill: u8, description: &str) -> Vec<u8> {
    let mut w = WriteCursor::new(4096);

    // --- Header ---
    w.write_bytes(&SAVE_MAGIC);
    w.write_u32(SAVE_VERSION);
    w.write_bytes(level_name);
    w.write_u8(skill);
    w.write_u32(gs.stats.level_time);
    let mut desc_buf = [0u8; 24];
    let mut copy_len = 0;
    for ch in description.chars() {
        if copy_len + ch.len_utf8() > 24 {
            break;
        }
        copy_len += ch.len_utf8();
    }
    let desc_bytes = description.as_bytes();
    desc_buf[..copy_len].copy_from_slice(&desc_bytes[..copy_len]);
    w.write_bytes(&desc_buf);

    // --- PlayerState ---
    write_player_state(&mut w, &gs.player);

    // --- RNG ---
    w.write_u32(gs.rng.index());

    // --- Counters ---
    w.write_u32(gs.tic_num);
    w.write_u32(gs.stats.level_time);
    w.write_u32(gs.stats.kill_count);
    w.write_u32(gs.stats.item_count);
    w.write_u32(gs.stats.secret_count);
    w.write_u32(gs.stats.total_kills);
    w.write_u32(gs.stats.total_items);
    w.write_u32(gs.stats.total_secrets);

    // --- Level name (string form) ---
    let name_bytes = gs.level_name.as_bytes();
    w.write_u32(name_bytes.len() as u32);
    w.write_bytes(name_bytes);

    // --- Exit request ---
    match gs.exit_request {
        None => w.write_u8(0),
        Some(req) => w.write_u8(req as u8 + 1),
    }

    // --- Door movers ---
    w.write_u32(gs.movers.active_doors.len() as u32);
    for door in &gs.movers.active_doors {
        write_door_mover(&mut w, door);
    }

    // --- Light specials ---
    w.write_u32(gs.movers.active_lights.len() as u32);
    for light in &gs.movers.active_lights {
        write_light_special(&mut w, light);
    }

    // --- Ceiling movers ---
    w.write_u32(gs.movers.active_ceilings.len() as u32);
    for ceil in &gs.movers.active_ceilings {
        write_ceiling_mover(&mut w, ceil);
    }

    // --- Floor movers ---
    w.write_u32(gs.movers.active_floors.len() as u32);
    for floor in &gs.movers.active_floors {
        write_floor_mover(&mut w, floor);
    }

    // --- Perpetual platforms ---
    w.write_u32(gs.movers.active_platforms.len() as u32);
    for plat in &gs.movers.active_platforms {
        write_perpetual_platform(&mut w, plat);
    }

    // --- Lifts ---
    w.write_u32(gs.movers.lifts.len() as u32);
    for lift in &gs.movers.lifts {
        write_lift_mover(&mut w, lift);
    }

    // --- Scrolling walls ---
    w.write_u32(gs.movers.scrolling_walls.len() as u32);
    for sw in &gs.movers.scrolling_walls {
        write_scrolling_wall(&mut w, sw);
    }

    // --- Conveyor belts ---
    w.write_u32(gs.movers.conveyors.len() as u32);
    for cb in &gs.movers.conveyors {
        write_conveyor_belt(&mut w, cb);
    }

    // --- Mobjs ---
    w.write_u32(gs.mobjslab.len() as u32);
    for h in gs.mobjslab.iter_handles() {
        // Write the handle itself so we can reconstruct the slab.
        write_mobj_handle(&mut w, h);
        let mo = gs
            .mobjslab
            .get(h)
            .expect("handle from iter_handles must be valid");
        write_mobj(&mut w, mo);
    }

    // --- Player mobj handle ---
    write_mobj_handle(&mut w, gs.player.handle);

    w.into_bytes()
}

/// Deserialize a save blob back into a `SaveGame`.
///
/// Returns `SaveError` if the data is malformed or truncated.
pub fn load_game(data: &[u8]) -> Result<SaveGame, SaveError> {
    match detect_save_format(data)? {
        SaveFormat::DoomRs => load_game_doomrs(data),
        SaveFormat::VanillaDsg => savegame_vanilla::load_game(data),
    }
}

fn load_game_doomrs(data: &[u8]) -> Result<SaveGame, SaveError> {
    // Minimum header size: 4 (magic) + 4 (version) + 8 (level_name) + 1 (skill) + 4 (level_time) + 24 (desc) = 45
    const HEADER_SIZE: usize = 4 + 4 + 8 + 1 + 4 + 24;
    if data.len() < HEADER_SIZE {
        return Err(SaveError::TooShort);
    }

    let mut r = ReadCursor::new(data);

    // --- Header ---
    let magic: [u8; 4] = r.read_bytes()?;
    if magic != SAVE_MAGIC {
        return Err(SaveError::BadMagic);
    }
    let version = r.read_u32()?;
    if version != SAVE_VERSION {
        return Err(SaveError::BadVersion);
    }
    let level_name_hdr: [u8; 8] = r.read_bytes()?;
    let skill = r.read_u8()?;
    let header_level_time = r.read_u32()?;
    let description: [u8; 24] = r.read_bytes()?;

    // 👹 Havoc: Guard against corrupted descriptions
    if core::str::from_utf8(&description).is_err() {
        return Err(SaveError::Truncated);
    }

    let header = SaveHeader {
        magic,
        version,
        level_name: level_name_hdr,
        skill,
        level_time: header_level_time,
        description,
    };

    // --- PlayerState ---
    let player = read_player_state(&mut r)?;

    // --- RNG ---
    let rng_index = r.read_u32()?;
    let mut rng = crate::random::DoomRng::new();
    rng.set_index(rng_index);

    // --- Counters ---
    let tic_num = r.read_u32()?;
    let level_time = r.read_u32()?;
    let kill_count = r.read_u32()?;
    let item_count = r.read_u32()?;
    let secret_count = r.read_u32()?;
    let total_kills = r.read_u32()?;
    let total_items = r.read_u32()?;
    let total_secrets = r.read_u32()?;

    // --- Level name (string) ---
    let name_len = r.read_u32()? as usize;
    if r.pos
        .checked_add(name_len)
        .is_none_or(|end| end > r.data.len())
    {
        return Err(SaveError::Truncated);
    }
    let level_name_str = {
        let bytes = &r.data[r.pos..r.pos + name_len];
        r.pos += name_len;
        if core::str::from_utf8(bytes).is_err() {
            return Err(SaveError::Truncated);
        }
        String::from_utf8_lossy(bytes).into_owned()
    };

    // --- Exit request ---
    let exit_request = match r.read_u8()? {
        0 => None,
        disc => Some(ExitRequest::from_repr(disc - 1).ok_or(SaveError::Truncated)?),
    };

    // --- Door movers ---
    let door_count = r.read_u32()? as usize;
    let max_doors = (r.data.len().saturating_sub(r.pos)) / 36;
    if door_count > max_doors {
        return Err(SaveError::Truncated);
    }
    let mut active_doors = Vec::with_capacity(door_count);
    for _ in 0..door_count {
        active_doors.push(read_door_mover(&mut r)?);
    }

    // --- Light specials ---
    let light_count = r.read_u32()? as usize;
    let max_lights = (r.data.len().saturating_sub(r.pos)) / 16;
    if light_count > max_lights {
        return Err(SaveError::Truncated);
    }
    let mut active_lights = Vec::with_capacity(light_count);
    for _ in 0..light_count {
        active_lights.push(read_light_special(&mut r)?);
    }

    // --- Ceiling movers ---
    let ceiling_count = r.read_u32()? as usize;
    let max_ceilings = (r.data.len().saturating_sub(r.pos)) / 36;
    if ceiling_count > max_ceilings {
        return Err(SaveError::Truncated);
    }
    let mut active_ceilings = Vec::with_capacity(ceiling_count);
    for _ in 0..ceiling_count {
        active_ceilings.push(read_ceiling_mover(&mut r)?);
    }

    // --- Floor movers ---
    let floor_count = r.read_u32()? as usize;
    let max_floors = (r.data.len().saturating_sub(r.pos)) / 36;
    if floor_count > max_floors {
        return Err(SaveError::Truncated);
    }
    let mut active_floors = Vec::with_capacity(floor_count);
    for _ in 0..floor_count {
        active_floors.push(read_floor_mover(&mut r)?);
    }

    // --- Perpetual platforms ---
    let platform_count = r.read_u32()? as usize;
    let max_platforms = (r.data.len().saturating_sub(r.pos)) / 28;
    if platform_count > max_platforms {
        return Err(SaveError::Truncated);
    }
    let mut active_platforms = Vec::with_capacity(platform_count);
    for _ in 0..platform_count {
        active_platforms.push(read_perpetual_platform(&mut r)?);
    }

    // --- Lifts ---
    let lift_count = r.read_u32()? as usize;
    let max_lifts = (r.data.len().saturating_sub(r.pos)) / 28;
    if lift_count > max_lifts {
        return Err(SaveError::Truncated);
    }
    let mut lifts = Vec::with_capacity(lift_count);
    for _ in 0..lift_count {
        lifts.push(read_lift_mover(&mut r)?);
    }

    // --- Scrolling walls ---
    let scroller_count = r.read_u32()? as usize;
    let max_scrollers = (r.data.len().saturating_sub(r.pos)) / 12;
    if scroller_count > max_scrollers {
        return Err(SaveError::Truncated);
    }
    let mut scrolling_walls = Vec::with_capacity(scroller_count);
    for _ in 0..scroller_count {
        scrolling_walls.push(read_scrolling_wall(&mut r)?);
    }

    // --- Conveyor belts ---
    let conveyor_count = r.read_u32()? as usize;
    let max_conveyors = (r.data.len().saturating_sub(r.pos)) / 12;
    if conveyor_count > max_conveyors {
        return Err(SaveError::Truncated);
    }
    let mut conveyors = Vec::with_capacity(conveyor_count);
    for _ in 0..conveyor_count {
        conveyors.push(read_conveyor_belt(&mut r)?);
    }

    // --- Mobjs ---
    let mobj_count = r.read_u32()? as usize;

    // Validate we actually have enough bytes for this many mobjs, avoiding pre-allocation panic
    // A mobj serialization uses roughly ~50 bytes plus the handle.
    let max_mobjs = (r.data.len().saturating_sub(r.pos)) / 36;
    if mobj_count > max_mobjs {
        return Err(SaveError::Truncated);
    }

    // Also protect against absurdly large mobj_count that passes the byte check (e.g. from malicious small saves)
    // Doom's static limits typically never exceed tens of thousands of mobjs even in extreme maps
    if mobj_count > 65536 {
        return Err(SaveError::Truncated);
    }

    let mut mobjslab = MobjSlab::new();

    for _ in 0..mobj_count {
        let saved_handle = read_mobj_handle(&mut r)?;
        let mo = read_mobj(&mut r)?;
        // Allocate the mobj in the slab. The new handle may differ from
        // the saved one, but since we insert in order and start from an
        // empty slab, the indices will match (slot 0, 1, 2, ...).
        // We rely on the slab assigning sequential indices for fresh slabs.
        let _new_handle = mobjslab.alloc(mo);
        // Note: We do NOT attempt to reconstruct generation tags here.
        // The player mobj handle is stored separately and remapped below.
        let _ = saved_handle; // suppress unused warning
    }

    // --- Player mobj handle ---
    let saved_player_handle = read_mobj_handle(&mut r)?;

    // Remap the player mobj handle: find the mobj by index in the new slab.
    // Since we allocated in order, slot indices match, but generations differ.
    // Walk the slab to find the matching index.
    let player_handle = {
        let mut found = MobjHandle::NULL;
        for h in mobjslab.iter_handles() {
            if h.index == saved_player_handle.index {
                found = h;
                break;
            }
        }
        found
    };

    // Havoc 👺: Defend against corrupted player_handle!
    if mobjslab.get(player_handle).is_none() {
        return Err(SaveError::Truncated);
    }

    // Build the GameState.
    let mut state = GameState::new(&level_name_str);
    state.tic_num = tic_num;
    state.rng = rng;
    state.mobjslab = mobjslab;
    state.player = player;
    state.player.handle = player_handle;
    state.stats.kill_count = kill_count;
    state.stats.item_count = item_count;
    state.stats.secret_count = secret_count;
    state.stats.total_kills = total_kills;
    state.stats.total_items = total_items;
    state.stats.total_secrets = total_secrets;
    state.movers.active_doors = active_doors;
    state.movers.active_lights = active_lights;
    state.movers.active_ceilings = active_ceilings;
    state.movers.active_floors = active_floors;
    state.movers.active_platforms = active_platforms;
    state.movers.lifts = lifts;
    state.movers.scrolling_walls = scrolling_walls;
    state.movers.conveyors = conveyors;
    state.exit_request = exit_request;
    state.stats.level_time = level_time;

    Ok(SaveGame { header, state })
}

/// Generate the filename for a save slot (e.g. `"doomsav0.dsg"`).
pub fn save_slot_filename(slot: usize) -> String {
    format!("doomsav{}.dsg", slot)
}

impl core::fmt::Display for SaveFormat {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SaveFormat::DoomRs => write!(f, "DoomRs"),
            SaveFormat::VanillaDsg => write!(f, "VanillaDsg"),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;
