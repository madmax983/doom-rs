sed -i 's/-> Vec<usize> {/-> smallvec::SmallVec<\[usize; 8\]> {/g' crates/doom-game/src/movement.rs
sed -i 's/let mut spechit: Vec<usize> = Vec::new();/let mut spechit: smallvec::SmallVec<\[usize; 8\]> = smallvec::SmallVec::new();/g' crates/doom-game/src/movement.rs
sed -i 's/let mut seen: Vec<usize> = Vec::new();/let mut seen: smallvec::SmallVec<\[usize; 32\]> = smallvec::SmallVec::new();/g' crates/doom-game/src/movement.rs
sed -i 's/vec!\[0usize\]/smallvec::smallvec!\[0usize\]/g' crates/doom-game/src/movement.rs
sed -i 's/None => Vec::new(),/None => smallvec::SmallVec::new(),/g' crates/doom-game/src/actions.rs
