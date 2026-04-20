import re

with open('crates/doom-app/src/main.rs', 'r') as f:
    content = f.read()

content = content.replace('.apply_modifier(comfy_table::modifiers::UTF8_ROUND_CORNERS);', '.apply_modifier(comfy_table::modifiers::UTF8_ROUND_CORNERS)\n                .set_content_arrangement(comfy_table::ContentArrangement::Dynamic);')

with open('crates/doom-app/src/main.rs', 'w') as f:
    f.write(content)
