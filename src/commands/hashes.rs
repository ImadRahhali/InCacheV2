use bytes::Bytes;
use crate::protocol::{Command, RespValue};
use crate::store::{Store, Value};

#[inline(always)]
fn arg_str<'a>(cmd: &Command, i: usize, buf: &'a [u8]) -> &'a str { unsafe { std::str::from_utf8_unchecked(cmd.arg(i, buf)) } }
#[inline(always)]
fn arg_bytes(cmd: &Command, i: usize, buf: &[u8]) -> Bytes { Bytes::copy_from_slice(cmd.arg(i, buf)) }

pub fn cmd_hset(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    let key = arg_str(cmd, 1, buf);
    match store.get_or_create_hash(key) {
        Err(e) => RespValue::error(e.into()),
        Ok(h) => {
            let mut added = 0i64;
            let mut i = 2;
            while i + 1 < cmd.argc() {
                let field: Box<str> = arg_str(cmd, i, buf).into();
                if !h.contains_key(&*field) { added += 1; }
                h.insert(field, arg_bytes(cmd, i+1, buf));
                i += 2;
            }
            RespValue::Integer(added)
        }
    }
}

pub fn cmd_hget(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    let field = arg_str(cmd, 2, buf);
    match store.get_entry(arg_str(cmd, 1, buf)) {
        None => RespValue::Null,
        Some(e) => match &e.value {
            Value::Hash(h) => h.get(field).map_or(RespValue::Null, |v| RespValue::bulk(v.clone())),
            _ => RespValue::wrongtype(),
        },
    }
}

pub fn cmd_hmset(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    let key = arg_str(cmd, 1, buf);
    match store.get_or_create_hash(key) {
        Err(e) => RespValue::error(e.into()),
        Ok(h) => {
            let mut i = 2;
            while i + 1 < cmd.argc() { h.insert(arg_str(cmd, i, buf).into(), arg_bytes(cmd, i+1, buf)); i += 2; }
            RespValue::ok()
        }
    }
}

pub fn cmd_hmget(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    let key = arg_str(cmd, 1, buf);
    let mut items = Vec::with_capacity(cmd.argc() - 2);
    for i in 2..cmd.argc() {
        items.push(match store.get_entry(key) {
            Some(e) => match &e.value {
                Value::Hash(h) => h.get(arg_str(cmd, i, buf)).map_or(RespValue::Null, |v| RespValue::bulk(v.clone())),
                _ => RespValue::Null,
            },
            None => RespValue::Null,
        });
    }
    RespValue::Array(items)
}

pub fn cmd_hgetall(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    match store.get_entry(arg_str(cmd, 1, buf)) {
        None => RespValue::Array(vec![]),
        Some(e) => match &e.value {
            Value::Hash(h) => {
                let mut items = Vec::with_capacity(h.len() * 2);
                for (f, v) in h { items.push(RespValue::bulk_from(f.as_bytes())); items.push(RespValue::bulk(v.clone())); }
                RespValue::Array(items)
            }
            _ => RespValue::wrongtype(),
        },
    }
}

pub fn cmd_hdel(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    let key = arg_str(cmd, 1, buf);
    let c = match store.get_entry_mut(key) {
        None => return RespValue::Integer(0),
        Some(e) => match &mut e.value {
            Value::Hash(h) => { let mut c = 0i64; for i in 2..cmd.argc() { if h.remove(arg_str(cmd, i, buf)).is_some() { c += 1; } } c }
            _ => return RespValue::wrongtype(),
        },
    };
    store.remove_if_empty(key);
    RespValue::Integer(c)
}

pub fn cmd_hexists(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    match store.get_entry(arg_str(cmd, 1, buf)) {
        None => RespValue::Integer(0),
        Some(e) => match &e.value {
            Value::Hash(h) => RespValue::Integer(if h.contains_key(arg_str(cmd, 2, buf)) { 1 } else { 0 }),
            _ => RespValue::wrongtype(),
        },
    }
}

pub fn cmd_hlen(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    match store.get_entry(arg_str(cmd, 1, buf)) {
        None => RespValue::Integer(0),
        Some(e) => match &e.value { Value::Hash(h) => RespValue::Integer(h.len() as i64), _ => RespValue::wrongtype() },
    }
}

pub fn cmd_hkeys(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    match store.get_entry(arg_str(cmd, 1, buf)) {
        None => RespValue::Array(vec![]),
        Some(e) => match &e.value {
            Value::Hash(h) => RespValue::Array(h.keys().map(|k| RespValue::bulk_from(k.as_bytes())).collect()),
            _ => RespValue::wrongtype(),
        },
    }
}

pub fn cmd_hvals(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    match store.get_entry(arg_str(cmd, 1, buf)) {
        None => RespValue::Array(vec![]),
        Some(e) => match &e.value {
            Value::Hash(h) => RespValue::Array(h.values().map(|v| RespValue::bulk(v.clone())).collect()),
            _ => RespValue::wrongtype(),
        },
    }
}

pub fn cmd_hincrby(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    let key = arg_str(cmd, 1, buf);
    let field: Box<str> = arg_str(cmd, 2, buf).into();
    let inc: i64 = arg_str(cmd, 3, buf).parse().unwrap_or(0);
    match store.get_or_create_hash(key) {
        Err(e) => RespValue::error(e.into()),
        Ok(h) => {
            let cur = h.get(&*field).and_then(|v| std::str::from_utf8(v).ok()?.parse::<i64>().ok());
            match h.get(&*field) {
                None => { h.insert(field, Bytes::from(inc.to_string())); RespValue::Integer(inc) }
                Some(_) => match cur {
                    Some(n) => { let r = n + inc; h.insert(field, Bytes::from(r.to_string())); RespValue::Integer(r) }
                    None => RespValue::error("ERR hash value is not an integer".into()),
                },
            }
        }
    }
}
