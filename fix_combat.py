import sys

def main():
    with open('crates/doom-game/src/combat.rs', 'r') as f:
        content = f.read()

    search = """        #[cfg(feature = "telemetry")]
        {
            if dmg > 0 {
                let (px, py) = gs
                    .mobjslab
                    .get(gs.player.handle)
                    .map(|mo| (mo.x, mo.y))
                    .unwrap_or_default();
                gs.telemetry.record(
                    gs.tic_num,
                    px.to_int(),
                    py.to_int(),
                    crate::telemetry::TelemetryKind::DamageTaken(dmg as u32),
                );
            }
        }
        dmg"""

    replace = """        #[cfg(feature = "telemetry")]
        {
            if dmg > 0 {
                let (px, py) = gs
                    .mobjslab
                    .get(gs.player.handle)
                    .map(|mo| (mo.x, mo.y))
                    .unwrap_or_default();
                gs.telemetry.record(
                    gs.tic_num,
                    px.to_int(),
                    py.to_int(),
                    crate::telemetry::TelemetryKind::DamageTaken(dmg as u32),
                );
            }
        }
        #[cfg(feature = "tension")]
        {
            // Increase tension heavily when damaged
            if dmg > 0 {
                gs.tension.add_threat(dmg as u32 * 2, gs.tic_num);
            }
        }
        dmg"""

    if search in content:
        content = content.replace(search, replace)
        with open('crates/doom-game/src/combat.rs', 'w') as f:
            f.write(content)
        print("Success")
    else:
        print("Could not find string")

if __name__ == "__main__":
    main()
