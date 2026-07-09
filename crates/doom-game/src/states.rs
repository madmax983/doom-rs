#![allow(missing_docs)]
//! Mobj state machine table with sprite/frame data.
//!
//! Each `MobjStateEntry` carries `sprite` (index into `SPRITE_NAMES`),
//! `frame` (0=A, 1=B, ... plus fullbright bit 0x80), `tics`, `action`,
//! and `next_state`.

const BFG_SOUND: u8 = crate::actions::Action::BfgSound as u8;
const BRAIN_AWAKE: u8 = crate::actions::Action::BrainAwake as u8;
const BRAIN_DIE: u8 = crate::actions::Action::BrainDie as u8;
const BRAIN_EXPLODE: u8 = crate::actions::Action::BrainExplode as u8;
const BRAIN_SCREAM: u8 = crate::actions::Action::BrainScream as u8;
const BRAIN_SPIT: u8 = crate::actions::Action::BrainSpit as u8;
const BRUIS_ATTACK: u8 = crate::actions::Action::BruisAttack as u8;
const BSPI_ATTACK: u8 = crate::actions::Action::BspiAttack as u8;
const CHASE: u8 = crate::actions::Action::Chase as u8;
const CHECK_RELOAD: u8 = crate::actions::Action::CheckReload as u8;
const CLOSE_SHOTGUN2: u8 = crate::actions::Action::CloseShotgun2 as u8;
const CPOS_ATTACK: u8 = crate::actions::Action::CposAttack as u8;
const FACE_TARGET: u8 = crate::actions::Action::FaceTarget as u8;
const FALL: u8 = crate::actions::Action::Fall as u8;
const FAT_ATTACK1: u8 = crate::actions::Action::FatAttack1 as u8;
const FIRE: u8 = crate::actions::Action::Fire as u8;
const FIRE_BFG: u8 = crate::actions::Action::FireBfg as u8;
const FIRE_CGUN: u8 = crate::actions::Action::FireCgun as u8;
const FIRE_MISSILE: u8 = crate::actions::Action::FireMissile as u8;
const FIRE_PISTOL: u8 = crate::actions::Action::FirePistol as u8;
const FIRE_PLASMA: u8 = crate::actions::Action::FirePlasma as u8;
const FIRE_SHOTGUN: u8 = crate::actions::Action::FireShotgun as u8;
const FIRE_SHOTGUN2: u8 = crate::actions::Action::FireShotgun2 as u8;
const GUN_FLASH: u8 = crate::actions::Action::GunFlash as u8;
const LIGHT0: u8 = crate::actions::Action::Light0 as u8;
const LIGHT1: u8 = crate::actions::Action::Light1 as u8;
const LIGHT2: u8 = crate::actions::Action::Light2 as u8;
const LOAD_SHOTGUN2: u8 = crate::actions::Action::LoadShotgun2 as u8;
const LOOK: u8 = crate::actions::Action::Look as u8;
const LOWER: u8 = crate::actions::Action::Lower as u8;
const NONE: u8 = crate::actions::Action::NoAction as u8;
const EXPLODE: u8 = crate::actions::Action::Explode as u8;
const OPEN_SHOTGUN2: u8 = crate::actions::Action::OpenShotgun2 as u8;
const PAIN_ATTACK: u8 = crate::actions::Action::PainAttack as u8;
const POS_ATTACK: u8 = crate::actions::Action::PosAttack as u8;
const PUNCH: u8 = crate::actions::Action::Punch as u8;
const RAISE: u8 = crate::actions::Action::Raise as u8;
const REFIRE: u8 = crate::actions::Action::Refire as u8;
const SARG_ATTACK: u8 = crate::actions::Action::SargAttack as u8;
const SAW: u8 = crate::actions::Action::Saw as u8;
const SCREAM: u8 = crate::actions::Action::Scream as u8;
const XSCREAM: u8 = crate::actions::Action::XScream as u8;
const SKEL_MISSILE: u8 = crate::actions::Action::SkelMissile as u8;
const SKULL_ATTACK: u8 = crate::actions::Action::SkullAttack as u8;
const SPOS_ATTACK: u8 = crate::actions::Action::SposAttack as u8;
const TROO_ATTACK: u8 = crate::actions::Action::TrooAttack as u8;
const VILE_ATTACK: u8 = crate::actions::Action::VileAttack as u8;
const VILE_CHASE: u8 = crate::actions::Action::VileChase as u8;
const VILE_START: u8 = crate::actions::Action::VileStart as u8;
const VILE_TARGET: u8 = crate::actions::Action::VileTarget as u8;
const WEAPON_READY: u8 = crate::actions::Action::WeaponReady as u8;
use crate::mobj::{MobjStateEntry, StateNum};

/// Fullbright bit for frame field.
const FB: u8 = 0x80;

// ---------------------------------------------------------------------------
// Sprite name table
// ---------------------------------------------------------------------------

/// Sprite name constants and lookup table for WAD sprite prefix matching.
pub mod sprite_names {
    pub const SPR_TROO: u16 = 0;
    pub const SPR_SHTG: u16 = 1;
    pub const SPR_PUNG: u16 = 2;
    pub const SPR_PISG: u16 = 3;
    pub const SPR_CHGG: u16 = 4;
    pub const SPR_ROCK: u16 = 5;
    pub const SPR_PLSG: u16 = 6;
    pub const SPR_BFGG: u16 = 7;
    pub const SPR_SAWG: u16 = 8;
    pub const SPR_SHT2: u16 = 9;
    pub const SPR_POSS: u16 = 10;
    pub const SPR_SPOS: u16 = 11;
    pub const SPR_SARG: u16 = 12;
    pub const SPR_HEAD: u16 = 13;
    pub const SPR_BOSS: u16 = 14;
    pub const SPR_BOS2: u16 = 15;
    pub const SPR_CYBR: u16 = 16;
    pub const SPR_SPID: u16 = 17;
    pub const SPR_SKUL: u16 = 18;
    pub const SPR_BSPI: u16 = 19;
    pub const SPR_PAIN: u16 = 20;
    pub const SPR_SKEL: u16 = 21;
    pub const SPR_FATT: u16 = 22;
    pub const SPR_VILE: u16 = 23;
    pub const SPR_CPOS: u16 = 24;
    pub const SPR_KEEN: u16 = 25;
    pub const SPR_BBRN: u16 = 26;
    pub const SPR_PLAY: u16 = 27;
    pub const SPR_BAL1: u16 = 28;
    pub const SPR_BAL2: u16 = 29;
    pub const SPR_MANF: u16 = 30;
    pub const SPR_FATB: u16 = 31;
    pub const SPR_MISL: u16 = 32;
    pub const SPR_PLSS: u16 = 33;
    pub const SPR_PLSE: u16 = 34;
    pub const SPR_BFS1: u16 = 35;
    pub const SPR_BFE1: u16 = 36;
    pub const SPR_BFE2: u16 = 37;
    pub const SPR_RSKE: u16 = 38;
    pub const SPR_APLS: u16 = 39;
    pub const SPR_APBX: u16 = 40;
    pub const SPR_PUFF: u16 = 41;
    pub const SPR_BLUD: u16 = 42;
    pub const SPR_TFOG: u16 = 43;
    pub const SPR_IFOG: u16 = 44;
    pub const SPR_CLIP: u16 = 45;
    pub const SPR_SHEL: u16 = 46;
    pub const SPR_CELL: u16 = 47;
    pub const SPR_AMMO: u16 = 48;
    pub const SPR_SBOX: u16 = 49;
    pub const SPR_BPAK: u16 = 50;
    pub const SPR_MEDI: u16 = 51;
    pub const SPR_STIM: u16 = 52;
    pub const SPR_BON1: u16 = 53;
    pub const SPR_BON2: u16 = 54;
    pub const SPR_SOUL: u16 = 55;
    pub const SPR_PINV: u16 = 56;
    pub const SPR_PINS: u16 = 57;
    pub const SPR_SUIT: u16 = 58;
    pub const SPR_PMAP: u16 = 59;
    pub const SPR_PVIS: u16 = 60;
    pub const SPR_MEGA: u16 = 61;
    pub const SPR_ARM1: u16 = 62;
    pub const SPR_ARM2: u16 = 63;
    pub const SPR_BKEY: u16 = 64;
    pub const SPR_RKEY: u16 = 65;
    pub const SPR_YKEY: u16 = 66;
    pub const SPR_BSKU: u16 = 67;
    pub const SPR_RSKU: u16 = 68;
    pub const SPR_YSKU: u16 = 69;
    pub const SPR_COLU: u16 = 70;
    pub const SPR_TBLU: u16 = 71;
    pub const SPR_TGRN: u16 = 72;
    pub const SPR_TRED: u16 = 73;
    pub const SPR_SMBT: u16 = 74;
    pub const SPR_SMGT: u16 = 75;
    pub const SPR_SMRT: u16 = 76;
    pub const SPR_CEYE: u16 = 77;
    pub const SPR_FSKU: u16 = 78;
    pub const SPR_FIRE: u16 = 79;
    pub const SPR_PLAS: u16 = 80;
    pub const SPR_BAR1: u16 = 81;
    pub const SPR_BEXP: u16 = 82;
    pub const SPR_NONE: u16 = 0xFFFF;

    pub const SPRITE_COUNT: usize = 83;

    /// Sprite name strings for WAD lookup.
    pub const SPRITE_NAMES: [&str; SPRITE_COUNT] = [
        "TROO", "SHTG", "PUNG", "PISG", "CHGG", "ROCK", "PLSG", "BFGG", "SAWG", "SHT2", "POSS",
        "SPOS", "SARG", "HEAD", "BOSS", "BOS2", "CYBR", "SPID", "SKUL", "BSPI", "PAIN", "SKEL",
        "FATT", "VILE", "CPOS", "KEEN", "BBRN", "PLAY", "BAL1", "BAL2", "MANF", "FATB", "MISL",
        "PLSS", "PLSE", "BFS1", "BFE1", "BFE2", "RSKE", "APLS", "APBX", "PUFF", "BLUD", "TFOG",
        "IFOG", "CLIP", "SHEL", "CELL", "AMMO", "SBOX", "BPAK", "MEDI", "STIM", "BON1", "BON2",
        "SOUL", "PINV", "PINS", "SUIT", "PMAP", "PVIS", "MEGA", "ARM1", "ARM2", "BKEY", "RKEY",
        "YKEY", "BSKU", "RSKU", "YSKU", "COLU", "TBLU", "TGRN", "TRED", "SMBT", "SMGT", "SMRT",
        "CEYE", "FSKU", "FIRE", "PLAS", "BAR1", "BEXP",
    ];
}

use sprite_names::*;

// ---------------------------------------------------------------------------
// State ID constants
// ---------------------------------------------------------------------------

/// Named indices into the `STATES` table.
pub mod ids {
    // --- Sentinel ---
    pub const S_NULL: u16 = 0;

    // --- Trooper (Zombie Man) ---
    pub const S_POSS_STND: u16 = 1;
    pub const S_POSS_RUN1: u16 = 2;
    pub const S_POSS_RUN2: u16 = 3;

    // --- Sergeant (Shotgun Guy) ---
    pub const S_SPOS_STND: u16 = 4;
    pub const S_SPOS_RUN1: u16 = 5;
    pub const S_SPOS_RUN2: u16 = 6;

    // --- Imp ---
    pub const S_TROO_STND: u16 = 7;
    pub const S_TROO_RUN1: u16 = 8;
    pub const S_TROO_RUN2: u16 = 9;

    // --- Demon (Pink Demon) ---
    pub const S_SARG_STND: u16 = 10;
    pub const S_SARG_RUN1: u16 = 11;
    pub const S_SARG_RUN2: u16 = 12;

    // --- Cacodemon ---
    pub const S_HEAD_STND: u16 = 13;
    pub const S_HEAD_RUN1: u16 = 14;
    pub const S_HEAD_RUN2: u16 = 15;

    // --- Baron of Hell ---
    pub const S_BOSS_STND: u16 = 16;
    pub const S_BOSS_RUN1: u16 = 17;
    pub const S_BOSS_RUN2: u16 = 18;

    // --- Cyberdemon ---
    pub const S_CYBER_STND: u16 = 19;
    pub const S_CYBER_RUN1: u16 = 20;
    pub const S_CYBER_RUN2: u16 = 21;

    // --- Spider Mastermind ---
    pub const S_SPID_STND: u16 = 22;
    pub const S_SPID_RUN1: u16 = 23;
    pub const S_SPID_RUN2: u16 = 24;

    // Death and pain states (original 8 monsters)
    pub const S_POSS_DIE1: u16 = 25;
    pub const S_POSS_DIE2: u16 = 26;
    pub const S_POSS_PAIN: u16 = 27;
    pub const S_SPOS_DIE1: u16 = 28;
    pub const S_SPOS_DIE2: u16 = 29;
    pub const S_SPOS_PAIN: u16 = 30;
    pub const S_TROO_DIE1: u16 = 31;
    pub const S_TROO_DIE2: u16 = 32;
    pub const S_TROO_PAIN: u16 = 33;
    pub const S_SARG_DIE1: u16 = 34;
    pub const S_SARG_DIE2: u16 = 35;
    pub const S_SARG_PAIN: u16 = 36;
    pub const S_HEAD_DIE1: u16 = 37;
    pub const S_HEAD_DIE2: u16 = 38;
    pub const S_HEAD_PAIN: u16 = 39;
    pub const S_BOSS_DIE1: u16 = 40;
    pub const S_BOSS_DIE2: u16 = 41;
    pub const S_BOSS_PAIN: u16 = 42;
    pub const S_CYBER_DIE1: u16 = 43;
    pub const S_CYBER_DIE2: u16 = 44;
    pub const S_CYBER_PAIN: u16 = 45;
    pub const S_SPID_DIE1: u16 = 46;
    pub const S_SPID_DIE2: u16 = 47;
    pub const S_SPID_PAIN: u16 = 48;
    // Extended death frames for original 8 monsters (appended after state 314)
    pub const S_POSS_DIE3: u16 = 315;
    pub const S_POSS_DIE4: u16 = 316;
    pub const S_POSS_DIE5: u16 = 317;
    pub const S_SPOS_DIE3: u16 = 318;
    pub const S_SPOS_DIE4: u16 = 319;
    pub const S_SPOS_DIE5: u16 = 320;
    pub const S_TROO_DIE3: u16 = 321;
    pub const S_TROO_DIE4: u16 = 322;
    pub const S_TROO_DIE5: u16 = 323;
    pub const S_SARG_DIE3: u16 = 324;
    pub const S_SARG_DIE4: u16 = 325;
    pub const S_SARG_DIE5: u16 = 326;
    pub const S_HEAD_DIE3: u16 = 327;
    pub const S_HEAD_DIE4: u16 = 328;
    pub const S_HEAD_DIE5: u16 = 329;
    pub const S_BOSS_DIE3: u16 = 330;
    pub const S_BOSS_DIE4: u16 = 331;
    pub const S_BOSS_DIE5: u16 = 332;
    pub const S_CYBER_DIE3: u16 = 333;
    pub const S_CYBER_DIE4: u16 = 334;
    pub const S_CYBER_DIE5: u16 = 335;
    pub const S_SPID_DIE3: u16 = 336;
    pub const S_SPID_DIE4: u16 = 337;
    pub const S_SPID_DIE5: u16 = 338;
    // Extended death frames for BOS2 (Hell Knight)
    pub const S_BOS2_DIE3: u16 = 339;
    pub const S_BOS2_DIE4: u16 = 340;
    pub const S_BOS2_DIE5: u16 = 341;

    // Attack states
    pub const S_POSS_ATK1: u16 = 49;
    pub const S_POSS_ATK2: u16 = 50;
    pub const S_POSS_ATK3: u16 = 51;
    pub const S_SPOS_ATK1: u16 = 52;
    pub const S_SPOS_ATK2: u16 = 53;
    pub const S_SPOS_ATK3: u16 = 54;
    pub const S_TROO_ATK1: u16 = 55;
    pub const S_TROO_ATK2: u16 = 56;
    pub const S_TROO_ATK3: u16 = 57;
    pub const S_SARG_ATK1: u16 = 58;
    pub const S_SARG_ATK2: u16 = 59;
    pub const S_SARG_ATK3: u16 = 60;

    // -----------------------------------------------------------------------
    // Projectile states (61..90)
    // -----------------------------------------------------------------------
    // Imp fireball (BAL1)
    pub const S_TBALL1: u16 = 61;
    pub const S_TBALL2: u16 = 62;
    pub const S_TBALLX1: u16 = 63;
    pub const S_TBALLX2: u16 = 64;
    pub const S_TBALLX3: u16 = 65;
    // Baron/HK fireball (BAL2)
    pub const S_BRBALL1: u16 = 66;
    pub const S_BRBALL2: u16 = 67;
    pub const S_BRBALLX1: u16 = 68;
    pub const S_BRBALLX2: u16 = 69;
    pub const S_BRBALLX3: u16 = 70;
    // Rocket (MISL)
    pub const S_ROCKET: u16 = 71;
    pub const S_EXPLODE1: u16 = 72;
    pub const S_EXPLODE2: u16 = 73;
    pub const S_EXPLODE3: u16 = 74;
    // Plasma ball (PLSS/PLSE)
    pub const S_PLASBALL1: u16 = 75;
    pub const S_PLASBALL2: u16 = 76;
    pub const S_PLASEXP1: u16 = 77;
    pub const S_PLASEXP2: u16 = 78;
    pub const S_PLASEXP3: u16 = 79;
    pub const S_PLASEXP4: u16 = 80;
    // BFG ball (BFS1/BFE1)
    pub const S_BFGSHOT1: u16 = 81;
    pub const S_BFGSHOT2: u16 = 82;
    pub const S_BFGLAND1: u16 = 83;
    pub const S_BFGLAND2: u16 = 84;
    pub const S_BFGLAND3: u16 = 85;
    pub const S_BFGLAND4: u16 = 86;
    pub const S_BFGLAND5: u16 = 87;
    pub const S_BFGLAND6: u16 = 88;
    // Revenant tracer (RSKE)
    pub const S_TRACER1: u16 = 89;
    pub const S_TRACER2: u16 = 90;
    pub const S_TRACEEXP1: u16 = 91;
    pub const S_TRACEEXP2: u16 = 92;
    pub const S_TRACEEXP3: u16 = 93;
    // Arachnotron plasma (APLS/APBX)
    pub const S_ARACH_PLAZ1: u16 = 94;
    pub const S_ARACH_PLAZ2: u16 = 95;
    pub const S_ARACH_PLEX1: u16 = 96;
    pub const S_ARACH_PLEX2: u16 = 97;
    pub const S_ARACH_PLEX3: u16 = 98;
    // Mancubus fireball (FATB/MANF)
    pub const S_FATSHOT1: u16 = 99;
    pub const S_FATSHOT2: u16 = 100;
    pub const S_FATSHOTX1: u16 = 101;
    pub const S_FATSHOTX2: u16 = 102;
    pub const S_FATSHOTX3: u16 = 103;

    // -----------------------------------------------------------------------
    // Effect states (104..118)
    // -----------------------------------------------------------------------
    // Bullet puff (PUFF)
    pub const S_PUFF1: u16 = 104;
    pub const S_PUFF2: u16 = 105;
    pub const S_PUFF3: u16 = 106;
    pub const S_PUFF4: u16 = 107;
    // Blood splat (BLUD)
    pub const S_BLOOD1: u16 = 108;
    pub const S_BLOOD2: u16 = 109;
    pub const S_BLOOD3: u16 = 110;
    // Teleport fog (TFOG)
    pub const S_TFOG1: u16 = 111;
    pub const S_TFOG2: u16 = 112;
    pub const S_TFOG3: u16 = 113;
    pub const S_TFOG4: u16 = 114;
    pub const S_TFOG5: u16 = 115;
    // Item respawn fog (IFOG)
    pub const S_IFOG1: u16 = 116;
    pub const S_IFOG2: u16 = 117;
    pub const S_IFOG3: u16 = 118;
    pub const S_IFOG4: u16 = 119;
    pub const S_IFOG5: u16 = 120;

    // -----------------------------------------------------------------------
    // Doom 2 monster states (121..190)
    // -----------------------------------------------------------------------
    // Lost Soul (SKUL)
    pub const S_SKULL_STND: u16 = 121;
    pub const S_SKULL_STND2: u16 = 122;
    pub const S_SKULL_RUN1: u16 = 123;
    pub const S_SKULL_RUN2: u16 = 124;
    pub const S_SKULL_RUN3: u16 = 125;
    pub const S_SKULL_RUN4: u16 = 126;
    pub const S_SKULL_ATK1: u16 = 127;
    pub const S_SKULL_ATK2: u16 = 128;
    pub const S_SKULL_ATK3: u16 = 129;
    pub const S_SKULL_PAIN: u16 = 130;
    pub const S_SKULL_DIE1: u16 = 131;
    pub const S_SKULL_DIE2: u16 = 132;
    pub const S_SKULL_DIE3: u16 = 133;
    pub const S_SKULL_DIE4: u16 = 134;
    pub const S_SKULL_DIE5: u16 = 135;
    // Arachnotron (BSPI)
    pub const S_BSPI_STND: u16 = 136;
    pub const S_BSPI_STND2: u16 = 137;
    pub const S_BSPI_RUN1: u16 = 138;
    pub const S_BSPI_RUN2: u16 = 139;
    pub const S_BSPI_RUN3: u16 = 140;
    pub const S_BSPI_RUN4: u16 = 141;
    pub const S_BSPI_ATK1: u16 = 142;
    pub const S_BSPI_ATK2: u16 = 143;
    pub const S_BSPI_ATK3: u16 = 144;
    pub const S_BSPI_PAIN: u16 = 145;
    pub const S_BSPI_DIE1: u16 = 146;
    pub const S_BSPI_DIE2: u16 = 147;
    pub const S_BSPI_DIE3: u16 = 148;
    pub const S_BSPI_DIE4: u16 = 149;
    pub const S_BSPI_DIE5: u16 = 150;
    // Pain Elemental (PAIN)
    pub const S_PAIN_STND: u16 = 151;
    pub const S_PAIN_STND2: u16 = 152;
    pub const S_PAIN_RUN1: u16 = 153;
    pub const S_PAIN_RUN2: u16 = 154;
    pub const S_PAIN_RUN3: u16 = 155;
    pub const S_PAIN_RUN4: u16 = 156;
    pub const S_PAIN_ATK1: u16 = 157;
    pub const S_PAIN_ATK2: u16 = 158;
    pub const S_PAIN_ATK3: u16 = 159;
    pub const S_PAIN_PAIN1: u16 = 160;
    pub const S_PAIN_DIE1: u16 = 161;
    pub const S_PAIN_DIE2: u16 = 162;
    pub const S_PAIN_DIE3: u16 = 163;
    pub const S_PAIN_DIE4: u16 = 164;
    pub const S_PAIN_DIE5: u16 = 165;
    // Revenant (SKEL)
    pub const S_SKEL_STND: u16 = 166;
    pub const S_SKEL_STND2: u16 = 167;
    pub const S_SKEL_RUN1: u16 = 168;
    pub const S_SKEL_RUN2: u16 = 169;
    pub const S_SKEL_RUN3: u16 = 170;
    pub const S_SKEL_RUN4: u16 = 171;
    pub const S_SKEL_ATK1: u16 = 172;
    pub const S_SKEL_ATK2: u16 = 173;
    pub const S_SKEL_ATK3: u16 = 174;
    pub const S_SKEL_PAIN: u16 = 175;
    pub const S_SKEL_DIE1: u16 = 176;
    pub const S_SKEL_DIE2: u16 = 177;
    pub const S_SKEL_DIE3: u16 = 178;
    pub const S_SKEL_DIE4: u16 = 179;
    pub const S_SKEL_DIE5: u16 = 180;
    // Mancubus (FATT)
    pub const S_FATT_STND: u16 = 181;
    pub const S_FATT_STND2: u16 = 182;
    pub const S_FATT_RUN1: u16 = 183;
    pub const S_FATT_RUN2: u16 = 184;
    pub const S_FATT_RUN3: u16 = 185;
    pub const S_FATT_RUN4: u16 = 186;
    pub const S_FATT_ATK1: u16 = 187;
    pub const S_FATT_ATK2: u16 = 188;
    pub const S_FATT_ATK3: u16 = 189;
    pub const S_FATT_PAIN: u16 = 190;
    pub const S_FATT_DIE1: u16 = 191;
    pub const S_FATT_DIE2: u16 = 192;
    pub const S_FATT_DIE3: u16 = 193;
    pub const S_FATT_DIE4: u16 = 194;
    pub const S_FATT_DIE5: u16 = 195;
    // Arch-Vile (VILE)
    pub const S_VILE_STND: u16 = 196;
    pub const S_VILE_STND2: u16 = 197;
    pub const S_VILE_RUN1: u16 = 198;
    pub const S_VILE_RUN2: u16 = 199;
    pub const S_VILE_RUN3: u16 = 200;
    pub const S_VILE_RUN4: u16 = 201;
    pub const S_VILE_ATK1: u16 = 202;
    pub const S_VILE_ATK2: u16 = 203;
    pub const S_VILE_ATK3: u16 = 204;
    pub const S_VILE_PAIN: u16 = 205;
    pub const S_VILE_DIE1: u16 = 206;
    pub const S_VILE_DIE2: u16 = 207;
    pub const S_VILE_DIE3: u16 = 208;
    pub const S_VILE_DIE4: u16 = 209;
    pub const S_VILE_DIE5: u16 = 210;
    // Chaingunner (CPOS)
    pub const S_CPOS_STND: u16 = 211;
    pub const S_CPOS_STND2: u16 = 212;
    pub const S_CPOS_RUN1: u16 = 213;
    pub const S_CPOS_RUN2: u16 = 214;
    pub const S_CPOS_RUN3: u16 = 215;
    pub const S_CPOS_RUN4: u16 = 216;
    pub const S_CPOS_ATK1: u16 = 217;
    pub const S_CPOS_ATK2: u16 = 218;
    pub const S_CPOS_ATK3: u16 = 219;
    pub const S_CPOS_PAIN: u16 = 220;
    pub const S_CPOS_DIE1: u16 = 221;
    pub const S_CPOS_DIE2: u16 = 222;
    pub const S_CPOS_DIE3: u16 = 223;
    pub const S_CPOS_DIE4: u16 = 224;
    pub const S_CPOS_DIE5: u16 = 225;
    // Hell Knight (BOS2) -- own states
    pub const S_BOS2_STND: u16 = 226;
    pub const S_BOS2_RUN1: u16 = 227;
    pub const S_BOS2_RUN2: u16 = 228;
    pub const S_BOS2_ATK1: u16 = 229;
    pub const S_BOS2_ATK2: u16 = 230;
    pub const S_BOS2_ATK3: u16 = 231;
    pub const S_BOS2_PAIN: u16 = 232;
    pub const S_BOS2_DIE1: u16 = 233;
    pub const S_BOS2_DIE2: u16 = 234;

    // -----------------------------------------------------------------------
    // Weapon states (235..274)
    // -----------------------------------------------------------------------
    // Fist (PUNG)
    pub const S_PUNCH_UP: u16 = 235;
    pub const S_PUNCH_DOWN: u16 = 236;
    pub const S_PUNCH_READY: u16 = 237;
    pub const S_PUNCH1: u16 = 238;
    pub const S_PUNCH2: u16 = 239;
    pub const S_PUNCH3: u16 = 240;
    pub const S_PUNCH4: u16 = 241;
    // Pistol (PISG)
    pub const S_PISTOL_UP: u16 = 242;
    pub const S_PISTOL_DOWN: u16 = 243;
    pub const S_PISTOL_READY: u16 = 244;
    pub const S_PISTOL1: u16 = 245;
    pub const S_PISTOL2: u16 = 246;
    pub const S_PISTOL3: u16 = 247;
    pub const S_PISTOL_FLASH1: u16 = 248;
    pub const S_PISTOL_FLASH2: u16 = 249;
    // Shotgun (SHTG)
    pub const S_SGUN_UP: u16 = 250;
    pub const S_SGUN_DOWN: u16 = 251;
    pub const S_SGUN_READY: u16 = 252;
    pub const S_SGUN1: u16 = 253;
    pub const S_SGUN2: u16 = 254;
    pub const S_SGUN3: u16 = 255;
    pub const S_SGUN4: u16 = 256;
    pub const S_SGUN_FLASH1: u16 = 257;
    pub const S_SGUN_FLASH2: u16 = 258;
    // SSG (SHT2)
    pub const S_DSGUN_UP: u16 = 259;
    pub const S_DSGUN_DOWN: u16 = 260;
    pub const S_DSGUN_READY: u16 = 261;
    pub const S_DSGUN1: u16 = 262;
    pub const S_DSGUN2: u16 = 263;
    pub const S_DSGUN3: u16 = 264;
    pub const S_DSGUN4: u16 = 265;
    pub const S_DSGUN5: u16 = 266;
    pub const S_DSGUN6: u16 = 267;
    pub const S_DSGUN7: u16 = 268;
    pub const S_DSGUN_FLASH1: u16 = 269;
    pub const S_DSGUN_FLASH2: u16 = 270;
    // Chaingun (CHGG)
    pub const S_CHAIN_UP: u16 = 271;
    pub const S_CHAIN_DOWN: u16 = 272;
    pub const S_CHAIN_READY: u16 = 273;
    pub const S_CHAIN1: u16 = 274;
    pub const S_CHAIN2: u16 = 275;
    pub const S_CHAIN_FLASH1: u16 = 276;
    pub const S_CHAIN_FLASH2: u16 = 277;
    // Rocket launcher (ROCK)
    pub const S_MISSILE_UP: u16 = 278;
    pub const S_MISSILE_DOWN: u16 = 279;
    pub const S_MISSILE_READY: u16 = 280;
    pub const S_MISSILE1: u16 = 281;
    pub const S_MISSILE2: u16 = 282;
    pub const S_MISSILE3: u16 = 283;
    pub const S_MISSILE_FLASH1: u16 = 284;
    pub const S_MISSILE_FLASH2: u16 = 285;
    // Plasma gun (PLSG)
    pub const S_PLASMA_UP: u16 = 286;
    pub const S_PLASMA_DOWN: u16 = 287;
    pub const S_PLASMA_READY: u16 = 288;
    pub const S_PLASMA1: u16 = 289;
    pub const S_PLASMA2: u16 = 290;
    // BFG (BFGG)
    pub const S_BFG_UP: u16 = 291;
    pub const S_BFG_DOWN: u16 = 292;
    pub const S_BFG_READY: u16 = 293;
    pub const S_BFG1: u16 = 294;
    pub const S_BFG2: u16 = 295;
    pub const S_BFG_FLASH1: u16 = 296;
    pub const S_BFG_FLASH2: u16 = 297;
    // Chainsaw (SAWG)
    pub const S_SAW_UP: u16 = 298;
    pub const S_SAW_DOWN: u16 = 299;
    pub const S_SAW_READY1: u16 = 300;
    pub const S_SAW_READY2: u16 = 301;
    pub const S_SAW1: u16 = 302;
    pub const S_SAW2: u16 = 303;
    pub const S_SAW3: u16 = 304;

    // -----------------------------------------------------------------------
    // Fire column states (305..308) — Arch-Vile's fire effect
    // -----------------------------------------------------------------------
    pub const S_FIRE1: u16 = 305;
    pub const S_FIRE2: u16 = 306;
    pub const S_FIRE3: u16 = 307;
    pub const S_FIRE4: u16 = 308;

    // -----------------------------------------------------------------------
    // Boss Brain states (309..314)
    // -----------------------------------------------------------------------
    pub const S_BRAIN_STND: u16 = 309;
    pub const S_BRAIN_SEE: u16 = 310;
    pub const S_BRAIN_SPIT: u16 = 311;
    pub const S_BRAIN_DIE1: u16 = 312;
    pub const S_BRAIN_DIE2: u16 = 313;
    pub const S_BRAIN_DIE3: u16 = 314;

    // -----------------------------------------------------------------------
    // Additional original-monster idle/run states (342..353)
    // -----------------------------------------------------------------------
    pub const S_POSS_STND2: u16 = 342;
    pub const S_POSS_RUN3: u16 = 343;
    pub const S_POSS_RUN4: u16 = 344;
    pub const S_SPOS_STND2: u16 = 345;
    pub const S_SPOS_RUN3: u16 = 346;
    pub const S_SPOS_RUN4: u16 = 347;
    pub const S_TROO_STND2: u16 = 348;
    pub const S_TROO_RUN3: u16 = 349;
    pub const S_TROO_RUN4: u16 = 350;
    pub const S_SARG_STND2: u16 = 351;
    pub const S_SARG_RUN3: u16 = 352;
    pub const S_SARG_RUN4: u16 = 353;
    pub const S_PLASMA_FLASH1: u16 = 354;
    pub const S_PLASMA_FLASH2: u16 = 355;
    pub const S_PUNCH5: u16 = 356;
    pub const S_SGUN5: u16 = 357;
    pub const S_CHAIN3: u16 = 358;
    pub const S_CHAIN_FLASH3: u16 = 359;
    pub const S_DSGUN8: u16 = 360;
    pub const S_DSGUN9: u16 = 361;
    pub const S_DSGUN_FLASH3: u16 = 362;
    pub const S_PLASMA3: u16 = 363;
    pub const S_PLASMA4: u16 = 364;
    pub const S_PLASMA5: u16 = 365;
    pub const S_PLAY: u16 = 366;
    pub const S_PLAY_ATK1: u16 = 367;
    pub const S_PLAY_ATK2: u16 = 368;
    pub const S_LIGHTDONE: u16 = 369;

    // -----------------------------------------------------------------------
    // Vanilla-parity states appended for demo-sync timing (370..375)
    // -----------------------------------------------------------------------
    /// Pistol refire state (vanilla `S_PISTOL4`).
    pub const S_PISTOL4: u16 = 370;
    /// Shotgun pump-animation states (vanilla `S_SGUN6..9`).
    pub const S_SGUN6: u16 = 371;
    pub const S_SGUN7: u16 = 372;
    pub const S_SGUN8: u16 = 373;
    pub const S_SGUN9: u16 = 374;
    /// Demon 6th death frame (vanilla `S_SARG_DIE6`).
    pub const S_SARG_DIE6: u16 = 375;
    /// Exploding-barrel idle loop (vanilla `S_BAR1`/`S_BAR2`).  Two 6-tic frames
    /// that cycle forever, keeping the barrel alive and `MF_SOLID` until shot.
    pub const S_BAR1: u16 = 376;
    pub const S_BAR2: u16 = 377;
    /// Exploding-barrel death animation (vanilla `S_BEXP`..`S_BEXP5`).  On death
    /// the barrel runs these five fullbright frames; `A_Explode` fires on entry
    /// to `S_BEXP4` (10-tic frame), dealing 128-radius splash damage.
    pub const S_BEXP: u16 = 378;
    pub const S_BEXP2: u16 = 379;
    pub const S_BEXP3: u16 = 380;
    pub const S_BEXP4: u16 = 381;
    pub const S_BEXP5: u16 = 382;

    /// Over-kill (gib / xdeath) chains, appended so existing state IDs are
    /// unchanged.  Entered from `P_KillMobj` when `health < -spawnhealth` and
    /// the actor has an `xdeath_state` (vanilla `info.c` `xdeathstate`).  The
    /// `A_XScream` on the second frame plays `sfx_slop` and draws no `P_Random`.
    // Trooper (MT_POSSESSED) — S_POSS_XDIE1..9 (9 frames).
    pub const S_POSS_XDIE1: u16 = 383;
    pub const S_POSS_XDIE2: u16 = 384;
    pub const S_POSS_XDIE3: u16 = 385;
    pub const S_POSS_XDIE4: u16 = 386;
    pub const S_POSS_XDIE5: u16 = 387;
    pub const S_POSS_XDIE6: u16 = 388;
    pub const S_POSS_XDIE7: u16 = 389;
    pub const S_POSS_XDIE8: u16 = 390;
    pub const S_POSS_XDIE9: u16 = 391;
    // Sergeant (MT_SHOTGUY) — S_SPOS_XDIE1..9 (9 frames).
    pub const S_SPOS_XDIE1: u16 = 392;
    pub const S_SPOS_XDIE2: u16 = 393;
    pub const S_SPOS_XDIE3: u16 = 394;
    pub const S_SPOS_XDIE4: u16 = 395;
    pub const S_SPOS_XDIE5: u16 = 396;
    pub const S_SPOS_XDIE6: u16 = 397;
    pub const S_SPOS_XDIE7: u16 = 398;
    pub const S_SPOS_XDIE8: u16 = 399;
    pub const S_SPOS_XDIE9: u16 = 400;
    // Imp (MT_TROOP) — S_TROO_XDIE1..8 (8 frames).
    pub const S_TROO_XDIE1: u16 = 401;
    pub const S_TROO_XDIE2: u16 = 402;
    pub const S_TROO_XDIE3: u16 = 403;
    pub const S_TROO_XDIE4: u16 = 404;
    pub const S_TROO_XDIE5: u16 = 405;
    pub const S_TROO_XDIE6: u16 = 406;
    pub const S_TROO_XDIE7: u16 = 407;
    pub const S_TROO_XDIE8: u16 = 408;

    /// Total number of entries in the `STATES` table.
    pub const STATES_COUNT: usize = 409;
}

// ---------------------------------------------------------------------------
// Helper macro for state entries
// ---------------------------------------------------------------------------

/// Shorthand for `MobjStateEntry` construction.
macro_rules! st {
    ($spr:expr, $fr:expr, $t:expr, $act:expr, $nxt:expr) => {
        MobjStateEntry {
            sprite: $spr,
            frame: $fr,
            tics: $t,
            action: $act,
            next_state: StateNum($nxt),
        }
    };
}

// ---------------------------------------------------------------------------
// Global state table
// ---------------------------------------------------------------------------

/// The global Mobj state machine table.
pub static STATES: &[MobjStateEntry] = &[
    // 0: S_NULL
    st!(SPR_NONE, 0, -1, NONE, ids::S_NULL),
    // === Original monsters (1..60) ===

    // --- Trooper (1..3) ---
    st!(SPR_POSS, 0, 10, LOOK, ids::S_POSS_STND2), // 1: idle A
    st!(SPR_POSS, 0, 4, CHASE, ids::S_POSS_RUN2),  // 2: run1
    st!(SPR_POSS, 1, 4, CHASE, ids::S_POSS_RUN3),  // 3: run2
    // --- Sergeant (4..6) --- vanilla S_SPOS_RUN* tics = 3
    st!(SPR_SPOS, 0, 10, LOOK, ids::S_SPOS_STND2),
    st!(SPR_SPOS, 0, 3, CHASE, ids::S_SPOS_RUN2),
    st!(SPR_SPOS, 1, 3, CHASE, ids::S_SPOS_RUN3),
    // --- Imp (7..9) --- vanilla S_TROO_RUN* tics = 3
    st!(SPR_TROO, 0, 10, LOOK, ids::S_TROO_STND2),
    st!(SPR_TROO, 0, 3, CHASE, ids::S_TROO_RUN2),
    st!(SPR_TROO, 1, 3, CHASE, ids::S_TROO_RUN3),
    // --- Demon (10..12) --- vanilla S_SARG_RUN* tics = 2
    st!(SPR_SARG, 0, 10, LOOK, ids::S_SARG_STND2),
    st!(SPR_SARG, 0, 2, CHASE, ids::S_SARG_RUN2),
    st!(SPR_SARG, 1, 2, CHASE, ids::S_SARG_RUN3),
    // --- Cacodemon (13..15) --- vanilla S_HEAD_RUN1 tics = 3
    st!(SPR_HEAD, 0, 10, LOOK, ids::S_HEAD_STND),
    st!(SPR_HEAD, 0, 3, CHASE, ids::S_HEAD_RUN2),
    st!(SPR_HEAD, 1, 3, CHASE, ids::S_HEAD_RUN1),
    // --- Baron of Hell (16..18) --- vanilla S_BOSS_RUN* tics = 3
    st!(SPR_BOSS, 0, 10, LOOK, ids::S_BOSS_STND),
    st!(SPR_BOSS, 0, 3, CHASE, ids::S_BOSS_RUN2),
    st!(SPR_BOSS, 1, 3, CHASE, ids::S_BOSS_RUN1),
    // --- Cyberdemon (19..21) --- vanilla S_CYBER_RUN* tics = 3
    st!(SPR_CYBR, 0, 10, LOOK, ids::S_CYBER_STND),
    st!(SPR_CYBR, 0, 3, CHASE, ids::S_CYBER_RUN2),
    st!(SPR_CYBR, 1, 3, CHASE, ids::S_CYBER_RUN1),
    // --- Spider Mastermind (22..24) --- vanilla S_SPID_RUN* tics = 3
    st!(SPR_SPID, 0, 10, LOOK, ids::S_SPID_STND),
    st!(SPR_SPID, 0, 3, CHASE, ids::S_SPID_RUN2),
    st!(SPR_SPID, 1, 3, CHASE, ids::S_SPID_RUN1),
    // === Death and pain states (25..48) ===
    // Trooper — death starts at WAD frame H(7), pain at G(6)
    // vanilla POSS die: 7/5/-, 8/5/Scream, 9/5/Fall, 10/5/-, 11/-1
    st!(SPR_POSS, 7, 5, NONE, ids::S_POSS_DIE2),   // 25: die1
    st!(SPR_POSS, 8, 5, SCREAM, ids::S_POSS_DIE3), // 26: die2 (A_Scream)
    st!(SPR_POSS, 6, 6, NONE, ids::S_POSS_RUN1),   // 27: pain (6 = vanilla 3+3)
    // Sergeant — vanilla SPOS die: 7/5/-, 8/5/Scream, 9/5/Fall, 10/5/-, 11/-1
    st!(SPR_SPOS, 7, 5, NONE, ids::S_SPOS_DIE2),   // 28: die1
    st!(SPR_SPOS, 8, 5, SCREAM, ids::S_SPOS_DIE3), // 29: die2 (A_Scream)
    st!(SPR_SPOS, 6, 6, NONE, ids::S_SPOS_RUN1),   // 30: pain (6 = vanilla 3+3)
    // Imp — vanilla TROO die: 8/8/-, 9/8/Scream, 10/6/-, 11/6/Fall, 12/-1
    st!(SPR_TROO, 8, 8, NONE, ids::S_TROO_DIE2),   // 31: die1
    st!(SPR_TROO, 9, 8, SCREAM, ids::S_TROO_DIE3), // 32: die2 (A_Scream)
    st!(SPR_TROO, 7, 4, NONE, ids::S_TROO_RUN1),   // 33: pain (4 = vanilla 2+2)
    // Demon — vanilla SARG die: 8/8/-, 9/8/Scream, 10/4/-, 11/4/Fall, 12/4/-, 13/-1
    st!(SPR_SARG, 8, 8, NONE, ids::S_SARG_DIE2),   // 34: die1
    st!(SPR_SARG, 9, 8, SCREAM, ids::S_SARG_DIE3), // 35: die2 (A_Scream)
    st!(SPR_SARG, 7, 4, NONE, ids::S_SARG_RUN1),   // 36: pain (4 = vanilla 2+2)
    // Cacodemon — death at E(4), pain at D(3)
    st!(SPR_HEAD, 4, 8, SCREAM, ids::S_HEAD_DIE2), // 37: die1
    st!(SPR_HEAD, 5, 8, FALL, ids::S_HEAD_DIE3),   // 38: die2 → die3
    st!(SPR_HEAD, 3, 12, NONE, ids::S_HEAD_RUN1),  // 39: pain (12 = vanilla 3+3+6)
    // Baron of Hell — same layout as Trooper/Sergeant
    st!(SPR_BOSS, 7, 8, SCREAM, ids::S_BOSS_DIE2), // 40: die1
    st!(SPR_BOSS, 8, 8, FALL, ids::S_BOSS_DIE3),   // 41: die2 → die3
    st!(SPR_BOSS, 6, 4, NONE, ids::S_BOSS_RUN1),   // 42: pain (4 = vanilla 2+2)
    // Cyberdemon — same layout
    st!(SPR_CYBR, 7, 8, SCREAM, ids::S_CYBER_DIE2), // 43: die1
    st!(SPR_CYBR, 8, 8, FALL, ids::S_CYBER_DIE3),   // 44: die2 → die3
    st!(SPR_CYBR, 6, 10, NONE, ids::S_CYBER_RUN1),  // 45: pain (10 = vanilla single)
    // Spider Mastermind — same layout
    st!(SPR_SPID, 7, 8, SCREAM, ids::S_SPID_DIE2), // 46: die1
    st!(SPR_SPID, 8, 8, FALL, ids::S_SPID_DIE3),   // 47: die2 → die3
    st!(SPR_SPID, 6, 6, NONE, ids::S_SPID_RUN1),   // 48: pain
    // === Attack states (49..60) ===
    // Trooper
    // vanilla POSS atk: 4/10/FaceTarget, 5/8/PosAttack, 4/8/-
    st!(SPR_POSS, 4, 10, FACE_TARGET, ids::S_POSS_ATK2), // 49
    st!(SPR_POSS, 5, 8, POS_ATTACK, ids::S_POSS_ATK3),   // 50
    st!(SPR_POSS, 4, 8, NONE, ids::S_POSS_RUN1),         // 51
    // vanilla SPOS atk: 4/10/FaceTarget, 5/10/SPosAttack, 4/10/-
    st!(SPR_SPOS, 4, 10, FACE_TARGET, ids::S_SPOS_ATK2), // 52
    st!(SPR_SPOS, 5, 10, SPOS_ATTACK, ids::S_SPOS_ATK3), // 53
    st!(SPR_SPOS, 4, 10, NONE, ids::S_SPOS_RUN1),        // 54
    // vanilla TROO atk: 4/8/FaceTarget, 5/8/FaceTarget, 6/6/TroopAttack
    st!(SPR_TROO, 4, 8, FACE_TARGET, ids::S_TROO_ATK2),  // 55
    st!(SPR_TROO, 5, 8, FACE_TARGET, ids::S_TROO_ATK3),  // 56
    st!(SPR_TROO, 6, 6, TROO_ATTACK, ids::S_TROO_RUN1),  // 57
    // vanilla SARG atk: 4/8/FaceTarget, 5/8/FaceTarget, 6/8/SargAttack
    st!(SPR_SARG, 4, 8, FACE_TARGET, ids::S_SARG_ATK2),  // 58
    st!(SPR_SARG, 5, 8, FACE_TARGET, ids::S_SARG_ATK3),  // 59
    st!(SPR_SARG, 6, 8, SARG_ATTACK, ids::S_SARG_RUN1),  // 60
    // ===================================================================
    // Projectile states (61..103)
    // ===================================================================
    // Imp fireball (BAL1) -- fly
    st!(SPR_BAL1, FB, 4, NONE, ids::S_TBALL2),     // 61
    st!(SPR_BAL1, 1 | FB, 4, NONE, ids::S_TBALL1), // 62
    // Imp fireball -- death
    st!(SPR_BAL1, 2 | FB, 6, NONE, ids::S_TBALLX2), // 63
    st!(SPR_BAL1, 3 | FB, 6, NONE, ids::S_TBALLX3), // 64
    st!(SPR_BAL1, 4 | FB, 6, NONE, ids::S_NULL),    // 65
    // Baron/HK fireball (BAL2) -- fly
    st!(SPR_BAL2, FB, 4, NONE, ids::S_BRBALL2),     // 66
    st!(SPR_BAL2, 1 | FB, 4, NONE, ids::S_BRBALL1), // 67
    // Baron/HK fireball -- death
    st!(SPR_BAL2, 2 | FB, 6, NONE, ids::S_BRBALLX2), // 68
    st!(SPR_BAL2, 3 | FB, 6, NONE, ids::S_BRBALLX3), // 69
    st!(SPR_BAL2, 4 | FB, 6, NONE, ids::S_NULL),     // 70
    // Rocket (MISL) -- fly
    st!(SPR_MISL, FB, 1, NONE, ids::S_ROCKET), // 71
    // Rocket -- death
    st!(SPR_MISL, 1 | FB, 8, EXPLODE, ids::S_EXPLODE2), // 72
    st!(SPR_MISL, 2 | FB, 6, NONE, ids::S_EXPLODE3), // 73
    st!(SPR_MISL, 3 | FB, 4, NONE, ids::S_NULL),     // 74
    // Plasma ball (PLSS) -- fly
    st!(SPR_PLSS, FB, 6, NONE, ids::S_PLASBALL2), // 75
    st!(SPR_PLSS, 1 | FB, 6, NONE, ids::S_PLASBALL1), // 76
    // Plasma -- death (PLSE)
    st!(SPR_PLSE, FB, 4, NONE, ids::S_PLASEXP2),     // 77
    st!(SPR_PLSE, 1 | FB, 4, NONE, ids::S_PLASEXP3), // 78
    st!(SPR_PLSE, 2 | FB, 4, NONE, ids::S_PLASEXP4), // 79
    st!(SPR_PLSE, 3 | FB, 4, NONE, ids::S_NULL),     // 80
    // BFG ball (BFS1) -- fly
    st!(SPR_BFS1, FB, 4, NONE, ids::S_BFGSHOT2),     // 81
    st!(SPR_BFS1, 1 | FB, 4, NONE, ids::S_BFGSHOT1), // 82
    // BFG -- explode (BFE1)
    st!(SPR_BFE1, FB, 8, NONE, ids::S_BFGLAND2),     // 83
    st!(SPR_BFE1, 1 | FB, 8, NONE, ids::S_BFGLAND3), // 84
    st!(SPR_BFE1, 2 | FB, 8, NONE, ids::S_BFGLAND4), // 85
    st!(SPR_BFE1, 3 | FB, 8, NONE, ids::S_BFGLAND5), // 86
    st!(SPR_BFE1, 4 | FB, 8, NONE, ids::S_BFGLAND6), // 87
    st!(SPR_BFE1, 5 | FB, 8, NONE, ids::S_NULL),     // 88
    // Revenant tracer (RSKE) -- fly
    st!(SPR_RSKE, FB, 2, NONE, ids::S_TRACER2),     // 89
    st!(SPR_RSKE, 1 | FB, 2, NONE, ids::S_TRACER1), // 90
    // Revenant tracer -- death
    st!(SPR_RSKE, 2 | FB, 6, NONE, ids::S_TRACEEXP2), // 91
    st!(SPR_RSKE, 3 | FB, 6, NONE, ids::S_TRACEEXP3), // 92
    st!(SPR_RSKE, 4 | FB, 6, NONE, ids::S_NULL),      // 93
    // Arachnotron plasma (APLS) -- fly
    st!(SPR_APLS, FB, 5, NONE, ids::S_ARACH_PLAZ2), // 94
    st!(SPR_APLS, 1 | FB, 5, NONE, ids::S_ARACH_PLAZ1), // 95
    // Arachnotron plasma -- death (APBX)
    st!(SPR_APBX, FB, 5, NONE, ids::S_ARACH_PLEX2), // 96
    st!(SPR_APBX, 1 | FB, 5, NONE, ids::S_ARACH_PLEX3), // 97
    st!(SPR_APBX, 2 | FB, 5, NONE, ids::S_NULL),    // 98
    // Mancubus fireball (FATB) -- fly
    st!(SPR_FATB, FB, 4, NONE, ids::S_FATSHOT2),     // 99
    st!(SPR_FATB, 1 | FB, 4, NONE, ids::S_FATSHOT1), // 100
    // Mancubus fireball -- death (MANF)
    st!(SPR_MANF, FB, 8, NONE, ids::S_FATSHOTX2), // 101
    st!(SPR_MANF, 1 | FB, 6, NONE, ids::S_FATSHOTX3), // 102
    st!(SPR_MANF, 2 | FB, 4, NONE, ids::S_NULL),  // 103
    // ===================================================================
    // Effect states (104..120)
    // ===================================================================
    // Bullet puff
    st!(SPR_PUFF, FB, 4, NONE, ids::S_PUFF2), // 104
    st!(SPR_PUFF, 1, 4, NONE, ids::S_PUFF3),  // 105
    st!(SPR_PUFF, 2, 4, NONE, ids::S_PUFF4),  // 106
    st!(SPR_PUFF, 3, 4, NONE, ids::S_NULL),   // 107
    // Blood splat
    st!(SPR_BLUD, 2, 8, NONE, ids::S_BLOOD2), // 108
    st!(SPR_BLUD, 1, 8, NONE, ids::S_BLOOD3), // 109
    st!(SPR_BLUD, 0, 8, NONE, ids::S_NULL),   // 110
    // Teleport fog
    st!(SPR_TFOG, FB, 6, NONE, ids::S_TFOG2),     // 111
    st!(SPR_TFOG, 1 | FB, 6, NONE, ids::S_TFOG3), // 112
    st!(SPR_TFOG, FB, 6, NONE, ids::S_TFOG4),     // 113
    st!(SPR_TFOG, 1 | FB, 6, NONE, ids::S_TFOG5), // 114
    st!(SPR_TFOG, 2 | FB, 6, NONE, ids::S_NULL),  // 115
    // Item respawn fog
    st!(SPR_IFOG, FB, 6, NONE, ids::S_IFOG2),     // 116
    st!(SPR_IFOG, 1 | FB, 6, NONE, ids::S_IFOG3), // 117
    st!(SPR_IFOG, FB, 6, NONE, ids::S_IFOG4),     // 118
    st!(SPR_IFOG, 1 | FB, 6, NONE, ids::S_IFOG5), // 119
    st!(SPR_IFOG, 2 | FB, 6, NONE, ids::S_NULL),  // 120
    // ===================================================================
    // Doom 2 monster states (121..234)
    // ===================================================================

    // --- Lost Soul (121..135) ---
    st!(SPR_SKUL, FB, 10, LOOK, ids::S_SKULL_STND2), // 121
    st!(SPR_SKUL, 1 | FB, 10, LOOK, ids::S_SKULL_STND), // 122
    st!(SPR_SKUL, FB, 6, CHASE, ids::S_SKULL_RUN2),  // 123
    st!(SPR_SKUL, 1 | FB, 6, CHASE, ids::S_SKULL_RUN3), // 124
    st!(SPR_SKUL, 2 | FB, 6, CHASE, ids::S_SKULL_RUN4), // 125
    st!(SPR_SKUL, 3 | FB, 6, CHASE, ids::S_SKULL_RUN1), // 126
    st!(SPR_SKUL, 4 | FB, 4, NONE, ids::S_SKULL_ATK2), // 127
    st!(SPR_SKUL, 5 | FB, 4, SKULL_ATTACK, ids::S_SKULL_ATK3), // 128
    st!(SPR_SKUL, 6 | FB, 4, NONE, ids::S_SKULL_RUN1), // 129
    st!(SPR_SKUL, 7, 3, NONE, ids::S_SKULL_RUN1),    // 130: pain
    st!(SPR_SKUL, 8 | FB, 6, NONE, ids::S_SKULL_DIE2), // 131: die1
    st!(SPR_SKUL, 9 | FB, 6, FALL, ids::S_SKULL_DIE3), // 132
    st!(SPR_SKUL, 10 | FB, 6, NONE, ids::S_SKULL_DIE4), // 133
    st!(SPR_SKUL, 11 | FB, 6, NONE, ids::S_SKULL_DIE5), // 134
    st!(SPR_SKUL, 12, -1, NONE, ids::S_NULL),        // 135
    // --- Arachnotron (136..150) ---
    st!(SPR_BSPI, 0, 10, LOOK, ids::S_BSPI_STND2), // 136
    st!(SPR_BSPI, 1, 10, LOOK, ids::S_BSPI_STND),  // 137
    st!(SPR_BSPI, 0, 3, CHASE, ids::S_BSPI_RUN2),  // 138
    st!(SPR_BSPI, 0, 3, CHASE, ids::S_BSPI_RUN3),  // 139
    st!(SPR_BSPI, 1, 3, CHASE, ids::S_BSPI_RUN4),  // 140
    st!(SPR_BSPI, 1, 3, CHASE, ids::S_BSPI_RUN1),  // 141
    st!(SPR_BSPI, 2, 10, NONE, ids::S_BSPI_ATK2),  // 142
    st!(SPR_BSPI, 3 | FB, 4, BSPI_ATTACK, ids::S_BSPI_ATK3), // 143
    st!(SPR_BSPI, 2, 4, NONE, ids::S_BSPI_RUN1),   // 144
    st!(SPR_BSPI, 4, 3, NONE, ids::S_BSPI_RUN1),   // 145: pain
    st!(SPR_BSPI, 5, 8, NONE, ids::S_BSPI_DIE2),   // 146: die1
    st!(SPR_BSPI, 6, 5, FALL, ids::S_BSPI_DIE3),   // 147
    st!(SPR_BSPI, 7, 5, NONE, ids::S_BSPI_DIE4),   // 148
    st!(SPR_BSPI, 8, 5, NONE, ids::S_BSPI_DIE5),   // 149
    st!(SPR_BSPI, 9, -1, NONE, ids::S_NULL),       // 150
    // --- Pain Elemental (151..165) ---
    st!(SPR_PAIN, 0, 10, LOOK, ids::S_PAIN_STND2), // 151
    st!(SPR_PAIN, 1, 10, LOOK, ids::S_PAIN_STND),  // 152
    st!(SPR_PAIN, 0, 3, CHASE, ids::S_PAIN_RUN2),  // 153
    st!(SPR_PAIN, 0, 3, CHASE, ids::S_PAIN_RUN3),  // 154
    st!(SPR_PAIN, 1, 3, CHASE, ids::S_PAIN_RUN4),  // 155
    st!(SPR_PAIN, 1, 3, CHASE, ids::S_PAIN_RUN1),  // 156
    st!(SPR_PAIN, 2, 5, NONE, ids::S_PAIN_ATK2),   // 157
    st!(SPR_PAIN, 3, 5, PAIN_ATTACK, ids::S_PAIN_ATK3), // 158
    st!(SPR_PAIN, 4, 5, NONE, ids::S_PAIN_RUN1),   // 159
    st!(SPR_PAIN, 5, 6, NONE, ids::S_PAIN_RUN1),   // 160: pain
    st!(SPR_PAIN, 6, 8, NONE, ids::S_PAIN_DIE2),   // 161: die1
    st!(SPR_PAIN, 7, 8, FALL, ids::S_PAIN_DIE3),   // 162
    st!(SPR_PAIN, 8, 8, NONE, ids::S_PAIN_DIE4),   // 163
    st!(SPR_PAIN, 9, 8, NONE, ids::S_PAIN_DIE5),   // 164
    st!(SPR_PAIN, 10, -1, NONE, ids::S_NULL),      // 165
    // --- Revenant (166..180) ---
    st!(SPR_SKEL, 0, 10, LOOK, ids::S_SKEL_STND2), // 166
    st!(SPR_SKEL, 1, 10, LOOK, ids::S_SKEL_STND),  // 167
    st!(SPR_SKEL, 0, 2, CHASE, ids::S_SKEL_RUN2),  // 168
    st!(SPR_SKEL, 1, 2, CHASE, ids::S_SKEL_RUN3),  // 169
    st!(SPR_SKEL, 2, 2, CHASE, ids::S_SKEL_RUN4),  // 170
    st!(SPR_SKEL, 3, 2, CHASE, ids::S_SKEL_RUN1),  // 171
    st!(SPR_SKEL, 4, 6, NONE, ids::S_SKEL_ATK2),   // 172
    st!(SPR_SKEL, 5, 6, SKEL_MISSILE, ids::S_SKEL_ATK3), // 173
    st!(SPR_SKEL, 4, 6, NONE, ids::S_SKEL_RUN1),   // 174
    st!(SPR_SKEL, 6, 5, NONE, ids::S_SKEL_RUN1),   // 175: pain
    st!(SPR_SKEL, 7, 7, NONE, ids::S_SKEL_DIE2),   // 176: die1
    st!(SPR_SKEL, 8, 7, FALL, ids::S_SKEL_DIE3),   // 177
    st!(SPR_SKEL, 9, 7, NONE, ids::S_SKEL_DIE4),   // 178
    st!(SPR_SKEL, 10, 7, NONE, ids::S_SKEL_DIE5),  // 179
    st!(SPR_SKEL, 11, -1, NONE, ids::S_NULL),      // 180
    // --- Mancubus (181..195) ---
    st!(SPR_FATT, 0, 15, LOOK, ids::S_FATT_STND2), // 181
    st!(SPR_FATT, 1, 15, LOOK, ids::S_FATT_STND),  // 182
    st!(SPR_FATT, 0, 4, CHASE, ids::S_FATT_RUN2),  // 183
    st!(SPR_FATT, 1, 4, CHASE, ids::S_FATT_RUN3),  // 184
    st!(SPR_FATT, 2, 4, CHASE, ids::S_FATT_RUN4),  // 185
    st!(SPR_FATT, 3, 4, CHASE, ids::S_FATT_RUN1),  // 186
    st!(SPR_FATT, 4, 10, NONE, ids::S_FATT_ATK2),  // 187
    st!(SPR_FATT, 5 | FB, 8, FAT_ATTACK1, ids::S_FATT_ATK3), // 188
    st!(SPR_FATT, 4, 5, NONE, ids::S_FATT_RUN1),   // 189
    st!(SPR_FATT, 6, 3, NONE, ids::S_FATT_RUN1),   // 190: pain
    st!(SPR_FATT, 7, 8, NONE, ids::S_FATT_DIE2),   // 191: die1
    st!(SPR_FATT, 8, 8, FALL, ids::S_FATT_DIE3),   // 192
    st!(SPR_FATT, 9, 8, NONE, ids::S_FATT_DIE4),   // 193
    st!(SPR_FATT, 10, 8, NONE, ids::S_FATT_DIE5),  // 194
    st!(SPR_FATT, 11, -1, NONE, ids::S_NULL),      // 195
    // --- Arch-Vile (196..210) ---
    st!(SPR_VILE, 0, 10, LOOK, ids::S_VILE_STND2), // 196
    st!(SPR_VILE, 1, 10, LOOK, ids::S_VILE_STND),  // 197
    st!(SPR_VILE, 0, 2, VILE_CHASE, ids::S_VILE_RUN2), // 198
    st!(SPR_VILE, 1, 2, VILE_CHASE, ids::S_VILE_RUN3), // 199
    st!(SPR_VILE, 2, 2, VILE_CHASE, ids::S_VILE_RUN4), // 200
    st!(SPR_VILE, 3, 2, VILE_CHASE, ids::S_VILE_RUN1), // 201
    st!(SPR_VILE, 4, 10, VILE_START, ids::S_VILE_ATK2), // 202: ATK1 → VILE_START
    st!(SPR_VILE, 5, 10, VILE_TARGET, ids::S_VILE_ATK3), // 203: ATK2 → VILE_TARGET
    st!(SPR_VILE, 6, 10, VILE_ATTACK, ids::S_VILE_RUN1), // 204: ATK3 → VILE_ATTACK
    st!(SPR_VILE, 7, 5, NONE, ids::S_VILE_RUN1),   // 205: pain
    st!(SPR_VILE, 8, 7, NONE, ids::S_VILE_DIE2),   // 206: die1
    st!(SPR_VILE, 9, 7, FALL, ids::S_VILE_DIE3),   // 207
    st!(SPR_VILE, 10, 7, NONE, ids::S_VILE_DIE4),  // 208
    st!(SPR_VILE, 11, 7, NONE, ids::S_VILE_DIE5),  // 209
    st!(SPR_VILE, 12, -1, NONE, ids::S_NULL),      // 210
    // --- Chaingunner (211..225) ---
    st!(SPR_CPOS, 0, 10, LOOK, ids::S_CPOS_STND2), // 211
    st!(SPR_CPOS, 1, 10, LOOK, ids::S_CPOS_STND),  // 212
    st!(SPR_CPOS, 0, 3, CHASE, ids::S_CPOS_RUN2),  // 213
    st!(SPR_CPOS, 1, 3, CHASE, ids::S_CPOS_RUN3),  // 214
    st!(SPR_CPOS, 2, 3, CHASE, ids::S_CPOS_RUN4),  // 215
    st!(SPR_CPOS, 3, 3, CHASE, ids::S_CPOS_RUN1),  // 216
    st!(SPR_CPOS, 4, 4, NONE, ids::S_CPOS_ATK2),   // 217
    st!(SPR_CPOS, 5 | FB, 4, CPOS_ATTACK, ids::S_CPOS_ATK3), // 218
    st!(SPR_CPOS, 4, 4, NONE, ids::S_CPOS_RUN1),   // 219
    st!(SPR_CPOS, 6, 3, NONE, ids::S_CPOS_RUN1),   // 220: pain
    st!(SPR_CPOS, 7, 5, NONE, ids::S_CPOS_DIE2),   // 221: die1
    st!(SPR_CPOS, 8, 5, FALL, ids::S_CPOS_DIE3),   // 222
    st!(SPR_CPOS, 9, 5, NONE, ids::S_CPOS_DIE4),   // 223
    st!(SPR_CPOS, 10, 5, NONE, ids::S_CPOS_DIE5),  // 224
    st!(SPR_CPOS, 11, -1, NONE, ids::S_NULL),      // 225
    // --- Hell Knight own states (226..234) ---
    st!(SPR_BOS2, 0, 10, LOOK, ids::S_BOS2_STND), // 226
    st!(SPR_BOS2, 0, 4, CHASE, ids::S_BOS2_RUN2), // 227
    st!(SPR_BOS2, 1, 4, CHASE, ids::S_BOS2_RUN1), // 228
    st!(SPR_BOS2, 2, 4, NONE, ids::S_BOS2_ATK2),  // 229
    st!(SPR_BOS2, 3, 4, BRUIS_ATTACK, ids::S_BOS2_ATK3), // 230
    st!(SPR_BOS2, 2, 4, NONE, ids::S_BOS2_RUN1),  // 231
    st!(SPR_BOS2, 6, 4, NONE, ids::S_BOS2_RUN1),  // 232: pain (4 = vanilla 2+2)
    st!(SPR_BOS2, 7, 8, SCREAM, ids::S_BOS2_DIE2), // 233: die1
    st!(SPR_BOS2, 8, 8, FALL, ids::S_BOS2_DIE3),  // 234: die2 → die3
    // ===================================================================
    // Weapon states (235..304)
    // ===================================================================

    // --- Fist (PUNG) 235..241 ---
    st!(SPR_PUNG, 0, 1, RAISE, ids::S_PUNCH_UP), // 235: up
    st!(SPR_PUNG, 0, 1, LOWER, ids::S_PUNCH_DOWN), // 236: down
    st!(SPR_PUNG, 0, 1, WEAPON_READY, ids::S_PUNCH_READY), // 237: ready
    st!(SPR_PUNG, 1, 4, PUNCH, ids::S_PUNCH2),   // 238: fire1
    st!(SPR_PUNG, 2, 4, NONE, ids::S_PUNCH3),    // 239: fire2
    st!(SPR_PUNG, 3, 5, NONE, ids::S_PUNCH4),    // 240: fire3
    st!(SPR_PUNG, 2, 4, NONE, ids::S_PUNCH5),    // 241: fire4
    // --- Pistol (PISG) 242..249 ---
    st!(SPR_PISG, 0, 1, RAISE, ids::S_PISTOL_UP), // 242: up
    st!(SPR_PISG, 0, 1, LOWER, ids::S_PISTOL_DOWN), // 243: down
    st!(SPR_PISG, 0, 1, WEAPON_READY, ids::S_PISTOL_READY), // 244: ready
    // vanilla pistol fire: PISTOL1 0/4/-, PISTOL2 1/6/FirePistol, PISTOL3 2/4/-, PISTOL4 1/5/ReFire
    st!(SPR_PISG, 0, 4, NONE, ids::S_PISTOL2),        // 245: fire1 (A_FirePistol on fire2)
    st!(SPR_PISG, 1, 6, FIRE_PISTOL, ids::S_PISTOL3), // 246: fire2
    st!(SPR_PISG, 2, 4, NONE, ids::S_PISTOL4),        // 247: fire3
    st!(SPR_PISG, 3 | FB, 7, LIGHT1, ids::S_PISTOL_FLASH2), // 248: flash1
    st!(SPR_PISG, 4 | FB, 7, NONE, ids::S_LIGHTDONE), // 249: flash2
    // --- Shotgun (SHTG) 250..258 ---
    st!(SPR_SHTG, 0, 1, RAISE, ids::S_SGUN_UP), // 250: up
    st!(SPR_SHTG, 0, 1, LOWER, ids::S_SGUN_DOWN), // 251: down
    st!(SPR_SHTG, 0, 1, WEAPON_READY, ids::S_SGUN_READY), // 252: ready
    // vanilla shotgun fire: SGUN1 0/3/-, SGUN2 0/7/FireShotgun, SGUN3 1/5/-, SGUN4 2/5/-,
    // SGUN5 3/4/-, SGUN6 2/5/-, SGUN7 1/5/-, SGUN8 0/3/-, SGUN9 0/7/ReFire
    st!(SPR_SHTG, 0, 3, NONE, ids::S_SGUN2),         // 253: fire1
    st!(SPR_SHTG, 0, 7, FIRE_SHOTGUN, ids::S_SGUN3), // 254: fire2 (A_FireShotgun)
    st!(SPR_SHTG, 1, 5, NONE, ids::S_SGUN4),         // 255: fire3
    st!(SPR_SHTG, 2, 5, NONE, ids::S_SGUN5),         // 256: fire4
    st!(SPR_SHTG, 4 | FB, 4, LIGHT1, ids::S_SGUN_FLASH2), // 257: flash1
    st!(SPR_SHTG, 5 | FB, 3, LIGHT2, ids::S_LIGHTDONE), // 258: flash2
    // --- SSG (SHT2) 259..270 ---
    st!(SPR_SHT2, 0, 1, RAISE, ids::S_DSGUN_UP), // 259: up
    st!(SPR_SHT2, 0, 1, LOWER, ids::S_DSGUN_DOWN), // 260: down
    st!(SPR_SHT2, 0, 1, WEAPON_READY, ids::S_DSGUN_READY), // 261: ready
    st!(SPR_SHT2, 1, 3, FIRE_SHOTGUN2, ids::S_DSGUN2), // 262: fire1
    st!(SPR_SHT2, 2, 7, NONE, ids::S_DSGUN3),    // 263: fire2
    st!(SPR_SHT2, 3, 7, NONE, ids::S_DSGUN4),    // 264: fire3
    st!(SPR_SHT2, 4, 7, CHECK_RELOAD, ids::S_DSGUN5), // 265: fire4
    st!(SPR_SHT2, 5, 6, OPEN_SHOTGUN2, ids::S_DSGUN6), // 266: fire5
    st!(SPR_SHT2, 6, 6, LOAD_SHOTGUN2, ids::S_DSGUN7), // 267: fire6
    st!(SPR_SHT2, 7, 5, CLOSE_SHOTGUN2, ids::S_DSGUN9), // 268: fire7
    st!(SPR_SHT2, 1 | FB, 5, LIGHT1, ids::S_DSGUN_FLASH2), // 269: flash1
    st!(SPR_SHT2, 2 | FB, 4, LIGHT2, ids::S_DSGUN_FLASH3), // 270: flash2
    // --- Chaingun (CHGG) 271..277 ---
    st!(SPR_CHGG, 0, 1, RAISE, ids::S_CHAIN_UP), // 271: up
    st!(SPR_CHGG, 0, 1, LOWER, ids::S_CHAIN_DOWN), // 272: down
    st!(SPR_CHGG, 0, 1, WEAPON_READY, ids::S_CHAIN_READY), // 273: ready
    st!(SPR_CHGG, 0, 4, FIRE_CGUN, ids::S_CHAIN2), // 274: fire1
    st!(SPR_CHGG, 1, 4, FIRE_CGUN, ids::S_CHAIN3), // 275: fire2
    st!(SPR_CHGG, 1 | FB, 5, LIGHT1, ids::S_CHAIN_FLASH2), // 276: flash1
    st!(SPR_CHGG, 2 | FB, 5, LIGHT2, ids::S_CHAIN_FLASH3), // 277: flash2
    // --- Rocket launcher (ROCK) 278..285 ---
    st!(SPR_ROCK, 0, 1, RAISE, ids::S_MISSILE_UP), // 278: up
    st!(SPR_ROCK, 0, 1, LOWER, ids::S_MISSILE_DOWN), // 279: down
    st!(SPR_ROCK, 0, 1, WEAPON_READY, ids::S_MISSILE_READY), // 280: ready
    st!(SPR_ROCK, 1, 8, GUN_FLASH, ids::S_MISSILE2), // 281: fire1
    st!(SPR_ROCK, 2, 12, FIRE_MISSILE, ids::S_MISSILE3), // 282: fire2
    st!(SPR_ROCK, 1, 0, REFIRE, ids::S_MISSILE_READY), // 283: fire3 (vanilla A_ReFire)
    st!(SPR_ROCK, 3 | FB, 3, LIGHT1, ids::S_MISSILE_FLASH2), // 284: flash1
    st!(SPR_ROCK, 4 | FB, 4, LIGHT2, ids::S_LIGHTDONE), // 285: flash2
    // --- Plasma gun (PLSG) 286..290 ---
    st!(SPR_PLSG, 0, 1, RAISE, ids::S_PLASMA_UP), // 286: up
    st!(SPR_PLSG, 0, 1, LOWER, ids::S_PLASMA_DOWN), // 287: down
    st!(SPR_PLSG, 0, 1, WEAPON_READY, ids::S_PLASMA_READY), // 288: ready
    st!(SPR_PLSG, 0, 3, FIRE_PLASMA, ids::S_PLASMA2), // 289: fire1
    st!(SPR_PLSG, 1, 3, NONE, ids::S_PLASMA3),    // 290: fire2
    // --- BFG (BFGG) 291..297 ---
    st!(SPR_BFGG, 0, 1, RAISE, ids::S_BFG_UP),   // 291: up
    st!(SPR_BFGG, 0, 1, LOWER, ids::S_BFG_DOWN), // 292: down
    st!(SPR_BFGG, 0, 1, WEAPON_READY, ids::S_BFG_READY), // 293: ready
    st!(SPR_BFGG, 0, 20, BFG_SOUND, ids::S_BFG2), // 294: fire1
    st!(SPR_BFGG, 1, 10, FIRE_BFG, ids::S_BFG_READY), // 295: fire2
    st!(SPR_BFGG, 1 | FB, 11, LIGHT1, ids::S_BFG_FLASH2), // 296: flash1
    st!(SPR_BFGG, 2 | FB, 6, LIGHT2, ids::S_LIGHTDONE), // 297: flash2
    // --- Chainsaw (SAWG) 298..304 ---
    st!(SPR_SAWG, 0, 1, RAISE, ids::S_SAW_UP),   // 298: up
    st!(SPR_SAWG, 0, 1, LOWER, ids::S_SAW_DOWN), // 299: down
    st!(SPR_SAWG, 0, 1, WEAPON_READY, ids::S_SAW_READY2), // 300: ready1
    st!(SPR_SAWG, 1, 1, WEAPON_READY, ids::S_SAW_READY1), // 301: ready2
    st!(SPR_SAWG, 0, 4, SAW, ids::S_SAW2),       // 302: fire1
    st!(SPR_SAWG, 1, 4, SAW, ids::S_SAW3),       // 303: fire2
    st!(SPR_SAWG, 1, 0, REFIRE, ids::S_SAW_READY1), // 304: fire3
    // ===================================================================
    // Fire column states (305..308) — Arch-Vile fire effect
    // ===================================================================
    st!(SPR_FIRE, FB, 2, FIRE, ids::S_FIRE2), // 305: FIRE1
    st!(SPR_FIRE, 1 | FB, 2, FIRE, ids::S_FIRE3), // 306: FIRE2
    st!(SPR_FIRE, 2 | FB, 2, FIRE, ids::S_FIRE4), // 307: FIRE3
    st!(SPR_FIRE, 3 | FB, 2, FIRE, ids::S_FIRE1), // 308: FIRE4 → loops
    // ===================================================================
    // Boss Brain states (309..314)
    // ===================================================================
    st!(SPR_BBRN, 0, -1, NONE, ids::S_NULL), // 309: BRAIN_STND (idle)
    st!(SPR_BBRN, 0, 150, BRAIN_AWAKE, ids::S_BRAIN_SPIT), // 310: BRAIN_SEE
    st!(SPR_BBRN, 1, 10, BRAIN_SPIT, ids::S_BRAIN_SEE), // 311: BRAIN_SPIT
    st!(SPR_BBRN, 2, 8, BRAIN_SCREAM, ids::S_BRAIN_DIE2), // 312: BRAIN_DIE1
    st!(SPR_BBRN, 3, 8, BRAIN_EXPLODE, ids::S_BRAIN_DIE3), // 313: BRAIN_DIE2
    st!(SPR_BBRN, 4, -1, BRAIN_DIE, ids::S_NULL), // 314: BRAIN_DIE3
    // ===================================================================
    // Extended death frames for original 8 monsters (315..338)
    // ===================================================================
    // Trooper DIE3-5: vanilla 9/5/Fall, 10/5/-, 11/-1
    st!(SPR_POSS, 9, 5, FALL, ids::S_POSS_DIE4),  // 315
    st!(SPR_POSS, 10, 5, NONE, ids::S_POSS_DIE5), // 316
    st!(SPR_POSS, 11, -1, NONE, ids::S_NULL),     // 317
    // Sergeant DIE3-5: vanilla 9/5/Fall, 10/5/-, 11/-1
    st!(SPR_SPOS, 9, 5, FALL, ids::S_SPOS_DIE4),  // 318
    st!(SPR_SPOS, 10, 5, NONE, ids::S_SPOS_DIE5), // 319
    st!(SPR_SPOS, 11, -1, NONE, ids::S_NULL),     // 320
    // Imp DIE3-5: vanilla 10/6/-, 11/6/Fall, 12/-1
    st!(SPR_TROO, 10, 6, NONE, ids::S_TROO_DIE4), // 321
    st!(SPR_TROO, 11, 6, FALL, ids::S_TROO_DIE5), // 322
    st!(SPR_TROO, 12, -1, NONE, ids::S_NULL),     // 323
    // Demon DIE3-6: vanilla 10/4/-, 11/4/Fall, 12/4/-, 13/-1
    st!(SPR_SARG, 10, 4, NONE, ids::S_SARG_DIE4), // 324
    st!(SPR_SARG, 11, 4, FALL, ids::S_SARG_DIE5), // 325
    st!(SPR_SARG, 12, 4, NONE, ids::S_SARG_DIE6), // 326
    // Cacodemon DIE3-5 (frames G/H/I = 6/7/8)
    st!(SPR_HEAD, 6, 8, NONE, ids::S_HEAD_DIE4), // 327
    st!(SPR_HEAD, 7, 8, NONE, ids::S_HEAD_DIE5), // 328
    st!(SPR_HEAD, 8, -1, NONE, ids::S_NULL),     // 329
    // Baron of Hell DIE3-5 (frames I/J/K = 9/10/11)
    st!(SPR_BOSS, 9, 8, NONE, ids::S_BOSS_DIE4),  // 330
    st!(SPR_BOSS, 10, 8, NONE, ids::S_BOSS_DIE5), // 331
    st!(SPR_BOSS, 11, -1, NONE, ids::S_NULL),     // 332
    // Cyberdemon DIE3-5 (frames I/J/K = 9/10/11)
    st!(SPR_CYBR, 9, 8, NONE, ids::S_CYBER_DIE4),  // 333
    st!(SPR_CYBR, 10, 8, NONE, ids::S_CYBER_DIE5), // 334
    st!(SPR_CYBR, 11, -1, NONE, ids::S_NULL),      // 335
    // Spider Mastermind DIE3-5 (frames I/J/K = 9/10/11)
    st!(SPR_SPID, 9, 8, NONE, ids::S_SPID_DIE4),  // 336
    st!(SPR_SPID, 10, 8, NONE, ids::S_SPID_DIE5), // 337
    st!(SPR_SPID, 11, -1, NONE, ids::S_NULL),     // 338
    // ===================================================================
    // Extended death frames for Hell Knight BOS2 (339..341)
    // ===================================================================
    st!(SPR_BOS2, 9, 8, NONE, ids::S_BOS2_DIE4),  // 339
    st!(SPR_BOS2, 10, 8, NONE, ids::S_BOS2_DIE5), // 340
    st!(SPR_BOS2, 11, -1, NONE, ids::S_NULL),     // 341
    // ===================================================================
    // Additional original-monster idle/run states (342..353)
    // ===================================================================
    st!(SPR_POSS, 1, 10, LOOK, ids::S_POSS_STND), // 342: idle B
    st!(SPR_POSS, 2, 4, CHASE, ids::S_POSS_RUN4), // 343: run3 (C)
    st!(SPR_POSS, 3, 4, CHASE, ids::S_POSS_RUN1), // 344: run4 (D)
    st!(SPR_SPOS, 1, 10, LOOK, ids::S_SPOS_STND), // 345: idle B
    st!(SPR_SPOS, 2, 3, CHASE, ids::S_SPOS_RUN4), // 346: run3 (C) vanilla tics=3
    st!(SPR_SPOS, 3, 3, CHASE, ids::S_SPOS_RUN1), // 347: run4 (D) vanilla tics=3
    st!(SPR_TROO, 1, 10, LOOK, ids::S_TROO_STND), // 348: idle B
    st!(SPR_TROO, 2, 3, CHASE, ids::S_TROO_RUN4), // 349: run3 (C) vanilla tics=3
    st!(SPR_TROO, 3, 3, CHASE, ids::S_TROO_RUN1), // 350: run4 (D) vanilla tics=3
    st!(SPR_SARG, 1, 10, LOOK, ids::S_SARG_STND), // 351: idle B
    st!(SPR_SARG, 2, 2, CHASE, ids::S_SARG_RUN4), // 352: run3 (C) vanilla tics=2
    st!(SPR_SARG, 3, 2, CHASE, ids::S_SARG_RUN1), // 353: run4 (D) vanilla tics=2
    st!(SPR_PLAS, FB, 4, LIGHT1, ids::S_LIGHTDONE), // 354: plasma flash1
    st!(SPR_PLAS, 1 | FB, 4, LIGHT1, ids::S_LIGHTDONE), // 355: plasma flash2
    // ===================================================================
    // Additional psprite parity states (356..365)
    // ===================================================================
    st!(SPR_PUNG, 0, 5, REFIRE, ids::S_PUNCH_READY), // 356: punch5
    st!(SPR_SHTG, 3, 4, NONE, ids::S_SGUN6), // 357: fire5 (vanilla SGUN5)
    st!(SPR_CHGG, 0, 0, REFIRE, ids::S_CHAIN_READY), // 358: chain3
    st!(SPR_CHGG, FB, 4, NONE, ids::S_LIGHTDONE),    // 359: chain flash3
    st!(SPR_SHT2, 0, 5, REFIRE, ids::S_DSGUN9),      // 360: dsgun8
    st!(SPR_SHT2, 0, 1, WEAPON_READY, ids::S_DSGUN_READY), // 361: dsgun9
    st!(SPR_SHT2, 3 | FB, 5, NONE, ids::S_LIGHTDONE), // 362: dsgun flash3
    st!(SPR_PLSG, 0, 3, FIRE_PLASMA, ids::S_PLASMA4), // 363: plasma3
    st!(SPR_PLSG, 1, 3, NONE, ids::S_PLASMA5),       // 364: plasma4
    st!(SPR_PLSG, 0, 0, REFIRE, ids::S_PLASMA_READY), // 365: plasma5
    // ===================================================================
    // Player presentation and light cleanup parity states (366..369)
    // ===================================================================
    st!(SPR_PLAY, 0, -1, NONE, ids::S_PLAY), // 366: player normal
    st!(SPR_PLAY, 3, -1, NONE, ids::S_PLAY_ATK1), // 367: player attack1
    st!(SPR_PLAY, 4, -1, NONE, ids::S_PLAY_ATK2), // 368: player attack2
    st!(SPR_NONE, 0, 0, LIGHT0, ids::S_NULL), // 369: lightdone
    // ===================================================================
    // Vanilla-parity states appended for demo-sync timing (370..375)
    // ===================================================================
    st!(SPR_PISG, 1, 5, REFIRE, ids::S_PISTOL_READY), // 370: S_PISTOL4 (A_ReFire)
    st!(SPR_SHTG, 2, 5, NONE, ids::S_SGUN7),          // 371: S_SGUN6
    st!(SPR_SHTG, 1, 5, NONE, ids::S_SGUN8),          // 372: S_SGUN7
    st!(SPR_SHTG, 0, 3, NONE, ids::S_SGUN9),          // 373: S_SGUN8
    st!(SPR_SHTG, 0, 7, REFIRE, ids::S_SGUN_READY),   // 374: S_SGUN9 (A_ReFire)
    st!(SPR_SARG, 13, -1, NONE, ids::S_NULL),         // 375: S_SARG_DIE6
    // --- Exploding barrel idle loop (vanilla S_BAR1/S_BAR2, 6 tics each) ---
    st!(SPR_BAR1, 0, 6, NONE, ids::S_BAR2),           // 376: S_BAR1
    st!(SPR_BAR1, 1, 6, NONE, ids::S_BAR1),           // 377: S_BAR2
    // --- Exploding-barrel death animation (vanilla S_BEXP..S_BEXP5). Frames
    //     are fullbright (vanilla frame|FF_FULLBRIGHT). A_Explode fires on entry
    //     to S_BEXP4, dealing 128-radius splash damage. ---
    st!(SPR_BEXP, FB, 5, NONE, ids::S_BEXP2),          // 378: S_BEXP
    st!(SPR_BEXP, 1 | FB, 5, SCREAM, ids::S_BEXP3),    // 379: S_BEXP2 (A_Scream)
    st!(SPR_BEXP, 2 | FB, 5, NONE, ids::S_BEXP4),      // 380: S_BEXP3
    st!(SPR_BEXP, 3 | FB, 10, EXPLODE, ids::S_BEXP5),  // 381: S_BEXP4 (A_Explode)
    st!(SPR_BEXP, 4 | FB, 10, NONE, ids::S_NULL),      // 382: S_BEXP5
    // ===================================================================
    // Over-kill (gib / xdeath) chains, appended for demo-sync fidelity.
    // Vanilla-literal frames/tics/next/action from info.c.  A_XScream plays
    // sfx_slop and draws no P_Random (unlike A_Scream on the normal death
    // chain), so an over-killed monster advances the RNG one draw less.
    // ===================================================================
    // --- Trooper gib (vanilla S_POSS_XDIE1..9, SPR_POSS frames 12..20) ---
    st!(SPR_POSS, 12, 5, NONE, ids::S_POSS_XDIE2),    // 383: S_POSS_XDIE1
    st!(SPR_POSS, 13, 5, XSCREAM, ids::S_POSS_XDIE3), // 384: S_POSS_XDIE2 (A_XScream)
    st!(SPR_POSS, 14, 5, FALL, ids::S_POSS_XDIE4),    // 385: S_POSS_XDIE3 (A_Fall)
    st!(SPR_POSS, 15, 5, NONE, ids::S_POSS_XDIE5),    // 386: S_POSS_XDIE4
    st!(SPR_POSS, 16, 5, NONE, ids::S_POSS_XDIE6),    // 387: S_POSS_XDIE5
    st!(SPR_POSS, 17, 5, NONE, ids::S_POSS_XDIE7),    // 388: S_POSS_XDIE6
    st!(SPR_POSS, 18, 5, NONE, ids::S_POSS_XDIE8),    // 389: S_POSS_XDIE7
    st!(SPR_POSS, 19, 5, NONE, ids::S_POSS_XDIE9),    // 390: S_POSS_XDIE8
    st!(SPR_POSS, 20, -1, NONE, ids::S_NULL),         // 391: S_POSS_XDIE9
    // --- Sergeant gib (vanilla S_SPOS_XDIE1..9, SPR_SPOS frames 12..20) ---
    st!(SPR_SPOS, 12, 5, NONE, ids::S_SPOS_XDIE2),    // 392: S_SPOS_XDIE1
    st!(SPR_SPOS, 13, 5, XSCREAM, ids::S_SPOS_XDIE3), // 393: S_SPOS_XDIE2 (A_XScream)
    st!(SPR_SPOS, 14, 5, FALL, ids::S_SPOS_XDIE4),    // 394: S_SPOS_XDIE3 (A_Fall)
    st!(SPR_SPOS, 15, 5, NONE, ids::S_SPOS_XDIE5),    // 395: S_SPOS_XDIE4
    st!(SPR_SPOS, 16, 5, NONE, ids::S_SPOS_XDIE6),    // 396: S_SPOS_XDIE5
    st!(SPR_SPOS, 17, 5, NONE, ids::S_SPOS_XDIE7),    // 397: S_SPOS_XDIE6
    st!(SPR_SPOS, 18, 5, NONE, ids::S_SPOS_XDIE8),    // 398: S_SPOS_XDIE7
    st!(SPR_SPOS, 19, 5, NONE, ids::S_SPOS_XDIE9),    // 399: S_SPOS_XDIE8
    st!(SPR_SPOS, 20, -1, NONE, ids::S_NULL),         // 400: S_SPOS_XDIE9
    // --- Imp gib (vanilla S_TROO_XDIE1..8, SPR_TROO frames 13..20) ---
    st!(SPR_TROO, 13, 5, NONE, ids::S_TROO_XDIE2),    // 401: S_TROO_XDIE1
    st!(SPR_TROO, 14, 5, XSCREAM, ids::S_TROO_XDIE3), // 402: S_TROO_XDIE2 (A_XScream)
    st!(SPR_TROO, 15, 5, NONE, ids::S_TROO_XDIE4),    // 403: S_TROO_XDIE3
    st!(SPR_TROO, 16, 5, FALL, ids::S_TROO_XDIE5),    // 404: S_TROO_XDIE4 (A_Fall)
    st!(SPR_TROO, 17, 5, NONE, ids::S_TROO_XDIE6),    // 405: S_TROO_XDIE5
    st!(SPR_TROO, 18, 5, NONE, ids::S_TROO_XDIE7),    // 406: S_TROO_XDIE6
    st!(SPR_TROO, 19, 5, NONE, ids::S_TROO_XDIE8),    // 407: S_TROO_XDIE7
    st!(SPR_TROO, 20, -1, NONE, ids::S_NULL),         // 408: S_TROO_XDIE8
];

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s_null_holds_forever() {
        let e = &STATES[ids::S_NULL as usize];
        assert_eq!(e.tics, -1);
        assert_eq!(e.next_state, StateNum(ids::S_NULL));
        assert_eq!(e.action, crate::actions::Action::NoAction as u8);
        assert_eq!(e.sprite, SPR_NONE);
    }

    #[test]
    fn barrel_death_chain_matches_vanilla_bexp_states() {
        // Vanilla exploding-barrel death: S_BEXP(5) -> S_BEXP2(5, A_Scream) ->
        // S_BEXP3(5) -> S_BEXP4(10, A_Explode) -> S_BEXP5(10) -> S_NULL, all
        // fullbright. `A_Explode` fires on entry to S_BEXP4, 15 tics into the
        // animation — the demo-sync barrel splash timing hinges on this chain.
        let explode = crate::actions::Action::Explode as u8;
        let scream = crate::actions::Action::Scream as u8;
        let none = crate::actions::Action::NoAction as u8;

        let bexp = &STATES[ids::S_BEXP as usize];
        assert_eq!(bexp.tics, 5);
        assert_eq!(bexp.action, none);
        assert_eq!(bexp.next_state, StateNum(ids::S_BEXP2));
        assert_ne!(bexp.frame & 0x80, 0, "S_BEXP must be fullbright");

        let bexp2 = &STATES[ids::S_BEXP2 as usize];
        assert_eq!(bexp2.tics, 5);
        assert_eq!(bexp2.action, scream, "S_BEXP2 runs A_Scream");
        assert_eq!(bexp2.next_state, StateNum(ids::S_BEXP3));

        let bexp3 = &STATES[ids::S_BEXP3 as usize];
        assert_eq!(bexp3.tics, 5);
        assert_eq!(bexp3.action, none);
        assert_eq!(bexp3.next_state, StateNum(ids::S_BEXP4));

        let bexp4 = &STATES[ids::S_BEXP4 as usize];
        assert_eq!(bexp4.tics, 10);
        assert_eq!(bexp4.action, explode, "S_BEXP4 runs A_Explode");
        assert_eq!(bexp4.next_state, StateNum(ids::S_BEXP5));

        let bexp5 = &STATES[ids::S_BEXP5 as usize];
        assert_eq!(bexp5.tics, 10);
        assert_eq!(bexp5.action, none);
        assert_eq!(bexp5.next_state, StateNum(ids::S_NULL));
    }

    #[test]
    fn barrel_mobjinfo_death_state_is_bexp() {
        // The exploding barrel must route its death to the S_BEXP explosion
        // chain — without this it silently vanishes when shot and never deals
        // the splash damage vanilla does (the DEMO1 lt225 desync).
        use doom_types::mobj_kind::MobjKind;
        let info = &crate::mobjinfo::MOBJINFO[MobjKind::Barrel as usize];
        assert_eq!(info.death_state, StateNum(ids::S_BEXP));
        assert_ne!(
            info.flags & crate::mobj::flags::MF_NOBLOOD,
            0,
            "barrel is MF_NOBLOOD in vanilla"
        );
    }

    #[test]
    fn poss_stnd_alternates_with_second_idle_frame() {
        let e = &STATES[ids::S_POSS_STND as usize];
        assert_eq!(e.tics, 10);
        assert_eq!(e.next_state, StateNum(ids::S_POSS_STND2));
        assert_eq!(e.action, crate::actions::Action::Look as u8);
        assert_eq!(e.sprite, SPR_POSS);
        assert_eq!(e.frame, 0);

        let e2 = &STATES[ids::S_POSS_STND2 as usize];
        assert_eq!(e2.tics, 10);
        assert_eq!(e2.next_state, StateNum(ids::S_POSS_STND));
        assert_eq!(e2.action, crate::actions::Action::Look as u8);
        assert_eq!(e2.sprite, SPR_POSS);
        assert_eq!(e2.frame, 1);
    }

    #[test]
    fn poss_run_cycles_with_chase() {
        let run1 = &STATES[ids::S_POSS_RUN1 as usize];
        let run2 = &STATES[ids::S_POSS_RUN2 as usize];
        let run3 = &STATES[ids::S_POSS_RUN3 as usize];
        let run4 = &STATES[ids::S_POSS_RUN4 as usize];
        assert_eq!(run1.next_state, StateNum(ids::S_POSS_RUN2));
        assert_eq!(run2.next_state, StateNum(ids::S_POSS_RUN3));
        assert_eq!(run3.next_state, StateNum(ids::S_POSS_RUN4));
        assert_eq!(run4.next_state, StateNum(ids::S_POSS_RUN1));
        assert_eq!(run1.action, crate::actions::Action::Chase as u8);
        assert_eq!(run2.action, crate::actions::Action::Chase as u8);
        assert_eq!(run3.action, crate::actions::Action::Chase as u8);
        assert_eq!(run4.action, crate::actions::Action::Chase as u8);
    }

    #[test]
    fn table_length_matches_count() {
        assert_eq!(STATES.len(), ids::STATES_COUNT);
    }

    #[test]
    fn all_next_state_indices_in_bounds() {
        for (i, e) in STATES.iter().enumerate() {
            let next = e.next_state.0 as usize;
            assert!(
                next < STATES.len(),
                "state {i} has next_state {next} out of bounds"
            );
        }
    }

    #[test]
    fn trooper_death_state_is_five_frames() {
        // DIE1 → DIE2 → DIE3 → DIE4 → DIE5 → S_NULL
        let die1 = &STATES[ids::S_POSS_DIE1 as usize];
        let die2 = &STATES[ids::S_POSS_DIE2 as usize];
        let die3 = &STATES[ids::S_POSS_DIE3 as usize];
        let die4 = &STATES[ids::S_POSS_DIE4 as usize];
        let die5 = &STATES[ids::S_POSS_DIE5 as usize];
        assert_eq!(die1.next_state, StateNum(ids::S_POSS_DIE2));
        assert_eq!(die2.next_state, StateNum(ids::S_POSS_DIE3));
        assert_eq!(die3.next_state, StateNum(ids::S_POSS_DIE4));
        assert_eq!(die4.next_state, StateNum(ids::S_POSS_DIE5));
        assert_eq!(die5.tics, -1);
        assert_eq!(die5.next_state, StateNum(ids::S_NULL));
    }

    #[test]
    fn monster_pain_states_return_to_chase() {
        // (pain_state, next_state, vanilla collapsed pain-duration tics, name)
        let cases = [
            (ids::S_POSS_PAIN, ids::S_POSS_RUN1, 6, "trooper"),
            (ids::S_SPOS_PAIN, ids::S_SPOS_RUN1, 6, "sergeant"),
            (ids::S_TROO_PAIN, ids::S_TROO_RUN1, 4, "imp"),
            (ids::S_SARG_PAIN, ids::S_SARG_RUN1, 4, "demon"),
            (ids::S_HEAD_PAIN, ids::S_HEAD_RUN1, 12, "cacodemon"),
            (ids::S_BOSS_PAIN, ids::S_BOSS_RUN1, 4, "baron"),
            (ids::S_CYBER_PAIN, ids::S_CYBER_RUN1, 10, "cyberdemon"),
            (ids::S_SPID_PAIN, ids::S_SPID_RUN1, 6, "spider mastermind"),
            (ids::S_BOS2_PAIN, ids::S_BOS2_RUN1, 4, "hell knight"),
        ];

        for (pain_state, next_state, tics, name) in cases {
            let pain = &STATES[pain_state as usize];
            assert_eq!(
                pain.next_state,
                StateNum(next_state),
                "{name} pain state must resume chasing, not idle"
            );
            assert_eq!(
                pain.tics, tics,
                "{name} pain-state total tics must match vanilla info.c"
            );
        }
    }

    #[test]
    fn attack_states_return_to_run() {
        let atk3 = &STATES[ids::S_POSS_ATK3 as usize];
        assert_eq!(atk3.next_state, StateNum(ids::S_POSS_RUN1));
        assert_eq!(atk3.action, crate::actions::Action::NoAction as u8);

        let spos_atk3 = &STATES[ids::S_SPOS_ATK3 as usize];
        assert_eq!(spos_atk3.next_state, StateNum(ids::S_SPOS_RUN1));

        let troo_atk3 = &STATES[ids::S_TROO_ATK3 as usize];
        assert_eq!(troo_atk3.next_state, StateNum(ids::S_TROO_RUN1));

        let sarg_atk3 = &STATES[ids::S_SARG_ATK3 as usize];
        assert_eq!(sarg_atk3.next_state, StateNum(ids::S_SARG_RUN1));
    }

    #[test]
    fn attack_actions_fire_on_vanilla_state() {
        // Vanilla info.c: hitscan zombies fire on ATK2; imp/demon fire on ATK3.
        assert_eq!(
            STATES[ids::S_POSS_ATK2 as usize].action,
            crate::actions::Action::PosAttack as u8
        );
        assert_eq!(
            STATES[ids::S_SPOS_ATK2 as usize].action,
            crate::actions::Action::SposAttack as u8
        );
        assert_eq!(
            STATES[ids::S_TROO_ATK3 as usize].action,
            crate::actions::Action::TrooAttack as u8
        );
        assert_eq!(
            STATES[ids::S_SARG_ATK3 as usize].action,
            crate::actions::Action::SargAttack as u8
        );
    }

    /// Regression guard: state tics / action placement that gate RNG-draw
    /// timing must match vanilla `info.c` exactly (demo sync).
    #[test]
    fn demo_actor_timing_matches_info_c() {
        use crate::actions::Action;
        let fall = Action::Fall as u8;
        let scream = Action::Scream as u8;
        let refire = Action::Refire as u8;
        // Attack-sequence tics (ATK1/ATK2/ATK3).
        assert_eq!(
            [
                STATES[ids::S_POSS_ATK1 as usize].tics,
                STATES[ids::S_POSS_ATK2 as usize].tics,
                STATES[ids::S_POSS_ATK3 as usize].tics
            ],
            [10, 8, 8]
        );
        assert_eq!(
            [
                STATES[ids::S_SPOS_ATK1 as usize].tics,
                STATES[ids::S_SPOS_ATK2 as usize].tics,
                STATES[ids::S_SPOS_ATK3 as usize].tics
            ],
            [10, 10, 10]
        );
        assert_eq!(
            [
                STATES[ids::S_TROO_ATK1 as usize].tics,
                STATES[ids::S_TROO_ATK2 as usize].tics,
                STATES[ids::S_TROO_ATK3 as usize].tics
            ],
            [8, 8, 6]
        );
        assert_eq!(
            [
                STATES[ids::S_SARG_ATK1 as usize].tics,
                STATES[ids::S_SARG_ATK2 as usize].tics,
                STATES[ids::S_SARG_ATK3 as usize].tics
            ],
            [8, 8, 8]
        );
        // A_Fall must fire on the vanilla death frame (governs corpse solidity).
        assert_eq!(STATES[ids::S_POSS_DIE3 as usize].action, fall);
        assert_eq!(STATES[ids::S_SPOS_DIE3 as usize].action, fall);
        assert_eq!(STATES[ids::S_TROO_DIE4 as usize].action, fall);
        assert_eq!(STATES[ids::S_SARG_DIE4 as usize].action, fall);
        assert_eq!(STATES[ids::S_POSS_DIE2 as usize].action, scream);
        // Player pistol: A_FirePistol on PISTOL2 (t=4), A_ReFire on PISTOL4.
        assert_eq!(STATES[ids::S_PISTOL1 as usize].tics, 4);
        assert_eq!(
            STATES[ids::S_PISTOL2 as usize].action,
            Action::FirePistol as u8
        );
        assert_eq!(STATES[ids::S_PISTOL4 as usize].action, refire);
        // Player shotgun: A_FireShotgun on SGUN2 (t=3), A_ReFire on SGUN9.
        assert_eq!(STATES[ids::S_SGUN1 as usize].tics, 3);
        assert_eq!(
            STATES[ids::S_SGUN2 as usize].action,
            Action::FireShotgun as u8
        );
        assert_eq!(STATES[ids::S_SGUN9 as usize].action, refire);
    }

    #[test]
    fn nearby_monster_attack_frames_match_vanilla_sequences() {
        assert_eq!(
            STATES[ids::S_POSS_ATK1 as usize].frame,
            4,
            "trooper ATK1 must use E"
        );
        assert_eq!(
            STATES[ids::S_POSS_ATK2 as usize].frame,
            5,
            "trooper ATK2 must use F"
        );
        assert_eq!(
            STATES[ids::S_POSS_ATK3 as usize].frame,
            4,
            "trooper ATK3 must use E"
        );

        assert_eq!(
            STATES[ids::S_SPOS_ATK1 as usize].frame,
            4,
            "sergeant ATK1 must use E"
        );
        assert_eq!(
            STATES[ids::S_SPOS_ATK2 as usize].frame,
            5,
            "sergeant ATK2 must use F"
        );
        assert_eq!(
            STATES[ids::S_SPOS_ATK3 as usize].frame,
            4,
            "sergeant ATK3 must use E"
        );

        assert_eq!(
            STATES[ids::S_TROO_ATK1 as usize].frame,
            4,
            "imp ATK1 must use E"
        );
        assert_eq!(
            STATES[ids::S_TROO_ATK2 as usize].frame,
            5,
            "imp ATK2 must use F"
        );
        assert_eq!(
            STATES[ids::S_TROO_ATK3 as usize].frame,
            6,
            "imp ATK3 must use G"
        );

        assert_eq!(
            STATES[ids::S_SARG_ATK1 as usize].frame,
            4,
            "demon ATK1 must use E"
        );
        assert_eq!(
            STATES[ids::S_SARG_ATK2 as usize].frame,
            5,
            "demon ATK2 must use F"
        );
        assert_eq!(
            STATES[ids::S_SARG_ATK3 as usize].frame,
            6,
            "demon ATK3 must use G"
        );
    }

    #[test]
    fn nearby_monster_run_chains_cover_four_frames() {
        assert_eq!(
            STATES[ids::S_POSS_RUN1 as usize].next_state,
            StateNum(ids::S_POSS_RUN2)
        );
        assert_eq!(
            STATES[ids::S_POSS_RUN2 as usize].next_state,
            StateNum(ids::S_POSS_RUN3)
        );
        assert_eq!(
            STATES[ids::S_POSS_RUN3 as usize].next_state,
            StateNum(ids::S_POSS_RUN4)
        );
        assert_eq!(
            STATES[ids::S_POSS_RUN4 as usize].next_state,
            StateNum(ids::S_POSS_RUN1)
        );
        assert_eq!(
            STATES[ids::S_POSS_RUN4 as usize].frame,
            3,
            "trooper RUN4 must use D"
        );

        assert_eq!(
            STATES[ids::S_SPOS_RUN2 as usize].next_state,
            StateNum(ids::S_SPOS_RUN3)
        );
        assert_eq!(
            STATES[ids::S_SPOS_RUN3 as usize].next_state,
            StateNum(ids::S_SPOS_RUN4)
        );
        assert_eq!(
            STATES[ids::S_SPOS_RUN4 as usize].next_state,
            StateNum(ids::S_SPOS_RUN1)
        );

        assert_eq!(
            STATES[ids::S_TROO_RUN2 as usize].next_state,
            StateNum(ids::S_TROO_RUN3)
        );
        assert_eq!(
            STATES[ids::S_TROO_RUN3 as usize].next_state,
            StateNum(ids::S_TROO_RUN4)
        );
        assert_eq!(
            STATES[ids::S_TROO_RUN4 as usize].next_state,
            StateNum(ids::S_TROO_RUN1)
        );

        assert_eq!(
            STATES[ids::S_SARG_RUN2 as usize].next_state,
            StateNum(ids::S_SARG_RUN3)
        );
        assert_eq!(
            STATES[ids::S_SARG_RUN3 as usize].next_state,
            StateNum(ids::S_SARG_RUN4)
        );
        assert_eq!(
            STATES[ids::S_SARG_RUN4 as usize].next_state,
            StateNum(ids::S_SARG_RUN1)
        );
    }

    /// Regression: RUN/SEE-state tics must match vanilla `info.c` exactly so the
    /// A_Chase active-sound RNG draw lands on the same tic as vanilla. Vanilla
    /// cadence: POSS=4, SPOS=3, TROO=3, SARG=2, HEAD=3, BOSS=3, CYBER=3, SPID=3.
    /// (Previously all eight were uniformly 4, making chasing monsters advance
    /// every 4 tics instead of vanilla's 3/2 and desyncing per-tic draw
    /// placement.)
    #[test]
    fn run_state_tics_match_vanilla_info_c() {
        let expect: &[(u16, i16)] = &[
            (ids::S_POSS_RUN1, 4),
            (ids::S_POSS_RUN2, 4),
            (ids::S_POSS_RUN3, 4),
            (ids::S_POSS_RUN4, 4),
            (ids::S_SPOS_RUN1, 3),
            (ids::S_SPOS_RUN2, 3),
            (ids::S_SPOS_RUN3, 3),
            (ids::S_SPOS_RUN4, 3),
            (ids::S_TROO_RUN1, 3),
            (ids::S_TROO_RUN2, 3),
            (ids::S_TROO_RUN3, 3),
            (ids::S_TROO_RUN4, 3),
            (ids::S_SARG_RUN1, 2),
            (ids::S_SARG_RUN2, 2),
            (ids::S_SARG_RUN3, 2),
            (ids::S_SARG_RUN4, 2),
            (ids::S_HEAD_RUN1, 3),
            (ids::S_HEAD_RUN2, 3),
            (ids::S_BOSS_RUN1, 3),
            (ids::S_BOSS_RUN2, 3),
            (ids::S_CYBER_RUN1, 3),
            (ids::S_CYBER_RUN2, 3),
            (ids::S_SPID_RUN1, 3),
            (ids::S_SPID_RUN2, 3),
        ];
        for &(state, tics) in expect {
            assert_eq!(
                STATES[state as usize].tics, tics,
                "state {state} RUN tics must be {tics} (vanilla info.c)"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Sprite field tests
    // -----------------------------------------------------------------------

    #[test]
    fn trooper_spawn_state_sprite_is_poss() {
        assert_eq!(STATES[ids::S_POSS_STND as usize].sprite, SPR_POSS);
    }

    #[test]
    fn sergeant_spawn_state_sprite_is_spos() {
        assert_eq!(STATES[ids::S_SPOS_STND as usize].sprite, SPR_SPOS);
    }

    #[test]
    fn imp_spawn_state_sprite_is_troo() {
        assert_eq!(STATES[ids::S_TROO_STND as usize].sprite, SPR_TROO);
    }

    #[test]
    fn demon_spawn_state_sprite_is_sarg() {
        assert_eq!(STATES[ids::S_SARG_STND as usize].sprite, SPR_SARG);
    }

    #[test]
    fn cacodemon_spawn_state_sprite_is_head() {
        assert_eq!(STATES[ids::S_HEAD_STND as usize].sprite, SPR_HEAD);
    }

    #[test]
    fn baron_spawn_state_sprite_is_boss() {
        assert_eq!(STATES[ids::S_BOSS_STND as usize].sprite, SPR_BOSS);
    }

    #[test]
    fn cyberdemon_spawn_state_sprite_is_cybr() {
        assert_eq!(STATES[ids::S_CYBER_STND as usize].sprite, SPR_CYBR);
    }

    #[test]
    fn spider_spawn_state_sprite_is_spid() {
        assert_eq!(STATES[ids::S_SPID_STND as usize].sprite, SPR_SPID);
    }

    // -----------------------------------------------------------------------
    // Frame field tests
    // -----------------------------------------------------------------------

    #[test]
    fn trooper_idle_frame_is_a() {
        assert_eq!(STATES[ids::S_POSS_STND as usize].frame, 0);
    }

    #[test]
    fn trooper_run2_frame_is_b() {
        assert_eq!(STATES[ids::S_POSS_RUN2 as usize].frame, 1);
    }

    #[test]
    fn imp_run2_frame_is_b() {
        assert_eq!(STATES[ids::S_TROO_RUN2 as usize].frame, 1);
    }

    // -----------------------------------------------------------------------
    // Fullbright tests
    // -----------------------------------------------------------------------

    #[test]
    fn imp_fireball_fly_is_fullbright() {
        let s = &STATES[ids::S_TBALL1 as usize];
        assert_eq!(s.sprite, SPR_BAL1);
        assert_ne!(s.frame & FB, 0, "imp fireball fly must be fullbright");
    }

    #[test]
    fn imp_fireball_death_is_fullbright() {
        let s = &STATES[ids::S_TBALLX1 as usize];
        assert_ne!(s.frame & FB, 0, "imp fireball death must be fullbright");
    }

    #[test]
    fn rocket_fly_is_fullbright() {
        let s = &STATES[ids::S_ROCKET as usize];
        assert_eq!(s.sprite, SPR_MISL);
        assert_ne!(s.frame & FB, 0);
    }

    #[test]
    fn plasma_fly_is_fullbright() {
        let s = &STATES[ids::S_PLASBALL1 as usize];
        assert_ne!(s.frame & FB, 0);
    }

    #[test]
    fn bfg_fly_is_fullbright() {
        let s = &STATES[ids::S_BFGSHOT1 as usize];
        assert_ne!(s.frame & FB, 0);
    }

    #[test]
    fn weapon_flash_is_fullbright() {
        assert_ne!(STATES[ids::S_PISTOL_FLASH1 as usize].frame & FB, 0);
        assert_ne!(STATES[ids::S_SGUN_FLASH1 as usize].frame & FB, 0);
        assert_ne!(STATES[ids::S_DSGUN_FLASH1 as usize].frame & FB, 0);
        assert_ne!(STATES[ids::S_CHAIN_FLASH1 as usize].frame & FB, 0);
        assert_ne!(STATES[ids::S_MISSILE_FLASH1 as usize].frame & FB, 0);
        assert_ne!(STATES[ids::S_BFG_FLASH1 as usize].frame & FB, 0);
    }

    #[test]
    fn lost_soul_fly_is_fullbright() {
        assert_ne!(STATES[ids::S_SKULL_STND as usize].frame & FB, 0);
    }

    // -----------------------------------------------------------------------
    // Projectile chain tests
    // -----------------------------------------------------------------------

    #[test]
    fn imp_fireball_fly_loops() {
        let s1 = &STATES[ids::S_TBALL1 as usize];
        let s2 = &STATES[ids::S_TBALL2 as usize];
        assert_eq!(s1.next_state, StateNum(ids::S_TBALL2));
        assert_eq!(s2.next_state, StateNum(ids::S_TBALL1));
    }

    #[test]
    fn imp_fireball_death_terminates() {
        let last = &STATES[ids::S_TBALLX3 as usize];
        assert_eq!(last.next_state, StateNum(ids::S_NULL));
    }

    #[test]
    fn rocket_fly_loops_to_self() {
        let s = &STATES[ids::S_ROCKET as usize];
        assert_eq!(s.next_state, StateNum(ids::S_ROCKET));
    }

    #[test]
    fn rocket_death_terminates() {
        let last = &STATES[ids::S_EXPLODE3 as usize];
        assert_eq!(last.next_state, StateNum(ids::S_NULL));
    }

    #[test]
    fn plasma_death_terminates() {
        let last = &STATES[ids::S_PLASEXP4 as usize];
        assert_eq!(last.next_state, StateNum(ids::S_NULL));
    }

    #[test]
    fn bfg_explode_terminates() {
        let last = &STATES[ids::S_BFGLAND6 as usize];
        assert_eq!(last.next_state, StateNum(ids::S_NULL));
    }

    #[test]
    fn revenant_tracer_fly_loops() {
        let s1 = &STATES[ids::S_TRACER1 as usize];
        let s2 = &STATES[ids::S_TRACER2 as usize];
        assert_eq!(s1.next_state, StateNum(ids::S_TRACER2));
        assert_eq!(s2.next_state, StateNum(ids::S_TRACER1));
    }

    #[test]
    fn revenant_tracer_death_terminates() {
        assert_eq!(
            STATES[ids::S_TRACEEXP3 as usize].next_state,
            StateNum(ids::S_NULL)
        );
    }

    #[test]
    fn arachnotron_plasma_fly_loops() {
        let s1 = &STATES[ids::S_ARACH_PLAZ1 as usize];
        let s2 = &STATES[ids::S_ARACH_PLAZ2 as usize];
        assert_eq!(s1.next_state, StateNum(ids::S_ARACH_PLAZ2));
        assert_eq!(s2.next_state, StateNum(ids::S_ARACH_PLAZ1));
    }

    #[test]
    fn mancubus_fireball_fly_loops() {
        let s1 = &STATES[ids::S_FATSHOT1 as usize];
        let s2 = &STATES[ids::S_FATSHOT2 as usize];
        assert_eq!(s1.next_state, StateNum(ids::S_FATSHOT2));
        assert_eq!(s2.next_state, StateNum(ids::S_FATSHOT1));
    }

    // -----------------------------------------------------------------------
    // Effect state tests
    // -----------------------------------------------------------------------

    #[test]
    fn bullet_puff_terminates() {
        assert_eq!(
            STATES[ids::S_PUFF4 as usize].next_state,
            StateNum(ids::S_NULL)
        );
    }

    #[test]
    fn blood_splat_terminates() {
        assert_eq!(
            STATES[ids::S_BLOOD3 as usize].next_state,
            StateNum(ids::S_NULL)
        );
    }

    #[test]
    fn teleport_fog_terminates() {
        assert_eq!(
            STATES[ids::S_TFOG5 as usize].next_state,
            StateNum(ids::S_NULL)
        );
    }

    #[test]
    fn item_fog_terminates() {
        assert_eq!(
            STATES[ids::S_IFOG5 as usize].next_state,
            StateNum(ids::S_NULL)
        );
    }

    #[test]
    fn bullet_puff_first_frame_fullbright() {
        assert_ne!(STATES[ids::S_PUFF1 as usize].frame & FB, 0);
    }

    // -----------------------------------------------------------------------
    // Weapon state chain tests
    // -----------------------------------------------------------------------

    #[test]
    fn fist_ready_loops() {
        let s = &STATES[ids::S_PUNCH_READY as usize];
        assert_eq!(s.next_state, StateNum(ids::S_PUNCH_READY));
    }

    #[test]
    fn pistol_ready_loops() {
        let s = &STATES[ids::S_PISTOL_READY as usize];
        assert_eq!(s.next_state, StateNum(ids::S_PISTOL_READY));
    }

    #[test]
    fn shotgun_ready_loops() {
        let s = &STATES[ids::S_SGUN_READY as usize];
        assert_eq!(s.next_state, StateNum(ids::S_SGUN_READY));
    }

    #[test]
    fn chainsaw_ready_loops() {
        let s = &STATES[ids::S_SAW_READY1 as usize];
        assert_eq!(s.next_state, StateNum(ids::S_SAW_READY2));
        let s2 = &STATES[ids::S_SAW_READY2 as usize];
        assert_eq!(s2.next_state, StateNum(ids::S_SAW_READY1));
    }

    #[test]
    fn fist_fire_returns_to_ready() {
        let last = &STATES[ids::S_PUNCH4 as usize];
        assert_eq!(last.next_state, StateNum(ids::S_PUNCH5));
        assert_eq!(
            STATES[ids::S_PUNCH5 as usize].next_state,
            StateNum(ids::S_PUNCH_READY)
        );
    }

    #[test]
    fn pistol_fire_returns_to_ready() {
        assert_eq!(
            STATES[ids::S_PISTOL3 as usize].next_state,
            StateNum(ids::S_PISTOL4)
        );
        assert_eq!(
            STATES[ids::S_PISTOL4 as usize].next_state,
            StateNum(ids::S_PISTOL_READY)
        );
    }

    #[test]
    fn pistol_flash_terminates() {
        assert_eq!(
            STATES[ids::S_PISTOL_FLASH2 as usize].next_state,
            StateNum(ids::S_LIGHTDONE)
        );
    }

    #[test]
    fn weapon_ready_and_transition_states_use_psprite_actions() {
        assert_eq!(
            STATES[ids::S_PISTOL_UP as usize].action,
            crate::actions::Action::Raise as u8
        );
        assert_eq!(
            STATES[ids::S_PISTOL_DOWN as usize].action,
            crate::actions::Action::Lower as u8
        );
        assert_eq!(
            STATES[ids::S_PISTOL_READY as usize].action,
            crate::actions::Action::WeaponReady as u8
        );
        assert_eq!(
            STATES[ids::S_SAW_READY1 as usize].action,
            crate::actions::Action::WeaponReady as u8
        );
        assert_eq!(
            STATES[ids::S_SAW_READY2 as usize].action,
            crate::actions::Action::WeaponReady as u8
        );
    }

    #[test]
    fn key_weapon_attack_states_fire_psprite_actions() {
        assert_eq!(
            STATES[ids::S_CHAIN2 as usize].action,
            crate::actions::Action::FireCgun as u8
        );
        assert_eq!(
            STATES[ids::S_CHAIN3 as usize].action,
            crate::actions::Action::Refire as u8
        );
        assert_eq!(
            STATES[ids::S_MISSILE1 as usize].action,
            crate::actions::Action::GunFlash as u8
        );
        assert_eq!(
            STATES[ids::S_PISTOL_FLASH1 as usize].action,
            crate::actions::Action::Light1 as u8
        );
        assert_eq!(
            STATES[ids::S_SGUN_FLASH2 as usize].action,
            crate::actions::Action::Light2 as u8
        );
        assert_eq!(
            STATES[ids::S_PLASMA_FLASH1 as usize].action,
            crate::actions::Action::Light1 as u8
        );
        assert_eq!(
            STATES[ids::S_LIGHTDONE as usize].action,
            crate::actions::Action::Light0 as u8
        );
        assert_eq!(
            STATES[ids::S_MISSILE2 as usize].action,
            crate::actions::Action::FireMissile as u8
        );
        assert_eq!(
            STATES[ids::S_PLASMA1 as usize].action,
            crate::actions::Action::FirePlasma as u8
        );
        assert_eq!(
            STATES[ids::S_PLASMA3 as usize].action,
            crate::actions::Action::FirePlasma as u8
        );
        assert_eq!(
            STATES[ids::S_PLASMA5 as usize].action,
            crate::actions::Action::Refire as u8
        );
        assert_eq!(
            STATES[ids::S_BFG1 as usize].action,
            crate::actions::Action::BfgSound as u8
        );
        assert_eq!(
            STATES[ids::S_BFG2 as usize].action,
            crate::actions::Action::FireBfg as u8
        );
        assert_eq!(
            STATES[ids::S_DSGUN4 as usize].action,
            crate::actions::Action::CheckReload as u8
        );
        assert_eq!(
            STATES[ids::S_DSGUN5 as usize].action,
            crate::actions::Action::OpenShotgun2 as u8
        );
        assert_eq!(
            STATES[ids::S_DSGUN6 as usize].action,
            crate::actions::Action::LoadShotgun2 as u8
        );
        assert_eq!(
            STATES[ids::S_DSGUN7 as usize].action,
            crate::actions::Action::CloseShotgun2 as u8
        );
        assert_eq!(
            STATES[ids::S_DSGUN8 as usize].action,
            crate::actions::Action::Refire as u8
        );
    }

    #[test]
    fn plasma_and_ssg_attack_chains_match_doom_psprite_shape() {
        assert_eq!(STATES[ids::S_PLASMA2 as usize].tics, 3);
        assert_eq!(
            STATES[ids::S_PLASMA2 as usize].next_state,
            StateNum(ids::S_PLASMA3)
        );
        assert_eq!(
            STATES[ids::S_PLASMA4 as usize].next_state,
            StateNum(ids::S_PLASMA5)
        );
        assert_eq!(
            STATES[ids::S_CHAIN2 as usize].next_state,
            StateNum(ids::S_CHAIN3)
        );
        assert_eq!(
            STATES[ids::S_CHAIN_FLASH2 as usize].next_state,
            StateNum(ids::S_CHAIN_FLASH3)
        );
        assert_eq!(STATES[ids::S_DSGUN4 as usize].tics, 7);
        assert_eq!(STATES[ids::S_DSGUN5 as usize].tics, 6);
        assert_eq!(STATES[ids::S_DSGUN6 as usize].tics, 6);
        assert_eq!(STATES[ids::S_DSGUN7 as usize].tics, 5);
        assert_eq!(STATES[ids::S_DSGUN8 as usize].tics, 5);
        assert_eq!(
            STATES[ids::S_DSGUN_FLASH2 as usize].next_state,
            StateNum(ids::S_DSGUN_FLASH3)
        );
        assert_eq!(
            STATES[ids::S_PUNCH4 as usize].next_state,
            StateNum(ids::S_PUNCH5)
        );
        assert_eq!(
            STATES[ids::S_PUNCH5 as usize].action,
            crate::actions::Action::Refire as u8
        );
        assert_eq!(
            STATES[ids::S_SGUN9 as usize].action,
            crate::actions::Action::Refire as u8
        );
        assert_eq!(
            STATES[ids::S_SAW3 as usize].action,
            crate::actions::Action::Refire as u8
        );
    }

    // -----------------------------------------------------------------------
    // Doom 2 monster state tests
    // -----------------------------------------------------------------------

    #[test]
    fn lost_soul_states_not_null() {
        assert_ne!(
            STATES[ids::S_SKULL_STND as usize].next_state,
            StateNum(ids::S_NULL)
        );
        assert_eq!(STATES[ids::S_SKULL_STND as usize].sprite, SPR_SKUL);
    }

    #[test]
    fn arachnotron_states_not_null() {
        assert_ne!(
            STATES[ids::S_BSPI_STND as usize].next_state,
            StateNum(ids::S_NULL)
        );
        assert_eq!(STATES[ids::S_BSPI_STND as usize].sprite, SPR_BSPI);
    }

    #[test]
    fn pain_elemental_states_not_null() {
        assert_ne!(
            STATES[ids::S_PAIN_STND as usize].next_state,
            StateNum(ids::S_NULL)
        );
        assert_eq!(STATES[ids::S_PAIN_STND as usize].sprite, SPR_PAIN);
    }

    #[test]
    fn revenant_states_not_null() {
        assert_ne!(
            STATES[ids::S_SKEL_STND as usize].next_state,
            StateNum(ids::S_NULL)
        );
        assert_eq!(STATES[ids::S_SKEL_STND as usize].sprite, SPR_SKEL);
    }

    #[test]
    fn mancubus_states_not_null() {
        assert_ne!(
            STATES[ids::S_FATT_STND as usize].next_state,
            StateNum(ids::S_NULL)
        );
        assert_eq!(STATES[ids::S_FATT_STND as usize].sprite, SPR_FATT);
    }

    #[test]
    fn archvile_states_not_null() {
        assert_ne!(
            STATES[ids::S_VILE_STND as usize].next_state,
            StateNum(ids::S_NULL)
        );
        assert_eq!(STATES[ids::S_VILE_STND as usize].sprite, SPR_VILE);
    }

    #[test]
    fn chaingunner_states_not_null() {
        assert_ne!(
            STATES[ids::S_CPOS_STND as usize].next_state,
            StateNum(ids::S_NULL)
        );
        assert_eq!(STATES[ids::S_CPOS_STND as usize].sprite, SPR_CPOS);
    }

    #[test]
    fn lost_soul_death_terminates() {
        assert_eq!(
            STATES[ids::S_SKULL_DIE5 as usize].next_state,
            StateNum(ids::S_NULL)
        );
        assert_eq!(STATES[ids::S_SKULL_DIE5 as usize].tics, -1);
    }

    #[test]
    fn arachnotron_death_terminates() {
        assert_eq!(
            STATES[ids::S_BSPI_DIE5 as usize].next_state,
            StateNum(ids::S_NULL)
        );
    }

    #[test]
    fn chaingunner_attack_uses_cpos_action() {
        assert_eq!(
            STATES[ids::S_CPOS_ATK2 as usize].action,
            crate::actions::Action::CposAttack as u8
        );
    }

    #[test]
    fn revenant_attack_uses_skel_missile() {
        assert_eq!(
            STATES[ids::S_SKEL_ATK2 as usize].action,
            crate::actions::Action::SkelMissile as u8
        );
    }

    #[test]
    fn mancubus_attack_uses_fat_attack() {
        assert_eq!(
            STATES[ids::S_FATT_ATK2 as usize].action,
            crate::actions::Action::FatAttack1 as u8
        );
    }

    #[test]
    fn arachnotron_attack_uses_bspi_attack() {
        assert_eq!(
            STATES[ids::S_BSPI_ATK2 as usize].action,
            crate::actions::Action::BspiAttack as u8
        );
    }

    #[test]
    fn pain_elemental_attack_uses_pain_attack() {
        assert_eq!(
            STATES[ids::S_PAIN_ATK2 as usize].action,
            crate::actions::Action::PainAttack as u8
        );
    }

    #[test]
    fn lost_soul_attack_uses_skull_attack() {
        assert_eq!(
            STATES[ids::S_SKULL_ATK2 as usize].action,
            crate::actions::Action::SkullAttack as u8
        );
    }

    // -----------------------------------------------------------------------
    // SPRITE_NAMES table tests
    // -----------------------------------------------------------------------

    #[test]
    fn sprite_names_len_matches_count() {
        assert_eq!(SPRITE_NAMES.len(), SPRITE_COUNT);
    }

    #[test]
    fn sprite_names_no_duplicates() {
        for (i, name_i) in SPRITE_NAMES.iter().enumerate() {
            for (j, name_j) in SPRITE_NAMES.iter().enumerate().skip(i + 1) {
                assert_ne!(
                    name_i, name_j,
                    "duplicate sprite name at indices {i} and {j}: {}",
                    name_i
                );
            }
        }
    }

    #[test]
    fn sprite_names_all_four_chars() {
        for (i, name) in SPRITE_NAMES.iter().enumerate() {
            assert_eq!(
                name.len(),
                4,
                "sprite name at index {i} ({name}) is not 4 characters"
            );
        }
    }

    // -----------------------------------------------------------------------
    // All state entries have valid sprites
    // -----------------------------------------------------------------------

    #[test]
    fn all_sprites_valid() {
        for (i, e) in STATES.iter().enumerate() {
            assert!(
                (e.sprite as usize) < SPRITE_COUNT || e.sprite == SPR_NONE,
                "state {i} has invalid sprite index {}",
                e.sprite
            );
        }
    }

    // -----------------------------------------------------------------------
    // Hell Knight own states
    // -----------------------------------------------------------------------

    #[test]
    fn hell_knight_has_own_states() {
        let s = &STATES[ids::S_BOS2_STND as usize];
        assert_eq!(s.sprite, SPR_BOS2);
        assert_eq!(s.action, LOOK);
    }

    /// Regression: verify 5-frame death chains for every original monster.
    /// Each must have DIE1→DIE2→DIE3→DIE4→DIE5→S_NULL with DIE5.tics == -1.
    #[test]
    fn all_original_monsters_have_five_frame_death_chains() {
        let cases: &[(&str, u16, u16, u16, u16, u16)] = &[
            (
                "Trooper",
                ids::S_POSS_DIE1,
                ids::S_POSS_DIE2,
                ids::S_POSS_DIE3,
                ids::S_POSS_DIE4,
                ids::S_POSS_DIE5,
            ),
            (
                "Sergeant",
                ids::S_SPOS_DIE1,
                ids::S_SPOS_DIE2,
                ids::S_SPOS_DIE3,
                ids::S_SPOS_DIE4,
                ids::S_SPOS_DIE5,
            ),
            (
                "Imp",
                ids::S_TROO_DIE1,
                ids::S_TROO_DIE2,
                ids::S_TROO_DIE3,
                ids::S_TROO_DIE4,
                ids::S_TROO_DIE5,
            ),
            (
                "Demon",
                ids::S_SARG_DIE1,
                ids::S_SARG_DIE2,
                ids::S_SARG_DIE3,
                ids::S_SARG_DIE4,
                ids::S_SARG_DIE5,
            ),
            (
                "Cacodemon",
                ids::S_HEAD_DIE1,
                ids::S_HEAD_DIE2,
                ids::S_HEAD_DIE3,
                ids::S_HEAD_DIE4,
                ids::S_HEAD_DIE5,
            ),
            (
                "Baron",
                ids::S_BOSS_DIE1,
                ids::S_BOSS_DIE2,
                ids::S_BOSS_DIE3,
                ids::S_BOSS_DIE4,
                ids::S_BOSS_DIE5,
            ),
            (
                "Cyberdemon",
                ids::S_CYBER_DIE1,
                ids::S_CYBER_DIE2,
                ids::S_CYBER_DIE3,
                ids::S_CYBER_DIE4,
                ids::S_CYBER_DIE5,
            ),
            (
                "Spider",
                ids::S_SPID_DIE1,
                ids::S_SPID_DIE2,
                ids::S_SPID_DIE3,
                ids::S_SPID_DIE4,
                ids::S_SPID_DIE5,
            ),
            (
                "HellKnight",
                ids::S_BOS2_DIE1,
                ids::S_BOS2_DIE2,
                ids::S_BOS2_DIE3,
                ids::S_BOS2_DIE4,
                ids::S_BOS2_DIE5,
            ),
        ];
        for &(name, d1, d2, d3, d4, d5) in cases {
            assert_eq!(
                STATES[d1 as usize].next_state,
                StateNum(d2),
                "{name} DIE1→DIE2"
            );
            assert_eq!(
                STATES[d2 as usize].next_state,
                StateNum(d3),
                "{name} DIE2→DIE3"
            );
            assert_eq!(
                STATES[d3 as usize].next_state,
                StateNum(d4),
                "{name} DIE3→DIE4"
            );
            assert_eq!(
                STATES[d4 as usize].next_state,
                StateNum(d5),
                "{name} DIE4→DIE5"
            );
            // Walk the remaining chain (the Demon has a 6th frame) until the
            // terminal `-1` frame, which must hand off to S_NULL.
            let mut cur = d5;
            for _ in 0..4 {
                if STATES[cur as usize].tics == -1 {
                    break;
                }
                cur = STATES[cur as usize].next_state.0;
            }
            assert_eq!(
                STATES[cur as usize].tics, -1,
                "{name} death chain must reach a hold-forever frame"
            );
            assert_eq!(
                STATES[cur as usize].next_state,
                StateNum(ids::S_NULL),
                "{name} final death frame → S_NULL"
            );
        }
    }

    #[test]
    fn hell_knight_death_terminates() {
        // DIE2 now chains to DIE3; the terminal state is DIE5.
        assert_eq!(
            STATES[ids::S_BOS2_DIE2 as usize].next_state,
            StateNum(ids::S_BOS2_DIE3)
        );
        assert_eq!(STATES[ids::S_BOS2_DIE5 as usize].tics, -1);
        assert_eq!(
            STATES[ids::S_BOS2_DIE5 as usize].next_state,
            StateNum(ids::S_NULL)
        );
    }
}
