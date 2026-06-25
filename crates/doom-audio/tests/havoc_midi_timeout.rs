#[test]
fn havoc_midi_missing_end() {
    let mut data = Vec::new();
    data.extend_from_slice(b"MUS\x1A"); // magic
    data.extend_from_slice(&[0, 0]); // score_length
    data.extend_from_slice(&[16, 0]); // score_start
    data.extend_from_slice(&[0, 0]); // primary_channels
    data.extend_from_slice(&[0, 0]); // secondary_channels
    data.extend_from_slice(&[0, 0]); // instrument_count
    data.extend_from_slice(&[0, 0]); // padding

    // Send a delta of 0 for every event. So they all execute on the same tick!
    for _ in 0..1000000 {
        data.push(0x10); // PlayNote, channel 0, not last in group
        data.push(0x00); // note=0, bit7=0 -> no volume
    }

    // Loom checks panic if we don't mock it when the feature is enabled.
    #[cfg(not(feature = "loom"))]
    {
        use doom_audio::driver::AudioDriver;
        let driver = AudioDriver::null();
        if let Ok(mut midi) = driver.midi.lock() {
            if let Ok(score) = doom_audio::mus::MusScore::parse(&data) {
                midi.load_score(score);
                let mut buf = vec![0.0; 1024];
                midi.advance_samples(1024, 44100, &mut buf);
            }
        }
    }
}
