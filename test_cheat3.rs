fn feed(buffer: &mut String, max_len: usize, ch: char) {
    if ch.is_alphabetic() || ch.is_digit(10) {
        buffer.push(ch.to_ascii_lowercase());
        if buffer.len() > max_len {
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
