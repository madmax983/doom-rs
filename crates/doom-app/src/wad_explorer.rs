//! Interactive Terminal User Interface for exploring WAD files.
//!
//! Provides a `ratatui`-based interface to scroll through the WAD lump directory
//! and preview the raw contents (hex or text) of the selected lump.

use anyhow::{Context, Result};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

use doom_wad::{LumpDef, WadStack};

/// Represents the state of the TUI WAD Explorer.
pub struct WadExplorerApp<'a> {
    pub wad_stack: &'a WadStack,
    pub lumps: Vec<LumpDef>,
    pub list_state: ListState,
    pub hex_scroll_offset: usize,
    pub should_quit: bool,
}

impl<'a> WadExplorerApp<'a> {
    /// Creates a new WAD explorer state from an attached `WadStack`.
    pub fn new(wad_stack: &'a WadStack) -> Self {
        let mut list_state = ListState::default();
        let lumps: Vec<LumpDef> = wad_stack
            .all_lumps()
            .map(|(_, lump)| lump.clone())
            .collect();

        if !lumps.is_empty() {
            list_state.select(Some(0));
        }

        Self {
            wad_stack,
            lumps,
            list_state,
            hex_scroll_offset: 0,
            should_quit: false,
        }
    }
}

impl<'a> WadExplorerApp<'a> {
    /// Render the main explorer layout.
    pub fn ui(&mut self, f: &mut ratatui::Frame) {
        let size = f.area();

        // Split main screen into two columns.
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(size);

        // -- Left pane: List of lumps --
        let list_items: Vec<ListItem> = self
            .lumps
            .iter()
            .map(|lump| {
                let name = lump.name.as_str();
                let size_str = format!("{:>7} B", lump.size);
                let content = Line::from(vec![
                    Span::styled(
                        format!("{name:<8} "),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(size_str, Style::default().fg(Color::DarkGray)),
                ]);
                ListItem::new(content)
            })
            .collect();

        let list_block = Block::default()
            .borders(Borders::ALL)
            .title(" WAD Lumps (Up/Down) ");

        let lump_list = List::new(list_items)
            .block(list_block)
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        f.render_stateful_widget(lump_list, chunks[0], &mut self.list_state);

        // -- Right pane: Content Preview --
        let preview_block = Block::default()
            .borders(Borders::ALL)
            .title(" Content Preview (PageUp/PageDown, Q to quit) ");

        if let Some(selected) = self.list_state.selected() {
            let lump = &self.lumps[selected];
            if let Some(data) = self.wad_stack.lump_data(lump.name.as_str()) {
                let mut text = String::new();

                // If lump is a TEXTMAP or simple text, show as ASCII; otherwise hex dump.
                if is_text_lump(data) {
                    text = String::from_utf8_lossy(data).into_owned();
                } else {
                    // Generate a quick hex dump
                    let start = self.hex_scroll_offset.min(data.len());
                    // Rough cap: show up to 100 lines at once to avoid sluggish rendering.
                    let end = (start + 1600).min(data.len());
                    let slice = &data[start..end];

                    for (i, chunk) in slice.chunks(16).enumerate() {
                        let offset = start + i * 16;
                        use std::fmt::Write;
                        let _ = write!(&mut text, "{:08X}  ", offset);

                        for (j, byte) in chunk.iter().enumerate() {
                            let _ = write!(&mut text, "{:02X} ", byte);
                            if j == 7 {
                                text.push(' '); // extra space halfway
                            }
                        }

                        // padding
                        let missing = 16 - chunk.len();
                        for _ in 0..missing {
                            text.push_str("   ");
                        }
                        if missing >= 8 {
                            text.push(' ');
                        }

                        text.push_str(" |");
                        for byte in chunk {
                            if byte.is_ascii_graphic() || *byte == b' ' {
                                text.push(*byte as char);
                            } else {
                                text.push('.');
                            }
                        }
                        text.push_str("|\n");
                    }
                }

                let preview_paragraph = Paragraph::new(text).block(preview_block);

                f.render_widget(preview_paragraph, chunks[1]);
            } else {
                let p = Paragraph::new("Failed to load lump data.").block(preview_block);
                f.render_widget(p, chunks[1]);
            }
        } else {
            let p = Paragraph::new("No lump selected.").block(preview_block);
            f.render_widget(p, chunks[1]);
        }
    }
}

/// Simple heuristic to check if a lump is primarily printable ASCII (like TEXTMAP or DEHACKED).
fn is_text_lump(data: &[u8]) -> bool {
    if data.is_empty() {
        return true;
    }
    // Check first 256 bytes (or less)
    let len = data.len().min(256);
    let slice = &data[..len];

    // Count printable vs non-printable characters.
    let mut printable = 0;
    for &b in slice {
        if b.is_ascii_graphic() || b == b' ' || b == b'\n' || b == b'\r' || b == b'\t' {
            printable += 1;
        } else if b == 0 {
            // Early out on null bytes (definitely binary)
            return false;
        }
    }

    // If more than 90% is printable, treat it as text.
    (printable as f32 / len as f32) > 0.9
}

impl<'a> WadExplorerApp<'a> {
    /// Advance selection up.
    pub fn list_up(&mut self) {
        if self.lumps.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.lumps.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
        self.hex_scroll_offset = 0;
    }

    /// Advance selection down.
    pub fn list_down(&mut self) {
        if self.lumps.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.lumps.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
        self.hex_scroll_offset = 0;
    }

    /// Scroll hex view down.
    pub fn scroll_down(&mut self) {
        self.hex_scroll_offset = self.hex_scroll_offset.saturating_add(256);
    }

    /// Scroll hex view up.
    pub fn scroll_up(&mut self) {
        self.hex_scroll_offset = self.hex_scroll_offset.saturating_sub(256);
    }
}

/// Start the interactive TUI WAD explorer.
pub fn run_explorer(wad_stack: &WadStack) -> Result<()> {
    // Setup terminal
    enable_raw_mode().context("failed to enable raw mode")?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen).context("failed to enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("failed to initialize terminal")?;

    let mut app = WadExplorerApp::new(wad_stack);

    let res = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode().context("failed to disable raw mode")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)
        .context("failed to leave alternate screen")?;
    terminal.show_cursor().context("failed to show cursor")?;

    if let Err(err) = res {
        println!("Error in explorer: {err:?}");
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut WadExplorerApp,
) -> Result<()>
where
    <B as ratatui::backend::Backend>::Error: std::error::Error + Send + Sync + 'static,
{
    loop {
        terminal.draw(|f| app.ui(f))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
                    KeyCode::Up | KeyCode::Char('k') => app.list_up(),
                    KeyCode::Down | KeyCode::Char('j') => app.list_down(),
                    KeyCode::PageDown => app.scroll_down(),
                    KeyCode::PageUp => app.scroll_up(),
                    _ => {}
                }
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;


    fn make_test_wad_bytes(lumps: &[(&str, &[u8])]) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(b"IWAD");
        data.extend_from_slice(&(lumps.len() as i32).to_le_bytes());
        data.extend_from_slice(&0i32.to_le_bytes()); // placeholder for directory offset

        let mut offsets = Vec::new();
        for (_, payload) in lumps {
            let pos = data.len();
            data.extend_from_slice(payload);
            offsets.push((pos, payload.len()));
        }

        let dir_offset = data.len() as i32;
        data[8..12].copy_from_slice(&dir_offset.to_le_bytes());

        for (i, (name, _)) in lumps.iter().enumerate() {
            let (filepos, size) = offsets[i];
            data.extend_from_slice(&(filepos as i32).to_le_bytes());
            data.extend_from_slice(&(size as i32).to_le_bytes());

            let mut name_buf = [0u8; 8];
            let name_bytes = name.as_bytes();
            for (j, &b) in name_bytes.iter().take(8).enumerate() {
                name_buf[j] = b.to_ascii_uppercase();
            }
            data.extend_from_slice(&name_buf);
        }

        data
    }

    fn make_test_stack() -> WadStack {
        let data = make_test_wad_bytes(&[
            ("PLAYPAL", b"binarydata_playpal"),
            ("TEXTMAP", b"namespace = \"doom\";\nthing { x = 0; y = 0; }"),
            ("EMPTY", b"")
        ]);
        let mut stack = WadStack::new();
        stack.push_iwad(data).unwrap();
        stack
    }

    #[test]
    fn test_wad_explorer_init() {
        let stack = make_test_stack();
        let app = WadExplorerApp::new(&stack);

        assert_eq!(app.lumps.len(), 3);
        assert_eq!(app.list_state.selected(), Some(0));
        assert_eq!(app.hex_scroll_offset, 0);
        assert!(!app.should_quit);
    }

    #[test]
    fn test_wad_explorer_list_down_up() {
        let stack = make_test_stack();
        let mut app = WadExplorerApp::new(&stack);

        assert_eq!(app.list_state.selected(), Some(0));
        app.list_down();
        assert_eq!(app.list_state.selected(), Some(1));
        app.list_down();
        assert_eq!(app.list_state.selected(), Some(2));
        // Wrap around
        app.list_down();
        assert_eq!(app.list_state.selected(), Some(0));

        // Up wraps around to max
        app.list_up();
        assert_eq!(app.list_state.selected(), Some(2));
        app.list_up();
        assert_eq!(app.list_state.selected(), Some(1));
    }

    #[test]
    fn test_hex_scroll() {
        let stack = make_test_stack();
        let mut app = WadExplorerApp::new(&stack);

        assert_eq!(app.hex_scroll_offset, 0);
        app.scroll_down();
        assert_eq!(app.hex_scroll_offset, 256);
        app.scroll_up();
        assert_eq!(app.hex_scroll_offset, 0);
        app.scroll_up();
        assert_eq!(app.hex_scroll_offset, 0); // doesn't underflow
    }

    #[test]
    fn test_is_text_lump() {
        assert!(is_text_lump(b"namespace = \"doom\";\nthing { x = 0; y = 0; }"));
        assert!(is_text_lump(b"")); // empty is treated as text
        assert!(!is_text_lump(b"PLAYPAL\x00\x01\x02\x03\x04")); // binary data with nulls
    }
}
