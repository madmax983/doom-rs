//! Offline MUS/OPL rendering helpers for faithful WAV export.
//!
//! This module is intentionally deterministic and does not depend on the
//! real-time audio callback cadence. It renders one sample per sequencer step
//! so loop boundaries are sample-accurate.

use crate::{MidiPlayer, midi::GenmidiBank, mus::MusScore};

/// Render a MUS score to a mono 16-bit PCM WAV byte vector.
///
/// `loops` controls how many complete score loops to render (minimum 1).
/// A `sample_rate` of 44_100 reproduces the classic Doom playback path.
///
/// ## Examples
/// ```
/// use doom_audio::wav::render_mus_to_wav_mono;
/// use doom_audio::mus::{MusScore, MusHeader, MusEvent};
///
/// let score = MusScore {
///     header: MusHeader { score_length: 0, score_start: 0, primary_channels: 1, secondary_channels: 0, instrument_count: 0 },
///     instruments: vec![],
///     events: vec![
///         (0, MusEvent::PlayNote { channel: 0, note: 60, volume: Some(127) }),
///         (14, MusEvent::ScoreEnd),
///     ],
/// };
///
/// let wav = render_mus_to_wav_mono(score, None, 44_100, 1);
/// assert!(wav.len() > 44); // Needs to be at least a valid header plus data
/// ```
#[must_use]
pub fn render_mus_to_wav_mono(
    score: MusScore,
    genmidi: Option<GenmidiBank>,
    sample_rate: u32,
    loops: u32,
) -> Vec<u8> {
    let target_loops = loops.max(1);

    let mut player = MidiPlayer::new();
    if let Some(bank) = genmidi {
        player.load_genmidi(bank);
    }
    player.load_score(score);

    let mut pcm_i16 = Vec::<i16>::new();
    let mut mono_sample = [0.0f32; 1];
    let mut completed_loops = 0u32;
    let mut prev_cursor = player.event_cursor;

    // Render with one-sample granularity to avoid buffer-boundary timing drift.
    while completed_loops < target_loops {
        player.advance_samples(1, sample_rate, &mut mono_sample);
        let s_temp = if mono_sample[0].is_nan() { 0.0 } else { mono_sample[0] };
        let s = (s_temp * i16::MAX as f32)
            .clamp(i16::MIN as f32, i16::MAX as f32)
            .round() as i16;
        pcm_i16.push(s);

        if prev_cursor > 0 && player.event_cursor == 0 {
            completed_loops += 1;
        }
        prev_cursor = player.event_cursor;
    }

    encode_pcm16_wav_mono(sample_rate, &pcm_i16)
}

/// Encode mono PCM16 samples as a RIFF/WAVE byte vector.
///
/// ## Examples
/// ```
/// use doom_audio::wav::encode_pcm16_wav_mono;
///
/// let samples = vec![0, 1000, -1000];
/// let wav = encode_pcm16_wav_mono(44_100, &samples);
///
/// assert_eq!(&wav[0..4], b"RIFF");
/// assert_eq!(wav.len(), 44 + 6); // 44 byte header + 3 i16 samples (6 bytes)
/// ```
#[must_use]
pub fn encode_pcm16_wav_mono(sample_rate: u32, samples: &[i16]) -> Vec<u8> {
    let channels: u16 = 1;
    let bits_per_sample: u16 = 16;
    let block_align: u16 = channels * (bits_per_sample / 8);
    let byte_rate: u32 = sample_rate * u32::from(block_align);
    let data_size: u32 = (samples.len() * 2) as u32;
    let riff_size: u32 = 36 + data_size;

    let mut out = Vec::with_capacity(44 + data_size as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_size.to_le_bytes());
    out.extend_from_slice(b"WAVE");

    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // PCM fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM format
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&bits_per_sample.to_le_bytes());

    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_size.to_le_bytes());
    for sample in samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mus::{MusEvent, MusHeader};

    #[test]
    fn encode_pcm16_wav_has_valid_header() {
        let wav = encode_pcm16_wav_mono(44_100, &[0, 1000, -1000]);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[12..16], b"fmt ");
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(wav.len(), 44 + 6);
    }

    #[test]
    fn render_mus_wav_produces_audio_data() {
        let score = MusScore {
            header: MusHeader {
                score_length: 0,
                score_start: 0,
                primary_channels: 1,
                secondary_channels: 0,
                instrument_count: 0,
            },
            instruments: Vec::new(),
            events: vec![
                (
                    0,
                    MusEvent::PlayNote {
                        channel: 0,
                        note: 60,
                        volume: Some(127),
                    },
                ),
                (14, MusEvent::ScoreEnd),
            ],
        };

        let wav = render_mus_to_wav_mono(score, None, 44_100, 1);
        assert!(wav.len() > 44, "WAV payload must include PCM data");
    }
}
