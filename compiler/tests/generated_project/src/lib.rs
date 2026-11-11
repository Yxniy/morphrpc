#![forbid(unsafe_code)]
include!(concat!(env!("OUT_DIR"), "/generated.rs"));
pub mod empty {
    include!(concat!(env!("OUT_DIR"), "/empty.rs"));
}

pub mod handoff {
    include!(concat!(env!("OUT_DIR"), "/handoff.rs"));
}

#[cfg(test)]
mod tests {
    use super::*;
    use morphrpc::{protocol::MessageKind, runtime};
    use std::time::Duration;
    use tokio::net::TcpListener;

    fn request() -> Request {
        Request {
            id: 2,
            count: 3,
            active: true,
            name: "Ada".into(),
            data: vec![0xaa, 0xbb],
        }
    }
    #[test]
    fn empty_message_round_trip() {
        let value = empty::EmptyRequest {};
        assert_eq!(value.encode().unwrap(), Vec::<u8>::new());
        assert_eq!(empty::EmptyRequest::decode(&[]).unwrap(), value);
        assert_eq!(
            empty::EmptyRequest::decode(&[0]),
            Err(empty::PayloadError::TrailingBytes)
        );
    }
    #[test]
    fn golden_bytes_and_malformed_inputs() {
        let expected = [
            0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 3, 1, 0, 0, 0, 3, b'A', b'd', b'a', 0, 0, 0, 2, 0xaa,
            0xbb,
        ];
        assert_eq!(request().encode().unwrap(), expected);
        assert_eq!(Request::decode(&expected).unwrap(), request());
        for n in 0..expected.len() {
            assert!(Request::decode(&expected[..n]).is_err());
        }
        let mut invalid = expected.to_vec();
        invalid[12] = 2;
        assert_eq!(Request::decode(&invalid), Err(PayloadError::InvalidBoolean));
        let mut invalid = expected.to_vec();
        invalid[17] = 0xff;
        assert_eq!(Request::decode(&invalid), Err(PayloadError::InvalidUtf8));
        let mut invalid = expected.to_vec();
        invalid.push(0);
        assert_eq!(Request::decode(&invalid), Err(PayloadError::TrailingBytes));
        let mut invalid = expected.to_vec();
        invalid[13..17].copy_from_slice(&u32::MAX.to_be_bytes());
        assert_eq!(Request::decode(&invalid), Err(PayloadError::TooLarge));
        let mut huge = request();
        huge.data = vec![0; morphrpc::protocol::MAX_FRAME_LEN];
        assert_eq!(huge.encode(), Err(PayloadError::TooLarge));
    }
    struct Handler;
    impl UsersService for Handler {
        fn get_user(&self, request: Request) -> Result<Response, String> {
            if request.id == 0 {
                return Err("private detail".into());
            }
            Ok(Response {
                id: request.id,
                name: request.name,
            })
        }
    }
    #[tokio::test]
    async fn generated_client_and_dispatch_use_runtime_and_preserve_errors() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            runtime::serve_connection(stream, Duration::from_secs(5), |op, payload| {
                dispatch(&Handler, op, payload)
            })
            .await
            .unwrap();
        });
        let inner = runtime::Client::connect(address, Duration::from_secs(5))
            .await
            .unwrap();
        let mut client = UsersClient::new(inner);
        let mut rejected = request();
        rejected.id = 0;
        assert!(
            matches!(client.get_user(&rejected).await, Err(RpcError::Remote(payload)) if payload == b"handler error")
        );
        for _ in 0..2 {
            assert_eq!(
                client.get_user(&request()).await.unwrap(),
                Response {
                    id: 2,
                    name: "Ada".into()
                }
            );
        }
        drop(client);
        server.await.unwrap();
        assert_eq!(dispatch(&Handler, 99, &[]).0, MessageKind::Error);
        assert_eq!(dispatch(&Handler, 17, &[]).0, MessageKind::Error);
    }
    struct HandoffHandler;
    impl handoff::UsersService for HandoffHandler {
        fn get_user(
            &self,
            request: handoff::GetUserRequest,
        ) -> Result<handoff::GetUserResponse, String> {
            Ok(handoff::GetUserResponse {
                name: request.user_id,
                active: true,
            })
        }
    }

    #[tokio::test]
    async fn reviewed_fastapi_contract_uses_generated_bindings_over_v1_tcp() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            runtime::serve_connection(stream, Duration::from_secs(5), |op, payload| {
                handoff::dispatch(&HandoffHandler, op, payload)
            })
            .await
            .unwrap();
        });
        let inner = runtime::Client::connect(address, Duration::from_secs(5))
            .await
            .unwrap();
        let mut client = handoff::UsersClient::new(inner);
        for user_id in ["Ada", "Grace"] {
            let request = handoff::GetUserRequest {
                user_id: user_id.to_owned(),
            };
            let response = client.get_user(&request).await.unwrap();
            assert_eq!(
                response,
                handoff::GetUserResponse {
                    name: user_id.to_owned(),
                    active: true
                }
            );
        }
        drop(client);
        server.await.unwrap();
    }
}
