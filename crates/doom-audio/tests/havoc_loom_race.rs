#[cfg(feature = "loom")]
use loom::sync::{Arc, Mutex};
#[cfg(feature = "loom")]
use loom::thread;

#[cfg(feature = "loom")]
#[test]
fn havoc_loom_race_test() {
    loom::model(|| {
        let mixer = Arc::new(Mutex::new(doom_audio::SfxMixer::new()));
        let m1 = mixer.clone();
        let m2 = mixer.clone();

        let t1 = thread::spawn(move || {
            let mut m = m1.lock().unwrap();
            m.stop_all();
            m.play(
                1,
                std::sync::Arc::new([1, 2, 3]),
                1.0,
                0.0,
                doom_audio::SfxPriority::Medium,
            );
        });

        let t2 = thread::spawn(move || {
            let mut m = m2.lock().unwrap();
            let mut buf = [0.0; 2];
            m.mix(&mut buf, 44100);
        });

        t1.join().unwrap();
        t2.join().unwrap();
    });
}
