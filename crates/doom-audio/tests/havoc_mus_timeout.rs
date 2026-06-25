use doom_audio::mus::MusScore;

#[test]
fn havoc_mus_missing_end() {
    let mut data = Vec::new();
    data.extend_from_slice(b"MUS\x1A"); // magic
    data.extend_from_slice(&[0, 0]); // score_length
    data.extend_from_slice(&[16, 0]); // score_start
    data.extend_from_slice(&[0, 0]); // primary_channels
    data.extend_from_slice(&[0, 0]); // secondary_channels
    data.extend_from_slice(&[0, 0]); // instrument_count
    data.extend_from_slice(&[0, 0]); // padding

    // PlayNote (1) -> requires 1 byte, optional volume. Bit 7 not set -> no volume.
    // We send infinite 0x10.
    for _ in 0..1000000 {
        data.push(0x10); // PlayNote, channel 0
        data.push(0x00); // note=0, bit7=0 -> no volume
    }

    let _ = MusScore::parse(&data);
}
