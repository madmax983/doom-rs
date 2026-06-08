import re
with open("crates/doom-app/src/main.rs", "r") as f:
    content = f.read()

# Replace .green(), .red(), .cyan(), .yellow(), .bold() with ratatui equivalents or simply remove them and format using comfy_table, or use print! with ANSI escape codes
# Since we just need to fix the build, let's restore `use crossterm::style::Stylize;` and use it. Wait, I deleted it with `sed` earlier!
# Let's restore the `use crossterm::style::Stylize;`
