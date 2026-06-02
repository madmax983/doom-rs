use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use doom_wad::WadStack;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use std::io;

struct TermGuard;
impl TermGuard {
    fn init() -> Result<Self> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;
        Ok(Self)
    }
}
impl Drop for TermGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}

struct ExplorerItem {
    name: String,
    size: usize,
    wad_name: String,
    data_preview: String,
}

pub fn run_explorer(wad_stack: &WadStack) -> Result<()> {
    // 1. Gather all lumps
    let mut items = Vec::new();
    for (wad, lump) in wad_stack.all_lumps() {
        let name = lump.name.as_str().to_string();
        let size = lump.size;
        let wad_name = format!("{:?}", wad.kind());

        let mut data_preview = String::new();
        let data = wad.lump_data(lump);
        let limit = data.len().min(256);
        for chunk in data[..limit].chunks(16) {
            let hex: Vec<String> = chunk.iter().map(|b| format!("{:02X}", b)).collect();
            let ascii: String = chunk
                .iter()
                .map(|&b| {
                    if b.is_ascii_graphic() || b == b' ' {
                        b as char
                    } else {
                        '.'
                    }
                })
                .collect();
            data_preview.push_str(&format!("{:<48} | {}\n", hex.join(" "), ascii));
        }

        items.push(ExplorerItem {
            name,
            size,
            wad_name,
            data_preview,
        });
    }

    if items.is_empty() {
        return Ok(());
    }

    // 2. Setup Terminal
    let _guard = TermGuard::init()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    terminal.clear()?;

    let mut list_state = ListState::default();
    list_state.select(Some(0));

    // 3. Main Loop
    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
                .split(f.area());

            let list_items: Vec<ListItem> = items
                .iter()
                .map(|i| {
                    ListItem::new(Line::from(vec![
                        Span::styled(
                            format!("{:<8} ", i.name),
                            Style::default().fg(Color::Yellow),
                        ),
                        Span::styled(format!("{:>8} B", i.size), Style::default().fg(Color::Gray)),
                    ]))
                })
                .collect();

            let list = List::new(list_items)
                .block(Block::default().borders(Borders::ALL).title("WAD Lumps"))
                .highlight_style(
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                );

            f.render_stateful_widget(list, chunks[0], &mut list_state);

            if let Some(selected) = list_state.selected() {
                let item = &items[selected];
                let detail_text = format!(
                    "Name: {}\nWAD: {}\nSize: {} bytes\n\nPreview (up to 256 bytes):\n{}",
                    item.name, item.wad_name, item.size, item.data_preview
                );
                let detail = Paragraph::new(detail_text)
                    .block(Block::default().borders(Borders::ALL).title("Lump Details"));
                f.render_widget(detail, chunks[1]);
            }
        })?;

        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Down | KeyCode::Char('j') => {
                            let i = match list_state.selected() {
                                Some(i) => {
                                    if i >= items.len() - 1 {
                                        0
                                    } else {
                                        i + 1
                                    }
                                }
                                None => 0,
                            };
                            list_state.select(Some(i));
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            let i = match list_state.selected() {
                                Some(i) => {
                                    if i == 0 {
                                        items.len() - 1
                                    } else {
                                        i - 1
                                    }
                                }
                                None => 0,
                            };
                            list_state.select(Some(i));
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    terminal.show_cursor()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_explorer_items_parse() {
        // Just testing that the concept is sound without rendering to terminal.
        let mut wad_data = vec![0u8; 12];
        wad_data[0..4].copy_from_slice(b"IWAD");
        wad_data[4..8].copy_from_slice(&1i32.to_le_bytes()); // 1 lump
        wad_data[8..12].copy_from_slice(&16i32.to_le_bytes()); // dir at 16

        wad_data.extend_from_slice(b"TEST"); // data at 12..16

        // Dir entry at 16
        wad_data.extend_from_slice(&12i32.to_le_bytes());
        wad_data.extend_from_slice(&4i32.to_le_bytes());
        wad_data.extend_from_slice(b"MOCKLUMP");

        let mut stack = WadStack::new();
        stack.push_iwad(wad_data).unwrap();

        let mut items = Vec::new();
        for (wad, lump) in stack.all_lumps() {
            let data = wad.lump_data(lump);
            items.push(data.to_vec());
        }

        assert_eq!(items.len(), 1);
        assert_eq!(items[0], b"TEST");
    }
}
