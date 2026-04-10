#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() == 8704 {
        let _ = doom_renderer::colormap::ColormapCache::from_test_data(data.to_vec());
    }
});
