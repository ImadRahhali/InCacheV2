use bytes::BytesMut;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};
use tokio::net::TcpListener;

use crate::protocol::{encode_into, parse_all};
use crate::store::Store;
use crate::commands::dispatch;

pub async fn run_server(host: &str, port: u16) {
    let store = Store::new();
    store.start_expiry_sweep();

    let addr = format!("{}:{}", host, port);
    let listener = TcpListener::bind(&addr).await.unwrap();

    loop {
        let (socket, _) = listener.accept().await.unwrap();
        let store = store.clone();
        tokio::spawn(handle_client(socket, store));
    }
}

async fn handle_client(socket: tokio::net::TcpStream, store: Store) {
    let (reader, writer) = socket.into_split();
    let mut reader = reader;
    let mut writer = BufWriter::new(writer);
    let mut buf = BytesMut::with_capacity(65536);
    let mut resp_buf = BytesMut::with_capacity(4096);

    loop {
        match reader.read_buf(&mut buf).await {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => break,
        }

        let commands = parse_all(&mut buf);
        resp_buf.clear();

        for cmd_parts in commands {
            if cmd_parts.is_empty() { continue; }
            let cmd_name = &cmd_parts[0];
            let args = &cmd_parts[1..];
            let result = dispatch(&store, cmd_name, args);
            encode_into(&result, &mut resp_buf);
        }

        if !resp_buf.is_empty() {
            if writer.write_all(&resp_buf).await.is_err() { break; }
            if writer.flush().await.is_err() { break; }
        }
    }
}
