import sys
import re

with open("crates/doom-app/src/main.rs", "r") as f:
    content = f.read()

# Remove mod cheats;
content = re.sub(r'^mod cheats;\n', '', content, flags=re.MULTILINE)
# Remove use cheats::CheatDetector; (not the game_cheats one)
content = re.sub(r'^\s*use cheats::CheatDetector;\n', '', content, flags=re.MULTILINE)

# Remove cheat_detector from struct DoomGame
content = re.sub(r'^\s*cheat_detector:\s*cheats::CheatDetector,\n', '', content, flags=re.MULTILINE)
# Remove cheat_detector initialization
content = re.sub(r'^\s*cheat_detector:\s*cheats::CheatDetector::new\(\),\n', '', content, flags=re.MULTILINE)

# Replace console usage of cheats::apply_cheat
old_console_cheat = """                        let msg = cheats::apply_cheat(&mut self.gs, &upper);
                        let display = if msg.is_empty() {
                            format!("(ERR) UNKNOWN COMMAND {}", line)
                        } else {
                            format!("(OK) {}", msg)
                        };"""

new_console_cheat = """                        let mut buffer = game_cheats::CheatBuffer::new();
                        for ch in upper.bytes() {
                            buffer.push(ch);
                        }

                        let display = if let Some(code) = game_cheats::check_cheats(&buffer) {
                            game_cheats::apply_cheat(&mut self.gs, code);
                            format!("(OK) {}", game_cheats::cheat_message(code))
                        } else if buffer.check(b"IDDT") {
                            self.automap_full_reveal = !self.automap_full_reveal;
                            format!("(OK) Map Revealed")
                        } else {
                            format!("(ERR) UNKNOWN COMMAND {}", line)
                        };"""
content = content.replace(old_console_cheat, new_console_cheat)

# Replace the input loop that fed into cheat_detector
old_block = """                // Console is closed: feed character to the cheat detector.
                if let Some(cheat_name) = self.cheat_detector.feed(ch) {
                    let msg = cheats::apply_cheat(&mut self.gs, cheat_name);
                    // IDDT toggles full automap reveal.
                    if cheat_name == "IDDT" {
                        self.automap_full_reveal = !self.automap_full_reveal;
                    }
                    if !msg.is_empty() {
                        self.hud_messages.push(msg.to_string(), 105); // 3 sec @ 35 tics/sec
                        self.console.print(msg.to_string());
                    }
                }"""

new_block = """                // Console is closed, but Doom intercepts characters in the chatchar loop below."""
content = content.replace(old_block, new_block)

old_check = """        if input.chatchar != 0 {
            self.cheat_buffer.push(input.chatchar);
            if let Some(code) = game_cheats::check_cheats(&self.cheat_buffer) {
                game_cheats::apply_cheat(&mut self.gs, code);
                let msg = game_cheats::cheat_message(code).to_string();
                self.hud_messages.push(msg, 105); // 3 seconds at 35 tics/sec
                self.cheat_buffer.clear();
            }
        }"""

new_check = """        if input.chatchar != 0 {
            self.cheat_buffer.push(input.chatchar);
            if let Some(code) = game_cheats::check_cheats(&self.cheat_buffer) {
                game_cheats::apply_cheat(&mut self.gs, code);
                let msg = game_cheats::cheat_message(code).to_string();
                self.hud_messages.push(msg, 105); // 3 seconds at 35 tics/sec
                self.console.print(game_cheats::cheat_message(code).to_string());
                self.cheat_buffer.clear();
            } else if self.cheat_buffer.check(b"iddt") {
                self.automap_full_reveal = !self.automap_full_reveal;
                self.hud_messages.push("Map Revealed".to_string(), 105);
                self.console.print("Map Revealed".to_string());
                self.cheat_buffer.clear();
            }
        }"""
content = content.replace(old_check, new_check)

with open("crates/doom-app/src/main.rs", "w") as f:
    f.write(content)
