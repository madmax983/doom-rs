with open('crates/doom-wad/src/stack.rs', 'r') as f:
    stack_content = f.read()

import re

# Remove the should_panic tests we added
stack_content = re.sub(r'    #\[test\]\n    #\[should_panic\(expected = "Expected Classic map lump group"\)\]\n    fn map_lump_group_searches_pwads_first_panics_on_udmf\(\) \{.*?    \}\n\n', '', stack_content, flags=re.DOTALL)
stack_content = re.sub(r'    #\[test\]\n    #\[should_panic\(expected = "Expected UDMF map lump group"\)\]\n    fn map_lump_group_searches_pwads_first_panics_on_classic\(\) \{.*?    \}\n\n', '', stack_content, flags=re.DOTALL)

with open('crates/doom-wad/src/stack.rs', 'w') as f:
    f.write(stack_content)
print("Removed should_panic tests from stack.rs")


with open('crates/doom-wad/src/wad.rs', 'r') as f:
    wad_content = f.read()

wad_content = re.sub(r'    #\[test\]\n    #\[should_panic\(expected = "expected UDMF map group"\)\]\n    fn map_lump_group_detects_udmf_group_panics_on_classic\(\) \{.*?    \}\n\n', '', wad_content, flags=re.DOTALL)
wad_content = re.sub(r'    #\[test\]\n    #\[should_panic\(expected = "Expected UDMF"\)\]\n    fn udmf_map_lump_group_methods_panics_on_classic\(\) \{.*?    \}\n\}', '}', wad_content, flags=re.DOTALL)

with open('crates/doom-wad/src/wad.rs', 'w') as f:
    f.write(wad_content)
print("Removed should_panic tests from wad.rs")
