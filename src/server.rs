use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream, SocketAddr};
use std::os::fd::{AsRawFd, RawFd};
use std::time::{Duration, Instant};

use bytes::BytesMut;

use crate::commands::dispatch;
use crate::protocol::{encode_into, parse_commands};
use crate::store::Store;

// ── Portable poller (kqueue on macOS, epoll on Linux) ──

#[cfg(target_os = "linux")]
mod poller {
    use super::*;

    pub struct Poller { fd: RawFd }
    pub struct Events { buf: Vec<libc::epoll_event> }

    impl Poller {
        pub fn new() -> Self {
            let fd = unsafe { libc::epoll_create1(0) };
            assert!(fd >= 0);
            Poller { fd }
        }
        pub fn add(&self, raw_fd: RawFd, token: u64) {
            let mut ev = libc::epoll_event { events: (libc::EPOLLIN | libc::EPOLLET) as u32, u64: token };
            unsafe { libc::epoll_ctl(self.fd, libc::EPOLL_CTL_ADD, raw_fd, &mut ev); }
        }
        pub fn poll(&self, events: &mut Events, timeout_ms: i32) -> usize {
            let n = unsafe { libc::epoll_wait(self.fd, events.buf.as_mut_ptr(), events.buf.len() as i32, timeout_ms) };
            if n < 0 { 0 } else { n as usize }
        }
    }

    impl Events {
        pub fn new(cap: usize) -> Self {
            Events { buf: vec![libc::epoll_event { events: 0, u64: 0 }; cap] }
        }
        pub fn token(&self, i: usize) -> u64 { self.buf[i].u64 }
    }
}

#[cfg(target_os = "macos")]
mod poller {
    use super::*;

    pub struct Poller { fd: RawFd }
    pub struct Events { buf: Vec<libc::kevent> }

    impl Poller {
        pub fn new() -> Self {
            let fd = unsafe { libc::kqueue() };
            assert!(fd >= 0);
            Poller { fd }
        }
        pub fn add(&self, raw_fd: RawFd, token: u64) {
            let ev = libc::kevent {
                ident: raw_fd as usize,
                filter: libc::EVFILT_READ,
                flags: libc::EV_ADD | libc::EV_CLEAR,
                fflags: 0, data: 0,
                udata: token as *mut libc::c_void,
            };
            unsafe { libc::kevent(self.fd, &ev, 1, std::ptr::null_mut(), 0, std::ptr::null()); }
        }
        pub fn poll(&self, events: &mut Events, timeout_ms: i32) -> usize {
            let ts = libc::timespec {
                tv_sec: (timeout_ms / 1000) as libc::time_t,
                tv_nsec: ((timeout_ms % 1000) * 1_000_000) as libc::c_long,
            };
            let n = unsafe {
                libc::kevent(self.fd, std::ptr::null(), 0, events.buf.as_mut_ptr(), events.buf.len() as i32, &ts)
            };
            if n < 0 { 0 } else { n as usize }
        }
    }

    impl Events {
        pub fn new(cap: usize) -> Self {
            Events { buf: vec![unsafe { std::mem::zeroed() }; cap] }
        }
        pub fn token(&self, i: usize) -> u64 { self.buf[i].udata as u64 }
    }
}

use poller::{Poller, Events};

// ── Connection state ──

struct Conn {
    stream: TcpStream,
    read_buf: BytesMut,
    write_buf: BytesMut,
}

impl Conn {
    fn new(stream: TcpStream) -> Self {
        Conn {
            stream,
            read_buf: BytesMut::with_capacity(32768),
            write_buf: BytesMut::with_capacity(4096),
        }
    }
}

// ── Main event loop ──

const LISTENER_TOKEN: u64 = 0;

pub fn run_server(host: &str, port: u16) {
    let addr: SocketAddr = format!("{}:{}", host, port).parse().unwrap();
    let listener = TcpListener::bind(addr).unwrap();
    listener.set_nonblocking(true).unwrap();

    let poll = Poller::new();
    let mut events = Events::new(1024);
    poll.add(listener.as_raw_fd(), LISTENER_TOKEN);

    let mut store = Store::new();
    let mut conns: HashMap<u64, Conn> = HashMap::new();
    let mut next_id: u64 = 1;
    let mut last_sweep = Instant::now();

    loop {
        let n = poll.poll(&mut events, 100);

        // Active expiry sweep
        if last_sweep.elapsed() >= Duration::from_millis(100) {
            store.sweep_expired();
            last_sweep = Instant::now();
        }

        for i in 0..n {
            let token = events.token(i);

            if token == LISTENER_TOKEN {
                // Accept all pending connections
                loop {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            stream.set_nonblocking(true).unwrap();
                            stream.set_nodelay(true).ok();
                            let id = next_id;
                            next_id += 1;
                            poll.add(stream.as_raw_fd(), id);
                            conns.insert(id, Conn::new(stream));
                        }
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => break,
                        Err(_) => break,
                    }
                }
                continue;
            }

            // Handle client I/O
            let closed = if let Some(conn) = conns.get_mut(&token) {
                handle(conn, &mut store)
            } else {
                false
            };

            if closed {
                conns.remove(&token);
            }
        }
    }
}

#[inline(always)]
fn handle(conn: &mut Conn, store: &mut Store) -> bool {
    // Read all available data — no intermediate copy
    let mut tmp = [0u8; 65536];
    loop {
        match conn.stream.read(&mut tmp) {
            Ok(0) => return true,
            Ok(n) => conn.read_buf.extend_from_slice(&tmp[..n]),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => break,
            Err(_) => return true,
        }
    }

    // Parse + execute
    let (commands, consumed) = parse_commands(&conn.read_buf);
    if commands.is_empty() { return false; }

    conn.write_buf.clear();
    let buf_ref: &[u8] = &conn.read_buf;
    for cmd in &commands {
        if cmd.argc() == 0 { continue; }
        let result = dispatch(store, cmd, buf_ref);
        encode_into(&result, &mut conn.write_buf);
    }

    // Advance read buffer past consumed bytes
    let _ = conn.read_buf.split_to(consumed);

    // Write response — tight loop, no async
    if !conn.write_buf.is_empty() {
        let mut pos = 0;
        while pos < conn.write_buf.len() {
            match conn.stream.write(&conn.write_buf[pos..]) {
                Ok(n) => pos += n,
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // Spin briefly for loopback — socket buffer almost never full
                    std::hint::spin_loop();
                    continue;
                }
                Err(_) => return true,
            }
        }
    }

    false
}
