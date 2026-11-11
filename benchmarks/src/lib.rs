use axum::{extract::Json, routing::post, Router};
use morphrpc::{protocol::MessageKind, runtime};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, time::Duration};
use tokio::net::TcpListener;
use tonic::{Request, Response, Status};

#[allow(clippy::result_large_err)]
pub mod proto {
    tonic::include_proto!("bench");
}
use proto::{
    users_server::{Users, UsersServer},
    GetUserRequest, User,
};

pub const MAX_DATA: usize = 900_000;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Query {
    pub id: u64,
    pub payload_size: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserData {
    pub id: u64,
    pub name: String,
    pub data: String,
}

pub fn get_user(query: &Query) -> Result<UserData, &'static str> {
    if query.payload_size as usize > MAX_DATA {
        return Err("payload too large");
    }
    let name = match query.id {
        1 => "Ada",
        2 => "Grace",
        _ => return Err("user not found"),
    };
    Ok(UserData {
        id: query.id,
        name: name.to_owned(),
        data: "x".repeat(query.payload_size as usize),
    })
}

pub fn morph_request(query: &Query) -> Vec<u8> {
    let mut data = query.id.to_be_bytes().to_vec();
    data.extend_from_slice(&query.payload_size.to_be_bytes());
    data
}
pub fn morph_response(data: &[u8]) -> Result<UserData, &'static str> {
    if data.len() < 9 {
        return Err("invalid response");
    }
    let id = u64::from_be_bytes(data[..8].try_into().unwrap());
    let name_len = data[8] as usize;
    if data.len() < 9 + name_len {
        return Err("invalid response");
    }
    let name = std::str::from_utf8(&data[9..9 + name_len])
        .map_err(|_| "invalid name")?
        .to_owned();
    let data = std::str::from_utf8(&data[9 + name_len..])
        .map_err(|_| "invalid data")?
        .to_owned();
    Ok(UserData { id, name, data })
}
pub fn morph_handler(operation: u16, payload: &[u8]) -> (MessageKind, Vec<u8>) {
    if operation != 1 {
        return (MessageKind::Error, b"unknown operation".to_vec());
    }
    if payload.len() != 12 {
        return (MessageKind::Error, b"invalid request".to_vec());
    }
    let query = Query {
        id: u64::from_be_bytes(payload[..8].try_into().unwrap()),
        payload_size: u32::from_be_bytes(payload[8..].try_into().unwrap()),
    };
    match get_user(&query) {
        Ok(user) => {
            let mut result = user.id.to_be_bytes().to_vec();
            result.push(user.name.len() as u8);
            result.extend_from_slice(user.name.as_bytes());
            result.extend_from_slice(user.data.as_bytes());
            (MessageKind::Response, result)
        }
        Err(e) => (MessageKind::Error, e.as_bytes().to_vec()),
    }
}

async fn http_get(
    Json(query): Json<Query>,
) -> Result<Json<UserData>, (axum::http::StatusCode, &'static str)> {
    get_user(&query)
        .map(Json)
        .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e))
}
#[derive(Default)]
struct GrpcUsers;
#[tonic::async_trait]
impl Users for GrpcUsers {
    async fn get_user(&self, request: Request<GetUserRequest>) -> Result<Response<User>, Status> {
        let request = request.into_inner();
        let user = get_user(&Query {
            id: request.id,
            payload_size: request.payload_size,
        })
        .map_err(Status::invalid_argument)?;
        Ok(Response::new(User {
            id: user.id,
            name: user.name,
            data: user.data,
        }))
    }
}

pub async fn serve(
    protocol: &str,
    address: SocketAddr,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match protocol {
        "morphrpc" => {
            let listener = TcpListener::bind(address).await?;
            runtime::serve(listener, Duration::from_secs(30), 4096, morph_handler).await?;
        }
        "http" => {
            let listener = TcpListener::bind(address).await?;
            axum::serve(listener, Router::new().route("/get_user", post(http_get))).await?;
        }
        "grpc" => {
            tonic::transport::Server::builder()
                .tcp_nodelay(false)
                .add_service(UsersServer::new(GrpcUsers))
                .serve(address)
                .await?;
        }
        _ => return Err("unknown protocol".into()),
    }
    Ok(())
}
