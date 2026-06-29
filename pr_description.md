🎨 Mosaic: UI Polish for Engine Failure

🖌️ **Before:** Fatal engine errors printed plain, ugly text with a simple prefix and indentation for the reason, which looks like a raw log file instead of a polished CLI tool.
✨ **After:** Wrapped the top-level error and its causes in a styled `comfy_table` with red headers, rounded corners, and bullet points, creating a clean dashboard-style error widget.
🖼️ **Visuals:** Shows an ASCII table with a red "❌ Engine Failure" header, the primary error description below, and a list of internal reasons formatted as bullet points in a dark grey color.
