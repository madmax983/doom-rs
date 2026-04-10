import os
import re

def fix_file(filepath):
    with open(filepath, 'r') as f:
        content = f.read()

    # Replace `pub struct/enum/fn/type/const` with `pub(crate) ...`
    # if they are not already pub(crate)

    # Be careful not to replace `pub use`
    lines = content.split('\n')
    out = []
    for line in lines:
        if line.startswith('pub struct '):
            out.append(line.replace('pub struct ', 'pub(crate) struct ', 1))
        elif line.startswith('pub enum '):
            out.append(line.replace('pub enum ', 'pub(crate) enum ', 1))
        elif line.startswith('pub type '):
            out.append(line.replace('pub type ', 'pub(crate) type ', 1))
        elif line.startswith('pub const '):
            out.append(line.replace('pub const ', 'pub(crate) const ', 1))
        elif line.startswith('pub fn '):
            out.append(line.replace('pub fn ', 'pub(crate) fn ', 1))
        elif line.startswith('    pub fn '):
            out.append(line.replace('    pub fn ', '    pub(crate) fn ', 1))
        elif line.startswith('    pub name:'):
            out.append(line.replace('    pub name:', '    pub(crate) name:', 1))
        elif line.startswith('    pub sequence:'):
            out.append(line.replace('    pub sequence:', '    pub(crate) sequence:', 1))
        elif line.startswith('    pub visible:'):
            out.append(line.replace('    pub visible:', '    pub(crate) visible:', 1))
        elif line.startswith('    pub input:'):
            out.append(line.replace('    pub input:', '    pub(crate) input:', 1))
        elif line.startswith('    pub messages:'):
            out.append(line.replace('    pub messages:', '    pub(crate) messages:', 1))
        elif line.startswith('    pub max_messages:'):
            out.append(line.replace('    pub max_messages:', '    pub(crate) max_messages:', 1))
        else:
            out.append(line)

    with open(filepath, 'w') as f:
        f.write('\n'.join(out))

for mod in ['audio_system.rs', 'cheats.rs', 'console.rs', 'demo_mode.rs', 'savegame.rs', 'cogmind/effects.rs', 'cogmind/glyphs.rs', 'cogmind/lighting.rs', 'cogmind/render.rs', 'cogmind/visibility.rs', 'cogmind/tile_grid.rs', 'cogmind/sight_line.rs']:
    fix_file('crates/doom-app/src/' + mod)
