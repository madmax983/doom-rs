🎨 Mosaic: UI Polish for [doom-app CLI]

🖌️ **Before:** The `--pathfind` CLI output displayed raw paths as a single line separated by arrows (e.g., `0 ➔ 18 ➔ 15 ➔ 143 ➔ 5`).
✨ **After:** The `--pathfind` output is now formatted into a beautiful, easy-to-read table.
🖼️ **Visuals:** Added `comfy-table` formatting showing the step index and sector ID, matching the style of other CLI flags like `--map-stats` and `--analyze`.
