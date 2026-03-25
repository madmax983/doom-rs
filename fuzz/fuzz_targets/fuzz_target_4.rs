#![no_main]
use doom_audio::sfx_mixer::{SfxMixer, SfxPriority};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut mixer = SfxMixer::new();
    let arc_data: std::sync::Arc<[u8]> = data.to_vec().into();
    if let Some(ch) = mixer.play(1, arc_data.clone(), 1.0, 0.0, SfxPriority::Medium) {
        mixer.play_on_channel(ch, 2, arc_data.clone(), 1.0, 0.0, SfxPriority::Medium);
    }
    let mut output = vec![0.0f32; 1024];
    mixer.mix(&mut output, 44100);
});
