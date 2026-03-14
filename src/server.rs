use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use bytes::BytesMut;
use mio::net::{TcpListener, TcpStream};
use mio::{Events, Interest, Poll, Token};

use crate::commands::dispatch;
use crate::protocol::{encode_into, parse_all};
use crate::store::Store;

const LISTENER: Token = Token(0);

struct Connection {
    socket: TcpStream,
    read_buf: BytesMut,
    write_buf: BytesMut,
    tmp: [u8; 65536],
}

impl Connection {
    fn new(socket: TcpStream) -> Self {
        Connection {
            socket,
            read_buf: BytesMut::with_capacity(65536),
            write_buf: BytesMut::with_capacity(4096),
            tmp: [0u8; 65536],
        }
    }
}

pub fn run_server(host: &str, port: u16) {
    let addr: SocketAddr = format!("{}:{}", host, port).parse().unwrap();
    let mut poll = Poll::new().unwrap();
    let mut events = Events::with_capacity(1024);
    let mut listener = TcpListener::bind(addr).unwrap();
    poll.registry()
        .register(&mut listener, LISTENER, Interest::READABLE)
        .unwrap();

    let mut store = Store::new();
    let mut connections: HashMap<usize, Connection> = HashMap::with_capacity(256);
    let mut next_token: usize = 1;
    let mut last_sweep = Instant::now();

    loop {
        poll.poll(&mut events, Some(Duration::from_millis(100))).unwrap();

        if last_sweep.elapsed() >= Duration::from_millis(100) {
            store.sweep_expired();
            last_sweep = Instant::now();
        }

        for event in events.iter() {
            match event.token() {
                LISTENER => loop {
                    match listener.accept() {
                        Ok((mut socket, _)) => {
                            let id = next_token;
                            next_token += 1;
                            socket.set_nodelay(true).ok();
                            poll.registry()
                                .register(&mut socket, Token(id), Interest::READABLE)
                                .unwrap();
                            connections.insert(id, Connection::new(socket));
                        }
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => break,
                        Err(_) => break,
                    }
                },
                token => {
                    let id = token.0;
                    let closed = if let Some(conn) = connections.get_mut(&id) {
                        handle(conn, &mut store)
                    } else {
                        false
                    };
                    if closed {
                        if let Some(mut conn) = connections.remove(&id) {
                            poll.registry().deregister(&mut conn.socket).ok();
                        }
                    }
                }
            }
        }
    }
}

#[inline(always)]
fn handle(conn: &mut Connection, store: &mut Store) -> bool {
    // Read all available data
    loop {
        match conn.socket.read(&mut conn.tmp) {
            Ok(0) => return true,
            Ok(n) => conn.read_buf.extend_from_slice(&conn.tmp[..n]),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => break,
            Err(_) => return true,
        }
    }

    // Parse and execute
    let commands = parse_all(&mut conn.read_buf);
    if commands.is_empty() {
        return false;
    }

    conn.write_buf.clear();
    for cmd_parts in &commands {
        if cmd_parts.is_empty() { continue; }
        let result = dispatch(store, &cmd_parts[0], &cmd_parts[1..]);
        encode_into(&result, &mut conn.write_buf);
    }

    // Write response
    if !conn.write_buf.is_empty() {
        let mut pos = 0;
        while pos < conn.write_buf.len() {
            match conn.socket.write(&conn.write_buf[pos..]) {
                Ok(n) => pos += n,
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(_) => return true,
            }
        }
    }

    false
}
