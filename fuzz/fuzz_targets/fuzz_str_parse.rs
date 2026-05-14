#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = doom_tui::charset::RendererMode::from_str_loose(s);
        let _ = doom_game::dehacked::DehPatch::parse(s);
        let _ = doom_game::phase::MapId::from_name(s);
        let _ = doom_game::state::GameState::new(s);
    }
});
