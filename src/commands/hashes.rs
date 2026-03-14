use bytes::Bytes;
use crate::protocol::RespValue;
use crate::store::{Store, Value};

#[inline(always)]
fn to_str(b: &Bytes) -> &str { unsafe { std::str::from_utf8_unchecked(b) } }

pub fn cmd_hset(store: &mut Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    match store.get_or_create_hash(key) {
        Err(e) => RespValue::error(e.into()),
        Ok(h) => {
            let mut added = 0i64;
            let mut i = 1;
            while i + 1 < args.len() {
                let field: Box<str> = to_str(&args[i]).into();
                if !h.contains_key(&*field) { added += 1; }
                h.insert(field, args[i+1].clone());
                i += 2;
            }
            RespValue::Integer(added)
        }
    }
}

pub fn cmd_hget(store: &mut Store, args: &[Bytes]) -> RespValue {
    let field = to_str(&args[1]);
    match store.get_entry(to_str(&args[0])) {
        None => RespValue::Null,
        Some(e) => match &e.value {
            Value::Hash(h) => h.get(field).map_or(RespValue::Null, |v| RespValue::bulk(v.clone())),
            _ => RespValue::wrongtype(),
        },
    }
}

pub fn cmd_hmset(store: &mut Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    match store.get_or_create_hash(key) {
        Err(e) => RespValue::error(e.into()),
        Ok(h) => {
            let mut i = 1;
            while i + 1 < args.len() { h.insert(to_str(&args[i]).into(), args[i+1].clone()); i += 2; }
            RespValue::ok()
        }
    }
}

pub fn cmd_hmget(store: &mut Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    let items: Vec<RespValue> = args[1..].iter().map(|a| {
        match store.get_entry(key) {
            Some(e) => match &e.value {
                Value::Hash(h) => h.get(to_str(a)).map_or(RespValue::Null, |v| RespValue::bulk(v.clone())),
                _ => RespValue::Null,
            },
            None => RespValue::Null,
        }
    }).collect();
    RespValue::Array(items)
}

pub fn cmd_hgetall(store: &mut Store, args: &[Bytes]) -> RespValue {
    match store.get_entry(to_str(&args[0])) {
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

pub fn cmd_hdel(store: &mut Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    let c = match store.get_entry_mut(key) {
        None => return RespValue::Integer(0),
        Some(e) => match &mut e.value {
            Value::Hash(h) => { let mut c = 0i64; for a in &args[1..] { if h.remove(to_str(a)).is_some() { c += 1; } } c }
            _ => return RespValue::wrongtype(),
        },
    };
    store.remove_if_empty(key);
    RespValue::Integer(c)
}

pub fn cmd_hexists(store: &mut Store, args: &[Bytes]) -> RespValue {
    match store.get_entry(to_str(&args[0])) {
        None => RespValue::Integer(0),
        Some(e) => match &e.value {
            Value::Hash(h) => RespValue::Integer(if h.contains_key(to_str(&args[1])) { 1 } else { 0 }),
            _ => RespValue::wrongtype(),
        },
    }
}

pub fn cmd_hlen(store: &mut Store, args: &[Bytes]) -> RespValue {
    match store.get_entry(to_str(&args[0])) {
        None => RespValue::Integer(0),
        Some(e) => match &e.value { Value::Hash(h) => RespValue::Integer(h.len() as i64), _ => RespValue::wrongtype() },
    }
}

pub fn cmd_hkeys(store: &mut Store, args: &[Bytes]) -> RespValue {
    match store.get_entry(to_str(&args[0])) {
        None => RespValue::Array(vec![]),
        Some(e) => match &e.value {
            Value::Hash(h) => RespValue::Array(h.keys().map(|k| RespValue::bulk_from(k.as_bytes())).collect()),
            _ => RespValue::wrongtype(),
        },
    }
}

pub fn cmd_hvals(store: &mut Store, args: &[Bytes]) -> RespValue {
    match store.get_entry(to_str(&args[0])) {
        None => RespValue::Array(vec![]),
        Some(e) => match &e.value {
            Value::Hash(h) => RespValue::Array(h.values().map(|v| RespValue::bulk(v.clone())).collect()),
            _ => RespValue::wrongtype(),
        },
    }
}

pub fn cmd_hincrby(store: &mut Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    let field: Box<str> = to_str(&args[1]).into();
    let inc: i64 = to_str(&args[2]).parse().unwrap_or(0);
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
