#![no_main]
use doom_audio::driver::AudioDriver;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // maybe MIDI?
    let driver = AudioDriver::null();
    if let Ok(mut midi) = driver.midi.lock() {
        if let Ok(score) = doom_audio::mus::MusScore::parse(data) {
            midi.load_score(score);
            let mut buf = vec![0.0; 1024];
            midi.advance_samples(1024, 44100, &mut buf);
        }
    }
});
