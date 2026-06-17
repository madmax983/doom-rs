use doom_audio::{mus::MusHeader, mus::MusScore, wav::render_mus_to_wav_mono};

#[test]
fn test_havoc_wav_infinite_loop() {
    let score = MusScore {
        header: MusHeader {
            score_length: 0,
            score_start: 0,
            primary_channels: 1,
            secondary_channels: 0,
            instrument_count: 0,
        },
        instruments: vec![],
        events: vec![], // Empty events vector!
    };

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = render_mus_to_wav_mono(score, None, 44100, 1);
        tx.send(()).unwrap();
    });

    rx.recv_timeout(std::time::Duration::from_secs(2))
        .expect("render_mus_to_wav_mono timed out!");
}
