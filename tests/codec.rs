use morphrpc::protocol::{CodecError, Frame, MAX_FRAME_LEN, MessageKind, decode, encode};

fn frame() -> Frame {
    Frame {
        kind: MessageKind::Request,
        operation: 1,
        request_id: 2,
        payload: vec![0xaa, 0xbb],
    }
}

#[test]
fn matches_wire_example_and_consumes_one_frame() {
    let expected = [0, 0, 0, 14, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 2, 0xaa, 0xbb];
    assert_eq!(encode(&frame()).unwrap(), expected);
    let mut stream = expected.to_vec();
    stream.extend_from_slice(&expected);
    let (decoded, consumed) = decode(&stream).unwrap();
    assert_eq!(decoded, frame());
    assert_eq!(consumed, expected.len());
    assert_eq!(decode(&stream[consumed..]).unwrap().0, frame());
}

#[test]
fn rejects_every_truncation() {
    let bytes = encode(&frame()).unwrap();
    for length in 0..bytes.len() {
        assert!(
            matches!(decode(&bytes[..length]), Err(CodecError::Truncated { .. })),
            "length {length}"
        );
    }
}

#[test]
fn rejects_invalid_header_fields() {
    let bytes = encode(&frame()).unwrap();
    let mut invalid = bytes.clone();
    invalid[4] = 2;
    assert_eq!(decode(&invalid), Err(CodecError::UnsupportedVersion(2)));
    invalid = bytes.clone();
    invalid[5] = 3;
    assert_eq!(decode(&invalid), Err(CodecError::InvalidKind(3)));
    invalid = bytes;
    invalid[6..8].fill(0);
    assert_eq!(decode(&invalid), Err(CodecError::InvalidOperation));
    assert_eq!(
        decode(&[0, 0, 0, 11]),
        Err(CodecError::FrameTooShort { length: 11 })
    );
    let mut invalid_frame = frame();
    invalid_frame.operation = 0;
    assert_eq!(encode(&invalid_frame), Err(CodecError::InvalidOperation));
}

#[test]
fn enforces_frame_size_on_encoding_and_prefix_read() {
    let mut large = frame();
    large.payload = vec![0; MAX_FRAME_LEN - 12];
    let bytes = encode(&large).unwrap();
    assert_eq!(decode(&bytes).unwrap().0, large);
    large.payload.push(0);
    assert_eq!(
        encode(&large),
        Err(CodecError::FrameTooLarge {
            length: MAX_FRAME_LEN + 1
        })
    );
    assert_eq!(
        decode(&((MAX_FRAME_LEN as u32 + 1).to_be_bytes())),
        Err(CodecError::FrameTooLarge {
            length: MAX_FRAME_LEN + 1
        })
    );
    assert_eq!(
        decode(&u32::MAX.to_be_bytes()),
        Err(CodecError::FrameTooLarge {
            length: u32::MAX as usize
        })
    );
}

#[test]
fn round_trips_all_kinds_with_empty_payload() {
    for kind in [
        MessageKind::Request,
        MessageKind::Response,
        MessageKind::Error,
    ] {
        let value = Frame {
            kind,
            operation: u16::MAX,
            request_id: u64::MAX,
            payload: vec![],
        };
        let encoded = encode(&value).unwrap();
        assert_eq!(decode(&encoded).unwrap(), (value, encoded.len()));
    }
}
