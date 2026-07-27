fn main() {
    let origin = Some(42);
    let mut channel_origins = [None; 8];
    channel_origins[3] = origin;

    if let Some(channel) = channel_origins.iter().position(|&o| o == origin) {
        println!("Channel: {}", channel);
    }
}
