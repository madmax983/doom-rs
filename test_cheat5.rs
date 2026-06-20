fn feed(buffer: &mut String, max_len: usize, ch: char) {
    if ch.is_ascii_alphabetic() || ch.is_ascii_digit() {
        buffer.push(ch.to_ascii_lowercase());
        if buffer.len() > max_len {
            let drain_to = buffer.len() - max_len;
            buffer.drain(..drain_to);
        }
    }
}
fn main() {
    let mut buffer = String::from("日本語");
    feed(&mut buffer, 5, 'a');
}
