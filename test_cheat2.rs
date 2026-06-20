fn feed(buffer: &mut String, max_len: usize, ch: char) {
    if ch.is_ascii_alphabetic() || ch.is_ascii_digit() {
        buffer.push(ch.to_ascii_lowercase());
        if buffer.len() > max_len {
            // Find the character boundary to prevent panics
            // Since we only ever push ascii lowercase chars, byte index == char index
            // But if the buffer already had non-ascii, this could fail
            let drain_to = buffer.len() - max_len;
            buffer.drain(..drain_to);
        }
    }
}
fn main() {
    let mut buffer = String::new();
    feed(&mut buffer, 5, '日');
    println!("buffer: {}", buffer);
    feed(&mut buffer, 5, 'A');
    println!("buffer: {}", buffer);
}
