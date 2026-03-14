use bytes::BytesMut;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::protocol::{encode, parse_all};
use crate::store::{new_shared_store, start_expiry_sweep, SharedStore};
use crate::commands::dispatch;

pub async fn run_server(host: &str, port: u16) {
    let store = new_shared_store();
    start_expiry_sweep(store.clone());

    let addr = format!("{}:{}", host, port);
    let listener = TcpListener::bind(&addr).await.unwrap();

    loop {
        let (socket, _) = listener.accept().await.unwrap();
        let store = store.clone();
        tokio::spawn(handle_client(socket, store));
    }
}

async fn handle_client(mut socket: tokio::net::TcpStream, store: SharedStore) {
    let mut buf = BytesMut::with_capacity(65536);
    loop {
        match socket.read_buf(&mut buf).await {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => break,
        }

        let commands = parse_all(&mut buf);
        let mut response = Vec::new();

        for cmd_parts in commands {
            if cmd_parts.is_empty() {
                continue;
            }
            let cmd_name = String::from_utf8_lossy(&cmd_parts[0]).to_uppercase();
            let args = &cmd_parts[1..];

            let mut store = store.lock().await;
            let result = dispatch(&mut store, &cmd_name, args);
            response.extend_from_slice(&encode(&result));
        }

        if !response.is_empty() {
            if socket.write_all(&response).await.is_err() {
                break;
            }
        }
    }
}
