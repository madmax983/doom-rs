#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(mut mus) = doom_audio::mus::MusScore::parse(data) {
        let mut sink = vec![0.0; 100];
        let mut ctx = doom_audio::mixer::MixerContext {
            sample_rate: 44100,
        };
        mus.mix(&mut ctx, &mut sink);
    }
});
