use morphrpc::{
    protocol::{self, CodecError, Frame, MessageKind},
    runtime::{self, Client, Error},
};
use std::{io, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

const DEADLINE: Duration = Duration::from_secs(2);

#[tokio::test]
async fn persistent_connection_correlates_requests_and_errors() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        runtime::serve_connection(stream, DEADLINE, |operation, payload| {
            if operation == 1 {
                (MessageKind::Response, payload.to_vec())
            } else {
                (MessageKind::Error, b"unknown operation".to_vec())
            }
        })
        .await
        .unwrap();
    });
    let mut client = Client::connect(address, DEADLINE).await.unwrap();
    let first = client.call(1, vec![42]).await.unwrap();
    let second = client.call(2, vec![]).await.unwrap();
    let third = client.call(1, vec![43]).await.unwrap();
    assert_eq!((first.request_id, first.payload), (0, vec![42]));
    assert_eq!((second.request_id, second.kind), (1, MessageKind::Error));
    assert_eq!((third.request_id, third.payload), (2, vec![43]));
    drop(client);
    server.await.unwrap();
}

#[tokio::test]
async fn timeout_closes_connection_and_reuse_fails() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = [0u8; 16];
        stream.read_exact(&mut request).await.unwrap();
        let mut byte = [0];
        assert_eq!(
            tokio::time::timeout(DEADLINE, stream.read(&mut byte))
                .await
                .unwrap()
                .unwrap(),
            0
        );
    });
    let mut client = Client::connect(address, DEADLINE).await.unwrap();
    // Server accepts the request but never sends a response.
    let result = tokio::time::timeout(Duration::from_millis(50), client.call(1, vec![])).await;
    assert!(result.is_err());
    assert!(matches!(
        client.call(1, vec![]).await,
        Err(Error::Disconnected)
    ));
    server.await.unwrap();
}

#[tokio::test]
async fn client_deadline_and_server_disconnect() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = [0; 16];
        stream.read_exact(&mut request).await.unwrap();
        let mut byte = [0];
        assert_eq!(stream.read(&mut byte).await.unwrap(), 0);
    });
    let mut client = Client::connect(address, Duration::from_millis(100))
        .await
        .unwrap();
    assert!(matches!(client.call(1, vec![]).await, Err(Error::Timeout)));
    assert!(matches!(
        client.call(1, vec![]).await,
        Err(Error::Disconnected)
    ));
    server.await.unwrap();
}

#[tokio::test]
async fn server_detects_truncated_frames_and_idle_timeout() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        runtime::serve_connection(stream, DEADLINE, |_, _| (MessageKind::Response, vec![])).await
    });
    let mut stream = TcpStream::connect(address).await.unwrap();
    stream.write_all(&[0, 0, 0, 12, 1]).await.unwrap();
    drop(stream);
    assert!(
        matches!(server.await.unwrap(), Err(Error::Io(e)) if e.kind() == io::ErrorKind::UnexpectedEof)
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        runtime::serve_connection(stream, Duration::from_millis(50), |_, _| {
            (MessageKind::Response, vec![])
        })
        .await
    });
    let _stream = TcpStream::connect(address).await.unwrap();
    assert!(matches!(server.await.unwrap(), Err(Error::Timeout)));
}

#[tokio::test]
async fn server_closes_malformed_connections_before_dispatch() {
    let oversized = 1_048_577_u32.to_be_bytes();
    let mut unsupported = *include_bytes!("fixtures/v1/empty-request.bin");
    unsupported[4] = 2;

    for (bytes, expected) in [
        (
            oversized.as_slice(),
            CodecError::FrameTooLarge { length: 1_048_577 },
        ),
        (unsupported.as_slice(), CodecError::UnsupportedVersion(2)),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            runtime::serve_connection(stream, DEADLINE, |_, _| {
                panic!("malformed frame reached the dispatcher")
            })
            .await
        });

        let mut stream = TcpStream::connect(address).await.unwrap();
        stream.write_all(bytes).await.unwrap();
        // Keep the write side open. An oversized prefix must be rejected
        // without waiting for a body or an EOF.
        assert!(matches!(server.await.unwrap(), Err(Error::Codec(error)) if error == expected));
        assert_eq!(
            tokio::time::timeout(DEADLINE, stream.read(&mut [0]))
                .await
                .unwrap()
                .unwrap(),
            0
        );
    }
}

#[tokio::test]
async fn client_rejects_mismatched_response_and_disconnect() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = [0; 16];
        stream.read_exact(&mut request).await.unwrap();
        let response = Frame {
            kind: MessageKind::Response,
            operation: 1,
            request_id: 99,
            payload: vec![],
        };
        stream
            .write_all(&protocol::encode(&response).unwrap())
            .await
            .unwrap();
    });
    let mut client = Client::connect(address, DEADLINE).await.unwrap();
    assert!(matches!(
        client.call(1, vec![]).await,
        Err(Error::UnexpectedResponse)
    ));
    assert!(matches!(
        client.call(1, vec![]).await,
        Err(Error::Disconnected)
    ));
    server.await.unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = [0; 16];
        stream.read_exact(&mut request).await.unwrap();
    });
    let mut client = Client::connect(address, DEADLINE).await.unwrap();
    assert!(matches!(
        client.call(1, vec![]).await,
        Err(Error::Disconnected)
    ));
    server.await.unwrap();
}
