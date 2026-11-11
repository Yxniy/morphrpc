use morphrpc::protocol::{CodecError, Frame, MAX_FRAME_LEN, MessageKind, decode, encode};

const REQUEST: &[u8] = include_bytes!("fixtures/v1/request.bin");
const RESPONSE: &[u8] = include_bytes!("fixtures/v1/response.bin");
const ERROR: &[u8] = include_bytes!("fixtures/v1/error.bin");
const EMPTY_REQUEST: &[u8] = include_bytes!("fixtures/v1/empty-request.bin");

fn golden_frames() -> [(Frame, &'static [u8]); 4] {
    [
        (
            Frame {
                kind: MessageKind::Request,
                operation: 1,
                request_id: 2,
                payload: vec![0xaa, 0xbb],
            },
            REQUEST,
        ),
        (
            Frame {
                kind: MessageKind::Response,
                operation: 0x1234,
                request_id: 0x0123_4567_89ab_cdef,
                payload: vec![0x00, 0x7f, 0x80, 0xff],
            },
            RESPONSE,
        ),
        (
            Frame {
                kind: MessageKind::Error,
                operation: u16::MAX,
                request_id: u64::MAX,
                payload: vec![],
            },
            ERROR,
        ),
        (
            Frame {
                kind: MessageKind::Request,
                operation: 1,
                request_id: 0,
                payload: vec![],
            },
            EMPTY_REQUEST,
        ),
    ]
}

#[test]
fn encodes_and_decodes_v1_golden_frames() {
    for (expected, bytes) in golden_frames() {
        assert_eq!(encode(&expected).unwrap(), bytes);
        assert_eq!(decode(bytes).unwrap(), (expected, bytes.len()));
    }
}

#[test]
fn consumes_one_golden_frame_at_a_time() {
    let stream = [REQUEST, RESPONSE, ERROR, EMPTY_REQUEST].concat();
    let mut remaining = stream.as_slice();
    for (expected, bytes) in golden_frames() {
        let (decoded, consumed) = decode(remaining).unwrap();
        assert_eq!(decoded, expected);
        assert_eq!(consumed, bytes.len());
        remaining = &remaining[consumed..];
    }
    assert!(remaining.is_empty());
}

#[test]
fn rejects_every_golden_frame_truncation() {
    for (_, bytes) in golden_frames() {
        for available in 0..bytes.len() {
            let needed = if available < 4 { 4 } else { bytes.len() };
            assert_eq!(
                decode(&bytes[..available]),
                Err(CodecError::Truncated { needed, available }),
            );
        }
    }
}

#[test]
fn rejects_invalid_header_fields() {
    let mut invalid = REQUEST.to_vec();
    for version in (0..=u8::MAX).filter(|&version| version != 1) {
        invalid[4] = version;
        assert_eq!(
            decode(&invalid),
            Err(CodecError::UnsupportedVersion(version))
        );
    }
    invalid[4] = 1;
    invalid[5] = 3;
    assert_eq!(decode(&invalid), Err(CodecError::InvalidKind(3)));
    invalid[5] = 0;
    invalid[6..8].fill(0);
    assert_eq!(decode(&invalid), Err(CodecError::InvalidOperation));
    assert_eq!(
        decode(&[0, 0, 0, 11]),
        Err(CodecError::FrameTooShort { length: 11 })
    );
    let mut invalid_frame = decode(REQUEST).unwrap().0;
    invalid_frame.operation = 0;
    assert_eq!(encode(&invalid_frame), Err(CodecError::InvalidOperation));
}

#[test]
fn enforces_frame_size_on_encoding_and_prefix_read() {
    const V1_MAX_BODY_LEN: usize = 1_048_576;
    assert_eq!(MAX_FRAME_LEN, V1_MAX_BODY_LEN);
    let mut large = decode(REQUEST).unwrap().0;
    large.payload = vec![0; V1_MAX_BODY_LEN - 12];
    let bytes = encode(&large).unwrap();
    assert_eq!(&bytes[..4], &[0, 16, 0, 0]);
    assert_eq!(
        decode(&bytes).unwrap(),
        (large.clone(), 4 + V1_MAX_BODY_LEN)
    );
    large.payload.push(0);
    assert_eq!(
        encode(&large),
        Err(CodecError::FrameTooLarge {
            length: V1_MAX_BODY_LEN + 1
        })
    );
    for length in [V1_MAX_BODY_LEN as u32 + 1, u32::MAX] {
        assert_eq!(
            decode(&length.to_be_bytes()),
            Err(CodecError::FrameTooLarge {
                length: length as usize
            })
        );
    }
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
