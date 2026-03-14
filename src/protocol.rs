/// RESP2 protocol parser and serialiser.
use bytes::{Buf, BytesMut};

#[derive(Debug, Clone)]
pub enum RespValue {
    SimpleString(String),
    Error(String),
    Integer(i64),
    BulkString(Vec<u8>),
    Array(Vec<RespValue>),
    Null,
}

/// Try to parse one complete RESP value from buf.
/// Returns Some((value, bytes_consumed)) or None if incomplete.
pub fn parse_one(buf: &[u8]) -> Option<(RespValue, usize)> {
    if buf.is_empty() {
        return None;
    }

    // Inline command (no RESP prefix)
    match buf[0] {
        b'+' | b'-' | b':' | b'$' | b'*' => {}
        _ => {
            let idx = find_crlf(buf)?;
            let line = std::str::from_utf8(&buf[..idx]).ok()?;
            let parts: Vec<RespValue> = line
                .split_whitespace()
                .map(|s| RespValue::BulkString(s.as_bytes().to_vec()))
                .collect();
            return Some((RespValue::Array(parts), idx + 2));
        }
    }

    let idx = find_crlf(buf)?;
    let line = &buf[1..idx];

    match buf[0] {
        b'+' => {
            let s = String::from_utf8_lossy(line).into_owned();
            Some((RespValue::SimpleString(s), idx + 2))
        }
        b'-' => {
            let s = String::from_utf8_lossy(line).into_owned();
            Some((RespValue::Error(s), idx + 2))
        }
        b':' => {
            let n: i64 = std::str::from_utf8(line).ok()?.parse().ok()?;
            Some((RespValue::Integer(n), idx + 2))
        }
        b'$' => {
            let len: i64 = std::str::from_utf8(line).ok()?.parse().ok()?;
            if len == -1 {
                return Some((RespValue::Null, idx + 2));
            }
            let len = len as usize;
            let end = idx + 2 + len + 2;
            if buf.len() < end {
                return None;
            }
            let data = buf[idx + 2..idx + 2 + len].to_vec();
            Some((RespValue::BulkString(data), end))
        }
        b'*' => {
            let count: i64 = std::str::from_utf8(line).ok()?.parse().ok()?;
            if count == -1 {
                return Some((RespValue::Null, idx + 2));
            }
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

/// Parse all complete commands from buffer, returning them and advancing the buffer.
pub fn parse_all(buf: &mut BytesMut) -> Vec<Vec<Vec<u8>>> {
    let mut commands = Vec::new();
    loop {
        match parse_one(buf) {
            Some((RespValue::Array(items), consumed)) => {
                let cmd: Vec<Vec<u8>> = items
                    .into_iter()
                    .map(|v| match v {
                        RespValue::BulkString(b) => b,
                        RespValue::SimpleString(s) => s.into_bytes(),
                        RespValue::Integer(n) => n.to_string().into_bytes(),
                        _ => Vec::new(),
                    })
                    .collect();
                buf.advance(consumed);
                commands.push(cmd);
            }
            _ => break,
        }
    }
    commands
}

/// Encode a RespValue into RESP2 bytes.
pub fn encode(value: &RespValue) -> Vec<u8> {
    match value {
        RespValue::SimpleString(s) => format!("+{}\r\n", s).into_bytes(),
        RespValue::Error(s) => format!("-{}\r\n", s).into_bytes(),
        RespValue::Integer(n) => format!(":{}\r\n", n).into_bytes(),
        RespValue::BulkString(b) => {
            let mut out = format!("${}\r\n", b.len()).into_bytes();
            out.extend_from_slice(b);
            out.extend_from_slice(b"\r\n");
            out
        }
        RespValue::Null => b"$-1\r\n".to_vec(),
        RespValue::Array(items) => {
            let mut out = format!("*{}\r\n", items.len()).into_bytes();
            for item in items {
                out.extend_from_slice(&encode(item));
            }
            out
        }
    }
}

fn find_crlf(buf: &[u8]) -> Option<usize> {
    buf.windows(2).position(|w| w == b"\r\n")
}
