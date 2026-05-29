import re
with open("crates/doom-game/src/specials.rs", "r") as f:
    data = f.read()

# Fix doc comments
data = re.sub(r'/// Standard lift wait time: 3 seconds at 35 Hz = 105 tics\.\n\n.*?\n/// Activate a CrushAndRaise ceiling', '/// Activate a CrushAndRaise ceiling', data, flags=re.DOTALL)
data = re.sub(r'/// Activate a lift \(lower-wait-raise\) on all sectors matching `tag`\.\n\n.*?\npub fn ev_do_lift', 'pub fn ev_do_lift', data, flags=re.DOTALL)
data = re.sub(r'/// Enqueue a door mover that closes a door\.\n\n.*?\n/// Port of `P_UseLines`', '/// Port of `P_UseLines`', data, flags=re.DOTALL)

with open("crates/doom-game/src/specials.rs", "w") as f:
    f.write(data)
