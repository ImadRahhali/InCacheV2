use bytes::Bytes;
use crate::protocol::RespValue;
use crate::store::{Store, Value};

#[inline]
fn to_str(b: &Bytes) -> &str { std::str::from_utf8(b).unwrap_or("") }
#[inline]
fn to_box(b: &Bytes) -> Box<str> { to_str(b).into() }

pub fn cmd_hset(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    match store.with_hash(key, |h| {
        let mut added = 0i64;
        let mut i = 1;
        while i + 1 < args.len() {
            let field = to_box(&args[i]);
            if !h.contains_key(&*field) { added += 1; }
            h.insert(field, args[i+1].clone());
            i += 2;
        }
        added
    }) {
        Ok(n) => RespValue::Integer(n),
        Err(e) => RespValue::error(e.into()),
    }
}

pub fn cmd_hget(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    let field = to_str(&args[1]);
    store.with_entry(key, |e| match &e.value {
        Value::Hash(h) => match h.get(field) {
            Some(v) => RespValue::bulk(v.clone()),
            None => RespValue::Null,
        },
        _ => RespValue::wrongtype(),
    }).unwrap_or(RespValue::Null)
}

pub fn cmd_hmset(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    match store.with_hash(key, |h| {
        let mut i = 1;
        while i + 1 < args.len() {
            h.insert(to_box(&args[i]), args[i+1].clone());
            i += 2;
        }
    }) {
        Ok(()) => RespValue::ok(),
        Err(e) => RespValue::error(e.into()),
    }
}

pub fn cmd_hmget(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    let items: Vec<RespValue> = args[1..].iter().map(|a| {
        let field = to_str(a);
        store.with_entry(key, |e| match &e.value {
            Value::Hash(h) => match h.get(field) {
                Some(v) => RespValue::bulk(v.clone()),
                None => RespValue::Null,
            },
            _ => RespValue::Null,
        }).unwrap_or(RespValue::Null)
    }).collect();
    RespValue::Array(items)
}

pub fn cmd_hgetall(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    store.with_entry(key, |e| match &e.value {
        Value::Hash(h) => {
            let mut items = Vec::with_capacity(h.len() * 2);
            for (field, value) in h {
                items.push(RespValue::BulkString(Bytes::copy_from_slice(field.as_bytes())));
                items.push(RespValue::bulk(value.clone()));
            }
            RespValue::Array(items)
        }
        _ => RespValue::wrongtype(),
    }).unwrap_or(RespValue::Array(vec![]))
}

pub fn cmd_hdel(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    let count = store.with_entry_mut(key, |entry| {
        match &mut entry.value {
            Value::Hash(h) => {
                let mut c = 0i64;
                for a in &args[1..] {
                    if h.remove(to_str(a)).is_some() { c += 1; }
                }
                Ok(c)
            }
            _ => Err(()),
        }
    });
    match count {
        None => RespValue::Integer(0),
        Some(Ok(c)) => { store.remove_if_empty(key); RespValue::Integer(c) }
        Some(Err(())) => RespValue::wrongtype(),
    }
}

pub fn cmd_hexists(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    let field = to_str(&args[1]);
    store.with_entry(key, |e| match &e.value {
        Value::Hash(h) => RespValue::Integer(if h.contains_key(field) { 1 } else { 0 }),
        _ => RespValue::wrongtype(),
    }).unwrap_or(RespValue::Integer(0))
}

pub fn cmd_hlen(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    store.with_entry(key, |e| match &e.value {
        Value::Hash(h) => RespValue::Integer(h.len() as i64),
        _ => RespValue::wrongtype(),
    }).unwrap_or(RespValue::Integer(0))
}

pub fn cmd_hkeys(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    store.with_entry(key, |e| match &e.value {
        Value::Hash(h) => RespValue::Array(h.keys().map(|k| RespValue::BulkString(Bytes::copy_from_slice(k.as_bytes()))).collect()),
        _ => RespValue::wrongtype(),
    }).unwrap_or(RespValue::Array(vec![]))
}

pub fn cmd_hvals(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    store.with_entry(key, |e| match &e.value {
        Value::Hash(h) => RespValue::Array(h.values().map(|v| RespValue::bulk(v.clone())).collect()),
        _ => RespValue::wrongtype(),
    }).unwrap_or(RespValue::Array(vec![]))
}

pub fn cmd_hincrby(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    let field = to_box(&args[1]);
    let increment: i64 = std::str::from_utf8(&args[2]).unwrap_or("0").parse().unwrap_or(0);
    match store.with_hash(key, |h| {
        let current = h.get(&*field).and_then(|v| std::str::from_utf8(v).ok()?.parse::<i64>().ok());
        match h.get(&*field) {
            None => {
                h.insert(field.clone(), Bytes::from(increment.to_string()));
                Ok(increment)
            }
            Some(_) => match current {
                Some(n) => {
                    let new_val = n + increment;
                    h.insert(field.clone(), Bytes::from(new_val.to_string()));
                    Ok(new_val)
                }
                None => Err("ERR hash value is not an integer"),
            },
        }
    }) {
        Ok(Ok(n)) => RespValue::Integer(n),
        Ok(Err(e)) => RespValue::error(e.into()),
        Err(e) => RespValue::error(e.into()),
    }
}
