use crate::protocol::RespValue;
use crate::store::{Store, Value};

fn to_str(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

pub fn cmd_hset(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    let h = match store.get_or_create_hash(&key) {
        Ok(h) => h,
        Err(e) => return RespValue::Error(e.into()),
    };
    let mut added = 0i64;
    let mut i = 1;
    while i + 1 < args.len() {
        let field = to_str(&args[i]);
        if !h.contains_key(&field) { added += 1; }
        h.insert(field, args[i + 1].clone());
        i += 2;
    }
    RespValue::Integer(added)
}

pub fn cmd_hget(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    let field = to_str(&args[1]);
    match store.get_entry(&key) {
        None => RespValue::Null,
        Some(e) => match &e.value {
            Value::Hash(h) => match h.get(&field) {
                Some(v) => RespValue::BulkString(v.clone()),
                None => RespValue::Null,
            },
            _ => RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".into()),
        },
    }
}

pub fn cmd_hmset(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    let h = match store.get_or_create_hash(&key) {
        Ok(h) => h,
        Err(e) => return RespValue::Error(e.into()),
    };
    let mut i = 1;
    while i + 1 < args.len() {
        let field = to_str(&args[i]);
        h.insert(field, args[i + 1].clone());
        i += 2;
    }
    RespValue::SimpleString("OK".into())
}

pub fn cmd_hmget(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    let entry = store.get_entry(&key);
    let items: Vec<RespValue> = args[1..].iter().map(|a| {
        let field = to_str(a);
        match entry {
            Some(e) => match &e.value {
                Value::Hash(h) => match h.get(&field) {
                    Some(v) => RespValue::BulkString(v.clone()),
                    None => RespValue::Null,
                },
                _ => RespValue::Null,
            },
            None => RespValue::Null,
        }
    }).collect();
    RespValue::Array(items)
}

pub fn cmd_hgetall(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    match store.get_entry(&key) {
        None => RespValue::Array(vec![]),
        Some(e) => match &e.value {
            Value::Hash(h) => {
                let mut items = Vec::with_capacity(h.len() * 2);
                for (field, value) in h {
                    items.push(RespValue::BulkString(field.as_bytes().to_vec()));
                    items.push(RespValue::BulkString(value.clone()));
                }
                RespValue::Array(items)
            }
            _ => RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".into()),
        },
    }
}

pub fn cmd_hdel(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    let count = {
        let entry = match store.get_entry_mut(&key) {
            None => return RespValue::Integer(0),
            Some(e) => e,
        };
        let h = match &mut entry.value {
            Value::Hash(h) => h,
            _ => return RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".into()),
        };
        let mut c = 0i64;
        for a in &args[1..] {
            let field = to_str(a);
            if h.remove(&field).is_some() { c += 1; }
        }
        c
    };
    store.remove_if_empty(&key);
    RespValue::Integer(count)
}

pub fn cmd_hexists(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    let field = to_str(&args[1]);
    match store.get_entry(&key) {
        None => RespValue::Integer(0),
        Some(e) => match &e.value {
            Value::Hash(h) => RespValue::Integer(if h.contains_key(&field) { 1 } else { 0 }),
            _ => RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".into()),
        },
    }
}

pub fn cmd_hlen(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    match store.get_entry(&key) {
        None => RespValue::Integer(0),
        Some(e) => match &e.value {
            Value::Hash(h) => RespValue::Integer(h.len() as i64),
            _ => RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".into()),
        },
    }
}

pub fn cmd_hkeys(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    match store.get_entry(&key) {
        None => RespValue::Array(vec![]),
        Some(e) => match &e.value {
            Value::Hash(h) => {
                RespValue::Array(h.keys().map(|k| RespValue::BulkString(k.as_bytes().to_vec())).collect())
            }
            _ => RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".into()),
        },
    }
}

pub fn cmd_hvals(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    match store.get_entry(&key) {
        None => RespValue::Array(vec![]),
        Some(e) => match &e.value {
            Value::Hash(h) => {
                RespValue::Array(h.values().map(|v| RespValue::BulkString(v.clone())).collect())
            }
            _ => RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".into()),
        },
    }
}

pub fn cmd_hincrby(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    let field = to_str(&args[1]);
    let increment: i64 = to_str(&args[2]).parse().unwrap_or(0);
    let h = match store.get_or_create_hash(&key) {
        Ok(h) => h,
        Err(e) => return RespValue::Error(e.into()),
    };
    let current = h.get(&field).and_then(|v| std::str::from_utf8(v).ok()?.parse::<i64>().ok());
    match h.get(&field) {
        None => {
            h.insert(field, increment.to_string().into_bytes());
            RespValue::Integer(increment)
        }
        Some(_) => match current {
            Some(n) => {
                let new_val = n + increment;
                h.insert(field, new_val.to_string().into_bytes());
                RespValue::Integer(new_val)
            }
            None => RespValue::Error("ERR hash value is not an integer".into()),
        },
    }
}
