#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(mut score) = doom_audio::mus::MusScore::parse(data) {
        let mut midi = doom_audio::midi::MidiPlayer::new();
        // Since play takes a bank and a score let's make a mock bank
        if let Ok(bank) = doom_audio::midi::GenmidiBank::parse(&[0; 10000]) {
            midi.play(&bank, score);
            let mut out = [0.0; 100];
            midi.mix_into(&mut out, 44100.0);
        }
    }
});
