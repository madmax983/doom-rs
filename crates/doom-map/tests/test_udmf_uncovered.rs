use doom_map::udmf::{UdmfError, UdmfMap};

#[test]
fn parse_unexpected_character() {
    let err = UdmfMap::parse(b"{").unwrap_err();
    match err {
        UdmfError::ParseFailed { message, .. } => assert!(message.contains("expected identifier")),
        _ => panic!("Expected ParseFailed"),
    }
}

#[test]
fn parse_block_comment() {
    let input = b"/* this is a block comment */ namespace = \"doom\";";
    let udmf = UdmfMap::parse(input).unwrap();
    assert_eq!(udmf.namespace, "doom");
}

#[test]
fn parse_line_comment() {
    let input = b"// this is a line comment\nnamespace = \"doom\";";
    let udmf = UdmfMap::parse(input).unwrap();
    assert_eq!(udmf.namespace, "doom");
}

#[test]
fn parse_unterminated_block_comment() {
    let err = UdmfMap::parse(b"/* this is an unterminated block comment namespace = \"doom\";")
        .unwrap_err();
    match err {
        UdmfError::ParseFailed { message, .. } => {
            assert!(message.contains("unterminated block comment"))
        }
        _ => panic!("Expected ParseFailed"),
    }
}

#[test]
fn test_parse_linedef_flags() {
    let input = b"namespace=\"doom\";
    linedef {
        v1 = 0;
        v2 = 1;
        sidefront = 0;
        sideback = 1;
        blocking = true;
        blockmonsters = true;
        dontpegtop = true;
        dontpegbottom = true;
        secret = true;
        blocksound = true;
        dontdraw = true;
        mapped = true;
        twosided = true;
    }
    vertex { x=0; y=0; }
    vertex { x=0; y=0; }
    sidedef { sector=0; }
    sidedef { sector=0; }
    sector { heightfloor=0; heightceiling=0; texturefloor=\"\"; textureceiling=\"\"; }
    ";
    let udmf = UdmfMap::parse(input).unwrap();
    let level = udmf.into_level_data().unwrap();
    let ld = &level.linedefs[0];

    use doom_map::lumps::*;

    const FLAG_SECRET: u16 = 0x0020;
    const FLAG_SOUNDBLOCK: u16 = 0x0040;
    const FLAG_DONTDRAW: u16 = 0x0080;
    const FLAG_MAPPED: u16 = 0x0100;

    assert_eq!(
        ld.flags,
        FLAG_BLOCKING
            | FLAG_BLOCKMONSTERS
            | FLAG_DONTPEGTOP
            | FLAG_DONTPEGBOTTOM
            | FLAG_SECRET
            | FLAG_SOUNDBLOCK
            | FLAG_DONTDRAW
            | FLAG_MAPPED
            | FLAG_TWO_SIDED
    );
}

#[test]
fn test_parse_number_exponent() {
    let input = b"namespace=\"doom\";
    vertex { x=1.0e2; y=-1.0E-2; }
    ";
    let _udmf = UdmfMap::parse(input).unwrap();
}

#[test]
fn test_parse_malformed_exponent() {
    let input = b"namespace=\"doom\"; vertex { x=1.0e+; }";
    let err = UdmfMap::parse(input).unwrap_err();
    match err {
        UdmfError::ParseFailed { message, .. } => {
            assert!(message.contains("malformed numeric exponent"))
        }
        _ => panic!("Expected ParseFailed for exponent"),
    }
}

#[test]
fn test_parse_unexpected_number() {
    let input = b"namespace=\"doom\"; vertex { x=+. ; }";
    let err = UdmfMap::parse(input).unwrap_err();
    match err {
        UdmfError::ParseFailed { message, .. } => {
            assert!(message.contains("expected numeric literal"))
        }
        _ => panic!("Expected ParseFailed"),
    }
}

#[test]
fn test_parse_unexpected_after_ident() {
    let input = b"namespace : \"doom\";";
    let err = UdmfMap::parse(input).unwrap_err();
    match err {
        UdmfError::ParseFailed { message, .. } => assert!(message.contains("expected '=' or '{'")),
        _ => panic!("Expected ParseFailed"),
    }
}

#[test]
fn test_parse_non_string_namespace() {
    let input = b"namespace = 1;";
    let err = UdmfMap::parse(input).unwrap_err();
    match err {
        UdmfError::ParseFailed { message, .. } => {
            assert!(message.contains("namespace must be a quoted string"))
        }
        _ => panic!("Expected ParseFailed"),
    }
}

#[test]
fn test_parse_thing_flags() {
    let input = b"namespace=\"doom\";
    thing {
        x=0; y=0; type=1;
        skill1 = true;
        skill2 = true;
        skill3 = true;
        skill4 = true;
        skill5 = true;
        ambush = true;
        single = false;
        dm = true;
        coop = true;
        friend = true;
    }
    ";
    let udmf = UdmfMap::parse(input).unwrap();
    let level = udmf.into_level_data().unwrap();
    let t = &level.things[0];

    // We expect the correct combined flags.
    const THING_FLAG_EASY: u16 = 0x0001;
    const THING_FLAG_MEDIUM: u16 = 0x0002;
    const THING_FLAG_HARD: u16 = 0x0004;
    const THING_FLAG_AMBUSH: u16 = 0x0008;
    const THING_FLAG_MULTIPLAYER: u16 = 0x0010;

    assert_eq!(
        t.flags,
        THING_FLAG_EASY
            | THING_FLAG_MEDIUM
            | THING_FLAG_HARD
            | THING_FLAG_AMBUSH
            | THING_FLAG_MULTIPLAYER
    );
}
