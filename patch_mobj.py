import sys

with open("crates/doom-game/src/mobj.rs", "r") as f:
    lines = f.readlines()

test_code = """
    #[test]
    #[should_panic(expected = "free list points to occupied slot")]
    fn alloc_unreachable_panic() {
        let mut slab = MobjSlab::new();
        let handle1 = slab.alloc(make_player_mobj());
        slab.free(handle1);

        // Artificially corrupt the slab to hit the unreachable arm
        slab.slots[0] = Slot::Occupied { mobj: make_player_mobj(), generation: 1 };

        let _handle2 = slab.alloc(make_player_mobj());
    }
}
"""

for i in range(len(lines) - 1, -1, -1):
    if lines[i].strip() == "}":
        lines[i] = test_code
        break

with open("crates/doom-game/src/mobj.rs", "w") as f:
    f.writelines(lines)
