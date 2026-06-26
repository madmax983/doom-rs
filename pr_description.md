🎨 Mosaic: UI Polish for CLI Outputs

🖌️ **Before:** Pathfinding results and engine errors printed as raw text walls or poorly styled lines, breaking visual hierarchy.
✨ **After:** Pathfinding results and engine errors are now formatted inside cleanly styled `comfy-table` boxes, maintaining consistency with `--analyze` output.
🖼️ **Visuals:** Errors display a bold red header with yellow reason breakdowns. Pathfinding displays a bold cyan header with a yellow path string. Both use full UTF-8 borders with rounded corners.
