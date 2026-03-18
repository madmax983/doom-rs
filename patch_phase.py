import sys

with open("crates/doom-game/src/phase.rs", "r") as f:
    lines = f.readlines()

test_code = """
    #[test]
    #[should_panic(expected = "internal error: entered unreachable code")]
    fn tick_intermission_unreachable_panic() {
        let mut ctrl = GamePhaseController::new(MapId::new(1, 1));

        // Force state into Playing while skip is requested.
        // This triggers the first branch of tick_intermission but fails the match
        ctrl.phase = GamePhase::Playing;
        ctrl.skip_requested = true;
        ctrl.tick_intermission();
    }
}
"""

for i in range(len(lines) - 1, -1, -1):
    if lines[i].strip() == "}":
        lines[i] = test_code
        break

with open("crates/doom-game/src/phase.rs", "w") as f:
    f.writelines(lines)
