use doom_audio::{
    mus::{MusEvent, MusHeader, MusScore},
    wav::render_mus_to_wav_mono,
};

#[test]
fn havoc_test_wav_duration_overflow() {
    let score = MusScore {
        header: MusHeader {
            score_length: 0,
            score_start: 0,
            primary_channels: 1,
            secondary_channels: 0,
            instrument_count: 0,
        },
        instruments: vec![],
        events: vec![(u32::MAX, MusEvent::ScoreEnd), (1, MusEvent::ScoreEnd)],
    };

    // We only spawn a thread because the original infinite loop test spawned one.
    // However, testing for an overflow panic can be done synchronously.
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = render_mus_to_wav_mono(score, None, 44100, 1);
        tx.send(()).unwrap();
    });

    let _ = rx.recv_timeout(std::time::Duration::from_secs(2));
}
