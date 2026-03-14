use bytes::Bytes;
use crate::protocol::RespValue;
use crate::store::{Store, Value};

#[inline]
fn to_str(b: &Bytes) -> &str { std::str::from_utf8(b).unwrap_or("") }

#[inline]
fn parse_i64(b: &Bytes) -> i64 { std::str::from_utf8(b).unwrap_or("0").parse().unwrap_or(0) }

pub fn cmd_lpush(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    match store.with_list(key, |d| {
        for v in &args[1..] { d.push_front(v.clone()); }
        d.len() as i64
    }) {
        Ok(n) => RespValue::Integer(n),
        Err(e) => RespValue::error(e.into()),
    }
}

pub fn cmd_rpush(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    match store.with_list(key, |d| {
        for v in &args[1..] { d.push_back(v.clone()); }
        d.len() as i64
    }) {
        Ok(n) => RespValue::Integer(n),
        Err(e) => RespValue::error(e.into()),
    }
}

pub fn cmd_lpop(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    let val = store.with_entry_mut(key, |entry| {
        match &mut entry.value {
            Value::List(d) => Ok(d.pop_front()),
            _ => Err(()),
        }
    });
    match val {
        None => RespValue::Null,
        Some(Err(())) => RespValue::wrongtype(),
        Some(Ok(None)) => RespValue::Null,
        Some(Ok(Some(v))) => { store.remove_if_empty(key); RespValue::bulk(v) }
    }
}

pub fn cmd_rpop(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    let val = store.with_entry_mut(key, |entry| {
        match &mut entry.value {
            Value::List(d) => Ok(d.pop_back()),
            _ => Err(()),
        }
    });
    match val {
        None => RespValue::Null,
        Some(Err(())) => RespValue::wrongtype(),
        Some(Ok(None)) => RespValue::Null,
        Some(Ok(Some(v))) => { store.remove_if_empty(key); RespValue::bulk(v) }
    }
}

pub fn cmd_lrange(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    let start = parse_i64(&args[1]);
    let stop = parse_i64(&args[2]);
    store.with_entry(key, |e| {
        match &e.value {
            Value::List(d) => {
                let len = d.len() as i64;
                let s = if start < 0 { (len + start).max(0) } else { start } as usize;
                let e = if stop < 0 { len + stop } else { stop } as usize;
                if s > e || s >= d.len() { return RespValue::Array(vec![]); }
                let e = e.min(d.len() - 1);
                let items: Vec<RespValue> = d.iter().skip(s).take(e - s + 1)
                    .map(|v| RespValue::bulk(v.clone())).collect();
                RespValue::Array(items)
            }
            _ => RespValue::wrongtype(),
        }
    }).unwrap_or(RespValue::Array(vec![]))
}

pub fn cmd_llen(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    store.with_entry(key, |e| match &e.value {
        Value::List(d) => RespValue::Integer(d.len() as i64),
        _ => RespValue::wrongtype(),
    }).unwrap_or(RespValue::Integer(0))
}

pub fn cmd_lindex(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    let mut index = parse_i64(&args[1]);
    store.with_entry(key, |e| match &e.value {
        Value::List(d) => {
            if index < 0 { index += d.len() as i64; }
            if index < 0 || index as usize >= d.len() { return RespValue::Null; }
            RespValue::bulk(d[index as usize].clone())
        }
        _ => RespValue::wrongtype(),
    }).unwrap_or(RespValue::Null)
}

pub fn cmd_lset(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    let mut index = parse_i64(&args[1]);
    let value = args[2].clone();
    match store.with_entry_mut(key, |entry| {
        match &mut entry.value {
            Value::List(d) => {
                if index < 0 { index += d.len() as i64; }
                if index < 0 || index as usize >= d.len() {
                    return Err("ERR index out of range");
                }
                d[index as usize] = value;
                Ok(())
            }
            _ => Err("WRONGTYPE Operation against a key holding the wrong kind of value"),
        }
    }) {
        None => RespValue::error("ERR no such key".into()),
        Some(Ok(())) => RespValue::ok(),
        Some(Err(e)) => RespValue::error(e.into()),
    }
}

pub fn cmd_linsert(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    let mut where_buf = args[1].to_vec();
    where_buf.make_ascii_uppercase();
    let before = where_buf == b"BEFORE";
    let pivot = &args[2];
    let value = args[3].clone();
    match store.with_entry_mut(key, |entry| {
        match &mut entry.value {
            Value::List(d) => {
                for i in 0..d.len() {
                    if &d[i] == pivot {
                        let pos = if before { i } else { i + 1 };
                        d.insert(pos, value);
                        return Ok(d.len() as i64);
                    }
                }
                Ok(-1)
            }
            _ => Err(()),
        }
    }) {
        None => RespValue::Integer(0),
        Some(Ok(n)) => RespValue::Integer(n),
        Some(Err(())) => RespValue::wrongtype(),
    }
}

pub fn cmd_lrem(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    let count = parse_i64(&args[1]);
    let value = &args[2];
    let removed = store.with_entry_mut(key, |entry| {
        match &mut entry.value {
            Value::List(d) => {
                let mut removed = 0i64;
                if count > 0 {
                    let mut i = 0;
                    while i < d.len() {
                        if &d[i] == value && removed < count { d.remove(i); removed += 1; }
                        else { i += 1; }
                    }
                } else if count < 0 {
                    let limit = count.unsigned_abs() as i64;
                    let mut i = d.len();
                    while i > 0 {
                        i -= 1;
                        if &d[i] == value && removed < limit { d.remove(i); removed += 1; }
                    }
                } else {
                    d.retain(|v| if v == value { removed += 1; false } else { true });
                }
                Ok(removed)
            }
            _ => Err(()),
        }
    });
    match removed {
        None => RespValue::Integer(0),
        Some(Ok(n)) => { store.remove_if_empty(key); RespValue::Integer(n) }
        Some(Err(())) => RespValue::wrongtype(),
    }
}
