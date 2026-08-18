//! Vanilla IWAD / game-mode auto-detection.
//!
//! Ports Chocolate Doom's `d_iwad.c` (the `iwads[]` filename table +
//! `IdentifyIWADByName`) and `d_main.c` (`D_IdentifyVersion`) so that the
//! engine can tell shareware from registered from Ultimate from Doom II purely
//! from the IWAD it was handed.
//!
//! These types are DELIBERATELY separate from `doom_game::GameMode`
//! (`SinglePlayer`/`Deathmatch`) — that enum is the network game *type*, an
//! unrelated concept. The types here carry vanilla release semantics.

use std::path::Path;

use doom_wad::WadStack;

/// Vanilla release / content tier, mirroring Chocolate Doom's `GameMode_t`.
///
/// * `Shareware`  — Doom 1 shareware: episode 1 only (E1M1..E1M9).
/// * `Registered` — Doom 1 registered: episodes 1-3.
/// * `Retail`     — The Ultimate Doom: episodes 1-4.
/// * `Commercial` — Doom II / Final Doom: MAP01..MAP32.
/// * `Indetermined` — could not be identified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameMode {
    Shareware,
    Registered,
    Commercial,
    Retail,
    Indetermined,
}

/// Which game/mission the IWAD contains, mirroring Chocolate Doom's
/// `GameMission_t`. Doom II mission packs (`PackTnt`/`PackPlutonia`) share the
/// commercial map layout but differ in content; Chex/HACX are the two total
/// conversions Chocolate ships detection for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameMission {
    Doom,
    Doom2,
    PackTnt,
    PackPlutonia,
    PackChex,
    PackHacx,
    None,
}

impl GameMode {
    /// The highest episode number valid for this release (Doom-1 family).
    /// Commercial builds use MAPxx rather than episodes, so this is only
    /// meaningful for the `E{e}M{m}` families; it returns `None` for
    /// commercial/indetermined.
    pub fn max_episode(self) -> Option<u8> {
        match self {
            GameMode::Shareware => Some(1),
            GameMode::Registered => Some(3),
            GameMode::Retail => Some(4),
            GameMode::Commercial | GameMode::Indetermined => None,
        }
    }
}

/// Does a lump with this exact name exist anywhere in the stack?
fn has_lump(wad_stack: &WadStack, name: &str) -> bool {
    wad_stack.lump_data(name).is_some()
}

/// The lowercased final path component (filename) of `iwad_path`.
fn iwad_filename(iwad_path: &Path) -> String {
    iwad_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// Identify the game version from the loaded IWAD, mirroring Chocolate Doom's
/// `D_FindIWAD`/`IdentifyIWADByName` (filename hint) combined with
/// `D_IdentifyVersion` (lump-content refinement).
///
/// `wad_stack` must already have the IWAD pushed (and no PWADs yet, matching
/// vanilla, which identifies before `-file` merges).
pub fn identify_version(wad_stack: &WadStack, iwad_path: &Path) -> (GameMode, GameMission) {
    let name = iwad_filename(iwad_path);

    // Chex Quest and HACX are total conversions whose lump layout does NOT fit
    // the standard episode/MAPxx heuristic (chex.wad ships only E1M1..E1M5, so
    // the E4/E3 episode probe below would misfire and report shareware). Their
    // mode/mission come from the iwads[] filename table verbatim, exactly as
    // Chocolate's IdentifyIWADByName seeds them before D_IdentifyVersion runs.
    if name == "chex.wad" {
        return (GameMode::Retail, GameMission::PackChex);
    }
    if name == "hacx.wad" {
        return (GameMode::Commercial, GameMission::PackHacx);
    }

    let has_map01 = has_lump(wad_stack, "MAP01");
    let has_e1m1 = has_lump(wad_stack, "E1M1");

    // Commercial family (Doom II / Final Doom): MAP01 present and no E1M1.
    // Mirrors D_IdentifyVersion, which sets gamemission=doom2 on the first
    // MAP01 it sees.
    if has_map01 && !has_e1m1 {
        // note: doom2 vs tnt vs plutonia cannot be told apart by lumps — every
        // commercial IWAD is MAP01..MAP32 — so vanilla/Chocolate fall back to
        // the IWAD *filename* here. We do the same.
        let mission = commercial_mission_from_name(&name);
        return (GameMode::Commercial, mission);
    }

    // Doom-1 family (episodes): E1M1 present. Mode is decided by which episode
    // markers exist, following Chocolate's D_IdentifyVersion:
    //   E4M1 present -> retail (Ultimate); else E3M1 present -> registered;
    //   else shareware.
    if has_e1m1 {
        let mode = if has_lump(wad_stack, "E4M1") {
            GameMode::Retail
        } else if has_lump(wad_stack, "E3M1") {
            // note: Chocolate's D_IdentifyVersion probes E3M1 for registered
            // (registered Doom ships E1-E3, so E3M1 is the decisive lump). The
            // task brief said "E2M1"; we follow Chocolate. Both agree on every
            // real IWAD and on the synthetic test corpus.
            GameMode::Registered
        } else {
            GameMode::Shareware
        };
        return (mode, GameMission::Doom);
    }

    // Neither MAP01 nor E1M1: fall back to the filename hint if we have one,
    // otherwise we genuinely cannot tell (Chocolate would I_Error here).
    if let Some(hint) = filename_hint(&name) {
        return hint;
    }
    (GameMode::Indetermined, GameMission::None)
}

/// Commercial mission from the IWAD filename (Chocolate's iwads[] table). Any
/// unrecognized commercial IWAD defaults to plain Doom II.
fn commercial_mission_from_name(name: &str) -> GameMission {
    match name {
        "tnt.wad" => GameMission::PackTnt,
        "plutonia.wad" => GameMission::PackPlutonia,
        // doom2.wad, doom2f.wad, freedoom2.wad, freedm.wad, or anything else.
        _ => GameMission::Doom2,
    }
}

/// The full (mode, mission) hint from the Chocolate `iwads[]` filename table,
/// used only when lump content was inconclusive. `freedoom1`/`freedoom2`/
/// `freedm` are covered by the lump heuristic above (they carry standard
/// E?M?/MAP?? markers), so they are not special-cased here.
fn filename_hint(name: &str) -> Option<(GameMode, GameMission)> {
    Some(match name {
        "doom1.wad" => (GameMode::Shareware, GameMission::Doom),
        "doom.wad" | "freedoom1.wad" => (GameMode::Retail, GameMission::Doom),
        "doom2.wad" | "doom2f.wad" | "freedoom2.wad" | "freedm.wad" => {
            (GameMode::Commercial, GameMission::Doom2)
        }
        "tnt.wad" => (GameMode::Commercial, GameMission::PackTnt),
        "plutonia.wad" => (GameMode::Commercial, GameMission::PackPlutonia),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_episode_returns_correct_bounds() {
        assert_eq!(GameMode::Shareware.max_episode(), Some(1));
        assert_eq!(GameMode::Registered.max_episode(), Some(3));
        assert_eq!(GameMode::Retail.max_episode(), Some(4));
        assert_eq!(GameMode::Commercial.max_episode(), None);
        assert_eq!(GameMode::Indetermined.max_episode(), None);
    }
}
