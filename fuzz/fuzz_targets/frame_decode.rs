#![no_main]

use libfuzzer_sys::fuzz_target;
use morphrpc::protocol::{MAX_FRAME_LEN, decode, encode};

fuzz_target!(|data: &[u8]| {
    if let Ok((frame, consumed)) = decode(data) {
        assert!((16..=MAX_FRAME_LEN + 4).contains(&consumed));
        assert_eq!(encode(&frame).unwrap(), data[..consumed]);
    }
});
