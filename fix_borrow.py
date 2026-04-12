import os

filepath = "crates/doom-game/src/specials.rs"
with open(filepath, "r") as f:
    content = f.read()

search1 = """.map(|(i, _)| (i, lowest_adjacent_ceiling(level, i) - 8))
                .collect::<Vec<_>>()
            {
                activate_floor_raise_single_typed("""

replace1 = """.map(|(i, _)| (i, lowest_adjacent_ceiling(level, i) - 8))
            {
                activate_floor_raise_single_typed("""

# Reverting `.collect` additions
content = content.replace(search1, replace1)

search2 = """.map(|(i, _)| (i, highest_adjacent_floor(level, i) + 8))
                .collect::<Vec<_>>()
            {
                activate_floor_lower_single_typed("""
replace2 = """.map(|(i, _)| (i, highest_adjacent_floor(level, i) + 8))
            {
                activate_floor_lower_single_typed("""
content = content.replace(search2, replace2)

search3 = """        // Type 56: W1 Floor raise to 8 below lowest adjacent ceiling (crush).
        56 => {
            let tag = level.linedefs[linedef_idx].tag;
            for (idx, target) in level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| (i, lowest_adjacent_ceiling(level, i) - 8))
            {
                activate_floor_raise_single_typed(
                    gs,
                    level,
                    idx,
                    tag,
                    target,
                    1,
                    true,
                    FloorType::RaiseCrush,
                );
            }
        }"""
replace3 = """        // Type 56: W1 Floor raise to 8 below lowest adjacent ceiling (crush).
        56 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in 0..level.sectors.len() {
                if level.sectors[idx].tag == tag {
                    let target = lowest_adjacent_ceiling(level, idx) - 8;
                    activate_floor_raise_single_typed(
                        gs,
                        level,
                        idx,
                        tag,
                        target,
                        1,
                        true,
                        FloorType::RaiseCrush,
                    );
                }
            }
        }"""
content = content.replace(search3, replace3)


search4 = """        // Type 65: SR Raise floor to 8 below lowest ceiling + crush.
        65 => {
            let tag = level.linedefs[linedef_idx].tag;
            for (idx, target) in level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| (i, lowest_adjacent_ceiling(level, i) - 8))
            {
                activate_floor_raise_single_typed(
                    gs,
                    level,
                    idx,
                    tag,
                    target,
                    1,
                    true,
                    FloorType::RaiseCrush,
                );
            }
        }"""
replace4 = """        // Type 65: SR Raise floor to 8 below lowest ceiling + crush.
        65 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in 0..level.sectors.len() {
                if level.sectors[idx].tag == tag {
                    let target = lowest_adjacent_ceiling(level, idx) - 8;
                    activate_floor_raise_single_typed(
                        gs,
                        level,
                        idx,
                        tag,
                        target,
                        1,
                        true,
                        FloorType::RaiseCrush,
                    );
                }
            }
        }"""
content = content.replace(search4, replace4)

search5 = """        // Type 94: WR Raise floor to 8 below lowest ceiling + crush.
        94 => {
            let tag = level.linedefs[linedef_idx].tag;
            for (idx, target) in level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| (i, lowest_adjacent_ceiling(level, i) - 8))
            {
                activate_floor_raise_single_typed(
                    gs,
                    level,
                    idx,
                    tag,
                    target,
                    1,
                    true,
                    FloorType::RaiseCrush,
                );
            }
        }"""
replace5 = """        // Type 94: WR Raise floor to 8 below lowest ceiling + crush.
        94 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in 0..level.sectors.len() {
                if level.sectors[idx].tag == tag {
                    let target = lowest_adjacent_ceiling(level, idx) - 8;
                    activate_floor_raise_single_typed(
                        gs,
                        level,
                        idx,
                        tag,
                        target,
                        1,
                        true,
                        FloorType::RaiseCrush,
                    );
                }
            }
        }"""
content = content.replace(search5, replace5)


search6 = """        // Type 36: W1 Lower floor to highest adjacent - 8 (turbo).
        36 => {
            let tag = level.linedefs[linedef_idx].tag;
            for (idx, target) in level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| (i, highest_adjacent_floor(level, i) + 8))
            {
                activate_floor_lower_single_typed(
                    gs,
                    level,
                    idx,
                    tag,
                    target,
                    4,
                    FloorType::LowerToHighest,
                );
            }
        }"""
replace6 = """        // Type 36: W1 Lower floor to highest adjacent - 8 (turbo).
        36 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in 0..level.sectors.len() {
                if level.sectors[idx].tag == tag {
                    let target = highest_adjacent_floor(level, idx) + 8;
                    activate_floor_lower_single_typed(
                        gs,
                        level,
                        idx,
                        tag,
                        target,
                        4,
                        FloorType::LowerToHighest,
                    );
                }
            }
        }"""
content = content.replace(search6, replace6)


search7 = """        // Type 69: SR Lower floor to highest adjacent - 8.
        69 => {
            let tag = level.linedefs[linedef_idx].tag;
            for (idx, target) in level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| (i, highest_adjacent_floor(level, i) + 8))
            {
                activate_floor_lower_single_typed(
                    gs,
                    level,
                    idx,
                    tag,
                    target,
                    1,
                    FloorType::LowerToHighest,
                );
            }
        }"""
replace7 = """        // Type 69: SR Lower floor to highest adjacent - 8.
        69 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in 0..level.sectors.len() {
                if level.sectors[idx].tag == tag {
                    let target = highest_adjacent_floor(level, idx) + 8;
                    activate_floor_lower_single_typed(
                        gs,
                        level,
                        idx,
                        tag,
                        target,
                        1,
                        FloorType::LowerToHighest,
                    );
                }
            }
        }"""
content = content.replace(search7, replace7)


search8 = """        // Type 70: SR Lower floor to highest adjacent - 8 (turbo).
        70 => {
            let tag = level.linedefs[linedef_idx].tag;
            for (idx, target) in level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| (i, highest_adjacent_floor(level, i) + 8))
            {
                activate_floor_lower_single_typed(
                    gs,
                    level,
                    idx,
                    tag,
                    target,
                    4,
                    FloorType::LowerToHighest,
                );
            }
        }"""
replace8 = """        // Type 70: SR Lower floor to highest adjacent - 8 (turbo).
        70 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in 0..level.sectors.len() {
                if level.sectors[idx].tag == tag {
                    let target = highest_adjacent_floor(level, idx) + 8;
                    activate_floor_lower_single_typed(
                        gs,
                        level,
                        idx,
                        tag,
                        target,
                        4,
                        FloorType::LowerToHighest,
                    );
                }
            }
        }"""
content = content.replace(search8, replace8)


search9 = """        // Type 71: S1 Lower floor to highest adjacent - 8 (turbo).
        71 => {
            let tag = level.linedefs[linedef_idx].tag;
            for (idx, target) in level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| (i, highest_adjacent_floor(level, i) + 8))
            {
                activate_floor_lower_single_typed(
                    gs,
                    level,
                    idx,
                    tag,
                    target,
                    4,
                    FloorType::LowerToHighest,
                );
            }
        }"""
replace9 = """        // Type 71: S1 Lower floor to highest adjacent - 8 (turbo).
        71 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in 0..level.sectors.len() {
                if level.sectors[idx].tag == tag {
                    let target = highest_adjacent_floor(level, idx) + 8;
                    activate_floor_lower_single_typed(
                        gs,
                        level,
                        idx,
                        tag,
                        target,
                        4,
                        FloorType::LowerToHighest,
                    );
                }
            }
        }"""
content = content.replace(search9, replace9)

search10 = """        // Type 98: WR Lower floor to highest adjacent - 8 (turbo).
        98 => {
            let tag = level.linedefs[linedef_idx].tag;
            for (idx, target) in level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| (i, highest_adjacent_floor(level, i) + 8))
            {
                activate_floor_lower_single_typed(
                    gs,
                    level,
                    idx,
                    tag,
                    target,
                    4,
                    FloorType::LowerToHighest,
                );
            }
        }"""
replace10 = """        // Type 98: WR Lower floor to highest adjacent - 8 (turbo).
        98 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in 0..level.sectors.len() {
                if level.sectors[idx].tag == tag {
                    let target = highest_adjacent_floor(level, idx) + 8;
                    activate_floor_lower_single_typed(
                        gs,
                        level,
                        idx,
                        tag,
                        target,
                        4,
                        FloorType::LowerToHighest,
                    );
                }
            }
        }"""
content = content.replace(search10, replace10)

with open(filepath, "w") as f:
    f.write(content)

print("done")
