import sys

def process(filepath):
    with open(filepath, "r") as f:
        content = f.read()

    old_code = """fn p_new_chase_dir_get_target(
    gs: &GameState,
    handle: MobjHandle,
) -> Option<(MobjHandle, i32, i32)> {
    let target_handle = gs.mobjslab.get(handle)?.target;
    if target_handle == MobjHandle::NULL {
        return None;
    }
    let t = gs.mobjslab.get(target_handle)?;
    Some((target_handle, t.x.to_int(), t.y.to_int()))
}"""

    new_code = """fn p_new_chase_dir_get_target(
    gs: &GameState,
    handle: MobjHandle,
) -> Option<(MobjHandle, Fixed16_16, Fixed16_16)> {
    let target_handle = gs.mobjslab.get(handle)?.target;
    if target_handle == MobjHandle::NULL {
        return None;
    }
    let t = gs.mobjslab.get(target_handle)?;
    Some((target_handle, t.x, t.y))
}"""

    old_code2 = """    let (mo_x, mo_y) = (mo.x.to_int(), mo.y.to_int());

    let Some((_target_handle, tx, ty)) = p_new_chase_dir_get_target(gs, handle) else {
        if let Some(mo) = gs.mobjslab.get_mut(handle) {
            mo.movedir = DI_NODIR;
        }
        return;
    };

    let candidates = p_new_chase_dir_candidates(tx - mo_x, ty - mo_y);"""

    new_code2 = """    let (mo_x, mo_y) = (mo.x, mo.y);

    let Some((_target_handle, tx, ty)) = p_new_chase_dir_get_target(gs, handle) else {
        if let Some(mo) = gs.mobjslab.get_mut(handle) {
            mo.movedir = DI_NODIR;
        }
        return;
    };

    let candidates = p_new_chase_dir_candidates((tx - mo_x).to_int(), (ty - mo_y).to_int());"""
    if old_code in content and old_code2 in content:
        with open(filepath, "w") as f:
            f.write(content.replace(old_code, new_code).replace(old_code2, new_code2))
        print("Success")
    else:
        print("Could not find old code")

if __name__ == "__main__":
    process("crates/doom-game/src/actions.rs")
