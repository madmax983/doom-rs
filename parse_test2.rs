fn tokenize_response_file(content: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = content.char_indices().peekable();

    while let Some(&(i, c)) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c == '"' {
            chars.next(); // consume '"'
            let start = chars.peek().map(|&(idx, _)| idx).unwrap_or(content.len());
            let mut end = content.len();
            while let Some(&(idx, c)) = chars.peek() {
                if c == '"' || c == '\n' {
                    end = idx;
                    break;
                }
                chars.next();
            }
            out.push(content[start..end].to_string());
            if let Some(&(_, c)) = chars.peek() {
                if c == '"' {
                    chars.next(); // consume closing quote
                }
            }
        } else {
            let start = i;
            let mut end = content.len();
            while let Some(&(idx, c)) = chars.peek() {
                if c.is_whitespace() {
                    end = idx;
                    break;
                }
                chars.next();
            }
            out.push(content[start..end].to_string());
        }
    }
    out
}
fn main() {}
