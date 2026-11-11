use morphrpc::{
    protocol::MessageKind,
    runtime::{self, Client},
};
use std::{env, error::Error, time::Duration};
use tokio::net::TcpListener;

fn handle(operation: u16, payload: &[u8]) -> (MessageKind, Vec<u8>) {
    if operation != 1 {
        return (MessageKind::Error, b"unknown operation".to_vec());
    }
    let Ok(bytes) = <[u8; 8]>::try_from(payload) else {
        return (MessageKind::Error, b"invalid user id".to_vec());
    };
    let id = u64::from_be_bytes(bytes);
    let name = match id {
        1 => "Ada",
        2 => "Grace",
        _ => return (MessageKind::Error, b"user not found".to_vec()),
    };
    let mut response = id.to_be_bytes().to_vec();
    response.extend_from_slice(name.as_bytes());
    (MessageKind::Response, response)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let deadline = Duration::from_secs(5);
    match env::args().nth(1).as_deref() {
        Some("server") => {
            let listener = TcpListener::bind("127.0.0.1:8080").await?;
            println!("Listening on 127.0.0.1:8080");
            runtime::serve(listener, deadline, 64, handle).await?;
        }
        Some("client") => {
            let mut client = Client::connect("127.0.0.1:8080", deadline).await?;
            for id in [1u64, 2] {
                let response = client.call(1, id.to_be_bytes().to_vec()).await?;
                if response.kind != MessageKind::Response {
                    return Err(format!(
                        "server error: {}",
                        String::from_utf8_lossy(&response.payload)
                    )
                    .into());
                }
                if response.payload.len() < 8 {
                    return Err("invalid response".into());
                }
                let returned_id = u64::from_be_bytes(response.payload[..8].try_into()?);
                let name = std::str::from_utf8(&response.payload[8..])?;
                println!("user {returned_id}: {name}");
            }
        }
        _ => return Err("usage: cargo run --example get_user -- server|client".into()),
    }
    Ok(())
}
