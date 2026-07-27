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

#[cfg(feature = "loom")]
use loom::sync::Arc;
#[cfg(not(feature = "loom"))]
use std::sync::Arc;

#[cfg(all(test, feature = "loom"))]
use loom::sync::atomic::{AtomicUsize, Ordering};
#[cfg(all(test, not(feature = "loom")))]
use std::sync::atomic::{AtomicUsize, Ordering};

use doom_audio::{
    AudioDriver, GenmidiBank, MAX_CHANNELS, MusScore, SfxCache, SfxPriority, mixer::PcmSample,
};
use doom_wad::WadStack;

// ---------------------------------------------------------------------------
// AudioEvent
// ---------------------------------------------------------------------------

/// Commands the game loop sends to the audio background thread.
// StopMusic and try_open_null are public API; silence dead_code warnings.
#[allow(dead_code)]
pub(crate) enum AudioEvent {
    /// Play a sound effect with the given priority.
    /// `sfx_id` is the sequential DS* lump index from `populate_sfx_cache`.
    /// Higher-priority sounds steal channels from lower-priority ones when
    /// all 8 channels are occupied — matching Doom's original behaviour.
    /// Fields: (sfx_id, priority, volume 0‥1, pan -1‥1)
    PlaySfx(u16, SfxPriority, f32, f32, Option<doom_game::MobjHandle>),
    /// Start playing a MUS track.  `data` is the raw MUS lump bytes.
    StartMusic(std::sync::Arc<[u8]>),
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
pub(crate) struct AudioSystem {
    sender: std::sync::mpsc::Sender<AudioEvent>,
    /// Keeps the cpal stream alive.  Must stay on the same thread that called
    /// `AudioDriver::open` (the main thread).
    _driver: AudioDriver,
    /// Per-system count of `StartMusic` events.  Test-only: absent in production
    /// builds so there is no runtime overhead outside the test harness.
    #[cfg(test)]
    music_start_count: Arc<AtomicUsize>,
}

// try_open_null and stop_music are used in tests and are intentional public API;
// the binary crate warns on items only referenced in #[cfg(test)] blocks.
#[allow(dead_code)]
impl AudioSystem {
    /// Attempt to open the system default audio output.
    ///
    /// Returns `None` if no audio device is available (headless CI, etc.).
    /// The game continues silently in that case — no crash.
    pub(crate) fn try_open(wad: &WadStack) -> Option<Self> {
        const SAMPLE_RATE: u32 = 44_100;

        let driver = match AudioDriver::open(SAMPLE_RATE) {
            Ok(d) => d,
            Err(e) => {
                log::warn!("[audio] INIT FAILED: {e} — running silently");
                return None;
            }
        };

        // Clone the Arc<Mutex<SfxMixer>> so the background thread can post samples.
        let mixer_arc = driver.mixer.clone();
        // Clone the Arc<Mutex<MidiPlayer>> so the background thread can load/stop scores.
        let midi_arc = driver.midi.clone();

        // Pre-populate SfxCache from WAD lumps whose names start with "DS".
        let mut sfx_cache = SfxCache::new();
        populate_sfx_cache(wad, &mut sfx_cache);

        // Try to load the GENMIDI bank for real FM instrument sounds.
        // Gracefully falls back to the default sine-wave instrument if absent or malformed.
        let genmidi_bank =
            wad.lump_data("GENMIDI")
                .and_then(|data| match GenmidiBank::parse(data) {
                    Ok(bank) => {
                        log::debug!(
                            "[audio] GENMIDI loaded: {} instruments",
                            bank.instruments.len()
                        );
                        Some(bank)
                    }
                    Err(e) => {
                        log::warn!(
                            "[audio] GENMIDI parse failed: {e} — using default sine instrument"
                        );
                        None
                    }
                });

        let (tx, rx) = std::sync::mpsc::channel::<AudioEvent>();

        #[cfg(test)]
        let music_start_count = Arc::new(AtomicUsize::new(0));
        #[cfg(test)]
        let on_music_start = {
            let c = Arc::clone(&music_start_count);
            move || {
                c.fetch_add(1, Ordering::SeqCst);
            }
        };
        #[cfg(not(test))]
        let on_music_start = || {};

        log::debug!(
            "[audio] SfxCache: {} entries loaded; spawning thread",
            sfx_cache.len()
        );

        // Spawn the audio command thread.  It owns SfxCache and shared Arcs.
        std::thread::spawn(move || {
            // Apply GENMIDI bank to the player if one was found in the WAD.
            if let Some(bank) = genmidi_bank {
                if let Ok(mut mp) = midi_arc.lock() {
                    mp.load_genmidi(bank);
                    log::debug!("[audio] GENMIDI applied to MidiPlayer");
                }
            } else {
                log::debug!("[audio] no GENMIDI — using default sine-wave instrument");
            }
            log::debug!("[audio] thread started");
            audio_cmd_thread(rx, &mixer_arc, &midi_arc, &sfx_cache, on_music_start);
            log::debug!("[audio] thread exited");
        });

        Some(Self {
            sender: tx,
            _driver: driver,
            #[cfg(test)]
            music_start_count,
        })
    }

    /// Create a null (silent) audio system for use in tests.
    ///
    /// Uses `AudioDriver::null()` which requires no real audio device.
    #[must_use]
    pub(crate) fn try_open_null() -> Option<Self> {
        let driver = AudioDriver::null();
        let mixer_arc = driver.mixer.clone();
        let midi_arc = driver.midi.clone();
        let sfx_cache = SfxCache::new();

        let (tx, rx) = std::sync::mpsc::channel::<AudioEvent>();

        #[cfg(test)]
        let music_start_count = Arc::new(AtomicUsize::new(0));
        #[cfg(test)]
        let on_music_start = {
            let c = Arc::clone(&music_start_count);
            move || {
                c.fetch_add(1, Ordering::SeqCst);
            }
        };
        #[cfg(not(test))]
        let on_music_start = || {};

        std::thread::spawn(move || {
            audio_cmd_thread(rx, &mixer_arc, &midi_arc, &sfx_cache, on_music_start);
        });

        Some(Self {
            sender: tx,
            _driver: driver,
            #[cfg(test)]
            music_start_count,
        })
    }

    /// Send a play-SFX command with explicit priority.
    ///
    /// The 8-channel mixer will steal the lowest-priority channel if all are
    /// occupied and the new sound's priority is >= that channel's priority.
    /// Fire-and-forget: silently ignored if the audio thread has exited.
    pub(crate) fn play_sfx(
        &self,
        sfx_id: u16,
        priority: SfxPriority,
        volume: f32,
        pan: f32,
        origin: Option<doom_game::MobjHandle>,
    ) {
        let _ = self
            .sender
            .send(AudioEvent::PlaySfx(sfx_id, priority, volume, pan, origin));
    }

    /// Send a start-music command with raw MUS lump bytes.
    pub(crate) fn start_music(&self, data: std::sync::Arc<[u8]>) {
        let _ = self.sender.send(AudioEvent::StartMusic(data));
    }

    /// Send a stop-music command.
    pub(crate) fn stop_music(&self) {
        let _ = self.sender.send(AudioEvent::StopMusic);
    }

    #[cfg(test)]
    pub(crate) fn debug_music_start_count(&self) -> usize {
        self.music_start_count.load(Ordering::SeqCst)
    }
}

// ---------------------------------------------------------------------------
// SFX lump scanning
// ---------------------------------------------------------------------------

/// Populate `cache` with all SFX lumps found in `wad`.
///
/// Uses [`sfx_candidate_names`] for lump discovery — the same ordering that
/// [`build_sfx_lookup`] uses — so IDs are always consistent.
fn populate_sfx_cache(wad: &WadStack, cache: &mut SfxCache) {
    for (idx, name) in sfx_candidate_names(wad).enumerate() {
        let id = (idx + 1) as u16;
        if let Some(data) = wad.lump_data(name.as_str()) {
            match PcmSample::parse_sfx_lump(data) {
                Ok(sample) => {
                    cache.insert(id, Arc::new(sample));
                }
                Err(_) => {
                    // Silently skip malformed lumps; non-fatal.
                }
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
    mixer_arc: &doom_audio::driver::SharedSfxMixer,
    midi_arc: &doom_audio::driver::SharedMidiPlayer,
    sfx_cache: &SfxCache,
    on_music_start: impl Fn(),
) {
    let mut channel_origins = [None; MAX_CHANNELS];

    while let Ok(event) = rx.recv() {
        match event {
            AudioEvent::PlaySfx(sfx_id, priority, volume, pan, origin) => {
                let Some(sample) = sfx_cache.get(sfx_id) else {
                    continue;
                };
                let Ok(mut mixer) = mixer_arc.lock() else {
                    continue;
                };

                if let Some(origin) = origin {
                    if let Some(channel) = channel_origins.iter().position(|&o| o == Some(origin)) {
                        mixer.play_on_channel(
                            channel,
                            sfx_id,
                            std::sync::Arc::clone(&sample.data),
                            volume,
                            pan,
                            priority,
                        );
                        channel_origins[channel] = Some(origin);
                        continue;
                    }
                }

                if let Some(channel) = mixer.play(
                    sfx_id,
                    std::sync::Arc::clone(&sample.data),
                    volume,
                    pan,
                    priority,
                ) {
                    channel_origins[channel] = origin;
                }
            }

            AudioEvent::StartMusic(data) => {
                on_music_start();
                log::debug!("[music] StartMusic received, data_len={}", data.len());
                let score = match MusScore::parse(&data) {
                    Ok(s) => s,
                    Err(e) => {
                        // Non-fatal: log and continue.
                        log::warn!("[music] parse failed: {e}");
                        continue;
                    }
                };

                log::debug!(
                    "[music] score parsed: {} events, {} instruments",
                    score.events.len(),
                    score.instruments.len()
                );

                let Ok(mut mp) = midi_arc.lock() else {
                    continue;
                };
                log::debug!("[music] genmidi={}", mp.genmidi.is_some());
                mp.load_score(score);
                log::debug!("[music] score loaded — playback started");
            }

            AudioEvent::StopMusic => {
                let Ok(mut mp) = midi_arc.lock() else {
                    continue;
                };
                mp.stop();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Weapon → SFX lump name mapping
// ---------------------------------------------------------------------------

/// Map a `WeaponType` to the Doom DS* lump name for its fire sound.
///
/// Returns the canonical lump name so the caller can resolve it via
/// `sfx_lookup` — the same map used for monster sounds.  This avoids the
/// fragile hardcoded-integer approach that assumed a specific WAD lump order.
pub(crate) fn weapon_fire_sfx_lump(weapon: doom_types::weapons::WeaponType) -> &'static str {
    use doom_types::weapons::WeaponType;
    match weapon {
        WeaponType::Fist => "DSPUNCH",
        WeaponType::Chainsaw => "DSSAWFUL",
        WeaponType::Pistol | WeaponType::Chaingun => "DSPISTOL",
        WeaponType::Shotgun | WeaponType::SuperShotgun => "DSSHOTGN",
        WeaponType::RocketLauncher => "DSRLAUNC",
        WeaponType::PlasmaRifle => "DSPLASMA",
        WeaponType::Bfg => "DSBFG",
    }
}

/// Map a game sound request to its Doom DS* lump name and priority.
pub(crate) fn sound_request_sfx(
    req: doom_game::SoundRequest,
) -> Option<(&'static str, SfxPriority)> {
    match req {
        doom_game::SoundRequest::MonsterWake(kind, _, _, _) => {
            Some((monster_wake_lump(kind), SfxPriority::High))
        }
        doom_game::SoundRequest::MonsterAttack(kind, _, _, _) => {
            Some((monster_attack_lump(kind), SfxPriority::Medium))
        }
        doom_game::SoundRequest::MonsterDie(kind, _, _, _) => {
            Some((monster_death_lump(kind), SfxPriority::High))
        }
        doom_game::SoundRequest::PlayerWeaponFire(weapon) => {
            Some((weapon_fire_sfx_lump(weapon), SfxPriority::Weapon))
        }
        doom_game::SoundRequest::PlayerSuperShotgunOpen => Some(("DSDBOPN", SfxPriority::Weapon)),
        doom_game::SoundRequest::PlayerSuperShotgunLoad => Some(("DSDBLOAD", SfxPriority::Weapon)),
        doom_game::SoundRequest::PlayerSuperShotgunClose => Some(("DSDBCLS", SfxPriority::Weapon)),
        doom_game::SoundRequest::PlayerDie => Some(("DSPLDETH", SfxPriority::Weapon)),
        doom_game::SoundRequest::PlayerUseFail => Some(("DSNOWAY", SfxPriority::High)),
        doom_game::SoundRequest::PlayerUseLockedDoor(_) => Some(("DSOOF", SfxPriority::High)),
    }
}

/// Build a name → SFX ID lookup map over all DS* lumps in `wad`.
///
/// Uses the same candidate-lump strategy as [`populate_sfx_cache`] so that
/// `sfx_lookup["DSPISTOL"]` always returns the same ID the mixer uses.
pub(crate) fn build_sfx_lookup(
    wad: &WadStack,
) -> std::collections::HashMap<doom_wad::lump::LumpName, u16> {
    let mut map = std::collections::HashMap::new();
    for (idx, name) in sfx_candidate_names(wad).enumerate() {
        map.insert(name, (idx + 1) as u16);
    }
    map
}

/// Collect the names (uppercase) of all candidate DS* SFX lumps in `wad`.
///
/// Scans all lumps in the WAD directory whose names start with `"DS"` and
/// have non-zero size.  Both `build_sfx_lookup` and `populate_sfx_cache`
/// must call this function to guarantee that name→ID assignments match.
///
/// Namespace markers (DS_START/DS_END) are intentionally ignored: they
/// do not reliably contain all DS-prefixed SFX lumps in every WAD variant.
fn sfx_candidate_names<'a>(
    wad: &'a WadStack,
) -> impl Iterator<Item = doom_wad::lump::LumpName> + 'a {
    wad.all_lumps()
        .filter(|(_, l)| l.size > 0 && l.name.as_str().starts_with("DS"))
        .map(|(_, l)| doom_wad::lump::LumpName::from_str(&l.name.as_str().to_ascii_uppercase()))
}

/// Return the Doom DS* lump name for a monster's wake (see) sound.
///
/// Returns `""` for kinds that have no wake sound (projectiles, pickups, etc.).
pub(crate) fn monster_wake_lump(kind: doom_types::mobj_kind::MobjKind) -> &'static str {
    use doom_types::mobj_kind::MobjKind;
    match kind {
        MobjKind::Trooper => "DSPOSSIT",
        MobjKind::Sergeant => "DSSGTSIT",
        MobjKind::Imp => "DSBGSIT1",
        MobjKind::Demon | MobjKind::Spectre => "DSSGTSIT",
        MobjKind::LostSoul => "DSSKLATK",
        MobjKind::Cacodemon => "DSCACSIT",
        MobjKind::BaronOfHell => "DSBRSSIT",
        MobjKind::HellKnight => "DSKNTSIT",
        MobjKind::Arachnotron => "DSBSPIT",
        MobjKind::PainElemental => "DSPESIT",
        MobjKind::Revenant => "DSSKESIT",
        MobjKind::Mancubus => "DSMNTSIT",
        MobjKind::ArchVile => "DSVILSIT",
        MobjKind::SpiderMastermind => "DSSPIDSIT",
        MobjKind::Cyberdemon => "DSCYBSIT",
        MobjKind::WolfSS => "DSSPOSSIT",
        _ => "",
    }
}

/// Return the Doom DS* lump name for a monster's attack sound.
///
/// Returns `""` for kinds that have no dedicated attack sound.
pub(crate) fn monster_attack_lump(kind: doom_types::mobj_kind::MobjKind) -> &'static str {
    use doom_types::mobj_kind::MobjKind;
    match kind {
        MobjKind::Trooper | MobjKind::WolfSS => "DSPISTOL",
        MobjKind::Sergeant => "DSSHOTGN",
        MobjKind::Imp => "DSBGSIT1",
        MobjKind::Demon | MobjKind::Spectre => "DSSGTATK",
        MobjKind::Cacodemon => "DSCLAW1",
        MobjKind::BaronOfHell | MobjKind::HellKnight => "DSBAREXP",
        MobjKind::Revenant => "DSSKEATK",
        MobjKind::Mancubus => "DSFIRSHT",
        MobjKind::ArchVile => "DSVILATK",
        MobjKind::Arachnotron => "DSBSPIT",
        MobjKind::Cyberdemon => "DSRLAUNC",
        MobjKind::SpiderMastermind => "DSSHOTGN",
        _ => "",
    }
}

/// Return the Doom DS* lump name for a monster's death sound.
///
/// Returns `""` for kinds that have no death sound.
pub(crate) fn monster_death_lump(kind: doom_types::mobj_kind::MobjKind) -> &'static str {
    use doom_types::mobj_kind::MobjKind;
    match kind {
        MobjKind::Trooper | MobjKind::WolfSS => "DSPODTH1",
        MobjKind::Sergeant => "DSSGTDTH",
        MobjKind::Imp => "DSBGDTH1",
        MobjKind::Demon | MobjKind::Spectre => "DSSGTDTH",
        MobjKind::LostSoul => "DSFIRXPL",
        MobjKind::Cacodemon => "DSCACDTH",
        MobjKind::BaronOfHell => "DSBRDTH1",
        MobjKind::HellKnight => "DSKNTDTH",
        MobjKind::Arachnotron => "DSBSDTH",
        MobjKind::PainElemental => "DSPEDTH",
        MobjKind::Revenant => "DSSKEPCH1",
        MobjKind::Mancubus => "DSMNTDTH",
        MobjKind::ArchVile => "DSVILDTH",
        MobjKind::SpiderMastermind => "DSSPIDTH",
        MobjKind::Cyberdemon => "DSCYBDTH",
        _ => "",
    }
}

// ---------------------------------------------------------------------------
// Music lump name helpers
// ---------------------------------------------------------------------------

/// Build the WAD music lump name for a map identifier string.
///
/// - `"E1M1"` → `"D_E1M1"`
/// - `"MAP1"` → `"D_MAP01"`
/// - `"MAP01"` → `"D_MAP01"`
/// - Unknown format → `None`
///
/// ## Examples
///
/// ```rust
/// use doom_app::audio_system::music_lump_for_map;
///
/// assert_eq!(music_lump_for_map("E1M1"), Some("D_E1M1".to_string()));
/// assert_eq!(music_lump_for_map("MAP01"), Some("D_MAP01".to_string()));
/// assert_eq!(music_lump_for_map("INVALID"), None);
/// ```
pub(crate) fn music_lump_for_map(map: &str) -> Option<String> {
    Some(format!(
        "D_{}",
        doom_game::MapId::from_name(map)?.map_name()
    ))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    fn stack_with_iwad_bytes(wad_bytes: Vec<u8>) -> WadStack {
        let mut stack = WadStack::new();
        stack
            .push_iwad(wad_bytes)
            .expect("IWAD test bytes must push");
        stack
    }

    #[allow(dead_code)]
    fn make_iwad(lumps: &[(&str, &[u8])]) -> Vec<u8> {
        let mut data: Vec<u8> = Vec::new();
        data.extend_from_slice(b"IWAD");
        data.extend_from_slice(&(lumps.len() as i32).to_le_bytes());
        data.extend_from_slice(&0i32.to_le_bytes()); // dir offset placeholder

        let mut offsets: Vec<(usize, usize)> = Vec::new();
        for (_, lump_bytes) in lumps {
            let pos = data.len();
            data.extend_from_slice(lump_bytes);
            offsets.push((pos, lump_bytes.len()));
        }

        let dir_offset = data.len() as i32;
        data[8..12].copy_from_slice(&dir_offset.to_le_bytes());

        for (i, (name, _)) in lumps.iter().enumerate() {
            let (filepos, size) = offsets[i];
            data.extend_from_slice(&(filepos as i32).to_le_bytes());
            data.extend_from_slice(&(size as i32).to_le_bytes());
            let mut name_buf = [0u8; 8];
            for (j, &b) in name.as_bytes().iter().take(8).enumerate() {
                name_buf[j] = b.to_ascii_uppercase();
            }
            data.extend_from_slice(&name_buf);
        }

        data
    }

    #[allow(dead_code)]
    fn valid_sfx_lump_1_sample() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&3u16.to_le_bytes()); // format
        data.extend_from_slice(&11_025u16.to_le_bytes()); // sample_rate
        data.extend_from_slice(&1u32.to_le_bytes()); // sample_count
        data.push(128u8); // one silent sample
        data
    }

    #[allow(dead_code)]
    fn test_pcm_sample() -> Arc<PcmSample> {
        Arc::new(PcmSample {
            sample_rate: 11_025,
            data: vec![200u8; 512].into(),
        })
    }

    #[allow(dead_code)]
    fn run_audio_events(events: Vec<AudioEvent>) -> usize {
        let driver = AudioDriver::null();
        let mixer = driver.mixer.clone();
        let midi = driver.midi.clone();
        let mut cache = SfxCache::new();
        cache.insert(1, test_pcm_sample());
        cache.insert(2, test_pcm_sample());

        let (tx, rx) = std::sync::mpsc::channel();
        for event in events {
            tx.send(event).expect("test send should succeed");
        }
        drop(tx);

        audio_cmd_thread(rx, &mixer, &midi, &cache, || {});

        // Since we can't `try_unwrap` easily when the underlying type might
        // be `loom::sync::Arc`, we just lock it and extract the data we need for the tests
        let count = mixer
            .lock()
            .expect("value must exist in test")
            .active_count();
        drop(driver); // make sure driver lives long enough
        count
    }

    #[test]
    #[cfg(not(feature = "loom"))]
    fn audio_system_try_open_null_does_not_panic() {
        // Verifies that try_open_null() succeeds regardless of audio device
        // availability (it bypasses cpal entirely).
        let system = AudioSystem::try_open_null();
        assert!(system.is_some(), "try_open_null must always succeed");
    }

    #[test]
    #[cfg(not(feature = "loom"))]
    fn send_events_to_null_system_does_not_panic() {
        let system = AudioSystem::try_open_null().expect("null audio must succeed");
        // Fire-and-forget: none of these should panic.
        system.play_sfx(32, doom_audio::SfxPriority::Medium, 1.0, 0.0, None);
        system.start_music(std::sync::Arc::<[u8]>::from(vec![0u8; 4])); // invalid MUS — audio thread logs and continues
        system.stop_music();
    }

    #[test]
    #[cfg(not(feature = "loom"))]
    fn audio_cmd_thread_reuses_channel_for_same_origin() {
        let origin = doom_game::MobjHandle {
            index: 7,
            generation: 3,
        };
        let active_count = run_audio_events(vec![
            AudioEvent::PlaySfx(1, SfxPriority::High, 1.0, 0.0, Some(origin)),
            AudioEvent::PlaySfx(2, SfxPriority::Medium, 0.6, -0.3, Some(origin)),
        ]);

        assert_eq!(
            active_count, 1,
            "same-origin sound retriggers should restart one live channel, not allocate two"
        );
    }

    #[test]
    #[cfg(not(feature = "loom"))]
    fn audio_cmd_thread_keeps_distinct_origins_on_distinct_channels() {
        let active_count = run_audio_events(vec![
            AudioEvent::PlaySfx(
                1,
                SfxPriority::High,
                1.0,
                0.0,
                Some(doom_game::MobjHandle {
                    index: 1,
                    generation: 1,
                }),
            ),
            AudioEvent::PlaySfx(
                2,
                SfxPriority::High,
                0.8,
                0.2,
                Some(doom_game::MobjHandle {
                    index: 2,
                    generation: 1,
                }),
            ),
        ]);

        assert_eq!(
            active_count, 2,
            "different origins should still occupy distinct channels"
        );
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
    fn music_lump_for_map_map1_zero_pads() {
        assert_eq!(music_lump_for_map("map1"), Some("D_MAP01".to_string()));
    }

    #[test]
    #[cfg(not(feature = "loom"))]
    fn music_lump_for_map_rejects_malformed_e_format() {
        assert_eq!(music_lump_for_map("E1MX"), None);
    }

    #[test]
    fn music_lump_for_map_unknown_returns_none() {
        assert_eq!(music_lump_for_map("INVALID"), None);
    }

    #[test]
    fn weapon_fire_sfx_lump_pistol() {
        assert_eq!(
            weapon_fire_sfx_lump(doom_types::weapons::WeaponType::Pistol),
            "DSPISTOL"
        );
    }

    #[test]
    fn weapon_fire_sfx_lump_bfg() {
        assert_eq!(
            weapon_fire_sfx_lump(doom_types::weapons::WeaponType::Bfg),
            "DSBFG"
        );
    }

    #[test]
    fn weapon_fire_sfx_lump_chaingun_same_as_pistol() {
        assert_eq!(
            weapon_fire_sfx_lump(doom_types::weapons::WeaponType::Chaingun),
            weapon_fire_sfx_lump(doom_types::weapons::WeaponType::Pistol)
        );
    }

    #[test]
    fn sound_request_sfx_maps_player_weapon_fire_to_weapon_lump() {
        assert_eq!(
            sound_request_sfx(doom_game::SoundRequest::PlayerWeaponFire(
                doom_types::weapons::WeaponType::Shotgun
            )),
            Some(("DSSHOTGN", doom_audio::SfxPriority::Weapon))
        );
    }

    #[test]
    fn sound_request_sfx_maps_ssg_mechanism_sounds() {
        assert_eq!(
            sound_request_sfx(doom_game::SoundRequest::PlayerSuperShotgunOpen),
            Some(("DSDBOPN", doom_audio::SfxPriority::Weapon))
        );
        assert_eq!(
            sound_request_sfx(doom_game::SoundRequest::PlayerSuperShotgunLoad),
            Some(("DSDBLOAD", doom_audio::SfxPriority::Weapon))
        );
        assert_eq!(
            sound_request_sfx(doom_game::SoundRequest::PlayerSuperShotgunClose),
            Some(("DSDBCLS", doom_audio::SfxPriority::Weapon))
        );
    }

    #[test]
    #[cfg(not(feature = "loom"))]
    fn sound_request_sfx_maps_player_use_fail_to_noway() {
        assert_eq!(
            sound_request_sfx(doom_game::SoundRequest::PlayerUseFail),
            Some(("DSNOWAY", doom_audio::SfxPriority::High))
        );
    }

    #[test]
    #[cfg(not(feature = "loom"))]
    fn sound_request_sfx_maps_locked_door_feedback_to_oof() {
        assert_eq!(
            sound_request_sfx(doom_game::SoundRequest::PlayerUseLockedDoor(
                doom_game::LockedDoorColor::Blue,
            )),
            Some(("DSOOF", doom_audio::SfxPriority::High))
        );
    }

    #[test]
    #[cfg(not(feature = "loom"))]
    fn sfx_cache_empty_on_empty_wad_does_not_panic() {
        // Build a minimal IWAD with no lumps and verify populate_sfx_cache
        // completes without panicking.
        let mut wad_bytes = Vec::new();
        wad_bytes.extend_from_slice(b"IWAD");
        wad_bytes.extend_from_slice(&0i32.to_le_bytes()); // numlumps = 0
        wad_bytes.extend_from_slice(&12i32.to_le_bytes()); // directory at offset 12
        let wad = stack_with_iwad_bytes(wad_bytes);
        let mut cache = SfxCache::new();
        populate_sfx_cache(&wad, &mut cache);
        // No entries — no panic.
        assert!(cache.get(1).is_none());
    }

    #[test]
    #[cfg(not(feature = "loom"))]
    fn sfx_cache_without_ds_markers_falls_back_to_ds_prefix_scan() {
        let sfx = valid_sfx_lump_1_sample();
        let wad_bytes = make_iwad(&[("THINGS", b"not_sfx"), ("DSPISTOL", sfx.as_slice())]);
        let wad = stack_with_iwad_bytes(wad_bytes);

        let mut cache = SfxCache::new();
        populate_sfx_cache(&wad, &mut cache);

        assert!(
            cache.get(1).is_some(),
            "without DS markers, DS-prefixed lumps should be indexed from 1"
        );
    }
}
