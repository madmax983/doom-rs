🎨 Mosaic: UI Polish for Pathfinding CLI

🖌️ **Before:** The `--pathfind` CLI output printed a raw, single-line log message for the resulting path.
✨ **After:** The pathfinding output now uses `comfy-table` to render a structured, visually appealing table, maintaining consistency with other CLI features like `--analyze` and `--map-stats`.
🖼️ **Visuals:** A rounded-corner table with "Feature" and "Data" columns, coloring the path sequence green to ensure important information pops.
