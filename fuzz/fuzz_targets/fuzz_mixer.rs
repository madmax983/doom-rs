#![no_main]
use libfuzzer_sys::fuzz_target;
use std::sync::Arc;

fuzz_target!(|data: &[u8]| {
    if data.len() < 10 {
        return;
    }

    // We create a PcmSample from random data to test the mixer
    let pcm = doom_audio::mixer::PcmSample {
        sample_rate: 11025,
        data: data.to_vec().into(),
    };

    let mut mixer = doom_audio::mixer::Mixer::new(44100);
    mixer.play(0, Arc::new(pcm), 128);

    let mut out = vec![0; 1024];
    mixer.mix_frame(&mut out);
});
