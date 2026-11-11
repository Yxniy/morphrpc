//! Bounded v1 frame encoding and decoding. See `docs/wire-format-v1.md`.

use std::error::Error;
use std::fmt;

const INITIAL_VERSION: u8 = 1;
const HEADER_LEN: usize = 12;
const PREFIX_LEN: usize = 4;
/// Maximum number of bytes following the length prefix.
pub const MAX_FRAME_LEN: usize = 1024 * 1024;

/// Returns whether the version is recognized by this implementation.
pub const fn supports_version(version: u8) -> bool {
    version == INITIAL_VERSION
}

/// The role of a frame in a request-response exchange.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageKind {
    Request = 0,
    Response = 1,
    Error = 2,
}

/// A frame with an opaque, operation-specific payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub kind: MessageKind,
    pub operation: u16,
    pub request_id: u64,
    pub payload: Vec<u8>,
}

/// Reasons a frame could not be encoded or decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    Truncated { needed: usize, available: usize },
    FrameTooLarge { length: usize },
    FrameTooShort { length: usize },
    UnsupportedVersion(u8),
    InvalidKind(u8),
    InvalidOperation,
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated { needed, available } => {
                write!(f, "truncated frame: need {needed} bytes, have {available}")
            }
            Self::FrameTooLarge { length } => {
                write!(f, "frame length {length} exceeds {MAX_FRAME_LEN}")
            }
            Self::FrameTooShort { length } => {
                write!(f, "frame length {length} is shorter than the header")
            }
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported protocol version {version}")
            }
            Self::InvalidKind(kind) => write!(f, "invalid message kind {kind}"),
            Self::InvalidOperation => write!(f, "operation identifier must be nonzero"),
        }
    }
}

impl Error for CodecError {}

/// Encodes one frame, including its four-byte length prefix.
pub fn encode(frame: &Frame) -> Result<Vec<u8>, CodecError> {
    if frame.operation == 0 {
        return Err(CodecError::InvalidOperation);
    }
    let length = HEADER_LEN
        .checked_add(frame.payload.len())
        .ok_or(CodecError::FrameTooLarge { length: usize::MAX })?;
    if length > MAX_FRAME_LEN {
        return Err(CodecError::FrameTooLarge { length });
    }
    let mut bytes = Vec::with_capacity(PREFIX_LEN + length);
    bytes.extend_from_slice(&(length as u32).to_be_bytes());
    bytes.push(INITIAL_VERSION);
    bytes.push(frame.kind as u8);
    bytes.extend_from_slice(&frame.operation.to_be_bytes());
    bytes.extend_from_slice(&frame.request_id.to_be_bytes());
    bytes.extend_from_slice(&frame.payload);
    Ok(bytes)
}

/// Decodes the first frame and returns it with the number of bytes consumed.
/// A truncated input may be retried after reading the reported number of bytes.
pub fn decode(bytes: &[u8]) -> Result<(Frame, usize), CodecError> {
    if bytes.len() < PREFIX_LEN {
        return Err(CodecError::Truncated {
            needed: PREFIX_LEN,
            available: bytes.len(),
        });
    }
    let length = u32::from_be_bytes(bytes[..PREFIX_LEN].try_into().unwrap()) as usize;
    if length > MAX_FRAME_LEN {
        return Err(CodecError::FrameTooLarge { length });
    }
    if length < HEADER_LEN {
        return Err(CodecError::FrameTooShort { length });
    }
    let total = PREFIX_LEN + length;
    if bytes.len() < total {
        return Err(CodecError::Truncated {
            needed: total,
            available: bytes.len(),
        });
    }
    let version = bytes[4];
    if !supports_version(version) {
        return Err(CodecError::UnsupportedVersion(version));
    }
    let kind = match bytes[5] {
        0 => MessageKind::Request,
        1 => MessageKind::Response,
        2 => MessageKind::Error,
        other => return Err(CodecError::InvalidKind(other)),
    };
    let operation = u16::from_be_bytes(bytes[6..8].try_into().unwrap());
    if operation == 0 {
        return Err(CodecError::InvalidOperation);
    }
    let request_id = u64::from_be_bytes(bytes[8..16].try_into().unwrap());
    Ok((
        Frame {
            kind,
            operation,
            request_id,
            payload: bytes[16..total].to_vec(),
        },
        total,
    ))
}
