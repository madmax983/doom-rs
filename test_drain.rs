fn main() {
    let mut buffer = String::from("abcdefghijklmnopqrstuvwxyz");
    buffer.drain(..4);
    println!("drain ascii: {}", buffer);

    let mut buffer = String::from("日本語");
    let drain_to = buffer.len() - 1;
    buffer.drain(..drain_to);
    println!("drain multibyte: {}", buffer);
}
