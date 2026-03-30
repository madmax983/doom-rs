fn main() {
    let data = vec![0u8; 100];
    let end = 8 + 50;
    let v: std::sync::Arc<[u8]> = data[8..end].to_vec().into();
    println!("OK: {}", v.len());
}
