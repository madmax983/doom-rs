//! MUS file format decoder.
//!
//! Parses Doom's proprietary `.mus` music format into a sequence of
//! [`MusEvent`] values with associated delta-tick timestamps.

use crate::AudioError;

// ---------------------------------------------------------------------------
// Magic bytes
// ---------------------------------------------------------------------------

const MUS_MAGIC: &[u8; 4] = b"MUS\x1a";

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Parsed MUS file header.
#[derive(Debug, Clone)]
pub struct MusHeader {
    /// Length of the score data in bytes.
    pub score_length: u16,
    /// Byte offset from the start of the file where the event stream begins.
    pub score_start: u16,
    /// Number of primary (melody) channels used.
    pub primary_channels: u16,
    /// Number of secondary (percussion) channels used.
    pub secondary_channels: u16,
    /// Number of instruments referenced in the file.
    pub instrument_count: u16,
}

/// A single decoded MUS event.
#[derive(Debug, Clone, PartialEq)]
pub enum MusEvent {
    /// Note-off for `note` on `channel`.
    ReleaseNote { channel: u8, note: u8 },
    /// Note-on for `note` on `channel`, with optional velocity override.
    PlayNote {
        channel: u8,
        note: u8,
        volume: Option<u8>,
    },
    /// Pitch-wheel change on `channel`.
    PitchWheel { channel: u8, value: u8 },
    /// System-level event on `channel`.
    SystemEvent { channel: u8, controller: u8 },
    /// Controller change on `channel`.
    Controller {
        channel: u8,
        controller: u8,
        value: u8,
    },
    /// Measure (bar) boundary marker — no payload.
    MeasureEnd,
    /// End of score — parsing stops after this event.
    ScoreEnd,
}

/// A fully-parsed MUS score.
pub struct MusScore {
    /// File header.
    pub header: MusHeader,
    /// Instrument patch numbers referenced by this score.
    pub instruments: Vec<u16>,
    /// Event stream: `(delta_ticks, event)`.
    ///
    /// `delta_ticks` is the number of ticks that should elapse *before* the
    /// corresponding event is played.  The first event always has a delta of 0.
    pub events: Vec<(u32, MusEvent)>,
}

impl MusScore {
    /// Parse a raw byte slice that contains a complete MUS file.
    ///
    /// # Errors
    /// Returns [`AudioError::InvalidMus`] if the data is too short, the magic
    /// bytes are wrong, or the event stream is truncated.
    pub fn parse(data: &[u8]) -> Result<Self, AudioError> {
        // ---------------------------------------------------------------
        // Validate magic + minimum header size
        // ---------------------------------------------------------------
        if data.len() < 16 {
            return Err(AudioError::InvalidMus("data too short for MUS header"));
        }
        if &data[0..4] != MUS_MAGIC {
            return Err(AudioError::InvalidMus("invalid MUS magic bytes"));
        }

        // ---------------------------------------------------------------
        // Read header fields (all u16 LE)
        // ---------------------------------------------------------------
        let read_u16 =
            |offset: usize| -> u16 { u16::from_le_bytes([data[offset], data[offset + 1]]) };

        let score_length = read_u16(4);
        let score_start = read_u16(6);
        let primary_channels = read_u16(8);
        let secondary_channels = read_u16(10);
        let instrument_count = read_u16(12);
        // bytes 14-15: padding, ignored

        let header = MusHeader {
            score_length,
            score_start,
            primary_channels,
            secondary_channels,
            instrument_count,
        };

        // ---------------------------------------------------------------
        // Read instrument list (instrument_count × u16 LE starting at byte 16)
        // ---------------------------------------------------------------
        let instruments_end = 16 + 2 * instrument_count as usize;
        if data.len() < instruments_end {
            return Err(AudioError::InvalidMus("data too short for instrument list"));
        }
        let instruments: Vec<u16> = (0..instrument_count as usize)
            .map(|i| read_u16(16 + 2 * i))
            .collect();

        // ---------------------------------------------------------------
        // Parse event stream starting at score_start
        // ---------------------------------------------------------------
        let start = score_start as usize;
        if data.len() < start {
            return Err(AudioError::InvalidMus(
                "score_start points past end of data",
            ));
        }

        let mut cursor = start;
        let mut events: Vec<(u32, MusEvent)> = Vec::new();
        let mut pending_delta: u32 = 0;

        loop {
            let event_byte = *data
                .get(cursor)
                .ok_or(AudioError::InvalidMus("unexpected end of event stream"))?;
            cursor += 1;

            let last_in_group = (event_byte & 0x80) != 0;
            let event_type = (event_byte >> 4) & 0x07;
            let channel = event_byte & 0x0F;

            let event = match event_type {
                // 0: Release note — 1 extra byte: note (bits 0-6)
                0 => {
                    let note_byte = *data
                        .get(cursor)
                        .ok_or(AudioError::InvalidMus("truncated release-note event"))?;
                    cursor += 1;
                    MusEvent::ReleaseNote {
                        channel,
                        note: note_byte & 0x7F,
                    }
                }

                // 1: Play note — 1 byte (bit7=has_volume, bits0-6=note) + optional volume
                1 => {
                    let note_byte = *data
                        .get(cursor)
                        .ok_or(AudioError::InvalidMus("truncated play-note event"))?;
                    cursor += 1;
                    let has_volume = (note_byte & 0x80) != 0;
                    let note = note_byte & 0x7F;
                    let volume = if has_volume {
                        let v = *data
                            .get(cursor)
                            .ok_or(AudioError::InvalidMus("truncated play-note volume"))?;
                        cursor += 1;
                        Some(v)
                    } else {
                        None
                    };
                    MusEvent::PlayNote {
                        channel,
                        note,
                        volume,
                    }
                }

                // 2: Pitch wheel — 1 extra byte
                2 => {
                    let value = *data
                        .get(cursor)
                        .ok_or(AudioError::InvalidMus("truncated pitch-wheel event"))?;
                    cursor += 1;
                    MusEvent::PitchWheel { channel, value }
                }

                // 3: System event — 1 extra byte
                3 => {
                    let controller = *data
                        .get(cursor)
                        .ok_or(AudioError::InvalidMus("truncated system event"))?;
                    cursor += 1;
                    MusEvent::SystemEvent {
                        channel,
                        controller,
                    }
                }

                // 4: Controller — 2 extra bytes
                4 => {
                    let controller = *data.get(cursor).ok_or(AudioError::InvalidMus(
                        "truncated controller event (controller)",
                    ))?;
                    cursor += 1;
                    let value = *data
                        .get(cursor)
                        .ok_or(AudioError::InvalidMus("truncated controller event (value)"))?;
                    cursor += 1;
                    MusEvent::Controller {
                        channel,
                        controller,
                        value,
                    }
                }

                // 5: Measure end — no extra bytes
                5 => MusEvent::MeasureEnd,

                // 6: Score end — no extra bytes; stop after recording this event
                6 => {
                    events.push((pending_delta, MusEvent::ScoreEnd));
                    break;
                }

                // 7: Unused — treat as a no-op measure-end for robustness
                _ => MusEvent::MeasureEnd,
            };

            events.push((pending_delta, event));

            // After each event, read optional delta-tick if this was the last
            // event in a group (bit 7 of the event byte was set).
            if last_in_group {
                let mut delta: u32 = 0;
                let mut shift: u32 = 0;
                loop {
                    let b = *data
                        .get(cursor)
                        .ok_or(AudioError::InvalidMus("truncated delta-time"))?;
                    cursor += 1;
                    let shifted = u32::from(b & 0x7F)
                        .checked_shl(shift)
                        .ok_or(AudioError::InvalidMus("delta-time shift overflow"))?;
                    delta = delta
                        .checked_add(shifted)
                        .ok_or(AudioError::InvalidMus("delta-time value overflow"))?;
                    shift += 7;
                    if (b & 0x80) == 0 {
                        break;
                    }
                }
                pending_delta = delta;
            } else {
                pending_delta = 0;
            }
        }

        Ok(MusScore {
            header,
            instruments,
            events,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Build the 16-byte fixed MUS header with given score_start and
    /// instrument_count (no instrument list entries).
    pub(crate) fn make_header(score_start: u16, instrument_count: u16) -> Vec<u8> {
        let mut h = Vec::with_capacity(16);
        h.extend_from_slice(b"MUS\x1a");
        h.extend_from_slice(&0u16.to_le_bytes()); // score_length (don't care)
        h.extend_from_slice(&score_start.to_le_bytes());
        h.extend_from_slice(&1u16.to_le_bytes()); // primary_channels
        h.extend_from_slice(&0u16.to_le_bytes()); // secondary_channels
        h.extend_from_slice(&instrument_count.to_le_bytes());
        h.extend_from_slice(&0u16.to_le_bytes()); // padding
        h
    }

    #[test]
    fn mus_wrong_magic_errors() {
        let result = MusScore::parse(b"BAD!");
        assert!(result.is_err(), "wrong magic should be an error");
    }

    #[test]
    fn mus_parse_minimal_score() {
        // Header with score_start=16 (right after header) and no instruments.
        let mut data = make_header(16, 0);
        // Score-end event: type=6, channel=0, not-last-in-group → 0x60
        data.push(0x60);

        let score = MusScore::parse(&data).expect("minimal score should parse");
        assert_eq!(score.instruments.len(), 0);
        assert_eq!(score.events.len(), 1);
        assert_eq!(score.events[0].1, MusEvent::ScoreEnd);
    }

    #[test]
    fn mus_play_note_with_volume() {
        let mut data = make_header(16, 0);
        // Play-note event: type=1, channel=1, last_in_group=1 → 0x91
        data.push(0x91);
        // Note byte: has_volume=1 (bit7), note=0x45=69 → 0xC5
        data.push(0xC5);
        // Volume byte: 0x7F = 127
        data.push(0x7F);
        // Delta-time: single byte 0x00 (high bit clear → done, value 0)
        data.push(0x00);
        // Score-end to terminate
        data.push(0x60);

        let score = MusScore::parse(&data).expect("play-note score should parse");
        assert!(score.events.len() >= 2);
        let (delta, ref event) = score.events[0];
        // pending_delta before this event is 0 (it's the very first)
        assert_eq!(delta, 0);
        assert_eq!(
            *event,
            MusEvent::PlayNote {
                channel: 1,
                note: 69,
                volume: Some(127)
            }
        );
    }

    #[test]
    fn mus_release_note() {
        let mut data = make_header(16, 0);
        // Release-note event: type=0, channel=0, last_in_group=0 → 0x00
        data.push(0x00);
        // Note byte: 0x3C = 60
        data.push(0x3C);
        // Score-end (not-last-in-group) to terminate
        data.push(0x60);

        let score = MusScore::parse(&data).expect("release-note score should parse");
        assert!(score.events.len() >= 2);
        assert_eq!(
            score.events[0].1,
            MusEvent::ReleaseNote {
                channel: 0,
                note: 60
            }
        );
    }
}

#[cfg(test)]
mod tests_havoc {
    use super::*;

    #[test]
    fn mus_delta_time_overflow() {
        let mut data = super::tests::make_header(16, 0);
        data.push(0x80); // Event type 0, channel 0, last_in_group = 1
        data.push(0x00); // Note byte
        data.push(0xff); // Delta byte 1 (continue)
        data.push(0xff); // Delta byte 2 (continue)
        data.push(0xff); // Delta byte 3 (continue)
        data.push(0xff); // Delta byte 4 (continue)
        data.push(0xff); // Delta byte 5 (continue)
        data.push(0xff); // Delta byte 6 (continue)
        let result = MusScore::parse(&data);
        assert!(result.is_err());
    }
}
