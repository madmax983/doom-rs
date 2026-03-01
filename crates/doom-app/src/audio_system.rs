//! Audio subsystem — thin wrapper around doom-audio that bridges the game loop
//! to the PCM mixer and MUS sequencer.
//!
//! # Design
//! `AudioSystem` holds the cpal `AudioDriver` on the main thread (keeping the
//! stream alive) and spawns a background thread that handles `AudioEvent`
//! commands.  Music commands call `load_score`/`stop` on the `MidiPlayer`
//! that lives inside `AudioDriver` — the cpal callback calls
//! `MidiPlayer::advance_samples` on every buffer fill, so timing is driven
//! by the real audio clock.
//!
//! If audio initialisation fails (no device, CI, headless) `try_open` returns
//! `None` and the game runs silently — no panics, no unwraps in hot paths.

use std::sync::{Arc, Mutex};

use doom_audio::{
    AudioDriver, GenmidiBank, MidiPlayer, Mixer, MusScore, SfxCache,
    mixer::PcmSample,
    sfx::play_sfx,
};
use doom_wad::WadFile;

// ---------------------------------------------------------------------------
// AudioEvent
// ---------------------------------------------------------------------------

/// Commands the game loop sends to the audio background thread.
// StopMusic and try_open_null are public API; silence dead_code warnings.
#[allow(dead_code)]
pub enum AudioEvent {
    /// Play a sound effect.  `sfx_id` is the Doom SFX lump number
    /// (e.g. 32 = DSPISTOL).
    PlaySfx(u16),
    /// Start playing a MUS track.  `data` is the raw MUS lump bytes.
    StartMusic(Vec<u8>),
    /// Stop current music (silences the OPL sequencer).
    StopMusic,
}

// ---------------------------------------------------------------------------
// AudioSystem
// ---------------------------------------------------------------------------

/// Thin facade over the doom-audio crate.
///
/// Holds the cpal stream alive via `AudioDriver` and sends events to the
/// background audio thread via an mpsc channel.
pub struct AudioSystem {
    sender: std::sync::mpsc::Sender<AudioEvent>,
    /// Keeps the cpal stream alive.  Must stay on the same thread that called
    /// `AudioDriver::open` (the main thread).
    _driver: AudioDriver,
}

// try_open_null and stop_music are used in tests and are intentional public API;
// the binary crate warns on items only referenced in #[cfg(test)] blocks.
#[allow(dead_code)]
impl AudioSystem {
    /// Attempt to open the system default audio output.
    ///
    /// Returns `None` if no audio device is available (headless CI, etc.).
    /// The game continues silently in that case — no crash.
    pub fn try_open(wad: &WadFile) -> Option<Self> {
        const SAMPLE_RATE: u32 = 44_100;

        let driver = AudioDriver::open(SAMPLE_RATE).ok()?;

        // Clone the Arc<Mutex<Mixer>> so the background thread can post samples.
        let mixer_arc: Arc<Mutex<Mixer>> = Arc::clone(&driver.mixer);
        // Clone the Arc<Mutex<MidiPlayer>> so the background thread can load/stop scores.
        let midi_arc: Arc<Mutex<MidiPlayer>> = Arc::clone(&driver.midi);

        // Pre-populate SfxCache from WAD lumps whose names start with "DS".
        let mut sfx_cache = SfxCache::new();
        populate_sfx_cache(wad, &mut sfx_cache);

        // Try to load the GENMIDI bank for real FM instrument sounds.
        // Gracefully falls back to the default sine-wave instrument if absent or malformed.
        let genmidi_bank = wad
            .find_lump_data("GENMIDI")
            .and_then(|data| GenmidiBank::parse(data).ok());

        let (tx, rx) = std::sync::mpsc::channel::<AudioEvent>();

        // Spawn the audio command thread.  It owns SfxCache and shared Arcs.
        std::thread::spawn(move || {
            // Apply GENMIDI bank to the player if one was found in the WAD.
            if let Some(bank) = genmidi_bank {
                if let Ok(mut mp) = midi_arc.lock() {
                    mp.load_genmidi(bank);
                }
            }
            audio_cmd_thread(rx, &mixer_arc, &midi_arc, &sfx_cache);
        });

        Some(Self { sender: tx, _driver: driver })
    }

    /// Create a null (silent) audio system for use in tests.
    ///
    /// Uses `AudioDriver::null()` which requires no real audio device.
    #[must_use]
    pub fn try_open_null() -> Option<Self> {
        let driver = AudioDriver::null();
        let mixer_arc: Arc<Mutex<Mixer>> = Arc::clone(&driver.mixer);
        let midi_arc: Arc<Mutex<MidiPlayer>> = Arc::clone(&driver.midi);
        let sfx_cache = SfxCache::new();

        let (tx, rx) = std::sync::mpsc::channel::<AudioEvent>();

        std::thread::spawn(move || {
            audio_cmd_thread(rx, &mixer_arc, &midi_arc, &sfx_cache);
        });

        Some(Self { sender: tx, _driver: driver })
    }

    /// Send a play-SFX command (fire-and-forget; silently ignored if the
    /// channel is full or the audio thread has exited).
    pub fn play_sfx(&self, sfx_id: u16) {
        // If the receiver has dropped, the error is silently swallowed.
        let _ = self.sender.send(AudioEvent::PlaySfx(sfx_id));
    }

    /// Send a start-music command with raw MUS lump bytes.
    pub fn start_music(&self, data: Vec<u8>) {
        let _ = self.sender.send(AudioEvent::StartMusic(data));
    }

    /// Send a stop-music command.
    pub fn stop_music(&self) {
        let _ = self.sender.send(AudioEvent::StopMusic);
    }
}

// ---------------------------------------------------------------------------
// SFX lump scanning
// ---------------------------------------------------------------------------

/// Populate `cache` with all SFX lumps found in `wad`.
///
/// Strategy:
/// 1. Look for lumps between `DS_START` / `DS_END` markers.
/// 2. Fall back to scanning *all* lumps whose names begin with `"DS"`.
///
/// Each discovered lump is decoded as an 8-bit PCM sample and inserted into
/// the cache keyed by its sequential discovery index (starting at 1).
/// If decoding fails the lump is silently skipped.
fn populate_sfx_cache(wad: &WadFile, cache: &mut SfxCache) {
    // Collect candidate lumps: prefer the DS_START/DS_END namespace; fall back
    // to scanning all lumps with a "DS" prefix.
    let candidates: Vec<(u16, Vec<u8>)> = {
        let between: Vec<_> = wad.lumps_between("DS_START", "DS_END").collect();
        if !between.is_empty() {
            between
                .into_iter()
                .enumerate()
                .filter(|(_, l)| l.size > 0)
                .map(|(i, l)| {
                    let id = (i + 1) as u16;
                    let data = wad.lump_data(l).to_vec();
                    (id, data)
                })
                .collect()
        } else {
            // Fall back: all lumps whose name starts with "DS"
            wad.lumps()
                .iter()
                .filter(|l| l.size > 0 && l.name.as_str().starts_with("DS"))
                .enumerate()
                .map(|(i, l)| {
                    let id = (i + 1) as u16;
                    let data = wad.lump_data(l).to_vec();
                    (id, data)
                })
                .collect()
        }
    };

    for (id, data) in candidates {
        match PcmSample::parse_sfx_lump(&data) {
            Ok(sample) => {
                cache.insert(id, Arc::new(sample));
            }
            Err(_) => {
                // Silently skip malformed lumps; non-fatal.
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Background audio command thread
// ---------------------------------------------------------------------------

/// Main loop of the audio background thread.
///
/// Receives `AudioEvent` values and dispatches them:
/// - `PlaySfx` — queues a PCM sound into the mixer.
/// - `StartMusic` — parses the MUS data and calls `MidiPlayer::load_score`.
///   The cpal callback in `AudioDriver::open` calls `advance_samples` on
///   every buffer fill, providing real-time timed playback.
/// - `StopMusic` — calls `MidiPlayer::stop` to silence the OPL chip.
///
/// Exits when the sender side of the channel is dropped (game shutdown).
fn audio_cmd_thread(
    rx: std::sync::mpsc::Receiver<AudioEvent>,
    mixer_arc: &Arc<Mutex<Mixer>>,
    midi_arc: &Arc<Mutex<MidiPlayer>>,
    sfx_cache: &SfxCache,
) {
    while let Ok(event) = rx.recv() {
        match event {
            AudioEvent::PlaySfx(sfx_id) => {
                if let Ok(mut mixer) = mixer_arc.lock() {
                    play_sfx(&mut mixer, sfx_cache, sfx_id, 100);
                }
            }

            AudioEvent::StartMusic(data) => {
                match MusScore::parse(&data) {
                    Ok(score) => {
                        if let Ok(mut mp) = midi_arc.lock() {
                            mp.load_score(score);
                        }
                    }
                    Err(e) => {
                        // Non-fatal: log and continue.
                        eprintln!("audio: failed to parse MUS data: {e}");
                    }
                }
            }

            AudioEvent::StopMusic => {
                if let Ok(mut mp) = midi_arc.lock() {
                    mp.stop();
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Weapon → SFX ID mapping
// ---------------------------------------------------------------------------

/// Map a `WeaponType` to the Doom SFX lump ID used when the weapon fires.
///
/// IDs below are the standard Doom SFX lump numbers for the DS* sounds.
/// They correspond to the sequential indices assigned by `populate_sfx_cache`
/// only when the WAD is a stock Doom IWAD; for generic WADs the values are
/// approximate but the worst-case outcome is silence (cache miss = no-op).
pub fn weapon_fire_sfx(weapon: doom_game::WeaponType) -> u16 {
    use doom_game::WeaponType;
    match weapon {
        WeaponType::Fist | WeaponType::Chainsaw => 64, // DSPUNCH / DSSAWFUL
        WeaponType::Pistol => 32,                       // DSPISTOL
        WeaponType::Shotgun => 34,                      // DSSHOTGN
        WeaponType::SuperShotgun => 84,                 // DSDBOPN (approx)
        WeaponType::Chaingun => 35,                     // DSPISTOL repeated
        WeaponType::RocketLauncher => 36,               // DSRLAUNC
        WeaponType::PlasmaRifle => 37,                  // DSPLASMA
        WeaponType::Bfg => 39,                          // DSBFG
    }
}

// ---------------------------------------------------------------------------
// Music lump name helpers
// ---------------------------------------------------------------------------

/// Build the WAD music lump name for a map identifier string.
///
/// - `"E1M1"` → `"D_E1M1"`
/// - `"MAP01"` → `"D_MAP01"`
/// - Unknown format → `None`
pub fn music_lump_for_map(map: &str) -> Option<String> {
    let upper = map.to_ascii_uppercase();
    if upper.starts_with('E') && upper.len() == 4 {
        Some(format!("D_{upper}"))
    } else if upper.starts_with("MAP") && upper.len() == 5 {
        Some(format!("D_{upper}"))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_system_try_open_null_does_not_panic() {
        // Verifies that try_open_null() succeeds regardless of audio device
        // availability (it bypasses cpal entirely).
        let system = AudioSystem::try_open_null();
        assert!(system.is_some(), "try_open_null must always succeed");
    }

    #[test]
    fn send_events_to_null_system_does_not_panic() {
        let system = AudioSystem::try_open_null().expect("null audio must succeed");
        // Fire-and-forget: none of these should panic.
        system.play_sfx(32);
        system.start_music(vec![0u8; 4]); // invalid MUS — audio thread logs and continues
        system.stop_music();
    }

    #[test]
    fn music_lump_for_map_e1m1() {
        assert_eq!(music_lump_for_map("E1M1"), Some("D_E1M1".to_string()));
    }

    #[test]
    fn music_lump_for_map_map01() {
        assert_eq!(music_lump_for_map("MAP01"), Some("D_MAP01".to_string()));
    }

    #[test]
    fn music_lump_for_map_unknown_returns_none() {
        assert_eq!(music_lump_for_map("INVALID"), None);
    }

    #[test]
    fn weapon_fire_sfx_pistol() {
        assert_eq!(weapon_fire_sfx(doom_game::WeaponType::Pistol), 32);
    }

    #[test]
    fn weapon_fire_sfx_bfg() {
        assert_eq!(weapon_fire_sfx(doom_game::WeaponType::Bfg), 39);
    }

    #[test]
    fn sfx_cache_empty_on_empty_wad_does_not_panic() {
        // Build a minimal IWAD with no lumps and verify populate_sfx_cache
        // completes without panicking.
        let mut wad_bytes = Vec::new();
        wad_bytes.extend_from_slice(b"IWAD");
        wad_bytes.extend_from_slice(&0i32.to_le_bytes()); // numlumps = 0
        wad_bytes.extend_from_slice(&12i32.to_le_bytes()); // directory at offset 12
        let wad = doom_wad::WadFile::parse(wad_bytes).expect("minimal WAD must parse");
        let mut cache = SfxCache::new();
        populate_sfx_cache(&wad, &mut cache);
        // No entries — no panic.
        assert!(cache.get(1).is_none());
    }
}
