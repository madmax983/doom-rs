with open("crates/doom-game/src/mobj.rs", "r") as f:
    content = f.read()

# I want to add `slab.alloc(make_player_mobj());` right after `generation: 1, };` in `alloc_unreachable_panic`
target = """        slab.slots[0] = Slot::Occupied {
            mobj: make_player_mobj(),
            generation: 1,
        };"""

replacement = """        slab.slots[0] = Slot::Occupied {
            mobj: make_player_mobj(),
            generation: 1,
        };

        // This should trigger the unreachable!("free list points to occupied slot") panic
        slab.alloc(make_player_mobj());"""

content = content.replace(target, replacement)
with open("crates/doom-game/src/mobj.rs", "w") as f:
    f.write(content)
