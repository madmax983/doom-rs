import sys

def main():
    with open('crates/doom-game/src/actions.rs', 'r') as f:
        content = f.read()

    search = """    let dist = (tx - mo_x).to_int().abs() + (ty - mo_y).to_int().abs();

    let info = match mobjinfo::MOBJINFO.get(mo_kind as usize) {"""

    replace = """    let dist = (tx - mo_x).to_int().abs() + (ty - mo_y).to_int().abs();

    #[cfg(feature = "tension")]
    {
        if current_target == gs.player.handle {
            // Threat scales inversely with distance, up to max threat of 5 per monster tick
            let threat_increase = (1000_i32 - dist.min(1000)).max(0) / 200;
            if threat_increase > 0 {
                gs.tension.add_threat(threat_increase as u32, gs.tic_num);
            }
        }
    }

    let info = match mobjinfo::MOBJINFO.get(mo_kind as usize) {"""

    if search in content:
        content = content.replace(search, replace)
        with open('crates/doom-game/src/actions.rs', 'w') as f:
            f.write(content)
        print("Success")
    else:
        print("Could not find string")

if __name__ == "__main__":
    main()
