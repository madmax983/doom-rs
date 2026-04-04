import re

with open('crates/doom-wad/src/wad.rs', 'r') as f:
    wad_content = f.read()

# Replace in wad.rs: map_lump_group_detects_udmf_group
wad_pattern1 = re.compile(r'        let group = wad.map_lump_group\("MAP01"\).expect\("map group"\);\s*let MapLumpGroup::Udmf\(group\) = group else \{\s*panic!\("expected UDMF map group"\);\s*\};\s*assert_eq!\(group.marker.name.as_str\(\), "MAP01"\);\s*assert_eq!\(group.textmap.name.as_str\(\), "TEXTMAP"\);\s*assert_eq!\(group.endmap.name.as_str\(\), "ENDMAP"\);\s*assert_eq!\(\s*group.find_lump\("ZNODES"\).map\(\|lump\| lump.name.as_str\(\)\),\s*Some\("ZNODES"\)\s*\);\s*\}')

wad_replacement1 = """        let group = wad.map_lump_group("MAP01").expect("map group");

        assert!(matches!(group, MapLumpGroup::Udmf(_)));
        if let MapLumpGroup::Udmf(group) = group {
            assert_eq!(group.marker.name.as_str(), "MAP01");
            assert_eq!(group.textmap.name.as_str(), "TEXTMAP");
            assert_eq!(group.endmap.name.as_str(), "ENDMAP");
            assert_eq!(
                group.find_lump("ZNODES").map(|lump| lump.name.as_str()),
                Some("ZNODES")
            );
        }
    }"""

if wad_pattern1.search(wad_content):
    wad_content = wad_pattern1.sub(wad_replacement1, wad_content)
    print("Replaced wad.rs: map_lump_group_detects_udmf_group")


# Replace in wad.rs: udmf_map_lump_group_methods
wad_pattern2 = re.compile(r'        let MapLumpGroup::Udmf\(udmf\) = wad.map_lump_group\("MAP01"\).unwrap\(\) else \{\s*panic!\("Expected UDMF"\);\s*\};\s*assert_eq!\(udmf.aux_lumps\(\).len\(\), 1\);\s*assert_eq!\(udmf.aux_lumps\(\)\[0\].name.as_str\(\), "ZNODES"\);\s*assert_eq!\(udmf.find_lump\("ZNODES"\).unwrap\(\).name.as_str\(\), "ZNODES"\);\s*assert!\(udmf.find_lump\("NONEXISTENT"\).is_none\(\)\);\s*\}')

wad_replacement2 = """        let group = wad.map_lump_group("MAP01").unwrap();
        assert!(matches!(group, MapLumpGroup::Udmf(_)));
        if let MapLumpGroup::Udmf(udmf) = group {
            assert_eq!(udmf.aux_lumps().len(), 1);
            assert_eq!(udmf.aux_lumps()[0].name.as_str(), "ZNODES");
            assert_eq!(udmf.find_lump("ZNODES").unwrap().name.as_str(), "ZNODES");
            assert!(udmf.find_lump("NONEXISTENT").is_none());
        }
    }"""

if wad_pattern2.search(wad_content):
    wad_content = wad_pattern2.sub(wad_replacement2, wad_content)
    print("Replaced wad.rs: udmf_map_lump_group_methods")

with open('crates/doom-wad/src/wad.rs', 'w') as f:
    f.write(wad_content)
