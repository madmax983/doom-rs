#[cfg(feature = "loom")]
#[test]
fn havoc_loom_deadlock_test_2() {
    use doom_audio::driver::AudioDriver;
    loom::model(|| {
        let driver = AudioDriver::null();
        let mixer1 = driver.mixer.clone();
        let mixer2 = driver.mixer.clone();

        let t1 = loom::thread::spawn(move || {
            let mut m = mixer1.lock().expect("value must exist in test");
            m.stop_all();
        });

        let t2 = loom::thread::spawn(move || {
            let mut m = mixer2.lock().expect("value must exist in test");
            let mut buf = [0.0; 2];
            m.mix(&mut buf, 44100);
        });

        t1.join().expect("value must exist in test");
        t2.join().expect("value must exist in test");
    });
}
