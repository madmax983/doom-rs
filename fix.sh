sed -i 's/set_skip(true)/set_diff_option(ratatui::buffer::CellDiffOption::Skip)/g' crates/doom-tui/src/event_loop.rs
sed -i 's/set_skip(true)/set_diff_option(ratatui::buffer::CellDiffOption::Skip)/g' crates/doom-tui/src/sixel.rs
