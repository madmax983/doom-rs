//! In-game console overlay for cheat entry and status display.
//!
//! The console is toggled with the `~` key. When visible, typed characters
//! accumulate in the input line; Enter submits the line and the caller can
//! process it as a command or cheat code.

// ---------------------------------------------------------------------------
// Console
// ---------------------------------------------------------------------------

use std::collections::VecDeque;

/// In-game console for cheat entry and status display.
pub(crate) struct Console {
    /// Whether the console overlay is currently visible.
    pub visible: bool,
    /// The current input line being typed.
    pub input: String,
    /// Recent output messages (newest at end, oldest evicted when full).
    pub messages: VecDeque<String>,
    /// Maximum number of messages to retain.
    pub max_messages: usize,
}

impl Console {
    /// Create a new hidden console with default capacity.
    pub(crate) fn new() -> Self {
        Self {
            visible: false,
            input: String::new(),
            messages: VecDeque::with_capacity(10),
            max_messages: 10,
        }
    }

    /// Toggle console visibility.
    ///
    /// Hiding the console also clears any partially-typed input.
    pub(crate) fn toggle(&mut self) {
        self.visible = !self.visible;
        if !self.visible {
            self.input.clear();
        }
    }

    /// Feed a character to the console input line.
    ///
    /// - `\x08` (backspace) removes the last character.
    /// - `\n` / `\r` (Enter) is a no-op here; the caller should call
    ///   [`Console::submit`] instead.
    /// - Other printable ASCII characters are appended.
    pub(crate) fn type_char(&mut self, ch: char) {
        if ch == '\x08' {
            self.input.pop();
        } else if ch == '\n' || ch == '\r' {
            // Caller handles submission via submit().
        } else if ch.is_ascii() && !ch.is_control() {
            self.input.push(ch);
        }
    }

    /// Submit the current input line.
    ///
    /// Clears the input buffer and returns the submitted string so the caller
    /// can process it as a command or cheat code.
    pub(crate) fn submit(&mut self) -> String {
        let line = self.input.clone();
        self.input.clear();
        line
    }

    /// Add an output message to the console.
    ///
    /// If the message count exceeds [`Console::max_messages`], the oldest
    /// message is removed.
    pub(crate) fn print(&mut self, msg: impl Into<String>) {
        self.messages.push_back(msg.into());
        if self.messages.len() > self.max_messages {
            self.messages.pop_front();
        }
    }

    /// Render the console overlay as a list of text lines.
    ///
    /// Returns up to 10 lines suitable for display by a TUI renderer.
    /// Newest messages appear near the top; the current input line is last.
    #[allow(dead_code)]
    pub(crate) fn render_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push("--- CONSOLE ---".to_string());
        for msg in self.messages.iter().rev().take(8) {
            lines.push(format!("  {msg}"));
        }
        lines.push(format!("> {}_", self.input));
        lines
    }
}

impl Default for Console {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_new_hidden() {
        let c = Console::new();
        assert!(!c.visible);
    }

    #[test]
    fn console_toggle_shows() {
        let mut c = Console::new();
        c.toggle();
        assert!(c.visible);
    }

    #[test]
    fn console_toggle_twice_hides() {
        let mut c = Console::new();
        c.toggle();
        c.toggle();
        assert!(!c.visible);
    }

    #[test]
    fn console_type_char_appends() {
        let mut c = Console::new();
        c.type_char('i');
        c.type_char('d');
        c.type_char('d');
        assert_eq!(c.input, "idd");
    }

    #[test]
    fn console_backspace_removes() {
        let mut c = Console::new();
        c.type_char('a');
        c.type_char('b');
        c.type_char('\x08');
        assert_eq!(c.input, "a");
    }

    #[test]
    fn console_submit_clears_and_returns() {
        let mut c = Console::new();
        for ch in "iddqd".chars() {
            c.type_char(ch);
        }
        let submitted = c.submit();
        assert_eq!(submitted, "iddqd");
        assert_eq!(c.input, "");
    }

    #[test]
    fn console_print_adds_message() {
        let mut c = Console::new();
        c.print("hello");
        assert_eq!(c.messages[0], "hello");
    }

    #[test]
    fn console_max_messages_evicts_oldest() {
        let mut c = Console::new();
        c.max_messages = 10;
        for i in 0..=10 {
            c.print(format!("msg {i}"));
        }
        assert_eq!(c.messages.len(), 10);
        // "msg 0" (oldest) should have been evicted; "msg 1" is now first.
        assert_eq!(c.messages[0], "msg 1");
    }
}
