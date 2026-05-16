#![no_main]

use libfuzzer_sys::fuzz_target;
use doom_audio::wav::encode_pcm16_wav_mono;

fuzz_target!(|data: &[u8]| {
    if data.len() < 4 {
        return;
    }

    let sample_rate = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);

    let mut samples = Vec::with_capacity((data.len() - 4) / 2);
    for chunk in data[4..].chunks_exact(2) {
        samples.push(i16::from_le_bytes([chunk[0], chunk[1]]));
    }

    let _ = encode_pcm16_wav_mono(sample_rate, &samples);
});
