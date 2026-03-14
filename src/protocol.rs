/// RESP2 protocol parser and serialiser — zero-allocation hot path.
use bytes::{Bytes, BytesMut, BufMut};

#[derive(Debug, Clone)]
pub enum RespValue {
    SimpleString(Bytes),
    Error(Bytes),
    Integer(i64),
    BulkString(Bytes),
    Array(Vec<RespValue>),
    Null,
}

impl RespValue {
    #[inline(always)]
    pub fn ok() -> Self { RespValue::SimpleString(Bytes::from_static(b"OK")) }
    #[inline(always)]
    pub fn pong() -> Self { RespValue::SimpleString(Bytes::from_static(b"PONG")) }
    #[inline(always)]
    pub fn error(s: String) -> Self { RespValue::Error(Bytes::from(s)) }
    #[inline(always)]
    pub fn wrongtype() -> Self {
        RespValue::Error(Bytes::from_static(b"WRONGTYPE Operation against a key holding the wrong kind of value"))
    }
    #[inline(always)]
    pub fn bulk(b: Bytes) -> Self { RespValue::BulkString(b) }
    #[inline(always)]
    pub fn bulk_from(b: &[u8]) -> Self { RespValue::BulkString(Bytes::copy_from_slice(b)) }
    #[inline(always)]
    pub fn simple_from_str(s: &str) -> Self { RespValue::SimpleString(Bytes::copy_from_slice(s.as_bytes())) }
}

/// A parsed command: up to 8 args on the stack, spills to heap for more.
/// Each arg is a (start, len) range into the read buffer.
const INLINE_ARGS: usize = 8;

pub struct Command {
    ranges: [(u32, u32); INLINE_ARGS],
    extra: Vec<(u32, u32)>,
    count: usize,
}

impl Command {
    #[inline(always)]
    fn new() -> Self {
        Command {
            ranges: [(0, 0); INLINE_ARGS],
            extra: Vec::new(),
            count: 0,
        }
    }

    #[inline(always)]
    fn push(&mut self, start: usize, len: usize) {
        if self.count < INLINE_ARGS {
            self.ranges[self.count] = (start as u32, len as u32);
        } else {
            self.extra.push((start as u32, len as u32));
        }
        self.count += 1;
    }

    #[inline(always)]
    pub fn argc(&self) -> usize { self.count }

    #[inline(always)]
    pub fn arg<'a>(&self, idx: usize, buf: &'a [u8]) -> &'a [u8] {
        let (start, len) = if idx < INLINE_ARGS {
            self.ranges[idx]
        } else {
            self.extra[idx - INLINE_ARGS]
        };
        &buf[start as usize..start as usize + len as usize]
    }
}

/// Parse all complete commands from buffer. Returns commands + total bytes consumed.
/// Commands reference positions in `buf` — caller must not modify buf until done.
pub fn parse_commands(buf: &[u8]) -> (Vec<Command>, usize) {
    let mut commands = Vec::new();
    let mut pos = 0;

    while pos < buf.len() {
        match buf[pos] {
            b'*' => {
                // RESP array
                let Some(crlf) = find_crlf(&buf[pos..]) else { break };
                let count = parse_int(&buf[pos + 1..pos + crlf]);
                if count < 0 { pos += crlf + 2; continue; }
                let count = count as usize;
                let mut cmd = Command::new();
                let mut p = pos + crlf + 2;
                let mut ok = true;
                for _ in 0..count {
                    if p >= buf.len() || buf[p] != b'$' { ok = false; break; }
                    let Some(crlf2) = find_crlf(&buf[p..]) else { ok = false; break };
                    let len = parse_int(&buf[p + 1..p + crlf2]) as usize;
                    let data_start = p + crlf2 + 2;
                    let data_end = data_start + len + 2;
                    if data_end > buf.len() { ok = false; break; }
                    cmd.push(data_start, len);
                    p = data_end;
                }
                if !ok { break; }
                commands.push(cmd);
                pos = p;
            }
            b'+' | b'-' | b':' | b'$' => {
                // Single value — skip (shouldn't happen in client commands)
                let Some(crlf) = find_crlf(&buf[pos..]) else { break };
                pos += crlf + 2;
            }
            _ => {
                // Inline command
                let Some(crlf) = find_crlf(&buf[pos..]) else { break };
                let line = &buf[pos..pos + crlf];
                let mut cmd = Command::new();
                let mut i = 0;
                while i < line.len() {
                    while i < line.len() && line[i] == b' ' { i += 1; }
                    if i >= line.len() { break; }
                    let start = pos + i;
                    while i < line.len() && line[i] != b' ' { i += 1; }
                    cmd.push(start, pos + i - start);
                }
                if cmd.argc() > 0 { commands.push(cmd); }
                pos += crlf + 2;
            }
        }
    }

    (commands, pos)
}

#[inline(always)]
fn parse_int(buf: &[u8]) -> i64 {
    let mut neg = false;
    let mut n: i64 = 0;
    for &b in buf {
        if b == b'-' { neg = true; }
        else if b >= b'0' && b <= b'9' { n = n * 10 + (b - b'0') as i64; }
    }
    if neg { -n } else { n }
}

/// Encode a RespValue directly into a BytesMut buffer.
#[inline(always)]
pub fn encode_into(value: &RespValue, out: &mut BytesMut) {
    match value {
        RespValue::SimpleString(s) => {
            out.put_u8(b'+');
            out.extend_from_slice(s);
            out.extend_from_slice(b"\r\n");
        }
        RespValue::Error(s) => {
            out.put_u8(b'-');
            out.extend_from_slice(s);
            out.extend_from_slice(b"\r\n");
        }
        RespValue::Integer(n) => {
            out.put_u8(b':');
            out.extend_from_slice(itoa::Buffer::new().format(*n).as_bytes());
            out.extend_from_slice(b"\r\n");
        }
        RespValue::BulkString(b) => {
            out.put_u8(b'$');
            out.extend_from_slice(itoa::Buffer::new().format(b.len()).as_bytes());
            out.extend_from_slice(b"\r\n");
            out.extend_from_slice(b);
            out.extend_from_slice(b"\r\n");
        }
        RespValue::Null => {
            out.extend_from_slice(b"$-1\r\n");
        }
        RespValue::Array(items) => {
            out.put_u8(b'*');
            out.extend_from_slice(itoa::Buffer::new().format(items.len()).as_bytes());
            out.extend_from_slice(b"\r\n");
            for item in items {
                encode_into(item, out);
            }
        }
    }
}

#[inline(always)]
fn find_crlf(buf: &[u8]) -> Option<usize> {
    memchr::memchr(b'\r', buf).filter(|&i| i + 1 < buf.len() && buf[i + 1] == b'\n')
}
