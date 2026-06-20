fn feed(buffer: &mut String, max_len: usize, ch: char) {
    if ch.is_ascii_alphabetic() || ch.is_ascii_digit() {
        buffer.push(ch.to_ascii_lowercase());
        while buffer.chars().count() > max_len {
            buffer.remove(0);
        }
    }
}
fn main() {
    let mut buffer = String::from("日本語");
    feed(&mut buffer, 5, 'a');
    println!("buffer: {}", buffer);
}
