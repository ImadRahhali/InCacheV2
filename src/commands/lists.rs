use crate::protocol::RespValue;
use crate::store::{Store, Value};

fn to_str(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn parse_i64(b: &[u8]) -> i64 {
    to_str(b).parse().unwrap_or(0)
}

pub fn cmd_lpush(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    let d = match store.get_or_create_list(&key) {
        Ok(d) => d,
        Err(e) => return RespValue::Error(e.into()),
    };
    for v in &args[1..] {
        d.push_front(v.clone());
    }
    RespValue::Integer(d.len() as i64)
}

pub fn cmd_rpush(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    let d = match store.get_or_create_list(&key) {
        Ok(d) => d,
        Err(e) => return RespValue::Error(e.into()),
    };
    for v in &args[1..] {
        d.push_back(v.clone());
    }
    RespValue::Integer(d.len() as i64)
}

pub fn cmd_lpop(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    let val = {
        let entry = match store.get_entry_mut(&key) {
            None => return RespValue::Null,
            Some(e) => e,
        };
        match &mut entry.value {
            Value::List(d) => d.pop_front(),
            _ => return RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".into()),
        }
    };
    store.remove_if_empty(&key);
    match val {
        Some(v) => RespValue::BulkString(v),
        None => RespValue::Null,
    }
}

pub fn cmd_rpop(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    let val = {
        let entry = match store.get_entry_mut(&key) {
            None => return RespValue::Null,
            Some(e) => e,
        };
        match &mut entry.value {
            Value::List(d) => d.pop_back(),
            _ => return RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".into()),
        }
    };
    store.remove_if_empty(&key);
    match val {
        Some(v) => RespValue::BulkString(v),
        None => RespValue::Null,
    }
}

pub fn cmd_lrange(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    let start = parse_i64(&args[1]);
    let stop = parse_i64(&args[2]);
    let entry = match store.get_entry(&key) {
        None => return RespValue::Array(vec![]),
        Some(e) => e,
    };
    let d = match &entry.value {
        Value::List(d) => d,
        _ => return RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".into()),
    };
    let len = d.len() as i64;
    let s = if start < 0 { (len + start).max(0) } else { start } as usize;
    let e = if stop < 0 { len + stop } else { stop } as usize;
    if s > e || s >= d.len() {
        return RespValue::Array(vec![]);
    }
    let e = e.min(d.len() - 1);
    let items: Vec<RespValue> = d.iter()
        .skip(s)
        .take(e - s + 1)
        .map(|v| RespValue::BulkString(v.clone()))
        .collect();
    RespValue::Array(items)
}

pub fn cmd_llen(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    match store.get_entry(&key) {
        None => RespValue::Integer(0),
        Some(e) => match &e.value {
            Value::List(d) => RespValue::Integer(d.len() as i64),
            _ => RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".into()),
        },
    }
}

pub fn cmd_lindex(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    let mut index = parse_i64(&args[1]);
    let entry = match store.get_entry(&key) {
        None => return RespValue::Null,
        Some(e) => e,
    };
    let d = match &entry.value {
        Value::List(d) => d,
        _ => return RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".into()),
    };
    if index < 0 { index += d.len() as i64; }
    if index < 0 || index as usize >= d.len() {
        return RespValue::Null;
    }
    RespValue::BulkString(d[index as usize].clone())
}

pub fn cmd_lset(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    let mut index = parse_i64(&args[1]);
    let value = args[2].clone();
    let entry = match store.get_entry_mut(&key) {
        None => return RespValue::Error("ERR no such key".into()),
        Some(e) => e,
    };
    let d = match &mut entry.value {
        Value::List(d) => d,
        _ => return RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".into()),
    };
    if index < 0 { index += d.len() as i64; }
    if index < 0 || index as usize >= d.len() {
        return RespValue::Error("ERR index out of range".into());
    }
    d[index as usize] = value;
    RespValue::SimpleString("OK".into())
}

pub fn cmd_linsert(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    let where_str = to_str(&args[1]).to_uppercase();
    let pivot = &args[2];
    let value = args[3].clone();
    let entry = match store.get_entry_mut(&key) {
        None => return RespValue::Integer(0),
        Some(e) => e,
    };
    let d = match &mut entry.value {
        Value::List(d) => d,
        _ => return RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".into()),
    };
    for i in 0..d.len() {
        if &d[i] == pivot {
            let pos = if where_str == "BEFORE" { i } else { i + 1 };
            d.insert(pos, value);
            return RespValue::Integer(d.len() as i64);
        }
    }
    RespValue::Integer(-1)
}

pub fn cmd_lrem(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    let count = parse_i64(&args[1]);
    let value = &args[2];
    let removed = {
        let entry = match store.get_entry_mut(&key) {
            None => return RespValue::Integer(0),
            Some(e) => e,
        };
        let d = match &mut entry.value {
            Value::List(d) => d,
            _ => return RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".into()),
        };
        let mut removed = 0i64;
        if count > 0 {
            let mut i = 0;
            while i < d.len() {
                if &d[i] == value && removed < count {
                    d.remove(i);
                    removed += 1;
                } else {
                    i += 1;
                }
            }
        } else if count < 0 {
            let limit = count.unsigned_abs() as i64;
            let mut i = d.len();
            while i > 0 {
                i -= 1;
                if &d[i] == value && removed < limit {
                    d.remove(i);
                    removed += 1;
                }
            }
        } else {
            d.retain(|v| {
                if v == value { removed += 1; false } else { true }
            });
        }
        removed
    };
    store.remove_if_empty(&key);
    RespValue::Integer(removed)
}
