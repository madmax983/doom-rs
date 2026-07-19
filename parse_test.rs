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

fn tokenize_response_file_old(content: &str) -> Vec<String> {
    let bytes: Vec<char> = content.chars().collect();
    let size = bytes.len();
    let mut out = Vec::new();
    let mut k = 0usize;
    while k < size {
        while k < size && bytes[k].is_whitespace() {
            k += 1;
        }
        if k >= size {
            break;
        }
        if bytes[k] == '"' {
            k += 1;
            let start = k;
            while k < size && bytes[k] != '"' && bytes[k] != '\n' {
                k += 1;
            }
            out.push(bytes[start..k].iter().collect());
            k += 1; // consume closing quote (or run past end)
        } else {
            let start = k;
            while k < size && !bytes[k].is_whitespace() {
                k += 1;
            }
            out.push(bytes[start..k].iter().collect());
            k += 1;
        }
    }
    out
}

fn test(s: &str) {
    let old = tokenize_response_file_old(s);
    let new = tokenize_response_file(s);
    if old != new {
        println!("FAIL: '{}'\nOld: {:?}\nNew: {:?}", s, old, new);
    } else {
        println!("PASS: '{}'", s);
    }
}

fn main() {
    test("");
    test(" ");
    test("   ");
    test("foo");
    test("foo ");
    test(" foo ");
    test("\"\"");
    test("\"foo\"");
    test("\"foo");
    test("\"foo\n");
    test("\"foo\"bar");
    test("foo\"bar\"");
    test("\"foo\" \"bar\"");
    test("hello \"world of\" \n foo\nbar");
}
