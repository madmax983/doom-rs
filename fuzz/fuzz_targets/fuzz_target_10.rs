#![no_main]
use libfuzzer_sys::fuzz_target;
use doom_audio::driver::AudioDriver;

fuzz_target!(|data: &[u8]| {
    if let Ok(mut driver) = AudioDriver::null().midi.lock() {
        if let Ok(score) = doom_audio::mus::MusScore::parse(data) {
            driver.load_score(score);
            let mut buf = vec![0.0; 1024];
            driver.advance_samples(1024, 44100, &mut buf);
        }
    }
});
