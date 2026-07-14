use doom_map::udmf::{UdmfError, UdmfMap, UdmfValue};

#[test]
fn test_udmf_thing_flags_parsing() {
    let map = UdmfMap::parse(
        br#"
        namespace = "doom";
        thing { x = 0; y = 0; type = 1; skill1 = true; skill2 = true; ambush = true; single = false; }
        thing { x = 0; y = 0; type = 1; skill3 = true; }
        thing { x = 0; y = 0; type = 1; skill4 = true; skill5 = true; }
        thing { x = 0; y = 0; type = 1; flags = 42; }
        "#,
    )
    .unwrap();

    let level_data = map.into_level_data().unwrap();
    assert_eq!(level_data.things.len(), 4);
    assert_eq!(level_data.things[0].flags, 0x0001 | 0x0008 | 0x0010); // Easy | Ambush | Multiplayer
    assert_eq!(level_data.things[1].flags, 0x0002); // Medium
    assert_eq!(level_data.things[2].flags, 0x0004); // Hard
    assert_eq!(level_data.things[3].flags, 42);
}

#[test]
fn test_udmf_parse_exponent() {
    let map = UdmfMap::parse(
        br#"
        namespace = "doom";
        vertex { x = 1e2; y = -1.5E-2; }
        "#,
    )
    .unwrap();

    let block = &map.blocks[0];
    assert_eq!(block.fields[0].value, UdmfValue::Float(100.0));
    assert_eq!(block.fields[1].value, UdmfValue::Float(-0.015));
}

#[test]
fn test_udmf_parse_exponent_invalid() {
    let err = UdmfMap::parse(
        br#"
        namespace = "doom";
        vertex { x = 1e; }
        "#,
    )
    .unwrap_err();
    assert!(
        matches!(err, UdmfError::ParseFailed { message, .. } if message == "malformed numeric exponent")
    );
}

#[test]
fn test_udmf_parse_integral_value_out_of_range() {
    let map = UdmfMap::parse(
        br#"
        namespace = "doom";
        vertex { x = 999999999999999999999999999999999999.0; y = 0; }
        "#,
    )
    .unwrap();

    let err = map.into_level_data().unwrap_err();
    assert!(matches!(err, UdmfError::OutOfRange { field, .. } if field == "x"));
}

#[test]
fn test_udmf_parse_wrong_type() {
    let map = UdmfMap::parse(
        br#"
        namespace = "doom";
        vertex { x = "not a number"; y = 0; }
        "#,
    )
    .unwrap();

    let err = map.into_level_data().unwrap_err();
    assert!(matches!(err, UdmfError::WrongType { field, .. } if field == "x"));
}

#[test]
fn test_udmf_parse_comments() {
    let map = UdmfMap::parse(
        br#"
        // Line comment
        namespace = "doom"; // Another line comment
        /* Block comment */
        vertex { /* Inline */ x = 0; y = 0; }
        /* Multiline
           Block
           Comment */
        "#,
    )
    .unwrap();

    assert_eq!(map.namespace, "doom");
    assert_eq!(map.blocks.len(), 1);
}

#[test]
fn test_udmf_parse_unterminated_block_comment() {
    let err = UdmfMap::parse(
        br#"
        namespace = "doom";
        /* Unterminated
        "#,
    )
    .unwrap_err();

    assert!(
        matches!(err, UdmfError::ParseFailed { message, .. } if message == "unterminated block comment")
    );
}

#[test]
fn test_udmf_parse_missing_identifier() {
    let err = UdmfMap::parse(
        br#"
        namespace = "doom";
        vertex { x = 0; y = 0; }
        123
        "#,
    )
    .unwrap_err();
    assert!(
        matches!(err, UdmfError::ParseFailed { message, .. } if message == "expected identifier")
    );

    let _err2 = UdmfMap::parse(
        br#"
        namespace = "doom";
        vertex { x = 0; y = 0; }
        "#,
    )
    .unwrap();

    let err3 = UdmfMap::parse(
        br#"
        namespace = "doom";
        vertex { x = 0; y = 0; }
        {"#,
    )
    .unwrap_err();
    assert!(
        matches!(err3, UdmfError::ParseFailed { message, .. } if message == "expected identifier")
    );
}

#[test]
fn test_udmf_linedef_flags() {
    let map = UdmfMap::parse(
        br#"
        namespace = "doom";
        linedef {
            v1 = 0; v2 = 1; sidefront = 0;
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
        "#,
    )
    .unwrap();

    let level_data = map.into_level_data().unwrap();
    assert_eq!(level_data.linedefs.len(), 1);
    let flags = level_data.linedefs[0].flags;
    assert_eq!(
        flags,
        0x0001 | 0x0002 | 0x0008 | 0x0010 | 0x0020 | 0x0040 | 0x0080 | 0x0100 | 0x0004
    );
}

#[test]
fn test_udmf_invalid_name() {
    let map = UdmfMap::parse(
        "namespace = \"doom\";\nsector { heightfloor = 0; heightceiling = 0; texturefloor = \"TéXTURE\"; textureceiling = \"TEX\"; }".as_bytes()
    ).unwrap();

    let err = map.into_level_data().unwrap_err();
    assert!(matches!(err, UdmfError::NameNotAscii { .. }));
}

#[test]
fn test_udmf_long_name() {
    let map = UdmfMap::parse(
        br#"
        namespace = "doom";
        sector { heightfloor = 0; heightceiling = 0; texturefloor = "LONG_TEXTURE_NAME_123456"; textureceiling = "TEX"; }
        "#,
    ).unwrap();

    let err = map.into_level_data().unwrap_err();
    assert!(matches!(err, UdmfError::NameTooLong { .. }));
}

#[test]
fn test_udmf_parse_string_escapes() {
    let map = UdmfMap::parse(
        br#"
        namespace = "doom";
        vertex { x = 0; y = 0; test = "\"\\\n\r\t"; }
        "#,
    )
    .unwrap();

    assert_eq!(
        map.blocks[0].fields[2].value,
        UdmfValue::Str("\"\\\n\r\t".to_string())
    );
}

#[test]
fn test_udmf_first_present_u16() {
    let map = UdmfMap::parse(
        br#"
        namespace = "doom";
        sector { heightfloor = 0; heightceiling = 0; texturefloor = "TEX"; textureceiling = "TEX"; id = 42; }
        "#,
    ).unwrap();
    let level_data = map.into_level_data().unwrap();
    assert_eq!(level_data.sectors[0].tag, 42);
}

#[test]
fn test_udmf_sidedef_index_out_of_bounds() {
    let map = UdmfMap::parse(
        br#"
        namespace = "doom";
        linedef {
            v1 = 0; v2 = 1; sidefront = 65536;
        }
        "#,
    )
    .unwrap();

    let err = map.into_level_data().unwrap_err();
    assert!(matches!(err, UdmfError::OutOfRange { field, .. } if field == "sidefront"));
}

#[test]
fn test_udmf_missing_first_present_u16_required() {
    let map = UdmfMap::parse(
        br#"
        namespace = "doom";
        thing { x = 0; y = 0; skill1 = true; }
        "#,
    )
    .unwrap();
    let err = map.into_level_data().unwrap_err();
    assert!(matches!(err, UdmfError::MissingField { field, .. } if field == "type"));
}

#[test]
fn test_udmf_sidedef_index_out_of_bounds_neg() {
    let map = UdmfMap::parse(
        br#"
        namespace = "doom";
        linedef {
            v1 = 0; v2 = 1; sidefront = 0; sideback = -1;
        }
        "#,
    )
    .unwrap();

    let level_data = map.into_level_data().unwrap();
    assert_eq!(
        level_data.linedefs[0].left_sidedef,
        doom_map::lumps::SIDEDEF_NONE
    );
}
