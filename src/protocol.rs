/// RESP2 protocol parser and serialiser.
use bytes::{Buf, Bytes, BytesMut, BufMut};

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
    #[inline]
    pub fn ok() -> Self { RespValue::SimpleString(Bytes::from_static(b"OK")) }
    #[inline]
    pub fn pong() -> Self { RespValue::SimpleString(Bytes::from_static(b"PONG")) }
    #[inline]
    pub fn simple(s: &'static str) -> Self { RespValue::SimpleString(Bytes::from_static(s.as_bytes())) }
    #[inline]
    pub fn error(s: String) -> Self { RespValue::Error(Bytes::from(s)) }
    #[inline]
    pub fn wrongtype() -> Self {
        RespValue::Error(Bytes::from_static(b"WRONGTYPE Operation against a key holding the wrong kind of value"))
    }
    #[inline]
    pub fn bulk(b: Bytes) -> Self { RespValue::BulkString(b) }
    #[inline]
    pub fn bulk_from(b: &[u8]) -> Self { RespValue::BulkString(Bytes::copy_from_slice(b)) }
    #[inline]
    pub fn simple_from_str(s: &str) -> Self { RespValue::SimpleString(Bytes::copy_from_slice(s.as_bytes())) }
}

/// Try to parse one complete RESP value from buf.
pub fn parse_one(buf: &[u8]) -> Option<(RespValue, usize)> {
    if buf.is_empty() { return None; }

    match buf[0] {
        b'+' | b'-' | b':' | b'$' | b'*' => {}
        _ => {
            // Inline command
            let idx = find_crlf(buf)?;
            let line = std::str::from_utf8(&buf[..idx]).ok()?;
            let parts: Vec<RespValue> = line.split_whitespace()
                .map(|s| RespValue::BulkString(Bytes::copy_from_slice(s.as_bytes())))
                .collect();
            return Some((RespValue::Array(parts), idx + 2));
        }
    }

    let idx = find_crlf(buf)?;
    let line = &buf[1..idx];

    match buf[0] {
        b'+' => Some((RespValue::SimpleString(Bytes::copy_from_slice(line)), idx + 2)),
        b'-' => Some((RespValue::Error(Bytes::copy_from_slice(line)), idx + 2)),
        b':' => {
            let n: i64 = std::str::from_utf8(line).ok()?.parse().ok()?;
            Some((RespValue::Integer(n), idx + 2))
        }
        b'$' => {
            let len: i64 = std::str::from_utf8(line).ok()?.parse().ok()?;
            if len == -1 { return Some((RespValue::Null, idx + 2)); }
            let len = len as usize;
            let end = idx + 2 + len + 2;
            if buf.len() < end { return None; }
            Some((RespValue::BulkString(Bytes::copy_from_slice(&buf[idx + 2..idx + 2 + len])), end))
        }
        b'*' => {
            let count: i64 = std::str::from_utf8(line).ok()?.parse().ok()?;
            if count == -1 { return Some((RespValue::Null, idx + 2)); }
            let count = count as usize;
            let mut pos = idx + 2;
            let mut items = Vec::with_capacity(count);
            for _ in 0..count {
                let (item, consumed) = parse_one(&buf[pos..])?;
                items.push(item);
                pos += consumed;
            }
            Some((RespValue::Array(items), pos))
        }
        _ => None,
    }
}

/// Parse all complete commands from buffer.
pub fn parse_all(buf: &mut BytesMut) -> Vec<Vec<Bytes>> {
    let mut commands = Vec::new();
    loop {
        match parse_one(buf) {
            Some((RespValue::Array(items), consumed)) => {
                let cmd: Vec<Bytes> = items.into_iter().map(|v| match v {
                    RespValue::BulkString(b) => b,
                    RespValue::SimpleString(s) => s,
                    RespValue::Integer(n) => Bytes::from(n.to_string()),
                    _ => Bytes::new(),
                }).collect();
                buf.advance(consumed);
                commands.push(cmd);
            }
            _ => break,
        }
    }
    commands
}

/// Encode a RespValue directly into a BytesMut buffer.
#[inline]
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

#[inline]
fn find_crlf(buf: &[u8]) -> Option<usize> {
    memchr::memchr(b'\r', buf).filter(|&i| i + 1 < buf.len() && buf[i + 1] == b'\n')
}
