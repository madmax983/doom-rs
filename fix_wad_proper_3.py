import re

with open('crates/doom-wad/src/stack.rs', 'r') as f:
    stack_content = f.read()

# Replace in stack.rs: map_lump_group_searches_pwads_first
stack_pattern = re.compile(r'        let \(wad, group\) = stack.find_map_lump_group\("MAP01"\).unwrap\(\);\s*// Ensure the WAD returned is the PWAD.\s*assert_eq!\(wad.kind\(\), WadKind::Pwad\);\s*match group \{\s*crate::wad::MapLumpGroup::Classic\(c\) => \{\s*// Check that we got the PWAD lumps, not the IWAD ones.\s*assert_eq!\(wad.lump_data\(c.lumps\[0\]\), b"pwad_things"\);\s*\}\s*_ => panic!\("Expected Classic map lump group"\),\s*\}')

stack_replacement = """        let (wad, group) = stack.find_map_lump_group("MAP01").unwrap();

        // Ensure the WAD returned is the PWAD.
        assert_eq!(wad.kind(), WadKind::Pwad);

        assert!(matches!(group, crate::wad::MapLumpGroup::Classic(_)));
        if let crate::wad::MapLumpGroup::Classic(c) = group {
            // Check that we got the PWAD lumps, not the IWAD ones.
            assert_eq!(wad.lump_data(c.lumps[0]), b"pwad_things");
        }"""

if stack_pattern.search(stack_content):
    stack_content = stack_pattern.sub(stack_replacement, stack_content)
    print("Replaced stack.rs: map_lump_group_searches_pwads_first")

# Replace in stack.rs: map_lump_group_searches_pwads_first_udmf
stack_pattern_udmf = re.compile(r'        let \(wad, group\) = stack.find_map_lump_group\("MAP01"\).unwrap\(\);\s*assert_eq!\(wad.kind\(\), WadKind::Pwad\);\s*match group \{\s*crate::wad::MapLumpGroup::Udmf\(u\) => \{\s*assert_eq!\(u.marker.name.as_str\(\), "MAP01"\);\s*assert_eq!\(u.textmap.name.as_str\(\), "TEXTMAP"\);\s*\}\s*_ => panic!\("Expected UDMF map lump group"\),\s*\}')

stack_replacement_udmf = """        let (wad, group) = stack.find_map_lump_group("MAP01").unwrap();

        assert_eq!(wad.kind(), WadKind::Pwad);

        assert!(matches!(group, crate::wad::MapLumpGroup::Udmf(_)));
        if let crate::wad::MapLumpGroup::Udmf(u) = group {
            assert_eq!(u.marker.name.as_str(), "MAP01");
            assert_eq!(u.textmap.name.as_str(), "TEXTMAP");
        }"""

if stack_pattern_udmf.search(stack_content):
    stack_content = stack_pattern_udmf.sub(stack_replacement_udmf, stack_content)
    print("Replaced stack.rs: map_lump_group_searches_pwads_first_udmf")


with open('crates/doom-wad/src/stack.rs', 'w') as f:
    f.write(stack_content)


with open('crates/doom-wad/src/wad.rs', 'r') as f:
    wad_content = f.read()

# Replace in wad.rs: map_lump_group_detects_udmf_group
wad_pattern1 = re.compile(r'        let group = wad.map_lump_group\("MAP01"\).expect\("map group"\);\s*match group \{\s*MapLumpGroup::Udmf\(group\) => \{\s*assert_eq!\(group.marker.name.as_str\(\), "MAP01"\);\s*assert_eq!\(group.textmap.name.as_str\(\), "TEXTMAP"\);\s*assert_eq!\(group.endmap.name.as_str\(\), "ENDMAP"\);\s*assert_eq!\(\s*group.find_lump\("ZNODES"\).map\(\|lump\| lump.name.as_str\(\)\),\s*Some\("ZNODES"\)\s*\);\s*\}\s*MapLumpGroup::Classic\(_\) => panic!\("expected UDMF map group"\),\s*\}')

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
        }"""

if wad_pattern1.search(wad_content):
    wad_content = wad_pattern1.sub(wad_replacement1, wad_content)
    print("Replaced wad.rs: map_lump_group_detects_udmf_group")

# Replace in wad.rs: udmf_map_lump_group_methods
wad_pattern2 = re.compile(r'        match wad.map_lump_group\("MAP01"\).unwrap\(\) \{\s*MapLumpGroup::Udmf\(udmf\) => \{\s*assert_eq!\(udmf.aux_lumps\(\).len\(\), 1\);\s*assert_eq!\(udmf.aux_lumps\(\)\[0\].name.as_str\(\), "ZNODES"\);\s*assert_eq!\(udmf.find_lump\("ZNODES"\).unwrap\(\).name.as_str\(\), "ZNODES"\);\s*assert!\(udmf.find_lump\("NONEXISTENT"\).is_none\(\)\);\s*\}\s*_ => panic!\("Expected UDMF"\),\s*\}')

wad_replacement2 = """        let group = wad.map_lump_group("MAP01").unwrap();
        assert!(matches!(group, MapLumpGroup::Udmf(_)));
        if let MapLumpGroup::Udmf(udmf) = group {
            assert_eq!(udmf.aux_lumps().len(), 1);
            assert_eq!(udmf.aux_lumps()[0].name.as_str(), "ZNODES");
            assert_eq!(udmf.find_lump("ZNODES").unwrap().name.as_str(), "ZNODES");
            assert!(udmf.find_lump("NONEXISTENT").is_none());
        }"""

if wad_pattern2.search(wad_content):
    wad_content = wad_pattern2.sub(wad_replacement2, wad_content)
    print("Replaced wad.rs: udmf_map_lump_group_methods")


with open('crates/doom-wad/src/wad.rs', 'w') as f:
    f.write(wad_content)
