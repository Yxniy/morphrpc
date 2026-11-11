//! Sequential, bounded TCP request-response transport.
use crate::protocol::{self, CodecError, Frame, MessageKind};
use std::{io, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream, ToSocketAddrs},
    sync::Semaphore,
    time::timeout,
};

/// Transport and protocol errors. A failed exchange closes the client connection.
#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Codec(CodecError),
    Timeout,
    Disconnected,
    UnexpectedResponse,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Codec(e) => write!(f, "invalid frame: {e}"),
            Self::Timeout => write!(f, "operation timed out"),
            Self::Disconnected => write!(f, "connection closed"),
            Self::UnexpectedResponse => write!(f, "unexpected response"),
        }
    }
}
impl std::error::Error for Error {}
impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<CodecError> for Error {
    fn from(e: CodecError) -> Self {
        Self::Codec(e)
    }
}

// A clean EOF between frames is distinct from an EOF inside a frame.
async fn read_frame<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Option<Frame>, Error> {
    let mut prefix = [0u8; 4];
    if reader.read(&mut prefix[..1]).await? == 0 {
        return Ok(None);
    }
    reader.read_exact(&mut prefix[1..]).await?;
    match protocol::decode(&prefix) {
        Err(CodecError::Truncated { needed, .. }) => {
            let mut bytes = vec![0; needed];
            bytes[..4].copy_from_slice(&prefix);
            reader.read_exact(&mut bytes[4..]).await?;
            Ok(Some(protocol::decode(&bytes)?.0))
        }
        Err(e) => Err(e.into()),
        Ok(_) => unreachable!("a prefix alone cannot contain a frame"),
    }
}
async fn write_frame<W: AsyncWrite + Unpin>(writer: &mut W, frame: &Frame) -> Result<(), Error> {
    writer.write_all(&protocol::encode(frame)?).await?;
    Ok(())
}

/// A persistent connection with at most one outstanding request.
pub struct Client {
    stream: Option<TcpStream>,
    deadline: Duration,
    next_id: u64,
}
impl Client {
    /// Connect with a deadline applied to connection establishment and each exchange.
    pub async fn connect(address: impl ToSocketAddrs, deadline: Duration) -> Result<Self, Error> {
        let stream = timeout(deadline, TcpStream::connect(address))
            .await
            .map_err(|_| Error::Timeout)??;
        Ok(Self {
            stream: Some(stream),
            deadline,
            next_id: 0,
        })
    }
    /// Send a request and receive the correlated response or error frame.
    /// Cancelling or timing out this future closes the connection to avoid stream desynchronization.
    pub async fn call(&mut self, operation: u16, payload: Vec<u8>) -> Result<Frame, Error> {
        let request = Frame {
            kind: MessageKind::Request,
            operation,
            request_id: self.next_id,
            payload,
        };
        // Encode before taking the stream, so local input errors do not disconnect it.
        let encoded = protocol::encode(&request)?;
        let mut stream = self.stream.take().ok_or(Error::Disconnected)?;
        self.next_id = self.next_id.wrapping_add(1);
        let result = timeout(self.deadline, async {
            stream.write_all(&encoded).await?;
            let response = read_frame(&mut stream).await?.ok_or(Error::Disconnected)?;
            if !matches!(response.kind, MessageKind::Response | MessageKind::Error)
                || response.operation != request.operation
                || response.request_id != request.request_id
            {
                return Err(Error::UnexpectedResponse);
            }
            Ok(response)
        })
        .await
        .map_err(|_| Error::Timeout)?;
        if result.is_ok() {
            self.stream = Some(stream);
        }
        result
    }
}

/// Serve one TCP connection sequentially until EOF or an error.
/// The handler returns a response or error kind and its payload.
pub async fn serve_connection<F>(
    mut stream: TcpStream,
    deadline: Duration,
    handler: F,
) -> Result<(), Error>
where
    F: Fn(u16, &[u8]) -> (MessageKind, Vec<u8>),
{
    loop {
        let result = timeout(deadline, async {
            let Some(request) = read_frame(&mut stream).await? else {
                return Ok(false);
            };
            if request.kind != MessageKind::Request {
                return Err(Error::UnexpectedResponse);
            }
            let (kind, payload) = handler(request.operation, &request.payload);
            if !matches!(kind, MessageKind::Response | MessageKind::Error) {
                return Err(Error::UnexpectedResponse);
            }
            let response = Frame {
                kind,
                operation: request.operation,
                request_id: request.request_id,
                payload,
            };
            write_frame(&mut stream, &response).await?;
            Ok(true)
        })
        .await
        .map_err(|_| Error::Timeout)??;
        if !result {
            return Ok(());
        }
    }
}

/// Accept connections with a fixed cap. Excess connections are closed immediately.
pub async fn serve<F>(
    listener: TcpListener,
    deadline: Duration,
    max_connections: usize,
    handler: F,
) -> io::Result<()>
where
    F: Fn(u16, &[u8]) -> (MessageKind, Vec<u8>) + Send + Sync + 'static,
{
    if max_connections == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "max_connections must be positive",
        ));
    }
    let permits = Arc::new(Semaphore::new(max_connections));
    let handler = Arc::new(handler);
    loop {
        let (stream, _) = listener.accept().await?;
        if let Ok(permit) = permits.clone().try_acquire_owned() {
            let handler = handler.clone();
            tokio::spawn(async move {
                let _permit = permit;
                let _ = serve_connection(stream, deadline, move |operation, payload| {
                    handler(operation, payload)
                })
                .await;
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn partial_reads_and_writes() {
        let frame = Frame {
            kind: MessageKind::Request,
            operation: 1,
            request_id: 3,
            payload: vec![7; 128],
        };
        let expected = frame.clone();
        let (mut writer, mut reader) = tokio::io::duplex(1);
        let task = tokio::spawn(async move {
            write_frame(&mut writer, &frame).await.unwrap();
        });
        assert_eq!(read_frame(&mut reader).await.unwrap(), Some(expected));
        task.await.unwrap();
        assert!(read_frame(&mut reader).await.unwrap().is_none());
    }
}
