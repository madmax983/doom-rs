1. **Modify `crates/doom-game/src/movement.rs`**
   - Use `replace_with_git_merge_diff` to change `move_spechit` to return `smallvec::SmallVec<[usize; 8]>` and use `SmallVec` for `spechit` and `seen`. Also update the test assertion in the same file.
```diff
<<<<<<< SEARCH
pub fn move_spechit(
    slab: &MobjSlab,
    handle: MobjHandle,
    new_x: Fixed16_16,
    new_y: Fixed16_16,
    level: &Level,
) -> Vec<usize> {
    let (radius, mo_flags) = match slab.get(handle) {
        Some(mo) => (mo.radius, mo.flags),
        None => return Vec::new(),
    };

    let mut spechit: Vec<usize> = Vec::new();
=======
pub fn move_spechit(
    slab: &MobjSlab,
    handle: MobjHandle,
    new_x: Fixed16_16,
    new_y: Fixed16_16,
    level: &Level,
) -> smallvec::SmallVec<[usize; 8]> {
    let (radius, mo_flags) = match slab.get(handle) {
        Some(mo) => (mo.radius, mo.flags),
        None => return smallvec::SmallVec::new(),
    };

    let mut spechit: smallvec::SmallVec<[usize; 8]> = smallvec::SmallVec::new();
>>>>>>> REPLACE
<<<<<<< SEARCH
    // Vanilla `validcount`: each linedef is examined once across the whole scan.
    let mut seen: Vec<usize> = Vec::new();

    // Vanilla iterates `for (bx...) for (by...)` — column-major.
=======
    // Vanilla `validcount`: each linedef is examined once across the whole scan.
    let mut seen: smallvec::SmallVec<[usize; 32]> = smallvec::SmallVec::new();

    // Vanilla iterates `for (bx...) for (by...)` — column-major.
>>>>>>> REPLACE
<<<<<<< SEARCH
        // Baseline: no other thing → the straddled special line is collected.
        let sh = move_spechit(&slab, mover, new_x, new_y, &level);
        assert_eq!(
            sh,
            vec![0usize],
            "box straddling a two-sided special line must collect it"
        );
=======
        // Baseline: no other thing → the straddled special line is collected.
        let sh = move_spechit(&slab, mover, new_x, new_y, &level);
        assert_eq!(
            sh.as_slice(),
            &[0usize],
            "box straddling a two-sided special line must collect it"
        );
>>>>>>> REPLACE
```
   - Verify the changes using `cat crates/doom-game/src/movement.rs | grep -A 10 "pub fn move_spechit"`.

2. **Modify `crates/doom-game/src/actions.rs`**
   - Use `replace_with_git_merge_diff` to update the caller `A_Chase` where `spechit` is explicitly matched to `Vec::new()`.
```diff
<<<<<<< SEARCH
        let spechit = match level {
            Some(lv) => crate::movement::move_spechit(&gs.mobjslab, handle, new_x, new_y, lv),
            None => Vec::new(),
        };
        if spechit.is_empty() {
=======
        let spechit = match level {
            Some(lv) => crate::movement::move_spechit(&gs.mobjslab, handle, new_x, new_y, lv),
            None => smallvec::SmallVec::new(),
        };
        if spechit.is_empty() {
>>>>>>> REPLACE
```
   - Verify the changes using `cat crates/doom-game/src/actions.rs | grep -B 2 -A 2 "None => smallvec::SmallVec::new()"`.

3. **Journal learnings**
   - Append critical learning to `.jules/bolt.md` using the exact text:
     ```bash
     DATE=$(date +%Y-%m-%d)
     cat << EOF >> .jules/bolt.md
     ## $DATE - SmallVec for spechit collection
     **Learning:** \`move_spechit\` allocates two \`Vec\`s (\`spechit\` and \`seen\`) on the hot path for every object move, creating overhead.
     **Action:** Replace \`Vec::new()\` with \`smallvec::SmallVec::new()\` for temporary buffers like crossed special lines (\`spechit\`) and examined linedefs (\`seen\`) to eliminate heap allocations.
     EOF
     ```

4. **Verify changes**
   - Run `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, and `cargo fmt --all`.

5. **Complete pre-commit steps**
   - Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.

6. **Submit PR**
   - Title: "⚡ Bolt: Remove heap allocations in move_spechit"
   - Body:
     - 💡 What: Swapped `Vec::new()` for `smallvec::SmallVec::new()` in `move_spechit`.
     - 🎯 Why: `move_spechit` allocates two `Vec`s (`spechit` and `seen`) on the hot path for every object move.
     - 📊 Impact: Eliminates two heap allocations per `move_spechit` call, as the number of crossed special lines and examined linedefs is typically small enough to stay on the stack.
     - 🔬 Measurement: Run `cargo test` and observe no regression.
