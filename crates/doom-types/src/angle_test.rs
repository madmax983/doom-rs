#[cfg(test)]
mod tests {
    extern crate std;
    use crate::angle::Bam;
    use std::thread;

    #[test]
    fn havoc_init_trig_tables() {
        let mut handles = std::vec::Vec::new();
        for _ in 0..10 {
            handles.push(thread::spawn(|| {
                unsafe { Bam::init_trig_tables() }
            }));
        }
        for h in handles {
            let _ = h.join();
        }
    }
}
