use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream, SocketAddr};
use std::os::fd::{AsRawFd, RawFd};
use std::time::{Duration, Instant};
use std::collections::VecDeque;

use bytes::BytesMut;

use crate::commands::dispatch;
use crate::protocol::{Command, RespValue, encode_into, parse_commands};
use crate::store::Store;

// ── Portable poller ──

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
        pub fn new(cap: usize) -> Self { Events { buf: vec![libc::epoll_event { events: 0, u64: 0 }; cap] } }
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
                ident: raw_fd as usize, filter: libc::EVFILT_READ,
                flags: libc::EV_ADD | libc::EV_CLEAR, fflags: 0, data: 0,
                udata: token as *mut libc::c_void,
            };
            unsafe { libc::kevent(self.fd, &ev, 1, std::ptr::null_mut(), 0, std::ptr::null()); }
        }
        pub fn poll(&self, events: &mut Events, timeout_ms: i32) -> usize {
            let ts = libc::timespec {
                tv_sec: (timeout_ms / 1000) as libc::time_t,
                tv_nsec: ((timeout_ms % 1000) * 1_000_000) as libc::c_long,
            };
            let n = unsafe { libc::kevent(self.fd, std::ptr::null(), 0, events.buf.as_mut_ptr(), events.buf.len() as i32, &ts) };
            if n < 0 { 0 } else { n as usize }
        }
    }
    impl Events {
        pub fn new(cap: usize) -> Self { Events { buf: vec![unsafe { std::mem::zeroed() }; cap] } }
        pub fn token(&self, i: usize) -> u64 { self.buf[i].udata as u64 }
    }
}

use poller::{Poller, Events};

// ── Slowlog ──

pub struct SlowlogEntry {
    pub id: u64,
    pub timestamp: u64,
    pub duration_us: u64,
    pub cmd: String,
}

pub struct Slowlog {
    entries: VecDeque<SlowlogEntry>,
    threshold_us: u64,
    max_len: usize,
    next_id: u64,
}

impl Slowlog {
    fn new() -> Self {
        Slowlog { entries: VecDeque::with_capacity(128), threshold_us: 10_000, max_len: 128, next_id: 0 }
    }
    fn maybe_log(&mut self, duration_us: u64, cmd: String) {
        if duration_us >= self.threshold_us {
            if self.entries.len() >= self.max_len { self.entries.pop_back(); }
            let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
            self.entries.push_front(SlowlogEntry { id: self.next_id, timestamp: ts, duration_us, cmd });
            self.next_id += 1;
        }
    }
    fn get(&self, count: usize) -> Vec<&SlowlogEntry> {
        self.entries.iter().take(count).collect()
    }
    fn len(&self) -> usize { self.entries.len() }
    fn reset(&mut self) { self.entries.clear(); }
}

// ── Connection state ──

struct Conn {
    stream: TcpStream,
    read_buf: BytesMut,
    write_buf: BytesMut,
    authenticated: bool,
    in_multi: bool,
    queue: Vec<(Vec<u8>, Vec<Vec<u8>>)>, // queued (cmd_name, args) for MULTI/EXEC
}

impl Conn {
    fn new(stream: TcpStream, needs_auth: bool) -> Self {
        Conn {
            stream,
            read_buf: BytesMut::with_capacity(32768),
            write_buf: BytesMut::with_capacity(4096),
            authenticated: !needs_auth,
            in_multi: false,
            queue: Vec::new(),
        }
    }
}

// ── Main event loop ──

const LISTENER_TOKEN: u64 = 0;

pub fn run_server(host: &str, port: u16, password: Option<String>) {
    let addr: SocketAddr = format!("{}:{}", host, port).parse().unwrap();
    let listener = TcpListener::bind(addr).unwrap();
    listener.set_nonblocking(true).unwrap();

    let poll = Poller::new();
    let mut events = Events::new(1024);
    poll.add(listener.as_raw_fd(), LISTENER_TOKEN);

    let mut store = Store::new();
    let mut slowlog = Slowlog::new();
    let mut conns: HashMap<u64, Conn> = HashMap::new();
    let mut next_id: u64 = 1;
    let mut last_sweep = Instant::now();
    let needs_auth = password.is_some();

    loop {
        let n = poll.poll(&mut events, 100);

        if last_sweep.elapsed() >= Duration::from_millis(100) {
            store.sweep_expired();
            last_sweep = Instant::now();
        }

        for i in 0..n {
            let token = events.token(i);

            if token == LISTENER_TOKEN {
                loop {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            stream.set_nonblocking(true).unwrap();
                            stream.set_nodelay(true).ok();
                            let id = next_id;
                            next_id += 1;
                            poll.add(stream.as_raw_fd(), id);
                            conns.insert(id, Conn::new(stream, needs_auth));
                        }
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => break,
                        Err(_) => break,
                    }
                }
                continue;
            }

            let closed = if let Some(conn) = conns.get_mut(&token) {
                handle(conn, &mut store, &mut slowlog, &password)
            } else {
                false
            };

            if closed { conns.remove(&token); }
        }
    }
}

#[inline(always)]
fn handle(conn: &mut Conn, store: &mut Store, slowlog: &mut Slowlog, password: &Option<String>) -> bool {
    let mut tmp = [0u8; 65536];
    loop {
        match conn.stream.read(&mut tmp) {
            Ok(0) => return true,
            Ok(n) => conn.read_buf.extend_from_slice(&tmp[..n]),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => break,
            Err(_) => return true,
        }
    }

    let (commands, consumed) = parse_commands(&conn.read_buf);
    if commands.is_empty() { return false; }

    conn.write_buf.clear();
    let buf_ref: &[u8] = &conn.read_buf;

    for cmd in &commands {
        if cmd.argc() == 0 { continue; }
        let cmd_name = cmd.arg(0, buf_ref);
        let mut upper = [0u8; 16];
        let len = cmd_name.len().min(16);
        upper[..len].copy_from_slice(&cmd_name[..len]);
        upper[..len].make_ascii_uppercase();
        let name = &upper[..len];

        // AUTH handling
        if name == b"AUTH" {
            if let Some(pw) = password {
                if cmd.argc() > 1 && cmd.arg(1, buf_ref) == pw.as_bytes() {
                    conn.authenticated = true;
                    encode_into(&RespValue::ok(), &mut conn.write_buf);
                } else {
                    encode_into(&RespValue::error("ERR invalid password".into()), &mut conn.write_buf);
                }
            } else {
                encode_into(&RespValue::error("ERR Client sent AUTH, but no password is set".into()), &mut conn.write_buf);
            }
            continue;
        }

        // Block unauthenticated clients (allow HELLO, PING, QUIT, AUTH, CLIENT)
        if !conn.authenticated && name != b"HELLO" && name != b"PING" && name != b"QUIT" && name != b"CLIENT" {
            encode_into(&RespValue::error("NOAUTH Authentication required.".into()), &mut conn.write_buf);
            continue;
        }

        // MULTI/EXEC/DISCARD
        if name == b"MULTI" {
            conn.in_multi = true;
            conn.queue.clear();
            encode_into(&RespValue::ok(), &mut conn.write_buf);
            continue;
        }
        if name == b"DISCARD" {
            if !conn.in_multi {
                encode_into(&RespValue::error("ERR DISCARD without MULTI".into()), &mut conn.write_buf);
            } else {
                conn.in_multi = false;
                conn.queue.clear();
                encode_into(&RespValue::ok(), &mut conn.write_buf);
            }
            continue;
        }
        if name == b"EXEC" {
            if !conn.in_multi {
                encode_into(&RespValue::error("ERR EXEC without MULTI".into()), &mut conn.write_buf);
            } else {
                conn.in_multi = false;
                let queued: Vec<(Vec<u8>, Vec<Vec<u8>>)> = std::mem::take(&mut conn.queue);
                let mut results = Vec::with_capacity(queued.len());
                for (cmd_bytes, arg_bytes) in &queued {
                    // Build a synthetic command for dispatch
                    let mut full = Vec::with_capacity(1 + arg_bytes.len());
                    full.push(cmd_bytes.clone());
                    full.extend_from_slice(arg_bytes);
                    let flat: Vec<u8> = build_resp_array(&full);
                    let (cmds, _) = parse_commands(&flat);
                    if let Some(c) = cmds.first() {
                        results.push(dispatch(store, c, &flat));
                    } else {
                        results.push(RespValue::error("ERR command parse failed".into()));
                    }
                }
                encode_into(&RespValue::Array(results), &mut conn.write_buf);
            }
            continue;
        }
        if conn.in_multi {
            // Queue the command — store raw bytes since buf_ref will be consumed
            let cmd_bytes = cmd.arg(0, buf_ref).to_vec();
            let mut args = Vec::new();
            for i in 1..cmd.argc() {
                args.push(cmd.arg(i, buf_ref).to_vec());
            }
            conn.queue.push((cmd_bytes, args));
            encode_into(&RespValue::SimpleString(bytes::Bytes::from_static(b"QUEUED")), &mut conn.write_buf);
            continue;
        }

        // SLOWLOG command
        if name == b"SLOWLOG" {
            if cmd.argc() > 1 {
                let sub = cmd.arg(1, buf_ref);
                let mut sub_upper = [0u8; 8];
                let sl = sub.len().min(8);
                sub_upper[..sl].copy_from_slice(&sub[..sl]);
                sub_upper[..sl].make_ascii_uppercase();
                match &sub_upper[..sl] {
                    b"GET" => {
                        let count = if cmd.argc() > 2 {
                            unsafe { std::str::from_utf8_unchecked(cmd.arg(2, buf_ref)) }.parse().unwrap_or(10)
                        } else { 10 };
                        let entries = slowlog.get(count);
                        let items: Vec<RespValue> = entries.iter().map(|e| {
                            RespValue::Array(vec![
                                RespValue::Integer(e.id as i64),
                                RespValue::Integer(e.timestamp as i64),
                                RespValue::Integer(e.duration_us as i64),
                                RespValue::BulkString(bytes::Bytes::from(e.cmd.clone())),
                            ])
                        }).collect();
                        encode_into(&RespValue::Array(items), &mut conn.write_buf);
                    }
                    b"LEN" => {
                        encode_into(&RespValue::Integer(slowlog.len() as i64), &mut conn.write_buf);
                    }
                    b"RESET" => {
                        slowlog.reset();
                        encode_into(&RespValue::ok(), &mut conn.write_buf);
                    }
                    _ => {
                        encode_into(&RespValue::error("ERR unknown SLOWLOG subcommand".into()), &mut conn.write_buf);
                    }
                }
            } else {
                encode_into(&RespValue::error("ERR wrong number of arguments for 'slowlog' command".into()), &mut conn.write_buf);
            }
            continue;
        }

        // Normal command dispatch with timing
        let start = Instant::now();
        let result = dispatch(store, cmd, buf_ref);
        let elapsed_us = start.elapsed().as_micros() as u64;

        // Build command string for slowlog
        if elapsed_us >= slowlog.threshold_us {
            let mut cmd_str = String::from(unsafe { std::str::from_utf8_unchecked(cmd_name) });
            for i in 1..cmd.argc().min(4) {
                cmd_str.push(' ');
                cmd_str.push_str(unsafe { std::str::from_utf8_unchecked(cmd.arg(i, buf_ref)) });
            }
            slowlog.maybe_log(elapsed_us, cmd_str);
        }

        encode_into(&result, &mut conn.write_buf);
    }

    let _ = conn.read_buf.split_to(consumed);

    if !conn.write_buf.is_empty() {
        let mut pos = 0;
        while pos < conn.write_buf.len() {
            match conn.stream.write(&conn.write_buf[pos..]) {
                Ok(n) => pos += n,
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    std::hint::spin_loop();
                    continue;
                }
                Err(_) => return true,
            }
        }
    }

    false
}

/// Build a RESP array from raw byte slices (for MULTI/EXEC replay).
fn build_resp_array(parts: &[Vec<u8>]) -> Vec<u8> {
    let mut out = format!("*{}\r\n", parts.len()).into_bytes();
    for p in parts {
        out.extend_from_slice(format!("${}\r\n", p.len()).as_bytes());
        out.extend_from_slice(p);
        out.extend_from_slice(b"\r\n");
    }
    out
}
