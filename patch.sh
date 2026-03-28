#!/bin/bash

awk '
BEGIN { added_main = 0; in_tests = 0; added_tests = 0 }
/^use doom_game::\{/ && !added_main {
    sub(/\{/, "{AutomapState, ")
    added_main = 1
}
/mod tests \{/ {
    in_tests = 1
}
in_tests && /use doom_game::\{GameState, Mobj,/ && !added_tests {
    sub(/\{/, "{AutomapState, ")
    added_tests = 1
}
{ print }
' crates/doom-app/src/main.rs > crates/doom-app/src/main.rs.tmp && mv crates/doom-app/src/main.rs.tmp crates/doom-app/src/main.rs
