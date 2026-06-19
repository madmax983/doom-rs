🎨 Mosaic: UI Polish for doom-app

🖌️ **Before:** The CLI used log-like text output with `crossterm::style::Stylize` colored macros, and `doom-tui` had deprecated `ratatui` APIs and boolean blindness in coordinate loops.
✨ **After:** Refactored CLI output to use `comfy_table` for a dashboard-like appearance, removed unused `Stylize` imports, updated `ratatui` methods, and applied `.skip(1)` for cleaner loops.
🖼️ **Visuals:** All CLI feedback loops now display within elegant dynamic tables, maintaining color hierarchy without polluting the console with unstructured text walls.
