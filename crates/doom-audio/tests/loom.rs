#[cfg(feature = "loom")]
#[test]
fn havoc_audio_driver_concurrency() {
    loom::model(|| {
        let mixer = doom_audio::driver::AudioDriver::null().mixer;
        let mixer_clone = mixer.clone();

        loom::thread::spawn(move || {
            let mut guard = mixer_clone.lock().unwrap();
            let data: std::sync::Arc<[u8]> = vec![128u8; 10].into();
            guard.play(0, data, 1.0, 0.0, doom_audio::sfx_mixer::SfxPriority::Medium);
        });

        let mut guard = mixer.lock().unwrap();
        let data: std::sync::Arc<[u8]> = vec![128u8; 10].into();
        guard.play(1, data, 1.0, 0.0, doom_audio::sfx_mixer::SfxPriority::Medium);
    });

    loom::model(|| {
        let midi = doom_audio::driver::AudioDriver::null().midi;
        let midi_clone = midi.clone();

        loom::thread::spawn(move || {
            let mut guard = midi_clone.lock().unwrap();
            let mut out = vec![0.0f32; 100];
            guard.advance_samples(100, 44100, &mut out);
        });

        let mut guard = midi.lock().unwrap();
        let mut out = vec![0.0f32; 100];
        guard.advance_samples(100, 44100, &mut out);
    });
}
