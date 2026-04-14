with open('crates/doom-game/src/tic.rs', 'r') as f:
    lines = f.readlines()

new_lines = []
for line in lines:
    if 'tick_world(self, level);' in line:
        new_lines.append(line)
        new_lines.append('        #[cfg(feature = "style_meter")]\n')
        new_lines.append('        self.style.tick(self.tic_num);\n')
    else:
        new_lines.append(line)

with open('crates/doom-game/src/tic.rs', 'w') as f:
    f.writelines(new_lines)
