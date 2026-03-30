#![no_main]

use doom_net::packet::{TicPacket, TicCmd, MAX_PLAYERS};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Some(packet) = TicPacket::from_bytes(data) {
        let _bytes = packet.to_bytes();
    }
});
