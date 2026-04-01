use doom_audio::mus::MusScore;

#[test]
fn test_mus_delta_overflow_crash() {
    let data = vec![
        b'M', b'U', b'S', 0x1a, // Magic
        0, 0, // score_length
        16, 0, // score_start
        1, 0, // primary_channels
        0, 0, // secondary_channels
        0, 0, // instrument_count
        0, 0, // padding
        0x80, // event 0: release note, last_in_group=1
        0x00, // note
        0x80, 0x80, 0x80, 0x80, 0x10, // delta with shift=28 -> 35
    ];
    let res = MusScore::parse(&data);
}
