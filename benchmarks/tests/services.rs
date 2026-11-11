use morphrpc::{protocol::MessageKind, runtime::Client};
use morphrpc_bench::{
    morph_request, morph_response,
    proto::{users_client::UsersClient, GetUserRequest},
    Query, UserData,
};
use std::time::Duration;
use tokio::{
    net::TcpStream,
    time::{sleep, timeout},
};

#[tokio::test]
async fn all_services_return_equivalent_responses_on_persistent_connections() {
    for protocol in ["morphrpc", "http", "grpc"] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let server = tokio::spawn(async move { morphrpc_bench::serve(protocol, address).await });
        timeout(Duration::from_secs(3), async {
            while TcpStream::connect(address).await.is_err() {
                sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        let mut morph = if protocol == "morphrpc" {
            Some(
                Client::connect(address, Duration::from_secs(3))
                    .await
                    .unwrap(),
            )
        } else {
            None
        };
        let http = reqwest::Client::builder().no_proxy().build().unwrap();
        let mut grpc = if protocol == "grpc" {
            Some(
                UsersClient::connect(format!("http://{address}"))
                    .await
                    .unwrap(),
            )
        } else {
            None
        };
        for size in [0, 1024, 65536, 900000] {
            for id in [1, 2] {
                let query = Query {
                    id,
                    payload_size: size,
                };
                let expected = UserData {
                    id,
                    name: if id == 1 { "Ada" } else { "Grace" }.to_owned(),
                    data: "x".repeat(size as usize),
                };
                let actual = match protocol {
                    "morphrpc" => {
                        let response = morph
                            .as_mut()
                            .unwrap()
                            .call(1, morph_request(&query))
                            .await
                            .unwrap();
                        assert_eq!(response.kind, MessageKind::Response);
                        morph_response(&response.payload).unwrap()
                    }
                    "http" => http
                        .post(format!("http://{address}/get_user"))
                        .json(&query)
                        .send()
                        .await
                        .unwrap()
                        .error_for_status()
                        .unwrap()
                        .json::<UserData>()
                        .await
                        .unwrap(),
                    "grpc" => {
                        let user = grpc
                            .as_mut()
                            .unwrap()
                            .get_user(GetUserRequest {
                                id,
                                payload_size: size,
                            })
                            .await
                            .unwrap()
                            .into_inner();
                        UserData {
                            id: user.id,
                            name: user.name,
                            data: user.data,
                        }
                    }
                    _ => unreachable!(),
                };
                assert_eq!(actual, expected);
            }
        }
        let invalid = Query {
            id: 99,
            payload_size: 0,
        };
        match protocol {
            "morphrpc" => {
                let response = morph
                    .as_mut()
                    .unwrap()
                    .call(1, morph_request(&invalid))
                    .await
                    .unwrap();
                assert_eq!(response.kind, MessageKind::Error);
                assert_eq!(response.payload, b"user not found");
            }
            "http" => {
                let response = http
                    .post(format!("http://{address}/get_user"))
                    .json(&invalid)
                    .send()
                    .await
                    .unwrap();
                assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
                assert_eq!(response.text().await.unwrap(), "user not found");
            }
            "grpc" => {
                let status = grpc
                    .as_mut()
                    .unwrap()
                    .get_user(GetUserRequest {
                        id: 99,
                        payload_size: 0,
                    })
                    .await
                    .unwrap_err();
                assert_eq!(status.code(), tonic::Code::InvalidArgument);
                assert_eq!(status.message(), "user not found");
            }
            _ => unreachable!(),
        }
        let oversized = Query {
            id: 1,
            payload_size: 900001,
        };
        match protocol {
            "morphrpc" => {
                let response = morph
                    .as_mut()
                    .unwrap()
                    .call(1, morph_request(&oversized))
                    .await
                    .unwrap();
                assert_eq!(response.kind, MessageKind::Error);
                assert_eq!(response.payload, b"payload too large");
            }
            "http" => {
                let response = http
                    .post(format!("http://{address}/get_user"))
                    .json(&oversized)
                    .send()
                    .await
                    .unwrap();
                assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
                assert_eq!(response.text().await.unwrap(), "payload too large");
            }
            "grpc" => {
                let status = grpc
                    .as_mut()
                    .unwrap()
                    .get_user(GetUserRequest {
                        id: 1,
                        payload_size: 900001,
                    })
                    .await
                    .unwrap_err();
                assert_eq!(status.code(), tonic::Code::InvalidArgument);
                assert_eq!(status.message(), "payload too large");
            }
            _ => unreachable!(),
        }
        server.abort();
    }
}
