use std::cell::RefCell;
use std::rc::Rc;
use bytes::BytesMut;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};
use tokio::net::TcpListener;
use tokio::time::{interval, Duration};

use crate::protocol::{encode_into, parse_all};
use crate::store::Store;
use crate::commands::dispatch;

type SharedStore = Rc<RefCell<Store>>;

pub async fn run_server(host: &str, port: u16) {
    let store: SharedStore = Rc::new(RefCell::new(Store::new()));

    // Expiry sweep — runs on the same single thread
    let sweep_store = store.clone();
    tokio::task::spawn_local(async move {
        let mut tick = interval(Duration::from_millis(100));
        loop {
            tick.tick().await;
            sweep_store.borrow_mut().sweep_expired();
        }
    });

    let addr = format!("{}:{}", host, port);
    let listener = TcpListener::bind(&addr).await.unwrap();

    loop {
        let (socket, _) = listener.accept().await.unwrap();
        socket.set_nodelay(true).ok();
        let store = store.clone();
        tokio::task::spawn_local(handle_client(socket, store));
    }
}

async fn handle_client(socket: tokio::net::TcpStream, store: SharedStore) {
    let (reader, writer) = socket.into_split();
    let mut reader = reader;
    let mut writer = BufWriter::with_capacity(65536, writer);
    let mut buf = BytesMut::with_capacity(65536);
    let mut resp_buf = BytesMut::with_capacity(8192);

    loop {
        match reader.read_buf(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }

        let commands = parse_all(&mut buf);
        if commands.is_empty() { continue; }

        resp_buf.clear();
        {
            let mut s = store.borrow_mut();
            for cmd_parts in &commands {
                if cmd_parts.is_empty() { continue; }
                let result = dispatch(&mut s, &cmd_parts[0], &cmd_parts[1..]);
                encode_into(&result, &mut resp_buf);
            }
        }

        if !resp_buf.is_empty() {
            if writer.write_all(&resp_buf).await.is_err() { break; }
            if writer.flush().await.is_err() { break; }
        }
    }
}
